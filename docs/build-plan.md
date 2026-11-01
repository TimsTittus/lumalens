# LumaLens — Step-by-Step Claude Code Prompts

A sequence of prompts to paste into Claude Code one at a time. After each, there's a **verify** step you do yourself before moving on. Do not run them in parallel. Do not skip the verify steps. If a verify fails, fix it before the next prompt — bugs compound.

A note on expectations before we start: these prompts are tight and specific, but LLMs still produce mistakes, especially around GStreamer caps negotiation and GTK4 lifecycles. Treat Claude Code as a fast junior developer who writes mostly-correct code — read the diffs, run the binaries, and push back when something looks off. The verify steps are where quality actually comes from.

Keep this whole plan in a file in your repo (`docs/build-plan.md`) and reference it in prompts: *"per step 7 of docs/build-plan.md…"*. Claude Code performs noticeably better when it can re-read the plan.

---

## Phase 0 — De-risking

### Prompt 0.1 — Preview sink proof

```
Create a throwaway Rust prototype at `prototypes/01-preview-sink/`.

Goal: prove that `gtk4paintablesink` renders a live V4L2 webcam feed cleanly on
Wayland + GTK4 before we commit to this stack.

Requirements:
- Single binary, single file `src/main.rs`, under 200 lines.
- Cargo.toml dependencies: gtk4 = "0.9", gstreamer = "0.23",
  gstreamer-video = "0.23", gstreamer-gtk4 = { version = "0.13",
  features = ["wayland", "x11egl"] }.
- Pipeline: v4l2src device=/dev/video0 ! videoconvert !
  gtk4paintablesink name=sink
- Open an AdwApplicationWindow (use libadwaita 0.7) 1280x720, embed the
  paintable from the sink in a gtk::Picture widget.
- Log pipeline state changes and bus errors to stderr via tracing-subscriber
  at INFO level.
- Handle Ctrl+C and window-close by setting the pipeline to Null before exit.
- No device selection, no effects, no config.

Do not add tests, CI, or extra abstractions. This is throwaway.
Write a one-page `README.md` in the prototype folder explaining what passing
looks like (stable preview, no tearing, releases /dev/video0 on exit) and the
exact pacman command to install missing deps on Arch.
```

**Verify:**
- `cargo run` from `prototypes/01-preview-sink/` opens a window with your webcam
- Preview is smooth for 10 minutes
- `top` shows the process under ~15% CPU on one core
- After closing, `fuser /dev/video0` prints nothing
- Works without `GDK_BACKEND=x11` (pure Wayland)

**If it fails:** stop and investigate. Do not proceed. The whole plan depends on this working.

---

### Prompt 0.2 — Virtual camera handshake proof

```
Create a second throwaway prototype at `prototypes/02-virtual-cam/`.

Prerequisite the README must mention: user has run
`sudo modprobe v4l2loopback exclusive_caps=1 video_nr=10 card_label="LumaLens Test"`.

Goal: prove that we can write a feed to a v4l2loopback device in a format
that Chromium, Firefox, and Zoom web all accept.

Requirements:
- Single binary, single file, no GUI.
- Pipeline:
  v4l2src device=/dev/video0 !
  videoconvert !
  video/x-raw,format=YUY2,width=1280,height=720,framerate=30/1 !
  tee name=t
  t. ! queue ! fakesink
  t. ! queue ! v4l2sink device=/dev/video10 sync=false
- On startup, query and print the negotiated caps on BOTH tee branches'
  sink pads using pad.current_caps(). Also print the output device's
  v4l2 format by shelling out to `v4l2-ctl --device=/dev/video10 --all`
  once after 2 seconds.
- Run until Ctrl+C.
- Dependencies: just gstreamer and tracing.

Write the README with a manual test matrix: Chromium Google Meet, Firefox
Google Meet, Zoom web. For each, note what to click and what passing looks
like (device named "LumaLens Test" appears and shows the feed within 5
seconds).
```

**Verify:**
- Run the binary, open each browser, confirm all three show your feed through "LumaLens Test"
- If one fails (likely Firefox), try changing the format to `NV12` or `I420` and document what works where in the README

**If more than one fails:** this is where Linux virtual cameras actually die. Budget a full day here. Do not proceed until you have at least Chromium + Firefox working, because that covers Google Meet and most WebRTC apps.

---

### Prompt 0.3 — PipeWire discovery proof

```
Create a third throwaway prototype at `prototypes/03-device-discovery/`.

Goal: understand which cameras appear via V4L2 vs PipeWire on our target
distros, so Phase 1 picks the right discovery path.

Requirements:
- Single binary, no GUI.
- Use gstreamer::DeviceMonitor. Add two filters:
  one for "Video/Source" with no caps, one explicitly scoped to PipeWire
  via caps "video/x-raw" and device.api=pipewire where supported.
- Print for each discovered device: display_name, device_class, properties
  (api, path, bus-info if present).
- Also enumerate /dev/video* directly and print their v4l2 CAPABILITIES
  by shelling to `v4l2-ctl --device=X --info`.
- Exit after one enumeration pass.
- Dependencies: gstreamer only.

README: run on Arch host and inside a Fedora Workstation VM, paste both
outputs. We're looking for which path reports usable cameras in each
environment.
```

