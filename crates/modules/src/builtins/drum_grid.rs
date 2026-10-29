//! `drum.grid`: a drum-machine grid. Eight rows of up to 16 steps program a whole beat; every
//! row has its own length (polymeter), and four patterns, A to D, are stored in the module.
//! Each rising edge on `clock` plays the next step of every row; a rising edge on `reset` makes
//! the next clock edge play step 1 again. Swing is the clock's: the grid steps on its pulses.
//!
//! **Outputs.** One gate per row, so the whole kit fits the engine's eight outputs per module.
//! A row's gate follows the clock pulse while its step plays, and carries the step's velocity in
//! its level: `0.5 + 0.5 * velocity`. The drum voices read the velocity of a hit from a
//! gate level above the 0.5 threshold when their `velocity` input is unpatched (see
//! `drums.rs`), so a row patches straight to a voice's `gate`. Rows 4 and 5 default to the
//! closed and open hat: patch them to `drum.hat` `closed` and `open` and the choke works.
//!
//! **Steps.** A step is a 5-bit code: 0 is off, otherwise `1 + velocity + 4 * chance` with four
//! velocity levels (`VELOCITY`) and four chances (`CHANCE`). Four steps pack into one param,
//! so a row is `CHUNKS` params and a pattern 32; the engine allows 175 params per module and
//! a param per step would need 512. Params are named `<pattern><row><chunk>` (`a11`, `d84`),
//! lengths `len1..len8`. An f32 holds 20 bits exactly.
//!
//! **Chance.** Every row draws once per clock edge whatever its steps hold (editing a chance
//! never shifts the stream); the draw is seeded from the module id and again on every reset.
//!
//! **Patterns.** The playing pattern is the `pattern` param plus `round(3 * pattern input)`
//! modulo 4, read on every clock edge, so a change launches on the next step. A new module has
//! a basic beat in pattern A and empty B to D.
//!
//! Row `r` plays step `tick % len(r)`, where `tick` counts clock edges since the last reset,
//! the same rule the cable pattern editor uses for its playhead.
//!
//! Global rate, like `clock` and `seq`.

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};

pub const ROWS: usize = 8;
pub const STEPS: usize = 16;
pub const PATTERNS: usize = 4;
pub const PATTERN_NAMES: [&str; PATTERNS] = ["A", "B", "C", "D"];
/// Steps per param.
pub const PER_CHUNK: usize = 4;
pub const CHUNKS: usize = STEPS / PER_CHUNK;
const BITS: u32 = 5;
const LEN_BASE: usize = PATTERNS * ROWS * CHUNKS;
const PATTERN_PARAM: usize = LEN_BASE + ROWS;
/// Clock edges after which every row length divides the tick count (lcm of 1..=16).
const WRAP: i64 = 720_720;

/// Default row names, in output order.
pub const ROW_NAMES: [&str; ROWS] = [
    "kick", "snare", "clap", "chat", "ohat", "tom", "perc1", "perc2",
];
/// Hit velocity of each level.
pub const VELOCITY: [f32; 4] = [0.30, 0.55, 0.78, 1.0];
/// Chance, in percent, of each level.
pub const CHANCE: [u32; 4] = [100, 75, 50, 25];

const fn out(name: &'static str) -> PortInfo {
    PortInfo {
        name,
        port_type: PortType::Gate,
        direction: PortDirection::Output,
        width: 1,
    }
}

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "clock",
        port_type: PortType::Gate,
        direction: PortDirection::Input,
        width: 1,
    },
    PortInfo {
        name: "reset",
        port_type: PortType::Gate,
        direction: PortDirection::Input,
        width: 1,
    },
    PortInfo {
        name: "pattern",
        port_type: PortType::UnipolarCv,
        direction: PortDirection::Input,
        width: 1,
    },
    out(ROW_NAMES[0]),
    out(ROW_NAMES[1]),
    out(ROW_NAMES[2]),
    out(ROW_NAMES[3]),
    out(ROW_NAMES[4]),
    out(ROW_NAMES[5]),
    out(ROW_NAMES[6]),
    out(ROW_NAMES[7]),
];

/// Steps of the default beat, one bit per step, per row (pattern A).
const BEAT: [u16; ROWS] = [
    0b0001_0001_0001_0001,
    0b0001_0000_0001_0000,
    0,
    0b0100_0100_0100_0100,
    0,
    0,
    0,
    0,
];

/// Step code of a full-velocity, certain hit.
const HIT: u32 = 4;

const fn default_chunk(pat: usize, row: usize, chunk: usize) -> f32 {
    if pat != 0 {
        return 0.0;
    }
    let mut v = 0u32;
    let mut k = 0;
    while k < PER_CHUNK {
        if BEAT[row] >> (chunk * PER_CHUNK + k) & 1 == 1 {
            v |= HIT << (BITS as usize * k);
        }
        k += 1;
    }
    v as f32
}

