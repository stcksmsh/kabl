//! `reverb`: a global stereo plate reverb. The algorithm is Jon Dattorro's plate ("Effect Design,
//! Part 1: Reverberator and Other Filters", J. Audio Eng. Soc. 45(9), 1997, figure 1 and
//! table 2), written from the paper; no code was copied. Its delay lengths are the paper's, at
//! its 29761 Hz, scaled to the running rate.
//!
//! One change from the paper: it sums its input to mono before one input diffuser chain. Here
//! each side has its own chain and feeds the tank half that the same side's output taps first,
//! so a panned source stays on its side in the early reverb. The two halves still cross-feed,
//! so the tail spreads to both sides.
//!
//! The history (every line, ~0.9 s of audio in all at any rate) is carried across graph swaps
//! by `carry_from`; `prepare` (control thread) allocates it.

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "in_l",
        port_type: PortType::Audio,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "in_r",
        port_type: PortType::Audio,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "left",
        port_type: PortType::Audio,
        direction: PortDirection::Output,
    },
    PortInfo {
        name: "right",
        port_type: PortType::Audio,
        direction: PortDirection::Output,
    },
];

const fn param(
    name: &'static str,
    min: f32,
    max: f32,
    default: f32,
    unit: &'static str,
    taper: Taper,
) -> ParamInfo {
    ParamInfo {
        name,
        min,
        max,
        default,
        unit,
        taper,
        smoothing_ms: 0.0,
    }
}

pub const MAX_PREDELAY_MS: f32 = 250.0;

const PARAMS: &[ParamInfo] = &[
    param("decay_s", 0.3, 30.0, 3.5, "s", Taper::Exponential),
    param("damp_hz", 500.0, 16000.0, 6000.0, "Hz", Taper::Exponential),
    param("mix", 0.0, 100.0, 30.0, "%", Taper::Linear),
    param(
        "predelay_ms",
        0.0,
        MAX_PREDELAY_MS,
        20.0,
        "ms",
        Taper::Linear,
    ),
    param("width", 0.0, 100.0, 100.0, "%", Taper::Linear),
];

pub static REVERB_INFO: ModuleInfo = ModuleInfo {
    kind: "reverb",
    name: "Reverb",
    category: Category::Effect,
    rate: Rate::Global,
    explain: "A stereo plate reverb. Decay sets how long the tail lasts, damping how dark it gets; mix 100 % is fully wet, for a send and return.",
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
    width_units: 8,
    advanced: &["predelay_ms", "width"],
};

/// The paper's rate.
const PAPER_SR: f32 = 29761.0;
/// Input diffusers per side: (length, coefficient).
const INPUT_AP: [(f32, f32); 4] = [(142.0, 0.75), (107.0, 0.75), (379.0, 0.625), (277.0, 0.625)];
/// Tank halves: modulated allpass, delay, allpass, delay. The first allpass has the paper's
/// reversed sign (decay diffusion 1 = 0.70).
const TANK: [[f32; 4]; 2] = [
    [672.0, 4453.0, 1800.0, 3720.0],
    [908.0, 4217.0, 2656.0, 3163.0],
];
const DECAY_DIFFUSION_1: f32 = 0.70;
/// Peak modulation excursion of the tank's first allpass, and its rate.
const EXCURSION: f32 = 16.0;
const MOD_HZ: f32 = 1.0;
/// Input band limit (the paper's `bandwidth`).
const BANDWIDTH: f32 = 0.9995;
/// Seconds one pass through a tank half takes (both halves' mean, at any rate), for turning a
/// decay time into the per-pass gain. The allpasses spread energy as well, so the decay time is
/// approximate (`tests/reverb.rs` checks it within a factor).
const HALF_LOOP_S: f32 =
    (672.0 + 4453.0 + 1800.0 + 3720.0 + 908.0 + 4217.0 + 2656.0 + 3163.0) / 2.0 / PAPER_SR;
