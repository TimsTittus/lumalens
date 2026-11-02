# ADR 0002 — Four-Crate Cargo Workspace

**Status:** Accepted

**Date:** 2026-11-02

---

## Context

LumaLens started as a single crate. As the engine, UI, and configuration
concerns grew it became useful to enforce their boundaries at the compiler
level — preventing accidental GTK imports in the engine, making the shared
type `CameraInfo` a stable contract, and allowing CI to verify dependency
isolation with `cargo tree`. We converted to a Cargo workspace at that point.

The obvious extreme would be one crate per module (~nine crates). We chose
four instead.

---

## Decision

### Four crates, not nine

```
lumalens-common   ←── shared types only (CameraInfo)
lumalens-config   ←── persistence (serde / toml)
lumalens-engine   ←── GStreamer pipeline, discovery, virtual-cam
lumalens-app      ←── GTK4 / Adwaita UI + binary
```

**Deferred extraction** is the rule: a module earns its own crate when it
has a real boundary that the module system cannot express — a different
dependency set, a different stability contract, or a reuse story beyond the
current binary.  Three modules we looked at and chose not to split:

| Candidate | Why it stays inside `lumalens-engine` |
|-----------|--------------------------------------|
| `camera` | Discovery is an implementation detail of the engine, not a public API consumers would call independently. |
| `virtual_cam` | One file; no deps beyond `std`. The seam isn't there yet. |
| `commands` / `events` | They only exist to bridge engine ↔ app. Pulling them into a fifth crate (`lumalens-ipc`) would create a diamond dependency with no payoff until a second binary or a C FFI exists. |

The four-crate boundary **was** earned:

- `lumalens-common` exists because `CameraInfo` crosses the engine/app
  boundary and the type must be defined exactly once.
- `lumalens-config` exists because its dependency set (`serde`, `toml`) is
  orthogonal to GStreamer and GTK — it should never pull either in.
- `lumalens-engine` exists to give the compiler a hard line: no `gtk4`, no
  `libadwaita`, no `gdk` in its `Cargo.toml`, enforced by `cargo tree`.
- `lumalens-app` exists because it is the binary entry point and the only
  place that may name GTK types.

### Dependency direction rule

```
lumalens-app
  └─ lumalens-engine   (GStreamer only)
  └─ lumalens-config   (serde / toml only)
  └─ lumalens-common   (std only)
       ↑
lumalens-engine
lumalens-config
```

The direction is strictly one-way: `app → {engine, config, common}`,
`engine → common`, `config → common`. No crate below `lumalens-app` may
depend on `gtk4`, `libadwaita`, or `gdk`.

`cargo tree -p lumalens-engine | grep -E 'gtk|adw|gdk'` is expected to
produce no output and is checked in CI.

### The one allowed exception — `paintable()` returns `glib::Object`

`Pipeline` in `lumalens-engine` builds a GStreamer pipeline that includes a
`gtk4paintablesink` element.  The sink exposes a `gdk::Paintable` property
that the preview widget needs.

Returning `gdk::Paintable` directly from `Pipeline::paintable()` would force
a `gtk4` / `gdk` compile-time dependency on `lumalens-engine` and break the
isolation rule.  Instead, `Pipeline` returns `gst::glib::Object` — the
erased GLib base type, which GStreamer already depends on — and
`lumalens-app` performs the downcast to `gdk::Paintable` after it has
already imported `gdk` for other reasons.

```rust
// lumalens-engine/src/pipeline.rs
pub fn paintable(&self) -> gst::glib::Object {
    self.sink_ref.property("paintable")
}

// lumalens-app/src/ui/window.rs
let paintable_obj: gst::glib::Object = sink.property("paintable");
let paintable = paintable_obj
    .downcast::<gdk::Paintable>()
    .expect("gtk4paintablesink paintable must implement gdk::Paintable");
```

`gst::glib` and `gtk4::glib` are the same underlying crate re-exported from
both; the downcast works at runtime via GLib's type system.

`gst-plugin-gtk4` is declared only in `lumalens-app`'s `Cargo.toml`.  The
plugin is statically linked into the binary and registered when
`gst::init()` runs, so `lumalens-engine` can create `gtk4paintablesink`
elements by name at runtime without a compile-time dep on the plugin crate.

---

## When to add more crates

### `lumalens-effects` — when there are three or more image-processing backends

Currently only `videobalance` (brightness) is wired up. Adding contrast,
saturation, and LUT support inside `lumalens-engine` is fine. Once there are
three or more distinct effect backends with different dependency sets (e.g. a
GLSL path pulling in `gstreamer-gl`, an OpenCV path), extract
`lumalens-effects` at that point. One or two effects do not earn the split.

### `lumalens-ai` — when `ort` (ONNX Runtime) is actually integrated

Background-removal and face-tracking require the `ort` crate (~40 MB of
native libs). That dependency must never land in `lumalens-engine` or it
becomes mandatory for every build. Extract `lumalens-ai` when the first
inference call is written — not before, and not as a placeholder.

### `lumalens-output` — when virtual-cam has a sibling backend

Today `virtual_cam.rs` is the only output path and it is trivial (sysfs
probe + `v4l2sink`). When a second backend exists — NDI, SRT, or RTMP — and
the backends share an abstraction, extract `lumalens-output` to own that
trait and its implementations. A single backend does not warrant a crate.

---

## Consequences

- `cargo tree -p lumalens-engine` shows no GTK/GDK nodes. Verified in CI.
- Adding a new GTK dep to `lumalens-engine` fails to compile; the boundary
  is enforced rather than just documented.
- The four-crate graph is shallow enough that `cargo check --workspace` adds
  no perceptible overhead versus a single crate.
- Crates that don't exist yet (`lumalens-effects`, `lumalens-ai`,
  `lumalens-output`) should not be created as empty placeholders. Empty
  crates add workspace overhead and signal intent that may never materialise.
