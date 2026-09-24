//! `seq`: an 8-step pitch/gate sequencer with four pattern banks, A–D. Each rising edge on
//! `clock` advances one step; `pitch` holds that step's pitch (whole semitones, same reference
//! as `midi.in`'s `pitch`) and `velocity` its velocity (0..1, independent of the step's gate, so
//! a rest stays a rest). `transpose` shifts every step by whole semitones. A rising edge on
//! `reset` jumps to the start step if the clock is high, otherwise it makes the next clock tick
//! play the start step.
//!
//! Banks: bank A is the params the module always had (`p1`, `g1`, `v1`, `length`, ...), so an
//! old patch is bank A unchanged; banks B–D are the same names prefixed `b.`/`c.`/`d.`. Only
//! the playing bank is read. The playing bank is runtime state: a fresh instance starts on
//! `bank` (the startup bank), a carried one keeps playing what it played. `arm` switches bank on
//! the first clock edge at or after a sample offset; that edge plays the new bank's start step.
//! See docs/composition-batch/design.md for launch timing and the step order of `direction`.
//!
//! Probability (`r1..r8`, %) is drawn once per advance from the module's own xorshift stream
//! (one draw per advance whatever the values, so editing a probability never shifts the
//! stream). A rejected step is a rest. The stream is seeded from the module id (`seed`) and
//! reseeded on every reset edge.
//!
//! Gate timing (`gate_mode`, per bank):
//! - CLOCK (default, the original behaviour): `gate` follows the clock pulse while the step is
//!   on, so consecutive on-steps retrigger an envelope.
//! - LENGTH: `gate` rises on the step's clock edge and stays high for `gate_len` % of the step
//!   period. The period is the interval between rising edges, so a divided clock works. An
//!   interval counts when it agrees within 10 % with the one before it (the delay's sync rule):
//!   a steady or slowly moving tempo is followed at once, a jump after two agreeing intervals,
//!   and the long interval across a Stop or the short one of a Restart never sets it. Until
//!   the first period is known (the first step after a load), the gate follows the clock. A gate
//!   still high when the next step starts (100 %, or the tempo sped up) drops for that one
//!   sample so the next note retriggers. Stop lets the playing gate finish its length.
//!
//! An input counts as high above 0, not at the usual 0.5: a `midi.in` gate reaching this global
//! module is averaged over the voices, so one held key arrives as 1 / voice count.
//!
//! Global rate, like `clock`: every voice of a voice-rate chain it feeds plays the same notes,
//! and the compiler's voice average keeps that as loud as one voice.

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};

pub const STEPS: usize = 8;
pub const BANKS: usize = 4;
pub const BANK_NAMES: [&str; BANKS] = ["A", "B", "C", "D"];

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "clock",
        port_type: PortType::Gate,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "reset",
        port_type: PortType::Gate,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "gate",
        port_type: PortType::Gate,
        direction: PortDirection::Output,
    },
    PortInfo {
        name: "pitch",
        port_type: PortType::Pitch,
        direction: PortDirection::Output,
    },
    PortInfo {
        name: "velocity",
        port_type: PortType::UnipolarCv,
        direction: PortDirection::Output,
    },
];

const fn pitch(name: &'static str, default: f32) -> ParamInfo {
    ParamInfo {
        name,
        min: -24.0,
        max: 24.0,
        default,
        unit: "st",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    }
}

const fn stepped(name: &'static str, max: f32, default: f32) -> ParamInfo {
    ParamInfo {
        name,
        min: 0.0,
        max,
        default,
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    }
}

const fn percent(name: &'static str, min: f32, default: f32) -> ParamInfo {
    ParamInfo {
        name,
        min,
        max: 100.0,
        default,
        unit: "%",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    }
}

const fn length(name: &'static str) -> ParamInfo {
    ParamInfo {
        name,
        min: 1.0,
        max: STEPS as f32,
        default: STEPS as f32,
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    }
}

