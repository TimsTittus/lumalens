//! Virtual camera (v4l2loopback) support.

use std::fs;

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
