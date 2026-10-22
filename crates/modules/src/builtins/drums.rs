//! The drum voices: `drum.kick`, `drum.tom`, `drum.snare`, `drum.clap` and `drum.hat`. Analog-style
//! synthesis in the 808 / 909 tradition, one module per voice so each has its own jacks and
//! face, can be mixed, filtered or sent to a compressor's sidechain on its own, and costs
//! nothing while silent.
//!
//! **Triggering.** Every voice has a `gate` input (the hat has `closed` and `open`), an `accent`
//! input and a `velocity` input. A rising edge through 0.5 starts a hit at that sample: the
//! voice reads `velocity` and `accent` on that sample only, so a sequencer's, arpeggiator's,
//! clock's or `midi.drums`' gate works unchanged. Falling edges are ignored: a drum is a
//! one-shot. Hit gain is `1 - vel_sens * (1 - velocity)` times `accent_db * accent` of boost. An
//! unpatched `velocity` (a held 0) counts as full velocity, so a voice patched to a bare gate
//! plays at full level.
//!
//! **Retrigger without a click.** A hit that starts while the voice still sounds keeps the
//! output where it was: the last output sample becomes an offset that fades out over 1 ms, added
//! to the new hit, so the waveform never steps (`Declick`).
//!
//! **Decay times** are the time to fall 60 dB (`t60`); envelopes are one multiply per sample.
//! A voice that has fallen below -80 dB (relative to its drive) stops processing and writes
//! zeros, so an idle kit costs a few comparisons per block.
//!
//! **Numerics.** Every frequency is clamped below 0.45 of the sample rate; filters are
//! recomputed once per block, and only when the control changed; the drive is `tanh`, so every
//! output is bounded whatever the settings. Hat squares use PolyBLEP, so they do not fold back
//! at 44.1 kHz.
//!
//! `view().position` is the seconds since the last hit (the face's playhead); the hat also
//! sets `cycle[0]` to 1.0 when that hit was the open one.

