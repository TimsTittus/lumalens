# 02 – Virtual Cam Prototype

Throwaway spike to prove we can write a feed to a v4l2loopback device in a
format that Chromium, Firefox, and Zoom web all accept.

## Prerequisite

Load the v4l2loopback kernel module **before** running the prototype:

```bash
sudo modprobe v4l2loopback exclusive_caps=1 video_nr=10 card_label="CamForge Test"
```

Verify the device exists:

```bash
v4l2-ctl --list-devices
# Should show "CamForge Test" on /dev/video10
```

## Arch Dependencies

```bash
sudo pacman -S --needed \
  base-devel rust \
  gstreamer gst-plugins-base gst-plugins-good \
  v4l-utils v4l2loopback-dkms linux-headers
```

## Build & Run

```bash
cd prototypes/02-virtual-cam
cargo run
```

On startup the binary prints:
1. Negotiated caps on both tee branches (should show `video/x-raw, format=YUY2, width=1280, height=720, framerate=30/1`).
2. Full `v4l2-ctl --all` output for `/dev/video10` after a 2-second settle.

Press Ctrl+C to stop. The pipeline transitions to Null and releases both devices.

## Manual Test Matrix

### Chromium – Google Meet

1. Open `meet.google.com` and start or join a call.
2. Click the three-dot menu → Settings → Video.
3. In the camera dropdown, select **"CamForge Test"**.
4. **Pass**: the preview shows your webcam feed within 5 seconds, no black
   frames, no "camera failed" error.

### Firefox – Google Meet

1. Open `meet.google.com` and start or join a call.
2. Click the three-dot menu → Settings → Video.
3. In the camera dropdown, select **"CamForge Test"**.
4. **Pass**: the preview shows your webcam feed within 5 seconds. Firefox may
   prompt for camera permission — grant it for the loopback device.

### Zoom Web Client

1. Open `app.zoom.us` in Chromium (Zoom web requires a Chromium-based browser).
2. Join a test meeting.
3. Click the arrow next to the camera icon → select **"CamForge Test"**.
4. **Pass**: your webcam feed appears in the Zoom self-view within 5 seconds.

## What Failure Looks Like

- **"Device busy"** on startup → another process holds `/dev/video0` or
  `/dev/video10`. Check with `fuser /dev/video0 /dev/video10`.
- **"CamForge Test" not in dropdown** → `v4l2loopback` not loaded, or loaded
  without `exclusive_caps=1`. Chromium requires `exclusive_caps=1` to
  enumerate the device.
- **Black feed in browser** → caps mismatch. Check the negotiated caps printed
  at startup; YUY2 at 1280x720@30fps is the most widely accepted format.
- **Feed freezes after a few seconds** → queue underrun. Check stderr for
  pipeline warnings.
