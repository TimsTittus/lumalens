# LumaLens — Technical Architecture

> Source of truth for the project's design. Updated as decisions are made.
> For individual decision records, see `docs/decisions/`.

---

## 1. Project Identity

**Name:** LumaLens
**Tagline:** Webcam customization studio for Linux
**License:** GPL-3.0-or-later

A Rust-based Linux desktop application using GTK4 for its native
interface, GStreamer for real-time video processing, V4L2 for camera
access, and v4l2loopback for system-wide virtual webcam output, with an
extensible effects engine and optional ONNX-based AI capabilities.

---

## 2. Target Platforms

- **Primary:** Arch Linux (rolling), Fedora Workstation (latest)
- **Display server:** Wayland (GNOME, KDE). X11 via XWayland supported
  but not primary target.
- **Packaging:** Native (AUR, COPR), Flatpak (with limitations — see §16)

---

## 3. Technology Stack

| Layer          | Technology                                        |
|----------------|---------------------------------------------------|
| Language       | Rust (edition 2024, MSRV 1.82)                    |
| UI toolkit     | GTK4 ~0.9, libadwaita ~0.7                        |
| Video engine   | GStreamer ~0.23 (gstreamer-rs bindings)            |
| Preview sink   | gtk4paintablesink (gst-plugin-gtk4 ~0.13)         |
| Camera input   | v4l2src (default), pipewiresrc (Flatpak/portal)   |
| Virtual output | v4l2loopback via v4l2sink / filesink              |
| AI inference   | ONNX Runtime via ort crate (optional, feature-gated) |
| Config         | TOML via serde + toml crate                       |
| Logging        | tracing + tracing-subscriber with env-filter       |

---

## 4. Workspace Layout

```
lumalens/
├── Cargo.toml                    # [workspace]
├── crates/
│   ├── lumalens-app/             # Binary: UI, app lifecycle
│   ├── lumalens-engine/          # Pipeline, camera, runtime (no GTK deps)
│   ├── lumalens-effects/         # Effect trait, built-in effects
│   ├── lumalens-config/          # Config load/save, presets
│   ├── lumalens-common/          # Shared types, errors
│   └── lumalens-ai/              # Optional: ONNX segmentation (feature-gated)
├── prototypes/                   # Throwaway Phase 0 spikes
├── docs/
│   ├── architecture.md           # This file
│   ├── decisions/                # ADRs
│   ├── build-plan.md             # Step-by-step prompt sequence
│   └── releases/                 # Release notes
├── packaging/
│   ├── arch/                     # PKGBUILD
│   └── flatpak/                  # Flatpak manifest
└── data/                         # .desktop, icon, metainfo
```

### Dependency Direction

```
lumalens-app → lumalens-engine
lumalens-app → lumalens-effects
lumalens-app → lumalens-config
lumalens-app → lumalens-common

lumalens-engine → lumalens-effects
lumalens-engine → lumalens-common

lumalens-effects → lumalens-common
lumalens-effects → lumalens-ai    (optional, behind "ai" feature)

lumalens-config → lumalens-common

lumalens-ai → (standalone, only ort + gstreamer)
```

**Rule:** `lumalens-engine` has NO gtk, adw, or gdk dependencies.
The `paintable()` method returns `glib::Object`; the app crate downcasts.

---

## 5. Application Architecture

### Command/Event Channel

UI and engine communicate via message passing:

- **Commands** (UI → Engine): `std::sync::mpsc::sync_channel` — blocking
  sends are acceptable because the UI thread generates commands at human
  interaction rates.
- **Events** (Engine → UI): `glib::MainContext::channel` — delivers
  events on the GTK main loop for safe UI updates.

```
┌──────────┐  EngineCommand   ┌──────────────┐  EngineEvent  ┌──────────┐
│  GTK UI  │ ───────────────► │ Engine Thread │ ────────────► │  GTK UI  │
└──────────┘  (std mpsc)      └──────────────┘  (glib chan)   └──────────┘
                                     │
                              ┌──────┴──────┐
                              │  GStreamer   │
                              │  Pipeline    │
                              └─────────────┘
```

### Engine Commands

```rust
pub enum EngineCommand {
    SelectCamera(String),
    Start,
    Stop,
    SetBrightness(f32),
    SetContrast(f32),
    SetSaturation(f32),
    SetFlip(FlipMethod),
    SetResolution { width: u32, height: u32, fps: u32 },
    EnableVirtualCamera(bool),
    ReorderEffect { stage: Stage, from: usize, to: usize },
    Shutdown,
}
```

### Engine Events

```rust
pub enum EngineEvent {
    CamerasDiscovered(Vec<CameraInfo>),
    Started,
    Stopped,
    Error(String),
    MetricsUpdated(Metrics),
}
```

**Coalescing:** When draining the command channel, multiple consecutive
`SetBrightness` (or other continuous-value) commands collapse to the
latest value.

---

## 6. GStreamer Pipeline

### Preview-only Pipeline

