//! `filter.ladder`: a resonant 4-pole (24 dB/octave) low-pass in the style of a transistor
//! ladder, beside the SVF. No claim of emulating any particular hardware.
//!
//! **Method.** Four topology-preserving-transform one-pole low-passes in series with global
//! negative feedback, solved without a unit delay in the loop (Zavalishin, *The Art of VA
//! Filter Design*, rev. 2.1.2, 2018, ch. 3 "one-pole", ch. 5 "ladder filter", ch. 6 "nonlinear
//! structures"; the ladder topology is Moog's 1969 patent US 3,475,623, analysed digitally by
//! Stilson & Smith, "Analyzing the Moog VCF with considerations for digital implementation",
//! ICMC 1996). Written from the equations, no code reused. Per sample, with g = tan(π·fc/fs),
//! G = g/(1+g), stage states s1..s4:
//!
//! - linear estimate of the output: y4 = (G⁴·x + S) / (1 + k·G⁴), S = G³(1−G)s1 +
//!   G²(1−G)s2 + G(1−G)s3 + (1−G)s4 (the zero-delay feedback solution, exact for the linear
//!   filter);
//! - the input stage saturates: u = tanh(d·(c·x − k·y4)) with drive d and resonance
//!   compensation c (Zavalishin's "cheap" nonlinear ZDF: the nonlinearity uses the linear
//!   loop estimate, so there is no iteration);
//! - u runs through the four one-poles, which update their states.
//!
//! **Why it is stable.** The one-poles are TPT (bilinear, prewarped) at any sample rate and
//! cutoff, the cutoff is kept below 0.45·fs, and tanh bounds what enters the stages to ±1, so
//! nothing runs away. Musical use (full resonance, fast sweeps, full-scale input) stays below
//! about ±1.2; an adversarial test with every control jumping at once reaches ±3.5 at worst
//! (tests/ladder.rs). Above about 91 % resonance (k ≥ 4) it self-oscillates, at a level tanh
//! sets.
//!
//! **Gain staging.** Resonance k = 4.4·`resonance`. The input is scaled by c = 1 + k/2, so the
//! pass-band (DC) gain of small signals is (1 + k/2)/(1 + k): 0 dB at no resonance, −4.6 dB at
//! full resonance (a ladder without compensation loses 14.6 dB). `drive_db` (0–24 dB) is gain
//! into the saturating input stage: at 0 dB a full-scale signal is already rounded (tanh(1) =
//! 0.76), each dB of drive raises quiet signals by that dB until the stage saturates near ±1.
//! Nothing adjusts the level automatically.
//!
//! **Modulation.** `cutoff_cv` is exponential like the SVF's (cutoff × 2^cv: ±1 = ±1 octave,
//! read per sample). Keyboard tracking is an ordinary route from a pitch source to `cutoff_hz`:
//! +50 % tracks the keyboard 1:1 (a 20 Hz–20 kHz exponential knob spans 9.97 octaves, a pitch
//! route's full scale is ±60 semitones). Block-rate changes of the cutoff knob (routes,
//! macros, CC) are interpolated across the block, so sweeps don't step.

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "in",
        port_type: PortType::Audio,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "cutoff_cv",
        port_type: PortType::Cv,
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
        name: "cutoff_hz",
        min: 20.0,
        max: 20000.0,
        default: 1000.0,
        unit: "Hz",
        taper: Taper::Exponential,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "resonance",
        min: 0.0,
        max: 1.0,
        default: 0.3,
        unit: "",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "drive_db",
        min: 0.0,
        max: 24.0,
        default: 0.0,
        unit: "dB",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
];

pub static FILTER_LADDER_INFO: ModuleInfo = ModuleInfo {
    kind: "filter.ladder",
    name: "Ladder Filter",
    category: Category::Filter,
    rate: Rate::Voice,
    explain: "A warm 24 dB low-pass: steeper than the SVF, it sings at high resonance and rounds off when driven.",
    lesson: None,
    requires: &[],
    ports: PORTS,
    params: PARAMS,
    quality: QualitySupport {
        oversampling: false,
        anti_aliasing: false,
        interpolation: false,
    },
    skin: None,
    width_units: 7,
    advanced: &["drive_db"],
};