/// One bank's params in slot order: pitches, gates, length, velocities, gate length, gate mode,
/// probabilities. Bank A interleaves `transpose` after `length` (its historic position).
macro_rules! bank {
    ($pre:literal) => {
        [
            pitch(concat!($pre, "p1"), 0.0),
            pitch(concat!($pre, "p2"), 3.0),
            pitch(concat!($pre, "p3"), 7.0),
            pitch(concat!($pre, "p4"), 10.0),
            pitch(concat!($pre, "p5"), 12.0),
            pitch(concat!($pre, "p6"), 10.0),
            pitch(concat!($pre, "p7"), 7.0),
            pitch(concat!($pre, "p8"), 3.0),
            stepped(concat!($pre, "g1"), 1.0, 1.0),
            stepped(concat!($pre, "g2"), 1.0, 1.0),
            stepped(concat!($pre, "g3"), 1.0, 1.0),
            stepped(concat!($pre, "g4"), 1.0, 1.0),
            stepped(concat!($pre, "g5"), 1.0, 1.0),
            stepped(concat!($pre, "g6"), 1.0, 1.0),
            stepped(concat!($pre, "g7"), 1.0, 1.0),
            stepped(concat!($pre, "g8"), 1.0, 1.0),
            length(concat!($pre, "length")),
            percent(concat!($pre, "v1"), 0.0, 100.0),
            percent(concat!($pre, "v2"), 0.0, 100.0),
            percent(concat!($pre, "v3"), 0.0, 100.0),
            percent(concat!($pre, "v4"), 0.0, 100.0),
            percent(concat!($pre, "v5"), 0.0, 100.0),
            percent(concat!($pre, "v6"), 0.0, 100.0),
            percent(concat!($pre, "v7"), 0.0, 100.0),
            percent(concat!($pre, "v8"), 0.0, 100.0),
            percent(concat!($pre, "gate_len"), 1.0, 50.0),
            // CLOCK, LENGTH.
            stepped(concat!($pre, "gate_mode"), 1.0, 0.0),
            percent(concat!($pre, "r1"), 0.0, 100.0),
            percent(concat!($pre, "r2"), 0.0, 100.0),
            percent(concat!($pre, "r3"), 0.0, 100.0),
            percent(concat!($pre, "r4"), 0.0, 100.0),
            percent(concat!($pre, "r5"), 0.0, 100.0),
            percent(concat!($pre, "r6"), 0.0, 100.0),
            percent(concat!($pre, "r7"), 0.0, 100.0),
            percent(concat!($pre, "r8"), 0.0, 100.0),
        ]
    };
}

/// Params per bank, in slot order.
pub const SLOTS: usize = 35;
const S_GATE: usize = STEPS;
const S_LENGTH: usize = 2 * STEPS;
const S_VEL: usize = 2 * STEPS + 1;
const S_GATE_LEN: usize = 3 * STEPS + 1;
const S_GATE_MODE: usize = 3 * STEPS + 2;
const S_PROB: usize = 3 * STEPS + 3;

const A: [ParamInfo; SLOTS] = bank!("");
const B: [ParamInfo; SLOTS] = bank!("b.");
const C: [ParamInfo; SLOTS] = bank!("c.");
const D: [ParamInfo; SLOTS] = bank!("d.");

const TRANSPOSE_PARAM: usize = S_LENGTH + 1;
const DIRECTION_PARAM: usize = SLOTS + 1;
const BANK_PARAM: usize = SLOTS + 2;
const BANK_B: usize = SLOTS + 3;

/// Bank A's first 28 params keep their historic order and indices (`p1..p8`, `g1..g8`,
/// `length`, `transpose`, `v1..v8`, `gate_len`, `gate_mode`); then `r1..r8`, `direction`,
/// `bank`, and banks B, C, D in slot order. `param_index` relies on this layout.
const PARAMS: [ParamInfo; SLOTS + 3 + 3 * SLOTS] = {
    let mut out = [A[0]; SLOTS + 3 + 3 * SLOTS];
    let mut i = 0;
    while i < SLOTS {
        out[if i <= S_LENGTH { i } else { i + 1 }] = A[i];
        out[BANK_B + i] = B[i];
        out[BANK_B + SLOTS + i] = C[i];
        out[BANK_B + 2 * SLOTS + i] = D[i];
        i += 1;
    }
    out[TRANSPOSE_PARAM] = pitch("transpose", 0.0);
    // FWD, REV, PEND.
    out[DIRECTION_PARAM] = stepped("direction", 2.0, 0.0);
    out[BANK_PARAM] = stepped("bank", 3.0, 0.0);
    out
};

/// Index in `SEQ_INFO.params` of `slot` in `bank`.
pub const fn param_index(bank: usize, slot: usize) -> usize {
    if bank == 0 {
        if slot <= S_LENGTH {
            slot
        } else {
            slot + 1
        }
    } else {
        BANK_B + (bank - 1) * SLOTS + slot
    }
}