```
v4l2src device={path} name=src
  ! videoconvert
  ! capsfilter caps=video/x-raw,width={W},height={H},framerate={fps}/1
  ! [Correction stage effects]
  ! [Transform stage effects]
  ! tee name=t
  ! queue max-size-buffers=2 leaky=downstream
  ! videoconvert
  ! gtk4paintablesink name=sink
```

### With Virtual Camera

```
t. ! queue max-size-buffers=2
   ! videoconvert
   ! video/x-raw,format=YUY2,width=1280,height=720,framerate=30/1
   ! v4l2sink device=/dev/video10 sync=false
```

The virtual camera branch is dynamically added/removed using GStreamer
pad probes (block the tee src pad, unlink, remove bin, release request
pad).

---

## 7. Fixed-Stage Effect Pipeline

Effects are organized in a fixed stage order. Reordering is allowed
within a stage but not across stages.

```rust
pub enum Stage {
    Capture,       // Source only, no user effects
    Correction,    // Brightness, contrast, saturation, white balance
    Segmentation,  // AI mask generation (optional)
    Composition,   // AI background effects (optional)
    Transform,     // Flip, rotate, crop, zoom
    Encode,        // Caps negotiation, format conversion
    Output,        // Preview + virtual cam branches (hard-coded)
}
```

### Effect Trait

```rust
pub trait Effect: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn stage(&self) -> Stage;
    fn is_enabled(&self) -> bool;
    fn set_enabled(&mut self, v: bool);
    fn build_elements(&self) -> Result<Vec<gst::Element>, EffectError>;
}
```

### Built-in Effects

| Effect     | Stage      | GStreamer Element | Parameters              |
|------------|------------|-------------------|-------------------------|
| Brightness | Correction | videobalance      | -1.0 to 1.0            |
| Contrast   | Correction | videobalance      | 0.0 to 2.0             |
| Saturation | Correction | videobalance      | 0.0 to 2.0             |
| Flip       | Transform  | videoflip         | FlipMethod enum         |
| Rotate     | Transform  | videoflip         | clockwise/counter/180   |

### AI Effects (optional, feature-gated)

| Effect          | Stage        | Mechanism            |
|-----------------|--------------|----------------------|
| Segmentation    | Segmentation | appsink → ort → mask |
| Background Blur | Composition  | mask + gaussian blur |

AI inference runs on a dedicated thread with latest-mask-wins semantics.

---

## 8. Camera Discovery

Two-tier approach (per ADR 0001):

1. **Primary:** `gstreamer::DeviceMonitor` filtered to "Video/Source".
   Works on PipeWire-enabled desktop sessions.
2. **Fallback:** Enumerate `/dev/video*`, probe each with V4L2 ioctls.
   Catches devices the monitor misses on minimal installs or headless
   sessions.

Deduplication: a single physical camera often exposes multiple
`/dev/video*` nodes. Keep only devices whose caps include at least one
`video/x-raw` or `image/jpeg` format.

---

## 9. Virtual Camera

- **Kernel module:** v4l2loopback (out-of-tree, requires
  `exclusive_caps=1`)
- **Format:** YUY2 at 1280×720@30fps (per ADR 0001)
- **Detection:** Enumerate `/dev/video*`, read
  `/sys/class/video4linux/videoN/name`, match "LumaLens" (case
  insensitive)
- **Secure Boot:** Unsigned module requires MOK enrollment or Secure
  Boot disabled. Documented in README, install scripts, and error UX.

---

## 10. Configuration

### File Location

`$XDG_CONFIG_HOME/lumalens/config.toml` (default:
`~/.config/lumalens/config.toml`)

### Schema

```toml
schema_version = 1

[camera]
selected_id = "..."
resolution_width = 1280
resolution_height = 720
fps = 30

[image]
brightness = 0.0
contrast = 1.0
saturation = 1.0
flip = "none"

[virtual_camera]
enabled = false

[effects]
# Per-stage ordering persisted here
```

### Presets

Directory: `$XDG_CONFIG_HOME/lumalens/profiles/*.toml`

Built-in presets (copied on first run): Natural, Warm, Cool.

### Robustness

- Malformed config: log warning, move to `config.toml.broken.{timestamp}`,
  start with defaults.
- Atomic writes: write temp file, fsync, rename.
- Schema migration: match on `schema_version`, migrate forward.

---

## 11. UI Layout

```
┌─────────────────────────────────────────────────────┐
│  AdwHeaderBar  [≡ Sidebar]              [Metrics]   │
├────────────────────────────┬────────────────────────┤
│                            │  Camera                │
│                            │  ┌──────────────────┐  │
│                            │  │ Camera dropdown   │  │
│                            │  │ Resolution dropdown│ │
│    Preview                 │  │ Brightness [━━━]  │  │
│    (gtk::Picture with      │  │ Contrast   [━━━]  │  │
│     gtk4paintablesink)     │  │ Saturation [━━━]  │  │
│                            │  │ Mirror/Rotate ▼   │  │
│                            │  └──────────────────┘  │
│                            │                        │
│                            │  Output                │
│                            │  ┌──────────────────┐  │
│                            │  │ Virtual Cam [⊙]   │  │
│                            │  └──────────────────┘  │
│                            │                        │
│                            │  Presets               │
│                            │  ┌──────────────────┐  │
│                            │  │ Preset dropdown   │  │
│                            │  │ [Apply][Save][Del]│  │
│                            │  └──────────────────┘  │
├────────────────────────────┴────────────────────────┤
│  AdwToastOverlay (error messages)                   │
└─────────────────────────────────────────────────────┘
```

