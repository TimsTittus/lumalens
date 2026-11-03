//! Shared types used across all LumaLens crates.

/// Metadata for a discovered camera device.
#[derive(Clone, Debug)]
pub struct CameraInfo {
    pub id: String,
    pub name: String,
    pub device_path: String,
}

/// Video flip / rotation method matching GStreamer's `videoflip` element.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlipMethod {
    #[default]
    None,
    HorizontalFlip,
    VerticalFlip,
    Clockwise,
    Rotate180,
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
