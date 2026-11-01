//! Camera device information.

/// Metadata for a discovered camera device.
pub struct CameraInfo {
    pub id: String,
    pub name: String,
    pub device_path: String,
}
