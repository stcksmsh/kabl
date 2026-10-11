//! `eq`: a four-band stereo equaliser: a low shelf, two parametric bells and a high shelf.
//!
//! **Why this shape.** Shelves do the broad tone work a musician reaches for first (more
//! weight, more air); two bells with their own frequency, gain and Q cover the targeted cuts
//! and boosts (a boxy 300 Hz, a harsh 3 kHz) with the two ranges overlapping so that either
//! can go where it is needed. Fixed filters at the ends and moving ones in the middle is the
//! classic channel-strip layout; more bands cost faces and CPU for little gain in a synth
//! rack. Cut filters (high-pass, low-pass) are already `filter.svf` and `filter.ladder`.
//!
//! **Method.** Four TPT state-variable filters in series per side, each mixing its band-pass
//! and low-pass states into a bell, a low shelf or a high shelf (Andrew Simper, Cytomic,
//! "Solving the continuous SVF equations using trapezoidal integration and equivalent currents",
//! 2013; the cookbook-free "linear trapezoidal optimised" form). Unlike a cookbook biquad, whose
//! state can blow up when the coefficients change every sample, this structure stays bounded
//! under audio-rate modulation of frequency, gain and Q. State and coefficients are `f64`.
//! At 0 dB a band's mix is exactly `m0 = 1`, `m1 = m2 = 0`, so the module is transparent: with
//! every gain at 0 dB the output equals the input sample for sample.
//!
//! **Modulation.** Every parameter is read each sample. The coefficients are recomputed only
//! while a band's controls actually move (a knob's glide, 20 ms, or a cable); a still band
//! costs four multiplies and no trigonometry. Gains and frequencies set by a knob glide, so
//! there is no zipper noise; a cable into a frequency or gain is followed at audio rate.

use super::fx::{coef, flush, param, Glide, STEREO_IN, STEREO_OUT};
use crate::info::{Category, ModuleInfo, ParamInfo, PortInfo, QualitySupport, Rate, Taper};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};

const PORTS: &[PortInfo] = &[STEREO_IN[0], STEREO_IN[1], STEREO_OUT[0], STEREO_OUT[1]];

const PARAMS: &[ParamInfo] = &[
    param("low_hz", 20.0, 1000.0, 100.0, "Hz", Taper::Exponential),
    param("low_db", -18.0, 18.0, 0.0, "dB", Taper::Linear),
    param("mid1_hz", 50.0, 5000.0, 400.0, "Hz", Taper::Exponential),
    param("mid1_db", -18.0, 18.0, 0.0, "dB", Taper::Linear),
    param("mid1_q", 0.2, 10.0, 1.0, "", Taper::Exponential),
    param("mid2_hz", 200.0, 16000.0, 3000.0, "Hz", Taper::Exponential),
    param("mid2_db", -18.0, 18.0, 0.0, "dB", Taper::Linear),
    param("mid2_q", 0.2, 10.0, 1.0, "", Taper::Exponential),
    param("high_hz", 1000.0, 20000.0, 8000.0, "Hz", Taper::Exponential),
    param("high_db", -18.0, 18.0, 0.0, "dB", Taper::Linear),
];

pub static EQ_INFO: ModuleInfo = ModuleInfo {
    kind: "eq",
    name: "EQ",
    category: Category::Effect,
    rate: Rate::Global,
    explain: "Shapes the tone: a low shelf and a high shelf for weight and air, and two bells you can place anywhere to cut or boost one region. All gains at 0 dB leave the sound untouched.",
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
    width_units: 10,
    advanced: &["mid1_q", "mid2_q"],
};

const GLIDE_TAU_S: f32 = 0.02;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    LowShelf,
    Bell,
    HighShelf,
}

