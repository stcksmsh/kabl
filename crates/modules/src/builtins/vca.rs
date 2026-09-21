//! `vca` (brief section 8: "linear and exponential response"). No internal state — a VCA is
//! purely `input * gain`, so there's nothing to carry across a repatch and `save_state`/
//! `load_state` are left as the trait's no-op defaults.
//!
//! "Exponential response" is approximated as squaring the linear gain (`gain * gain`) — a
//! common, cheap way to make a linear 0..1 knob feel more like perceived loudness (which is
//! roughly logarithmic). Not a precise psychoacoustic curve; flagged as an approximation, not
//! presented as more rigorous than it is.

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "in",
        port_type: PortType::Audio,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "cv",
        port_type: PortType::Cv,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "out",
        port_type: PortType::Audio,
        direction: PortDirection::Output,
    },
];

const PARAMS: &[ParamInfo] = &[
    ParamInfo {
        name: "gain",
        min: 0.0,
        max: 1.0,
        default: 1.0,
        unit: "",
        taper: Taper::Linear,
        smoothing_ms: 5.0,
    },
    ParamInfo {
        name: "exponential",
        min: 0.0,
        max: 1.0,
        default: 0.0, // linear response by default
        unit: "",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
];

pub static VCA_INFO: ModuleInfo = ModuleInfo {
    kind: "vca",
    name: "VCA",
    category: Category::Utility,
    rate: Rate::Voice,
    explain: "A voltage-controlled amplifier: scales a signal's volume, by hand or by CV.",
    lesson: None,
    requires: &[],
    ports: PORTS,
    params: PARAMS,
    quality: QualitySupport {
        oversampling: false,
        anti_aliasing: false,
        interpolation: false,
    },
};

const IN: usize = 0;
const CV: usize = 1;
const OUT: usize = 0;
const GAIN_PARAM: usize = 0;
const EXPONENTIAL_PARAM: usize = 1;

#[derive(Default)]
pub struct Vca;

impl Vca {
    pub fn new() -> Self {
        Vca
    }
}

impl Module for Vca {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &VCA_INFO
    }

    fn prepare(&mut self, _sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {}

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let input = io.input(IN);
        let cv = io.input(CV);
        let gain_param = io.param(GAIN_PARAM);
        let exponential = io.param(EXPONENTIAL_PARAM).at(0) >= 0.5;
        let block_len = io.block_len();

        let out = &mut io.output(OUT)[..block_len];
        for (i, sample) in out.iter_mut().enumerate() {
            let mut gain = (gain_param.at(i) + cv.at(i)).clamp(0.0, 1.0);
            if exponential {
                gain *= gain;
            }
            *sample = input.at(i) * gain;
        }
    }

    fn reset(&mut self) {}
}
