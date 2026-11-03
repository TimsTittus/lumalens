//! Events emitted by the engine back to the UI.

use lumalens_common::CameraInfo;

/// An event the engine sends to the UI.
pub enum EngineEvent {
    /// Camera list was refreshed.
    CamerasDiscovered(Vec<CameraInfo>),
    /// Pipeline is now playing.
    Started,
    /// Pipeline has stopped.
    Stopped,
    /// A non-fatal error to display as a toast.
    Error(String),
}
