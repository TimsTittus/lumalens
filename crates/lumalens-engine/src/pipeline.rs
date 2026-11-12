//! GStreamer pipeline management.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

use gstreamer as gst;
use gstreamer::prelude::*;
use lumalens_effects::EffectRegistry;

use crate::errors::EngineError;
use crate::events::Metrics;

/// Pixel format negotiated with the v4l2loopback virtual camera.
pub const VIRTUAL_CAM_FORMAT: &str = "YUY2";

/// Shared counters written by pad probes, read by the metrics timer.
pub(crate) struct MetricsProbes {
    pub input_count: AtomicU64,
    pub output_count: AtomicU64,
    pub last_tick: Mutex<Instant>,
    pub processing_ns: AtomicU64,
    pub processing_samples: AtomicU64,
}

impl MetricsProbes {
    fn new() -> Self {
        Self {
            input_count: AtomicU64::new(0),
            output_count: AtomicU64::new(0),
            last_tick: Mutex::new(Instant::now()),
            processing_ns: AtomicU64::new(0),
            processing_samples: AtomicU64::new(0),
        }
    }
}

/// Owns and drives the GStreamer pipeline for a camera session.
///
/// Effect elements (correction, transform, etc.) are built by the
/// [`EffectRegistry`] and spliced into the pipeline at build time.
/// Runtime property updates go through the registry's effect objects,
/// which hold references to their live GStreamer elements.
pub struct Pipeline {
    inner: gst::Pipeline,
    res_filter: gst::Element,
    sink_ref: gst::Element,
    device_path: String,
    vcam_bin: Option<gst::Bin>,
    vcam_tee_pad: Option<gst::Pad>,
    pub(crate) metrics_probes: Arc<MetricsProbes>,
}

