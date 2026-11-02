//! Commands sent from the UI to the engine.

use super::errors::EngineError;

/// A command the UI sends to the camera engine.
pub enum EngineCommand {
    SelectCamera(String),
    Start,
    Stop,
    SetBrightness(f32),
    EnableVirtualCamera(bool),
    Shutdown,
    // Internal: forwarded from the pipeline bus watch.
    BusError(EngineError),
}