/// States this small are flushed to 0 (a decaying filter must not go subnormal).
const TINY: f32 = 1e-20;
/// Feedback at full resonance: a little past the self-oscillation threshold (k = 4), so the
/// top of the knob sings steadily at a level the input stage sets.
const K_MAX: f32 = 4.4;

#[derive(Debug, Clone, Copy)]
struct Core {
    s: [f32; 4],
    /// Prewarped g of the last sample, the start of the next block's ramp; negative at first.
    g: f32,
    /// Smoothed resonance and drive gain.
    k: f32,
    d: f32,
}

pub struct FilterLadder {
    c: Core,
    sample_rate: f32,
}

impl FilterLadder {
    pub fn new() -> Self {
        FilterLadder {
            c: Core {
                s: [0.0; 4],
                g: -1.0,
                k: -1.0,
                d: 1.0,
            },
            sample_rate: 48000.0,
        }
    }
}

impl Default for FilterLadder {
    fn default() -> Self {
        Self::new()
    }
}

#[inline]
fn prewarp(fc: f32, sr: f32) -> f32 {
    (std::f32::consts::PI * fc.clamp(10.0, 0.45 * sr) / sr).tan()
}

#[inline]
fn flush(x: f32) -> f32 {
    if x.abs() < TINY {
        0.0
    } else {
        x
    }
}

impl Module for FilterLadder {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &FILTER_LADDER_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let sr = self.sample_rate;
        let input = io.input(0);
        let cv = io.input(1);
        let cutoff = io.param(0).at(0);
        let k_target = K_MAX * io.param(1).at(0).clamp(0.0, 1.0);
        let d_target = 10f32.powf(io.param(2).at(0).clamp(0.0, 24.0) / 20.0);
        let c = &mut self.c;
        // A CV that moves inside the block needs its own coefficient per sample; otherwise the
        // block's cutoff is a linear ramp of g from the last sample's.
        let cv0 = cv.at(0);
        let cv_moves = (1..n).any(|i| cv.at(i) != cv0);
        let g_target = prewarp(cutoff * 2f32.powf(cv0), sr);
        if c.g < 0.0 {
            (c.g, c.k, c.d) = (g_target, k_target, d_target);
        }
        let (g0, k0, d0) = (c.g, c.k, c.d);
        let step = 1.0 / n as f32;
        let out = &mut io.output(0)[..n];
        for (i, o) in out.iter_mut().enumerate() {
            let t = (i + 1) as f32 * step;
            let g = if cv_moves {
                prewarp(cutoff * 2f32.powf(cv.at(i)), sr)
            } else {
                g0 + (g_target - g0) * t
            };
            let k = k0 + (k_target - k0) * t;
            let d = d0 + (d_target - d0) * t;
            let big_g = g / (1.0 + g);
            let b = 1.0 - big_g;
            let [s1, s2, s3, s4] = c.s;
            let g2 = big_g * big_g;
            let sum = g2 * big_g * b * s1 + g2 * b * s2 + big_g * b * s3 + b * s4;
            let x = input.at(i) * (1.0 + 0.5 * k);
            let y4 = (g2 * g2 * x + sum) / (1.0 + k * g2 * g2);
            let mut u = (d * (x - k * y4)).tanh();
            for s in c.s.iter_mut() {
                let v = (u - *s) * big_g;
                let y = v + *s;
                *s = flush(y + v);
                u = y;
            }
            *o = u;
            if cv_moves && i + 1 == n {
                c.g = g;
            }
        }
        if !cv_moves {
            c.g = g_target;
        }
        (c.k, c.d) = (k_target, d_target);
    }

    fn reset(&mut self) {
        self.c = FilterLadder::new().c;
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        for (i, key) in ["s1", "s2", "s3", "s4"].iter().enumerate() {
            out.write_f32(key, self.c.s[i]);
        }
        out.write_f32("g", self.c.g);
        out.write_f32("k", self.c.k);
        out.write_f32("d", self.c.d);
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        for (i, key) in ["s1", "s2", "s3", "s4"].iter().enumerate() {
            if let Some(v) = s.read_f32(key) {
                self.c.s[i] = v;
            }
        }
        for (key, slot) in [
            ("g", &mut self.c.g),
            ("k", &mut self.c.k),
            ("d", &mut self.c.d),
        ] {
            if let Some(v) = s.read_f32(key) {
                *slot = v;
            }
        }
    }
}