impl Pipeline {
    /// Assemble the GStreamer pipeline for `device_path`, the shared `sink`,
    /// and effects from `registry`.
    pub fn build(
        device_path: &str,
        sink: &gst::Element,
        registry: &mut EffectRegistry,
    ) -> Result<Self, EngineError> {
        // --- Capture stage (hard-coded) ---

        let src = if let Some(serial) = device_path.strip_prefix("pipewire:") {
            gst::ElementFactory::make("pipewiresrc")
                .name("src")
                .property("target-object", serial)
                .build()
                .map_err(|e| EngineError::PipelineBuildFailed(format!("pipewiresrc: {e}")))?
        } else {
            gst::ElementFactory::make("v4l2src")
                .name("src")
                .property("device", device_path)
                .build()
                .map_err(|e| EngineError::PipelineBuildFailed(format!("v4l2src: {e}")))?
        };

        let raw_caps = gst::Caps::builder("video/x-raw").build();
        let capsfilter = gst::ElementFactory::make("capsfilter")
            .property("caps", &raw_caps)
            .build()
            .map_err(|e| EngineError::PipelineBuildFailed(format!("capsfilter: {e}")))?;

        let convert1 = gst::ElementFactory::make("videoconvert")
            .build()
            .map_err(|e| EngineError::PipelineBuildFailed(format!("videoconvert: {e}")))?;

        let res_caps = gst::Caps::builder("video/x-raw").build();
        let res_filter = gst::ElementFactory::make("capsfilter")
            .name("res-filter")
            .property("caps", &res_caps)
            .build()
            .map_err(|e| EngineError::PipelineBuildFailed(format!("res capsfilter: {e}")))?;

        // --- Effect stages from registry ---

        let effect_elements = registry
            .build_all_effect_elements()
            .map_err(|e| EngineError::PipelineBuildFailed(e.to_string()))?;

        // --- Output stage (hard-coded) ---

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

        // --- Assemble the full chain ---

        let mut chain: Vec<&gst::Element> = vec![&src, &capsfilter, &convert1, &res_filter];
        for el in &effect_elements {
            chain.push(el);
        }
        chain.extend([&tee, &queue, &convert2, sink]);

        let pipeline = gst::Pipeline::default();
        pipeline.add_many(chain.iter().copied())?;
        gst::Element::link_many(chain.iter().copied())?;

        let probes = Arc::new(MetricsProbes::new());

        // Pad probe on the source's src pad — counts input frames.
        if let Some(pad) = src.static_pad("src") {
            let p = Arc::clone(&probes);
            pad.add_probe(gst::PadProbeType::BUFFER, move |_pad, _info| {
                p.input_count.fetch_add(1, Ordering::Relaxed);
                gst::PadProbeReturn::Ok
            });
        }

        // Pad probe on the sink's sink pad — counts output frames and
        // measures the processing span (source → sink).
        if let Some(pad) = sink.static_pad("sink") {
            let p = Arc::clone(&probes);
            pad.add_probe(gst::PadProbeType::BUFFER, move |_pad, _info| {
                p.output_count.fetch_add(1, Ordering::Relaxed);
                gst::PadProbeReturn::Ok
            });
        }

        // Probes on the effect span: before first effect element → after
        // last effect element, measuring wall-clock processing time.
        // Frames flow sequentially, so a shared atomic timestamp is safe.
        if !effect_elements.is_empty() {
            let first = &effect_elements[0];
            let last = &effect_elements[effect_elements.len() - 1];
            let enter_ns: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
            if let Some(pad) = first.static_pad("sink") {
                let ts = Arc::clone(&enter_ns);
                pad.add_probe(gst::PadProbeType::BUFFER, move |_pad, _info| {
                    ts.store(nanos_now(), Ordering::Release);
                    gst::PadProbeReturn::Ok
                });
            }
            if let Some(pad) = last.static_pad("src") {
                let ts = Arc::clone(&enter_ns);
                let p = Arc::clone(&probes);
                pad.add_probe(gst::PadProbeType::BUFFER, move |_pad, _info| {
                    let entered = ts.load(Ordering::Acquire);
                    if entered != 0 {
                        let now = nanos_now();
                        let elapsed = now.saturating_sub(entered);
                        p.processing_ns.fetch_add(elapsed, Ordering::Relaxed);
                        p.processing_samples.fetch_add(1, Ordering::Relaxed);
                    }
                    gst::PadProbeReturn::Ok
                });
            }
        }

        Ok(Self {
            inner: pipeline,
            res_filter,
            sink_ref: sink.clone(),
            device_path: device_path.to_string(),
            vcam_bin: None,
            vcam_tee_pad: None,
            metrics_probes: probes,
        })
    }

    /// Transition the pipeline to `Playing`.
    pub fn start(&self) -> Result<(), EngineError> {
        self.inner.set_state(gst::State::Playing)?;
        Ok(())
    }

    /// Transition the pipeline to `Null`.
    pub fn stop(&self) -> Result<(), EngineError> {
        self.inner.set_state(gst::State::Null)?;
        Ok(())
    }

