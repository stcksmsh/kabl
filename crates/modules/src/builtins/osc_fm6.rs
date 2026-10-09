//! `osc.fm6`: a six-operator FM voice. Six sine operators, each with its own ratio, fine tune,
//! level, ADSR and velocity sensitivity, wired by one of eight algorithms (who modulates whom,
//! who is heard). The whole voice runs at 2x (or 4x) the sample rate and is filtered down once,
//! at the end, so operators meet each other with no latency: a modulator and its carrier keep
//! their ideal phase relationship at every pitch, however deep the chain, and an operator
//! costs one sine per internal sample, nothing more.
//!
//! `out = sum over carriers of o_k / sqrt(carriers)`, `o_k = amp_k * sin(2π (phase_k +
//! (index * 8 * sum of its modulators' o_j + feedback_k) / 2π))`, `amp_k = level_k * env_k *
//! velocity factor`. Operator numbers run 1..6 and a modulator always has a higher number than
//! its target, so the voice is evaluated from 6 down to 1. Operator 6 can feed back on itself
//! (`feedback`). A note-on (rising `gate`) restarts every phase at 0 (key sync, as a DX does),
//! so a note always starts with the same spectrum.

use std::f32::consts::TAU;

use crate::fmdsp::lowpass;
use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};
use crate::view::{ModuleView, Ring};

pub const OPS: usize = 6;

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "pitch",
        port_type: PortType::Pitch,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "gate",
        port_type: PortType::Gate,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "velocity",
        port_type: PortType::UnipolarCv,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "out",
        port_type: PortType::Audio,
        direction: PortDirection::Output,
    },
];

macro_rules! op_params {
    ($($n:literal $level:literal),*) => {
        [
            ParamInfo { name: "base_hz", min: 20.0, max: 20000.0, default: 261.63, unit: "Hz", taper: Taper::Exponential, smoothing_ms: 5.0 },
            ParamInfo { name: "algorithm", min: 0.0, max: 7.0, default: 0.0, unit: "", taper: Taper::Stepped, smoothing_ms: 0.0 },
            ParamInfo { name: "feedback", min: 0.0, max: 1.0, default: 0.0, unit: "", taper: Taper::Linear, smoothing_ms: 0.0 },
            ParamInfo { name: "index", min: 0.0, max: 2.0, default: 1.0, unit: "", taper: Taper::Linear, smoothing_ms: 0.0 },
            ParamInfo { name: "fine", min: -100.0, max: 100.0, default: 0.0, unit: "ct", taper: Taper::Linear, smoothing_ms: 0.0 },
            ParamInfo { name: "oversample", min: 0.0, max: 1.0, default: 0.0, unit: "", taper: Taper::Stepped, smoothing_ms: 0.0 },
            $(
                ParamInfo { name: concat!("ratio", $n), min: 0.0, max: 32.0, default: 1.0, unit: "x", taper: Taper::Stepped, smoothing_ms: 0.0 },
                ParamInfo { name: concat!("fine", $n), min: -600.0, max: 600.0, default: 0.0, unit: "ct", taper: Taper::Linear, smoothing_ms: 0.0 },
                ParamInfo { name: concat!("level", $n), min: 0.0, max: 1.0, default: $level, unit: "", taper: Taper::Linear, smoothing_ms: 0.0 },
                ParamInfo { name: concat!("attack", $n), min: 0.1, max: 10000.0, default: 1.0, unit: "ms", taper: Taper::Exponential, smoothing_ms: 0.0 },
                ParamInfo { name: concat!("decay", $n), min: 0.1, max: 10000.0, default: 300.0, unit: "ms", taper: Taper::Exponential, smoothing_ms: 0.0 },
                ParamInfo { name: concat!("sustain", $n), min: 0.0, max: 1.0, default: 0.7, unit: "", taper: Taper::Linear, smoothing_ms: 0.0 },
                ParamInfo { name: concat!("release", $n), min: 0.1, max: 10000.0, default: 200.0, unit: "ms", taper: Taper::Exponential, smoothing_ms: 0.0 },
                ParamInfo { name: concat!("vel", $n), min: 0.0, max: 1.0, default: 0.0, unit: "", taper: Taper::Linear, smoothing_ms: 0.0 },
            )*
        ]
    };
}

static PARAMS: [ParamInfo; 6 + 8 * OPS] =
    op_params!("1" 1.0, "2" 0.3, "3" 0.0, "4" 0.0, "5" 0.0, "6" 0.0);

