//! `cues`: holds up to eight named section cues. A cue picks a bank for several sequencers and
//! launches them together on a reference clock's next step or bar. It makes no sound: the cue
//! definitions are presentation data on the module (`cue<N>.*` params, names as labels) that
//! the editor reads, and a launch is a runtime command to the engine. Having a module gives
//! the cues a place in the rack, the Perform panel and the undo log.

use crate::info::{Category, ModuleInfo, QualitySupport, Rate};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};

pub const CUES: usize = 8;

pub static CUES_INFO: ModuleInfo = ModuleInfo {
    kind: "cues",
    name: "Cues",
    category: Category::Sequencer,
    rate: Rate::Global,
    explain: "Named sections: each cue switches several sequencers to chosen banks together, \
              on the next step or bar.",
    lesson: None,
    requires: &[],
    ports: &[],
    params: &[],
    quality: QualitySupport {
        oversampling: false,
        anti_aliasing: false,
        interpolation: false,
    },
    skin: None,
    width_units: 16,
    advanced: &[],
};

#[derive(Default)]
pub struct Cues;

impl Module for Cues {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn info(&self) -> &'static ModuleInfo {
        &CUES_INFO
    }
    fn prepare(&mut self, _sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {}
    fn process(&mut self, _io: &mut ProcessIo) {}
    fn reset(&mut self) {}
}
