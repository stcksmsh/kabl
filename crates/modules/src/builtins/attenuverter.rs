//! `attenuverter`: `out = in * amount + offset`. Amount below 1 attenuates, above 1 amplifies,
//! negative inverts (an inverted LFO, an envelope turned upside down); offset shifts the
//! result (to centre a unipolar signal, or to transpose a pitch). Works on any signal: control,
//! pitch or audio. Param changes ramp in the engine, so there is no zipper. Follows its input:
//! one instance per voice behind a `midi.in`, otherwise one.

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "in",
        port_type: PortType::Cv,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "out",
        port_type: PortType::Cv,
        direction: PortDirection::Output,
    },
];

const PARAMS: &[ParamInfo] = &[
    ParamInfo {
        name: "amount",
        min: -24.0,
        max: 24.0,
        default: 1.0,
        unit: "x",
        taper: Taper::Cubic,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "offset",
        min: -24.0,
        max: 24.0,
        default: 0.0,
        unit: "",
        taper: Taper::Cubic,
        smoothing_ms: 0.0,
    },
];

pub static ATTENUVERTER_INFO: ModuleInfo = ModuleInfo {
    kind: "attenuverter",
    name: "Attenuverter",
    category: Category::Utility,
    rate: Rate::Voice,
    explain: "Scales a signal (lower, raise or flip it) and shifts it up or down.",
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
    width_units: 5,
    advanced: &[],
};

#[derive(Default)]
pub struct Attenuverter;

impl Attenuverter {
    pub fn new() -> Self {
        Attenuverter
    }
}

impl Module for Attenuverter {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &ATTENUVERTER_INFO
    }

    fn prepare(&mut self, _sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {}

    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let (input, amount, offset) = (io.input(0), io.param(0), io.param(1));
        for (i, o) in io.output(0)[..n].iter_mut().enumerate() {
            let y = input.at(i) * amount.at(i) + offset.at(i);
            *o = if y.is_finite() { y } else { 0.0 };
        }
    }

    fn reset(&mut self) {}
}
