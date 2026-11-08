//! Composition-stage effect: reads the segmentation mask and composites
//! the original foreground over a blurred background.
//!
//! Uses a pad probe on an `identity` element to replace each buffer with
//! a blurred+composited version. When no mask is available (inference
//! hasn't finished yet), frames pass through unchanged.

use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_video as gst_video;

use crate::{Effect, EffectError, SharedMask, Stage};

// Blur a downsampled copy of the frame then sample from it when
// compositing. At DOWNSCALE=4 the blur pass does ~16× less work, and the
// effective blur radius is multiplied by DOWNSCALE for free, so we can
// keep the same visual strength with a smaller kernel and fewer passes.
const DOWNSCALE: usize = 4;
const BLUR_RADIUS: usize = 3;
const BLUR_PASSES: usize = 2;

/// Blurs the background of camera frames using the segmentation mask
/// produced by [`SegmentationEffect`](super::SegmentationEffect).
///
/// The GStreamer elements are kept in the pipeline unconditionally so
/// toggling blur at runtime never requires a pipeline rebuild (which
/// would close the v4l2 virtual-camera sink and drop any consumer such
/// as a running video call). The pad probe short-circuits to a plain
/// passthrough when `active` is false.
pub struct BackgroundBlurEffect {
    active: Arc<AtomicBool>,
    shared_mask: Arc<SharedMask>,
}

impl BackgroundBlurEffect {
    /// Create a new disabled background blur effect sharing `mask`.
    pub fn new(mask: Arc<SharedMask>) -> Self {
        Self {
            active: Arc::new(AtomicBool::new(false)),
            shared_mask: mask,
        }
    }
}

