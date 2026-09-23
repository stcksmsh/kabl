//! `delay`: a global echo. Mono in, stereo out. Two delay lines always run as a ping-pong pair
//! (input into the left line, each line feeding the other), so echoes land at 1, 2, 3.. × the
//! delay time in either mode. Mono only changes the output matrix: both lines summed at
//! 1/√2 to both sides, equal power with ping-pong. So a mode switch never loses the tail.
//! The tone low-pass sits in the feedback path, so the first echo is full-range and every
//! repeat after it is darker.
//!
//! Sync reads the `clock` cable, one pulse per 16th note. The interval between rising edges is
//! accepted only when it agrees (within 10 %) with the interval before it, so the first pulse
//! after a stop or a restart, whose interval includes the stopped time or a one-sample gap,
//! never becomes the tempo. Relocking takes three pulses after Run. Until the first accepted
//! interval (and whenever `sync` is FREE) the delay plays the free `time`. Without new pulses
//! the last accepted interval is kept.
//!
//! The audio history (up to 4 s per line) is too big for `StateBuf`; `carry_from` copies it
//! across graph swaps. `prepare` (control thread) allocates it.

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "in",
        port_type: PortType::Audio,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "clock",
        port_type: PortType::Gate,
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

pub const MAX_MS: f32 = 4000.0;
const MIN_MS: f32 = 20.0;

const PARAMS: &[ParamInfo] = &[
    param("time_ms", MIN_MS, MAX_MS, 375.0, "ms", Taper::Exponential),
    // FREE, 1/16, 1/8, dotted 1/8, 1/4.
    param("sync", 0.0, 4.0, 2.0, "", Taper::Stepped),
    param("feedback", 0.0, 95.0, 40.0, "%", Taper::Linear),
    param("mix", 0.0, 100.0, 35.0, "%", Taper::Linear),
    param("tone_hz", 500.0, 16000.0, 5000.0, "Hz", Taper::Exponential),
    // MONO, PING.
    param("mode", 0.0, 1.0, 0.0, "", Taper::Stepped),
];

/// Clock pulses (16th notes) per sync choice, indexed by `sync`; 0 is FREE.
const SYNC_PULSES: [f32; 5] = [0.0, 1.0, 2.0, 3.0, 4.0];

pub static DELAY_INFO: ModuleInfo = ModuleInfo {
    kind: "delay",
    name: "Delay",
    category: Category::Effect,
    rate: Rate::Global,
    explain: "Echoes its input. Sync follows a clock cable; feedback repeats, tone darkens the repeats, ping-pong bounces them left and right.",
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
    advanced: &["tone_hz", "mode"],
};

/// What the delay time follows right now, for the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelayLock {
    /// `sync` is FREE.
    Free,
    /// Sync is on but no clock interval has been accepted yet: plays the free time.
    Unlocked,
    /// Following the clock.
    Locked,
    /// Locked before, but no pulse for more than two intervals (clock stopped): the last time
    /// is kept.
    Held,
}

/// Smoothing time constants.
const TIME_TAU_S: f32 = 0.1;
const CTRL_TAU_S: f32 = 0.02;
/// Most the read position may speed up or slow down while the time glides, in samples per
/// sample: ±7 semitones of pitch at most, never reversed.
const MAX_GLIDE: f64 = 0.5;
/// Interval agreement needed to accept a clock interval.
const AGREE: f32 = 0.1;
/// Values this small are written as 0, so a decaying tail never goes subnormal (slow on x86).
const TINY: f32 = 1e-20;

/// Everything but the audio history. `Copy`, so a swap carries it whole.
#[derive(Debug, Clone, Copy)]
struct Core {
    sample_rate: f32,
    write: usize,
    /// Smoothed delay, in samples; negative until the first block sets it. f64: in f32 the
    /// glide's last steps round away and it settles several samples short.
    delay: f64,
    feedback: f32,
    mix: f32,
    /// 0 = mono matrix, 1 = ping-pong.
    width: f32,
    tone_l: f32,
    tone_r: f32,
    clock_high: bool,
    seen_edge: bool,
    /// Samples since the last rising clock edge.
    since: u32,
    /// The last measured interval, accepted or not; 0 = none.
    last_interval: f32,
    /// The accepted pulse interval in samples; 0 = unlocked.
    period: f32,
    /// Target delay and lock state of the last processed sample (UI readout).
    target_ms: f32,
    lock: DelayLock,
}

