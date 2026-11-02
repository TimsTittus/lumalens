//! Shared types used across all LumaLens crates.

/// Metadata for a discovered camera device.
#[derive(Clone, Debug)]
pub struct CameraInfo {
    pub id: String,
    pub name: String,
    pub device_path: String,
}
