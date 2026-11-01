//! Engine error types.

use gstreamer as gst;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("GStreamer error: {0}")]
    Gstreamer(#[from] gst::glib::Error),

    #[error("GStreamer operation failed: {0}")]
    GstreamerBool(#[from] gst::glib::BoolError),

    #[error("no cameras found")]
    NoCamerasFound,

    #[error("failed to build pipeline: {0}")]
    PipelineBuildFailed(String),
}
