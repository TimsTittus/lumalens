//! Effect trait, fixed-stage pipeline, and built-in effects.
#![warn(missing_docs)]

pub mod correction;
pub mod transform;

use std::any::Any;
use std::collections::BTreeMap;

use gstreamer as gst;

/// Fixed stage order for the effect pipeline.
///
/// Effects are organized in a fixed stage order. Reordering is allowed
/// within a stage but not across stages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Stage {
    /// Source only — no user effects.
    Capture,
    /// Brightness, contrast, saturation, white balance.
    Correction,
    /// AI mask generation (future).
    Segmentation,
    /// AI background effects (future).
    Composition,
    /// Flip, rotate, crop, zoom.
    Transform,
    /// Caps negotiation, format conversion.
    Encode,
    /// Preview + virtual cam branches (hard-coded).
    Output,
}

impl Stage {
    /// All stages in pipeline order.
    pub const ALL: [Stage; 7] = [
        Stage::Capture,
        Stage::Correction,
        Stage::Segmentation,
        Stage::Composition,
        Stage::Transform,
        Stage::Encode,
        Stage::Output,
    ];

    /// Stages that produce user-configurable effect elements.
    pub const EFFECT_STAGES: [Stage; 4] = [
        Stage::Correction,
        Stage::Segmentation,
        Stage::Composition,
        Stage::Transform,
    ];

    /// Config-file key for this stage.
    pub fn as_str(self) -> &'static str {
        match self {
            Stage::Capture => "capture",
            Stage::Correction => "correction",
            Stage::Segmentation => "segmentation",
            Stage::Composition => "composition",
            Stage::Transform => "transform",
            Stage::Encode => "encode",
            Stage::Output => "output",
        }
    }

    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Stage::Capture => "Capture",
            Stage::Correction => "Correction",
            Stage::Segmentation => "Segmentation",
            Stage::Composition => "Composition",
            Stage::Transform => "Transform",
            Stage::Encode => "Encode",
            Stage::Output => "Output",
        }
    }
}

impl std::fmt::Display for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Errors from effect element construction.
#[derive(Debug, thiserror::Error)]
pub enum EffectError {
    /// A GStreamer element could not be created.
    #[error("failed to create GStreamer element: {0}")]
    ElementCreation(String),
}

/// A video processing effect that contributes GStreamer elements to the
/// pipeline.
pub trait Effect: Send + Sync {
    /// Unique identifier (e.g. `"brightness"`).
    fn id(&self) -> &'static str;
    /// Human-readable name.
    fn name(&self) -> &'static str;
    /// Which pipeline stage this effect belongs to.
    fn stage(&self) -> Stage;
    /// Whether this effect is active.
    fn is_enabled(&self) -> bool;
    /// Enable or disable this effect.
    fn set_enabled(&mut self, v: bool);
    /// Build standalone GStreamer elements for this effect.
    fn build_elements(&self) -> Result<Vec<gst::Element>, EffectError>;

    /// Downcast support.
    fn as_any(&self) -> &dyn Any;
    /// Downcast support (mutable).
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// If this effect shares a GStreamer element factory with other effects
    /// in the same stage, return the factory name. The registry creates one
    /// element per factory and calls [`configure_shared`](Self::configure_shared)
    /// on each effect.
    fn shared_element_factory(&self) -> Option<&'static str> {
        None
    }

    /// Set this effect's properties on a shared element.
    fn configure_shared(&self, _element: &gst::Element) {}

    /// Store a reference to the live GStreamer element for runtime updates.
    fn bind_live_element(&mut self, _element: gst::Element) {}
}

/// Ordered collection of effects, grouped by stage.
pub struct EffectRegistry {
    effects: Vec<Box<dyn Effect>>,
}