pub static OSC_FM6_INFO: ModuleInfo = ModuleInfo {
    kind: "osc.fm6",
    name: "FM Voice (6 operators)",
    category: Category::Source,
    rate: Rate::Voice,
    explain: "Six FM operators with their own envelopes in eight standard routings: electric pianos, bells, brass, basses.",
    lesson: None,
    requires: &[],
    ports: PORTS,
    params: &PARAMS,
    quality: QualitySupport {
        oversampling: true,
        anti_aliasing: false,
        interpolation: true,
    },
    skin: None,
    width_units: 10,
    advanced: &["base_hz", "fine", "oversample"],
};

const PITCH_IN: usize = 0;
const GATE_IN: usize = 1;
const VELOCITY_IN: usize = 2;
const OUT: usize = 0;
const BASE_HZ: usize = 0;
const ALGORITHM: usize = 1;
const FEEDBACK: usize = 2;
const INDEX: usize = 3;
const FINE: usize = 4;
const OVERSAMPLE: usize = 5;
const FIRST_OP: usize = 6;
const PER_OP: usize = 8;
const RATIO: usize = 0;
const OP_FINE: usize = 1;
const LEVEL: usize = 2;
const ATTACK: usize = 3;
const DECAY: usize = 4;
const SUSTAIN: usize = 5;
const RELEASE: usize = 6;
const VEL: usize = 7;

/// Radians of phase deviation a modulator of full output (level 1) adds at `index` = 1.
pub const MOD_RAD: f32 = 8.0;
/// Phase deviation at `feedback` = 1, radians.
pub const FEEDBACK_RAD: f32 = 2.0;

pub const ALGORITHM_LABELS: [&str; 8] = [
    "6>5>4>3>2>1",
    "2>1 4>3 6>5",
    "3>2>1 6>5>4",
    "2>1 6>5>4>3",
    "2-6>1",
    "6>1-5",
    "6>4>2>1 5>3>1",
    "ADD",
];

/// Bit `j` of `mods[k]`: operator `j + 1` modulates operator `k + 1` (always `j > k`).
pub struct Algorithm {
    pub mods: [u8; OPS],
    pub carriers: u8,
}

pub const ALGORITHMS: [Algorithm; 8] = [
    Algorithm {
        mods: [1 << 1, 1 << 2, 1 << 3, 1 << 4, 1 << 5, 0],
        carriers: 0b000001,
    },
    Algorithm {
        mods: [1 << 1, 0, 1 << 3, 0, 1 << 5, 0],
        carriers: 0b010101,
    },
    Algorithm {
        mods: [1 << 1, 1 << 2, 0, 1 << 4, 1 << 5, 0],
        carriers: 0b001001,
    },
    Algorithm {
        mods: [1 << 1, 0, 1 << 3, 1 << 4, 1 << 5, 0],
        carriers: 0b000101,
    },
    Algorithm {
        mods: [0b111110, 0, 0, 0, 0, 0],
        carriers: 0b000001,
    },
    Algorithm {
        mods: [1 << 5, 1 << 5, 1 << 5, 1 << 5, 1 << 5, 0],
        carriers: 0b011111,
    },
    Algorithm {
        mods: [(1 << 1) | (1 << 2), 1 << 3, 1 << 4, 1 << 5, 0, 0],
        carriers: 0b000001,
    },
    Algorithm {
        mods: [0; OPS],
        carriers: 0b111111,
    },
];

const TAPS: usize = 33;
const TAPS4: usize = 65;
const IDLE: u8 = 0;
const ATTACKING: u8 = 1;
const DECAYING: u8 = 2;
const RELEASING: u8 = 3;
/// An operator below this is silent: its sine is skipped.
const SILENT: f32 = 1e-6;

#[derive(Debug, Clone, Copy)]
struct Core {
    phase: [f32; OPS],
    env: [f32; OPS],
    stage: [u8; OPS],
    velocity: [f32; OPS],
    /// Output of operator 6 at the last two internal samples (feedback source).
    y1: f32,
    y2: f32,
    last_gate: f32,
    line: [f32; TAPS4],
    /// Smoothed levels, index and feedback; negative before the first block.
    level: [f32; OPS],
    index: f32,
    feedback: f32,
    /// Samples of continuous silence, so a finished voice costs nothing.
    quiet: u32,
    ring: Ring,
    dt1: f32,
}

pub struct OscFm6 {
    c: Core,
    sample_rate: f32,
    taps: [f32; TAPS],
    taps4: [f32; TAPS4],
    factor: usize,
}