use crate::dsp::{Svf, SvfCoeffs};
use crate::info::{
    Category, ModuleInfo, ParamInfo, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::{ProcessIo, Signal};
use crate::module::{Module, QualityConfig};
use crate::view::ModuleView;

use super::fx::{param, port};

const TAU: f32 = std::f32::consts::TAU;
/// ln(1000): 60 dB as a neper ratio.
const LN_1000: f32 = 6.907_755;
/// Below this envelope level (relative to the drive gain) a voice is finished.
const FLOOR: f32 = 1e-4;
/// Fade of the retrigger offset (`Declick`) in ms.
const DECLICK_MS: f32 = 1.0;

const fn input(name: &'static str, t: PortType) -> PortInfo {
    port(name, t, true)
}

const OUT: PortInfo = port("out", PortType::Audio, false);

const NO_QUALITY: QualitySupport = QualitySupport {
    oversampling: false,
    anti_aliasing: false,
    interpolation: false,
};

const fn lin(
    name: &'static str,
    min: f32,
    max: f32,
    default: f32,
    unit: &'static str,
) -> ParamInfo {
    param(name, min, max, default, unit, Taper::Linear)
}

const fn exp(
    name: &'static str,
    min: f32,
    max: f32,
    default: f32,
    unit: &'static str,
) -> ParamInfo {
    param(name, min, max, default, unit, Taper::Exponential)
}

/// The three params every voice ends with.
const LEVEL: ParamInfo = lin("level_db", -24.0, 6.0, -3.0, "dB");
const VEL_SENS: ParamInfo = lin("vel_sens", 0.0, 1.0, 0.6, "");
const ACCENT: ParamInfo = lin("accent_db", 0.0, 12.0, 4.0, "dB");

// ---- shared state ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct Rng(u32);

impl Rng {
    #[inline]
    fn white(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 * (2.0 / 16_777_216.0) - 1.0
    }
}

/// Rising edge through 0.5.
#[derive(Debug, Clone, Copy, Default)]
struct Edge(bool);

impl Edge {
    #[inline]
    fn rising(&mut self, g: f32) -> bool {
        let high = g > 0.5;
        let r = high && !self.0;
        self.0 = high;
        r
    }
}

/// The offset that keeps a retriggered output continuous: the first sample of the new hit is
/// moved onto the last sample of the old output, and the difference fades out over 1 ms. A hit
/// from silence (last output near 0) is left alone, so its attack stays sharp.
#[derive(Debug, Clone, Copy, Default)]
struct Declick {
    off: f32,
    last: f32,
    pending: bool,
}

impl Declick {
    #[inline]
    fn hit(&mut self) {
        self.pending = true;
    }
    /// `x` plus the fading offset; call once per sample, silent samples included.
    #[inline]
    fn apply(&mut self, x: f32, k: f32) -> f32 {
        if self.pending {
            self.pending = false;
            if self.last.abs() > 1e-4 {
                self.off = self.last - x;
            }
        }
        let y = x + self.off;
        self.off *= k;
        if self.off.abs() < 1e-9 {
            self.off = 0.0;
        }
        self.last = y;
        y
    }
}

/// Per-sample retention for a 60 dB decay in `ms`.
#[inline]
fn t60(ms: f32, sr: f32) -> f32 {
    (-LN_1000 / (ms.max(0.05) * 0.001 * sr)).exp()
}

#[inline]
fn max_hz(sr: f32) -> f32 {
    0.45 * sr
}

#[inline]
fn db(x: f32) -> f32 {
    10f32.powf(x / 20.0)
}

/// Hit gain from the velocity and accent inputs at the trigger sample.
#[inline]
fn hit_gain(vel: Signal, accent: Signal, i: usize, sens: f32, accent_db: f32) -> f32 {
    let v = vel.at(i);
    // An unpatched velocity is a held 0: full velocity.
    let v = if vel.is_scalar() && v == 0.0 {
        1.0
    } else {
        v.max(0.0).min(1.0)
    };
    let a = accent.at(i).max(0.0).min(1.0);
    (1.0 - sens * (1.0 - v)) * db(accent_db * a)
}

/// `tanh` drive normalised to unit peak: `g` is the linear drive gain (at least 1).
#[inline]
fn drive(x: f32, g: f32, norm: f32) -> f32 {
    (x * g).tanh() * norm
}

fn drive_consts(drive_db: f32) -> (f32, f32) {
    let g = db(drive_db.max(0.0).min(36.0));
    (g, 1.0 / g.tanh())
}

/// A param read once per block, with a finite fallback.
#[inline]
fn p(io: &ProcessIo, i: usize, lo: f32, hi: f32) -> f32 {
    let v = io.param(i).at(0);
    if v.is_finite() {
        v.max(lo).min(hi)
    } else {
        lo
    }
}

// ---- kick and tom ----------------------------------------------------------------------------

/// Params of `drum.kick` and `drum.tom` (same order, different ranges).
const T_TUNE: usize = 0;
const T_SWEEP: usize = 1;
const T_SWEEP_MS: usize = 2;
const T_DECAY: usize = 3;
const T_CLICK: usize = 4;
const T_TONE: usize = 5;
const T_DRIVE: usize = 6;
const T_LEVEL: usize = 7;
const T_VEL: usize = 8;
const T_ACCENT: usize = 9;

const THUMP_PORTS: &[PortInfo] = &[
    input("gate", PortType::Gate),
    input("accent", PortType::UnipolarCv),
    input("velocity", PortType::UnipolarCv),
    OUT,
];

const KICK_PARAMS: &[ParamInfo] = &[
    exp("tune_hz", 30.0, 120.0, 50.0, "Hz"),
    lin("sweep_st", 0.0, 60.0, 34.0, "st"),
    exp("sweep_ms", 2.0, 200.0, 28.0, "ms"),
    exp("decay_ms", 60.0, 2000.0, 420.0, "ms"),
    lin("click", 0.0, 1.0, 0.35, ""),
    lin("tone", 0.0, 1.0, 0.55, ""),
    lin("drive_db", 0.0, 30.0, 8.0, "dB"),
    LEVEL,
    VEL_SENS,
    ACCENT,
];

const TOM_PARAMS: &[ParamInfo] = &[
    exp("tune_hz", 60.0, 500.0, 140.0, "Hz"),
    lin("sweep_st", 0.0, 36.0, 10.0, "st"),
    exp("sweep_ms", 2.0, 200.0, 45.0, "ms"),
    exp("decay_ms", 60.0, 2000.0, 380.0, "ms"),
    lin("click", 0.0, 1.0, 0.2, ""),
    lin("tone", 0.0, 1.0, 0.7, ""),
    lin("drive_db", 0.0, 30.0, 3.0, "dB"),
    LEVEL,
    VEL_SENS,
    ACCENT,
];

pub static KICK_INFO: ModuleInfo = ModuleInfo {
    kind: "drum.kick",
    name: "Kick",
    category: Category::Source,
    rate: Rate::Global,
    explain: "A kick drum. A sine that starts high and drops fast to its Tune pitch is the punch; Decay is the body, Click the beater, Drive the weight. Send it a gate.",
    lesson: None,
    requires: &[],
    ports: THUMP_PORTS,
    params: KICK_PARAMS,
    quality: NO_QUALITY,
    skin: None,
    width_units: 8,
    advanced: &["vel_sens", "accent_db"],
};

pub static TOM_INFO: ModuleInfo = ModuleInfo {
    kind: "drum.tom",
    name: "Tom",
    category: Category::Source,
    rate: Rate::Global,
    explain: "A tom drum: a tuned sine with a small pitch drop. Tune it for a low floor tom or a high rack tom. Send it a gate.",
    lesson: None,
    requires: &[],
    ports: THUMP_PORTS,
    params: TOM_PARAMS,
    quality: NO_QUALITY,
    skin: None,
    width_units: 8,
    advanced: &["vel_sens", "accent_db"],
};

/// What the face draws for a kick or tom: frequency and amplitude at time `t` (s) after a hit.
pub fn thump_curve(tune: f32, sweep_st: f32, sweep_ms: f32, decay_ms: f32, t: f32) -> (f32, f32) {
    let start = tune * 2f32.powf(sweep_st / 12.0);
    let f = tune + (start - tune) * (-3.0 * t / (sweep_ms * 0.001).max(1e-4)).exp();
    let a = (-LN_1000 * t / (decay_ms * 0.001).max(1e-4)).exp();
    (f, a)
}

#[derive(Debug, Clone, Copy)]
struct Thump {
    trig: Edge,
    declick: Declick,
    rng: Rng,
    phase: f32,
    pitch: f32,
    amp: f32,
    click: f32,
    lp: f32,
    gain: f32,
    active: bool,
    age: u32,
}

impl Thump {
    const fn new(seed: u32) -> Self {
        Thump {
            trig: Edge(false),
            declick: Declick {
                off: 0.0,
                last: 0.0,
                pending: false,
            },
            rng: Rng(seed),
            phase: 0.0,
            pitch: 0.0,
            amp: 0.0,
            click: 0.0,
            lp: 0.0,
            gain: 1.0,
            active: false,
            age: 0,
        }
    }

    fn run(&mut self, io: &mut ProcessIo, sr: f32) {
        let n = io.block_len();
        let (gate, accent, vel) = (io.input(0), io.input(1), io.input(2));
        let tune = p(io, T_TUNE, 1.0, 2000.0);
        let sweep = p(io, T_SWEEP, 0.0, 96.0);
        let start = (tune * 2f32.powf(sweep / 12.0)).min(max_hz(sr));
        let kp = (-3.0 / (p(io, T_SWEEP_MS, 0.5, 2000.0) * 0.001 * sr)).exp();
        let ka = t60(p(io, T_DECAY, 5.0, 10000.0), sr);
        let kc = t60(2.5, sr);
        let click_amt = p(io, T_CLICK, 0.0, 1.0) * 0.8;
        let tone = p(io, T_TONE, 0.0, 1.0);
        let fc = (350.0 * 50f32.powf(tone)).min(max_hz(sr));
        let a_lp = 1.0 - (-TAU * fc / sr).exp();
        let (g, norm) = drive_consts(p(io, T_DRIVE, 0.0, 36.0));
        let level = db(p(io, T_LEVEL, -60.0, 12.0));
        let (sens, acc_db) = (p(io, T_VEL, 0.0, 1.0), p(io, T_ACCENT, 0.0, 24.0));
        let kd = t60(DECLICK_MS, sr);
        let floor = FLOOR / g;
        let base = tune.min(max_hz(sr));
        let out = &mut io.output(0)[..n];
        for (i, o) in out.iter_mut().enumerate() {
            if self.trig.rising(gate.at(i)) {
                self.declick.hit();
                self.phase = 0.0;
                self.pitch = 1.0;
                self.amp = 1.0;
                self.click = 1.0;
                self.gain = hit_gain(vel, accent, i, sens, acc_db);
                self.active = true;
                self.age = 0;
            }
            if !self.active {
                *o = self.declick.apply(0.0, kd);
                continue;
            }
            let f = base + (start - base) * self.pitch;
            self.phase += f / sr;
            self.phase -= self.phase.floor();
            let x = (TAU * self.phase).sin() * self.amp + self.rng.white() * self.click * click_amt;
            self.lp += a_lp * (x - self.lp);
            *o = self
                .declick
                .apply(drive(self.lp, g, norm) * self.gain * level, kd);
            self.pitch *= kp;
            self.amp *= ka;
            self.click *= kc;
            self.age = self.age.saturating_add(1);
            if self.amp < floor {
                self.active = false;
                self.lp = 0.0;
            }
        }
    }

    fn view(&self, sr: f32, out: &mut ModuleView) {
        out.valid = self.active;
        out.position = self.age as f32 / sr;
    }
}

macro_rules! thump_module {
    ($ty:ident, $info:ident, $seed:expr) => {
        pub struct $ty {
            v: Thump,
            sample_rate: f32,
        }
        impl $ty {
            pub fn new() -> Self {
                $ty {
                    v: Thump::new($seed),
                    sample_rate: 48000.0,
                }
            }
        }
        impl Default for $ty {
            fn default() -> Self {
                Self::new()
            }
        }
        impl Module for $ty {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
            fn info(&self) -> &'static ModuleInfo {
                &$info
            }
            fn prepare(&mut self, sample_rate: f32, _max_block: usize, _q: &QualityConfig) {
                self.sample_rate = sample_rate;
            }
            #[inline]
            fn process(&mut self, io: &mut ProcessIo) {
                self.v.run(io, self.sample_rate);
            }
            fn view(&self, out: &mut ModuleView) {
                self.v.view(self.sample_rate, out);
            }
            fn reset(&mut self) {
                self.v = Thump::new($seed);
            }
            fn carry_from(&mut self, old: &dyn Module) {
                if let Some(o) = old.as_any().downcast_ref::<$ty>() {
                    self.v = o.v;
                }
            }
        }
    };
}