/// `(bank, slot)` of a bank param name (`"p3"` → A, `"c.v2"` → C), `None` for module-wide ones.
pub fn bank_of(name: &str) -> Option<(usize, usize)> {
    let (bank, bare) = match name.split_once('.') {
        Some((b, rest)) => (["b", "c", "d"].iter().position(|&x| x == b)? + 1, rest),
        None => (0, name),
    };
    A.iter()
        .position(|p| p.name == bare)
        .map(|slot| (bank, slot))
}

/// The same slot's param name in `bank`.
pub fn bank_param(bank: usize, slot: usize) -> &'static str {
    PARAMS[param_index(bank, slot)].name
}

/// The bank-A name of a bank param's slot (`"c.v2"` → `"v2"`); other names unchanged.
pub fn slot_name(name: &str) -> &str {
    match bank_of(name) {
        Some((_, slot)) => A[slot].name,
        None => name,
    }
}

/// What a cleared bank holds, per slot: every step off at 0 st, 100 % velocity and
/// probability, the default length and gate settings.
pub fn cleared(slot: usize) -> f32 {
    match slot {
        s if s < STEPS => 0.0,
        s if s < S_LENGTH => 0.0,
        s => A[s].default,
    }
}

const ADVANCED: &[&str] = &{
    // Everything but bank A's pitches and gates; the face shows the edit bank's in their place.
    let mut out = [""; 3 * SLOTS + 3 + SLOTS - 2 * STEPS];
    let mut n = 0;
    let mut i = 0;
    while i < PARAMS.len() {
        if i >= S_LENGTH {
            out[n] = PARAMS[i].name;
            n += 1;
        }
        i += 1;
    }
    out
};

pub static SEQ_INFO: ModuleInfo = ModuleInfo {
    kind: "seq",
    name: "Sequencer",
    category: Category::Sequencer,
    rate: Rate::Global,
    explain: "Plays a looping pattern of 8 notes, one step per clock tick; switch steps off for \
              rests. Four banks hold four patterns; launch one to switch on the beat.",
    lesson: None,
    requires: &[],
    ports: PORTS,
    params: &PARAMS,
    quality: QualitySupport {
        oversampling: false,
        anti_aliasing: false,
        interpolation: false,
    },
    skin: None,
    width_units: 22,
    advanced: ADVANCED,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Forward,
    Reverse,
    Pendulum,
}

impl Direction {
    fn from_param(v: f32) -> Self {
        match v.round() as i32 {
            1 => Direction::Reverse,
            2 => Direction::Pendulum,
            _ => Direction::Forward,
        }
    }
}

/// The step after `step` in a loop of `len` steps, and the new pendulum heading (`up`).
pub fn next_step(step: usize, up: bool, len: usize, dir: Direction) -> (usize, bool) {
    if len <= 1 {
        return (0, true);
    }
    match dir {
        Direction::Forward => (if step + 1 < len { step + 1 } else { 0 }, true),
        Direction::Reverse => (
            if step == 0 || step >= len {
                len - 1
            } else {
                step - 1
            },
            false,
        ),
        Direction::Pendulum => {
            let step = step.min(len - 1);
            if up && step + 1 < len {
                (step + 1, true)
            } else if up || step > 0 {
                // Turning at the top, or on the way down.
                if step == 0 {
                    (1, true)
                } else {
                    (step - 1, false)
                }
            } else {
                (1, true)
            }
        }
    }
}

/// The first step a (re)started pattern plays.
pub fn start_step(len: usize, dir: Direction) -> (usize, bool) {
    match dir {
        Direction::Reverse => (len.max(1) - 1, false),
        _ => (0, true),
    }
}

const NONE: u8 = u8::MAX;

/// Everything `process` carries from sample to sample.
#[derive(Clone, Copy)]
struct St {
    step: usize,
    /// Pendulum heading.
    up: bool,
    bank: u8,
    /// The startup bank has been read (a fresh instance reads it on its first block).
    started: bool,
    /// The next edge plays the start step (after a load, a reset between pulses).
    restart: bool,
    /// Bank to switch to on the first clock edge at sample `armed_from` or later; `NONE`.
    armed: u8,
    armed_from: u32,
    /// The current step passed its probability draw.
    play: bool,
    rng: u32,
    seed: u32,
    clock_high: bool,
    reset_high: bool,
    gate_high: bool,
    seen_edge: bool,
    /// Samples since the last rising clock edge.
    since: u32,
    /// Accepted step period in samples; 0 = none yet.
    period: f32,
    /// The last measured interval, accepted or not; 0 = none.
    cand: f32,
}

