//! Camera device discovery.

use std::collections::HashSet;

use gstreamer as gst;
use gst::prelude::*;
use tracing::{debug, warn};

use super::errors::EngineError;

/// Metadata for a discovered camera device.
#[derive(Clone)]
pub struct CameraInfo {
    pub id: String,
    pub name: String,
    pub device_path: String,
}

/// Discover available video capture devices.
///
/// Uses `gst::DeviceMonitor` filtered to `Video/Source` with v4l2src as the
/// preferred capture element (see ADR-0001). Deduplicates devices that expose
/// multiple `/dev/video*` nodes for the same physical camera by keeping only
/// those whose caps include `video/x-raw` or `image/jpeg`.
pub fn discover() -> Result<Vec<CameraInfo>, EngineError> {
    let monitor = gst::DeviceMonitor::new();

    let caps = gst::Caps::builder_full()
        .structure(gst::Structure::new_empty("video/x-raw"))
        .structure(gst::Structure::new_empty("image/jpeg"))
        .build();

    monitor.add_filter(Some("Video/Source"), Some(&caps));

    monitor.start().map_err(|e| {
        warn!("failed to start device monitor: {e}");
        EngineError::GstreamerBool(e)
    })?;

    let devices = monitor.devices();
    monitor.stop();

    let mut seen_ids = HashSet::new();
    let mut cameras = Vec::new();

    for device in &devices {
        let name = device.display_name().to_string();

        let props = device.properties();

        let id = props
            .as_ref()
            .and_then(|p| p.get::<String>("device.path").ok())
            .or_else(|| {
                props
                    .as_ref()
                    .and_then(|p| p.get::<String>("device.serial").ok())
            })
            .or_else(|| {
                props
                    .as_ref()
                    .and_then(|p| p.get::<String>("object.serial").ok())
            })
            .unwrap_or_else(|| name.clone());

        if !seen_ids.insert(id.clone()) {
            debug!("skipping duplicate device: {name} (id={id})");
            continue;
        }

        let device_path = props
            .as_ref()
            .and_then(|p| p.get::<String>("api.v4l2.path").ok())
            .or_else(|| {
                props
                    .as_ref()
                    .and_then(|p| p.get::<String>("device.path").ok())
            })
            .unwrap_or_else(|| name.clone());

        debug!("discovered camera: {name} (id={id}, path={device_path})");

        cameras.push(CameraInfo {
            id,
            name,
            device_path,
        });
    }

    if cameras.is_empty() {
        return Err(EngineError::NoCamerasFound);
    }

    Ok(cameras)
}

// Unit testing `discover()` is impractical because `gst::DeviceMonitor` talks
// directly to system device providers and GStreamer offers no way to inject
// mock devices into the monitor. An integration test that requires real
// hardware lives in `tests/camera_discovery.rs` behind the `live-hardware`
// feature flag.