thump_module!(Kick, KICK_INFO, 0x1234_5679);
thump_module!(Tom, TOM_INFO, 0x2468_ACE1);

// ---- snare -----------------------------------------------------------------------------------

const S_TUNE: usize = 0;
const S_TONE: usize = 1;
const S_SNAPPY: usize = 2;
const S_DECAY: usize = 3;
const S_DRIVE: usize = 4;
const S_LEVEL: usize = 5;
const S_VEL: usize = 6;
const S_ACCENT: usize = 7;

const SNARE_PARAMS: &[ParamInfo] = &[
    exp("tune_hz", 100.0, 400.0, 185.0, "Hz"),
    lin("tone", 0.0, 1.0, 0.5, ""),
    lin("snappy", 0.0, 1.0, 0.55, ""),
    exp("decay_ms", 60.0, 1200.0, 260.0, "ms"),
    lin("drive_db", 0.0, 30.0, 6.0, "dB"),
    LEVEL,
    VEL_SENS,
    ACCENT,
];

pub static SNARE_INFO: ModuleInfo = ModuleInfo {
    kind: "drum.snare",
    name: "Snare",
    category: Category::Source,
    rate: Rate::Global,
    explain: "A snare drum: two tuned drum tones plus a burst of noise. Snappy balances noise against the drum, Tone brightens the noise, Decay sets how long the rattle rings.",
    lesson: None,
    requires: &[],
    ports: THUMP_PORTS,
    params: SNARE_PARAMS,
    quality: NO_QUALITY,
    skin: None,
    width_units: 8,
    advanced: &["vel_sens", "accent_db"],
};

