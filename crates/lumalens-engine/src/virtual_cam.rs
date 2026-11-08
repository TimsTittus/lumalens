//! Virtual camera (v4l2loopback) support.

use std::fs;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

/// Enumerate `/dev/video*` devices and return the first whose sysfs name
/// contains "LumaLens" (case-insensitive).
pub fn find_device() -> Option<String> {
    let entries = fs::read_dir("/sys/class/video4linux").ok()?;
    for entry in entries.flatten() {
        let name_path = entry.path().join("name");
        if let Ok(name) = fs::read_to_string(&name_path) {
            if name.to_ascii_lowercase().contains("lumalens") {
                let dev_name = entry.file_name();
                return Some(format!("/dev/{}", dev_name.to_string_lossy()));
            }
        }
    }
    None
}

/// Check whether the `v4l2loopback` kernel module is currently loaded.
pub fn is_module_loaded() -> bool {
    fs::read_to_string("/proc/modules")
        .map(|s| s.lines().any(|line| line.starts_with("v4l2loopback ")))
        .unwrap_or(false)
}

/// Load the v4l2loopback module via pkexec and persist it so it loads
/// at every subsequent boot without further prompts. Returns Ok if the
/// module is loaded by the time this returns (either it already was, or
/// we just loaded it).
pub fn ensure_module_loaded() -> Result<(), String> {
    if is_module_loaded() {
        return Ok(());
    }

    // One pkexec invocation that does everything: write persistent
    // configs so the module loads on every boot, then modprobe it now.
    // The user sees a single graphical password prompt the first time;
    // after that the module is loaded at boot and this short-circuits.
    // The filename starts with `zz-` so it sorts after any pre-existing
    // v4l2loopback.conf on the system; modprobe applies options in load
    // order, so the last file wins and LumaLens always gets the right
    // card_label at boot.
    let script = r#"
set -e
printf 'v4l2loopback\n' > /etc/modules-load.d/v4l2loopback.conf
printf 'options v4l2loopback exclusive_caps=1 card_label="LumaLens" video_nr=10\n' \
    > /etc/modprobe.d/zz-lumalens-v4l2loopback.conf
# Remove the old filename if a previous build created it; harmless if absent.
rm -f /etc/modprobe.d/lumalens-v4l2loopback.conf
# Reload so the new options take effect now even if the module was
# previously loaded with different options (e.g. a different card_label).
modprobe -r v4l2loopback 2>/dev/null || true
modprobe v4l2loopback
"#;

    let status = Command::new("pkexec")
        .args(["sh", "-c", script])
        .status()
        .map_err(|e| format!("failed to invoke pkexec: {e}"))?;

    if !status.success() {
        return Err(format!("pkexec exited with status {status}"));
    }

    // udev needs a moment to create the /dev/video* node.
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if is_module_loaded() && find_device().is_some() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    if is_module_loaded() {
        Ok(())
    } else {
        Err("module did not appear after modprobe".to_string())
    }
}

/// User-facing instructions for installing the v4l2loopback module.
pub fn module_install_instructions() -> &'static str {
    "\
v4l2loopback is required for virtual camera output.\n\
\n\
Install the module for your distribution:\n\
\n\
  Arch:   sudo pacman -S v4l2loopback-dkms linux-headers\n\
  Fedora: sudo dnf install v4l2loopback\n\
  Ubuntu: sudo apt install v4l2loopback-dkms\n\
\n\
Then load it:\n\
\n\
  sudo modprobe v4l2loopback exclusive_caps=1 card_label=\"LumaLens\""
}
