//! Engine runtime — owns the GStreamer pipeline on a dedicated thread.
//!
//! Two channel types bridge the UI and engine threads:
//! - `std::sync::mpsc::sync_channel` for commands (UI → engine): the engine
//!   thread blocks on `recv()`, which needs no GLib main loop.
//! - `std::sync::mpsc::channel` for events (engine → UI): the GTK main thread
//!   polls this with a `glib::timeout_add_local` source so that event handlers
//!   can safely touch widgets without `Send` bounds.

use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use gstreamer as gst;
use tracing::error;

use super::camera;
use super::commands::EngineCommand;
use super::events::EngineEvent;
use super::pipeline::Pipeline;

pub struct Engine {
    cmd_tx: mpsc::SyncSender<EngineCommand>,
    _thread: JoinHandle<()>,
}

impl Engine {
    pub fn send(&self, cmd: EngineCommand) {
        if let Err(e) = self.cmd_tx.try_send(cmd) {
            error!("failed to send engine command: {e}");
        }
    }

    pub fn shutdown(self) {
        let _ = self.cmd_tx.send(EngineCommand::Shutdown);
        let _ = self._thread.join();
    }
}

pub fn spawn(sink: gst::Element) -> (Engine, mpsc::Receiver<EngineEvent>) {
    let (cmd_tx, cmd_rx) = mpsc::sync_channel::<EngineCommand>(32);
    let (event_tx, event_rx) = mpsc::channel::<EngineEvent>();

    let handle = thread::Builder::new()
        .name("lumalens-engine".into())
        .spawn(move || engine_loop(cmd_rx, event_tx, sink))
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
    event_tx: &mpsc::Sender<EngineEvent>,
) -> Option<Pipeline> {
    match Pipeline::build(device, sink) {
        Ok(pipe) => Some(pipe),
        Err(e) => {
            emit(event_tx, EngineEvent::Error(e.to_string()));
            None
        }
    }
}

fn engine_loop(
    cmd_rx: mpsc::Receiver<EngineCommand>,
    event_tx: mpsc::Sender<EngineEvent>,
    sink: gst::Element,
) {
    let mut pipeline: Option<Pipeline> = None;
    let mut stashed: Option<EngineCommand> = None;

    loop {
        let cmd = if let Some(cmd) = stashed.take() {
            cmd
        } else {
            match cmd_rx.recv() {
                Ok(cmd) => cmd,
                Err(_) => break,
            }
        };

        match cmd {
            EngineCommand::Start => {
                if pipeline.is_none() {
                    match camera::discover() {
                        Ok(cameras) => {
                            let device = cameras[0].device_path.clone();
                            emit(&event_tx, EngineEvent::CamerasDiscovered(cameras));
                            pipeline = build_pipeline(&device, &sink, &event_tx);
                        }
                        Err(e) => {
                            emit(&event_tx, EngineEvent::Error(e.to_string()));
                            continue;
                        }
                    }
                }
                if let Some(ref pipe) = pipeline {
                    match pipe.start() {
                        Ok(()) => emit(&event_tx, EngineEvent::Started),
                        Err(e) => emit(&event_tx, EngineEvent::Error(e.to_string())),
                    }
                }
            }

            EngineCommand::Stop => {
                if let Some(ref pipe) = pipeline {
                    match pipe.stop() {
                        Ok(()) => emit(&event_tx, EngineEvent::Stopped),
                        Err(e) => emit(&event_tx, EngineEvent::Error(e.to_string())),
                    }
                }
            }

            EngineCommand::SelectCamera(device_path) => {
                if let Some(ref pipe) = pipeline {
                    pipe.release_sink(&sink);
                }
                pipeline = None;

                pipeline = build_pipeline(&device_path, &sink, &event_tx);
                if let Some(ref pipe) = pipeline {
                    match pipe.start() {
                        Ok(()) => emit(&event_tx, EngineEvent::Started),
                        Err(e) => emit(&event_tx, EngineEvent::Error(e.to_string())),
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
                if let Some(ref pipe) = pipeline {
                    pipe.set_brightness(value);
                }
            }

            EngineCommand::EnableVirtualCamera(_) => {
                // TODO
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