impl OscFm6 {
    pub fn new() -> Self {
        OscFm6 {
            c: Core {
                phase: [0.0; OPS],
                env: [0.0; OPS],
                stage: [IDLE; OPS],
                velocity: [1.0; OPS],
                y1: 0.0,
                y2: 0.0,
                last_gate: 0.0,
                line: [0.0; TAPS4],
                level: [-1.0; OPS],
                index: -1.0,
                feedback: -1.0,
                quiet: u32::MAX,
                ring: Ring::default(),
                dt1: 0.0,
            },
            sample_rate: 48000.0,
            taps: lowpass(2),
            taps4: lowpass(4),
            factor: 2,
        }
    }
}

impl Default for OscFm6 {
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

/// Per-sample step coefficient for a segment that settles (to -40 dB) in `ms`.
fn settle(ms: f32, sr: f32) -> f32 {
    1.0 - (-4.6 / (ms.max(0.1) * 0.001 * sr)).exp()
}

impl Module for OscFm6 {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &OSC_FM6_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    fn process(&mut self, io: &mut ProcessIo) {
        let pitch = io.input(PITCH_IN);
        let gate = io.input(GATE_IN);
        let velocity = io.input(VELOCITY_IN);
        let base_hz = io.param(BASE_HZ);
        let algorithm = &ALGORITHMS[(io.param(ALGORITHM).at(0).round().max(0.0) as usize).min(7)];
        let fb_t = io.param(FEEDBACK).at(0);
        let index_t = io.param(INDEX).at(0);
        let master = 2f32.powf(io.param(FINE).at(0) / 1200.0);
        let want = if io.param(OVERSAMPLE).at(0) >= 0.5 {
            4
        } else {
            2
        };
        let n = io.block_len();
        let sr = self.sample_rate;
        let c = &mut self.c;
        if want != self.factor {
            self.factor = want;
            c.line = [0.0; TAPS4];
        }
        let factor = self.factor;
        let factor_f = factor as f32;

        let (mut ratio, mut lvl_t) = ([0.0f32; OPS], [0.0f32; OPS]);
        let (mut a_step, mut d_k, mut r_k) = ([0.0f32; OPS], [0.0f32; OPS], [0.0f32; OPS]);
        let (mut sustain, mut vel) = ([0.0f32; OPS], [0.0f32; OPS]);
        for k in 0..OPS {
            let at = |field: usize| io.param(FIRST_OP + k * PER_OP + field).at(0);
            ratio[k] = ratio_of(at(RATIO)) * 2f32.powf(at(OP_FINE) / 1200.0);
            lvl_t[k] = at(LEVEL);
            a_step[k] = 1.0 / (at(ATTACK).max(0.1) * 0.001 * sr);
            d_k[k] = settle(at(DECAY), sr);
            r_k[k] = settle(at(RELEASE), sr);
            sustain[k] = at(SUSTAIN);
            vel[k] = at(VEL);
        }
        if c.index < 0.0 {
            c.level = lvl_t;
            c.index = index_t;
            c.feedback = fb_t;
        }
        let step_l: [f32; OPS] = std::array::from_fn(|k| (lvl_t[k] - c.level[k]) / n as f32);
        let (step_i, step_f) = (
            (index_t - c.index) / n as f32,
            (fb_t - c.feedback) / n as f32,
        );
        let carriers = algorithm.carriers.count_ones().max(1) as f32;
        let out_gain = 1.0 / carriers.sqrt();

