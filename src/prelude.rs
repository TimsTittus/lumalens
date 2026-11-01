//! Crate-wide re-exports.

pub use crate::engine::camera::CameraInfo;
pub use crate::engine::commands::EngineCommand;
pub use crate::engine::events::EngineEvent;

/// Crate-level result type.
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
