# Contributing to LumaLens

Thanks for your interest in contributing! LumaLens is pre-alpha software and
contributions of all sizes are welcome — from typo fixes to new features.

## Development setup

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

### Run tests

```bash
cargo test
```

Hardware-dependent tests (camera discovery, pipeline smoke tests) are behind
a feature flag and require a connected webcam:

```bash
cargo test --features live-hardware
```

## Code style

- Format with `rustfmt` — the default `rustfmt.toml` applies.
- `cargo clippy -- -D warnings` must pass with no warnings.
- Keep comments minimal. Code should be self-explanatory; add a comment only
  when the *why* is non-obvious.

## Pull requests

- **Small and focused.** One topic per PR. A bug fix and a refactor are two
  PRs.
- **Include tests where practical.** Pure-logic helpers should have unit
  tests. UI and hardware paths are harder to test — don't force it.
- **Update CHANGELOG.md.** Add a bullet under the `[Unreleased]` section
  describing what changed.
- **CI must be green.** `cargo fmt --check`, `cargo clippy -- -D warnings`,
  and `cargo test` all pass.

## Finding a good first issue

Look for issues labelled
[`good first issue`](../../labels/good%20first%20issue). These are scoped,
well-described tasks that don't require deep familiarity with the codebase.
If none are open, feel free to open an issue proposing what you'd like to
work on.

## Questions?

Open a [Question](../../issues/new?template=question.md) issue — there are
no bad questions.