/// The snare's decays in ms: (drum body, noise).
pub fn snare_decays(decay_ms: f32) -> (f32, f32) {
    ((decay_ms * 0.4).max(30.0), decay_ms)
}

#[derive(Debug, Clone, Copy)]
pub struct Snare {
    trig: Edge,
    declick: Declick,
    rng: Rng,
    ph1: f32,
    ph2: f32,
    pitch: f32,
    body: f32,
    noise: f32,
    hp: Svf,
    gain: f32,
    active: bool,
    age: u32,
    sample_rate: f32,
}

impl Snare {
    pub fn new() -> Self {
        Snare {
            trig: Edge(false),
            declick: Declick::default(),
            rng: Rng(0x7F4A_7C15),
            ph1: 0.0,
            ph2: 0.0,
            pitch: 0.0,
            body: 0.0,
            noise: 0.0,
            hp: Svf::new(),
            gain: 1.0,
            active: false,
            age: 0,
            sample_rate: 48000.0,
        }
    }
}

impl Default for Snare {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Snare {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn info(&self) -> &'static ModuleInfo {
        &SNARE_INFO
    }
    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _q: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let sr = self.sample_rate;
        let n = io.block_len();
        let (gate, accent, vel) = (io.input(0), io.input(1), io.input(2));
        let tune = p(io, S_TUNE, 20.0, 1000.0).min(max_hz(sr) / 2.0);
        let tone = p(io, S_TONE, 0.0, 1.0);
        let snappy = p(io, S_SNAPPY, 0.0, 1.0);
        let (body_ms, noise_ms) = snare_decays(p(io, S_DECAY, 5.0, 10000.0));
        let (kb, kn) = (t60(body_ms, sr), t60(noise_ms, sr));
        let kp = t60(12.0, sr);
        let hp = SvfCoeffs::compute((1200.0 * 6f32.powf(tone)).min(max_hz(sr)), 0.3, sr);
        let (wb, wn) = (
            (snappy * std::f32::consts::FRAC_PI_2).cos() * 0.8,
            (snappy * std::f32::consts::FRAC_PI_2).sin() * 1.3,
        );
        let (g, norm) = drive_consts(p(io, S_DRIVE, 0.0, 36.0));
        let level = db(p(io, S_LEVEL, -60.0, 12.0));
        let (sens, acc_db) = (p(io, S_VEL, 0.0, 1.0), p(io, S_ACCENT, 0.0, 24.0));
        let kd = t60(DECLICK_MS, sr);
        let floor = FLOOR / g;
        let out = &mut io.output(0)[..n];
        for (i, o) in out.iter_mut().enumerate() {
            if self.trig.rising(gate.at(i)) {
                self.declick.hit();
                (self.ph1, self.ph2) = (0.0, 0.0);
                (self.pitch, self.body, self.noise) = (1.0, 1.0, 1.0);
                self.gain = hit_gain(vel, accent, i, sens, acc_db);
                self.active = true;
                self.age = 0;
            }
            if !self.active {
                *o = self.declick.apply(0.0, kd);
                continue;
            }
            let f = tune * (1.0 + 0.4 * self.pitch);
            self.ph1 += f / sr;
            self.ph1 -= self.ph1.floor();
            self.ph2 += f * 1.78 / sr;
            self.ph2 -= self.ph2.floor();
            let drum = ((TAU * self.ph1).sin() * 0.7 + (TAU * self.ph2).sin() * 0.3) * self.body;
            let hiss = self
                .hp
                .process_with_coeffs_multi(self.rng.white(), &hp)
                .highpass
                * self.noise;
            let x = drum * wb + hiss * wn;
            *o = self
                .declick
                .apply(drive(x, g, norm) * self.gain * level, kd);
            self.pitch *= kp;
            self.body *= kb;
            self.noise *= kn;
            self.age = self.age.saturating_add(1);
            if self.noise < floor && self.body < floor {
                self.active = false;
                self.hp = Svf::new();
            }
        }
    }

    fn view(&self, out: &mut ModuleView) {
        out.valid = self.active;
        out.position = self.age as f32 / self.sample_rate;
    }

    fn reset(&mut self) {
        *self = Snare {
            sample_rate: self.sample_rate,
            ..Snare::new()
        };
    }

    fn carry_from(&mut self, old: &dyn Module) {
        if let Some(o) = old.as_any().downcast_ref::<Snare>() {
            *self = *o;
        }
    }
}

