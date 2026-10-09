//! `osc.fm`: a sine operator for phase-modulation FM. Wire one operator's `out` to another's
//! `pm` and the second becomes a carrier whose phase the first moves at audio rate; stack,
//! branch and loop operators as the sound needs (DX-style algorithms are patches, not a mode).
//!
//! `out = level * sin(2π (phase + (index * pm + feedback_term) / 2π))`. The frequency is
//! `base_hz * 2^(pitch/12) * ratio * 2^(fine/1200)`; `ratio` is the coarse DX-style multiple
//! (0 means one half) and `fine` detunes it by up to ±600 cents, so every inharmonic ratio from
//! 1.41 to 5.66 is a knob position away from an integer. `feedback` feeds the operator's own
//! last two outputs back into its phase (their average, so it stays musical instead of
//! chaotic) up to `FEEDBACK_MAX_RAD`.
//!
//! Aliasing: high-index FM spreads energy far past Nyquist. The operator runs at twice the
//! sample rate internally (pm input interpolated by a 6-point Lagrange midpoint, output through
//! a 33-tap Kaiser low-pass FIR), which costs 10 samples of latency per operator hop (2 for the
//! interpolation, 8 for the filter) -- docs/decisions.md "Sound engines: FM and wavetable" has
//! the measurement and the reasoning.
//!
//! `level`, `index` and `feedback` are smoothed per sample across each block, so a block-rate
//! cable into them (an envelope on `index`) never zippers.

use std::f32::consts::TAU;

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::fmdsp::{lowpass, weights4};
use crate::module::{Module, QualityConfig, StateReader, StateWriter};
use crate::view::{ModuleView, Ring};

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "pitch",
        port_type: PortType::Pitch,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "pm",
        port_type: PortType::Audio,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "out",
        port_type: PortType::Audio,
        direction: PortDirection::Output,
    },
];

