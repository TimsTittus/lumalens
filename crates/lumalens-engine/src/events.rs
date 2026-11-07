//! Events emitted by the engine back to the UI.

use lumalens_common::CameraInfo;

/// Pipeline performance metrics, computed every 500ms from pad probes.
#[derive(Clone, Debug, Default)]
pub struct Metrics {
    /// Measured input (capture) frame rate.
    pub input_fps: f32,
    /// Measured output (sink) frame rate.
    pub output_fps: f32,
    /// Cumulative frames dropped by leaky queues.
    pub dropped_frames: u64,
    /// Wall-clock time for the effect-processing span (ms).
    pub processing_ms: f32,
    /// Buffer depth of the preview queue.
    pub queue_depth_preview: u32,
    /// Buffer depth of the virtual-camera queue (0 when vcam is off).
    pub queue_depth_virtual: u32,
    /// ONNX inference latency when AI effects are active.
    pub ai_inference_ms: Option<f32>,
    /// Age of the most recent segmentation mask (ms).
    pub ai_mask_age_ms: Option<u32>,
}

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
    /// Live performance metrics (sent every ~500ms while playing).
    MetricsUpdated(Metrics),
}
