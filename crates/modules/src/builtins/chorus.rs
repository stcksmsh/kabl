//! `chorus`: a stereo chorus/ensemble in the manner of the classic two-channel BBD choruses
//! (one modulated delay per side, the sides swept in opposite phase). Each input side runs
//! through its own short delay line, read by one tap whose delay a slow sine sweeps, with a
//! small, faster vibrato on top (6.7 × the rate, a sixth of the depth) for the ensemble
//! shimmer. The right side's sweep runs `width` × 180° away from the left's: 0 % gives the same
//! chorus on both sides, 100 % the widest. Delays sweep between 3.5 and 10.5 ms at full depth.
//!
//! One tap per side, not several summed: three taps swept 120° apart (tried first) average to
//! 1/3 + 2/3·J0(ω·A·√3) in power, which notched the wet signal by up to 10 dB around
//! 100–200 Hz. A single swept tap is flat (a moving delay, unity at every frequency); the
//! chorus comb comes from the dry/wet blend, as intended.
//!
//! **Dry image.** The dry signal passes each side untouched, and each side's wet signal comes
//! only from that side's input, so a panned source stays where it is. A mono source patched
//! to both inputs comes out wide.
//!
//! **Levels.** `mix` is an equal-power crossfade (as the delay's), exact at the endpoints: 0 %
//! is the dry input sample for sample, 100 % the wet signal only. The wet signal decorrelates
//! from the dry above a few tens of Hz, so broadband material keeps its level at any mix; only
//! the lowest bass, where the two stay in phase, gains up to 3 dB at 50 %. No feedback, so nothing can build up, and no DC is
//! added.
//!
//! **Continuity.** Depth, mix and width glide (50/20/20 ms); the LFO phase is continuous
//! whatever the rate does. The delay lines and phases ride across live edits in `carry_from`,
//! like the delay's, so an edit neither cuts the sound nor restarts the sweep.

use super::delay::{line, read};
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

const PARAMS: &[ParamInfo] = &[
    param("rate_hz", 0.05, 8.0, 0.5, "Hz", Taper::Exponential),
    param("depth", 0.0, 100.0, 50.0, "%", Taper::Linear),
    param("mix", 0.0, 100.0, 50.0, "%", Taper::Linear),
    param("width", 0.0, 100.0, 100.0, "%", Taper::Linear),
];

pub static CHORUS_INFO: ModuleInfo = ModuleInfo {
    kind: "chorus",
    name: "Chorus",
    category: Category::Effect,
    rate: Rate::Global,
    explain: "A stereo ensemble: slowly moving copies thicken the sound and spread it wide. Width 0 % keeps it centred; mix 0 % is dry.",
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
    advanced: &["width"],
};

/// Centre delay and full-depth excursion, in seconds.
const BASE_S: f32 = 0.007;
const DEPTH_S: f32 = 0.0035;
/// The fast vibrato: its rate and depth relative to the main sweep.
const FAST_RATIO: f32 = 6.7;
const FAST_DEPTH: f32 = 1.0 / 6.0;
/// Line length: longest delay plus interpolation room.
const LINE_S: f32 = 0.02;
const DEPTH_TAU_S: f32 = 0.05;
const CTRL_TAU_S: f32 = 0.02;

#[derive(Debug, Clone, Copy)]
struct Core {
    sample_rate: f32,
    write: usize,
    /// Slow and fast LFO phases, 0..1.
    phase: f32,
    fast: f32,
    /// Smoothed depth (0..1), mix, width; negative before the first block.
    depth: f32,
    mix: f32,
    width: f32,
}

pub struct Chorus {
    lines: [Vec<f32>; 2],
    /// The right output, built while the left one is borrowed.
    scratch: Vec<f32>,
    c: Core,
}

impl Chorus {
    pub fn new() -> Self {
        Chorus {
            lines: [Vec::new(), Vec::new()],
            scratch: Vec::new(),
            c: Core {
                sample_rate: 48000.0,
                write: 0,
                phase: 0.0,
                fast: 0.0,
                depth: -1.0,
                mix: 0.0,
                width: 0.0,
            },
        }
    }
}