/// TPT state-variable coefficients `[a1, a2, a3, m0, m1, m2]` of one band.
fn design(kind: Kind, f: f32, db: f32, q: f32, sr: f32) -> [f64; 6] {
    let f = f64::from(f.clamp(10.0, 0.45 * sr));
    let a = 10f64.powf(f64::from(db) / 40.0);
    let t = (std::f64::consts::PI * f / f64::from(sr)).tan();
    // Shelves use a fixed slope (Q = 1/sqrt 2), the bell its Q.
    let shelf_k = std::f64::consts::SQRT_2;
    let (g, k, m) = match kind {
        Kind::Bell => {
            let k = 1.0 / (f64::from(q) * a);
            (t, k, [1.0, k * (a * a - 1.0), 0.0])
        }
        Kind::LowShelf => (
            t / a.sqrt(),
            shelf_k,
            [1.0, shelf_k * (a - 1.0), a * a - 1.0],
        ),
        Kind::HighShelf => (
            t * a.sqrt(),
            shelf_k,
            [a * a, shelf_k * (1.0 - a) * a, 1.0 - a * a],
        ),
    };
    let a1 = 1.0 / (1.0 + g * (g + k));
    [a1, g * a1, g * g * a1, m[0], m[1], m[2]]
}

/// One band: its controls' glides, the coefficients they gave, and per-side filter state.
#[derive(Debug, Clone, Copy)]
struct Band {
    f: Glide,
    db: Glide,
    q: Glide,
    key: [f32; 3],
    co: [f64; 6],
    /// State `ic1eq`, `ic2eq` per side.
    z: [[f64; 2]; 2],
}

impl Band {
    fn new() -> Self {
        Band {
            f: Glide::default(),
            db: Glide::default(),
            q: Glide::default(),
            key: [f32::NAN; 3],
            co: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            z: [[0.0; 2]; 2],
        }
    }

    #[inline]
    fn tick(&mut self, ch: usize, x: f32) -> f32 {
        let [a1, a2, a3, m0, m1, m2] = self.co;
        let x = f64::from(x);
        let z = &mut self.z[ch];
        let v3 = x - z[1];
        let v1 = a1 * z[0] + a2 * v3;
        let v2 = z[1] + a2 * z[0] + a3 * v3;
        z[0] = 2.0 * v1 - z[0];
        z[1] = 2.0 * v2 - z[1];
        flush((m0 * x + m1 * v1 + m2 * v2) as f32)
    }
}

pub struct Eq {
    bands: [Band; 4],
    sample_rate: f32,
}

impl Eq {
    pub fn new() -> Self {
        Eq {
            bands: [Band::new(); 4],
            sample_rate: 48000.0,
        }
    }
}

impl Default for Eq {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Eq {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &EQ_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        const KINDS: [Kind; 4] = [Kind::LowShelf, Kind::Bell, Kind::Bell, Kind::HighShelf];
        let n = io.block_len();
        let sr = self.sample_rate;
        let (in_l, in_r) = (io.input(0), io.input(1));
        let k = coef(GLIDE_TAU_S, sr);
        // Param index of each band's (freq, gain, q); the shelves have no q.
        let at = [(0, 1, None), (2, 3, Some(4)), (5, 6, Some(7)), (8, 9, None)];
        for i in 0..n {
            let mut x = [in_l.at(i), in_r.at(i)];
            for (b, (band, &(pf, pg, pq))) in self.bands.iter_mut().zip(&at).enumerate() {
                let f = band.f.next(io.param(pf), i, k);
                let db = band.db.next(io.param(pg), i, k).clamp(-18.0, 18.0);
                let q = pq.map_or(1.0, |p| band.q.next(io.param(p), i, k).clamp(0.2, 10.0));
                if band.key != [f, db, q] {
                    band.key = [f, db, q];
                    band.co = design(KINDS[b], f, db, q, sr);
                }
                x[0] = band.tick(0, x[0]);
                x[1] = band.tick(1, x[1]);
            }
            io.output(0)[i] = x[0];
            io.output(1)[i] = x[1];
        }
    }

    fn reset(&mut self) {
        self.bands = [Band::new(); 4];
    }

    fn carry_from(&mut self, old: &dyn Module) {
        if let Some(o) = old.as_any().downcast_ref::<Eq>() {
            if o.sample_rate == self.sample_rate {
                self.bands = o.bands;
            }
        }
    }
}
