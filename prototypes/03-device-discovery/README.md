# 03 – Device Discovery

Throwaway prototype to compare V4L2 vs PipeWire device visibility across
distros, so Phase 1 picks the right discovery path.

## What it does

1. Uses `gstreamer::DeviceMonitor` with a generic "Video/Source" filter (all APIs).
2. Uses `DeviceMonitor` again with caps scoped to `device.api=pipewire`.
3. Enumerates `/dev/video*` directly and shells out to `v4l2-ctl --info` for each.

For every device it prints: display name, device class, and all GStreamer
properties (api, path, bus-info, etc.).

## Run

```bash
cargo run
```

Needs: GStreamer dev libs (`gstreamer-1.0`), `v4l2-ctl` (from `v4l-utils`).

On Arch:
```bash
sudo pacman -S gstreamer gst-plugins-base v4l-utils
```

On Fedora:
```bash
sudo dnf install gstreamer1-devel gstreamer1-plugins-base v4l-utils
```

## Instructions

Run on:
1. **Arch host** — paste output below.
2. **Fedora Workstation VM** — paste output below.

We're looking for which path (V4L2 direct, GStreamer generic, GStreamer+PipeWire)
reports usable cameras in each environment.

### Arch output

```
(paste here)
```

### Fedora output

```
(paste here)
```