pub struct Delay {
    left: Vec<f32>,
    right: Vec<f32>,
    /// The right output, built while the left one is borrowed.
    scratch: Vec<f32>,
    c: Core,
}

impl Delay {
    pub fn new() -> Self {
        Delay {
            left: Vec::new(),
            right: Vec::new(),
            scratch: Vec::new(),
            c: Core {
                sample_rate: 48000.0,
                write: 0,
                delay: -1.0,
                feedback: 0.0,
                mix: 0.0,
                width: 0.0,
                tone_l: 0.0,
                tone_r: 0.0,
                clock_high: false,
                seen_edge: false,
                since: 0,
                last_interval: 0.0,
                period: 0.0,
                target_ms: PARAMS[0].default,
                lock: DelayLock::Unlocked,
            },
        }
    }

    /// The delay time being approached and what it follows. No allocation.
    pub fn status(&self) -> (DelayLock, f32) {
        (self.c.lock, self.c.target_ms)
    }

    /// Delay-line length in samples at `sample_rate`: the longest time plus interpolation room.
    pub fn line_len(sample_rate: f32) -> usize {
        (MAX_MS / 1000.0 * sample_rate).ceil() as usize + 4
    }
}

impl Default for Delay {
    fn default() -> Self {
        Self::new()
    }
}

/// Allocates a zeroed line and writes every page, so the audio thread never takes the first-
/// touch page faults of a fresh (lazily zeroed) allocation.
fn line(len: usize) -> Vec<f32> {
    let mut v = vec![0.0f32; len];
    for x in v.iter_mut().step_by(1024) {
        *x = std::hint::black_box(0.0);
    }
    v
}

/// 4-point Hermite read `d` samples behind `write` (d ≥ 3).
#[inline]
fn read(buf: &[f32], write: usize, d: f64) -> f32 {
    let len = buf.len();
    let di = d.floor();
    let t = (d - di) as f32;
    // Samples at delays di-1 (newest), di, di+1, di+2.
    let base = write + len - di as usize;
    let at = |k: usize| buf[(base + k) % len];
    let (xm1, x0, x1, x2) = (at(1), at(0), at(len - 1), at(len - 2));
    // Interpolating from x0 (delay di) toward x1 (delay di+1) by t.
    let c1 = 0.5 * (x1 - xm1);
    let c2 = xm1 - 2.5 * x0 + 2.0 * x1 - 0.5 * x2;
    let c3 = 0.5 * (x2 - xm1) + 1.5 * (x0 - x1);
    ((c3 * t + c2) * t + c1) * t + x0
}

#[inline]
fn flush(x: f32) -> f32 {
    if x.abs() < TINY {
        0.0
    } else {
        x
    }
}

