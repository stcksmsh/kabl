//! `flanger`: a stereo flanger: a short delay swept by a sine LFO, mixed with
//! the dry signal, with feedback of either sign. Each side has its own delay line; the right
//! side's sweep runs a quarter turn behind the left's.
//!
//! **Sweep.** The delay runs 0.5 ms to 6.5 ms around a 3.5 ms centre: `0.5 + 3·(1 + depth·lfo)`
//! ms, so depth 0 holds 3.5 ms and 100 % sweeps the whole range. Reads use the delay's 4-point
//! Hermite interpolation. Rate, depth and feedback take audio-rate modulation through a cable.
//!
//! **Feedback.** −95…95 %. Positive feedback rings the comb's peaks (metallic, bright);
//! negative feedback rings its notches (hollow, with the bass thinned). The delayed signal goes
//! through `tanh` before it is fed back, so the loop is bounded at any setting: the loop gain
//! never exceeds 0.95.
//!
//! **Mix.** A linear crossfade, exact at the ends (0 % is the dry input sample for sample).
//! Depth, feedback and mix glide (20 ms) when set by a knob; a patched cable is followed as it
//! is. Delay lines and the LFO phase ride across live edits in `carry_from`.

use super::delay::{line, read};
use super::fx::{blend, coef, flush, param, sin1, Glide, STEREO_IN, STEREO_OUT};
use crate::info::{Category, ModuleInfo, ParamInfo, PortInfo, QualitySupport, Rate, Taper};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};

const PORTS: &[PortInfo] = &[STEREO_IN[0], STEREO_IN[1], STEREO_OUT[0], STEREO_OUT[1]];

const PARAMS: &[ParamInfo] = &[
    param("rate_hz", 0.02, 5.0, 0.25, "Hz", Taper::Exponential),
    param("depth", 0.0, 100.0, 70.0, "%", Taper::Linear),
    param("feedback", -95.0, 95.0, 50.0, "%", Taper::Linear),
    param("mix", 0.0, 100.0, 50.0, "%", Taper::Linear),
];

pub static FLANGER_INFO: ModuleInfo = ModuleInfo {
    kind: "flanger",
    name: "Flanger",
    category: Category::Effect,
    rate: Rate::Global,
    explain: "A jet-plane sweep: a very short delay moves back and forth against the dry sound. Feedback sharpens it; a negative value gives a hollower tone.",
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
    width_units: 7,
    advanced: &[],
};

const MIN_MS: f32 = 0.5;
const SPAN_MS: f32 = 3.0;
/// Line length: longest delay (6.5 ms) plus interpolation room.
const LINE_S: f32 = 0.0075;
const GLIDE_TAU_S: f32 = 0.02;

#[derive(Debug, Clone, Copy)]
struct Core {
    write: usize,
    phase: f32,
    depth: Glide,
    feedback: Glide,
    mix: Glide,
}

pub struct Flanger {
    lines: [Vec<f32>; 2],
    c: Core,
    sample_rate: f32,
}

impl Flanger {
    pub fn new() -> Self {
        Flanger {
            lines: [Vec::new(), Vec::new()],
            c: Core {
                write: 0,
                phase: 0.0,
                depth: Glide::default(),
                feedback: Glide::default(),
                mix: Glide::default(),
            },
            sample_rate: 48000.0,
        }
    }
}

impl Default for Flanger {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Flanger {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &FLANGER_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
        let len = (LINE_S * sample_rate).ceil() as usize + 4;
        self.lines = [line(len), line(len)];
        self.c.write = 0;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let len = self.lines[0].len();
        if len == 0 {
            return;
        }
        let sr = self.sample_rate;
        let (in_l, in_r) = (io.input(0), io.input(1));
        let (rate, depth, feedback, mix) = (io.param(0), io.param(1), io.param(2), io.param(3));
        let k = coef(GLIDE_TAU_S, sr);
        let ms = 0.001 * sr;
        let c = &mut self.c;
        for i in 0..n {
            let d = c.depth.next(depth, i, k).clamp(0.0, 100.0) / 100.0;
            let fb = c.feedback.next(feedback, i, k).clamp(-95.0, 95.0) / 100.0;
            let m = c.mix.next(mix, i, k).clamp(0.0, 100.0) / 100.0;
            let x = [in_l.at(i), in_r.at(i)];
            for (ch, &xc) in x.iter().enumerate() {
                let lfo = sin1(c.phase - 0.25 * ch as f32);
                let delay = (MIN_MS + SPAN_MS * (1.0 + d * lfo)) * ms;
                let wet = read(&self.lines[ch], c.write, delay as f64);
                self.lines[ch][c.write] = flush(xc + fb * wet.tanh());
                io.output(ch)[i] = blend(xc, wet, m);
            }
            c.write = (c.write + 1) % len;
            c.phase = (c.phase + rate.at(i).clamp(0.02, 5.0) / sr).fract();
        }
    }

    fn reset(&mut self) {
        for l in &mut self.lines {
            l.fill(0.0);
        }
        self.c = Flanger::new().c;
    }

    fn carry_from(&mut self, old: &dyn Module) {
        let Some(o) = old.as_any().downcast_ref::<Flanger>() else {
            return;
        };
        if o.lines[0].len() != self.lines[0].len() || o.sample_rate != self.sample_rate {
            return;
        }
        self.lines[0].copy_from_slice(&o.lines[0]);
        self.lines[1].copy_from_slice(&o.lines[1]);
        self.c = o.c;
    }
}
