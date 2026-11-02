# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Live camera preview with GTK4 + GStreamer pipeline.
- Camera device discovery (GStreamer DeviceMonitor + sysfs fallback).
- Brightness control via `videobalance`.
- Collapsible sidebar with camera selector dropdown and brightness slider.
- Virtual camera output via v4l2loopback with a UI toggle.
- Dedicated engine thread for the GStreamer pipeline, communicating with the
  UI over typed command/event channels.
- Automatic recovery from camera disconnections with error display.
- Hot-plug detection: cameras are re-discovered after unplug/replug.
