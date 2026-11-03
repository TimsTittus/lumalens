//! Shared types used across all LumaLens crates.
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};

/// A discrete resolution + frame-rate combination a camera can produce.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Resolution {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Frames per second.
    pub fps: u32,
}

impl Resolution {
    /// Built-in resolution presets offered in the UI.
    pub const PRESETS: &[Resolution] = &[
        Resolution {
            width: 640,
            height: 480,
            fps: 30,
        },
        Resolution {
            width: 1280,
            height: 720,
            fps: 30,
        },
        Resolution {
            width: 1280,
            height: 720,
            fps: 60,
        },
        Resolution {
            width: 1920,
            height: 1080,
            fps: 30,
        },
    ];

    /// Human-readable label, e.g. `"1280x720 @ 30 fps"`.
    pub fn label(self) -> String {
        format!("{}x{} @ {} fps", self.width, self.height, self.fps)
    }
}

/// Metadata for a discovered camera device.
#[derive(Clone, Debug)]
pub struct CameraInfo {
    /// Stable identifier used for deduplication.
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// `/dev/videoN` path.
    pub device_path: String,
    /// Resolution presets the camera supports in `video/x-raw`.
    pub supported_resolutions: Vec<Resolution>,
}

/// Video flip / rotation method matching GStreamer's `videoflip` element.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlipMethod {
    /// No transform.
    #[default]
    #[serde(rename = "none")]
    None,
    /// Mirror horizontally.
    #[serde(rename = "horizontal-flip")]
    HorizontalFlip,
    /// Mirror vertically.
    #[serde(rename = "vertical-flip")]
    VerticalFlip,
    /// Rotate 90° clockwise.
    #[serde(rename = "clockwise")]
    Clockwise,
    /// Rotate 180°.
    #[serde(rename = "rotate-180")]
    Rotate180,
    /// Rotate 90° counter-clockwise.
    #[serde(rename = "counterclockwise")]
    Counterclockwise,
}

impl FlipMethod {
    /// All variants in UI display order.
    pub const ALL: [FlipMethod; 6] = [
        FlipMethod::None,
        FlipMethod::HorizontalFlip,
        FlipMethod::VerticalFlip,
        FlipMethod::Clockwise,
        FlipMethod::Rotate180,
        FlipMethod::Counterclockwise,
    ];

    /// GStreamer `videoflip` `method` property value.
    pub fn as_gst_str(self) -> &'static str {
        match self {
            FlipMethod::None => "none",
            FlipMethod::HorizontalFlip => "horizontal-flip",
            FlipMethod::VerticalFlip => "vertical-flip",
            FlipMethod::Clockwise => "clockwise",
            FlipMethod::Rotate180 => "rotate-180",
            FlipMethod::Counterclockwise => "counterclockwise",
        }
    }

    /// Human-readable label for the UI dropdown.
    pub fn label(self) -> &'static str {
        match self {
            FlipMethod::None => "None",
            FlipMethod::HorizontalFlip => "Horizontal Flip",
            FlipMethod::VerticalFlip => "Vertical Flip",
            FlipMethod::Clockwise => "Rotate 90° CW",
            FlipMethod::Rotate180 => "Rotate 180°",
            FlipMethod::Counterclockwise => "Rotate 90° CCW",
        }
    }
}
