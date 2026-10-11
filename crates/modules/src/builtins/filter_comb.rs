//! `filter.comb`: a feedback comb filter you can play. A delay line of one period (`1/f`) with
//! its output fed back, so the sound rings at `f` and its harmonics: the plucked-string,
//! tube and metallic-resonator sound, and the Karplus–Strong string when excited by a click
//! or noise burst.
//!
//! **Pitch.** `freq_hz` is the frequency played at pitch 0 (C4 = 261.63 Hz, the oscillators'
//! convention); the `pitch` input moves it in semitones, so a keyboard or sequencer on the
//! pitch jack plays the comb in tune with an oscillator given the same pitch. The loop delay
//! is exactly `sr/f` samples: the delay is read with 4-point Hermite interpolation, and the
//! lag the damping filter adds at low frequencies is taken off the delay, so the pitch stays
//! put as damping changes. The frequency is limited to 20 Hz … `sr`/6. A frequency knob
//! glides in the log domain (10 ms); a patched `pitch` is applied as it is, at audio rate.
//!
//! **Feedback.** −97…97 %. Positive gives all harmonics; negative inverts the feedback, so
//! only odd harmonics ring and the pitch sounds an octave lower. `damp` (0–100 %) puts a
//! one-pole low-pass in the loop (pole up to 0.6): each trip round loses more highs, so the
//! ringing darkens as it decays, like a string. Loop gain is at most 0.97 and below that at
//! any frequency (the low-pass and the interpolator only lose gain), and the stored loop signal
//! is bounded to ±16, so nothing runs away.
//!
//! **Level.** The output is scaled by `sqrt(1 − fb²)`, which keeps the level of noise through
//! it the same at every feedback; a pure tone on a ringing peak still gains
//! sqrt((1 + |fb|)/(1 − |fb|)), about 18 dB at the top of the range.

use super::delay::{line, read};
use super::fx::{coef, flush, param, port, Glide};
use crate::info::{
    Category, ModuleInfo, ParamInfo, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};

const PORTS: &[PortInfo] = &[
    port("in", PortType::Audio, true),
    port("pitch", PortType::Cv, true),
    port("out", PortType::Audio, false),
];

const PARAMS: &[ParamInfo] = &[
    param("freq_hz", 20.0, 2000.0, 261.63, "Hz", Taper::Exponential),
    param("feedback", -97.0, 97.0, 80.0, "%", Taper::Linear),
    param("damp", 0.0, 100.0, 30.0, "%", Taper::Linear),
];

pub static FILTER_COMB_INFO: ModuleInfo = ModuleInfo {
    kind: "filter.comb",
    name: "Comb Filter",
    category: Category::Filter,
    rate: Rate::Voice,
    explain: "A resonator tuned to a pitch: sound fed in rings at that note and its harmonics, like a plucked string or a metal tube. Patch a keyboard to pitch to play it.",
    lesson: None,
    requires: &[],
    ports: PORTS,
    params: PARAMS,
    quality: QualitySupport {
        oversampling: false,
        anti_aliasing: false,
        interpolation: true,
    },
    skin: None,
    width_units: 6,
    advanced: &[],
};

const MIN_HZ: f32 = 20.0;
const GLIDE_TAU_S: f32 = 0.01;
const POLE_MAX: f32 = 0.6;
const LIMIT: f32 = 16.0;

#[derive(Debug, Clone, Copy)]
struct Core {
    write: usize,
    /// Damping filter state.
    lp: f32,
    /// Log2 of the frequency knob.
    log_f: Glide,
    feedback: Glide,
    damp: Glide,
}

pub struct FilterComb {
    line: Vec<f32>,
    c: Core,
    sample_rate: f32,
}

impl FilterComb {
    pub fn new() -> Self {
        FilterComb {
            line: Vec::new(),
            c: Core {
                write: 0,
                lp: 0.0,
                log_f: Glide::default(),
                feedback: Glide::default(),
                damp: Glide::default(),
            },
            sample_rate: 48000.0,
        }
    }
}

impl Default for FilterComb {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for FilterComb {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &FILTER_COMB_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
        self.line = line((sample_rate / MIN_HZ).ceil() as usize + 8);
        self.c.write = 0;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let len = self.line.len();
        if len == 0 {
            return;
        }
        let sr = self.sample_rate;
        let (input, pitch) = (io.input(0), io.input(1));
        let (freq, feedback, damp) = (io.param(0), io.param(1), io.param(2));
        let k = coef(GLIDE_TAU_S, sr);
        let (min_d, max_d) = (3.0, (len - 4) as f32);
        let c = &mut self.c;
        for i in 0..n {
            let lf = c
                .log_f
                .to(freq.at(i).clamp(MIN_HZ, 2000.0).log2(), freq.is_scalar(), k);
            let fb = c.feedback.next(feedback, i, k).clamp(-97.0, 97.0) / 100.0;
            let a = POLE_MAX * c.damp.next(damp, i, k).clamp(0.0, 100.0) / 100.0;
            let f = (lf + pitch.at(i) / 12.0).exp2().clamp(MIN_HZ, sr / 6.0);
            // The damping pole lags a/(1−a) samples at low frequency: take it off the delay.
            let d = (sr / f - a / (1.0 - a)).clamp(min_d, max_d);
            let wet = read(&self.line, c.write, d as f64);
            c.lp = flush((1.0 - a) * wet + a * c.lp);
            let y = input.at(i) + fb * c.lp;
            self.line[c.write] = flush(y.clamp(-LIMIT, LIMIT));
            c.write = (c.write + 1) % len;
            io.output(0)[i] = y * (1.0 - fb * fb).sqrt();
        }
    }

    fn reset(&mut self) {
        self.line.fill(0.0);
        self.c = FilterComb::new().c;
    }

    fn carry_from(&mut self, old: &dyn Module) {
        let Some(o) = old.as_any().downcast_ref::<FilterComb>() else {
            return;
        };
        if o.line.len() != self.line.len() || o.sample_rate != self.sample_rate {
            return;
        }
        self.line.copy_from_slice(&o.line);
        self.c = o.c;
    }
}
