use gstreamer::prelude::*;
use std::process::Command;

fn main() {
    gstreamer::init().expect("Failed to initialise GStreamer");

    println!("=== GStreamer DeviceMonitor: Video/Source (all APIs) ===\n");
    enumerate_gst_devices(None);

    println!("\n=== GStreamer DeviceMonitor: PipeWire scoped ===\n");
    enumerate_gst_devices(Some("pipewire"));

    println!("\n=== Direct /dev/video* enumeration (v4l2-ctl) ===\n");
    enumerate_v4l2_devices();
}

fn enumerate_gst_devices(api_filter: Option<&str>) {
    let monitor = gstreamer::DeviceMonitor::new();

    match api_filter {
        None => {
            monitor.add_filter(Some("Video/Source"), None);
        }
        Some(_api) => {
            let caps = gstreamer::Caps::builder("video/x-raw")
                .field("device.api", "pipewire")
                .build();
            monitor.add_filter(Some("Video/Source"), Some(&caps));
        }
    }

    if monitor.start().is_err() {
        println!("  (DeviceMonitor failed to start)");
        return;
    }

    let devices = monitor.devices();
    if devices.is_empty() {
        println!("  (no devices found)");
    }

    for dev in &devices {
        println!("  display_name : {}", dev.display_name());
        println!("  device_class : {}", dev.device_class());

        if let Some(props) = dev.properties() {
            let interesting = [
                "api.v4l2.path",
                "device.api",
                "device.path",
                "device.bus-path",
                "device.bus_path",
                "object.path",
                "node.name",
                "v4l2.device.path",
                "device.bus-info",
                "device.bus_info",
            ];
            for key in interesting {
                if let Ok(val) = props.get::<String>(key) {
                    println!("  {key:20} : {val}");
                }
            }
            // Dump all properties for discovery purposes
            println!("  --- all properties ---");
            for (idx, (key, val)) in props.iter().enumerate() {
                let _ = idx;
                println!("    {key} = {val:?}");
            }
        }
        println!();
    }

    monitor.stop();
}

fn enumerate_v4l2_devices() {
    let entries = match std::fs::read_dir("/dev") {
        Ok(e) => e,
        Err(e) => {
            println!("  Cannot read /dev: {e}");
            return;
        }
    };

    let mut video_devs: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("video") {
                Some(format!("/dev/{name}"))
            } else {
                None
            }
        })
        .collect();

    video_devs.sort();

    if video_devs.is_empty() {
        println!("  (no /dev/video* devices found)");
        return;
    }

    for dev in &video_devs {
        println!("  --- {dev} ---");
        match Command::new("v4l2-ctl")
            .args(["--device", dev, "--info"])
            .output()
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                if !stdout.is_empty() {
                    for line in stdout.lines() {
                        println!("    {line}");
                    }
                }
                if !stderr.is_empty() {
                    for line in stderr.lines() {
                        println!("    (stderr) {line}");
                    }
                }
            }
            Err(e) => println!("    v4l2-ctl failed: {e}"),
        }
        println!();
    }
}
