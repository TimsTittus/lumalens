//! GStreamer pipeline management.

use gstreamer as gst;
use gstreamer::prelude::*;

use super::errors::EngineError;

/// Owns and drives the GStreamer pipeline for a camera session.
///
/// The `gtk4paintablesink` element is created on the main thread (so its
/// `Paintable` stays there) and lent to [`build`](Self::build) via reference.
pub struct Pipeline {
    inner: gst::Pipeline,
    balance: gst::Element,
}

impl Pipeline {
    pub fn build(device_path: &str, sink: &gst::Element) -> Result<Self, EngineError> {
        let src = gst::ElementFactory::make("v4l2src")
            .name("src")
            .property("device", device_path)
            .build()
            .map_err(|e| EngineError::PipelineBuildFailed(format!("v4l2src: {e}")))?;

        let convert1 = gst::ElementFactory::make("videoconvert")
            .build()
            .map_err(|e| EngineError::PipelineBuildFailed(format!("videoconvert: {e}")))?;

        let balance = gst::ElementFactory::make("videobalance")
            .name("balance")
            .property("brightness", 0.0f64)
            .property("contrast", 1.0f64)
            .property("saturation", 1.0f64)
            .build()
            .map_err(|e| EngineError::PipelineBuildFailed(format!("videobalance: {e}")))?;

        let tee = gst::ElementFactory::make("tee")
            .name("t")
            .build()
            .map_err(|e| EngineError::PipelineBuildFailed(format!("tee: {e}")))?;

        let queue = gst::ElementFactory::make("queue")
            .property("max-size-buffers", 2u32)
            .property_from_str("leaky", "downstream")
            .build()
            .map_err(|e| EngineError::PipelineBuildFailed(format!("queue: {e}")))?;

        let convert2 = gst::ElementFactory::make("videoconvert")
            .build()
            .map_err(|e| EngineError::PipelineBuildFailed(format!("videoconvert: {e}")))?;

        let pipeline = gst::Pipeline::default();
        pipeline.add_many([&src, &convert1, &balance, &tee, &queue, &convert2, sink])?;
        gst::Element::link_many([&src, &convert1, &balance, &tee, &queue, &convert2, sink])?;

        Ok(Self {
            inner: pipeline,
            balance,
        })
    }

    pub fn start(&self) -> Result<(), EngineError> {
        self.inner.set_state(gst::State::Playing)?;
        Ok(())
    }

    pub fn stop(&self) -> Result<(), EngineError> {
        self.inner.set_state(gst::State::Null)?;
        Ok(())
    }

    pub fn set_brightness(&self, value: f32) {
        let clamped = value.clamp(-1.0, 1.0);
        self.balance.set_property("brightness", clamped as f64);
    }

    /// Stop the pipeline and detach the shared sink so it can be reused in a
    /// new pipeline.
    pub fn release_sink(&self, sink: &gst::Element) {
        let _ = self.inner.set_state(gst::State::Null);
        let _ = self.inner.remove(sink);
    }
}
