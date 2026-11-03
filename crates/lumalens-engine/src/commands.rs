//! Commands sent from the UI to the engine.

use lumalens_common::FlipMethod;

use crate::errors::EngineError;

/// A command the UI sends to the camera engine.
pub enum EngineCommand {
    SelectCamera(String),
    Start,
    Stop,
    SetBrightness(f32),
    SetContrast(f32),
    SetSaturation(f32),
    SetFlip(FlipMethod),
    EnableVirtualCamera(bool),
    Shutdown,
    // Internal: forwarded from the pipeline bus watch.
    BusError(EngineError),
}
