//! Transform-stage effects: flip and rotate.
//!
//! Each uses its own `videoflip` GStreamer element, so both can be
//! active simultaneously (e.g. horizontal flip + 90° rotation).

use std::any::Any;

use gstreamer as gst;
use gstreamer::prelude::*;
use lumalens_common::FlipMethod;

use crate::{Effect, EffectError, Stage};

/// Mirror the video horizontally or vertically.
pub struct FlipEffect {
    enabled: bool,
    method: FlipMethod,
    live_element: Option<gst::Element>,
}

impl FlipEffect {
    /// Create with the given initial flip method.
    pub fn new(method: FlipMethod) -> Self {
        Self {
            enabled: true,
            method,
            live_element: None,
        }
    }

    /// Current flip method.
    pub fn method(&self) -> FlipMethod {
        self.method
    }

    /// Update the flip method, propagating to the live pipeline element.
    pub fn set_method(&mut self, method: FlipMethod) {
        self.method = method;
        if let Some(ref el) = self.live_element {
            el.set_property_from_str("method", method.as_gst_str());
        }
    }
}

impl Effect for FlipEffect {
    fn id(&self) -> &'static str {
        "flip"
    }
    fn name(&self) -> &'static str {
        "Flip"
    }
    fn stage(&self) -> Stage {
        Stage::Transform
    }
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    fn set_enabled(&mut self, v: bool) {
        self.enabled = v;
    }

    fn build_elements(&self) -> Result<Vec<gst::Element>, EffectError> {
        let el = gst::ElementFactory::make("videoflip")
            .property_from_str("method", self.method.as_gst_str())
            .build()
            .map_err(|e| EffectError::ElementCreation(format!("videoflip: {e}")))?;
        Ok(vec![el])
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn bind_live_element(&mut self, element: gst::Element) {
        self.live_element = Some(element);
    }
}

/// Rotate the video (90° CW, 180°, 90° CCW).
pub struct RotateEffect {
    enabled: bool,
    method: FlipMethod,
    live_element: Option<gst::Element>,
}

impl RotateEffect {
    /// Create with the given initial rotation method.
    pub fn new(method: FlipMethod) -> Self {
        Self {
            enabled: false,
            method,
            live_element: None,
        }
    }

    /// Current rotation method.
    pub fn method(&self) -> FlipMethod {
        self.method
    }

    /// Update the rotation method, propagating to the live pipeline element.
    pub fn set_method(&mut self, method: FlipMethod) {
        self.method = method;
        if let Some(ref el) = self.live_element {
            el.set_property_from_str("method", method.as_gst_str());
        }
    }
}

impl Effect for RotateEffect {
    fn id(&self) -> &'static str {
        "rotate"
    }
    fn name(&self) -> &'static str {
        "Rotate"
    }
    fn stage(&self) -> Stage {
        Stage::Transform
    }
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    fn set_enabled(&mut self, v: bool) {
        self.enabled = v;
    }

    fn build_elements(&self) -> Result<Vec<gst::Element>, EffectError> {
        let el = gst::ElementFactory::make("videoflip")
            .property_from_str("method", self.method.as_gst_str())
            .build()
            .map_err(|e| EffectError::ElementCreation(format!("videoflip: {e}")))?;
        Ok(vec![el])
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn bind_live_element(&mut self, element: gst::Element) {
        self.live_element = Some(element);
    }
}
