//! `filter.formant`: a vowel filter. Five parallel band-pass resonators sit at the formant
//! frequencies of a sung vowel; `vowel` morphs continuously through a – e – i – o – u (0 … 4),
//! so a buzzy or noisy sound put through it speaks.
//!
//! **Data.** The five-formant table of a tenor voice from the Csound manual's formant
//! appendix (frequency Hz, level dB, bandwidth Hz per vowel); published reference values, not
//! derived from any one product. Between two vowels the formant frequencies interpolate in the
//! log domain (an even sweep in pitch), the levels in dB and the bandwidths in Hz.
//!
//! **Method.** Each formant is a TPT state-variable band-pass (Zavalishin, ch. 4) with
//! `k = bandwidth/frequency`, normalised to a peak gain of 1 at its centre, weighted by its
//! level and summed. `shift_st` moves every formant by semitones (−12 … 12: a smaller or
//! larger mouth); `q` (0.5 … 4) scales every formant's sharpness (1 = the table's bandwidths).
//! Formants are kept below 0.45·fs. The filter is stable at any setting (TPT with k > 0).
//!
//! **Level.** The output is the resonators only: no dry sound, and the first formant peaks at
//! unity, so a signal comes out well below its input level unless it is rich around F1–F2.
//! Patch a gain after it where level matters.
//!
//! **Modulation.** All three controls are read every sample; a knob glides (20 ms), a cable is
//! followed at audio rate. Coefficients are recomputed only while a control moves.

use super::fx::{coef, flush, param, port, Glide, NEPER_PER_DB};
use crate::info::{
    Category, ModuleInfo, ParamInfo, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};

const PORTS: &[PortInfo] = &[
    port("in", PortType::Audio, true),
    port("out", PortType::Audio, false),
];

const PARAMS: &[ParamInfo] = &[
    param("vowel", 0.0, 4.0, 0.0, "", Taper::Linear),
    param("shift_st", -12.0, 12.0, 0.0, "st", Taper::Linear),
    param("q", 0.5, 4.0, 1.0, "", Taper::Exponential),
];

pub static FILTER_FORMANT_INFO: ModuleInfo = ModuleInfo {
    kind: "filter.formant",
    name: "Formant",
    category: Category::Filter,
    rate: Rate::Voice,
    explain: "Makes a sound speak: it shapes the tone like a mouth. Vowel morphs through a, e, i, o, u; shift makes the mouth smaller or larger.",
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
    width_units: 6,
    advanced: &["q"],
};

const FORMANTS: usize = 5;
/// (Hz, dB, bandwidth Hz) per formant, for a e i o u.
const VOWELS: [[(f32, f32, f32); FORMANTS]; 5] = [
    [
        (650.0, 0.0, 80.0),
        (1080.0, -6.0, 90.0),
        (2650.0, -7.0, 120.0),
        (2900.0, -8.0, 130.0),
        (3300.0, -22.0, 140.0),
    ],
    [
        (400.0, 0.0, 70.0),
        (1700.0, -14.0, 80.0),
        (2600.0, -12.0, 100.0),
        (3200.0, -14.0, 120.0),
        (3580.0, -20.0, 120.0),
    ],
    [
        (290.0, 0.0, 40.0),
        (1870.0, -15.0, 90.0),
        (2800.0, -18.0, 100.0),
        (3250.0, -20.0, 120.0),
        (3540.0, -30.0, 120.0),
    ],
    [
        (400.0, 0.0, 40.0),
        (800.0, -10.0, 80.0),
        (2600.0, -12.0, 100.0),
        (2800.0, -12.0, 120.0),
        (3000.0, -26.0, 120.0),
    ],
    [
        (350.0, 0.0, 40.0),
        (600.0, -20.0, 60.0),
        (2700.0, -17.0, 100.0),
        (2900.0, -14.0, 120.0),
        (3300.0, -26.0, 120.0),
    ],
];
const GLIDE_TAU_S: f32 = 0.02;

