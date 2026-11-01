//! Integration test for camera discovery — requires real hardware.
//!
//! Run with: `cargo test --features live-hardware -- camera_discovery`

use lumalens::engine::camera;

#[test]
#[cfg_attr(not(feature = "live-hardware"), ignore)]
fn discover_finds_at_least_one_camera() {
    gstreamer::init().expect("failed to init GStreamer");

    let cameras = camera::discover().expect("discover() should succeed with hardware attached");

    assert!(!cameras.is_empty(), "expected at least one camera");

    for cam in &cameras {
        assert!(!cam.id.is_empty(), "camera id must not be empty");
        assert!(!cam.name.is_empty(), "camera name must not be empty");
        assert!(
            !cam.device_path.is_empty(),
            "camera device_path must not be empty"
        );
    }
}