**Verify:** output on both environments documented in the prototype README. You now know whether to prefer `v4l2src` or `pipewiresrc` by default.

---

### Prompt 0.4 — Phase 0 decision record

```
Create `docs/decisions/` and add `0001-phase-0-findings.md`. Format:
ADR-style with Status, Context, Decision, Consequences.

Fill in:
- GTK4 + gtk4paintablesink: viable / not viable (based on prototype 01)
- Virtual camera format of choice: YUY2 vs NV12 vs I420 (based on 02)
- Default capture source: v4l2src vs pipewiresrc (based on 03)
- Known Secure Boot caveat for v4l2loopback

Also create `docs/architecture.md` and paste in the full technical spec
we agreed on. This is the source of truth going forward.
```

**Verify:** both files exist, decisions are specific (not "it probably works"), and the architecture doc is the one we've refined through this conversation.

---

## Phase 1 — Walking skeleton (single crate)

### Prompt 1.1 — Repo bootstrap

```
At the repo root (sibling to `prototypes/`), initialize a Cargo project
called `lumalens`.

- Cargo.toml: edition = "2024", license = "GPL-3.0-or-later",
  rust-version = "1.82", authors = ["Neer <email>"]
- Add dependencies (pin exact minor versions based on what worked in
  prototypes): gtk4 ~0.9, libadwaita ~0.7, gstreamer ~0.23,
  gstreamer-video ~0.23, gstreamer-app ~0.23,
  gstreamer-gtk4 ~0.13 with wayland+x11egl features,
  serde ~1 with derive, toml ~0.9, thiserror ~2,
  tracing ~0.1, tracing-subscriber ~0.3 with env-filter.
- Add .gitignore (standard Rust + /target + *.swp + .idea + .vscode).
- Add rustfmt.toml (empty = defaults).
- Add clippy.toml with warn-on-all-wildcard-imports = true.
- Add LICENSE (full GPL-3.0-or-later text).
- Add README.md: one-paragraph pitch, "status: pre-alpha, not usable
  yet" banner, build-from-source steps for Arch.
- src/main.rs: minimal adw::Application with application ID
  "io.github.lumalens.LumaLens", opens an empty AdwApplicationWindow
  titled "LumaLens", initializes GStreamer and tracing_subscriber.

Do not add CI, modules, or features yet.
Do not touch /prototypes.
```

**Verify:** `cargo run` opens an empty window titled "LumaLens" and closes cleanly.

---

### Prompt 1.2 — Module skeleton

```
Inside `src/`, create this module layout. Each file should have module
documentation and the types/enums listed, with `todo!()` or empty impls
where functionality isn't ready. This is scaffolding — do not implement
logic yet.

src/
  main.rs            // existing
  app.rs             // pub fn run() -> glib::ExitCode
  ui/
    mod.rs
    window.rs        // pub struct Window; pub fn new(app: &adw::Application) -> adw::ApplicationWindow
    preview.rs       // pub struct PreviewWidget wrapping gtk::Picture
    controls.rs      // (empty for now)
  engine/
    mod.rs
    commands.rs      // pub enum EngineCommand { SelectCamera(String), Start, Stop, SetBrightness(f32), EnableVirtualCamera(bool), Shutdown }
    events.rs        // pub enum EngineEvent { CamerasDiscovered(Vec<CameraInfo>), Started, Stopped, Error(String) }
    pipeline.rs      // pub struct Pipeline;
    camera.rs        // pub struct CameraInfo { pub id: String, pub name: String, pub device_path: String }
    virtual_cam.rs
  config/
    mod.rs           // pub struct Config; pub fn load() / pub fn save()

Also add a top-level `prelude.rs` module re-exporting Result and the
engine command/event enums.

Update main.rs to call app::run().

Verify compiles with `cargo build` and `cargo clippy -- -D warnings`.
```

**Verify:** `cargo build` and `cargo clippy -- -D warnings` both clean. Window still opens.

---

### Prompt 1.3 — Camera discovery

```
Implement `src/engine/camera.rs`:

- `pub fn discover() -> Result<Vec<CameraInfo>, EngineError>` using
  gstreamer::DeviceMonitor filtered to "Video/Source". Based on the
  Phase 0 ADR, prefer [v4l2src OR pipewiresrc — pick the one that won].
- Use the device's display_name as `name`, device.path or device.serial
  property as `id` (fall back to display_name if missing),
  properties.device.path or object.serial as `device_path`.
- Deduplicate: a single physical camera often exposes multiple /dev/video*
  nodes. Keep only devices whose caps include at least one
  `video/x-raw` or `image/jpeg` format.
- Add a unit test with mocked DeviceMonitor results — if that's hard
  because of gst's design, skip the unit test and add a comment
  explaining why; add an integration test behind
  `#[cfg_attr(not(feature = "live-hardware"), ignore)]` instead.

Add EngineError enum in a new `src/engine/errors.rs` using thiserror,
with variants: Gstreamer(gst::glib::Error), NoCamerasFound,
PipelineBuildFailed(String).

Wire this into `src/engine/mod.rs` and re-export CameraInfo from the prelude.
```

**Verify:**
- `cargo test` passes
- Add a temporary `println!("{:#?}", engine::camera::discover()?)` in `main.rs`, run it, confirm your webcam appears exactly once with sensible fields. Remove the println before next prompt.

---

### Prompt 1.4 — Pipeline (preview branch only)

```
Implement `src/engine/pipeline.rs` for the PREVIEW branch only. The virtual
camera branch comes in a later prompt.

