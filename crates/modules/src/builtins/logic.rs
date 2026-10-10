//! `logic` and `comparator`: gates from gates, and gates from levels.
//!
//! `logic` has inputs `a` and `b` and four outputs at once: `and` (both high), `or` (either),
//! `xor` (exactly one) and `not` (the opposite of `a`). A gate is high at 0.5 or more, so any
//! gate, clock or comparator output works, and the outputs are exactly 0 or 1. Two clocks into
//! `and` give a rhythm that only fires where they coincide; into `xor`, where they differ.
//!
//! `comparator` makes a gate from a level: high while `in` is above `threshold`. `hysteresis`
//! is a dead band around the threshold (the output switches high above `threshold + h/2` and
//! low below `threshold - h/2`), so a slow or noisy signal does not chatter. Starts low.
//!
//! Both follow their inputs: one instance per voice behind a `midi.in`, otherwise one.

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};

const NO_QUALITY: QualitySupport = QualitySupport {
    oversampling: false,
    anti_aliasing: false,
    interpolation: false,
};

const LOGIC_PORTS: &[PortInfo] = &[
    PortInfo {
        name: "a",
        port_type: PortType::Gate,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "b",
        port_type: PortType::Gate,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "and",
        port_type: PortType::Gate,
        direction: PortDirection::Output,
    },
    PortInfo {
        name: "or",
        port_type: PortType::Gate,
        direction: PortDirection::Output,
    },
    PortInfo {
        name: "xor",
        port_type: PortType::Gate,
        direction: PortDirection::Output,
    },
    PortInfo {
        name: "not",
        port_type: PortType::Gate,
        direction: PortDirection::Output,
    },
];

pub static LOGIC_INFO: ModuleInfo = ModuleInfo {
    kind: "logic",
    name: "Logic",
    category: Category::Utility,
    rate: Rate::Voice,
    explain: "Combines two gates: and, or, xor, and the opposite of the first.",
    lesson: None,
    requires: &[],
    ports: LOGIC_PORTS,
    params: &[],
    quality: NO_QUALITY,
    skin: None,
    width_units: 6,
    advanced: &[],
};

#[derive(Default)]
pub struct Logic;

impl Logic {
    pub fn new() -> Self {
        Logic
    }
}

impl Module for Logic {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &LOGIC_INFO
    }

    fn prepare(&mut self, _sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {}

    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let (a, b) = (io.input(0), io.input(1));
        for i in 0..n {
            let (x, y) = (a.at(i) >= 0.5, b.at(i) >= 0.5);
            let f = |v: bool| v as u8 as f32;
            io.output(0)[i] = f(x && y);
            io.output(1)[i] = f(x || y);
            io.output(2)[i] = f(x != y);
            io.output(3)[i] = f(!x);
        }
    }

    fn reset(&mut self) {}
}

const COMPARATOR_PORTS: &[PortInfo] = &[
    PortInfo {
        name: "in",
        port_type: PortType::Cv,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "out",
        port_type: PortType::Gate,
        direction: PortDirection::Output,
    },
];

const COMPARATOR_PARAMS: &[ParamInfo] = &[
    ParamInfo {
        name: "threshold",
        min: -1.0,
        max: 1.0,
        default: 0.0,
        unit: "",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "hysteresis",
        min: 0.0,
        max: 1.0,
        default: 0.02,
        unit: "",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
];

pub static COMPARATOR_INFO: ModuleInfo = ModuleInfo {
    kind: "comparator",
    name: "Comparator",
    category: Category::Utility,
    rate: Rate::Voice,
    explain: "Turns a level into a gate: high while the signal is above the threshold.",
    lesson: None,
    requires: &[],
    ports: COMPARATOR_PORTS,
    params: COMPARATOR_PARAMS,
    quality: NO_QUALITY,
    skin: None,
    width_units: 5,
    advanced: &["hysteresis"],
};

#[derive(Default)]
pub struct Comparator {
    high: bool,
}

impl Comparator {
    pub fn new() -> Self {
        Comparator { high: false }
    }
}

impl Module for Comparator {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &COMPARATOR_INFO
    }

    fn prepare(&mut self, _sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {}

    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let (input, thr, hyst) = (io.input(0), io.param(0), io.param(1));
        for (i, o) in io.output(0)[..n].iter_mut().enumerate() {
            let (x, t, h) = (input.at(i), thr.at(i), hyst.at(i).max(0.0) * 0.5);
            if x > t + h {
                self.high = true;
            } else if x < t - h {
                self.high = false;
            }
            *o = self.high as u8 as f32;
        }
    }

    fn reset(&mut self) {
        self.high = false;
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        out.write_f32("high", self.high as u8 as f32);
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        self.high = s.read_f32("high").is_some_and(|v| v >= 0.5);
    }
}
