//! Segmentation-stage effect: feeds frames to an ONNX inference thread.
//!
//! Uses a pad probe on an `identity` element to copy RGBA frames to a
//! dedicated inference thread (latest-frame-wins semantics) without
//! blocking the pipeline. The inference thread updates a [`SharedMask`]
//! consumed by [`BackgroundBlurEffect`](super::BackgroundBlurEffect).

use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_video as gst_video;
use tracing::{error, warn};

use crate::{Effect, EffectError, SharedMask, Stage};

struct FrameData {
    data: Vec<u8>,
    width: u32,
    height: u32,
}

struct InferenceHandle {
    _frame_tx: Arc<Mutex<mpsc::SyncSender<FrameData>>>,
    _thread: std::thread::JoinHandle<()>,
}

/// Runs the ONNX segmentation model on camera frames and publishes
/// the resulting foreground mask via a [`SharedMask`].
///
/// The GStreamer elements are kept in the pipeline unconditionally so
/// enabling/disabling this effect never forces a pipeline rebuild.
/// When `active` is false the pad probe stops pushing frames into the
/// inference thread (so no model work happens) and the shared mask is
/// cleared so downstream effects see "no mask" and pass frames through.
pub struct SegmentationEffect {
    active: Arc<AtomicBool>,
    shared_mask: Arc<SharedMask>,
    inference: Mutex<Option<InferenceHandle>>,
}

impl SegmentationEffect {
    /// Create a new disabled segmentation effect sharing `mask`.
    pub fn new(mask: Arc<SharedMask>) -> Self {
        Self {
            active: Arc::new(AtomicBool::new(false)),
            shared_mask: mask,
            inference: Mutex::new(None),
        }
    }
}

impl Effect for SegmentationEffect {
    fn id(&self) -> &'static str {
        "segmentation"
    }
    fn name(&self) -> &'static str {
        "Segmentation"
    }
    fn stage(&self) -> Stage {
        Stage::Segmentation
    }
    fn is_enabled(&self) -> bool {
        true
    }
    fn set_enabled(&mut self, v: bool) {
        self.active.store(v, Ordering::Relaxed);
        if !v {
            self.shared_mask.clear();
        }
    }

    fn build_elements(&self) -> Result<Vec<gst::Element>, EffectError> {
        let mut guard = self.inference.lock().unwrap();
        guard.take();

        let (frame_tx, frame_rx) = mpsc::sync_channel::<FrameData>(2);
        let mask = self.shared_mask.clone();
        let thread = std::thread::Builder::new()
            .name("lumalens-segmentation".into())
            .spawn(move || inference_loop(frame_rx, mask))
            .map_err(|e| EffectError::ElementCreation(e.to_string()))?;
        let frame_tx = Arc::new(Mutex::new(frame_tx));
        *guard = Some(InferenceHandle {
            _frame_tx: frame_tx.clone(),
            _thread: thread,
        });
        drop(guard);
        let active = self.active.clone();

        let convert = gst::ElementFactory::make("videoconvert")
            .build()
            .map_err(|e| EffectError::ElementCreation(format!("videoconvert: {e}")))?;

        let rgba_caps = gst::Caps::builder("video/x-raw")
            .field("format", "RGBA")
            .build();
        let capsfilter = gst::ElementFactory::make("capsfilter")
            .property("caps", &rgba_caps)
            .build()
            .map_err(|e| EffectError::ElementCreation(format!("capsfilter: {e}")))?;

        let identity = gst::ElementFactory::make("identity")
            .build()
            .map_err(|e| EffectError::ElementCreation(format!("identity: {e}")))?;

        let src_pad = identity.static_pad("src").unwrap();
        src_pad.add_probe(gst::PadProbeType::BUFFER, move |pad, info| {
            if !active.load(Ordering::Relaxed) {
                return gst::PadProbeReturn::Ok;
            }
            let Some(gst::PadProbeData::Buffer(ref buffer)) = info.data else {
                return gst::PadProbeReturn::Ok;
            };
            let Some(caps) = pad.current_caps() else {
                return gst::PadProbeReturn::Ok;
            };
            let Ok(video_info) = gst_video::VideoInfo::from_caps(&caps) else {
                return gst::PadProbeReturn::Ok;
            };
            let Ok(map) = buffer.map_readable() else {
                return gst::PadProbeReturn::Ok;
            };
            let frame = FrameData {
                data: map.as_slice().to_vec(),
                width: video_info.width(),
                height: video_info.height(),
            };
            if let Ok(tx) = frame_tx.lock() {
                let _ = tx.try_send(frame);
            }
            gst::PadProbeReturn::Ok
        });

        Ok(vec![convert, capsfilter, identity])
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

fn inference_loop(frame_rx: mpsc::Receiver<FrameData>, shared_mask: Arc<SharedMask>) {
    let models = lumalens_ai::available_models();
    if models.is_empty() {
        error!("no segmentation models registered");
        return;
    }

    let model = match lumalens_ai::load(&models[0]) {
        Ok(m) => m,
        Err(e) => {
            error!("failed to load segmentation model: {e}");
            return;
        }
    };

    loop {
        let mut frame = match frame_rx.recv() {
            Ok(f) => f,
            Err(_) => break,
        };
        while let Ok(newer) = frame_rx.try_recv() {
            frame = newer;
        }

        let video_frame = lumalens_ai::VideoFrame {
            data: frame.data,
            width: frame.width,
            height: frame.height,
        };
        match lumalens_ai::segment(&model, &video_frame) {
            Ok(mask) => shared_mask.update(mask.data, mask.width, mask.height),
            Err(e) => warn!("segmentation inference failed: {e}"),
        }
    }
}
