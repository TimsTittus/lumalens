//! GStreamer pipeline management.

use std::sync::{Arc, Condvar, Mutex};

use gstreamer as gst;
use gstreamer::prelude::*;

use lumalens_common::FlipMethod;

use crate::errors::EngineError;

pub const VIRTUAL_CAM_FORMAT: &str = "YUY2";

/// Owns and drives the GStreamer pipeline for a camera session.
///
/// The `gtk4paintablesink` element is created on the GTK main thread and
/// lent to [`build`](Self::build) via reference.  A cloned GStreamer
/// reference is stored here so that [`paintable`](Self::paintable) can be
/// called from within the engine crate without naming any GTK/GDK types.
pub struct Pipeline {
    inner: gst::Pipeline,
    balance: gst::Element,
    flip: gst::Element,
    /// Cloned GStreamer ref to the sink, held for [`paintable()`](Self::paintable).
    /// The engine thread owns the canonical reference; this clone is
    /// reference-counted at the GStreamer level and costs no extra allocation.
    sink_ref: gst::Element,
    device_path: String,
    vcam_bin: Option<gst::Bin>,
    vcam_tee_pad: Option<gst::Pad>,
}

impl Pipeline {
    pub fn build(device_path: &str, sink: &gst::Element) -> Result<Self, EngineError> {
        let src = gst::ElementFactory::make("v4l2src")
            .name("src")
            .property("device", device_path)
            .build()
            .map_err(|e| EngineError::PipelineBuildFailed(format!("v4l2src: {e}")))?;

        let raw_caps = gst::Caps::builder("video/x-raw").build();
        let capsfilter = gst::ElementFactory::make("capsfilter")
            .property("caps", &raw_caps)
            .build()
            .map_err(|e| EngineError::PipelineBuildFailed(format!("capsfilter: {e}")))?;

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

        let flip = gst::ElementFactory::make("videoflip")
            .name("flip")
            .property_from_str("method", "none")
            .build()
            .map_err(|e| EngineError::PipelineBuildFailed(format!("videoflip: {e}")))?;

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
        pipeline.add_many([
            &src,
            &capsfilter,
            &convert1,
            &balance,
            &flip,
            &tee,
            &queue,
            &convert2,
            sink,
        ])?;
        gst::Element::link_many([
            &src,
            &capsfilter,
            &convert1,
            &balance,
            &flip,
            &tee,
            &queue,
            &convert2,
            sink,
        ])?;

        Ok(Self {
            inner: pipeline,
            balance,
            flip,
            sink_ref: sink.clone(),
            device_path: device_path.to_string(),
            vcam_bin: None,
            vcam_tee_pad: None,
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

    pub fn set_contrast(&self, value: f32) {
        let clamped = value.clamp(0.0, 2.0);
        self.balance.set_property("contrast", clamped as f64);
    }

    pub fn set_saturation(&self, value: f32) {
        let clamped = value.clamp(0.0, 2.0);
        self.balance.set_property("saturation", clamped as f64);
    }

    pub fn set_flip_method(&self, method: FlipMethod) {
        self.flip
            .set_property_from_str("method", method.as_gst_str());
    }

    /// Returns the `gtk4paintablesink` paintable as an erased `glib::Object`.
    ///
    /// We return `glib::Object` rather than `gdk::Paintable` so that this
    /// crate does not need a compile-time dependency on `gtk4` or `gdk`.
    /// The caller (`lumalens-app`) performs the downcast after it has already
    /// imported `gdk`.
    pub fn paintable(&self) -> gst::glib::Object {
        self.sink_ref.property("paintable")
    }

    /// Install a bus watch that forwards GStreamer errors to `on_error`.
    ///
    /// Resource errors from v4l2src (NotFound, Read, OpenRead, Close, Busy)
    /// are mapped to [`EngineError::CameraDisconnected`]; everything else
    /// becomes [`EngineError::PipelineBuildFailed`].
    pub fn install_bus_watch(&self, on_error: impl Fn(EngineError) + Send + Sync + 'static) {
        let bus = self.inner.bus().unwrap();
        let device_path = self.device_path.clone();
        bus.set_sync_handler(move |_bus, msg| {
            if let gst::MessageView::Error(err) = msg.view() {
                let gerror = err.error();
                let src_name = msg.src().map(|s| s.name().to_string()).unwrap_or_default();
                on_error(classify_bus_error(&gerror, &src_name, &device_path));
            }
            gst::BusSyncReply::Pass
        });
    }

    pub fn enable_virtual_camera(
        &mut self,
        enabled: bool,
        device_path: &str,
    ) -> Result<(), EngineError> {
        if enabled {
            if self.vcam_bin.is_some() {
                return Ok(());
            }

            let tee = self
                .inner
                .by_name("t")
                .ok_or_else(|| EngineError::PipelineBuildFailed("tee not found".into()))?;

            let queue = gst::ElementFactory::make("queue")
                .property("max-size-buffers", 2u32)
                .property_from_str("leaky", "downstream")
                .build()
                .map_err(|e| EngineError::PipelineBuildFailed(format!("vcam queue: {e}")))?;

            let convert = gst::ElementFactory::make("videoconvert")
                .build()
                .map_err(|e| EngineError::PipelineBuildFailed(format!("vcam videoconvert: {e}")))?;

            let scale = gst::ElementFactory::make("videoscale")
                .build()
                .map_err(|e| EngineError::PipelineBuildFailed(format!("vcam videoscale: {e}")))?;

            let caps = gst::Caps::builder("video/x-raw")
                .field("format", VIRTUAL_CAM_FORMAT)
                .field("width", 1280i32)
                .field("height", 720i32)
                .field("framerate", gst::Fraction::new(30, 1))
                .build();
            let capsfilter = gst::ElementFactory::make("capsfilter")
                .property("caps", &caps)
                .build()
                .map_err(|e| EngineError::PipelineBuildFailed(format!("vcam capsfilter: {e}")))?;

            let v4l2sink = gst::ElementFactory::make("v4l2sink")
                .property("device", device_path)
                .property("sync", false)
                .build()
                .map_err(|e| EngineError::PipelineBuildFailed(format!("v4l2sink: {e}")))?;

            let bin = gst::Bin::builder().name("vcam-bin").build();
            bin.add_many([&queue, &convert, &scale, &capsfilter, &v4l2sink])?;
            gst::Element::link_many([&queue, &convert, &scale, &capsfilter, &v4l2sink])?;

            let queue_sink = queue
                .static_pad("sink")
                .ok_or_else(|| EngineError::PipelineBuildFailed("queue sink pad missing".into()))?;
            let ghost = gst::GhostPad::with_target(&queue_sink)
                .map_err(|e| EngineError::PipelineBuildFailed(format!("ghost pad: {e}")))?;
            bin.add_pad(&ghost)?;

            self.inner.add(&bin)?;

            let tee_src = tee
                .request_pad_simple("src_%u")
                .ok_or_else(|| EngineError::PipelineBuildFailed("tee pad request failed".into()))?;
            tee_src
                .link(&ghost)
                .map_err(|e| EngineError::PipelineBuildFailed(format!("tee-vcam link: {e:?}")))?;

            bin.sync_state_with_parent()?;

            self.vcam_bin = Some(bin);
            self.vcam_tee_pad = Some(tee_src);
        } else {
            let bin = match self.vcam_bin.take() {
                Some(b) => b,
                None => return Ok(()),
            };
            let tee_pad = match self.vcam_tee_pad.take() {
                Some(p) => p,
                None => return Ok(()),
            };

            // Dynamic pipeline removal using blocking pad probes.
            // https://gstreamer.freedesktop.org/documentation/application-development/advanced/pipeline-manipulation.html#dynamically-changing-the-pipeline
            let pipeline = self.inner.clone();
            let done = Arc::new((Mutex::new(false), Condvar::new()));
            let done_inner = Arc::clone(&done);

            tee_pad.add_probe(gst::PadProbeType::BLOCK_DOWNSTREAM, move |pad, _info| {
                if let Some(peer) = pad.peer() {
                    let _ = pad.unlink(&peer);
                }
                bin.send_event(gst::event::Eos::new());
                let _ = bin.set_state(gst::State::Null);
                let _ = pipeline.remove(&bin);

                let (lock, cvar) = &*done_inner;
                *lock.lock().unwrap() = true;
                cvar.notify_one();

                gst::PadProbeReturn::Remove
            });

            let (lock, cvar) = &*done;
            let mut finished = lock.lock().unwrap();
            while !*finished {
                finished = cvar.wait(finished).unwrap();
            }

            let tee = self.inner.by_name("t").unwrap();
            tee.release_request_pad(&tee_pad);
        }

        Ok(())
    }

    /// Stop the pipeline and detach the shared sink so it can be reused in a
    /// new pipeline.
    pub fn release_sink(&self, sink: &gst::Element) {
        let _ = self.inner.set_state(gst::State::Null);
        let _ = self.inner.state(Some(gst::ClockTime::from_seconds(3)));
        let _ = self.inner.remove(sink);
    }
}

fn classify_bus_error(error: &gst::glib::Error, src_name: &str, device_path: &str) -> EngineError {
    if src_name == "src" {
        if let Some(kind) = error.kind::<gst::ResourceError>() {
            if matches!(
                kind,
                gst::ResourceError::NotFound
                    | gst::ResourceError::Read
                    | gst::ResourceError::OpenRead
                    | gst::ResourceError::Close
                    | gst::ResourceError::Busy
            ) {
                return EngineError::CameraDisconnected {
                    name: device_path.to_string(),
                };
            }
        }
    }
    EngineError::PipelineBuildFailed(error.to_string())
}
