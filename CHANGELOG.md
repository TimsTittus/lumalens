# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-11-03

### Added

- Live camera preview with GTK4 + GStreamer pipeline.
- Camera device discovery (sysfs primary, GStreamer DeviceMonitor fallback).
- Multi-camera support with a dropdown selector.
- Brightness, contrast, and saturation sliders via `videobalance`.
- Mirror / rotate dropdown via `videoflip`.
- Runtime resolution switching with caps-based preset filtering.
- Virtual camera output via v4l2loopback with a UI toggle.
- Image presets: save, load, and delete. Three built-in presets
  (Natural, Warm, Cool) are installed on first run.
- Configuration persistence in `$XDG_CONFIG_HOME/lumalens/config.toml`
  with atomic writes, schema versioning, and corrupt-file recovery.
- Collapsible sidebar with camera, image, preset, and output controls.
- Dedicated engine thread communicating with the UI over typed
  command/event channels.
- Automatic recovery from camera disconnections with error toasts.
- Hot-plug detection: cameras are re-discovered after unplug/replug.
- `#![warn(missing_docs)]` on all library crates.
- GitHub Actions CI for fmt, clippy, build, and test.