pub struct Seq {
    s: St,
}

fn xorshift(x: &mut u32) -> u32 {
    *x ^= *x << 13;
    *x ^= *x >> 17;
    *x ^= *x << 5;
    *x
}

/// A nonzero xorshift state from any seed.
fn scramble(seed: u32) -> u32 {
    let x = seed.wrapping_mul(0x9E37_79B9) ^ 0x6A09_E667;
    if x == 0 {
        1
    } else {
        x
    }
}

impl Seq {
    pub fn new() -> Self {
        Seq {
            s: St {
                // Parked on the last step (the next edge plays the start step).
                step: STEPS - 1,
                up: true,
                bank: 0,
                started: false,
                restart: true,
                armed: NONE,
                armed_from: 0,
                play: true,
                rng: scramble(0),
                seed: 0,
                clock_high: false,
                reset_high: false,
                gate_high: false,
                seen_edge: false,
                since: 0,
                period: 0.0,
                cand: 0.0,
            },
        }
    }

    /// The step playing now, 0-based (for the UI's step light).
    pub fn step(&self) -> usize {
        self.s.step
    }

    /// The playing bank, 0 = A.
    pub fn bank(&self) -> usize {
        self.s.bank as usize
    }

    /// The bank armed to start on the next clock edge.
    pub fn armed(&self) -> Option<usize> {
        (self.s.armed != NONE).then_some(self.s.armed as usize)
    }

    /// Audio thread: switch to `bank` on the first clock edge at sample offset `from` of the
    /// next block processed, or later. Replaces an earlier arm. No allocation.
    pub fn arm(&mut self, bank: usize, from: usize) {
        self.s.armed = bank.min(BANKS - 1) as u8;
        self.s.armed_from = from as u32;
    }

    pub fn cancel(&mut self) {
        self.s.armed = NONE;
    }

    /// Seeds the probability stream (the compiler passes the module id). Only a fresh instance
    /// uses it; carried state keeps its stream.
    pub fn seed(&mut self, seed: u64) {
        self.s.seed = (seed ^ (seed >> 32)) as u32;
        self.s.rng = scramble(self.s.seed);
    }
}

impl Default for Seq {
    fn default() -> Self {
        Self::new()
    }
}

/// An interval between rising clock edges becomes the period when it agrees within 10 % with
/// the interval before it (or when there is no period yet), the delay's sync rule.
fn accept(s: &mut St, interval: f32) {
    if s.period == 0.0 || (interval - s.cand).abs() <= 0.1 * s.cand {
        s.period = interval;
    }
    s.cand = interval;
}

/// One bank's values for a block.
#[derive(Clone, Copy)]
struct Bank {
    pitches: [f32; STEPS],
    gates: [bool; STEPS],
    velocities: [f32; STEPS],
    probs: [f32; STEPS],
    length: usize,
    timed: bool,
    gate_len: f32,
}

