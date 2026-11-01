//! GStreamer pipeline management.

use std::sync::{Arc, Mutex};

use gstreamer as gst;
use gstreamer::prelude::*;
use tracing::{error, warn};

use super::errors::EngineError;

type ErrorHandler = Box<dyn Fn(EngineError) + Send + 'static>;

/// Owns and drives the GStreamer pipeline for a camera session.
pub struct Pipeline {
    inner: gst::Pipeline,
    balance: gst::Element,
    sink: gst::Element,
    error_handler: Arc<Mutex<Option<ErrorHandler>>>,
    bus_watch: Option<gst::bus::BusWatchGuard>,
}

impl Pipeline {
    pub fn build(device_path: &str) -> Result<Self, EngineError> {
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

        let sink = gst::ElementFactory::make("gtk4paintablesink")
            .name("sink")
            .build()
            .map_err(|e| EngineError::PipelineBuildFailed(format!("gtk4paintablesink: {e}")))?;

        let pipeline = gst::Pipeline::default();
        pipeline.add_many([&src, &convert1, &balance, &tee, &queue, &convert2, &sink])?;
        gst::Element::link_many([&src, &convert1, &balance, &tee, &queue, &convert2, &sink])?;

        let error_handler: Arc<Mutex<Option<ErrorHandler>>> = Arc::new(Mutex::new(None));

        let bus = pipeline.bus().ok_or_else(|| {
            EngineError::PipelineBuildFailed("failed to get pipeline bus".into())
        })?;

        let handler = Arc::clone(&error_handler);
        let bus_watch = bus
            .add_watch_local(move |_, msg| {
                match msg.view() {
                    gst::MessageView::Error(err) => {
                        let src_name = err
                            .src()
                            .map(|s| s.name().to_string())
                            .unwrap_or_default();
                        let dbg_info = err.debug().unwrap_or_default();
                        error!(element = %src_name, debug_info = %dbg_info, "{}", err.error());

                        if let Ok(guard) = handler.lock() {
                            if let Some(cb) = guard.as_ref() {
                                cb(EngineError::PipelineBuildFailed(
                                    err.error().to_string(),
                                ));
                            }
                        }
                    }
                    gst::MessageView::Warning(w) => {
                        let src_name = w
                            .src()
                            .map(|s| s.name().to_string())
                            .unwrap_or_default();
                        warn!(element = %src_name, "{}", w.error());
                    }
                    _ => {}
                }
                gst::glib::ControlFlow::Continue
            })
            .map_err(|e| EngineError::PipelineBuildFailed(format!("bus watch: {e}")))?;

        Ok(Self {
            inner: pipeline,
            balance,
            sink,
            error_handler,
            bus_watch: Some(bus_watch),
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

    pub fn set_error_handler(&self, handler: Box<dyn Fn(EngineError) + Send + 'static>) {
        if let Ok(mut guard) = self.error_handler.lock() {
            *guard = Some(handler);
        }
    }

    pub fn paintable(&self) -> gtk4::gdk::Paintable {
        self.sink.property::<gtk4::gdk::Paintable>("paintable")
    }
}
