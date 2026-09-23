//! Module metadata (brief section 8): "Every module declares name, category, one-line
//! explanation, lesson id, prerequisites, ports, params, quality levels." This is what the
//! catalog UI (unbuilt) and lessons (unbuilt) read — it's mandatory per brief section 4.1
//! foundational decision 3 ("the catalog cannot tell built-in, composite and code modules
//! apart"), so every `Module` impl carries a `&'static ModuleInfo`, checked from day one even
//! though nothing reads it yet.

/// Where a module sits in the catalog (brief section 8's `ModuleInfo.category` comment).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Source,
    Filter,
    Modulator,
    Sequencer,
    Utility,
    Effect,
}

/// Voice-rate modules are instanced per voice; global-rate modules are shared (brief section 4.1
/// foundational decision 4, "polyphony from day 1").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rate {
    Voice,
    Global,
}

/// Signal semantics for a port. Drives UI colour, default-cable scaling and connection hints —
/// brief section 8: "not engine behaviour," i.e. this is metadata, `process()` doesn't branch on
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortType {
    Audio,
    /// Control voltage, ±1 nominal range.
    Cv,
    /// Control voltage, 0..1 nominal range (envelope output, velocity).
    UnipolarCv,
    Gate,
    /// Float semitones, 1V/oct semantics (brief section 8).
    Pitch,
}

impl PortType {
    /// Nominal `(low, high)` range of a signal of this type. Parameter modulation scales a
    /// source by `1 / max(|low|, |high|)`, so a full-scale source moves a knob by exactly its
    /// route amount, whether the source is bipolar or unipolar. Pitch uses ±60 semitones
    /// (five octaves) as its nominal full scale.
    pub fn nominal_range(self) -> (f32, f32) {
        match self {
            PortType::Audio | PortType::Cv => (-1.0, 1.0),
            PortType::UnipolarCv | PortType::Gate => (0.0, 1.0),
            PortType::Pitch => (-60.0, 60.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PortDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy)]
pub struct PortInfo {
    pub name: &'static str,
    pub port_type: PortType,
    pub direction: PortDirection,
}

/// How a param's displayed/edited value maps to its underlying linear range — matters for knob
/// feel (a cutoff knob wants exponential taper, a mix knob wants linear). Not yet consumed by
/// anything (no UI exists), but part of the metadata contract from day one per brief section 8.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Taper {
    Linear,
    Exponential,
    /// Whole-number choices from `min` to `max` (waveform, mode switches). Linear travel,
    /// rounded to the nearest option.
    Stepped,
}

impl ParamInfo {
    /// Position of `value` along the knob's travel, 0..1, in this param's own taper
    /// (logarithmic for `Exponential`). Parameter modulation adds in this space.
    pub fn to_norm(&self, value: f32) -> f32 {
        let n = match self.taper {
            Taper::Exponential => (value / self.min).ln() / (self.max / self.min).ln(),
            Taper::Linear | Taper::Stepped => (value - self.min) / (self.max - self.min),
        };
        if n.is_finite() {
            n.clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Inverse of `to_norm`: knob travel 0..1 back to destination units. `Stepped` rounds to
    /// the nearest option.
    pub fn from_norm(&self, norm: f32) -> f32 {
        let n = norm.clamp(0.0, 1.0);
        match self.taper {
            // Clamped: exp/ln round-off can land a hair outside the range (20000.002 Hz).
            Taper::Exponential => {
                (self.min * ((self.max / self.min).ln() * n).exp()).clamp(self.min, self.max)
            }
            Taper::Linear => self.min + (self.max - self.min) * n,
            Taper::Stepped => (self.min + (self.max - self.min) * n).round(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ParamInfo {
    pub name: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: &'static str,
    pub taper: Taper,
    /// Every parameter change is smoothed (brief section 3, RT rule: "no zipper noise").
    pub smoothing_ms: f32,
}

/// Which brief-section-7 quality-tier levers this module actually honours. Not every module
/// touches every lever (e.g. `mixer` has no oversampling to speak of) — this says which ones do,
/// so quality-tier switching (not yet built) knows what to ask each module for.
#[derive(Debug, Clone, Copy, Default)]
pub struct QualitySupport {
    pub oversampling: bool,
    pub anti_aliasing: bool,
    pub interpolation: bool,
}

pub type LessonId = &'static str;

#[derive(Debug, Clone, Copy)]
pub struct ModuleInfo {
    /// Stable kind string, e.g. `"osc.va"`, `"filter.svf"` — this is what `Op::AddModule` in
    /// `kabl-core` stores (as an owned `String` there; this is the registry's static copy).
    pub kind: &'static str,
    pub name: &'static str,
    pub category: Category,
    pub rate: Rate,
    pub explain: &'static str,
    pub lesson: Option<LessonId>,
    pub requires: &'static [&'static str],
    pub ports: &'static [PortInfo],
    pub params: &'static [ParamInfo],
    pub quality: QualitySupport,
    /// Optional custom panel art + explicit control layout (owner ask, not a brief feature) — see
    /// `crate::skin`'s module doc. `None` for every built-in except one demo; a skin-aware
    /// renderer falls back to its own auto-layout when this is `None`.
    pub skin: Option<&'static crate::skin::ModuleSkin>,
    /// Panel width in rack grid units (owner ask: a Eurorack-style tiled view, modules snapped to
    /// a square grid, each declaring its own width like real HP-width hardware). A UI's grid cell
    /// size in pixels is its own choice — this is unitless, just how many cells wide. Unused by
    /// the free-form patchbay view (every module there stays a uniform width); only the tiled view
    /// reads it.
    pub width_units: u32,
    /// Params the UI keeps off the module face by default (the "advanced" controls, shown when
    /// the module expands). Every other param is a default primary control. Presentation only:
    /// the user's own per-instance choice overrides it, and the engine never reads it.
    pub advanced: &'static [&'static str],
}