impl Effect for BackgroundBlurEffect {
    fn id(&self) -> &'static str {
        "background_blur"
    }
    fn name(&self) -> &'static str {
        "Background Blur"
    }
    fn stage(&self) -> Stage {
        Stage::Composition
    }
    fn is_enabled(&self) -> bool {
        // Always kept in the pipeline; the pad probe decides whether to
        // actually composite based on `active`.
        true
    }
    fn set_enabled(&mut self, v: bool) {
        self.active.store(v, Ordering::Relaxed);
    }

    fn build_elements(&self) -> Result<Vec<gst::Element>, EffectError> {
        let shared_mask = self.shared_mask.clone();
        let active = self.active.clone();

        let convert = gst::ElementFactory::make("videoconvert")
            .build()
            .map_err(|e| EffectError::ElementCreation(format!("videoconvert: {e}")))?;

        let rgba_caps = gst::Caps::builder("video/x-raw")
            .field("format", "RGBA")
            .build();
        let capsfilter = gst::ElementFactory::make("capsfilter")
            .property("caps", &rgba_caps)
            .build()
            .map_err(|e| EffectError::ElementCreation(format!("capsfilter: {e}")))?;

        let identity = gst::ElementFactory::make("identity")
            .build()
            .map_err(|e| EffectError::ElementCreation(format!("identity: {e}")))?;

        let src_pad = identity.static_pad("src").unwrap();
        src_pad.add_probe(gst::PadProbeType::BUFFER, move |pad, info| {
            if !active.load(Ordering::Relaxed) {
                return gst::PadProbeReturn::Ok;
            }
            let mask_snapshot = shared_mask.get();
            if mask_snapshot.is_none() {
                return gst::PadProbeReturn::Ok;
            }
            let (mask_data, mask_w, mask_h, _age) = mask_snapshot.unwrap();

            let Some(caps) = pad.current_caps() else {
                return gst::PadProbeReturn::Ok;
            };
            let Ok(video_info) = gst_video::VideoInfo::from_caps(&caps) else {
                return gst::PadProbeReturn::Ok;
            };
            let w = video_info.width() as usize;
            let h = video_info.height() as usize;

            let old_buf = match info.data.take() {
                Some(gst::PadProbeData::Buffer(buf)) => buf,
                other => {
                    info.data = other;
                    return gst::PadProbeReturn::Ok;
                }
            };

            let pts = old_buf.pts();
            let dts = old_buf.dts();
            let duration = old_buf.duration();

            let composited = {
                let map = old_buf.map_readable().expect("buffer map failed");
                composite_with_blur(
                    map.as_slice(),
                    w,
                    h,
                    &mask_data,
                    mask_w as usize,
                    mask_h as usize,
                )
            };

            let mut new_buf = gst::Buffer::from_slice(composited);
            {
                let buf_ref = new_buf.get_mut().unwrap();
                buf_ref.set_pts(pts);
                buf_ref.set_dts(dts);
                buf_ref.set_duration(duration);
            }
            info.data = Some(gst::PadProbeData::Buffer(new_buf));

            gst::PadProbeReturn::Ok
        });

        Ok(vec![convert, capsfilter, identity])
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ---------------------------------------------------------------------------
// Blur + composite
// ---------------------------------------------------------------------------

fn composite_with_blur(
    original: &[u8],
    w: usize,
    h: usize,
    mask: &[u8],
    mask_w: usize,
    mask_h: usize,
) -> Vec<u8> {
    let pixel_count = w * h * 4;
    if original.len() < pixel_count {
        return original.to_vec();
    }

    // Blur runs on a 1/DOWNSCALE copy; upscaling happens implicitly when
    // we sample it below.
    let sw = (w / DOWNSCALE).max(1);
    let sh = (h / DOWNSCALE).max(1);
    let blurred_small = blur_frame_small(original, w, h, sw, sh);

    let mut output = vec![0u8; pixel_count];
    for y in 0..h {
        let my = y * mask_h / h;
        let sy = y * sh / h;
        for x in 0..w {
            let px = (y * w + x) * 4;
            let mx = x * mask_w / w;
            let alpha = mask[my * mask_w + mx] as u32;
            let inv = 255 - alpha;
            let sx = x * sw / w;
            let bpx = (sy * sw + sx) * 3;
            output[px] =
                ((original[px] as u32 * alpha + blurred_small[bpx] as u32 * inv + 127) / 255) as u8;
            output[px + 1] = ((original[px + 1] as u32 * alpha
                + blurred_small[bpx + 1] as u32 * inv
                + 127)
                / 255) as u8;
            output[px + 2] = ((original[px + 2] as u32 * alpha
                + blurred_small[bpx + 2] as u32 * inv
                + 127)
                / 255) as u8;
            output[px + 3] = original[px + 3];
        }
    }
    output
}

// Downsample RGBA → RGB at (sw, sh) using nearest sampling, then box-blur.
fn blur_frame_small(original: &[u8], w: usize, h: usize, sw: usize, sh: usize) -> Vec<u8> {
    let mut a = vec![0u8; sw * sh * 3];
    for y in 0..sh {
        let sy = y * h / sh;
        for x in 0..sw {
            let sx = x * w / sw;
            let src = (sy * w + sx) * 4;
            let dst = (y * sw + x) * 3;
            a[dst] = original[src];
            a[dst + 1] = original[src + 1];
            a[dst + 2] = original[src + 2];
        }
    }
    let mut b = vec![0u8; sw * sh * 3];
    for _ in 0..BLUR_PASSES {
        box_blur_h(&a, &mut b, sw, sh, BLUR_RADIUS);
        box_blur_v(&b, &mut a, sw, sh, BLUR_RADIUS);
    }
    a
}

fn idx_clamp(val: isize, max: usize) -> usize {
    val.max(0).min(max as isize - 1) as usize
}

fn box_blur_h(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize) {
    let diam = 2 * r + 1;
    let stride = w * 3;
    for y in 0..h {
        let row = y * stride;
        let mut sum = [0u32; 3];
        for k in 0..diam {
            let sx = idx_clamp(k as isize - r as isize, w);
            let idx = row + sx * 3;
            sum[0] += src[idx] as u32;
            sum[1] += src[idx + 1] as u32;
            sum[2] += src[idx + 2] as u32;
        }
        dst[row] = (sum[0] / diam as u32) as u8;
        dst[row + 1] = (sum[1] / diam as u32) as u8;
        dst[row + 2] = (sum[2] / diam as u32) as u8;

        for x in 1..w {
            let old = idx_clamp(x as isize - 1 - r as isize, w);
            let new_ = idx_clamp(x as isize + r as isize, w);
            let old_idx = row + old * 3;
            let new_idx = row + new_ * 3;
            sum[0] = sum[0] - src[old_idx] as u32 + src[new_idx] as u32;
            sum[1] = sum[1] - src[old_idx + 1] as u32 + src[new_idx + 1] as u32;
            sum[2] = sum[2] - src[old_idx + 2] as u32 + src[new_idx + 2] as u32;
            let idx = row + x * 3;
            dst[idx] = (sum[0] / diam as u32) as u8;
            dst[idx + 1] = (sum[1] / diam as u32) as u8;
            dst[idx + 2] = (sum[2] / diam as u32) as u8;
        }
    }
}

fn box_blur_v(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize) {
    let diam = 2 * r + 1;
    let stride = w * 3;
    for x in 0..w {
        let col = x * 3;
        let mut sum = [0u32; 3];
        for k in 0..diam {
            let sy = idx_clamp(k as isize - r as isize, h);
            let idx = sy * stride + col;
            sum[0] += src[idx] as u32;
            sum[1] += src[idx + 1] as u32;
            sum[2] += src[idx + 2] as u32;
        }
        dst[col] = (sum[0] / diam as u32) as u8;
        dst[col + 1] = (sum[1] / diam as u32) as u8;
        dst[col + 2] = (sum[2] / diam as u32) as u8;

        for y in 1..h {
            let old_y = idx_clamp(y as isize - 1 - r as isize, h);
            let new_y = idx_clamp(y as isize + r as isize, h);
            let old_idx = old_y * stride + col;
            let new_idx = new_y * stride + col;
            sum[0] = sum[0] - src[old_idx] as u32 + src[new_idx] as u32;
            sum[1] = sum[1] - src[old_idx + 1] as u32 + src[new_idx + 1] as u32;
            sum[2] = sum[2] - src[old_idx + 2] as u32 + src[new_idx + 2] as u32;
            let idx = y * stride + col;
            dst[idx] = (sum[0] / diam as u32) as u8;
            dst[idx + 1] = (sum[1] / diam as u32) as u8;
            dst[idx + 2] = (sum[2] / diam as u32) as u8;
        }
    }
}
