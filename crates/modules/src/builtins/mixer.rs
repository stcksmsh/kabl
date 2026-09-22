//! `mixer` (brief section 8: "4 inputs"). Per-channel level knobs, defaulting to unity (1.0) —
//! a reasonable, minimal reading of "4 inputs" (a fader per channel is the universal expectation
//! for anything called a mixer), not scope creep beyond the brief's one-line spec. Sums, doesn't
//! auto-normalize — gain staging is the patcher's job, matching how hardware/software mixers
//! conventionally behave.

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "in1",
        port_type: PortType::Audio,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "in2",
        port_type: PortType::Audio,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "in3",
        port_type: PortType::Audio,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "in4",
        port_type: PortType::Audio,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "out",
        port_type: PortType::Audio,
        direction: PortDirection::Output,
    },
];

const LEVEL_PARAM: ParamInfo = ParamInfo {
    name: "level",
    min: 0.0,
    max: 1.0,
    default: 1.0,
    unit: "",
    taper: Taper::Linear,
    smoothing_ms: 5.0,
};

const PARAMS: &[ParamInfo] = &[LEVEL_PARAM, LEVEL_PARAM, LEVEL_PARAM, LEVEL_PARAM];

pub static MIXER_INFO: ModuleInfo = ModuleInfo {
    kind: "mixer",
    name: "Mixer",
    category: Category::Utility,
    rate: Rate::Voice,
    explain: "Sums up to 4 signals, each with its own level.",
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
};

const INPUTS: [usize; 4] = [0, 1, 2, 3];
const OUT: usize = 0;
const LEVEL_PARAMS: [usize; 4] = [0, 1, 2, 3];

#[derive(Default)]
pub struct Mixer;

impl Mixer {
    pub fn new() -> Self {
        Mixer
    }
}

impl Module for Mixer {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &MIXER_INFO
    }

    fn prepare(&mut self, _sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {}

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let inputs: [_; 4] = INPUTS.map(|idx| io.input(idx));
        let levels: [_; 4] = LEVEL_PARAMS.map(|idx| io.param(idx));
        let block_len = io.block_len();

        let out = &mut io.output(OUT)[..block_len];
        for (i, sample) in out.iter_mut().enumerate() {
            *sample = inputs
                .iter()
                .zip(levels.iter())
                .map(|(input, level)| input.at(i) * level.at(i))
                .sum();
        }
    }

    fn reset(&mut self) {}
}
