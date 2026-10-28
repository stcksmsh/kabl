//! `osc.va` (brief section 8's v1 built-in table: "saw/square/tri/sine, PolyBLEP, hard sync
//! input"). Saw and square are PolyBLEP-corrected; triangle and sine are not (triangle needs
//! polyBLAMP for its corners, not implemented; its harmonics fall at 12 dB/octave, so it aliases
//! far less than an uncorrected saw would).
//!
//! Sound palette batch (docs/sound-palette-batch/README.md): pulse width (`pw`, 5–95 %, the
//! square only, ramped per sample across each block so a modulated width never steps, DC
//! removed so PWM doesn't thump), fine tuning (`fine`, ±100 cents), unison (`unison`, 1–16
//! oscillators per voice, `detune` = spread from lowest to highest in cents, each oscillator
//! fading in or out over 20 ms when the count changes), and band-limited hard sync (see
//! `process`). Defaults (50 %, 0 ct, 1 oscillator) are the old oscillator sample for sample.
//!
//! Unison shaping (all neutral at their defaults, so a stack of up to 4 plays as it always
//! did): `curve` bends the detune distribution from even steps (0) toward outer oscillators
//! detuned much more than inner ones (100, the JP-8000 supersaw's shape); `blend` weighs the
//! centre oscillator(s) against the side ones (50 = equal); `width` pans the stack across the
//! field. Level law: equal power, each oscillator at `weight / √(Σ weight²)`, so loudness
//! stays put as the count or the blend changes. `out` is the whole stack when `width` is 0 and
//! the left channel otherwise; `out_r` is the right channel (a copy of `out` at width 0).
//! `restart` chooses what a `sync` edge does to a stack: 0 restarts every oscillator at phase 0
//! (hard sync), 1 restarts each at its own start phase, so a retriggered stack never aligns.
//!
//! Waveform selection is a discrete choice represented as a stepped float param (`0..=3`,
//! rounded in `process`) — same convention `lfo.rs` uses, and the same reasoning: `ParamInfo`
//! only models float ranges, no enum-param variant exists yet. Parameter index order mirrors
//! `lfo.rs`'s (`Sine, Triangle, Saw, Square[, S&H for lfo only]`) for consistency across the two
//! oscillator-shaped modules; the default is kept at `Saw` (index 2) specifically to match this
//! module's pre-existing saw-only behavior, not because saw is otherwise privileged.
//!
//! `sync` is an audio-rate input, edge-detected per sample (rising through the same >0.5 "gate"
//! threshold `env.adsr`/`midi.in` use) — a discrete hard-sync event needs per-sample precision,
//! not a once-per-block read like the other params.

use std::f32::consts::PI;

use crate::dsp::OscWaveform;
use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "pitch",
        port_type: PortType::Pitch,
        direction: PortDirection::Input,
        width: 1,
    },
    PortInfo {
        name: "sync",
        port_type: PortType::Gate,
        direction: PortDirection::Input,
        width: 1,
    },
    PortInfo {
        name: "out",
        port_type: PortType::Audio,
        direction: PortDirection::Output,
        width: 1,
    },
    PortInfo {
        name: "out_r",
        port_type: PortType::Audio,
        direction: PortDirection::Output,
        width: 1,
    },
];

