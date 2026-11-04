//! Correction-stage effects: brightness, contrast, saturation.
//!
//! All three wrap the same `videobalance` GStreamer element. The registry
//! collapses them into a single element, with each effect configuring its
//! own property.

use std::any::Any;

use gstreamer as gst;
use gstreamer::prelude::*;

use crate::{Effect, EffectError, Stage};

/// Brightness adjustment (−1.0 to 1.0, neutral = 0.0).
pub struct BrightnessEffect {
    enabled: bool,
    value: f64,
    live_element: Option<gst::Element>,
}

impl BrightnessEffect {
    /// Create with the given initial brightness.
    pub fn new(value: f64) -> Self {
        Self {
            enabled: true,
            value: value.clamp(-1.0, 1.0),
            live_element: None,
        }
    }

    /// Current brightness value.
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Update brightness, propagating to the live pipeline element.
    pub fn set_value(&mut self, v: f64) {
        self.value = v.clamp(-1.0, 1.0);
        if let Some(ref el) = self.live_element {
            el.set_property("brightness", self.value);
        }
    }
}

impl Effect for BrightnessEffect {
    fn id(&self) -> &'static str {
        "brightness"
    }
    fn name(&self) -> &'static str {
        "Brightness"
    }
    fn stage(&self) -> Stage {
        Stage::Correction
    }
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    fn set_enabled(&mut self, v: bool) {
        self.enabled = v;
    }

    fn build_elements(&self) -> Result<Vec<gst::Element>, EffectError> {
        let el = gst::ElementFactory::make("videobalance")
            .property("brightness", self.value)
            .build()
            .map_err(|e| EffectError::ElementCreation(format!("videobalance: {e}")))?;
        Ok(vec![el])
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn shared_element_factory(&self) -> Option<&'static str> {
        Some("videobalance")
    }
    fn configure_shared(&self, element: &gst::Element) {
        element.set_property("brightness", self.value);
    }
    fn bind_live_element(&mut self, element: gst::Element) {
        self.live_element = Some(element);
    }
}

/// Contrast adjustment (0.0 to 2.0, neutral = 1.0).
pub struct ContrastEffect {
    enabled: bool,
    value: f64,
    live_element: Option<gst::Element>,
}

impl ContrastEffect {
    /// Create with the given initial contrast.
    pub fn new(value: f64) -> Self {
        Self {
            enabled: true,
            value: value.clamp(0.0, 2.0),
            live_element: None,
        }
    }

    /// Current contrast value.
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Update contrast, propagating to the live pipeline element.
    pub fn set_value(&mut self, v: f64) {
        self.value = v.clamp(0.0, 2.0);
        if let Some(ref el) = self.live_element {
            el.set_property("contrast", self.value);
        }
    }
}

impl Effect for ContrastEffect {
    fn id(&self) -> &'static str {
        "contrast"
    }
    fn name(&self) -> &'static str {
        "Contrast"
    }
    fn stage(&self) -> Stage {
        Stage::Correction
    }
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    fn set_enabled(&mut self, v: bool) {
        self.enabled = v;
    }

    fn build_elements(&self) -> Result<Vec<gst::Element>, EffectError> {
        let el = gst::ElementFactory::make("videobalance")
            .property("contrast", self.value)
            .build()
            .map_err(|e| EffectError::ElementCreation(format!("videobalance: {e}")))?;
        Ok(vec![el])
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn shared_element_factory(&self) -> Option<&'static str> {
        Some("videobalance")
    }
    fn configure_shared(&self, element: &gst::Element) {
        element.set_property("contrast", self.value);
    }
    fn bind_live_element(&mut self, element: gst::Element) {
        self.live_element = Some(element);
    }
}

/// Saturation adjustment (0.0 to 2.0, neutral = 1.0).
pub struct SaturationEffect {
    enabled: bool,
    value: f64,
    live_element: Option<gst::Element>,
}

impl SaturationEffect {
    /// Create with the given initial saturation.
    pub fn new(value: f64) -> Self {
        Self {
            enabled: true,
            value: value.clamp(0.0, 2.0),
            live_element: None,
        }
    }

    /// Current saturation value.
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Update saturation, propagating to the live pipeline element.
    pub fn set_value(&mut self, v: f64) {
        self.value = v.clamp(0.0, 2.0);
        if let Some(ref el) = self.live_element {
            el.set_property("saturation", self.value);
        }
    }
}

impl Effect for SaturationEffect {
    fn id(&self) -> &'static str {
        "saturation"
    }
    fn name(&self) -> &'static str {
        "Saturation"
    }
    fn stage(&self) -> Stage {
        Stage::Correction
    }
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    fn set_enabled(&mut self, v: bool) {
        self.enabled = v;
    }

    fn build_elements(&self) -> Result<Vec<gst::Element>, EffectError> {
        let el = gst::ElementFactory::make("videobalance")
            .property("saturation", self.value)
            .build()
            .map_err(|e| EffectError::ElementCreation(format!("videobalance: {e}")))?;
        Ok(vec![el])
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn shared_element_factory(&self) -> Option<&'static str> {
        Some("videobalance")
    }
    fn configure_shared(&self, element: &gst::Element) {
        element.set_property("saturation", self.value);
    }
    fn bind_live_element(&mut self, element: gst::Element) {
        self.live_element = Some(element);
    }
}
