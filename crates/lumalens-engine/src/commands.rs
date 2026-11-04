//! Commands sent from the UI to the engine.

use lumalens_common::FlipMethod;
use lumalens_effects::Stage;

use crate::errors::EngineError;

/// A command the UI sends to the camera engine.
pub enum EngineCommand {
    /// Switch to a different camera by device path.
    SelectCamera(String),
    /// Start (or restart) the pipeline.
    Start,
    /// Stop the pipeline.
    Stop,
    /// Set the brightness level (−1.0 to 1.0).
    SetBrightness(f32),
    /// Set the contrast level (0.0 to 2.0).
    SetContrast(f32),
    /// Set the saturation level (0.0 to 2.0).
    SetSaturation(f32),
    /// Set the flip / rotation method.
    SetFlip(FlipMethod),
    /// Change the capture resolution at runtime.
    SetResolution {
        /// Frame width in pixels.
        width: u32,
        /// Frame height in pixels.
        height: u32,
        /// Frames per second.
        fps: u32,
    },
    /// Enable or disable the virtual camera output.
    EnableVirtualCamera(bool),
    /// Reorder an effect within its stage.
    ReorderEffect {
        /// The stage containing the effect.
        stage: Stage,
        /// Source position within the stage.
        from: usize,
        /// Destination position within the stage.
        to: usize,
    },
    /// Shut down the engine thread.
    Shutdown,
    /// Internal: forwarded from the pipeline bus watch.
    BusError(EngineError),
}