impl Default for Chorus {
    fn default() -> Self {
        Self::new()
    }
}

/// Sine of a 0..1 phase.
#[inline]
fn sin1(p: f32) -> f32 {
    (std::f32::consts::TAU * p).sin()
}

impl Module for Chorus {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &CHORUS_INFO
    }

    fn prepare(&mut self, sample_rate: f32, max_block: usize, _quality: &QualityConfig) {
        self.c.sample_rate = sample_rate;
        let len = (LINE_S * sample_rate).ceil() as usize + 4;
        self.lines = [line(len), line(len)];
        self.scratch = vec![0.0; max_block];
        self.c.write = 0;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let len = self.lines[0].len();
        if len == 0 {
            return;
        }
        let (in_l, in_r) = (io.input(0), io.input(1));
        let c = &mut self.c;
        let sr = c.sample_rate;
        let rate = io.param(0).at(0).clamp(0.05, 8.0);
        let depth = (io.param(1).at(0) / 100.0).clamp(0.0, 1.0);
        let mix = (io.param(2).at(0) / 100.0).clamp(0.0, 1.0);
        let width = (io.param(3).at(0) / 100.0).clamp(0.0, 1.0);
        if c.depth < 0.0 {
            (c.depth, c.mix, c.width) = (depth, mix, width);
        }
        let k_depth = 1.0 - (-1.0 / (DEPTH_TAU_S * sr)).exp();
        let k_ctrl = 1.0 - (-1.0 / (CTRL_TAU_S * sr)).exp();
        let (dp, df) = (rate / sr, rate * FAST_RATIO / sr);
        let (base, excursion) = (BASE_S * sr, DEPTH_S * sr);

        let [left, right] = &mut self.lines;
        let out_l = &mut io.output(0)[..n];
        for (i, (ol, or)) in out_l.iter_mut().zip(&mut self.scratch[..n]).enumerate() {
            c.depth += (depth - c.depth) * k_depth;
            c.mix = settle(c.mix + (mix - c.mix) * k_ctrl, mix);
            c.width += (width - c.width) * k_ctrl;
            let (xl, xr) = (in_l.at(i), in_r.at(i));
            left[c.write] = xl;
            right[c.write] = xr;
            let d = |off: f32| {
                let m = sin1(c.phase + off) + FAST_DEPTH * sin1(c.fast + off);
                (base + excursion * c.depth * m / (1.0 + FAST_DEPTH)) as f64
            };
            let wl = read(left, c.write, d(0.0));
            let wr = read(right, c.write, d(0.5 * c.width));
            c.write = (c.write + 1) % len;
            c.phase = (c.phase + dp).fract();
            c.fast = (c.fast + df).fract();
            let (dry_g, wet_g) = match c.mix {
                m if m <= 0.0 => (1.0, 0.0),
                m if m >= 1.0 => (0.0, 1.0),
                m => {
                    let a = m * std::f32::consts::FRAC_PI_2;
                    (a.cos(), a.sin())
                }
            };
            (*ol, *or) = (dry_g * xl + wet_g * wl, dry_g * xr + wet_g * wr);
        }
        io.output(1)[..n].copy_from_slice(&self.scratch[..n]);
    }

    fn reset(&mut self) {
        for l in &mut self.lines {
            l.fill(0.0);
        }
        let sr = self.c.sample_rate;
        self.c = Chorus::new().c;
        self.c.sample_rate = sr;
    }

    fn carry_from(&mut self, old: &dyn Module) {
        let Some(o) = old.as_any().downcast_ref::<Chorus>() else {
            return;
        };
        if o.lines[0].len() != self.lines[0].len() || o.c.sample_rate != self.c.sample_rate {
            return;
        }
        self.lines[0].copy_from_slice(&o.lines[0]);
        self.lines[1].copy_from_slice(&o.lines[1]);
        self.c = o.c;
    }
}

/// Snaps a glide onto its target once it's within float noise, so the endpoints are exact.
#[inline]
fn settle(v: f32, target: f32) -> f32 {
    if (v - target).abs() < 1e-6 {
        target
    } else {
        v
    }
}
