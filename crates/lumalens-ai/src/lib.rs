#![warn(missing_docs)]
//! AI-powered segmentation for LumaLens.
//!
//! The segmentation model is embedded at compile time by `build.rs`
//! (downloaded once into `models/` next to the crate, then bundled).
//! All ONNX inference code lives behind the `ort` feature gate.

mod model_registry;

#[cfg(feature = "ort")]
mod inference;

pub use model_registry::{ModelInfo, available_models, is_downloaded};

#[cfg(feature = "ort")]
pub use inference::{SegmentationModel, load, segment};

mod error;
pub use error::AiError;

/// A single video frame as contiguous RGBA pixels.
pub struct VideoFrame {
    /// Raw pixel data in RGBA order.
    pub data: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// A single-channel mask the same dimensions as the input frame.
///
/// Each byte is 0–255 where 255 = foreground, 0 = background.
pub struct Mask {
    /// Raw mask data, one byte per pixel, row-major.
    pub data: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}