// ---- clap ------------------------------------------------------------------------------------

const C_BURSTS: usize = 0;
const C_SPREAD: usize = 1;
const C_DECAY: usize = 2;
const C_TONE: usize = 3;
const C_LEVEL: usize = 4;
const C_VEL: usize = 5;
const C_ACCENT: usize = 6;

const CLAP_PARAMS: &[ParamInfo] = &[
    param("bursts", 2.0, 6.0, 4.0, "", Taper::Stepped),
    exp("spread_ms", 3.0, 30.0, 9.0, "ms"),
    exp("decay_ms", 50.0, 1000.0, 240.0, "ms"),
    lin("tone", 0.0, 1.0, 0.45, ""),
    LEVEL,
    VEL_SENS,
    ACCENT,
];

pub static CLAP_INFO: ModuleInfo = ModuleInfo {
    kind: "drum.clap",
    name: "Clap",
    category: Category::Source,
    rate: Rate::Global,
    explain: "Hand claps: several quick noise bursts, then a ringing tail. Bursts is how many hands, Spread the gap between them, Tone moves the colour of the noise.",
    lesson: None,
    requires: &[],
    ports: THUMP_PORTS,
    params: CLAP_PARAMS,
    quality: NO_QUALITY,
    skin: None,
    width_units: 8,
    advanced: &["vel_sens", "accent_db"],
};