const fn cell(name: &'static str, pat: usize, row: usize, chunk: usize) -> ParamInfo {
    ParamInfo {
        name,
        min: 0.0,
        max: ((1u32 << (BITS as usize * PER_CHUNK)) - 1) as f32,
        default: default_chunk(pat, row, chunk),
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    }
}

macro_rules! row {
    ($p:literal, $pi:expr, $ri:expr, $r:literal) => {
        [
            cell(concat!($p, $r, "1"), $pi, $ri, 0),
            cell(concat!($p, $r, "2"), $pi, $ri, 1),
            cell(concat!($p, $r, "3"), $pi, $ri, 2),
            cell(concat!($p, $r, "4"), $pi, $ri, 3),
        ]
    };
}

macro_rules! pattern {
    ($p:literal, $pi:expr) => {
        [
            row!($p, $pi, 0, "1"),
            row!($p, $pi, 1, "2"),
            row!($p, $pi, 2, "3"),
            row!($p, $pi, 3, "4"),
            row!($p, $pi, 4, "5"),
            row!($p, $pi, 5, "6"),
            row!($p, $pi, 6, "7"),
            row!($p, $pi, 7, "8"),
        ]
    };
}

const fn len(name: &'static str) -> ParamInfo {
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

const PARAMS: [ParamInfo; PATTERN_PARAM + 1] = {
    let pats = [
        pattern!("a", 0),
        pattern!("b", 1),
        pattern!("c", 2),
        pattern!("d", 3),
    ];
    let mut out = [len("pattern"); PATTERN_PARAM + 1];
    let mut i = 0;
    while i < LEN_BASE {
        out[i] = pats[i / (ROWS * CHUNKS)][i / CHUNKS % ROWS][i % CHUNKS];
        i += 1;
    }
    out[LEN_BASE] = len("len1");
    out[LEN_BASE + 1] = len("len2");
    out[LEN_BASE + 2] = len("len3");
    out[LEN_BASE + 3] = len("len4");
    out[LEN_BASE + 4] = len("len5");
    out[LEN_BASE + 5] = len("len6");
    out[LEN_BASE + 6] = len("len7");
    out[LEN_BASE + 7] = len("len8");
    out[PATTERN_PARAM] = ParamInfo {
        name: "pattern",
        min: 0.0,
        max: (PATTERNS - 1) as f32,
        default: 0.0,
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    };
    out
};

/// Index in `DRUM_GRID_INFO.params` of the param holding steps `chunk * 4..` of `row`.
pub const fn cell_index(pattern: usize, row: usize, chunk: usize) -> usize {
    (pattern * ROWS + row) * CHUNKS + chunk
}

/// Index of row `row`'s length param.
pub const fn len_index(row: usize) -> usize {
    LEN_BASE + row
}

/// Index of the `pattern` param.
pub const fn pattern_index() -> usize {
    PATTERN_PARAM
}

/// Step code at `step` (0..16) of a row, from the row's chunk params in order.
pub fn code_in(chunks: [f32; CHUNKS], step: usize) -> u32 {
    let v = chunks[step / PER_CHUNK] as u32;
    v >> (BITS as usize * (step % PER_CHUNK)) & ((1 << BITS) - 1)
}

/// The chunk value after setting the step at `slot` (0..4) in it to `code`.
pub fn with_code(chunk: f32, slot: usize, code: u32) -> f32 {
    let shift = BITS as usize * slot;
    let mask = ((1u32 << BITS) - 1) << shift;
    ((chunk as u32 & !mask) | (code & ((1 << BITS) - 1)) << shift) as f32
}

/// Step code of velocity level `vel` and chance level `chance`, each 0..4.
pub const fn make_code(vel: usize, chance: usize) -> u32 {
    1 + vel as u32 + 4 * chance as u32
}

/// Velocity and chance levels of a nonzero code.
pub const fn levels(code: u32) -> (usize, usize) {
    ((code as usize - 1) % 4, (code as usize - 1) / 4 % 4)
}

pub static DRUM_GRID_INFO: ModuleInfo = ModuleInfo {
    kind: "drum.grid",
    name: "Drum Grid",
    category: Category::Sequencer,
    rate: Rate::Global,
    explain: "Programs a whole beat: eight rows of up to 16 steps, each row a gate for one \
              drum. Steps have a velocity and a chance; rows can differ in length. Four \
              patterns, switchable on the beat.",
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
    advanced: &[],
};

fn xorshift(x: &mut u32) -> u32 {
    *x ^= *x << 13;
    *x ^= *x >> 17;
    *x ^= *x << 5;
    *x
}

fn scramble(seed: u32) -> u32 {
    let x = seed.wrapping_mul(0x9E37_79B9) ^ 0x6A09_E667;
    if x == 0 {
        1
    } else {
        x
    }
}

pub struct DrumGrid {
    /// Clock edges since the last reset, -1 before the first.
    tick: i64,
    clock_high: bool,
    reset_high: bool,
    /// Gate level of each row for the step now playing.
    level: [f32; ROWS],
    rng: u32,
    seed: u32,
    /// Pattern a cue switched to, `NONE` = follow the `pattern` param. A new value of the
    /// param (a tab click, automation) takes the grid back: the hand wins.
    cued: u8,
    /// Cue switch waiting for the first clock edge at or after sample `armed_from` of a block.
    armed: u8,
    armed_from: u32,
    /// The `pattern` param as last seen, to notice a hand edit.
    seen_param: f32,
    /// Pattern playing now, before the pattern input is added (for the cue display).
    playing: u8,
}

const NONE: u8 = u8::MAX;

impl DrumGrid {
    pub fn new() -> Self {
        DrumGrid {
            tick: -1,
            clock_high: false,
            reset_high: false,
            level: [0.0; ROWS],
            rng: scramble(0),
            seed: 0,
            cued: NONE,
            armed: NONE,
            armed_from: 0,
            seen_param: -1.0,
            playing: 0,
        }
    }

    /// Audio thread: switch to pattern `pat` on the first clock edge at or after sample `from`
    /// of the next block processed. Replaces an earlier arm. No allocation.
    pub fn arm(&mut self, pat: usize, from: usize) {
        self.armed = pat.min(PATTERNS - 1) as u8;
        self.armed_from = from as u32;
    }

    pub fn cancel(&mut self) {
        self.armed = NONE;
    }

    pub fn armed(&self) -> Option<usize> {
        (self.armed != NONE).then_some(self.armed as usize)
    }

    /// Pattern playing now (the `pattern` param or the cue's, without the pattern input).
    pub fn pattern(&self) -> usize {
        self.playing as usize
    }

    /// Step within the bar of 16.
    pub fn step(&self) -> usize {
        self.tick.max(0) as usize % STEPS
    }

    /// Seeds the chance stream (the compiler passes the module id). Only a fresh instance
    /// uses it; carried state keeps its stream.
    pub fn seed(&mut self, seed: u64) {
        self.seed = (seed ^ (seed >> 32)) as u32;
        self.rng = scramble(self.seed);
    }

    /// Plays the step at `self.tick` of the pattern `pat` in every row.
    fn fire(&mut self, io: &ProcessIo, pat: usize) {
        for r in 0..ROWS {
            let len = io
                .param(LEN_BASE + r)
                .at(0)
                .round()
                .clamp(1.0, STEPS as f32) as i64;
            let step = (self.tick % len) as usize;
            let mut chunks = [0.0; CHUNKS];
            for (c, v) in chunks.iter_mut().enumerate() {
                *v = io.param(cell_index(pat, r, c)).at(0);
            }
            let code = code_in(chunks, step);
            // One draw per row and edge, used or not.
            let draw = xorshift(&mut self.rng) % 100;
            self.level[r] = if code == 0 {
                0.0
            } else {
                let (v, c) = levels(code);
                if draw < CHANCE[c] {
                    0.5 + 0.5 * VELOCITY[v]
                } else {
                    0.0
                }
            };
        }
    }
}

impl Default for DrumGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for DrumGrid {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn info(&self) -> &'static ModuleInfo {
        &DRUM_GRID_INFO
    }
    fn prepare(&mut self, _sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {}

    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let (clock, reset, patcv) = (io.input(0), io.input(1), io.input(2));
        let param = io.param(PATTERN_PARAM).at(0);
        if param != self.seen_param {
            self.seen_param = param;
            self.cued = NONE;
        }
        let base = param.round().clamp(0.0, 3.0) as usize;
        for i in 0..n {
            let r_high = reset.at(i) > 0.5;
            let r_edge = r_high && !self.reset_high;
            self.reset_high = r_high;
            let c_high = clock.at(i) > 0.5;
            let c_edge = c_high && !self.clock_high;
            self.clock_high = c_high;
            if r_edge {
                self.tick = -1;
                self.rng = scramble(self.seed);
            }
            if c_edge || (r_edge && c_high) {
                self.tick = (self.tick + 1) % WRAP;
                if self.armed != NONE && i as u32 >= self.armed_from {
                    (self.cued, self.armed) = (self.armed, NONE);
                }
                let base = if self.cued == NONE {
                    base
                } else {
                    self.cued as usize
                };
                self.playing = base as u8;
                let pat = (base + (3.0 * patcv.at(i).clamp(0.0, 1.0)).round() as usize) % PATTERNS;
                self.fire(io, pat);
            }
            for r in 0..ROWS {
                io.output(r)[i] = if c_high { self.level[r] } else { 0.0 };
            }
        }
    }

    fn reset(&mut self) {
        self.tick = -1;
        self.level = [0.0; ROWS];
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        out.write_f32("tick", self.tick as f32);
        out.write_f32("cued", self.cued as f32);
        out.write_f32("armed", self.armed as f32);
        out.write_f32("seen", self.seen_param);
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        self.tick = s.read_f32("tick").map_or(-1, |t| t as i64);
        self.cued = s.read_f32("cued").map_or(NONE, |v| v as u8);
        self.armed = s.read_f32("armed").map_or(NONE, |v| v as u8);
        self.seen_param = s.read_f32("seen").unwrap_or(-1.0);
    }
}