const PARAMS: &[ParamInfo] = &[
    ParamInfo {
        name: "base_hz",
        min: 20.0,
        max: 20000.0,
        default: 261.63, // C4 — the frequency a "pitch" input of 0 semitones resolves to.
        unit: "Hz",
        taper: Taper::Exponential,
        smoothing_ms: 5.0,
    },
    ParamInfo {
        name: "waveform",
        min: 0.0,
        max: 3.0,
        default: 2.0, // Saw — matches this module's behavior before waveform selection existed.
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    },
    // Pulse width of the square (SQR) wave. 50 % is the old square, sample for sample.
    ParamInfo {
        name: "pw",
        min: 5.0,
        max: 95.0,
        default: 50.0,
        unit: "%",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "fine",
        min: -100.0,
        max: 100.0,
        default: 0.0,
        unit: "ct",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
    // Oscillators per voice, 1 (the old oscillator) to 16.
    ParamInfo {
        name: "unison",
        min: 1.0,
        max: 16.0,
        default: 1.0,
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    },
    // Spread of the unison oscillators, lowest to highest, in cents.
    ParamInfo {
        name: "detune",
        min: 0.0,
        max: 100.0,
        default: 15.0,
        unit: "ct",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
    // Detune distribution: 0 even steps, 100 the outer oscillators far wider than the inner.
    ParamInfo {
        name: "curve",
        min: 0.0,
        max: 100.0,
        default: 0.0,
        unit: "%",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
    // Stereo width of the stack: 0 mono, 100 lowest hard left and highest hard right.
    ParamInfo {
        name: "width",
        min: 0.0,
        max: 100.0,
        default: 0.0,
        unit: "%",
        taper: Taper::Linear,
        smoothing_ms: 10.0,
    },
    // Centre against side oscillators: 0 centre only, 50 equal, 100 sides only.
    ParamInfo {
        name: "blend",
        min: 0.0,
        max: 100.0,
        default: 50.0,
        unit: "%",
        taper: Taper::Linear,
        smoothing_ms: 10.0,
    },
    // What a sync edge does to every oscillator: 0 restart at phase 0, 1 at its start phase.
    ParamInfo {
        name: "restart",
        min: 0.0,
        max: 1.0,
        default: 0.0,
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    },
];

pub static OSC_VA_INFO: ModuleInfo = ModuleInfo {
    kind: "osc.va",
    name: "VA Oscillator",
    category: Category::Source,
    rate: Rate::Voice,
    explain: "A virtual-analog oscillator: turns a pitch into sine, triangle, saw, or square.",
    lesson: None,
    requires: &[],
    ports: PORTS,
    params: PARAMS,
    quality: QualitySupport {
        oversampling: false,
        anti_aliasing: true, // PolyBLEP -- saw/square only, see dsp::FullOsc's doc.
        interpolation: false,
    },
    skin: None,
    width_units: 7,
    advanced: &[
        "pw", "fine", "unison", "detune", "curve", "width", "blend", "restart",
    ],
};

const PITCH_IN: usize = 0;
const SYNC_IN: usize = 1;
const OUT: usize = 0;
const BASE_HZ_PARAM: usize = 0;
const WAVEFORM_PARAM: usize = 1;
const PW_PARAM: usize = 2;
const FINE_PARAM: usize = 3;
const UNISON_PARAM: usize = 4;
const DETUNE_PARAM: usize = 5;
const CURVE_PARAM: usize = 6;
const WIDTH_PARAM: usize = 7;
const BLEND_PARAM: usize = 8;
const RESTART_PARAM: usize = 9;
const OUT_R: usize = 1;

pub const MAX_UNISON: usize = 16;
/// The stacks the first unison laid out by table; larger ones use even steps.
const TABLED: usize = 4;

/// Where each unison oscillator sits within the detune spread (−0.5 = lowest, +0.5 =
/// highest), by count. Oscillator 0 alone plays at the centre, so one oscillator is the old
/// oscillator.
const SPREAD: [[f32; TABLED]; TABLED] = [
    [0.0, 0.0, 0.0, 0.0],
    [-0.5, 0.5, 0.0, 0.0],
    [0.0, -0.5, 0.5, 0.0],
    [-1.0 / 6.0, -0.5, 0.5, 1.0 / 6.0],
];
/// Starting phases of the unison oscillators: never aligned, so a new unison voice doesn't
/// double the first cycle. Oscillator 0 starts at 0, as before; the first four are the
/// original values, the rest step by the golden ratio (evenly spread whatever the count).
const START_PHASE: [f32; MAX_UNISON] = [
    0.0, 0.31, 0.62, 0.87, 0.472, 0.090, 0.708, 0.326, 0.944, 0.562, 0.180, 0.798, 0.416, 0.034,
    0.652, 0.270,
];

/// Where oscillator `k` of `count` sits in the spread, −0.5 (lowest) to 0.5 (highest).
fn position(count: usize, k: usize) -> f32 {
    if count <= TABLED {
        SPREAD[count - 1][k]
    } else {
        k as f32 / (count - 1) as f32 - 0.5
    }
}
/// Unison oscillators fade in and out over this time constant when the count changes.
const GAIN_TAU_S: f32 = 0.02;

fn waveform_from_param(v: f32) -> OscWaveform {
    match v.round() as i32 {
        1 => OscWaveform::Triangle,
        2 => OscWaveform::Saw,
        3 => OscWaveform::Square,
        _ => OscWaveform::Sine,
    }
}

/// PolyBLEP residual (Välimäki et al.): `t` = phase since the last edge (or before the next).
#[inline]
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        t + t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

/// The waveform at phase `p` without band-limiting (the square without its DC correction).
#[inline]
fn naive(p: f32, w: OscWaveform, pw: f32) -> f32 {
    match w {
        OscWaveform::Saw => 2.0 * p - 1.0,
        OscWaveform::Square => {
            if p < pw {
                1.0
            } else {
                -1.0
            }
        }
        OscWaveform::Triangle => {
            if p < 0.25 {
                4.0 * p
            } else if p < 0.75 {
                2.0 - 4.0 * p
            } else {
                4.0 * p - 4.0
            }
        }
        OscWaveform::Sine => (2.0 * PI * p).sin(),
    }
}

/// One band-limited sample at phase `p`: the same arithmetic as `dsp::FullOsc::next`, with
/// the square's second edge at `pw` and its DC removed (0 at 50 %, so the old square is exact).
#[inline]
fn sample(p: f32, dt: f32, w: OscWaveform, pw: f32) -> f32 {
    match w {
        OscWaveform::Saw => {
            let mut v = 2.0 * p - 1.0;
            v -= poly_blep(p, dt);
            v
        }
        OscWaveform::Square => {
            let mut v = if p < pw { 1.0 } else { -1.0 };
            v += poly_blep(p, dt);
            v -= poly_blep((p + (1.0 - pw)).fract(), dt);
            v - (2.0 * pw - 1.0)
        }
        _ => naive(p, w, pw),
    }
}

/// Everything an oscillator carries across a live edit. `Copy`, so `carry_from` takes it whole.
#[derive(Debug, Clone, Copy)]
struct Core {
    phase: [f32; MAX_UNISON],
    /// Smoothed level of each unison oscillator; negative before the first block.
    gain: [f32; MAX_UNISON],
    /// Pulse width (0..1) at the end of the last block: the next block ramps from it.
    pw: f32,
    /// The last `sync` input sample, for edge detection and the edge's fractional position.
    last_sync: f32,
}

pub struct OscVa {
    c: Core,
    sample_rate: f32,
}

impl OscVa {
    pub fn new() -> Self {
        OscVa {
            c: Core {
                phase: START_PHASE,
                gain: [-1.0; MAX_UNISON],
                pw: -1.0,
                last_sync: 0.0,
            },
            sample_rate: 48000.0,
        }
    }
}

impl Default for OscVa {
    fn default() -> Self {
        Self::new()
    }
}

/// Where between `a` (previous sample) and `b` a rising edge through 0.5 lies, 0..1.
#[inline]
fn crossing(a: f32, b: f32) -> f32 {
    let f = (0.5 - a) / (b - a);
    if f.is_finite() {
        f.clamp(0.0, 1.0)
    } else {
        1.0
    }
}

impl Module for OscVa {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &OSC_VA_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    /// Hard sync: a rising edge through 0.5 on `sync` restarts every oscillator from phase 0
    /// at the edge's interpolated position between samples. The jump that causes is
    /// band-limited with the same polynomial residual as the waveform edges: the sample after
    /// the edge always, the sample before it when the edge lies inside the block (lookahead).
    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let pitch = io.input(PITCH_IN);
        let sync = io.input(SYNC_IN);
        let base_hz = io.param(BASE_HZ_PARAM);
        let waveform = waveform_from_param(io.param(WAVEFORM_PARAM).at(0));
        let pw_target = (io.param(PW_PARAM).at(0) / 100.0).clamp(0.05, 0.95);
        let fine = io.param(FINE_PARAM).at(0);
        let count = (io.param(UNISON_PARAM).at(0).round() as usize).clamp(1, MAX_UNISON);
        let detune = io.param(DETUNE_PARAM).at(0);
        // Params a hand-wired caller may leave out take their defaults.
        let extra = |i: usize, default: f32| {
            if i < io.param_count() {
                io.param(i).at(0)
            } else {
                default
            }
        };
        let curve = extra(CURVE_PARAM, 0.0).clamp(0.0, 100.0) / 100.0;
        let width = extra(WIDTH_PARAM, 0.0).clamp(0.0, 100.0) / 100.0;
        let blend = extra(BLEND_PARAM, 50.0).clamp(0.0, 100.0) / 100.0;
        let restart = extra(RESTART_PARAM, 0.0) >= 0.5;
        let stereo = io.output_count() > OUT_R;
        let n = io.block_len();
        let sr = self.sample_rate;
        let c = &mut self.c;

        // Per-oscillator frequency ratio, level and pan, per block. Ratio exactly 1 for the old
        // oscillator.
        let mut ratio = [1.0f32; MAX_UNISON];
        let mut target = [0.0f32; MAX_UNISON];
        let mut pan = [(1.0f32, 1.0f32); MAX_UNISON];
        let mut start = [0.0f32; MAX_UNISON];
        // Centre oscillators: the one (odd count) or two (even) nearest the middle. A stack of
        // one or two has no side oscillators, so blend does nothing there.
        let sides = count > 2;
        let (w_centre, w_side) = if sides {
            ((2.0 * (1.0 - blend)).min(1.0), (2.0 * blend).min(1.0))
        } else {
            (1.0, 1.0)
        };
        let mut weight = [0.0f32; MAX_UNISON];
        let mut power = 0.0f32;
        for (k, w) in weight.iter_mut().enumerate().take(count) {
            let x = position(count, k);
            *w = if x.abs() * ((count - 1).max(1) as f32) < 0.51 {
                w_centre
            } else {
                w_side
            };
            power += *w * *w;
        }
        let level = 1.0 / power.sqrt();
        for k in 0..MAX_UNISON {
            let mut x = if k < count { position(count, k) } else { 0.0 };
            if curve > 0.0 {
                // Power curve: inner oscillators pulled toward the centre, the outermost stay.
                x = x.signum() * 0.5 * (2.0 * x.abs()).powf(1.0 + 2.0 * curve);
            }
            let cents = fine + x * detune;
            if cents != 0.0 {
                ratio[k] = 2f32.powf(cents / 1200.0);
            }
            if k < count {
                target[k] = weight[k] * level;
                if width > 0.0 {
                    // Equal-power pan; the middle is unity in each channel.
                    let a = (position(count, k) * 2.0 * width + 1.0) * (PI * 0.25);
                    pan[k] = (
                        a.cos() * std::f32::consts::SQRT_2,
                        a.sin() * std::f32::consts::SQRT_2,
                    );
                }
            }
            if restart {
                start[k] = START_PHASE[k];
            }
        }
        if c.gain[0] < 0.0 {
            c.gain = target;
        }
        if c.pw < 0.0 {
            c.pw = pw_target;
        }
        let k_gain = 1.0 - (-1.0 / (GAIN_TAU_S * sr)).exp();
        let pw_step = (pw_target - c.pw) / n as f32;

        let mut pw = c.pw;
        let mut prev_sync = c.last_sync;
        for i in 0..n {
            pw += pw_step;
            let s = sync.at(i);
            let edge = s > 0.5 && prev_sync <= 0.5;
            let f = if edge { crossing(prev_sync, s) } else { 0.0 };
            // An edge between this sample and the next one (inside the block).
            let next = if i + 1 < n && s <= 0.5 && sync.at(i + 1) > 0.5 {
                Some(crossing(s, sync.at(i + 1)))
            } else {
                None
            };
            prev_sync = s;
            let freq = base_hz.at(i) * 2f32.powf(pitch.at(i) / 12.0);

            let (mut v, mut v_r) = (0.0, 0.0);
            for k in 0..MAX_UNISON {
                let g = &mut c.gain[k];
                if *g != target[k] {
                    *g += (target[k] - *g) * k_gain;
                    if (*g - target[k]).abs() < 1e-5 {
                        *g = target[k];
                    }
                }
                if *g == 0.0 {
                    continue;
                }
                // Above half the sample rate no band-limiting holds; keep the phase sane.
                let dt = (freq * ratio[k] / sr).min(0.5);
                let p = &mut c.phase[k];
                let s0 = start[k];
                let mut y = if edge {
                    // Phase at the edge, one sample's (1 - f) back; restart from `s0` there.
                    let at_edge = (*p - dt * (1.0 - f)).rem_euclid(1.0);
                    let jump = naive(s0, waveform, pw) - naive(at_edge, waveform, pw);
                    *p = s0 + dt * (1.0 - f);
                    naive(*p, waveform, pw) - 0.5 * f * f * jump - dc(waveform, pw)
                } else {
                    sample(*p, dt, waveform, pw)
                };
                if let Some(fn_) = next {
                    let at_edge = (*p + dt * fn_).rem_euclid(1.0);
                    let jump = naive(s0, waveform, pw) - naive(at_edge, waveform, pw);
                    y += 0.5 * (1.0 - fn_) * (1.0 - fn_) * jump;
                }
                *p += dt;
                if *p >= 1.0 {
                    *p -= 1.0;
                }
                let y = if *g == 1.0 { y } else { *g * y };
                if width > 0.0 {
                    v += y * pan[k].0;
                    v_r += y * pan[k].1;
                } else {
                    v += y;
                }
            }
            io.output(OUT)[i] = v;
            if stereo {
                io.output(OUT_R)[i] = if width > 0.0 { v_r } else { v };
            }
        }
        c.pw = pw_target;
        c.last_sync = prev_sync;
    }

    fn reset(&mut self) {
        let sr = self.sample_rate;
        *self = OscVa::new();
        self.sample_rate = sr;
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        out.write_f32("phase", self.c.phase[0]);
        out.write_f32("sync_was_high", (self.c.last_sync > 0.5) as u8 as f32);
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        if let Some(phase) = s.read_f32("phase") {
            self.c.phase[0] = phase;
        }
        if let Some(v) = s.read_f32("sync_was_high") {
            self.c.last_sync = if v != 0.0 { 1.0 } else { 0.0 };
        }
    }

    /// The whole oscillator state: every unison phase, fade levels, pulse width, sync history.
    fn carry_from(&mut self, old: &dyn Module) {
        if let Some(o) = old.as_any().downcast_ref::<OscVa>() {
            self.c = o.c;
        }
    }
}

/// The DC the square's `sample` removes (the naive pulse's mean).
#[inline]
fn dc(w: OscWaveform, pw: f32) -> f32 {
    if w == OscWaveform::Square {
        2.0 * pw - 1.0
    } else {
        0.0
    }
}
