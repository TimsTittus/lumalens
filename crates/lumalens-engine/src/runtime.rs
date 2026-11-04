//! Engine runtime — owns the GStreamer pipeline on a dedicated thread.
//!
//! Two channel types bridge the UI and engine threads:
//! - `std::sync::mpsc::sync_channel` for commands (UI → engine): the engine
//!   thread blocks on `recv()`, which needs no GLib main loop.
//! - `std::sync::mpsc::channel` for events (engine → UI): the GTK main thread
//!   polls this with a `glib::timeout_add_local` source so that event handlers
//!   can safely touch widgets without `Send` bounds.

use std::sync::{Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use gstreamer as gst;
use lumalens_effects::EffectRegistry;
use lumalens_effects::correction::{BrightnessEffect, ContrastEffect, SaturationEffect};
use lumalens_effects::transform::FlipEffect;
use tracing::{debug, error, warn};

use crate::camera;
use crate::commands::EngineCommand;
use crate::errors::EngineError;
use crate::events::EngineEvent;
use crate::pipeline::Pipeline;

/// Handle to the running engine thread.
pub struct Engine {
    cmd_tx: mpsc::SyncSender<EngineCommand>,
    _thread: JoinHandle<()>,
}

impl Engine {
    /// Send a command to the engine thread.
    pub fn send(&self, cmd: EngineCommand) {
        if let Err(e) = self.cmd_tx.try_send(cmd) {
            error!("failed to send engine command: {e}");
        }
    }

    /// Stop the engine and join its thread.
    pub fn shutdown(self) {
        let _ = self.cmd_tx.send(EngineCommand::Shutdown);
        let _ = self._thread.join();
    }
}

/// Start the engine on a dedicated thread, returning a handle and event receiver.
pub fn spawn(sink: gst::Element) -> (Engine, mpsc::Receiver<EngineEvent>) {
    let (cmd_tx, cmd_rx) = mpsc::sync_channel::<EngineCommand>(32);
    let (event_tx, event_rx) = mpsc::channel::<EngineEvent>();

    let cmd_tx_clone = cmd_tx.clone();
    let handle = thread::Builder::new()
        .name("lumalens-engine".into())
        .spawn(move || engine_loop(cmd_rx, cmd_tx_clone, event_tx, sink))
        .expect("failed to spawn engine thread");

    let engine = Engine {
        cmd_tx,
        _thread: handle,
    };

    (engine, event_rx)
}

fn emit(tx: &mpsc::Sender<EngineEvent>, event: EngineEvent) {
    if tx.send(event).is_err() {
        error!("event channel closed");
    }
}

fn build_pipeline(
    device: &str,
    sink: &gst::Element,
    registry: &mut EffectRegistry,
    cmd_tx: &mpsc::SyncSender<EngineCommand>,
    event_tx: &mpsc::Sender<EngineEvent>,
) -> Option<Pipeline> {
    match Pipeline::build(device, sink, registry) {
        Ok(pipe) => {
            let tx = Mutex::new(cmd_tx.clone());
            pipe.install_bus_watch(move |err| {
                let _ = tx.lock().unwrap().try_send(EngineCommand::BusError(err));
            });
            Some(pipe)
        }
        Err(e) => {
            warn!("failed to build pipeline: {e}");
            emit(event_tx, EngineEvent::Error(e.to_string()));
            None
        }
    }
}

fn engine_loop(
    cmd_rx: mpsc::Receiver<EngineCommand>,
    cmd_tx: mpsc::SyncSender<EngineCommand>,
    event_tx: mpsc::Sender<EngineEvent>,
    sink: gst::Element,
) {
    let mut pipeline: Option<Pipeline> = None;
    let mut registry = EffectRegistry::with_defaults();
    let mut stashed: Option<EngineCommand> = None;
    let mut last_camera_count: usize = 0;

    loop {
        let timeout = if pipeline.is_none() {
            Duration::from_secs(2)
        } else {
            Duration::from_secs(5)
        };

        let cmd = if let Some(cmd) = stashed.take() {
            cmd
        } else {
            match cmd_rx.recv_timeout(timeout) {
                Ok(cmd) => cmd,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if pipeline.is_none() {
                        try_auto_discover(
                            &sink,
                            &cmd_tx,
                            &event_tx,
                            &mut registry,
                            &mut pipeline,
                            &mut last_camera_count,
                        );
                    } else {
                        refresh_camera_list(&event_tx, &mut last_camera_count);
                    }
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        };

        match cmd {
            EngineCommand::Start => {
                if pipeline.is_none() {
                    match camera::discover() {
                        Ok(cameras) => {
                            let device = cameras[0].device_path.clone();
                            emit(&event_tx, EngineEvent::CamerasDiscovered(cameras));
                            pipeline =
                                build_pipeline(&device, &sink, &mut registry, &cmd_tx, &event_tx);
                        }
                        Err(e) => {
                            warn!("camera discovery failed: {e}");
                            emit(&event_tx, EngineEvent::Error(e.to_string()));
                            emit(&event_tx, EngineEvent::CamerasDiscovered(vec![]));
                            continue;
                        }
                    }
                }
                if let Some(ref pipe) = pipeline {
                    match pipe.start() {
                        Ok(()) => emit(&event_tx, EngineEvent::Started),
                        Err(e) => {
                            warn!("pipeline start failed: {e}");
                            emit(&event_tx, EngineEvent::Error(e.to_string()));
                        }
                    }
                }
            }

            EngineCommand::Stop => {
                if let Some(ref pipe) = pipeline {
                    match pipe.stop() {
                        Ok(()) => emit(&event_tx, EngineEvent::Stopped),
                        Err(e) => {
                            warn!("pipeline stop failed: {e}");
                            emit(&event_tx, EngineEvent::Error(e.to_string()));
                        }
                    }
                }
            }

            EngineCommand::SelectCamera(device_path) => {
                if let Some(ref pipe) = pipeline {
                    pipe.release_sink(&sink);
                }

                pipeline =
                    build_pipeline(&device_path, &sink, &mut registry, &cmd_tx, &event_tx);
                if let Some(ref pipe) = pipeline {
                    match pipe.start() {
                        Ok(()) => emit(&event_tx, EngineEvent::Started),
                        Err(e) => {
                            warn!("pipeline start failed after camera switch: {e}");
                            emit(&event_tx, EngineEvent::Error(e.to_string()));
                        }
                    }
                }
            }

            EngineCommand::SetBrightness(mut value) => {
                while let Ok(next) = cmd_rx.try_recv() {
                    match next {
                        EngineCommand::SetBrightness(v) => value = v,
                        other => {
                            stashed = Some(other);
                            break;
                        }
                    }
                }
                if let Some(effect) = registry.effect_mut::<BrightnessEffect>("brightness") {
                    effect.set_value(value as f64);
                }
            }

            EngineCommand::SetContrast(mut value) => {
                while let Ok(next) = cmd_rx.try_recv() {
                    match next {
                        EngineCommand::SetContrast(v) => value = v,
                        other => {
                            stashed = Some(other);
                            break;
                        }
                    }
                }
                if let Some(effect) = registry.effect_mut::<ContrastEffect>("contrast") {
                    effect.set_value(value as f64);
                }
            }

            EngineCommand::SetSaturation(mut value) => {
                while let Ok(next) = cmd_rx.try_recv() {
                    match next {
                        EngineCommand::SetSaturation(v) => value = v,
                        other => {
                            stashed = Some(other);
                            break;
                        }
                    }
                }
                if let Some(effect) = registry.effect_mut::<SaturationEffect>("saturation") {
                    effect.set_value(value as f64);
                }
            }

            EngineCommand::SetFlip(method) => {
                if let Some(effect) = registry.effect_mut::<FlipEffect>("flip") {
                    effect.set_method(method);
                }
            }

            EngineCommand::SetResolution { width, height, fps } => {
                if let Some(ref pipe) = pipeline {
                    if let Err(e) = pipe.set_resolution(width, height, fps) {
                        warn!("resolution change failed: {e}");
                        emit(&event_tx, EngineEvent::Error(e.to_string()));
                    }
                }
            }

            EngineCommand::EnableVirtualCamera(enabled) => {
                if let Some(ref mut pipe) = pipeline {
                    if enabled {
                        match crate::virtual_cam::find_device() {
                            Some(device) => {
                                if let Err(e) = pipe.enable_virtual_camera(true, &device) {
                                    warn!("failed to enable virtual camera: {e}");
                                    emit(&event_tx, EngineEvent::Error(e.to_string()));
                                }
                            }
                            None => {
                                warn!("virtual camera device not found");
                                emit(
                                    &event_tx,
                                    EngineEvent::Error(
                                        crate::virtual_cam::module_install_instructions()
                                            .to_string(),
                                    ),
                                );
                            }
                        }
                    } else if let Err(e) = pipe.enable_virtual_camera(false, "") {
                        warn!("failed to disable virtual camera: {e}");
                        emit(&event_tx, EngineEvent::Error(e.to_string()));
                    }
                }
            }

            EngineCommand::ReorderEffect { stage, from, to } => {
                registry.reorder(stage, from, to);
            }

            EngineCommand::BusError(err) => {
                if pipeline.is_none() {
                    continue;
                }

                if let Some(ref pipe) = pipeline {
                    pipe.release_sink(&sink);
                }
                pipeline = None;

                loop {
                    match cmd_rx.try_recv() {
                        Ok(EngineCommand::BusError(_)) => continue,
                        Ok(other) => {
                            stashed = Some(other);
                            break;
                        }
                        Err(_) => break,
                    }
                }

                last_camera_count = 0;

                match err {
                    EngineError::CameraDisconnected { ref name } => {
                        warn!("camera disconnected: {name}");
                        emit(
                            &event_tx,
                            EngineEvent::Error(format!("Camera disconnected: {name}")),
                        );
                        emit(&event_tx, EngineEvent::CamerasDiscovered(vec![]));
                    }
                    other => {
                        warn!("pipeline error: {other}");
                        emit(&event_tx, EngineEvent::Error(other.to_string()));
                    }
                }
            }

            EngineCommand::Shutdown => {
                if let Some(ref pipe) = pipeline {
                    pipe.release_sink(&sink);
                }
                break;
            }
        }
    }
}

fn try_auto_discover(
    sink: &gst::Element,
    cmd_tx: &mpsc::SyncSender<EngineCommand>,
    event_tx: &mpsc::Sender<EngineEvent>,
    registry: &mut EffectRegistry,
    pipeline: &mut Option<Pipeline>,
    last_camera_count: &mut usize,
) {
    debug!("polling for cameras…");
    let cameras = match camera::discover() {
        Ok(c) if !c.is_empty() => c,
        Ok(_) => {
            debug!("no cameras found, will retry");
            return;
        }
        Err(e) => {
            debug!("discovery failed: {e}");
            return;
        }
    };

    let device = cameras[0].device_path.clone();

    let pipe = match Pipeline::build(&device, sink, registry) {
        Ok(p) => p,
        Err(e) => {
            warn!("auto-discover: pipeline build failed: {e}");
            return;
        }
    };

    let tx = Mutex::new(cmd_tx.clone());
    pipe.install_bus_watch(move |err| {
        let _ = tx.lock().unwrap().try_send(EngineCommand::BusError(err));
    });

    match pipe.start() {
        Ok(()) => {
            *last_camera_count = cameras.len();
            *pipeline = Some(pipe);
            emit(event_tx, EngineEvent::CamerasDiscovered(cameras));
            emit(event_tx, EngineEvent::Started);
        }
        Err(e) => {
            warn!("auto-discover: start failed (will retry): {e}");
            pipe.release_sink(sink);
        }
    }
}

/// Re-scan cameras while a pipeline is already running and update the UI
/// dropdown if the set of available cameras changed.
fn refresh_camera_list(event_tx: &mpsc::Sender<EngineEvent>, last_camera_count: &mut usize) {
    let count = match camera::discover() {
        Ok(c) => c.len(),
        Err(_) => 0,
    };

    if count != *last_camera_count {
        *last_camera_count = count;
        match camera::discover() {
            Ok(cameras) => emit(event_tx, EngineEvent::CamerasDiscovered(cameras)),
            Err(_) => emit(event_tx, EngineEvent::CamerasDiscovered(vec![])),
        }
    }
}
