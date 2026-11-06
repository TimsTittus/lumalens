//! Composition-stage effect: reads the segmentation mask and composites
//! the original foreground over a blurred background.
//!
//! Uses a pad probe on an `identity` element to replace each buffer with
//! a blurred+composited version. When no mask is available (inference
//! hasn't finished yet), frames pass through unchanged.

use std::any::Any;
use std::sync::Arc;

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_video as gst_video;

use crate::{Effect, EffectError, SharedMask, Stage};

const BLUR_RADIUS: usize = 12;
const BLUR_PASSES: usize = 3;

/// Blurs the background of camera frames using the segmentation mask
/// produced by [`SegmentationEffect`](super::SegmentationEffect).
pub struct BackgroundBlurEffect {
    enabled: bool,
    shared_mask: Arc<SharedMask>,
}

impl BackgroundBlurEffect {
    /// Create a new disabled background blur effect sharing `mask`.
    pub fn new(mask: Arc<SharedMask>) -> Self {
        Self {
            enabled: false,
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
        self.enabled
    }
    fn set_enabled(&mut self, v: bool) {
        self.enabled = v;
    }

    fn build_elements(&self) -> Result<Vec<gst::Element>, EffectError> {
        let shared_mask = self.shared_mask.clone();

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

    let blurred = blur_frame(original, w, h);

    let mut output = vec![0u8; pixel_count];
    for y in 0..h {
        for x in 0..w {
            let px = (y * w + x) * 4;
            let mx = x * mask_w / w;
            let my = y * mask_h / h;
            let alpha = mask[my * mask_w + mx] as u32;
            let inv = 255 - alpha;
            output[px] =
                ((original[px] as u32 * alpha + blurred[px] as u32 * inv + 127) / 255) as u8;
            output[px + 1] = ((original[px + 1] as u32 * alpha
                + blurred[px + 1] as u32 * inv
                + 127)
                / 255) as u8;
            output[px + 2] = ((original[px + 2] as u32 * alpha
                + blurred[px + 2] as u32 * inv
                + 127)
                / 255) as u8;
            output[px + 3] = original[px + 3];
        }
    }
    output
}

fn blur_frame(original: &[u8], w: usize, h: usize) -> Vec<u8> {
    let len = w * h * 4;
    let mut a = original[..len].to_vec();
    let mut b = vec![0u8; len];
    for _ in 0..BLUR_PASSES {
        box_blur_h(&a, &mut b, w, h, BLUR_RADIUS);
        box_blur_v(&b, &mut a, w, h, BLUR_RADIUS);
    }
    a
}

fn idx_clamp(val: isize, max: usize) -> usize {
    val.max(0).min(max as isize - 1) as usize
}

fn box_blur_h(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize) {
    let diam = 2 * r + 1;
    let stride = w * 4;
    for y in 0..h {
        let row = y * stride;
        let mut sum = [0u32; 3];
        for k in 0..diam {
            let sx = idx_clamp(k as isize - r as isize, w);
            let idx = row + sx * 4;
            sum[0] += src[idx] as u32;
            sum[1] += src[idx + 1] as u32;
            sum[2] += src[idx + 2] as u32;
        }
        let idx = row;
        dst[idx] = (sum[0] / diam as u32) as u8;
        dst[idx + 1] = (sum[1] / diam as u32) as u8;
        dst[idx + 2] = (sum[2] / diam as u32) as u8;
        dst[idx + 3] = src[idx + 3];

        for x in 1..w {
            let old = idx_clamp(x as isize - 1 - r as isize, w);
            let new = idx_clamp(x as isize + r as isize, w);
            let old_idx = row + old * 4;
            let new_idx = row + new * 4;
            sum[0] = sum[0] - src[old_idx] as u32 + src[new_idx] as u32;
            sum[1] = sum[1] - src[old_idx + 1] as u32 + src[new_idx + 1] as u32;
            sum[2] = sum[2] - src[old_idx + 2] as u32 + src[new_idx + 2] as u32;
            let idx = row + x * 4;
            dst[idx] = (sum[0] / diam as u32) as u8;
            dst[idx + 1] = (sum[1] / diam as u32) as u8;
            dst[idx + 2] = (sum[2] / diam as u32) as u8;
            dst[idx + 3] = src[idx + 3];
        }
    }
}

fn box_blur_v(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize) {
    let diam = 2 * r + 1;
    let stride = w * 4;
    for x in 0..w {
        let col = x * 4;
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
        dst[col + 3] = src[col + 3];

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
            dst[idx + 3] = src[idx + 3];
        }
    }
}
