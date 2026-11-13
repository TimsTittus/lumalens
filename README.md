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
  v4l2loopback. The module is auto-loaded when you enable virtual
  camera output.
- **PipeWire camera support** — automatic `pipewiresrc` fallback for
  cameras exposed through PipeWire (e.g. libcamera devices).
- **Multi-camera support** — switch between connected cameras from a
  dropdown.
- **Image presets** — save, load, and delete named profiles. Three
  built-in presets (Natural, Warm, Cool) are included.
- **Effect pipeline** — fixed-stage effect chain with drag-and-drop
  reordering.
- **AI background blur** (optional) — ONNX-based person segmentation
  with real-time background blur, running on a downsampled copy for
  performance. Requires the `ai` feature flag at build time.
- **Diagnostics** — live pipeline metrics panel and one-click diagnostic
  bundle export for troubleshooting.
- **Config persistence** — settings are saved automatically and restored
  on next launch.
- **Hot-plug detection** — cameras are discovered automatically when
  plugged in and removed gracefully when unplugged.
- **Collapsible sidebar** with camera, image, preset, and output
  controls.

## Install

### Arch Linux (binary package from AUR)

```bash
# Install from the Arch User Repository (AUR)
yay -S lumalens
```

Or manually:

```bash
git clone https://aur.archlinux.org/lumalens.git
cd lumalens
makepkg -si
```

For development builds from `main`:

```bash
yay -S lumalens-git
```

### Flatpak

```bash
# Install from Flathub (once published)
flatpak install flathub io.github.timstittus.lumalens
```

> **Note:** The Flatpak build does **not** support virtual camera output.
> v4l2loopback requires kernel-level device access that the Flatpak sandbox
> cannot provide. Camera input works normally via the XDG Camera portal.
> For the full feature set including virtual camera, use the native package.
> See [ADR 0003](docs/decisions/0003-flatpak-virtual-cam.md) for details.

### Build from source

#### Arch Linux

```bash
sudo pacman -S --needed \
  base-devel rust \
  gtk4 libadwaita \
  gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-libav \
  v4l-utils
```

#### Fedora

```bash
sudo dnf install \
  gcc rust cargo \
  gtk4-devel libadwaita-devel \
  gstreamer1-devel gstreamer1-plugins-base-devel \
  gstreamer1-plugins-good gstreamer1-plugins-bad-free \
  v4l-utils
```

#### Build and run

```bash
cargo run
```

To enable AI background blur (optional), build with the `ai` feature:

```bash
cargo run --features ai
```

This pulls in the ONNX Runtime and downloads the segmentation model on
first build.

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
- **No virtual camera in Flatpak.** The sandbox blocks v4l2loopback
  access. Use the native package for virtual camera output.
- **Wayland tested on GNOME and KDE only.** X11 works via XWayland but
  is not the primary test target.
- **AI effects require the `ai` feature flag.** Background blur is
  available but requires ONNX Runtime and a compatible CPU/GPU.
- **No audio handling.** LumaLens is video-only.

## Architecture

LumaLens is a Cargo workspace with six crates:

| Crate | Purpose |
|---|---|
| `lumalens-app` | GTK4/Libadwaita UI |
| `lumalens-engine` | GStreamer pipeline, camera, virtual cam |
| `lumalens-effects` | Fixed-stage effect chain and built-in effects |
| `lumalens-ai` | ONNX segmentation (optional `ai` feature) |
| `lumalens-config` | Settings persistence |
| `lumalens-common` | Shared types and utilities |

See [ADR 0002](docs/decisions/0002-four-crate-workspace.md) for the
workspace design rationale.

## Roadmap

See the [issue tracker](../../issues) for planned work. Key areas:

- Additional AI effects (background replace, style transfer)
- Flatpak on Flathub (manifest ready, pending submission)
- More effect stages and user-authored effects

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for
development setup, code style, and PR guidelines.

## License

[GPL-3.0-or-later](LICENSE)