/// Output taps (table 2): (half, element, paper delay, sign). Element 1 and 3 are the delays,
/// 2 the second allpass's line.
const TAPS: [[(usize, usize, f32, f32); 7]; 2] = [
    [
        (1, 1, 266.0, 1.0),
        (1, 1, 2974.0, 1.0),
        (1, 2, 1913.0, -1.0),
        (1, 3, 1996.0, 1.0),
        (0, 1, 1990.0, -1.0),
        (0, 2, 187.0, -1.0),
        (0, 3, 1066.0, -1.0),
    ],
    [
        (0, 1, 353.0, 1.0),
        (0, 1, 3627.0, 1.0),
        (0, 2, 1228.0, -1.0),
        (0, 3, 2673.0, 1.0),
        (1, 1, 2111.0, -1.0),
        (1, 2, 335.0, -1.0),
        (1, 3, 121.0, -1.0),
    ],
];
const OUT_GAIN: f32 = 0.6;
const CTRL_TAU_S: f32 = 0.02;
const PREDELAY_TAU_S: f32 = 0.1;
const TINY: f32 = 1e-20;

#[inline]
fn flush(x: f32) -> f32 {
    if x.abs() < TINY {
        0.0
    } else {
        x
    }
}

/// A circular line. `read(k)` is the sample written `k` writes ago (k ≥ 1).
struct Line {
    buf: Vec<f32>,
    pos: usize,
}

impl Line {
    fn new(len: usize) -> Self {
        // Every page written, so the audio thread takes no first-touch page faults.
        let mut buf = vec![0.0f32; len.max(1)];
        for x in buf.iter_mut().step_by(1024) {
            *x = std::hint::black_box(0.0);
        }
        Line { buf, pos: 0 }
    }
    #[inline]
    fn read(&self, k: usize) -> f32 {
        let len = self.buf.len();
        self.buf[(self.pos + len - k) % len]
    }
    /// Linear interpolation, `d` ≥ 1.
    #[inline]
    fn read_frac(&self, d: f32) -> f32 {
        let k = d.floor();
        let t = d - k;
        let k = k as usize;
        self.read(k) * (1.0 - t) + self.read(k + 1) * t
    }
    #[inline]
    fn write(&mut self, x: f32) {
        self.buf[self.pos] = x;
        self.pos = (self.pos + 1) % self.buf.len();
    }
    fn carry(&mut self, o: &Line) {
        if o.buf.len() == self.buf.len() {
            self.buf.copy_from_slice(&o.buf);
            self.pos = o.pos;
        }
    }
}

/// Allpass `(g + z^-d) / (1 + g z^-d)` on `line` (`w = x - g·w[n-d]`, `y = g·w + w[n-d]`).
#[inline]
fn allpass(line: &mut Line, d: f32, g: f32, x: f32) -> f32 {
    let delayed = if d.fract() == 0.0 {
        line.read(d as usize)
    } else {
        line.read_frac(d)
    };
    let w = flush(x - g * delayed);
    line.write(w);
    g * w + delayed
}

#[derive(Clone, Copy)]
struct Core {
    sample_rate: f32,
    scale: f32,
    /// Smoothed controls; `decay` is the per-pass gain, `damp` the damping low-pass coefficient.
    decay: f32,
    damp: f32,
    mix: f32,
    width: f32,
    /// Smoothed pre-delay in samples; negative until the first block sets every control.
    predelay: f32,
    phase: f32,
    band: [f32; 2],
    lp: [f32; 2],
    /// Each half's last output, fed to the other half on the next sample.
    fb: [f32; 2],
}

pub struct Reverb {
    pre: [Line; 2],
    diff: [[Line; 4]; 2],
    /// Per half: modulated allpass, delay, allpass, delay.
    tank: [[Line; 4]; 2],
    scratch: Vec<f32>,
    c: Core,
}

fn empty() -> Line {
    Line {
        buf: Vec::new(),
        pos: 0,
    }
}