impl EffectRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            effects: Vec::new(),
        }
    }

    /// Create a registry pre-populated with the built-in effects.
    pub fn with_defaults() -> Self {
        let mut reg = Self::new();
        reg.register(Box::new(correction::BrightnessEffect::new(0.0)));
        reg.register(Box::new(correction::ContrastEffect::new(1.0)));
        reg.register(Box::new(correction::SaturationEffect::new(1.0)));
        reg.register(Box::new(transform::FlipEffect::new(
            lumalens_common::FlipMethod::None,
        )));
        reg.register(Box::new(transform::RotateEffect::new(
            lumalens_common::FlipMethod::None,
        )));
        reg
    }

    /// Add an effect to the registry.
    pub fn register(&mut self, effect: Box<dyn Effect>) {
        self.effects.push(effect);
    }

    /// All registered effects.
    pub fn effects(&self) -> &[Box<dyn Effect>] {
        &self.effects
    }

    /// All registered effects (mutable).
    pub fn effects_mut(&mut self) -> &mut [Box<dyn Effect>] {
        &mut self.effects
    }

    /// Find an effect by id and downcast to a concrete type.
    pub fn effect_mut<T: 'static>(&mut self, id: &str) -> Option<&mut T> {
        self.effects
            .iter_mut()
            .find(|e| e.id() == id)
            .and_then(|e| e.as_any_mut().downcast_mut::<T>())
    }

    /// Swap the order of two effects within a stage.
    pub fn reorder(&mut self, stage: Stage, from: usize, to: usize) {
        let stage_indices: Vec<usize> = self
            .effects
            .iter()
            .enumerate()
            .filter(|(_, e)| e.stage() == stage)
            .map(|(i, _)| i)
            .collect();

        if from < stage_indices.len() && to < stage_indices.len() {
            self.effects.swap(stage_indices[from], stage_indices[to]);
        }
    }

    /// Apply a saved effect order, reordering effects within each stage.
    pub fn apply_order(&mut self, order: &BTreeMap<String, Vec<String>>) {
        let mut by_stage: BTreeMap<Stage, Vec<Box<dyn Effect>>> = BTreeMap::new();

        for effect in self.effects.drain(..) {
            let stage = effect.stage();
            by_stage.entry(stage).or_default().push(effect);
        }

        for (stage, effects) in &mut by_stage {
            if let Some(ids) = order.get(stage.as_str()) {
                effects.sort_by_key(|e| {
                    ids.iter()
                        .position(|saved| saved == e.id())
                        .unwrap_or(usize::MAX)
                });
            }
        }

        for &stage in &Stage::ALL {
            if let Some(effects) = by_stage.remove(&stage) {
                self.effects.extend(effects);
            }
        }
    }

    /// Build GStreamer elements for a single stage.
    ///
    /// For stages whose effects share an element factory (e.g. Correction
    /// effects all use `videobalance`), a single element is created and each
    /// effect configures its property on it. All effects are bound to their
    /// live element for runtime property updates.
    pub fn build_stage_elements(&mut self, stage: Stage) -> Result<Vec<gst::Element>, EffectError> {
        if matches!(stage, Stage::Capture | Stage::Encode | Stage::Output) {
            return Ok(vec![]);
        }

        let indices: Vec<usize> = self
            .effects
            .iter()
            .enumerate()
            .filter(|(_, e)| e.stage() == stage && e.is_enabled())
            .map(|(i, _)| i)
            .collect();

        if indices.is_empty() {
            return Ok(vec![]);
        }

        let mut shared_groups: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        let mut standalone_indices = Vec::new();

        for &i in &indices {
            if let Some(factory) = self.effects[i].shared_element_factory() {
                shared_groups.entry(factory).or_default().push(i);
            } else {
                standalone_indices.push(i);
            }
        }

        let mut elements = Vec::new();

        for (factory, group) in &shared_groups {
            let element = gst::ElementFactory::make(factory)
                .build()
                .map_err(|e| EffectError::ElementCreation(format!("{factory}: {e}")))?;
            for &i in group {
                self.effects[i].configure_shared(&element);
                self.effects[i].bind_live_element(element.clone());
            }
            elements.push(element);
        }

        for &i in &standalone_indices {
            let built = self.effects[i].build_elements()?;
            if built.len() == 1 {
                self.effects[i].bind_live_element(built[0].clone());
            }
            elements.extend(built);
        }

        Ok(elements)
    }

    /// Build GStreamer elements for all effect stages, in stage order.
    pub fn build_all_effect_elements(&mut self) -> Result<Vec<gst::Element>, EffectError> {
        let mut all = Vec::new();
        for &stage in &Stage::EFFECT_STAGES {
            all.extend(self.build_stage_elements(stage)?);
        }
        Ok(all)
    }
}

impl Default for EffectRegistry {
    fn default() -> Self {
        Self::new()
    }
}
