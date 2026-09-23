//! `clock.div`: passes every Nth clock pulse, whole and at its own width. The first pulse after
//! a reset (or after the module starts) passes. A new `div` takes effect on the next rising
//! input edge, which then passes and restarts the count, so a change never cuts or adds a pulse
//! mid-way. A reset and a clock edge on the same sample: the reset goes first, so that pulse
//! passes; that is how `clock`'s Restart lines up direct and divided patterns.

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
        name: "gate",
        port_type: PortType::Gate,
        direction: PortDirection::Output,
    },
];

const PARAMS: &[ParamInfo] = &[ParamInfo {
    name: "div",
    min: 1.0,
    max: 8.0,
    default: 2.0,
    unit: "",
    taper: Taper::Stepped,
    smoothing_ms: 0.0,
}];

pub static CLOCK_DIV_INFO: ModuleInfo = ModuleInfo {
    kind: "clock.div",
    name: "Divider",
    category: Category::Sequencer,
    rate: Rate::Global,
    explain: "Lets every Nth clock tick through, so a sequencer on it plays N times slower.",
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
    width_units: 9,
    advanced: &[],
};

pub struct ClockDiv {
    /// Division in effect; 0 until the first edge commits one.
    div: u32,
    /// Pulses since the last anchor, modulo `div`: 0 means the next pulse passes.
    count: u32,
    clock_high: bool,
    reset_high: bool,
    /// The current input pulse is being passed.
    pass: bool,
}

impl ClockDiv {
    pub fn new() -> Self {
        ClockDiv {
            div: 0,
            count: 0,
            clock_high: false,
            reset_high: false,
            pass: false,
        }
    }
}

impl Default for ClockDiv {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for ClockDiv {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &CLOCK_DIV_INFO
    }

    fn prepare(&mut self, _sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {}

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let want = (io.param(0).at(0).round() as u32).clamp(1, 8);
        let (clock, reset) = (io.input(0), io.input(1));
        let n = io.block_len();
        for (i, o) in io.output(0)[..n].iter_mut().enumerate() {
            let r = reset.at(i) > 0.0;
            if r && !self.reset_high {
                self.count = 0;
            }
            let c = clock.at(i) > 0.0;
            if c && !self.clock_high {
                if want != self.div {
                    (self.div, self.count) = (want, 0);
                }
                self.pass = self.count == 0;
                self.count = (self.count + 1) % self.div;
            }
            (self.clock_high, self.reset_high) = (c, r);
            *o = (c && self.pass) as u8 as f32;
        }
    }

    fn reset(&mut self) {
        *self = ClockDiv::new();
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        out.write_f32("div", self.div as f32);
        out.write_f32("count", self.count as f32);
        out.write_f32("clock_high", self.clock_high as u8 as f32);
        out.write_f32("reset_high", self.reset_high as u8 as f32);
        out.write_f32("pass", self.pass as u8 as f32);
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        if let Some(v) = s.read_f32("div") {
            self.div = (v as u32).min(8);
        }
        if let Some(v) = s.read_f32("count") {
            self.count = (v as u32).min(7);
        }
        for (k, v) in [
            ("clock_high", &mut self.clock_high),
            ("reset_high", &mut self.reset_high),
            ("pass", &mut self.pass),
        ] {
            if let Some(x) = s.read_f32(k) {
                *v = x >= 0.5;
            }
        }
    }
}
