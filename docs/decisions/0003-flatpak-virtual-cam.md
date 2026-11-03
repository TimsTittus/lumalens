# ADR 0003 — Flatpak as preview vehicle; native package for full feature set

**Date:** 2026-11-03  
**Status:** Accepted

---

## Context

LumaLens has two distinct distribution targets:

1. **Native packages** (currently AUR `lumalens`) — run with full access to
   the host kernel and devices.
2. **Flatpak** — run inside an XDG portal sandbox; suitable for Flathub and
   zero-setup installs.

The application's signature feature — **virtual camera output** — depends on
`v4l2loopback`, a kernel module that exposes a `/dev/videoN` loopback device.
Writing frames to that device requires `ioctl` calls that the Flatpak sandbox
blocks.  Flathub policy also forbids `--device=all`, which would otherwise
grant broad device access; reviewers reject manifests that include it.

Camera *input* (reading from a real webcam) works cleanly inside the sandbox
via the XDG Camera portal (`org.freedesktop.portal.Camera`) and a
`pipewiresrc` GStreamer element, so a sandboxed build is still useful.

---

## Decision

Ship two distinct packaging artifacts with explicitly different feature scopes:

| Package | Virtual camera | Camera input | Distribution |
|---------|---------------|--------------|--------------|
| Native (AUR) | ✅ | ✅ | AUR, future distro repos |
| Flatpak | ❌ (sandbox limitation) | ✅ | Flathub, direct install |

The Flatpak manifest (`packaging/flatpak/io.github.timstittus.lumalens.yml`)
carries a prominent comment block that:

- Explains that virtual camera output is unavailable in the sandbox.
- Points to the tracking issue for the sandbox story.
- Directs virtual-camera users to the native package.

The Flatpak build is intentionally positioned as a **preview and discovery
vehicle**: new users can install it with zero setup and explore all features
except virtual camera output.  The native package remains the recommended
install for anyone who needs the full feature set.

---

## Alternatives considered

### `--device=all`

Would grant access to all `/dev` nodes, potentially including the loopback
device.  Rejected: Flathub policy forbids it, and it is a broader privilege
than LumaLens needs.

### Bundle a user-space v4l2loopback workaround

No stable user-space path exists.  The kernel module is the only supported
mechanism.  Exploring `v4l2loopback` via FUSE or eBPF is an active research
area in the broader ecosystem but nothing is production-ready.

### Ship Flatpak only

Would permanently lose virtual camera for sandboxed users.  The native package
gives the full experience today; removing it serves no one.

### Gate the feature at runtime with a friendly error

Already done: LumaLens detects the missing loopback device and surfaces an
in-app message.  This ADR governs the packaging split, not the runtime UX.

---

## Consequences

- The Flatpak page on Flathub (if/when submitted) must clearly document the
  virtual-camera limitation in its description to avoid support confusion.
- The README and release notes should call out the split so users choose the
  right package up front.
- If the XDG portal gains a standardised virtual-camera API (tracked at the
  issue linked in the manifest), this decision should be revisited and the
  Flatpak promoted to feature-complete.

---

## Tracking issue

https://github.com/TimsTittus/lumalens/issues  
(search label `flatpak-sandbox` or title "flatpak virtual camera")