/// The clap envelope at `t` seconds after a hit, for the face: each of the `bursts - 1` short
/// bursts falls to 5 % within `spread_ms`; the last burst is the tail with a 60 dB time of `decay_ms`.
pub fn clap_curve(bursts: usize, spread_ms: f32, decay_ms: f32, t: f32) -> f32 {
    let spread = (spread_ms * 0.001).max(1e-4);
    let stage = (t / spread) as usize;
    if stage + 1 < bursts {
        (-(20f32).ln() * (t - stage as f32 * spread) / spread).exp()
    } else {
        let t0 = (bursts.max(1) - 1) as f32 * spread;
        (-LN_1000 * (t - t0) / (decay_ms * 0.001).max(1e-4)).exp()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Clap {
    trig: Edge,
    declick: Declick,
    rng: Rng,
    bp: Svf,
    env: f32,
    stage: u32,
    count: u32,
    gain: f32,
    active: bool,
    age: u32,
    sample_rate: f32,
}

impl Clap {
    pub fn new() -> Self {
        Clap {
            trig: Edge(false),
            declick: Declick::default(),
            rng: Rng(0x94D0_49BB),
            bp: Svf::new(),
            env: 0.0,
            stage: 0,
            count: 0,
            gain: 1.0,
            active: false,
            age: 0,
            sample_rate: 48000.0,
        }
    }
}

impl Default for Clap {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Clap {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn info(&self) -> &'static ModuleInfo {
        &CLAP_INFO
    }
    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _q: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let sr = self.sample_rate;
        let n = io.block_len();
        let (gate, accent, vel) = (io.input(0), io.input(1), io.input(2));
        let bursts = (p(io, C_BURSTS, 2.0, 6.0).round() as u32).max(2);
        let spread = ((p(io, C_SPREAD, 1.0, 100.0) * 0.001 * sr) as u32).max(1);
        let kb = (-(20f32).ln() / spread as f32).exp();
        let kt = t60(p(io, C_DECAY, 5.0, 10000.0), sr);
        let tone = p(io, C_TONE, 0.0, 1.0);
        let bp = SvfCoeffs::compute((600.0 * 5f32.powf(tone)).min(max_hz(sr)), 0.7, sr);
        let level = db(p(io, C_LEVEL, -60.0, 12.0)) * 2.4;
        let (sens, acc_db) = (p(io, C_VEL, 0.0, 1.0), p(io, C_ACCENT, 0.0, 24.0));
        let kd = t60(DECLICK_MS, sr);
        let out = &mut io.output(0)[..n];
        for (i, o) in out.iter_mut().enumerate() {
            if self.trig.rising(gate.at(i)) {
                self.declick.hit();
                (self.env, self.stage, self.count) = (1.0, 0, 0);
                self.gain = hit_gain(vel, accent, i, sens, acc_db);
                self.active = true;
                self.age = 0;
            }
            if !self.active {
                *o = self.declick.apply(0.0, kd);
                continue;
            }
            let x = self
                .bp
                .process_with_coeffs_multi(self.rng.white(), &bp)
                .bandpass
                * self.env;
            *o = self.declick.apply((x * self.gain * level).tanh(), kd);
            self.age = self.age.saturating_add(1);
            if self.stage + 1 < bursts {
                self.env *= kb;
                self.count += 1;
                if self.count >= spread {
                    (self.count, self.stage, self.env) = (0, self.stage + 1, 1.0);
                }
            } else {
                self.env *= kt;
                if self.env < FLOOR {
                    self.active = false;
                    self.bp = Svf::new();
                }
            }
        }
    }

    fn view(&self, out: &mut ModuleView) {
        out.valid = self.active;
        out.position = self.age as f32 / self.sample_rate;
    }

    fn reset(&mut self) {
        *self = Clap {
            sample_rate: self.sample_rate,
            ..Clap::new()
        };
    }

    fn carry_from(&mut self, old: &dyn Module) {
        if let Some(o) = old.as_any().downcast_ref::<Clap>() {
            *self = *o;
        }
    }
}

// ---- hi-hat ----------------------------------------------------------------------------------

const H_TUNE: usize = 0;
const H_TONE: usize = 1;
const H_CLOSED: usize = 2;
const H_OPEN: usize = 3;
const H_LEVEL: usize = 4;
const H_VEL: usize = 5;
const H_ACCENT: usize = 6;

const HAT_PORTS: &[PortInfo] = &[
    input("closed", PortType::Gate),
    input("open", PortType::Gate),
    input("accent", PortType::UnipolarCv),
    input("velocity", PortType::UnipolarCv),
    OUT,
];

const HAT_PARAMS: &[ParamInfo] = &[
    lin("tune_st", -12.0, 12.0, 0.0, "st"),
    lin("tone", 0.0, 1.0, 0.5, ""),
    exp("closed_ms", 15.0, 300.0, 55.0, "ms"),
    exp("open_ms", 100.0, 2000.0, 480.0, "ms"),
    LEVEL,
    VEL_SENS,
    ACCENT,
];

pub static HAT_INFO: ModuleInfo = ModuleInfo {
    kind: "drum.hat",
    name: "Hi-Hat",
    category: Category::Source,
    rate: Rate::Global,
    explain: "A hi-hat from six detuned square waves, the 808 way. Gate Closed for a tick and Open for a long ring; a Closed hit cuts an Open one off, like a real pedal.",
    lesson: None,
    requires: &[],
    ports: HAT_PORTS,
    params: HAT_PARAMS,
    quality: NO_QUALITY,
    skin: None,
    width_units: 8,
    advanced: &["vel_sens", "accent_db"],
};

/// The 808's six square-wave frequencies (Hz), as measured from the circuit.
const HAT_HZ: [f32; 6] = [205.3, 304.4, 369.6, 522.7, 540.0, 800.0];

/// Square wave with PolyBLEP corrections at both edges; `p` in 0..1, `dt` the phase step.
#[inline]
fn square(p: f32, dt: f32) -> f32 {
    #[inline]
    fn blep(t: f32, dt: f32) -> f32 {
        if t < dt {
            let x = t / dt;
            x + x - x * x - 1.0
        } else if t > 1.0 - dt {
            let x = (t - 1.0) / dt;
            x * x + x + x + 1.0
        } else {
            0.0
        }
    }
    let naive = if p < 0.5 { 1.0 } else { -1.0 };
    naive + blep(p, dt) - blep((p + 0.5).fract(), dt)
}

/// The hat's high-pass corner (Hz) for `tone`.
pub fn hat_corner_hz(tone: f32) -> f32 {
    4500.0 * 2.6f32.powf(tone)
}

#[derive(Debug, Clone, Copy)]
pub struct Hat {
    closed: Edge,
    open: Edge,
    declick: Declick,
    phase: [f32; 6],
    hp: Svf,
    env: f32,
    k: f32,
    gain: f32,
    active: bool,
    was_open: bool,
    age: u32,
    sample_rate: f32,
}

impl Hat {
    pub fn new() -> Self {
        Hat {
            closed: Edge(false),
            open: Edge(false),
            declick: Declick::default(),
            phase: [0.0, 0.31, 0.13, 0.57, 0.77, 0.42],
            hp: Svf::new(),
            env: 0.0,
            k: 0.0,
            gain: 1.0,
            active: false,
            was_open: false,
            age: 0,
            sample_rate: 48000.0,
        }
    }
}

impl Default for Hat {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Hat {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn info(&self) -> &'static ModuleInfo {
        &HAT_INFO
    }
    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _q: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let sr = self.sample_rate;
        let n = io.block_len();
        let (closed, open, accent, vel) = (io.input(0), io.input(1), io.input(2), io.input(3));
        let ratio = 2f32.powf(p(io, H_TUNE, -24.0, 24.0) / 12.0);
        let dts: [f32; 6] = std::array::from_fn(|j| (HAT_HZ[j] * ratio / sr).min(0.45));
        let hp = SvfCoeffs::compute(
            hat_corner_hz(p(io, H_TONE, 0.0, 1.0)).min(max_hz(sr)),
            0.15,
            sr,
        );
        let (kc, ko) = (
            t60(p(io, H_CLOSED, 2.0, 5000.0), sr),
            t60(p(io, H_OPEN, 10.0, 10000.0), sr),
        );
        let level = db(p(io, H_LEVEL, -60.0, 12.0)) * 6.0;
        let (sens, acc_db) = (p(io, H_VEL, 0.0, 1.0), p(io, H_ACCENT, 0.0, 24.0));
        let kd = t60(DECLICK_MS, sr);
        let out = &mut io.output(0)[..n];
        for (i, o) in out.iter_mut().enumerate() {
            // The closed gate wins when both rise on one sample.
            let c = self.closed.rising(closed.at(i));
            let op = self.open.rising(open.at(i));
            if c || op {
                self.declick.hit();
                self.env = 1.0;
                self.k = if c { kc } else { ko };
                self.was_open = !c;
                self.gain = hit_gain(vel, accent, i, sens, acc_db);
                self.active = true;
                self.age = 0;
            }
            if !self.active {
                *o = self.declick.apply(0.0, kd);
                continue;
            }
            let mut s = 0.0;
            for (ph, &dt) in self.phase.iter_mut().zip(&dts) {
                *ph += dt;
                *ph -= ph.floor();
                s += square(*ph, dt);
            }
            let x = self
                .hp
                .process_with_coeffs_multi(s * (1.0 / 6.0), &hp)
                .highpass;
            *o = self
                .declick
                .apply((x * self.env * self.gain * level).tanh(), kd);
            self.env *= self.k;
            self.age = self.age.saturating_add(1);
            if self.env < FLOOR {
                self.active = false;
                self.hp = Svf::new();
            }
        }
    }

    fn view(&self, out: &mut ModuleView) {
        out.valid = self.active;
        out.position = self.age as f32 / self.sample_rate;
        out.cycle[0] = f32::from(u8::from(self.was_open));
    }

    fn reset(&mut self) {
        *self = Hat {
            sample_rate: self.sample_rate,
            ..Hat::new()
        };
    }

    fn carry_from(&mut self, old: &dyn Module) {
        if let Some(o) = old.as_any().downcast_ref::<Hat>() {
            *self = *o;
        }
    }
}
