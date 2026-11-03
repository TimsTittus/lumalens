//! Shared types used across all LumaLens crates.

use serde::{Deserialize, Serialize};

/// A discrete resolution + frame-rate combination a camera can produce.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

impl Resolution {
    pub const PRESETS: &[Resolution] = &[
        Resolution { width: 640, height: 480, fps: 30 },
        Resolution { width: 1280, height: 720, fps: 30 },
        Resolution { width: 1280, height: 720, fps: 60 },
        Resolution { width: 1920, height: 1080, fps: 30 },
    ];

    pub fn label(self) -> String {
        format!("{}x{} @ {} fps", self.width, self.height, self.fps)
    }
}

/// Metadata for a discovered camera device.
#[derive(Clone, Debug)]
pub struct CameraInfo {
    pub id: String,
    pub name: String,
    pub device_path: String,
    pub supported_resolutions: Vec<Resolution>,
}

/// Video flip / rotation method matching GStreamer's `videoflip` element.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlipMethod {
    #[default]
    #[serde(rename = "none")]
    None,
    #[serde(rename = "horizontal-flip")]
    HorizontalFlip,
    #[serde(rename = "vertical-flip")]
    VerticalFlip,
    #[serde(rename = "clockwise")]
    Clockwise,
    #[serde(rename = "rotate-180")]
    Rotate180,
    #[serde(rename = "counterclockwise")]
    Counterclockwise,
}

impl FlipMethod {
    pub const ALL: [FlipMethod; 6] = [
        FlipMethod::None,
        FlipMethod::HorizontalFlip,
        FlipMethod::VerticalFlip,
        FlipMethod::Clockwise,
        FlipMethod::Rotate180,
        FlipMethod::Counterclockwise,
    ];

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