impl Module for Delay {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &DELAY_INFO
    }

    fn prepare(&mut self, sample_rate: f32, max_block: usize, _quality: &QualityConfig) {
        self.c.sample_rate = sample_rate;
        self.scratch = vec![0.0; max_block];
        let len = Delay::line_len(sample_rate);
        self.left = line(len);
        self.right = line(len);
        self.c.write = 0;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let (input, clock) = (io.input(0), io.input(1));
        let c = &mut self.c;
        let sr = c.sample_rate;
        let free_ms = io.param(0).at(0).clamp(MIN_MS, MAX_MS);
        let pulses = SYNC_PULSES[(io.param(1).at(0).round() as usize).min(4)];
        let fb = (io.param(2).at(0) / 100.0).clamp(0.0, 0.95);
        let mix = (io.param(3).at(0) / 100.0).clamp(0.0, 1.0);
        let tone = io.param(4).at(0).clamp(20.0, 0.45 * sr);
        let width = (io.param(5).at(0) >= 0.5) as u8 as f32;
        let a_tone = 1.0 - (-std::f32::consts::TAU * tone / sr).exp();
        let k_time = 1.0 - (-1.0 / (TIME_TAU_S as f64 * sr as f64)).exp();
        let k_ctrl = 1.0 - (-1.0 / (CTRL_TAU_S * sr)).exp();
        let (min_d, max_d) = (MIN_MS / 1000.0 * sr, MAX_MS / 1000.0 * sr);
        let len = self.left.len();
        if len == 0 {
            return;
        }

        let (left, right) = (&mut self.left, &mut self.right);
        let out_l = &mut io.output(0)[..n];
        for (i, (ol, or)) in out_l.iter_mut().zip(&mut self.scratch[..n]).enumerate() {
            let high = clock.at(i) > 0.0;
            c.since = c.since.saturating_add(1);
            if high && !c.clock_high {
                if c.seen_edge {
                    let t = c.since as f32;
                    if c.last_interval > 0.0
                        && (t - c.last_interval).abs() <= AGREE * c.last_interval
                    {
                        c.period = t;
                    }
                    c.last_interval = t;
                }
                (c.seen_edge, c.since) = (true, 0);
            }
            c.clock_high = high;

            let (target, lock) = if pulses == 0.0 {
                (free_ms / 1000.0 * sr, DelayLock::Free)
            } else if c.period == 0.0 {
                (free_ms / 1000.0 * sr, DelayLock::Unlocked)
            } else if c.since as f32 > 2.0 * c.period {
                (c.period * pulses, DelayLock::Held)
            } else {
                (c.period * pulses, DelayLock::Locked)
            };
            let target = target.clamp(min_d, max_d);
            let target = target as f64;
            if c.delay < 0.0 {
                (c.delay, c.feedback, c.mix, c.width) = (target, fb, mix, width);
            }
            let step = (target - c.delay) * k_time;
            c.delay = if step.abs() < 1e-9 {
                target
            } else {
                c.delay + step.clamp(-MAX_GLIDE, MAX_GLIDE)
            };
            c.feedback += (fb - c.feedback) * k_ctrl;
            c.mix += (mix - c.mix) * k_ctrl;
            c.width += (width - c.width) * k_ctrl;
            (c.target_ms, c.lock) = (target as f32 / sr * 1000.0, lock);

            let yl = read(left, c.write, c.delay);
            let yr = read(right, c.write, c.delay);
            c.tone_l = flush(c.tone_l + a_tone * (yl - c.tone_l));
            c.tone_r = flush(c.tone_r + a_tone * (yr - c.tone_r));
            let x = input.at(i);
            left[c.write] = flush(x + c.feedback * c.tone_r);
            right[c.write] = flush(c.feedback * c.tone_l);
            c.write = (c.write + 1) % len;

            let centre = std::f32::consts::FRAC_1_SQRT_2 * (yl + yr);
            let wet_l = c.width * yl + (1.0 - c.width) * centre;
            let wet_r = c.width * yr + (1.0 - c.width) * centre;
            // Equal-power dry/wet, with exact endpoints.
            let (wet_g, dry_g) = match c.mix {
                m if m <= 0.0 => (0.0, 1.0),
                m if m >= 1.0 => (1.0, 0.0),
                m => {
                    let a = m * std::f32::consts::FRAC_PI_2;
                    (a.sin(), a.cos())
                }
            };
            *ol = dry_g * x + wet_g * wet_l;
            *or = dry_g * x + wet_g * wet_r;
        }
        io.output(1)[..n].copy_from_slice(&self.scratch[..n]);
    }

    fn reset(&mut self) {
        self.left.fill(0.0);
        self.right.fill(0.0);
        let sr = self.c.sample_rate;
        self.c = Delay::new().c;
        self.c.sample_rate = sr;
    }

    fn carry_from(&mut self, old: &dyn Module) {
        let Some(o) = old.as_any().downcast_ref::<Delay>() else {
            return;
        };
        if o.left.len() != self.left.len() || o.c.sample_rate != self.c.sample_rate {
            return;
        }
        self.left.copy_from_slice(&o.left);
        self.right.copy_from_slice(&o.right);
        self.c = o.c;
    }
}
