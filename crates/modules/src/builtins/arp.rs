//! `arp`: the arpeggiator. The keys you hold come in from the keyboard (the engine passes every
//! key event to every `arp`, `note_on` / `note_off` / `sustain` / `clear`); each rising edge on
//! `clock` plays the next note of them. `pitch` (whole semitones, the same reference as
//! `midi.in`), `velocity` (the key's) and `gate` are plain signals, so it patches into an
//! envelope and oscillator exactly as `seq` does, and into `quantizer`, `logic`, `slew` and
//! `random` after it. It is a global module like `seq`: every voice of a voice-rate chain it
//! feeds plays the same note. `midi.in` itself is not touched; to keep a held chord sounding
//! as well, patch both.
//!
//! Modes (`mode`): UP, DOWN, UPDOWN (the end notes play once per turn), PLAYED (the order the
//! keys went down) and RANDOM. `octaves` (1..4) repeats the chord that many octaves up; UP and
//! DOWN go through the whole span, PLAYED plays the chord in press order in each octave in turn.
//!
//! What it does when the keys change:
//! - One key: that note repeats on every clock edge (through its octaves).
//! - A key added mid-pattern joins at its place in the order: UP, DOWN and UPDOWN carry on from
//!   the pitch last played, so a note above it is reached in this pass and a note below it in the
//!   next; PLAYED appends it to the end of the chord.
//! - A key let go leaves the pattern at once, the rest carries on. When the last one is let go
//!   the gate falls at once (no note rings on) and the pattern starts over with the next chord;
//!   the next note after a silence plays on the next clock edge, never between edges.
//! - `latch` (HOLD): the chord stays after release. Pressing a key when nothing is physically
//!   down starts a new chord; pressing while others are down adds to it. The sustain pedal does
//!   the same job per press: keys let go while it is down stay in until it comes up.
//! - A key event lands at the start of the block it arrives in, and the pattern takes it at the
//!   next clock edge.
//!
//! `gate` is high for `gate_len` % of the step, which is as long as the clock interval two edges
//! back (the position in a swung pair, so a swung clock gives each note its own length); the first
//! step after a load follows the clock. 100 % still drops the gate for a sample so the next note
//! retriggers. `ratchet` (1..4) plays that many evenly spaced notes in each step. A rising
//! `reset` starts the pattern over on its first note and reseeds RANDOM; the stream otherwise
//! is seeded from the module id, so a render repeats exactly.

use crate::builtins::seq::{ratchet_gate, step_len};
use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};

/// Most keys held at once; a seventeenth press is ignored.
pub const MAX_NOTES: usize = 16;
const MAX_OCTAVES: usize = 4;
const MAX_CANDIDATES: usize = MAX_NOTES * MAX_OCTAVES;
/// MIDI note that plays semitone 0 (the engine's `BASE_NOTE`).
const BASE_NOTE: i32 = 60;

const DOWN: u8 = 1;
const UPDOWN: u8 = 2;
const PLAYED: u8 = 3;
const RANDOM: u8 = 4;

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

const fn stepped(name: &'static str, min: f32, max: f32, default: f32) -> ParamInfo {
    ParamInfo {
        name,
        min,
        max,
        default,
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    }
}

