//! `ringmod` (brief section 8: "two-input multiply"). No params, no state — kept exactly as
//! minimal as the brief's one-line spec, resisting the temptation to add a dry/wet mix knob or
//! similar that the brief didn't ask for (scope discipline, brief section 2).

use crate::info::{Category, ModuleInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "a",
        port_type: PortType::Audio,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "b",
        port_type: PortType::Audio,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "out",
        port_type: PortType::Audio,
        direction: PortDirection::Output,
    },
];

pub static RINGMOD_INFO: ModuleInfo = ModuleInfo {
    kind: "ringmod",
    name: "Ring Modulator",
    category: Category::Effect,
    rate: Rate::Voice,
    explain: "Multiplies two signals together, producing metallic, bell-like sum/difference tones.",
    lesson: None,
    requires: &[],
    ports: PORTS,
    params: &[],
    quality: QualitySupport {
        oversampling: false,
        anti_aliasing: false,
        interpolation: false,
    },
    skin: None,
    width_units: 5,
};

const A: usize = 0;
const B: usize = 1;
const OUT: usize = 0;

#[derive(Default)]
pub struct RingMod;

impl RingMod {
    pub fn new() -> Self {
        RingMod
    }
}

impl Module for RingMod {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &RINGMOD_INFO
    }

    fn prepare(&mut self, _sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {}

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let a = io.input(A);
        let b = io.input(B);
        let block_len = io.block_len();
        let out = &mut io.output(OUT)[..block_len];
        for (i, sample) in out.iter_mut().enumerate() {
            *sample = a.at(i) * b.at(i);
        }
    }

    fn reset(&mut self) {}
}