Pipeline construction:
- v4l2src device={device_path} name=src !
  videoconvert !
  videobalance name=balance brightness=0 contrast=1 saturation=1 !
  tee name=t !
  queue max-size-buffers=2 leaky=downstream !
  videoconvert !
  gtk4paintablesink name=sink

API:
- pub struct Pipeline { inner: gst::Pipeline, balance: gst::Element, sink: gst::Element }
- pub fn build(device_path: &str) -> Result<Pipeline, EngineError>
- pub fn start(&self) -> Result<(), EngineError>
- pub fn stop(&self) -> Result<(), EngineError>  // sets to Null
- pub fn set_brightness(&self, value: f32)       // clamp -1.0..=1.0, set on balance
- pub fn paintable(&self) -> gdk::Paintable      // from the sink's "paintable" property

Add a bus watch via gst::Bus::add_watch_local that logs errors and warnings
via tracing. Do NOT panic on bus errors — surface them via a callback
registered on the Pipeline (add a `set_error_handler` method taking
Box<dyn Fn(EngineError) + Send + 'static>).

Important: do NOT depend on gtk here. gstreamer-gtk4 provides the sink
element registered by name; just use ElementFactory::make("gtk4paintablesink").
The gdk::Paintable return type is unavoidable but it comes via glib::Object
cast — keep gtk/gdk imports isolated to one function.
```

**Verify:** code compiles. Pipeline not wired to UI yet; next prompt does that.

---

### Prompt 1.5 — Preview wired into UI

```
Wire the preview pipeline into the window.

In `src/ui/preview.rs`:
- pub struct PreviewWidget { picture: gtk::Picture }
- pub fn new() -> Self (creates a gtk::Picture with content_fit = Contain,
  min size 640x360)
- pub fn widget(&self) -> &gtk::Picture
- pub fn set_paintable(&self, paintable: &gdk::Paintable)

In `src/ui/window.rs`:
- Build an AdwToolbarView with an AdwHeaderBar at top and the
  PreviewWidget in the content area.
- On window realize, call engine::camera::discover(), pick the first
  camera, build and start a Pipeline for it, call
  preview.set_paintable(&pipeline.paintable()).
- Store the Pipeline in a RefCell<Option<Pipeline>> owned by the window
  so it isn't dropped.
- On window close-request, call pipeline.stop() before letting the
  window close.
- If discover() returns no cameras OR pipeline build fails, show an
  AdwStatusPage in the content area instead of the preview with a
  helpful message.

Do NOT yet add the command/event channel — keep it simple and
synchronous for this prompt. We refactor to async messaging next.
```

**Verify:**
- `cargo run` → see your face
- Close window → terminal shows pipeline going to Null, process exits, `fuser /dev/video0` is empty
- Unplug camera at build time → see the "no cameras" status page instead of a crash

**This is the first real milestone. Tag it:**
```
git tag v0.0.1-first-preview && git push --tags
```

---

### Prompt 1.6 — Command/event channel

```
Refactor to decouple UI and engine via message passing. This is critical
for later; do it now while the code is small.

Create `src/engine/mod.rs` module `runtime`:
- pub struct Engine { cmd_tx: mpsc::SyncSender<EngineCommand>, _thread: JoinHandle<()> }
- pub fn spawn() -> (Engine, glib::Receiver<EngineEvent>)
  - Create std::sync::mpsc::sync_channel::<EngineCommand>(32) for commands
  - Create glib::MainContext::channel::<EngineEvent>(PRIORITY_DEFAULT) for events
  - Spawn a thread that owns the Pipeline and loops on cmd_rx.recv()
  - Thread handles each EngineCommand:
    - SelectCamera: rebuild pipeline for that device, send CamerasDiscovered first if list is stale
    - Start / Stop: pipeline.start()/stop(), send Started/Stopped
    - SetBrightness: pipeline.set_brightness
    - EnableVirtualCamera: no-op for now (TODO)
    - Shutdown: break loop
  - Coalesce SetBrightness: when draining, if multiple SetBrightness are
    queued back-to-back, only apply the latest. Implement by non-blocking
    try_recv after each recv.

Update `src/ui/window.rs`:
- Spawn engine on window construction.
- On realize, send Start command.
- Subscribe to event channel; on CamerasDiscovered populate (later) a
  dropdown; on Error show toast via adw::ToastOverlay.
- On close-request, send Shutdown and join cleanly.

Add a comment at the top of runtime.rs explaining why we use two different
channel types (std mpsc for commands, glib channel for events).
```

**Verify:**
- Still works end-to-end
- Add `tracing::info!` logs on each command received; drag a (future) slider rapidly and confirm coalescing works
- Close window → clean shutdown, no "pipeline not stopped" warnings in logs

---

### Prompt 1.7 — Brightness slider + camera dropdown

```
Add minimal controls to prove the message channel works.

In `src/ui/controls.rs`:
- pub struct Controls { pub root: adw::PreferencesGroup, pub brightness: gtk::Scale, pub camera_dropdown: gtk::DropDown }
- Build a PreferencesGroup titled "Camera" with:
  - An AdwComboRow (or gtk::DropDown wrapped) for camera selection
  - An AdwActionRow containing a gtk::Scale for brightness, range -1.0..=1.0, step 0.05, draw_value = true

In `src/ui/window.rs`:
- Add an AdwOverlaySplitView with the preview in content and the Controls
  group in the sidebar (width ~320).
- Wire camera dropdown's selected-item signal to send
  EngineCommand::SelectCamera.
- Wire brightness scale's value-changed signal to send
  EngineCommand::SetBrightness. Debounce in the UI with a 60ms timeout
  via glib::timeout_add_local — reset the timer on each change, send
  only when it fires.
- On CamerasDiscovered event, populate the dropdown model.

Keep the sidebar collapsible using the split view's show_sidebar property;
add a toggle button in the header bar.
```

**Verify:**
- Window has sidebar with camera dropdown and brightness slider
- Dragging brightness updates preview smoothly without CPU spiking (coalescing + GStreamer-native effect)
- Switching cameras (if you have two) rebuilds pipeline cleanly

---

### Prompt 1.8 — Virtual camera branch

```
Add the virtual camera branch to Pipeline and wire it up.

In `src/engine/pipeline.rs`:
- Add method `pub fn enable_virtual_camera(&self, enabled: bool, device_path: &str) -> Result<(), EngineError>`.
- When enabling:
  - Build a bin: queue max-size-buffers=2 ! videoconvert !
    video/x-raw,format={FORMAT_FROM_ADR},width=1280,height=720,framerate=30/1 !
    v4l2sink device={device_path} sync=false
  - Add bin to pipeline, request a src pad from the tee, link.
  - Sync bin state with parent.
- When disabling:
  - Block the tee src pad, unlink and remove the bin, set bin to Null,
    release the tee's request pad. Follow the GStreamer "dynamic pipelines"
    pattern — add a comment linking to the GStreamer docs section on
    pad probes if you cite one.
- Format constant: pub const VIRTUAL_CAM_FORMAT: &str = "{YUY2|NV12 — from ADR}";

In `src/engine/virtual_cam.rs`:
- pub fn find_device() -> Option<String> — enumerate /dev/video* via
  std::fs, read /sys/class/video4linux/videoN/name, return the first
  one whose name contains "LumaLens" (case insensitive).
- pub fn is_module_loaded() -> bool — check /proc/modules for v4l2loopback.
- pub fn module_install_instructions() -> &'static str — multi-line
  string with pacman/dnf/apt commands.

In runtime, handle EngineCommand::EnableVirtualCamera by calling
pipeline.enable_virtual_camera with the discovered device path.

In Controls, add a gtk::Switch for "Virtual Camera" in a new
PreferencesGroup "Output". If virtual_cam::find_device() returns None
when toggled on, send an Error event with the install instructions.
```

**Verify:**
- With v4l2loopback loaded, toggle switch on, open Chromium → Meet → see LumaLens feed
- Toggle off → device still exists but no frames (fakesink in Meet)
- Toggle on/off rapidly 10 times → no crashes, no leaked pads (`gst-inspect-1.0` or GST_DEBUG tee:5 if curious)

**Second real milestone. Tag it:**
```
git tag v0.0.2-virtual-cam && git push --tags
```

This is where you have a *useful* program, even if ugly.

---

### Prompt 1.9 — Hot-unplug and error UX

```
Make the app survive camera disconnections and show errors well.

In Pipeline:
- The bus watch already catches gst::MessageView::Error. Map these to
  EngineError and forward via the error handler callback.
- Detect ResourceNotFound and DeviceClosed errors specifically; map to
  EngineError::CameraDisconnected { name }.

In runtime:
- When CameraDisconnected is received from the pipeline, stop the pipeline
  cleanly, re-run discovery, send Error event with a human message,
  and send CamerasDiscovered with the new list.

In window.rs:
- Wrap content in adw::ToastOverlay.
- On Error events, show a toast with the message (timeout 5s).
- If the currently selected camera disappears from CamerasDiscovered,
  auto-select the first available or show the "no cameras" status page.

Add tracing::warn! at every error boundary so the terminal shows what
happened even if the toast is dismissed.
```

**Verify:**
- Unplug USB webcam while preview is running → toast appears, UI switches to status page or alternate camera, no crash
- Replug → appears in dropdown within a second or two (if your discovery is polling; if event-driven even better, but polling is fine for now)
- Close app → clean exit

---

## Phase 2 — Open-source hygiene

### Prompt 2.1 — Contribution scaffolding

```
Add OSS scaffolding files at repo root:

- CONTRIBUTING.md: dev setup (Arch pacman command, Fedora dnf command),
  build/run/test, code style (rustfmt + clippy -D warnings), PR process
  (small PRs, one topic each, tests where practical, update CHANGELOG),
  how to find a good first issue.
- CODE_OF_CONDUCT.md: Contributor Covenant 2.1 verbatim with contact email.
- SECURITY.md: supported versions (just main for now), reporting process
  (plain email), expected response time (best effort, pre-alpha).
- CHANGELOG.md: Keep-a-Changelog format, with Unreleased section
  summarizing what's been built so far per git log.
- .github/ISSUE_TEMPLATE/ with bug_report.md, feature_request.md, question.md.
- .github/PULL_REQUEST_TEMPLATE.md with checklist (fmt, clippy, tests, changelog).
- Expand README.md: pitch paragraph, status banner, screenshot placeholder
  comment, feature list (what works today only), install (build-from-source
  for now), roadmap link, contributing link, license.

Do not fabricate maintainer names, emails, or Discord links. Use
placeholders in angle brackets where I need to fill in.
```

**Verify:** read each file, fill placeholders, commit.

---

### Prompt 2.2 — CI

```
Add `.github/workflows/ci.yml`:

- Trigger: push to main, pull_request to main.
- Single job on ubuntu-24.04.
- Steps:
  1. actions/checkout@v4
  2. Install system deps: apt install build-essential pkg-config
     libgtk-4-dev libadwaita-1-dev libgstreamer1.0-dev
     libgstreamer-plugins-base1.0-dev libgstreamer-plugins-bad1.0-dev
     gstreamer1.0-plugins-good gstreamer1.0-plugins-bad
     gstreamer1.0-libav gstreamer1.0-tools libclang-dev
  3. dtolnay/rust-toolchain@stable with components rustfmt, clippy
  4. Swatinem/rust-cache@v2
  5. cargo fmt --all --check
  6. cargo clippy --workspace --all-targets -- -D warnings
  7. cargo build --workspace --release
  8. cargo test --workspace -- --skip live-hardware

No macOS or Windows jobs. No release automation yet. No matrix.
Keep the file under 60 lines.
```

**Verify:** push to a branch, open a PR against main, CI goes green. If it fails for a real reason, fix the code; if for a CI config reason, fix the YAML.

---

### Prompt 2.3 — Demo GIF and public announcement prep

```
Create `docs/demo/` with a README describing how to record a 60-90s demo
GIF using `peek` or `wf-recorder + gifski`:

- Open LumaLens
- Select camera, show preview
- Adjust brightness
- Toggle virtual camera
- Switch to Firefox, show Meet with LumaLens feed

Add a placeholder `![demo](docs/demo/demo.gif)` to the README. I'll
record and commit the GIF myself.

Also draft `docs/announcement.md` with a short post suitable for r/linux,
r/rust, and Hacker News Show HN — 3 paragraphs max, honest about pre-alpha
status, link to repo, screenshot. Do NOT publish; just draft.
```

**Verify:** record GIF, review announcement draft, keep it in your pocket until Phase 4.

---

## Phase 3 — Workspace split

### Prompt 3.1 — Four-crate workspace

```
Convert the single crate into a Cargo workspace with four members:

lumalens/
  Cargo.toml                  # [workspace] only
  crates/
    lumalens-app/             # binary; was src/main.rs, src/app.rs, src/ui/*
    lumalens-engine/          # was src/engine/* (minus config concerns)
    lumalens-config/          # was src/config/*
    lumalens-common/          # errors, shared types (CameraInfo moves here)

Rules:
- lumalens-engine has NO gtk, adw, or gdk dependencies in its Cargo.toml.
  gstreamer-gtk4 is fine (it's a plugin, no GTK types leak into our API)
  but camera.rs and runtime.rs must not import gtk/adw/gdk.
- The `paintable()` method on Pipeline returns `glib::Object` instead of
  `gdk::Paintable`; lumalens-app does the downcast. Add a doc comment
  explaining why.
- lumalens-app depends on all three others.
- lumalens-config and lumalens-common have no inter-dependency except
  common might be used by config.

Workspace Cargo.toml:
- resolver = "2"
- [workspace.package] with version, edition, license, rust-version
- [workspace.dependencies] for every third-party dep, pinned
- Each member Cargo.toml uses { workspace = true } for shared deps.

After the move:
- cargo build --workspace must pass
- cargo clippy --workspace -- -D warnings must pass
- cargo test --workspace must pass
- cargo run -p lumalens-app must behave identically to before
- cargo tree -p lumalens-engine must show NO gtk/adw/gdk dependencies

List any files you had to leave with cross-cutting imports and explain.
```

**Verify:** all four cargo commands pass. Run the binary; behavior unchanged. `cargo tree -p lumalens-engine | grep -E 'gtk|adw|gdk'` returns nothing (or only `gdk-sys` transitively via `gstreamer-gtk4`, which is acceptable — note it in the ADR).

---

### Prompt 3.2 — ADR for the split

```
Add `docs/decisions/0002-four-crate-workspace.md` explaining:
- Why four crates not nine (deferred extraction until seams are earned)
- Dependency direction rule (app → engine, engine isolated from GTK)
- The one allowed exception (paintable returned as glib::Object)
- When to add more crates (effects when there are 3+, ai when ort is
  actually integrated, output when virtual_cam has sibling backends)
```

---

## Phase 4 — Presets and polish for v0.1

### Prompt 4.1 — Full native effects

```
Extend Pipeline with the remaining GStreamer-native image controls.

Add to the preview branch, in this order after v4l2src ! videoconvert:
- videobalance (already there — add contrast and saturation setters)
- videoflip name=flip method=none (settable: none, horizontal-flip,
  vertical-flip, clockwise, rotate-180, counterclockwise)

Add Pipeline methods:
- set_contrast(f32) — 0.0..=2.0, default 1.0
- set_saturation(f32) — 0.0..=2.0, default 1.0
- set_flip_method(FlipMethod) — new enum in lumalens-common

Add EngineCommand variants: SetContrast(f32), SetSaturation(f32), SetFlip(FlipMethod).

In Controls, add to the Camera PreferencesGroup:
- Contrast scale (0.0..=2.0, step 0.05)
- Saturation scale (0.0..=2.0, step 0.05)
- A DropDown for mirror/rotate (6 options matching FlipMethod)

Same 60ms UI debounce for scales. No debounce for the dropdown.
```

**Verify:** each control works live, no stutter when dragging multiple sliders.

---

### Prompt 4.2 — Config and presets

```
Implement lumalens-config properly.

Schema (TOML, versioned):

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

API:
- pub struct Config (serde Deserialize/Serialize, with #[serde(default)]
  on every field)
- pub fn load() -> Config — reads $XDG_CONFIG_HOME/lumalens/config.toml
  (fall back to ~/.config/lumalens/config.toml), returns default if missing
  or malformed (log a warning in the malformed case and move the broken
  file to config.toml.broken.{timestamp}).
- pub fn save(&Config) -> Result<(), ConfigError> — writes atomically
  (write to temp, fsync, rename).
- pub fn migrate(raw: toml::Value) -> toml::Value — matches on
  schema_version. Add a test that migrates a version-less config to v1.

Presets:
- Directory: $XDG_CONFIG_HOME/lumalens/profiles/*.toml
- pub struct Preset { name: String, image: ImageSettings } where
  ImageSettings is the [image] block.
- pub fn list_presets() -> Vec<Preset>
- pub fn save_preset(name: &str, image: &ImageSettings) -> Result<()>
- pub fn delete_preset(name: &str) -> Result<()>

Ship 3 built-in presets as resources (include_str!): Natural, Warm, Cool.
Copy them to the user's profiles dir on first run if absent.

In lumalens-app:
- Load config at startup, apply to initial pipeline state.
- Save config on each setting change, debounced 500ms.
- Add a "Presets" PreferencesGroup with a DropDown of available presets,
  Apply/Save as/Delete buttons.
```

**Verify:**
- Change a setting, close app, reopen → setting persisted
- Save a preset, change settings, apply preset → settings restored
- Corrupt the config file manually → app starts with defaults, warns in log, moves the corrupt file aside

---

### Prompt 4.3 — Resolution renegotiation

```
Add runtime resolution switching.

In Pipeline:
- Store a caps_filter element (capsfilter with video/x-raw,width,height,framerate)
  between videoconvert and videobalance.
- pub fn set_resolution(&self, width: u32, height: u32, fps: u32) ->
  Result<(), EngineError> — pause pipeline, set new caps on filter,
  restart. If caps negotiation fails, revert to previous caps and
  return an error.

In Controls, add a "Resolution" DropDown with presets 640x480@30,
1280x720@30, 1280x720@60, 1920x1080@30. Filter options by what the
current camera's caps actually support — query via
device.caps().iter().collect() at discovery time and store on CameraInfo.

Add resolution and fps fields to CameraInfo and populate from caps.
```

**Verify:** switch resolution live, no crash, preview resizes smoothly, Meet consumer sees the new resolution (may need to re-select camera in Meet).

---

### Prompt 4.4 — v0.1.0 polish checklist

```
Prepare the first public release.

- Update CHANGELOG.md Unreleased → 0.1.0, dated.
- Update README screenshot (I'll replace with real one).
- Add a "Known limitations" section to README listing: Linux only,
  v4l2loopback required for virtual cam, Wayland tested only on
  GNOME/KDE, no AI effects yet, no audio handling.
- Verify every public item in lumalens-engine, lumalens-config,
  lumalens-common has a doc comment. Where missing, add one.
  Run `cargo doc --workspace --no-deps` and ensure no warnings.
- Add `#![warn(missing_docs)]` to each library crate's lib.rs.
- Bump version to 0.1.0 in workspace Cargo.toml.
- Create a release notes draft at docs/releases/0.1.0.md: what's in,
  what's not, install instructions, screenshot.

Run the full MVP acceptance list from docs/architecture.md §18 manually.
Report which items pass and which don't. Fix the fails before I tag.
```

**Verify:** manual acceptance pass. Then:
```
git tag v0.1.0 && git push --tags
```
Create GitHub release with the notes. Post the announcement draft from Phase 2 if you're ready.

---

## Phase 5 — Packaging

### Prompt 5.1 — AUR package

```
Create `packaging/arch/PKGBUILD` for the tagged release.

- pkgname=lumalens
- pkgver / pkgrel following AUR conventions
- arch=('x86_64')
- url, license=('GPL3')
- depends: gtk4, libadwaita, gstreamer, gst-plugins-base, gst-plugins-good,
  gst-plugins-bad, gst-libav
- optdepends: 'v4l2loopback-dkms: for virtual camera output',
  'v4l-utils: for debugging camera devices'
- makedepends: cargo, pkgconf
- source: GitHub release tarball for the current version
- sha256sums: ('SKIP' for now, I'll fill after release)
- prepare(): cargo fetch --locked
- build(): cargo build --release --frozen --workspace
- package(): install -Dm755 target/release/lumalens-app
  "$pkgdir/usr/bin/lumalens", install the .desktop file and icon

Also create packaging/arch/lumalens.install with a post_install note
about Secure Boot and MOK enrollment for v4l2loopback.

Also create a parallel lumalens-git PKGBUILD that builds from main.

Do not submit to AUR from this prompt — I'll do that manually.
```

**Verify:** `makepkg -si` in `packaging/arch/` builds and installs, `lumalens` command launches the app.

---

### Prompt 5.2 — Desktop integration

```
Add Linux desktop integration files.

- data/io.github.lumalens.LumaLens.desktop:
  Name=LumaLens, Comment=Webcam customization studio,
  Exec=lumalens, Icon=io.github.lumalens.LumaLens, Categories=AudioVideo;Video;
- data/io.github.lumalens.LumaLens.svg (placeholder icon — simple
  camera glyph in a rounded square, I'll replace later)
- data/io.github.lumalens.LumaLens.metainfo.xml (AppStream metadata
  with id, name, summary, description, screenshots section with
  placeholder URL, releases from CHANGELOG).

Add install rules to PKGBUILD. Validate the metainfo with:
`appstreamcli validate data/io.github.lumalens.LumaLens.metainfo.xml`
and fix any errors.
```

**Verify:** install locally, appears in GNOME/KDE application menu, metainfo validates clean.

---

### Prompt 5.3 — Flatpak manifest (deferred decision)

```
Create packaging/flatpak/io.github.lumalens.LumaLens.yml.

- runtime: org.gnome.Platform//47
- sdk: org.gnome.Sdk//47
- sdk-extensions: org.freedesktop.Sdk.Extension.rust-stable
- build-options: append-path with the rust bin dir, env with
  CARGO_HOME and RUSTFLAGS
- finish-args:
  --share=ipc, --socket=wayland, --socket=fallback-x11,
  --device=dri, --talk-name=org.freedesktop.portal.Camera
  (NOT --device=all — Flathub reviewers will reject it)
- modules: build lumalens from local source with cargo

Add a prominent comment block at the top of the manifest explaining:
- Camera INPUT works via the portal + pipewiresrc.
- Virtual camera OUTPUT via v4l2loopback is NOT currently supported
  inside the sandbox. For virtual camera use, the native package is
  required. This is a known limitation; linking to the tracking issue.

Also open a `docs/decisions/0003-flatpak-virtual-cam.md` ADR capturing
this split: native package for the full feature set, Flatpak as a
preview/discovery vehicle only until the sandbox story improves.
```

**Verify:** `flatpak-builder --user --install build-dir packaging/flatpak/io.github.lumalens.LumaLens.yml` builds and the installed Flatpak runs with preview working (not virtual cam).

---

## Phase 6 — Fixed-stage effect pipeline

### Prompt 6.1 — Extract lumalens-effects crate

```
Introduce a fifth workspace crate: crates/lumalens-effects/.

Define the fixed-stage architecture per docs/architecture.md and the
Phase 6 plan:

pub enum Stage {
    Capture,       // source only, no effects here
    Correction,    // brightness, contrast, saturation, white balance
    Segmentation,  // (future AI)
    Composition,   // (future AI)
    Transform,     // flip, rotate, crop, zoom
    Encode,        // caps negotiation, format conversion
    Output,        // preview + virtual cam branches
}

pub trait Effect: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn stage(&self) -> Stage;
    fn is_enabled(&self) -> bool;
    fn set_enabled(&mut self, v: bool);
    /// Apply as GStreamer element config to the shared pipeline.
    /// Return a Vec<gst::Element> this effect contributes, already
    /// configured, in order. The pipeline builder splices them in.
    fn build_elements(&self) -> Result<Vec<gst::Element>, EffectError>;
}

pub struct EffectRegistry {
    effects: Vec<Box<dyn Effect>>,  // ordered within stages
}

Implementations in lumalens-effects/src/correction.rs and transform.rs:
- BrightnessEffect, ContrastEffect, SaturationEffect (each wraps a
  single videobalance element — the registry collapses multiple
  correction effects into one videobalance for performance, if that's
  practical; otherwise document why not).
- FlipEffect, RotateEffect.

Refactor lumalens-engine::pipeline to build its element chain by asking
the EffectRegistry for elements per stage, in stage order. The
tee+preview+virtual-cam branches remain hard-coded in Output.

Keep all existing EngineCommand variants working — they now route
through the registry instead of hitting pipeline elements directly.
```

**Verify:** behavior unchanged from user's perspective; `cargo test --workspace` green; `cargo tree -p lumalens-effects` shows no gtk deps.

---

### Prompt 6.2 — Effect reordering UI (within stages)

```
Add UI for reordering effects within a stage.

In Controls, add an "Effects" page (new adw::NavigationPage in a
NavigationView) listing effects grouped by stage as PreferencesGroups.
Each effect row has a drag handle (gtk::ListBox with reorderable rows).

Reordering within a stage sends EngineCommand::ReorderEffect {
  stage: Stage, from: usize, to: usize }.

Reordering ACROSS stages is disabled by UI (drag handles only work within
their group) and rejected by the engine with an error if attempted.

Persist effect order in config.toml under new [effects] section.
```

**Verify:** drag an effect within Correction stage, order persists across restart. Try to drag Correction into Transform via DevTools (if possible) — engine rejects it.

---

## Phase 7 — Optional AI

### Prompt 7.1 — lumalens-ai crate behind feature flag

```
Create crates/lumalens-ai/ as the sixth workspace member.

- Default feature of lumalens-ai: none. All ONNX code behind `features = ["ort"]`.
- lumalens-app has a feature `ai = ["lumalens-ai/ort"]`, OFF by default.
- CI matrix extends: build once without --features ai, once with.

lumalens-ai public API (feature-gated):

pub struct SegmentationModel {
    session: ort::Session,  // only exists with ort feature
    input_size: (u32, u32),
}

pub struct ModelInfo {
    pub id: &'static str,
    pub display_name: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub size_bytes: u64,
    pub license: &'static str,
}

pub fn available_models() -> &'static [ModelInfo];
pub fn is_downloaded(info: &ModelInfo) -> bool;  // checks XDG data dir
pub fn load(info: &ModelInfo) -> Result<SegmentationModel, AiError>;
pub fn segment(model: &SegmentationModel, frame: &VideoFrame) -> Result<Mask, AiError>;

For now, hardcode one model: MediaPipe Selfie Segmentation (small, Apache-2.0).
Do NOT bundle the model binary. Do NOT auto-download.
```

**Verify:** `cargo build --workspace` works. `cargo build --workspace --features ai` also works (will require `ort` system deps — document in CONTRIBUTING).

---

### Prompt 7.2 — Background blur effect (gated)

```
Add BackgroundBlurEffect in lumalens-effects, feature-gated behind
lumalens-effects/ai (which pulls lumalens-ai/ort).

- Stage: Segmentation + Composition (two cooperating effects actually —
  SegmentationEffect runs the model and attaches a mask meta to the
  buffer; BackgroundBlurEffect reads the mask and composites).
- Use appsink + appsrc for this stage only — this is the one place
  custom frame processing is justified. Document why in a comment.
- Run inference on a dedicated std::thread (NOT a tokio runtime), with
  a channel for latest-mask-wins semantics. If inference is slower than
  frame rate, composition reuses the last mask (freshness in milliseconds
  surfaced via metrics).

Add UI toggle under a new "AI Effects" PreferencesGroup, visible only
when built with --features ai. When ai feature is off, the group is
simply not added to the UI.

Add a first-run download dialog: when the user enables background blur
and the model isn't present, show an AdwMessageDialog listing name, size,
URL, license, SHA256; on confirm, download with reqwest to the XDG data
dir and verify the hash; on failure, show toast and leave the effect off.

Add reqwest ~0.12 with rustls-tls (NOT native-tls) only to lumalens-ai
under the ort feature.
```

**Verify:**
- Build with and without `--features ai`, both work
- With feature: first-run dialog appears, downloads model, blur works at ≥20fps on your dev machine
- Disable blur, close app, reopen → model stays on disk, feature re-enables without re-download

---

### Prompt 7.3 — Metrics panel

```
Add a diagnostics panel (hidden by default, shown via Ctrl+Alt+D).

lumalens-engine exposes:
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

Collected via pad probes on key elements, computed every 500ms, sent as
EngineEvent::MetricsUpdated.

UI shows a floating adw::Window with these values updating live.
Button to copy a diagnostic bundle to clipboard: metrics + last 100
log lines + GStreamer version + GTK version + distro.

Users will file bugs. This is how you get usable bug reports.
```

**Verify:** panel shows real numbers, bundle copy works, GitHub issue template can reference "paste the diagnostic bundle".

---

## Phase 8 — v1.0 and beyond (not prompts, just direction)

By now you have:
- Working app on Arch and Flatpak
- Preview, virtual cam, basic adjustments, presets
- AI blur behind a feature flag
- Decent diagnostics, OSS scaffolding, tagged releases

For v1.0, go through `docs/architecture.md` §18 acceptance list, close any gaps, add i18n via `fluent-rs` if you want (one more prompt — the time to add it is before translators exist), write a proper user manual at `docs/manual.md`, record a better demo, and announce more widely.

Beyond v1.0: plugin system is a trap for a while longer. Virtual backgrounds (not just blur), LUT support, and PipeWire-native virtual cam (if the ecosystem matures past v4l2loopback) are the natural v1.x additions.

---

## Working-with-Claude-Code reminders

- **One prompt at a time.** Paste, let it work, read the diff, verify, commit, then next.
- **Reference the plan.** Every non-trivial prompt should start with "per step N of docs/build-plan.md".
- **Reference the architecture.** "Per docs/architecture.md §X" keeps it anchored.
- **Reject hallucinated APIs.** gstreamer-rs and gtk4-rs change between versions. If Claude Code writes `foo.bar()` and it doesn't compile, don't ask it to fix — tell it to check docs.rs for the actual API first.
- **Commit after every verified prompt.** `git commit -m "step N.M: <what>"`. Easy to revert when a prompt goes sideways.
- **Keep PRs small when collaborators arrive.** One prompt usually = one PR.
- **Don't skip verify steps even when they feel tedious.** The step 1.6 coalescing bug or the step 1.8 pad-leak bug will only surface if you actually poke at them.

Honest closing note: thirty-odd prompts over four to five months of weekend-and-evening work is a realistic shape for this project solo, alongside law school. If a step takes two weekends instead of one, that's normal — GStreamer in particular punishes rushing. The sequence matters more than the pace: the walking skeleton at step 1.8 is the single most important thing to reach, because everything after it is iteration on a working product rather than hoping an architecture will come together.