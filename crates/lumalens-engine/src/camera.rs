//! Camera device discovery.

use std::collections::HashSet;

use gst::prelude::*;
use gstreamer as gst;
use tracing::{debug, warn};

pub use lumalens_common::CameraInfo;
use lumalens_common::Resolution;

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

        if name.to_ascii_lowercase().contains("lumalens") {
            continue;
        }

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

        let v4l2_path = props
            .as_ref()
            .and_then(|p| p.get::<String>("api.v4l2.path").ok());

        let (device_path, supported_resolutions) = if let Some(ref path) = v4l2_path {
            (path.clone(), query_supported_resolutions(path))
        } else if let Some(serial) = props
            .as_ref()
            .and_then(|p| p.get::<String>("object.serial").ok())
        {
            let pw_path = format!("pipewire:{serial}");
            let resolutions = resolutions_from_device_caps(device);
            debug!("PipeWire camera: {name} (serial={serial})");
            (pw_path, resolutions)
        } else {
            let fallback = props
                .as_ref()
                .and_then(|p| p.get::<String>("device.path").ok())
                .unwrap_or_else(|| name.clone());
            (fallback.clone(), query_supported_resolutions(&fallback))
        };

        debug!("discovered camera: {name} (id={id}, path={device_path})");
        cameras.push(CameraInfo {
            id,
            name,
            device_path,
            supported_resolutions,
        });
    }

    cameras
}

/// Extract supported resolution presets from a GstDevice's advertised caps.
fn resolutions_from_device_caps(device: &gst::Device) -> Vec<Resolution> {
    let caps = device.caps();
    let Some(caps) = caps else {
        return vec![];
    };

    let mut seen = HashSet::new();
    let mut resolutions = Vec::new();

    for s in caps.iter() {
        if s.name() != "video/x-raw" && s.name() != "image/jpeg" {
            continue;
        }
        let width = s.get::<i32>("width").ok().map(|v| v as u32);
        let height = s.get::<i32>("height").ok().map(|v| v as u32);
        let fps = s.get::<gst::Fraction>("framerate").ok().map(|f| {
            (f.numer() as u32)
                .checked_div(f.denom() as u32)
                .unwrap_or(0)
        });

        if let (Some(w), Some(h), Some(f)) = (width, height, fps) {
            for preset in Resolution::PRESETS {
                if preset.width == w
                    && preset.height == h
                    && preset.fps == f
                    && seen.insert(*preset)
                {
                    resolutions.push(*preset);
                }
            }
        }
    }

    resolutions
}

/// Query which resolution presets a V4L2 device supports by probing its caps.
pub fn query_supported_resolutions(device_path: &str) -> Vec<Resolution> {
    let Ok(src) = gst::ElementFactory::make("v4l2src")
        .property("device", device_path)
        .build()
    else {
        warn!("failed to create v4l2src for caps query on {device_path}");
        return vec![];
    };

    if src.set_state(gst::State::Ready).is_err() {
        let _ = src.set_state(gst::State::Null);
        warn!("failed to set v4l2src to READY for {device_path}");
        return vec![];
    }

    let device_caps = match src.static_pad("src") {
        Some(pad) => pad.query_caps(None),
        None => {
            let _ = src.set_state(gst::State::Null);
            return vec![];
        }
    };

    let _ = src.set_state(gst::State::Null);

    Resolution::PRESETS
        .iter()
        .filter(|r| {
            let test = gst::Caps::builder("video/x-raw")
                .field("width", r.width as i32)
                .field("height", r.height as i32)
                .field("framerate", gst::Fraction::new(r.fps as i32, 1))
                .build();
            device_caps.can_intersect(&test)
        })
        .copied()
        .collect()
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
        let supported_resolutions = query_supported_resolutions(&device_path);
        cameras.push(CameraInfo {
            id: device_path.clone(),
            name,
            device_path,
            supported_resolutions,
        });
    }

    cameras
}