impl Module for Seq {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &SEQ_INFO
    }

    fn prepare(&mut self, _sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {}

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let transpose = io.param(TRANSPOSE_PARAM).at(0).round();
        let dir = Direction::from_param(io.param(DIRECTION_PARAM).at(0));
        let p = |io: &ProcessIo, b: usize, slot: usize| io.param(param_index(b, slot)).at(0);
        let banks: [Bank; BANKS] = std::array::from_fn(|b| Bank {
            pitches: std::array::from_fn(|k| p(io, b, k).round() + transpose),
            gates: std::array::from_fn(|k| p(io, b, S_GATE + k) >= 0.5),
            velocities: std::array::from_fn(|k| (p(io, b, S_VEL + k) / 100.0).clamp(0.0, 1.0)),
            probs: std::array::from_fn(|k| p(io, b, S_PROB + k)),
            length: (p(io, b, S_LENGTH).round() as usize).clamp(1, STEPS),
            timed: p(io, b, S_GATE_MODE) >= 0.5,
            gate_len: (p(io, b, S_GATE_LEN) / 100.0).clamp(0.01, 1.0),
        });
        if !self.s.started {
            self.s.started = true;
            self.s.bank = (io.param(BANK_PARAM).at(0).round() as usize).min(BANKS - 1) as u8;
        }
        let (clock, reset) = (io.input(0), io.input(1));
        let n = io.block_len();
        // Walks the inputs from the stored state, yielding the state after each sample. Run once
        // per output: `ProcessIo` lends one output buffer at a time.
        let start = self.s;
        let walk = move || {
            (0..n).scan(start, move |s, i| {
                let c = clock.at(i) > 0.0;
                let edge = c && !s.clock_high;
                s.since = s.since.saturating_add(1);
                if edge {
                    let launch = s.armed != NONE && i as u32 >= s.armed_from;
                    if launch {
                        (s.bank, s.armed, s.restart) = (s.armed, NONE, true);
                    }
                    let len = banks[s.bank as usize].length;
                    (s.step, s.up) = if s.restart {
                        start_step(len, dir)
                    } else {
                        next_step(s.step, s.up, len, dir)
                    };
                    s.restart = false;
                    let draw = (xorshift(&mut s.rng) >> 8) as f32 / (1u32 << 24) as f32 * 100.0;
                    s.play = draw < banks[s.bank as usize].probs[s.step];
                    if s.seen_edge {
                        accept(s, s.since as f32);
                    }
                    (s.seen_edge, s.since) = (true, 0);
                }
                // During a clock pulse, reset restarts that pulse's note on the start step (two
                // clocks rarely tick on the same sample); between pulses it arms the start step
                // for the next tick, parked on the step before it.
                let r = reset.at(i) > 0.0;
                if r && !s.reset_high {
                    let len = banks[s.bank as usize].length;
                    s.rng = scramble(s.seed);
                    if c {
                        (s.step, s.up) = start_step(len, dir);
                        s.play = true;
                        s.restart = false;
                    } else {
                        s.step = if dir == Direction::Reverse {
                            0
                        } else {
                            len - 1
                        };
                        s.restart = true;
                    }
                }
                let b = &banks[s.bank as usize];
                let on = b.gates[s.step] && s.play;
                s.gate_high = if b.timed && s.period > 0.0 {
                    let len = (b.gate_len * s.period).max(2.0);
                    on && (s.since as f32) < len && !(edge && s.gate_high)
                } else {
                    on && c
                };
                (s.clock_high, s.reset_high) = (c, r);
                Some(*s)
            })
        };
        for (g, s) in io.output(0)[..n].iter_mut().zip(walk()) {
            *g = s.gate_high as u8 as f32;
        }
        for (p, s) in io.output(1)[..n].iter_mut().zip(walk()) {
            *p = banks[s.bank as usize].pitches[s.step];
        }
        for (v, s) in io.output(2)[..n].iter_mut().zip(walk()) {
            *v = banks[s.bank as usize].velocities[s.step];
            self.s = s;
        }
        self.s.armed_from = 0;
    }

    fn reset(&mut self) {
        let seed = self.s.seed;
        *self = Seq::new();
        self.seed(seed as u64);
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        let s = &self.s;
        out.write_f32("step", s.step as f32);
        out.write_f32("bank", s.bank as f32);
        for (k, v) in [
            ("clock_high", s.clock_high),
            ("reset_high", s.reset_high),
            ("gate_high", s.gate_high),
            ("seen_edge", s.seen_edge),
        ] {
            out.write_f32(k, v as u8 as f32);
        }
        // f32 holds whole sample counts exactly up to 2^24 (~5.8 min at 48 kHz); beyond that
        // only "a long time ago" matters.
        out.write_f32("since", s.since as f32);
        out.write_f32("period", s.period);
        out.write_f32("cand", s.cand);
    }

    fn load_state(&mut self, r: &dyn StateReader) {
        let s = &mut self.s;
        if let Some(v) = r.read_f32("step") {
            s.step = (v as usize).min(STEPS - 1);
            s.restart = false;
        }
        if let Some(v) = r.read_f32("bank") {
            (s.bank, s.started) = ((v as usize).min(BANKS - 1) as u8, true);
        }
        for (k, v) in [
            ("clock_high", &mut s.clock_high),
            ("reset_high", &mut s.reset_high),
            ("gate_high", &mut s.gate_high),
            ("seen_edge", &mut s.seen_edge),
        ] {
            if let Some(x) = r.read_f32(k) {
                *v = x >= 0.5;
            }
        }
        if let Some(v) = r.read_f32("since") {
            s.since = v as u32;
        }
        if let Some(v) = r.read_f32("period") {
            s.period = v;
        }
        if let Some(v) = r.read_f32("cand") {
            s.cand = v;
        }
    }

    /// The whole state (arm, heading, probability stream) rides along exactly.
    fn carry_from(&mut self, old: &dyn Module) {
        if let Some(o) = old.as_any().downcast_ref::<Seq>() {
            self.s = o.s;
        }
    }
}
