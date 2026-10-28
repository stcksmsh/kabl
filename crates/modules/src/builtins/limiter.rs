//! `limiter`: a stereo-linked peak limiter with instant attack and no lookahead.
//!
//! **Why no lookahead.** A lookahead limiter delays the signal by its lookahead time, and the
//! plugin reports a single fixed latency to the host (`set_latency_samples`, the timeline's
//! 64 frames); there is no per-patch latency reporting. A delayed limiter in a patch would
//! shift the sound against the host's timeline and any dry path. So this limiter adds none.
//!
//! **Method.** `gain_db` (0–24 dB) pushes the signal up into the limiter. The gain needed to
//! bring the larger of the two sides' absolute values to the ceiling is applied at once
//! (instant attack), and relaxes back to unity with a one-pole of `release_ms` once the peak
//! has passed. The output is finally clamped to ±ceiling, so no sample ever exceeds it, at any
//! input level. Because the gain moves within one sample, a very hard-driven limiter adds
//! distortion on the peaks it catches (a lookahead limiter would spread the same reduction
//! over several ms). Long releases and moderate drive keep it clean; for the cleanest result,
//! put a compressor first.
//!
//! The ceiling is read every sample; `gain_db` glides (20 ms) when set by a knob. With 0 dB of
//! gain and a signal under the ceiling the module passes the input sample for sample.

use super::fx::{coef, param, Glide, TimeCoef, NEPER_PER_DB, STEREO_IN, STEREO_OUT};
use crate::info::{Category, ModuleInfo, ParamInfo, PortInfo, QualitySupport, Rate, Taper};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};
use crate::view::ModuleView;

const PORTS: &[PortInfo] = &[STEREO_IN, STEREO_OUT];

const PARAMS: &[ParamInfo] = &[
    param("gain_db", 0.0, 24.0, 0.0, "dB", Taper::Linear),
    param("ceiling_db", -24.0, 0.0, -1.0, "dB", Taper::Linear),
    param("release_ms", 10.0, 1000.0, 100.0, "ms", Taper::Exponential),
];

pub static LIMITER_INFO: ModuleInfo = ModuleInfo {
    kind: "limiter",
    name: "Limiter",
    category: Category::Effect,
    rate: Rate::Global,
    explain: "A safety ceiling: nothing gets louder than the ceiling, however hard it is pushed. Gain pushes the sound up into it; release is how fast it lets go.",
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
    advanced: &[],
};

const GLIDE_TAU_S: f32 = 0.02;

#[derive(Debug, Clone, Copy)]
struct Core {
    /// Limiting gain, ≤ 1.
    g: f32,
    gain: Glide,
    release: TimeCoef,
    /// Ceiling in dB and as a linear value, recomputed when the dB value changes.
    ceil_db: f32,
    ceil: f32,
    /// Lowest gain over the last block, for the face's meter.
    low: f32,
}

pub struct Limiter {
    c: Core,
    sample_rate: f32,
}

impl Limiter {
    pub fn new() -> Self {
        Limiter {
            c: Core {
                g: 1.0,
                gain: Glide::default(),
                release: TimeCoef::new(),
                ceil_db: f32::NAN,
                ceil: 1.0,
                low: 1.0,
            },
            sample_rate: 48000.0,
        }
    }
}

impl Default for Limiter {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Limiter {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &LIMITER_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let sr = self.sample_rate;
        let (in_l, in_r) = (io.input(0), io.input(1));
        let (gain, ceiling, release) = (io.param(0), io.param(1), io.param(2));
        let k = coef(GLIDE_TAU_S, sr);
        let c = &mut self.c;
        let mut low = 1.0f32;
        for i in 0..n {
            let up = (c.gain.next(gain, i, k).clamp(0.0, 24.0) * NEPER_PER_DB).exp();
            let db = ceiling.at(i).clamp(-24.0, 0.0);
            if db != c.ceil_db {
                (c.ceil_db, c.ceil) = (db, (db * NEPER_PER_DB).exp());
            }
            let (l, r) = (in_l.at(i) * up, in_r.at(i) * up);
            let peak = l.abs().max(r.abs());
            let need = if peak > c.ceil { c.ceil / peak } else { 1.0 };
            let a = c.release.get(release.at(i).clamp(10.0, 1000.0), sr);
            // Relax toward unity, but never above what this sample needs.
            c.g = (1.0 - (1.0 - c.g) * a).min(need);
            low = low.min(c.g);
            io.output(0)[i] = (l * c.g).clamp(-c.ceil, c.ceil);
            io.output(1)[i] = (r * c.g).clamp(-c.ceil, c.ceil);
        }
        c.low = low;
    }

    /// Gain reduction in dB over the last block (`position`, 0 or more).
    fn view(&self, out: &mut ModuleView) {
        out.valid = true;
        out.position = -20.0 * self.c.low.max(1e-6).log10();
    }

    fn reset(&mut self) {
        self.c = Limiter::new().c;
    }

    fn carry_from(&mut self, old: &dyn Module) {
        if let Some(o) = old.as_any().downcast_ref::<Limiter>() {
            if o.sample_rate == self.sample_rate {
                self.c = o.c;
            }
        }
    }
}
