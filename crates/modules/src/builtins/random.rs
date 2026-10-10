//! `random`: a stepped random value that changes on each rising `clock` edge (a gate is high at
//! 0.5 or more) and holds in between. The values are reproducible: the same patch plays the same
//! values every time.
//!
//! - `length` 0 never repeats. 1 to 16 plays a loop of that many values, remembered from the
//!   first pass, so a short random pattern comes round again and again.
//! - `change` (in a loop) is the chance, per step per pass, that the remembered value is
//!   replaced by a new one: 0 keeps the loop exactly, higher lets it evolve slowly.
//! - `bipolar` picks -1..1 or 0..1; `range` multiplies it (set 12 or more to drive a pitch).
//! - A rising `reset` returns to the start: the stream is reseeded and the loop begins again at
//!   its first value, on the next clock. Until the first clock the output is 0.
//!
//! **Random state.** As `noise`: the compiler seeds each instance from the module id and the
//! voice lane (`seed`), so two modules or two voices never play the same values, and a live edit
//! carries the stream on (`carry_from`) instead of reseeding. It is a voice-rate module that
//! follows its clock: a global `clock` or `seq` gives one instance (every voice hears the same
//! value, as a shared melody should); a clock from `midi.in` gives one stream per voice.

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};

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
        name: "out",
        port_type: PortType::Cv,
        direction: PortDirection::Output,
    },
];

const MAX_LENGTH: usize = 16;

const PARAMS: &[ParamInfo] = &[
    ParamInfo {
        name: "length",
        min: 0.0,
        max: MAX_LENGTH as f32,
        default: 0.0,
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "change",
        min: 0.0,
        max: 100.0,
        default: 0.0,
        unit: "%",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
    // UNI, BI.
    ParamInfo {
        name: "bipolar",
        min: 0.0,
        max: 1.0,
        default: 1.0,
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "range",
        min: 0.1,
        max: 60.0,
        default: 1.0,
        unit: "x",
        taper: Taper::Exponential,
        smoothing_ms: 0.0,
    },
];

pub const BIPOLAR_LABELS: [&str; 2] = ["UNI", "BI"];

pub static RANDOM_INFO: ModuleInfo = ModuleInfo {
    kind: "random",
    name: "Random",
    category: Category::Utility,
    rate: Rate::Voice,
    explain: "A new random value on every clock tick, or a short random loop that repeats and can slowly change.",
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
    width_units: 6,
    advanced: &["range"],
};

#[derive(Debug, Clone, Copy)]
struct Core {
    rng: u32,
    /// The loop's remembered values, 0..1; the first `filled` are valid.
    vals: [f32; MAX_LENGTH],
    filled: usize,
    pos: usize,
    /// Last drawn value, 0..1; `drawn` is false until the first clock (the output is 0 then).
    value: f32,
    drawn: bool,
    clock_high: bool,
    reset_high: bool,
}

pub struct Random {
    c: Core,
    /// The seed this instance was given: its identity for `carry_from`.
    seed: u32,
}

/// A nonzero xorshift32 state from any 64-bit seed (splitmix64 finalizer), as `noise`.
fn scramble(seed: u64) -> u32 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z as u32 ^ (z >> 32) as u32).max(1)
}

fn draw(rng: &mut u32) -> f32 {
    *rng ^= *rng << 13;
    *rng ^= *rng >> 17;
    *rng ^= *rng << 5;
    (*rng >> 8) as f32 / 16_777_216.0
}

impl Core {
    fn fresh(rng: u32) -> Core {
        Core {
            rng,
            vals: [0.0; MAX_LENGTH],
            filled: 0,
            pos: 0,
            value: 0.0,
            drawn: false,
            clock_high: false,
            reset_high: false,
        }
    }
}

impl Random {
    pub fn new() -> Self {
        let seed = scramble(0);
        Random {
            c: Core::fresh(seed),
            seed,
        }
    }

    /// Seeds the stream from the module id and voice lane (0 for one instance). Called by the
    /// compiler on a fresh instance, before any state is carried into it.
    pub fn seed(&mut self, module: u64, lane: usize) {
        self.seed = scramble(module.wrapping_mul(0x2_0000_0003) ^ ((lane as u64) << 48) ^ 0x52);
        self.c = Core::fresh(self.seed);
    }
}

impl Default for Random {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Random {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &RANDOM_INFO
    }

    fn prepare(&mut self, _sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {}

    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let (clock, reset) = (io.input(0), io.input(1));
        let length = io.param(0).at(0).round().clamp(0.0, MAX_LENGTH as f32) as usize;
        let change = io.param(1).at(0).clamp(0.0, 100.0) / 100.0;
        let bipolar = io.param(2).at(0) >= 0.5;
        let range = io.param(3).at(0);
        let c = &mut self.c;
        for (i, o) in io.output(0)[..n].iter_mut().enumerate() {
            let (r, k) = (reset.at(i) >= 0.5, clock.at(i) >= 0.5);
            if r && !c.reset_high {
                c.rng = self.seed;
                c.filled = 0;
                c.pos = 0;
            }
            if k && !c.clock_high {
                c.drawn = true;
                if length == 0 {
                    c.value = draw(&mut c.rng);
                } else {
                    let pos = c.pos % length;
                    while c.filled <= pos {
                        c.vals[c.filled] = draw(&mut c.rng);
                        c.filled += 1;
                    }
                    if change > 0.0 && draw(&mut c.rng) < change {
                        c.vals[pos] = draw(&mut c.rng);
                    }
                    c.value = c.vals[pos];
                    c.pos = (pos + 1) % length;
                }
            }
            (c.clock_high, c.reset_high) = (k, r);
            let v = if bipolar {
                c.value * 2.0 - 1.0
            } else {
                c.value
            };
            *o = if c.drawn { v * range } else { 0.0 };
        }
    }

    fn reset(&mut self) {
        self.c = Core::fresh(self.seed);
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        out.write_f32("value", if self.c.drawn { self.c.value } else { -1.0 });
        out.write_f32("pos", self.c.pos as f32);
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        if let Some(v) = s.read_f32("value").filter(|v| (0.0..1.0).contains(v)) {
            (self.c.value, self.c.drawn) = (v, true);
        }
        if let Some(p) = s.read_f32("pos") {
            self.c.pos = (p.max(0.0) as usize).min(MAX_LENGTH - 1);
        }
    }

    fn carry_from(&mut self, old: &dyn Module) {
        if let Some(o) = old.as_any().downcast_ref::<Random>() {
            if o.seed == self.seed {
                self.c = o.c;
            }
        }
    }
}