        let out = &mut io.output(OUT)[..n];
        for (i, o) in out.iter_mut().enumerate() {
            for (l, d) in c.level.iter_mut().zip(&step_l) {
                *l += d;
            }
            c.index += step_i;
            c.feedback += step_f;

            let g = gate.at(i);
            if g > 0.5 && c.last_gate <= 0.5 {
                c.phase = [0.0; OPS];
                c.stage = [ATTACKING; OPS];
                let v = velocity.at(i);
                c.velocity = vel.map(|s| (1.0 - s) + s * v);
                c.y1 = 0.0;
                c.y2 = 0.0;
            } else if g <= 0.5 && c.last_gate > 0.5 {
                for st in &mut c.stage {
                    if *st != IDLE {
                        *st = RELEASING;
                    }
                }
            }
            c.last_gate = g;

            let mut amp = [0.0f32; OPS];
            let mut any = false;
            for k in 0..OPS {
                match c.stage[k] {
                    ATTACKING => {
                        c.env[k] += a_step[k];
                        if c.env[k] >= 1.0 {
                            c.env[k] = 1.0;
                            c.stage[k] = DECAYING;
                        }
                    }
                    DECAYING => c.env[k] += (sustain[k] - c.env[k]) * d_k[k],
                    RELEASING => {
                        c.env[k] -= c.env[k] * r_k[k];
                        if c.env[k] < SILENT {
                            c.env[k] = 0.0;
                            c.stage[k] = IDLE;
                        }
                    }
                    _ => {}
                }
                amp[k] = c.level[k] * c.env[k] * c.velocity[k];
                any |= amp[k] > SILENT;
            }

            let dt1 = base_hz.at(i) * 2f32.powf(pitch.at(i) / 12.0) * master / sr;
            c.dt1 = dt1;
            if !any {
                c.quiet = c.quiet.saturating_add(1);
            } else {
                c.quiet = 0;
            }
            // 80 silent samples flush the filter: from then on the voice outputs zero.
            if c.quiet > 80 {
                c.line = [0.0; TAPS4];
                c.y1 = 0.0;
                c.y2 = 0.0;
                for (p, r) in c.phase.iter_mut().zip(&ratio) {
                    *p = (*p + dt1 * r).fract();
                }
                *o = 0.0;
                c.ring.push(0.0);
                continue;
            }

            let depth = c.index * MOD_RAD;
            let fb = c.feedback * FEEDBACK_RAD;
            let inc: [f32; OPS] = std::array::from_fn(|k| dt1 * ratio[k] / factor_f);
            if factor == 4 {
                c.line.copy_within(0..TAPS4 - 4, 4);
            } else {
                c.line.copy_within(0..TAPS - 2, 2);
            }
            for s in 0..factor {
                let mut op = [0.0f32; OPS];
                for k in (0..OPS).rev() {
                    if amp[k] > SILENT {
                        let mut m = 0.0;
                        let mut bits = algorithm.mods[k];
                        while bits != 0 {
                            m += op[bits.trailing_zeros() as usize];
                            bits &= bits - 1;
                        }
                        let fbt = if k == OPS - 1 {
                            fb * 0.5 * (c.y1 + c.y2)
                        } else {
                            0.0
                        };
                        op[k] = amp[k] * (c.phase[k] * TAU + m * depth + fbt).sin();
                    }
                    c.phase[k] += inc[k];
                    c.phase[k] -= c.phase[k].floor();
                }
                c.y2 = c.y1;
                c.y1 = op[OPS - 1];
                let mut sum = 0.0;
                let mut bits = algorithm.carriers;
                while bits != 0 {
                    sum += op[bits.trailing_zeros() as usize];
                    bits &= bits - 1;
                }
                c.line[factor - 1 - s] = sum * out_gain;
            }
            let y: f32 = if factor == 4 {
                self.taps4.iter().zip(&c.line).map(|(h, x)| h * x).sum()
            } else {
                self.taps.iter().zip(&c.line).map(|(h, x)| h * x).sum()
            };
            *o = if y.is_finite() { y } else { 0.0 };
            c.ring.push(*o);
        }
        if !c.line.iter().all(|v| v.is_finite()) || !c.phase.iter().all(|v| v.is_finite()) {
            c.line = [0.0; TAPS4];
            c.phase = [0.0; OPS];
            c.y1 = 0.0;
            c.y2 = 0.0;
        }
    }

    fn view(&self, out: &mut ModuleView) {
        out.valid = self.c.dt1 > 0.0 && self.c.quiet < u32::MAX;
        if out.valid {
            self.c.ring.cycle(1.0 / self.c.dt1, &mut out.cycle);
        }
    }

    fn reset(&mut self) {
        let sr = self.sample_rate;
        *self = OscFm6::new();
        self.sample_rate = sr;
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        for k in 0..OPS {
            out.write_f32(["p1", "p2", "p3", "p4", "p5", "p6"][k], self.c.phase[k]);
            out.write_f32(["e1", "e2", "e3", "e4", "e5", "e6"][k], self.c.env[k]);
        }
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        for k in 0..OPS {
            if let Some(v) = s.read_f32(["p1", "p2", "p3", "p4", "p5", "p6"][k]) {
                self.c.phase[k] = v.rem_euclid(1.0);
            }
            if let Some(v) = s.read_f32(["e1", "e2", "e3", "e4", "e5", "e6"][k]) {
                self.c.env[k] = v.clamp(0.0, 1.0);
                if v > SILENT {
                    self.c.stage[k] = DECAYING;
                }
            }
        }
    }

    fn carry_from(&mut self, old: &dyn Module) {
        if let Some(o) = old.as_any().downcast_ref::<OscFm6>() {
            self.c = o.c;
            self.factor = o.factor;
        }
    }
}
