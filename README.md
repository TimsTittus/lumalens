# LumaLens

> **Status: pre-alpha — not usable yet.**

LumaLens is a webcam customization studio for Linux. It gives you a live
preview of your camera with brightness, contrast, saturation, and
mirror/rotate controls, then pipes the result to a system-wide virtual
webcam (via v4l2loopback) so every app — Meet, Zoom, OBS — sees your
adjusted feed. Built with Rust, GTK4, and GStreamer.

## Build from source (Arch)

Install dependencies:

```bash
sudo pacman -S --needed \
  base-devel rust \
  gtk4 libadwaita \
  gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-libav \
  v4l-utils
```

Build and run:

```bash
cargo run
```

## License

GPL-3.0-or-later
