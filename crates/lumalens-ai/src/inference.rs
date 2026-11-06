use std::sync::Mutex;

use ort::session::builder::SessionBuilder;

use crate::error::AiError;
use crate::model_registry::ModelInfo;
use crate::{Mask, VideoFrame};

// Embedded at compile time by build.rs — always present, no runtime download.
static MODEL_BYTES: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/selfie-segmentation.onnx"
));

/// A loaded ONNX segmentation model, ready for inference.
pub struct SegmentationModel {
    session: Mutex<ort::session::Session>,
    input_size: (u32, u32),
}

/// Loads the embedded model into an ONNX Runtime session.
///
/// The model is compiled into the binary by `build.rs`; no file I/O occurs.
pub fn load(_info: &ModelInfo) -> Result<SegmentationModel, AiError> {
    let env = ort::init().build()?;
    let mut builder = SessionBuilder::new(&env)?;
    let session = builder.commit_from_memory(MODEL_BYTES)?;

    Ok(SegmentationModel {
        session: Mutex::new(session),
        input_size: (256, 256),
    })
}

/// Runs segmentation on a single frame, returning a foreground mask.
pub fn segment(model: &SegmentationModel, frame: &VideoFrame) -> Result<Mask, AiError> {
    let (in_w, in_h) = model.input_size;
    let in_pixels = (in_w * in_h) as usize;

    // NCHW layout: [1, 3, 256, 256], float32 in [0, 1]
    let mut nchw = vec![0.0f32; in_pixels * 3];
    let plane_size = in_pixels;
    for y in 0..in_h {
        for x in 0..in_w {
            let src_x = (x as u64 * frame.width as u64 / in_w as u64) as usize;
            let src_y = (y as u64 * frame.height as u64 / in_h as u64) as usize;
            let src_idx = (src_y * frame.width as usize + src_x) * 4; // RGBA
            let pixel = y as usize * in_w as usize + x as usize;
            nchw[pixel] = frame.data[src_idx] as f32 / 255.0;                     // R plane
            nchw[plane_size + pixel] = frame.data[src_idx + 1] as f32 / 255.0;    // G plane
            nchw[2 * plane_size + pixel] = frame.data[src_idx + 2] as f32 / 255.0; // B plane
        }
    }

    let input = ort::value::Tensor::from_array((
        [1usize, 3, in_h as usize, in_w as usize],
        nchw.into_boxed_slice(),
    ))?;

    let mut session = model.session.lock().expect("session lock poisoned");
    let outputs = session.run(ort::inputs![input])?;

    // Output: [1, 1, 256, 256] alpha matte, float32 in [0, 1]
    let (_shape, raw) = outputs[0].try_extract_tensor::<f32>()?;

    let out_pixels = (frame.width * frame.height) as usize;
    let mut mask_data = vec![0u8; out_pixels];
    for y in 0..frame.height {
        for x in 0..frame.width {
            let src_x = (x as u64 * in_w as u64 / frame.width as u64) as usize;
            let src_y = (y as u64 * in_h as u64 / frame.height as u64) as usize;
            let idx = src_y * in_w as usize + src_x;
            let confidence = raw.get(idx).copied().unwrap_or(0.0);
            mask_data[(y * frame.width + x) as usize] = (confidence.clamp(0.0, 1.0) * 255.0) as u8;
        }
    }

    Ok(Mask {
        data: mask_data,
        width: frame.width,
        height: frame.height,
    })
}
