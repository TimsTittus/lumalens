# LumaLens

LumaLens is a webcam studio for Linux. It gives you a live preview of your
camera with real-time image adjustments, then pipes the result to a
system-wide virtual webcam so every app — Meet, Zoom, OBS — sees your
tuned feed. No browser extensions, no Electron, just native GTK4 and
GStreamer.

<!-- Replace with an actual screenshot before tagging the release -->
[LumaLens screenshot](docs/screenshot.png)

## Features

- **Live camera preview** with hardware-accelerated rendering via
  `gtk4paintablesink`.
- **Image adjustments** — brightness, contrast, and saturation sliders
  through GStreamer's `videobalance`.
- **Mirror / rotate** — six transforms via `videoflip`.
- **Resolution switching** — change capture resolution at runtime from
  camera-supported presets (640×480, 720p, 1080p).
- **Virtual camera output** — feed your adjusted video to any app via
  v4l2loopback.
- **Multi-camera support** — switch between connected cameras from a
  dropdown.
- **Image presets** — save, load, and delete named profiles. Three
  built-in presets (Natural, Warm, Cool) are included.
- **Config persistence** — settings are saved automatically and restored
  on next launch.
- **Hot-plug detection** — cameras are discovered automatically when
  plugged in and removed gracefully when unplugged.
- **Collapsible sidebar** with camera, image, preset, and output
  controls.

## Install (build from source)

### Arch Linux

```bash
sudo pacman -S --needed \
  base-devel rust \
  gtk4 libadwaita \
  gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-libav \
  v4l-utils
```

### Fedora

```bash
sudo dnf install \
  gcc rust cargo \
  gtk4-devel libadwaita-devel \
  gstreamer1-devel gstreamer1-plugins-base-devel \
  gstreamer1-plugins-good gstreamer1-plugins-bad-free \
  v4l-utils
```

### Build and run

```bash
cargo run
```

### Virtual camera (optional)

To use the virtual camera output, load the `v4l2loopback` kernel module:

```bash
sudo modprobe v4l2loopback video_nr=10 card_label="LumaLens" exclusive_caps=1
```

To persist across reboots, see the
[v4l2loopback documentation](https://github.com/umlaeute/v4l2loopback).

## Known Limitations

- **Linux only.** No macOS or Windows support.
- **v4l2loopback required** for virtual camera output — this is an
  out-of-tree kernel module. Secure Boot requires MOK enrollment.
- **Wayland tested on GNOME and KDE only.** X11 works via XWayland but
  is not the primary test target.
- **No AI effects yet.** Background blur and segmentation are planned
  but not in this release.
- **No audio handling.** LumaLens is video-only.

## Roadmap

See the [issue tracker](../../issues) for planned work. Key areas:

- Fixed-stage effect engine
- Optional AI background effects (blur, replace)
- Flatpak packaging
- AUR package

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for
development setup, code style, and PR guidelines.

## License

[GPL-3.0-or-later](LICENSE)
