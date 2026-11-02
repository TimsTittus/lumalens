//! Camera device discovery.

use std::collections::HashSet;

use gst::prelude::*;
use gstreamer as gst;
use tracing::{debug, warn};

pub use lumalens_common::CameraInfo;

use crate::errors::EngineError;

/// Discover available video capture devices.
///
/// Uses direct sysfs enumeration as the primary path (reliable across
/// hot-plug cycles), then falls back to `gst::DeviceMonitor` when sysfs
/// returns nothing.
pub fn discover() -> Result<Vec<CameraInfo>, EngineError> {
    let cameras = discover_v4l2_sysfs();
    if !cameras.is_empty() {
        return Ok(cameras);
    }

    debug!("sysfs found nothing, trying DeviceMonitor");
    let cameras = discover_via_monitor();
    if cameras.is_empty() {
        return Err(EngineError::NoCamerasFound);
    }

    Ok(cameras)
}

fn discover_via_monitor() -> Vec<CameraInfo> {
    let monitor = gst::DeviceMonitor::new();

    let caps = gst::Caps::builder_full()
        .structure(gst::Structure::new_empty("video/x-raw"))
        .structure(gst::Structure::new_empty("image/jpeg"))
        .build();

    monitor.add_filter(Some("Video/Source"), Some(&caps));

    if let Err(e) = monitor.start() {
        warn!("failed to start device monitor: {e}");
        return vec![];
    }

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

    cameras
}

/// Enumerate cameras via `/sys/class/video4linux` + sysfs attributes.
///
/// This avoids creating GStreamer elements during discovery, which can fail
/// after a hot-plug cycle due to internal v4l2src state caching.
fn discover_v4l2_sysfs() -> Vec<CameraInfo> {
    let Ok(entries) = std::fs::read_dir("/sys/class/video4linux") else {
        return vec![];
    };

    let mut dev_names: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            if name.starts_with("video") {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    dev_names.sort();

    let mut seen_names = HashSet::new();
    let mut cameras = Vec::new();

    for dev_name in &dev_names {
        let sysfs = format!("/sys/class/video4linux/{dev_name}");

        if let Ok(index) = std::fs::read_to_string(format!("{sysfs}/index")) {
            if index.trim() != "0" {
                continue;
            }
        }

        let name = match std::fs::read_to_string(format!("{sysfs}/name")) {
            Ok(n) => n.trim().to_string(),
            Err(_) => continue,
        };

        if name.to_ascii_lowercase().contains("lumalens") {
            continue;
        }

        if !seen_names.insert(name.clone()) {
            continue;
        }

        let device_path = format!("/dev/{dev_name}");
        if !std::path::Path::new(&device_path).exists() {
            continue;
        }

        debug!("sysfs: found camera: {name} ({device_path})");
        cameras.push(CameraInfo {
            id: device_path.clone(),
            name,
            device_path,
        });
    }

    cameras
}