    /// Change capture resolution at runtime. Reverts on negotiation failure.
    pub fn set_resolution(&self, width: u32, height: u32, fps: u32) -> Result<(), EngineError> {
        let prev_caps: gst::Caps = self.res_filter.property("caps");

        let new_caps = gst::Caps::builder("video/x-raw")
            .field("width", width as i32)
            .field("height", height as i32)
            .field("framerate", gst::Fraction::new(fps as i32, 1))
            .build();

        self.inner.set_state(gst::State::Ready)?;
        let _ = self.inner.state(Some(gst::ClockTime::from_seconds(5)));

        self.res_filter.set_property("caps", &new_caps);

        if let Err(e) = self.inner.set_state(gst::State::Playing) {
            let _ = self.inner.set_state(gst::State::Null);
            let _ = self.inner.state(Some(gst::ClockTime::from_seconds(2)));
            self.res_filter.set_property("caps", &prev_caps);
            let _ = self.inner.set_state(gst::State::Playing);
            return Err(EngineError::ResolutionChangeFailed(format!(
                "{width}x{height}@{fps}: {e}"
            )));
        }

        let (result, _, _) = self.inner.state(Some(gst::ClockTime::from_seconds(5)));
        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                let _ = self.inner.set_state(gst::State::Null);
                let _ = self.inner.state(Some(gst::ClockTime::from_seconds(2)));
                self.res_filter.set_property("caps", &prev_caps);
                let _ = self.inner.set_state(gst::State::Playing);
                Err(EngineError::ResolutionChangeFailed(format!(
                    "{width}x{height}@{fps}: {e}"
                )))
            }
        }
    }

    /// Returns the `gtk4paintablesink` paintable as an erased `glib::Object`.
    pub fn paintable(&self) -> gst::glib::Object {
        self.sink_ref.property("paintable")
    }

    /// Install a bus watch that forwards GStreamer errors to `on_error`.
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

    /// Dynamically attach or detach the v4l2loopback virtual camera branch.
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

            // Request a tee pad and block it before linking so no data
            // flows until the bin has finished its state transition.
            // Without this, v4l2sink receives caps queries before its
            // internal state is initialised, which crashes in the C layer.
            let tee_src = tee
                .request_pad_simple("src_%u")
                .ok_or_else(|| EngineError::PipelineBuildFailed("tee pad request failed".into()))?;

            let block_id = tee_src.add_probe(gst::PadProbeType::BLOCK_DOWNSTREAM, |_pad, _info| {
                gst::PadProbeReturn::Ok
            });

            tee_src
                .link(&ghost)
                .map_err(|e| EngineError::PipelineBuildFailed(format!("tee-vcam link: {e:?}")))?;

            if bin.sync_state_with_parent().is_err() {
                if let Some(id) = block_id {
                    tee_src.remove_probe(id);
                }
                let _ = tee_src.unlink(&ghost);
                let _ = bin.set_state(gst::State::Null);
                let _ = self.inner.remove(&bin);
                tee.release_request_pad(&tee_src);
                return Err(EngineError::PipelineBuildFailed(
                    "virtual camera failed to start".into(),
                ));
            }

            if let Some(id) = block_id {
                tee_src.remove_probe(id);
            }

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

    /// The device path this pipeline was built for.
    pub fn device_path(&self) -> &str {
        &self.device_path
    }

    /// Stop the pipeline and detach the shared sink so it can be reused.
    pub fn release_sink(&self, sink: &gst::Element) {
        let _ = self.inner.set_state(gst::State::Null);
        let _ = self.inner.state(Some(gst::ClockTime::from_seconds(3)));
        let _ = self.inner.remove(sink);
    }
}

/// Monotonic nanosecond timestamp (relative to an arbitrary epoch).
fn nanos_now() -> u64 {
    static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let epoch = EPOCH.get_or_init(Instant::now);
    epoch.elapsed().as_nanos() as u64
}

impl MetricsProbes {
    /// Snapshot current counters and reset them for the next interval.
    pub fn snapshot(&self) -> Metrics {
        let mut last = self.last_tick.lock().unwrap();
        let elapsed = last.elapsed();
        *last = Instant::now();

        let elapsed_secs = elapsed.as_secs_f32().max(0.001);
        let in_count = self.input_count.swap(0, Ordering::Relaxed);
        let out_count = self.output_count.swap(0, Ordering::Relaxed);

        let proc_ns = self.processing_ns.swap(0, Ordering::Relaxed);
        let proc_samples = self.processing_samples.swap(0, Ordering::Relaxed);
        let processing_ms = if proc_samples > 0 {
            (proc_ns as f32 / proc_samples as f32) / 1_000_000.0
        } else {
            0.0
        };

        Metrics {
            input_fps: in_count as f32 / elapsed_secs,
            output_fps: out_count as f32 / elapsed_secs,
            dropped_frames: in_count.saturating_sub(out_count),
            processing_ms,
            queue_depth_preview: 0,
            queue_depth_virtual: 0,
            ai_inference_ms: None,
            ai_mask_age_ms: None,
        }
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
