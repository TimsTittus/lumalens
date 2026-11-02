//! Engine error types.

use gstreamer as gst;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("GStreamer error: {0}")]
    Gstreamer(#[from] gst::glib::Error),

    #[error("GStreamer operation failed: {0}")]
    GstreamerBool(#[from] gst::glib::BoolError),

    #[error("state change failed: {0}")]
    StateChange(#[from] gst::StateChangeError),

    #[error("no cameras found")]
    NoCamerasFound,

    #[error("failed to build pipeline: {0}")]
    PipelineBuildFailed(String),

    #[error("camera disconnected: {name}")]
    CameraDisconnected { name: String },
}
