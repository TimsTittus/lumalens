# ADR 0001 — Phase 0 De-risking Findings

**Status:** Accepted

**Date:** 2026-11-01

---

## Context

Before committing to a full implementation, we built three throwaway
prototypes to validate the core technical risks on our target stack
(Rust + GTK4 + GStreamer + v4l2loopback, running on Arch and Fedora):

1. **01-preview-sink** — Can `gtk4paintablesink` render a live V4L2
   webcam feed on Wayland + GTK4?
2. **02-virtual-cam** — Can we write frames to a v4l2loopback device in
   a format that Chromium, Firefox, and Zoom all accept?
3. **03-device-discovery** — Which cameras appear via V4L2 direct
   enumeration vs GStreamer DeviceMonitor vs PipeWire-scoped queries?

---

## Decisions

### 1. GTK4 + gtk4paintablesink: VIABLE

Prototype 01 confirmed that the `gtk4paintablesink` element (from
`gst-plugin-gtk4` 0.13) renders a live V4L2 feed cleanly in an
`AdwApplicationWindow` on Wayland. The pipeline
`v4l2src ! videoconvert ! gtk4paintablesink` works without
`GDK_BACKEND=x11`, no tearing observed, and the device is released
cleanly on window close.

**Decision:** Use `gtk4paintablesink` as the preview renderer. Embed its
paintable in a `gtk::Picture` widget inside an Adwaita window.

### 2. Virtual camera format: YUY2

Prototype 02 tested writing frames to `/dev/video10` (v4l2loopback) with
`format=YUY2,width=1280,height=720,framerate=30/1`. The pipeline used
`v4l2src ! jpegdec ! videoconvert ! video/x-raw,format=YUY2 ! tee` with
one branch to `fakesink` and one to `filesink location=/dev/video10`.

YUY2 at 1280×720@30fps was accepted by Chromium (Google Meet), Firefox
(Google Meet), and Zoom (web client via Chromium) when `exclusive_caps=1`
was set on the v4l2loopback module. The device appeared as "LumaLens
Test" in all three.

**Decision:** Use YUY2 as the default virtual camera format.
`VIRTUAL_CAM_FORMAT = "YUY2"`. If a consumer application is later found
to reject YUY2, NV12 and I420 are fallback candidates — NV12 is the
next choice due to lower bandwidth than I420.

### 3. Default capture source: v4l2src

Prototype 03 ran `gstreamer::DeviceMonitor` with two filters: a generic
"Video/Source" filter (no caps) and a PipeWire-scoped filter
(`video/x-raw` with `device.api=pipewire`). On Arch:

- **GStreamer DeviceMonitor (generic):** no devices found.
- **GStreamer DeviceMonitor (PipeWire-scoped):** no devices found.
- **Direct `/dev/video*` + `v4l2-ctl --info`:** found `/dev/video10`
  (the v4l2loopback device) with correct capabilities.

The DeviceMonitor returning empty while V4L2 direct enumeration succeeds
indicates that the GStreamer monitor depends on running providers
(PipeWire session, or a V4L2 monitor provider being loaded). On a
desktop session with PipeWire active, DeviceMonitor should find real
cameras — but it cannot be the sole discovery path.

**Decision:** Default to `v4l2src` as the capture element. Use
`gstreamer::DeviceMonitor` filtered to "Video/Source" as the primary
discovery mechanism (it works on PipeWire-enabled desktops like Fedora
Workstation), but add a fallback that enumerates `/dev/video*` directly
and probes each with V4L2 ioctls to catch devices the monitor misses.
Do not use `pipewiresrc` as the default — it adds a hard PipeWire
dependency and the V4L2 path covers both environments.

### 4. Secure Boot caveat for v4l2loopback

v4l2loopback is an out-of-tree kernel module. On systems with Secure
Boot enabled, loading unsigned kernel modules is blocked by default.
Users must either:

- Enroll a Machine Owner Key (MOK) and sign the module, or
- Disable Secure Boot.

This affects the virtual camera feature specifically. The preview
pipeline (`v4l2src ! gtk4paintablesink`) does not require v4l2loopback
and works regardless of Secure Boot status.

**Decision:** Document the Secure Boot requirement prominently in the
README, the AUR `.install` script, and the virtual camera toggle's error
message. Do not attempt to auto-load or auto-sign the module — surface
clear instructions and link to the Arch/Fedora wiki pages for MOK
enrollment. The Flatpak build (Phase 5) will not support virtual camera
output at all due to sandbox restrictions; this is a separate limitation
documented in ADR 0003.

---

## Consequences

- The preview stack is confirmed: GTK4 + libadwaita + gtk4paintablesink
  on Wayland. No X11 fallback needed for preview.
- Virtual camera output targets YUY2. The pipeline will include
  `videoconvert` before the v4l2loopback sink to handle format
  negotiation from any source format.
- Camera discovery uses a two-tier approach: DeviceMonitor first, direct
  V4L2 enumeration as fallback. This adds ~50 lines of code but covers
  edge cases on headless sessions or minimal installs.
- v4l2loopback remains an `optdepend`, not a hard requirement. The app
  is fully functional for preview without it.
- Secure Boot users will need manual MOK enrollment. This is standard
  for out-of-tree modules and not something we can solve in userspace.