#[derive(Debug, Clone, Copy)]
struct Band {
    /// TPT coefficients a1, a2, a3, the band-pass normaliser k and the level.
    co: [f32; 5],
    ic: [f32; 2],
}

#[derive(Debug, Clone, Copy)]
struct Core {
    bands: [Band; FORMANTS],
    vowel: Glide,
    shift: Glide,
    q: Glide,
    key: [f32; 3],
}

pub struct FilterFormant {
    c: Core,
    sample_rate: f32,
}

impl FilterFormant {
    pub fn new() -> Self {
        FilterFormant {
            c: Core {
                bands: [Band {
                    co: [1.0, 0.0, 0.0, 0.0, 0.0],
                    ic: [0.0; 2],
                }; FORMANTS],
                vowel: Glide::default(),
                shift: Glide::default(),
                q: Glide::default(),
                key: [f32::NAN; 3],
            },
            sample_rate: 48000.0,
        }
    }
}

impl Default for FilterFormant {
    fn default() -> Self {
        Self::new()
    }
}

/// Fills in each band's coefficients for a vowel position, shift and sharpness.
fn design(bands: &mut [Band; FORMANTS], vowel: f32, shift_st: f32, q: f32, sr: f32) {
    let v = vowel.clamp(0.0, 4.0);
    let i = (v.floor() as usize).min(3);
    let t = v - i as f32;
    let ratio = (shift_st / 12.0).exp2();
    for (b, (lo, hi)) in bands.iter_mut().zip(VOWELS[i].iter().zip(&VOWELS[i + 1])) {
        let f = (lo.0.ln() + (hi.0.ln() - lo.0.ln()) * t).exp() * ratio;
        let f = f.clamp(20.0, 0.45 * sr);
        let level = ((lo.1 + (hi.1 - lo.1) * t) * NEPER_PER_DB).exp();
        let bw = lo.2 + (hi.2 - lo.2) * t;
        let k = (bw / f / q).clamp(0.02, 2.0);
        let g = (std::f32::consts::PI * f / sr).tan();
        let a1 = 1.0 / (1.0 + g * (g + k));
        b.co = [a1, g * a1, g * g * a1, k, level];
    }
}

impl Module for FilterFormant {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &FILTER_FORMANT_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
        self.c.key = [f32::NAN; 3];
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let sr = self.sample_rate;
        let input = io.input(0);
        let (vowel, shift, qp) = (io.param(0), io.param(1), io.param(2));
        let k = coef(GLIDE_TAU_S, sr);
        let c = &mut self.c;
        for i in 0..n {
            let key = [
                c.vowel.next(vowel, i, k),
                c.shift.next(shift, i, k).clamp(-12.0, 12.0),
                c.q.next(qp, i, k).clamp(0.5, 4.0),
            ];
            if c.key != key {
                c.key = key;
                design(&mut c.bands, key[0], key[1], key[2], sr);
            }
            let x = input.at(i);
            let mut sum = 0.0;
            for b in &mut c.bands {
                let [a1, a2, a3, kk, level] = b.co;
                let v3 = x - b.ic[1];
                let v1 = a1 * b.ic[0] + a2 * v3;
                let v2 = b.ic[1] + a2 * b.ic[0] + a3 * v3;
                b.ic[0] = flush(2.0 * v1 - b.ic[0]);
                b.ic[1] = flush(2.0 * v2 - b.ic[1]);
                sum += level * kk * v1;
            }
            io.output(0)[i] = sum;
        }
    }

    fn reset(&mut self) {
        self.c = FilterFormant::new().c;
    }

    fn carry_from(&mut self, old: &dyn Module) {
        if let Some(o) = old.as_any().downcast_ref::<FilterFormant>() {
            if o.sample_rate == self.sample_rate {
                self.c = o.c;
            }
        }
    }
}
