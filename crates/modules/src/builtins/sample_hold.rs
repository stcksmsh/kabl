//! `sample.hold`: takes the value of `in` on each rising edge of `clock` and holds it. In
//! TRACK mode it follows `in` while `clock` is high and holds the last value when `clock` goes
//! low (track-and-hold). Feed it noise, an LFO or a random source for stepped values; anything
//! it holds stays exactly as sampled (a pitch stays a pitch). A gate is high at 0.5 or more.
//! Until the first edge the output is 0. Follows its inputs: one instance per voice behind a
//! `midi.in`, otherwise one.

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "in",
        port_type: PortType::Cv,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "clock",
        port_type: PortType::Gate,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "out",
        port_type: PortType::Cv,
        direction: PortDirection::Output,
    },
];

const PARAMS: &[ParamInfo] = &[ParamInfo {
    name: "mode",
    min: 0.0,
    max: 1.0,
    default: 0.0,
    unit: "",
    taper: Taper::Stepped,
    smoothing_ms: 0.0,
}];

pub const MODE_LABELS: [&str; 2] = ["SAMPLE", "TRACK"];

pub static SAMPLE_HOLD_INFO: ModuleInfo = ModuleInfo {
    kind: "sample.hold",
    name: "Sample & Hold",
    category: Category::Utility,
    rate: Rate::Voice,
    explain:
        "Freezes a signal each time the clock ticks: steps out of noise, an LFO or a sequence.",
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
    width_units: 4,
    advanced: &[],
};

pub struct SampleHold {
    held: f32,
    clock_high: bool,
}

impl SampleHold {
    pub fn new() -> Self {
        SampleHold {
            held: 0.0,
            clock_high: false,
        }
    }
}

impl Default for SampleHold {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for SampleHold {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &SAMPLE_HOLD_INFO
    }

    fn prepare(&mut self, _sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {}

    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let (input, clock) = (io.input(0), io.input(1));
        let track = io.param(0).at(0) >= 0.5;
        for (i, o) in io.output(0)[..n].iter_mut().enumerate() {
            let high = clock.at(i) >= 0.5;
            let x = input.at(i);
            if x.is_finite() && high && (track || !self.clock_high) {
                self.held = x;
            }
            self.clock_high = high;
            *o = self.held;
        }
    }

    fn reset(&mut self) {
        *self = SampleHold::new();
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        out.write_f32("held", self.held);
        out.write_f32("high", self.clock_high as u8 as f32);
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        if let Some(v) = s.read_f32("held").filter(|v| v.is_finite()) {
            self.held = v;
        }
        self.clock_high = s.read_f32("high").is_some_and(|v| v >= 0.5);
    }
}
