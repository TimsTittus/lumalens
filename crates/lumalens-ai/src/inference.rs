use std::sync::Mutex;

use ort::session::{Session, builder::SessionBuilder};

use crate::error::AiError;
use crate::model_registry::{ModelInfo, model_path};
use crate::{Mask, VideoFrame};

/// A loaded ONNX segmentation model, ready for inference.
pub struct SegmentationModel {
    session: Mutex<Session>,
    input_size: (u32, u32),
}

/// Loads a model from disk into an ONNX Runtime session.
///
/// Returns [`AiError::NotDownloaded`] if the file is missing.
pub fn load(info: &ModelInfo) -> Result<SegmentationModel, AiError> {
    let path = model_path(info);
    if !path.is_file() {
        return Err(AiError::NotDownloaded(info.id.to_string()));
    }

    let env = ort::init().build()?;
    let mut builder = SessionBuilder::new(&env)?;
    let session = builder.commit_from_file(&path)?;

    Ok(SegmentationModel {
        session: Mutex::new(session),
        input_size: (256, 256),
    })
}

/// Runs segmentation on a single frame, returning a foreground mask.
pub fn segment(model: &SegmentationModel, frame: &VideoFrame) -> Result<Mask, AiError> {
    let (in_w, in_h) = model.input_size;
    let in_pixels = (in_w * in_h) as usize;

    let mut rgb = vec![0.0f32; in_pixels * 3];
    for y in 0..in_h {
        for x in 0..in_w {
            let src_x = (x as u64 * frame.width as u64 / in_w as u64) as usize;
            let src_y = (y as u64 * frame.height as u64 / in_h as u64) as usize;
            let src_idx = (src_y * frame.width as usize + src_x) * 4;
            let dst_idx = (y as usize * in_w as usize + x as usize) * 3;
            rgb[dst_idx] = frame.data[src_idx] as f32 / 255.0;
            rgb[dst_idx + 1] = frame.data[src_idx + 1] as f32 / 255.0;
            rgb[dst_idx + 2] = frame.data[src_idx + 2] as f32 / 255.0;
        }
    }

    let input = ort::value::Tensor::from_array((
        [1usize, in_h as usize, in_w as usize, 3],
        rgb.into_boxed_slice(),
    ))?;

    let mut session = model.session.lock().expect("session lock poisoned");
    let outputs = session.run(ort::inputs![input])?;

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
