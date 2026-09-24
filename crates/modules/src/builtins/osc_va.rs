//! `osc.va` (brief section 8's v1 built-in table: "saw/square/tri/sine, PolyBLEP, hard sync
//! input"). Saw and square are PolyBLEP-corrected; triangle and sine are not (triangle needs
//! polyBLAMP for its corners, not implemented; its harmonics fall at 12 dB/octave, so it aliases
//! far less than an uncorrected saw would).
//!
//! Sound palette batch (docs/sound-palette-batch/README.md): pulse width (`pw`, 5–95 %, the
//! square only, ramped per sample across each block so a modulated width never steps, DC
//! removed so PWM doesn't thump), fine tuning (`fine`, ±100 cents), unison (`unison`, 1–4
//! oscillators per voice, `detune` = spread from lowest to highest in cents, level 1/√N so the
//! loudness of uncorrelated oscillators stays put, each oscillator fading in or out over 20 ms
//! when the count changes), and band-limited hard sync (see `process`). Defaults (50 %, 0 ct,
//! 1 oscillator) are the old oscillator sample for sample. The output stays one mono `out`:
//! stereo width is the chorus's job, so every existing cable keeps its meaning.
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
    },
    PortInfo {
        name: "sync",
        port_type: PortType::Gate,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "out",
        port_type: PortType::Audio,
        direction: PortDirection::Output,
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
    // Oscillators per voice, 1 (the old oscillator) to 4.
    ParamInfo {
        name: "unison",
        min: 1.0,
        max: 4.0,
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
    advanced: &["pw", "fine", "unison", "detune"],
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

pub const MAX_UNISON: usize = 4;

/// Where each unison oscillator sits within the detune spread (−0.5 = lowest, +0.5 =
/// highest), by count. Oscillator 0 alone plays at the centre, so one oscillator is the old
/// oscillator.
const SPREAD: [[f32; MAX_UNISON]; MAX_UNISON] = [
    [0.0, 0.0, 0.0, 0.0],
    [-0.5, 0.5, 0.0, 0.0],
    [0.0, -0.5, 0.5, 0.0],
    [-1.0 / 6.0, -0.5, 0.5, 1.0 / 6.0],
];
/// Starting phases of the unison oscillators: never aligned, so a new unison voice doesn't
/// double the first cycle. Oscillator 0 starts at 0, as before.
const START_PHASE: [f32; MAX_UNISON] = [0.0, 0.31, 0.62, 0.87];
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
        let n = io.block_len();
        let sr = self.sample_rate;
        let c = &mut self.c;

        // Per-oscillator frequency ratio, per block. Exactly 1 for the old oscillator.
        let mut ratio = [1.0f32; MAX_UNISON];
        let mut target = [0.0f32; MAX_UNISON];
        let level = 1.0 / (count as f32).sqrt();
        for k in 0..MAX_UNISON {
            let cents = fine + SPREAD[count - 1][k] * detune;
            if cents != 0.0 {
                ratio[k] = 2f32.powf(cents / 1200.0);
            }
            if k < count {
                target[k] = level;
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

        let out = &mut io.output(OUT)[..n];
        let mut pw = c.pw;
        let mut prev_sync = c.last_sync;
        for (i, o) in out.iter_mut().enumerate() {
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

            let mut v = 0.0;
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
                let dt = freq * ratio[k] / sr;
                let p = &mut c.phase[k];
                let mut y = if edge {
                    // Phase at the edge, one sample's (1 - f) back; restart from 0 there.
                    let at_edge = (*p - dt * (1.0 - f)).rem_euclid(1.0);
                    let jump = naive(0.0, waveform, pw) - naive(at_edge, waveform, pw);
                    *p = dt * (1.0 - f);
                    naive(*p, waveform, pw) - 0.5 * f * f * jump - dc(waveform, pw)
                } else {
                    sample(*p, dt, waveform, pw)
                };
                if let Some(fn_) = next {
                    let at_edge = (*p + dt * fn_).rem_euclid(1.0);
                    let jump = naive(0.0, waveform, pw) - naive(at_edge, waveform, pw);
                    y += 0.5 * (1.0 - fn_) * (1.0 - fn_) * jump;
                }
                *p += dt;
                if *p >= 1.0 {
                    *p -= 1.0;
                }
                v += if *g == 1.0 { y } else { *g * y };
            }
            *o = v;
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