const PARAMS: &[ParamInfo] = &[
    ParamInfo {
        name: "base_hz",
        min: 20.0,
        max: 20000.0,
        default: 261.63, // C4, as osc.va.
        unit: "Hz",
        taper: Taper::Exponential,
        smoothing_ms: 5.0,
    },
    // Frequency multiple: 0 = 0.5, then 1..=32.
    ParamInfo {
        name: "ratio",
        min: 0.0,
        max: 32.0,
        default: 1.0,
        unit: "x",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "fine",
        min: -600.0,
        max: 600.0,
        default: 0.0,
        unit: "ct",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "level",
        min: 0.0,
        max: 1.0,
        default: 1.0,
        unit: "",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
    // Radians of phase deviation per unit of `pm` input.
    ParamInfo {
        name: "index",
        min: 0.0,
        max: 16.0,
        default: 1.0,
        unit: "rad",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "feedback",
        min: 0.0,
        max: 1.0,
        default: 0.0,
        unit: "",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
    // Internal rate: 0 = 2x (the default), 1 = 4x (twice the cost, cleaner at extreme index).
    ParamInfo {
        name: "oversample",
        min: 0.0,
        max: 1.0,
        default: 0.0,
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    },
];

pub static OSC_FM_INFO: ModuleInfo = ModuleInfo {
    kind: "osc.fm",
    name: "FM Operator",
    category: Category::Source,
    rate: Rate::Voice,
    explain: "A sine operator: feed one into another's pm input for FM bells, keys and basses.",
    lesson: None,
    requires: &[],
    ports: PORTS,
    params: PARAMS,
    quality: QualitySupport {
        oversampling: true,
        anti_aliasing: false,
        interpolation: true,
    },
    skin: None,
    width_units: 6,
    advanced: &["base_hz", "fine", "oversample"],
};

const PITCH_IN: usize = 0;
const PM_IN: usize = 1;
const OUT: usize = 0;
const BASE_HZ_PARAM: usize = 0;
const RATIO_PARAM: usize = 1;
const FINE_PARAM: usize = 2;
const LEVEL_PARAM: usize = 3;
const INDEX_PARAM: usize = 4;
const FEEDBACK_PARAM: usize = 5;
const OVERSAMPLE_PARAM: usize = 6;

/// Names for the `ratio` choices: 0 is one half, then the integers.
pub const RATIO_LABELS: [&str; 33] = [
    "x0.5", "x1", "x2", "x3", "x4", "x5", "x6", "x7", "x8", "x9", "x10", "x11", "x12", "x13",
    "x14", "x15", "x16", "x17", "x18", "x19", "x20", "x21", "x22", "x23", "x24", "x25", "x26",
    "x27", "x28", "x29", "x30", "x31", "x32",
];

/// Phase deviation at `feedback` = 1, in radians.
pub const FEEDBACK_MAX_RAD: f32 = 2.0;

const TAPS: usize = 33;
const TAPS4: usize = 65;
/// Samples a signal is late after one operator: 2 for the `pm` interpolation, 8 for the
/// filter (16 taps at the doubled rate, 32 at four times). A modulated operator runs its own
/// phase this much behind, so it lines up with the late modulation and a 1:1 pair keeps the
/// spectrum of the ideal zero-latency pair at every pitch.
const HOP: f32 = 10.0;
/// The `modulated` estimate settles over about this long, so the phase shift never clicks.
const MODULATED_TAU_S: f32 = 0.02;

/// The sine of `x` cycles.
#[inline]
fn sin_cycles(x: f32) -> f32 {
    (x * TAU).sin()
}

#[derive(Debug, Clone, Copy)]
struct Core {
    phase: f32,
    /// The operator's last two raw outputs (feedback source).
    y1: f32,
    y2: f32,
    /// The last five `pm` input samples, newest first.
    pm: [f32; 5],
    /// The 2x-rate samples the filter still needs, newest first.
    line: [f32; TAPS4],
    /// level, index and feedback at the end of the last block; negative before the first.
    level: f32,
    index: f32,
    feedback: f32,
    /// How much of the time the `pm` input has carried signal, 0..1 (smoothed); see `HOP`.
    modulated: f32,
    /// For `view`: the last outputs and the phase step per sample.
    ring: Ring,
    dt1: f32,
}

pub struct OscFm {
    c: Core,
    sample_rate: f32,
    taps: [f32; TAPS],
    taps4: [f32; TAPS4],
    w4: [[f32; 6]; 4],
    /// 1 (measurement only), 2 (default) or 4.
    factor: usize,
    plain: bool,
}

impl OscFm {
    pub fn new() -> Self {
        OscFm {
            c: Core {
                phase: 0.0,
                y1: 0.0,
                y2: 0.0,
                pm: [0.0; 5],
                line: [0.0; TAPS4],
                level: -1.0,
                index: -1.0,
                feedback: -1.0,
                modulated: 0.0,
                ring: Ring::default(),
                dt1: 0.0,
            },
            sample_rate: 48000.0,
            taps: lowpass(2),
            taps4: lowpass(4),
            w4: weights4(),
            factor: 2,
            plain: false,
        }
    }

    /// Off runs the operator at the plain sample rate (the measurement baseline).
    pub fn set_oversample(&mut self, on: bool) {
        self.plain = !on;
        self.factor = if on { 2 } else { 1 };
    }
}

impl Default for OscFm {
    fn default() -> Self {
        Self::new()
    }
}

fn ratio_of(param: f32) -> f32 {
    let r = param.round();
    if r < 1.0 {
        0.5
    } else {
        r
    }
}

impl Module for OscFm {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &OSC_FM_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let pitch = io.input(PITCH_IN);
        let pm = io.input(PM_IN);
        let base_hz = io.param(BASE_HZ_PARAM);
        let mult =
            ratio_of(io.param(RATIO_PARAM).at(0)) * 2f32.powf(io.param(FINE_PARAM).at(0) / 1200.0);
        let level_t = io.param(LEVEL_PARAM).at(0);
        let index_t = io.param(INDEX_PARAM).at(0);
        let fb_t = io.param(FEEDBACK_PARAM).at(0);
        let n = io.block_len();
        let c = &mut self.c;
        if c.level < 0.0 {
            c.level = level_t;
            c.index = index_t;
            c.feedback = fb_t;
        }
        let (dl, di, df) = (
            (level_t - c.level) / n as f32,
            (index_t - c.index) / n as f32,
            (fb_t - c.feedback) / n as f32,
        );
        let (mut level, mut index, mut fb) = (c.level, c.index, c.feedback);
        let sr = self.sample_rate;
        if !self.plain {
            let want = if io.param_count() > OVERSAMPLE_PARAM && io.param(OVERSAMPLE_PARAM).at(0) >= 0.5 {
                4
            } else {
                2
            };
            if want != self.factor {
                self.factor = want;
                c.line = [0.0; TAPS4];
            }
        }
        let factor = self.factor;
        let factor_f = factor as f32;
        let hop = if factor > 1 { HOP } else { 0.0 };
        let has_signal = match pm {
            crate::io::Signal::Scalar(v) => v != 0.0,
            crate::io::Signal::Buffer(b) => b[..n].iter().any(|v| v.abs() > 1e-4),
        };
        let target = if has_signal { 1.0 } else { 0.0 };
        let k_mod = 1.0 - (-1.0 / (MODULATED_TAU_S * sr)).exp();
        let mut modulated = c.modulated;
        let out = &mut io.output(OUT)[..n];
        for (i, o) in out.iter_mut().enumerate() {
            level += dl;
            index += di;
            fb += df;
            modulated += (target - modulated) * k_mod;
            let dt = base_hz.at(i) * 2f32.powf(pitch.at(i) / 12.0) * mult / sr / factor_f;
            c.dt1 = dt * factor_f;
            let p0 = pm.at(i);
            // Cycles of lag, from the 1x phase increment (`dt` is per internal sample).
            let lag = -hop * modulated * dt * factor_f;
            let step = |c: &mut Core, m: f32| -> f32 {
                let fbk = fb * FEEDBACK_MAX_RAD * 0.5 * (c.y1 + c.y2);
                let y = sin_cycles(c.phase + lag + (index * m + fbk) * (1.0 / TAU));
                c.y2 = c.y1;
                c.y1 = y;
                c.phase += dt;
                c.phase -= c.phase.floor();
                y
            };
            let y = if factor == 4 {
                // Quarter points of the same interval, then its newer sample.
                let [x4, x3, x2, x1, x0] = c.pm;
                c.line.copy_within(0..TAPS4 - 4, 4);
                for k in 0..4 {
                    let w = &self.w4[k];
                    let m = w[0] * x0 + w[1] * x1 + w[2] * x2 + w[3] * x3 + w[4] * x4 + w[5] * p0;
                    c.line[3 - k] = step(c, m);
                }
                self.taps4.iter().zip(&c.line).map(|(h, x)| h * x).sum()
            } else if factor == 2 {
                // The modulation arrives two input samples late: its midpoint is interpolated
                // between the older pair of the last six (6-point Lagrange), then the pair's
                // newer sample itself.
                let [x4, x3, x2, x1, x0] = c.pm;
                let mid = (3.0 * (x0 + p0) - 25.0 * (x1 + x4) + 150.0 * (x2 + x3)) * (1.0 / 256.0);
                let a = step(c, mid);
                let b = step(c, x3);
                c.line.copy_within(0..TAPS - 2, 2);
                c.line[1] = a;
                c.line[0] = b;
                self.taps.iter().zip(&c.line).map(|(h, x)| h * x).sum()
            } else {
                step(c, p0)
            };
            c.pm = [p0, c.pm[0], c.pm[1], c.pm[2], c.pm[3]];
            // A NaN on an input must not reach the buffers downstream modules read.
            *o = if y.is_finite() { level * y } else { 0.0 };
            c.ring.push(*o);
        }
        c.level = level_t;
        c.index = index_t;
        c.feedback = fb_t;
        c.modulated = modulated;
        if !c.phase.is_finite() {
            c.phase = 0.0;
        }
        // A NaN or infinity on an input must not stay in the operator's memory.
        if !(c.y1.is_finite()
            && c.y2.is_finite()
            && c.pm.iter().chain(&c.line).all(|v| v.is_finite()))
        {
            c.y1 = 0.0;
            c.y2 = 0.0;
            c.pm = [0.0; 5];
            c.line = [0.0; TAPS4];
        }
    }

    fn view(&self, out: &mut ModuleView) {
        out.valid = self.c.dt1 > 0.0;
        if out.valid {
            self.c.ring.cycle(1.0 / self.c.dt1, &mut out.cycle);
        }
    }

    fn reset(&mut self) {
        let (sr, plain) = (self.sample_rate, self.plain);
        *self = OscFm::new();
        self.sample_rate = sr;
        self.set_oversample(!plain);
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        out.write_f32("phase", self.c.phase);
        out.write_f32("y1", self.c.y1);
        out.write_f32("y2", self.c.y2);
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        if let Some(v) = s.read_f32("phase") {
            self.c.phase = v.rem_euclid(1.0);
        }
        if let Some(v) = s.read_f32("y1") {
            self.c.y1 = v.clamp(-1.0, 1.0);
        }
        if let Some(v) = s.read_f32("y2") {
            self.c.y2 = v.clamp(-1.0, 1.0);
        }
    }

    fn carry_from(&mut self, old: &dyn Module) {
        if let Some(o) = old.as_any().downcast_ref::<OscFm>() {
            self.c = o.c;
        }
    }
}
