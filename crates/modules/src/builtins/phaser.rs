//! `phaser`: a stereo phase shifter. Each side runs through a chain of 2–12 first-order all-pass
//! stages whose corner frequency a sine LFO sweeps; the dry/wet blend turns the moving phase
//! into moving notches (one notch per two stages). The right side's LFO runs a quarter turn
//! behind the left's, so a mono source comes out moving across the field.
//!
//! **Sweep.** The corner centres on 800 Hz and moves ±2.5 octaves at full depth (about 140 Hz
//! to 4.5 kHz), clamped below 0.45·fs. Each stage is the TPT all-pass `y = a·x + z`,
//! `z' = x − a·y` with `a = (tan(π·f/fs) − 1)/(tan(π·f/fs) + 1)`, recomputed every sample, so
//! `rate_hz`, `depth` and `feedback` take audio-rate modulation through a cable.
//!
//! **Feedback.** The chain's output of the previous sample, through `tanh`, is fed back to its
//! input (−95…95 %). The all-pass chain has unit gain at every frequency, so the loop gain
//! stays below 0.95 and `tanh` bounds what is fed back: nothing can run away, but at full
//! feedback the notches turn into sharp resonant peaks.
//!
//! **Mix.** A linear crossfade, exact at the ends: 0 % is the dry input sample for sample and
//! 100 % the all-pass chain alone. At 50 % the notches are deepest. Depth, feedback and mix
//! glide (20 ms) when set by a knob; a patched cable is followed as it is. The LFO phase is
//! continuous whatever the rate does, and state rides across live edits in `carry_from`.

use super::fx::{blend, coef, flush, param, sin1, Glide, STEREO_IN, STEREO_OUT};
use crate::info::{Category, ModuleInfo, ParamInfo, PortInfo, QualitySupport, Rate, Taper};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};
use crate::view::ModuleView;

const PORTS: &[PortInfo] = &[STEREO_IN[0], STEREO_IN[1], STEREO_OUT[0], STEREO_OUT[1]];

const PARAMS: &[ParamInfo] = &[
    param("stages", 2.0, 12.0, 4.0, "", Taper::Stepped),
    param("rate_hz", 0.05, 10.0, 0.4, "Hz", Taper::Exponential),
    param("depth", 0.0, 100.0, 70.0, "%", Taper::Linear),
    param("feedback", -95.0, 95.0, 40.0, "%", Taper::Linear),
    param("mix", 0.0, 100.0, 50.0, "%", Taper::Linear),
];

pub static PHASER_INFO: ModuleInfo = ModuleInfo {
    kind: "phaser",
    name: "Phaser",
    category: Category::Effect,
    rate: Rate::Global,
    explain: "A swirling, hollow sweep: moving notches pass up and down the sound. More stages give more notches; feedback sharpens them.",
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
    width_units: 8,
    advanced: &["feedback"],
};

const MAX_STAGES: usize = 12;
const CENTER_HZ: f32 = 800.0;
const SWEEP_OCT: f32 = 2.5;
const GLIDE_TAU_S: f32 = 0.02;

#[derive(Debug, Clone, Copy)]
struct Core {
    phase: f32,
    z: [[f32; MAX_STAGES]; 2],
    /// Chain output of the previous sample, per side.
    prev: [f32; 2],
    depth: Glide,
    feedback: Glide,
    mix: Glide,
}

pub struct Phaser {
    c: Core,
    sample_rate: f32,
}

impl Phaser {
    pub fn new() -> Self {
        Phaser {
            c: Core {
                phase: 0.0,
                z: [[0.0; MAX_STAGES]; 2],
                prev: [0.0; 2],
                depth: Glide::default(),
                feedback: Glide::default(),
                mix: Glide::default(),
            },
            sample_rate: 48000.0,
        }
    }
}

impl Default for Phaser {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Phaser {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &PHASER_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let sr = self.sample_rate;
        let (in_l, in_r) = (io.input(0), io.input(1));
        let stages = (io.param(0).at(0).round() as usize).clamp(2, MAX_STAGES);
        let (rate, depth, feedback, mix) = (io.param(1), io.param(2), io.param(3), io.param(4));
        let k = coef(GLIDE_TAU_S, sr);
        let max_hz = 0.45 * sr;
        let c = &mut self.c;
        for i in 0..n {
            let d = c.depth.next(depth, i, k).clamp(0.0, 100.0) / 100.0;
            let fb = c.feedback.next(feedback, i, k).clamp(-95.0, 95.0) / 100.0;
            let m = c.mix.next(mix, i, k).clamp(0.0, 100.0) / 100.0;
            let x = [in_l.at(i), in_r.at(i)];
            for (ch, &xc) in x.iter().enumerate() {
                let lfo = sin1(c.phase - 0.25 * ch as f32);
                let f = (CENTER_HZ * (d * SWEEP_OCT * lfo).exp2()).clamp(20.0, max_hz);
                let t = (std::f32::consts::PI * f / sr).tan();
                let a = (t - 1.0) / (t + 1.0);
                let mut v = xc + fb * c.prev[ch].tanh();
                for z in &mut c.z[ch][..stages] {
                    let y = a * v + *z;
                    *z = flush(v - a * y);
                    v = y;
                }
                c.prev[ch] = flush(v);
                io.output(ch)[i] = blend(xc, v, m);
            }
            c.phase = (c.phase + rate.at(i).clamp(0.05, 10.0) / sr).fract();
        }
    }

    /// The LFO phase, 0..1, for the face's sweep marker.
    fn view(&self, out: &mut ModuleView) {
        out.valid = true;
        out.position = self.c.phase;
    }

    fn reset(&mut self) {
        self.c = Phaser::new().c;
    }

    fn carry_from(&mut self, old: &dyn Module) {
        if let Some(o) = old.as_any().downcast_ref::<Phaser>() {
            if o.sample_rate == self.sample_rate {
                self.c = o.c;
            }
        }
    }
}
