# 01 – Preview Sink Prototype

Throwaway spike to prove `gtk4paintablesink` renders a live V4L2 webcam feed
cleanly on Wayland + GTK4 before committing to this stack.

## Arch Dependencies

```bash
sudo pacman -S --needed \
  base-devel rust \
  gtk4 libadwaita \
  gstreamer gst-plugins-base gst-plugins-good gst-plugin-gtk4 \
  v4l-utils
```

`gst-plugin-gtk4` provides the `gtk4paintablesink` element.
`gst-plugins-good` provides `v4l2src` and `videoconvert`.

## Build & Run

```bash
cd prototypes/01-preview-sink
cargo run
```

## What "Passing" Looks Like

1. **Stable preview** – a 1280×720 window opens showing your webcam feed with no
   frame drops or visual corruption.
2. **No tearing** – panning across a high-contrast scene (e.g. waving a hand)
   shows no horizontal tear lines. On Wayland this should be guaranteed by the
   compositor, but verify anyway.
3. **Clean shutdown** – closing the window or pressing Ctrl+C prints pipeline
   state transitions (`Playing → Paused → Ready → Null`) to stderr and exits
   with code 0.
4. **Device released** – after exit, `/dev/video0` is no longer held:
   ```bash
   fuser /dev/video0   # should print nothing
   ```

## What Failure Looks Like

- Window opens but stays black → missing `gst-plugins-good` or wrong device
  path. Check `v4l2-ctl --list-devices`.
- Tearing under X11 → expected; this prototype targets Wayland. Run under a
  Wayland session.
- Crash on close → pipeline teardown order issue; check stderr tracing output.