- `AdwOverlaySplitView` with collapsible sidebar (~320px)
- `AdwToolbarView` with `AdwHeaderBar`
- Sidebar toggle button in header bar
- Diagnostics panel via Ctrl+Alt+D (floating `adw::Window`)

---

## 12. Error Handling

- Pipeline bus errors → `EngineError` → `EngineEvent::Error` → toast
- Camera disconnect → stop pipeline, re-discover, auto-select fallback
  or show "no cameras" status page
- No cameras at startup → `AdwStatusPage` with helpful message
- v4l2loopback not loaded → error toast with install instructions
- Config corruption → warn, backup, reset to defaults
- All error boundaries logged via `tracing::warn!`

---

## 13. Diagnostics

Metrics collected via GStreamer pad probes every 500ms:

```rust
pub struct Metrics {
    pub input_fps: f32,
    pub output_fps: f32,
    pub dropped_frames: u64,
    pub processing_ms: f32,
    pub queue_depth_preview: u32,
    pub queue_depth_virtual: u32,
    pub ai_inference_ms: Option<f32>,
    pub ai_mask_age_ms: Option<u32>,
}
```

Diagnostic bundle (copyable): metrics + last 100 log lines + GStreamer
version + GTK version + distro info.

---

## 14. AI Subsystem

- **Crate:** `lumalens-ai`, behind `features = ["ort"]`
- **App feature:** `ai = ["lumalens-ai/ort"]`, OFF by default
- **Model:** MediaPipe Selfie Segmentation (small, Apache-2.0)
- **Download:** On-demand with user consent dialog (name, size, URL,
  license, SHA256). Never auto-download. Never bundled.
- **Inference thread:** Dedicated `std::thread`, not tokio. Latest-mask-wins
  channel. If inference < frame rate, composition reuses last mask.
- **HTTP client:** reqwest with rustls-tls (not native-tls), only in
  lumalens-ai under ort feature.

---

## 15. Packaging

### Native (Arch — AUR)

- `lumalens` package from tagged release
- `lumalens-git` package from main branch
- depends: gtk4, libadwaita, gstreamer, gst-plugins-base/good/bad,
  gst-libav
- optdepends: v4l2loopback-dkms, v4l-utils
- `.install` script with Secure Boot / MOK guidance

### Flatpak

- Runtime: org.gnome.Platform//47
- Camera input via portal (`org.freedesktop.portal.Camera`) +
  `pipewiresrc`
- Virtual camera output NOT supported in sandbox (see ADR 0003)
- No `--device=all` (Flathub policy)

---

## 16. Known Limitations

- Linux only. No macOS or Windows support planned.
- v4l2loopback required for virtual camera output (out-of-tree module).
- Secure Boot requires MOK enrollment for v4l2loopback.
- Wayland primary; X11 via XWayland works but is not the test target.
- No audio handling.
- No plugin system (deferred — "plugin system is a trap" until the
  effect API is battle-tested).
- Flatpak cannot do virtual camera output due to sandbox.
- AI effects require ONNX Runtime system library and model download.

---

## 17. Roadmap

| Phase | Milestone                  | Tag       |
|-------|----------------------------|-----------|
| 0     | De-risking prototypes      | —         |
| 1     | Walking skeleton           | v0.0.1    |
| 1     | Virtual camera working     | v0.0.2    |
| 2     | OSS scaffolding + CI       | —         |
| 3     | Four-crate workspace split | —         |
| 4     | Presets + polish           | v0.1.0    |
| 5     | AUR + Flatpak packaging    | —         |
| 6     | Fixed-stage effect engine  | —         |
| 7     | Optional AI effects        | —         |
| 8     | v1.0                       | v1.0.0    |

---

## 18. MVP Acceptance Checklist (v0.1.0)

- [ ] App launches on Arch with Wayland (GNOME)
- [ ] App launches on Fedora Workstation with Wayland
- [ ] Camera preview renders at ≥25fps with <15% CPU on one core
- [ ] Camera dropdown lists available cameras
- [ ] Switching cameras rebuilds pipeline without crash
- [ ] Brightness, contrast, saturation sliders work live
- [ ] Mirror/rotate dropdown works
- [ ] Virtual camera toggle sends feed to Chromium Google Meet
- [ ] Virtual camera toggle sends feed to Firefox Google Meet
- [ ] Unplugging USB camera shows toast, no crash
- [ ] Replugging camera re-appears in dropdown
- [ ] Config persists across restarts
- [ ] Presets: save, load, delete
- [ ] Resolution switching works
- [ ] Clean shutdown: no leaked devices, no orphan processes
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [ ] `cargo test --workspace` green
- [ ] `cargo doc --workspace --no-deps` no warnings