const PARAMS: &[ParamInfo] = &[
    // UP, DOWN, UPDOWN, PLAYED, RANDOM.
    stepped("mode", 0.0, 4.0, 0.0),
    stepped("octaves", 1.0, MAX_OCTAVES as f32, 1.0),
    ParamInfo {
        name: "gate_len",
        min: 1.0,
        max: 100.0,
        default: 50.0,
        unit: "%",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
    // OFF, ON.
    stepped("latch", 0.0, 1.0, 0.0),
    stepped("ratchet", 1.0, 4.0, 1.0),
];

pub static ARP_INFO: ModuleInfo = ModuleInfo {
    kind: "arp",
    name: "Arpeggiator",
    category: Category::Sequencer,
    rate: Rate::Global,
    explain: "Plays the keys you hold one at a time, a note per clock tick, up, down, back and \
              forth, in the order pressed or at random, across up to four octaves. Latch keeps \
              the chord after you let go; ratchet repeats each note.",
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
    advanced: &[],
};

/// The held keys and the pedal: what the keyboard has told the arpeggiator.
#[derive(Clone, Copy)]
struct Held {
    /// `(note, velocity)` in press order.
    notes: [(u8, u8); MAX_NOTES],
    n: usize,
    /// Keys physically down, bit n = note n.
    down: u128,
    /// Keys let go while the pedal was down.
    sus: u128,
    pedal: bool,
    /// `latch` as of the last block.
    latch: bool,
}

fn bit(note: u8) -> u128 {
    1u128 << (note & 0x7F)
}

impl Held {
    fn find(&self, note: u8) -> Option<usize> {
        self.notes[..self.n].iter().position(|&(x, _)| x == note)
    }

    fn remove(&mut self, i: usize) {
        self.notes.copy_within(i + 1..self.n, i);
        self.n -= 1;
    }

    fn note_on(&mut self, note: u8, velocity: u8) {
        let note = note & 0x7F;
        if self.latch && self.down == 0 {
            // A new chord replaces the latched one.
            self.n = 0;
            self.sus = 0;
        }
        self.down |= bit(note);
        self.sus &= !bit(note);
        match self.find(note) {
            Some(i) => self.notes[i].1 = velocity,
            None if self.n < MAX_NOTES => {
                self.notes[self.n] = (note, velocity);
                self.n += 1;
            }
            None => {}
        }
    }

    fn note_off(&mut self, note: u8) {
        let note = note & 0x7F;
        self.down &= !bit(note);
        if self.pedal {
            self.sus |= bit(note);
        } else if !self.latch {
            if let Some(i) = self.find(note) {
                self.remove(i);
            }
        }
    }

    /// Drops every note that is neither down nor held by the pedal.
    fn release_unheld(&mut self) {
        let keep = self.down | self.sus;
        let mut i = 0;
        while i < self.n {
            if keep & bit(self.notes[i].0) == 0 {
                self.remove(i);
            } else {
                i += 1;
            }
        }
    }
}

/// Where the pattern stands.
#[derive(Clone, Copy)]
struct Pattern {
    /// Last pitch played, in semitones above `BASE_NOTE` (UP, DOWN, UPDOWN); `None` = start.
    last: Option<i32>,
    /// UPDOWN heading.
    up: bool,
    /// PLAYED: key index and octave.
    k: usize,
    o: usize,
}

impl Pattern {
    const START: Pattern = Pattern {
        last: None,
        up: true,
        k: 0,
        o: 0,
    };
}

/// Every pitch of the chord over `octaves`, ascending, equal pitches once, with the velocity
/// of the key that makes it (the lower octave's if two keys land on one pitch).
fn candidates(h: &Held, octaves: usize, out: &mut [(i32, u8); MAX_CANDIDATES]) -> usize {
    let mut n = 0;
    for o in 0..octaves {
        for &(note, vel) in &h.notes[..h.n] {
            let p = note as i32 - BASE_NOTE + 12 * o as i32;
            if !out[..n].iter().any(|&(q, _)| q == p) {
                out[n] = (p, vel);
                n += 1;
            }
        }
    }
    out[..n].sort_unstable_by_key(|&(p, _)| p);
    n
}

/// The next `(pitch, velocity)` of the pattern after `pat`, which it advances. `draw` is a
/// uniform number in 0..1 (RANDOM). `None` when no key is held.
fn next(h: &Held, mode: u8, octaves: usize, pat: &mut Pattern, draw: f32) -> Option<(i32, u8)> {
    if h.n == 0 {
        return None;
    }
    if mode == PLAYED {
        // Press order within each octave; an octave change or a removal can leave `k` past the
        // end, which wraps to the next octave.
        let (mut k, mut o) = match pat.last {
            None => (0, 0),
            Some(_) => (pat.k + 1, pat.o),
        };
        if k >= h.n {
            (k, o) = (0, o + 1);
        }
        if o >= octaves {
            o = 0;
        }
        (pat.k, pat.o, pat.last) = (k, o, Some(0));
        let (note, vel) = h.notes[k];
        return Some((note as i32 - BASE_NOTE + 12 * o as i32, vel));
    }
    let mut c = [(0, 0); MAX_CANDIDATES];
    let n = candidates(h, octaves, &mut c);
    let c = &c[..n];
    let above = |p: i32| c.iter().find(|&&(q, _)| q > p).copied();
    let below = |p: i32| c.iter().rev().find(|&&(q, _)| q < p).copied();
    let pick = match (mode, pat.last) {
        (RANDOM, _) => Some(c[((draw * n as f32) as usize).min(n - 1)]),
        (DOWN, None) => c.last().copied(),
        (DOWN, Some(p)) => below(p).or(c.last().copied()),
        (_, None) => {
            pat.up = true;
            c.first().copied()
        }
        (UPDOWN, Some(p)) => {
            let (fwd, back) = if pat.up {
                (above(p), below(p))
            } else {
                (below(p), above(p))
            };
            fwd.or_else(|| {
                pat.up = !pat.up;
                back
            })
            .or_else(|| c.iter().find(|&&(q, _)| q == p).copied())
        }
        (_, Some(p)) => above(p).or(c.first().copied()),
    };
    // The chord can have changed since `last`; a missing pick falls back to the first note.
    let (p, v) = pick.unwrap_or(c[0]);
    pat.last = Some(p);
    Some((p, v))
}

/// Everything `process` carries from sample to sample.
#[derive(Clone, Copy)]
struct St {
    pat: Pattern,
    /// The next clock edge starts the pattern over.
    restart: bool,
    /// A note is playing this step.
    play: bool,
    pitch: f32,
    velocity: f32,
    rng: u32,
    seed: u32,
    clock_high: bool,
    reset_high: bool,
    gate_high: bool,
    seen_edge: bool,
    since: u32,
    /// The last two clock intervals, samples; 0 = unknown.
    iv: [f32; 2],
}

pub struct Arp {
    s: St,
    held: Held,
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

impl Arp {
    pub fn new() -> Self {
        Arp {
            s: St {
                pat: Pattern::START,
                restart: true,
                play: false,
                pitch: 0.0,
                velocity: 0.0,
                rng: scramble(0),
                seed: 0,
                clock_high: false,
                reset_high: false,
                gate_high: false,
                seen_edge: false,
                since: 0,
                iv: [0.0; 2],
            },
            held: Held {
                notes: [(0, 0); MAX_NOTES],
                n: 0,
                down: 0,
                sus: 0,
                pedal: false,
                latch: false,
            },
        }
    }

    /// Seeds the RANDOM stream (the compiler passes the module id). Only a fresh instance uses
    /// it; carried state keeps its stream.
    pub fn seed(&mut self, seed: u64) {
        self.s.seed = (seed ^ (seed >> 32)) as u32;
        self.s.rng = scramble(self.s.seed);
    }

    /// Audio thread: a key went down. No allocation.
    pub fn note_on(&mut self, note: u8, velocity: u8) {
        self.held.note_on(note, velocity);
    }

    /// Audio thread: a key came up (the pedal may keep it). No allocation.
    pub fn note_off(&mut self, note: u8) {
        self.held.note_off(note);
    }

    /// Audio thread: the sustain pedal went down or up. No allocation.
    pub fn sustain(&mut self, down: bool) {
        self.held.pedal = down;
        if !down {
            self.held.sus = 0;
            if !self.held.latch {
                self.held.release_unheld();
            }
        }
    }

    /// Audio thread: forget every key, the pedal and the latch (panic, notes off).
    pub fn clear(&mut self) {
        (self.held.n, self.held.down, self.held.sus, self.held.pedal) = (0, 0, 0, false);
    }

    /// The held notes in press order (for the UI and tests).
    pub fn held(&self) -> impl Iterator<Item = u8> + '_ {
        self.held.notes[..self.held.n].iter().map(|&(n, _)| n)
    }
}

impl Default for Arp {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Arp {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &ARP_INFO
    }

    fn prepare(&mut self, _sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {}

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let mode = io.param(0).at(0).round().clamp(0.0, 4.0) as u8;
        let octaves = (io.param(1).at(0).round() as usize).clamp(1, MAX_OCTAVES);
        let width = (io.param(2).at(0) / 100.0).clamp(0.01, 1.0);
        let latch = io.param(3).at(0) >= 0.5;
        let ratchets = io.param(4).at(0).round().clamp(1.0, 4.0);
        if latch != self.held.latch {
            self.held.latch = latch;
            if !latch {
                self.held.release_unheld();
            }
        }
        let held = self.held;
        let (clock, reset) = (io.input(0), io.input(1));
        let n = io.block_len();
        let start = self.s;
        let walk = move || {
            (0..n).scan(start, move |s, i| {
                let c = clock.at(i) > 0.0;
                let edge = c && !s.clock_high;
                s.since = s.since.saturating_add(1);
                // Pitch and velocity change only on the edge, in the sample the edge is seen.
                let step = |s: &mut St, held: &Held| {
                    if s.restart {
                        s.pat = Pattern::START;
                        s.restart = false;
                    }
                    let draw = (xorshift(&mut s.rng) >> 8) as f32 / (1u32 << 24) as f32;
                    match next(held, mode, octaves, &mut s.pat, draw) {
                        Some((p, v)) => {
                            (s.pitch, s.velocity, s.play) = (p as f32, v as f32 / 127.0, true)
                        }
                        None => {
                            s.play = false;
                            s.restart = true;
                        }
                    }
                };
                if edge {
                    step(s, &held);
                    if s.seen_edge {
                        s.iv = [s.since as f32, s.iv[0]];
                    }
                    (s.seen_edge, s.since) = (true, 0);
                }
                let r = reset.at(i) > 0.0;
                if r && !s.reset_high {
                    s.rng = scramble(s.seed);
                    s.restart = true;
                    if c {
                        // Mid-pulse: this pulse's note restarts on the first note.
                        step(s, &held);
                    }
                }
                // Keys let go between edges silence the note at once; none held, no note.
                if held.n == 0 {
                    (s.play, s.restart) = (false, true);
                }
                let on = s.play;
                let len = step_len(&s.iv);
                s.gate_high = if len > 0.0 {
                    on && ratchet_gate(s.since as f32, len, ratchets, width)
                        && !(edge && s.gate_high)
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
            *p = s.pitch;
        }
        for (v, s) in io.output(2)[..n].iter_mut().zip(walk()) {
            *v = s.velocity;
            self.s = s;
        }
    }

    fn reset(&mut self) {
        let (seed, held) = (self.s.seed, self.held);
        *self = Arp::new();
        self.seed(seed as u64);
        self.held = held;
    }

    /// The held keys are the player's, not the patch's: they are not saved.
    fn save_state(&self, _out: &mut dyn StateWriter) {}

    fn load_state(&mut self, _s: &dyn StateReader) {}

    /// The whole state (keys, pattern position, random stream) rides along exactly.
    fn carry_from(&mut self, old: &dyn Module) {
        if let Some(o) = old.as_any().downcast_ref::<Arp>() {
            self.s = o.s;
            self.held = o.held;
        }
    }
}