impl Reverb {
    pub fn new() -> Self {
        Reverb {
            pre: [empty(), empty()],
            diff: std::array::from_fn(|_| std::array::from_fn(|_| empty())),
            tank: std::array::from_fn(|_| std::array::from_fn(|_| empty())),
            scratch: Vec::new(),
            c: Core {
                sample_rate: 48000.0,
                scale: 48000.0 / PAPER_SR,
                decay: 0.0,
                damp: 0.0,
                mix: 0.0,
                width: 0.0,
                predelay: -1.0,
                phase: 0.0,
                band: [0.0; 2],
                lp: [0.0; 2],
                fb: [0.0; 2],
            },
        }
    }

    /// The gain applied twice per pass through a tank half, for an RT60 of `seconds`.
    pub fn decay_gain(seconds: f32) -> f32 {
        0.001f32.powf(HALF_LOOP_S / (2.0 * seconds.max(0.01)))
    }
}

impl Default for Reverb {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Reverb {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &REVERB_INFO
    }

    fn prepare(&mut self, sample_rate: f32, max_block: usize, _quality: &QualityConfig) {
        let s = sample_rate / PAPER_SR;
        let n = |paper: f32| (paper * s).ceil() as usize + 2;
        self.c.sample_rate = sample_rate;
        self.c.scale = s;
        let pre = (MAX_PREDELAY_MS / 1000.0 * sample_rate).ceil() as usize + 4;
        self.pre = [Line::new(pre), Line::new(pre)];
        self.diff = std::array::from_fn(|_| std::array::from_fn(|k| Line::new(n(INPUT_AP[k].0))));
        self.tank = std::array::from_fn(|h| {
            std::array::from_fn(|k| {
                let extra = if k == 0 { EXCURSION + 2.0 } else { 0.0 };
                Line::new(n(TANK[h][k] + extra))
            })
        });
        self.scratch = vec![0.0; max_block];
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        if self.tank[0][0].buf.len() <= 1 {
            return;
        }
        let n = io.block_len();
        let (in_l, in_r) = (io.input(0), io.input(1));
        let c = &mut self.c;
        let sr = c.sample_rate;
        let s = c.scale;
        let decay = Reverb::decay_gain(io.param(0).at(0).clamp(0.3, 30.0));
        let damp_hz = io.param(1).at(0).clamp(20.0, 0.45 * sr);
        let damp = (-std::f32::consts::TAU * damp_hz / sr).exp();
        let mix = (io.param(2).at(0) / 100.0).clamp(0.0, 1.0);
        // +1: `read_frac(1)` is the sample just written.
        let pre = io.param(3).at(0).clamp(0.0, MAX_PREDELAY_MS) / 1000.0 * sr + 1.0;
        let width = (io.param(4).at(0) / 100.0).clamp(0.0, 1.0);
        let k_ctrl = 1.0 - (-1.0 / (CTRL_TAU_S * sr)).exp();
        let k_pre = 1.0 - (-1.0 / (PREDELAY_TAU_S * sr)).exp();
        if c.predelay < 0.0 {
            (c.decay, c.damp, c.mix, c.width, c.predelay) = (decay, damp, mix, width, pre);
        }
        let d = |paper: f32| (paper * s).round();
        let exc = EXCURSION * s;
        let dphase = MOD_HZ / sr;
        let taps = TAPS.map(|side| side.map(|(h, e, k, sign)| (h, e, d(k) as usize, sign)));
        let ap_len = INPUT_AP.map(|(len, _)| d(len));
        let tank_len = TANK.map(|half| half.map(d));

        let out_l = &mut io.output(0)[..n];
        for (i, (ol, or)) in out_l.iter_mut().zip(&mut self.scratch[..n]).enumerate() {
            c.decay += (decay - c.decay) * k_ctrl;
            c.damp += (damp - c.damp) * k_ctrl;
            c.mix += (mix - c.mix) * k_ctrl;
            c.width += (width - c.width) * k_ctrl;
            // Glides at most ±0.5 samples per sample (a pitch bend, never reversed).
            c.predelay += ((pre - c.predelay) * k_pre).clamp(-0.5, 0.5);
            let diffusion_2 = (c.decay + 0.15).clamp(0.25, 0.5);
            c.phase = (c.phase + dphase).fract();
            let m = (std::f32::consts::TAU * c.phase).sin() * exc;

            let x = [in_l.at(i), in_r.at(i)];
            let mut inject = [0.0f32; 2];
            for side in 0..2 {
                let p = &mut self.pre[side];
                p.write(x[side]);
                let delayed = p.read_frac(c.predelay);
                c.band[side] = flush(c.band[side] + BANDWIDTH * (delayed - c.band[side]));
                let mut v = c.band[side];
                for (k, line) in self.diff[side].iter_mut().enumerate() {
                    v = allpass(line, ap_len[k], INPUT_AP[k].1, v);
                }
                inject[side] = v;
            }
            let fb = c.fb;
            for h in 0..2 {
                let t = &mut self.tank[h];
                let lens = tank_len[h];
                // Quadrature modulation between the halves.
                let md = if h == 0 { m } else { -m };
                // Each input feeds the half whose nodes the same side's output taps first.
                let v = inject[1 - h] + fb[1 - h];
                let v = allpass(&mut t[0], lens[0] + md, -DECAY_DIFFUSION_1, v);
                t[1].write(v);
                let v = t[1].read(lens[1] as usize);
                c.lp[h] = flush(v + c.damp * (c.lp[h] - v));
                let v = allpass(&mut t[2], lens[2], diffusion_2, c.lp[h] * c.decay);
                t[3].write(v);
                c.fb[h] = t[3].read(lens[3] as usize) * c.decay;
            }
            let wet = taps.map(|side| {
                side.iter()
                    .map(|&(h, e, k, sign)| sign * self.tank[h][e].read(k.max(1)))
                    .sum::<f32>()
                    * OUT_GAIN
            });
            let mid = 0.5 * (wet[0] + wet[1]);
            let side = 0.5 * (wet[0] - wet[1]) * c.width;
            // Equal-power dry/wet, with exact endpoints.
            let (wet_g, dry_g) = match c.mix {
                m if m <= 0.0 => (0.0, 1.0),
                m if m >= 1.0 => (1.0, 0.0),
                m => {
                    let a = m * std::f32::consts::FRAC_PI_2;
                    (a.sin(), a.cos())
                }
            };
            *ol = dry_g * x[0] + wet_g * (mid + side);
            *or = dry_g * x[1] + wet_g * (mid - side);
        }
        io.output(1)[..n].copy_from_slice(&self.scratch[..n]);
    }

    fn reset(&mut self) {
        for l in self
            .pre
            .iter_mut()
            .chain(self.diff.iter_mut().flatten())
            .chain(self.tank.iter_mut().flatten())
        {
            l.buf.fill(0.0);
            l.pos = 0;
        }
        let (sr, s) = (self.c.sample_rate, self.c.scale);
        self.c = Reverb::new().c;
        (self.c.sample_rate, self.c.scale) = (sr, s);
    }

    fn carry_from(&mut self, old: &dyn Module) {
        let Some(o) = old.as_any().downcast_ref::<Reverb>() else {
            return;
        };
        if o.c.sample_rate != self.c.sample_rate {
            return;
        }
        for (a, b) in self.pre.iter_mut().zip(&o.pre) {
            a.carry(b);
        }
        for (a, b) in self.diff.iter_mut().flatten().zip(o.diff.iter().flatten()) {
            a.carry(b);
        }
        for (a, b) in self.tank.iter_mut().flatten().zip(o.tank.iter().flatten()) {
            a.carry(b);
        }
        self.c = o.c;
    }
}
