//! `bitcrusher`: a stereo lo-fi effect: fewer amplitude levels (bit depth) and fewer samples
//! (sample-rate reduction), blended with the dry sound.
//!
//! **Bit depth.** `bits` (1–16, fractional allowed) rounds each sample to the nearest of a
//! grid of step `2^(1 − bits)` (full scale ±1 spans 2^bits steps, mid-tread, so silence stays
//! silence). Fractional bits give a continuous sweep between whole depths. No dither: the
//! stepping noise is the sound.
//!
//! **Rate reduction.** `downsample` (1–64, fractional allowed) holds each sample for that many
//! samples, by a phase accumulator advancing `1/downsample` per sample. It is a sample-and-hold
//! with no filter before or after, so the aliasing it makes is the effect; it depends on the
//! host rate only through the factor, so a patch sounds the same at 44.1 and 192 kHz in
//! character. At 1 it holds nothing.
//!
//! **Mix.** A linear crossfade on the dry sample, exact at the ends. All three controls are
//! read every sample: a cable moves them at audio rate. Bit depth and downsample move in
//! steps by nature (no zipper to smooth); mix glides (20 ms). Both sides share one hold
//! phase, so the stereo image is kept.

use super::fx::{blend, coef, param, Glide, STEREO_IN, STEREO_OUT};
use crate::info::{Category, ModuleInfo, ParamInfo, PortInfo, QualitySupport, Rate, Taper};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};

const PORTS: &[PortInfo] = &[STEREO_IN, STEREO_OUT];

const PARAMS: &[ParamInfo] = &[
    param("bits", 1.0, 16.0, 8.0, "bit", Taper::Linear),
    param("downsample", 1.0, 64.0, 4.0, "x", Taper::Exponential),
    param("mix", 0.0, 100.0, 100.0, "%", Taper::Linear),
];

pub static BITCRUSHER_INFO: ModuleInfo = ModuleInfo {
    kind: "bitcrusher",
    name: "Bitcrusher",
    category: Category::Effect,
    rate: Rate::Global,
    explain: "Lo-fi grit: fewer bits make the sound steppy and noisy, a lower sample rate makes it gritty and aliased. Mix blends the clean sound back.",
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
    /// Hold phase, 0..1; a new sample is taken when it wraps.
    acc: f32,
    held: [f32; 2],
    mix: Glide,
}

pub struct Bitcrusher {
    c: Core,
    sample_rate: f32,
}

impl Bitcrusher {
    pub fn new() -> Self {
        Bitcrusher {
            c: Core {
                acc: 1.0,
                held: [0.0; 2],
                mix: Glide::default(),
            },
            sample_rate: 48000.0,
        }
    }
}

impl Default for Bitcrusher {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Bitcrusher {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &BITCRUSHER_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let k = coef(GLIDE_TAU_S, self.sample_rate);
        let (in_l, in_r) = (io.input(0), io.input(1));
        let (bits, down, mix) = (io.param(0), io.param(1), io.param(2));
        let c = &mut self.c;
        for i in 0..n {
            let step = (1.0 - bits.at(i).clamp(1.0, 16.0)).exp2();
            let m = c.mix.next(mix, i, k).clamp(0.0, 100.0) / 100.0;
            let x = [in_l.at(i), in_r.at(i)];
            if c.acc >= 1.0 {
                c.acc = (c.acc - 1.0).min(1.0);
                for (h, &v) in c.held.iter_mut().zip(&x) {
                    *h = (v / step).round() * step;
                }
            }
            c.acc += 1.0 / down.at(i).clamp(1.0, 64.0);
            io.output(0)[i] = blend(x[0], c.held[0], m);
            io.output(1)[i] = blend(x[1], c.held[1], m);
        }
    }

    fn reset(&mut self) {
        self.c = Bitcrusher::new().c;
    }

    fn carry_from(&mut self, old: &dyn Module) {
        if let Some(o) = old.as_any().downcast_ref::<Bitcrusher>() {
            self.c = o.c;
        }
    }
}
