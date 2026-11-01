//! Events emitted by the engine back to the UI.

use super::camera::CameraInfo;

/// An event the engine sends to the UI.
pub enum EngineEvent {
    CamerasDiscovered(Vec<CameraInfo>),
    Started,
    Stopped,
    Error(String),
}
