//! Engine error types.

use gstreamer as gst;

/// Errors produced by the camera engine.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// A GLib-level GStreamer error.
    #[error("GStreamer error: {0}")]
    Gstreamer(#[from] gst::glib::Error),

    /// A GStreamer operation that returned a boolean failure.
    #[error("GStreamer operation failed: {0}")]
    GstreamerBool(#[from] gst::glib::BoolError),

    /// A pipeline state transition failed.
    #[error("state change failed: {0}")]
    StateChange(#[from] gst::StateChangeError),

    /// No video capture devices were found.
    #[error("no cameras found")]
    NoCamerasFound,

    /// The GStreamer pipeline could not be assembled.
    #[error("failed to build pipeline: {0}")]
    PipelineBuildFailed(String),

    /// The active camera was physically disconnected.
    #[error("camera disconnected: {name}")]
    CameraDisconnected {
        /// Device path or display name.
        name: String,
    },

    /// A runtime resolution change did not negotiate.
    #[error("resolution change failed: {0}")]
    ResolutionChangeFailed(String),
}
