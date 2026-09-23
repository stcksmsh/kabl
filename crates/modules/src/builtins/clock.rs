//! `clock`: the shared tempo source. One global instance drives any number of sequencers, so
//! every pattern patched to it stays in step. Emits a 50 % duty gate, one pulse per 16th note
//! at `bpm` (four pulses per beat, the usual step length of a step sequencer).

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};

const PORTS: &[PortInfo] = &[PortInfo {
    name: "gate",
    port_type: PortType::Gate,
    direction: PortDirection::Output,
}];

const PARAMS: &[ParamInfo] = &[ParamInfo {
    name: "bpm",
    min: 20.0,
    max: 300.0,
    default: 120.0,
    unit: "bpm",
    taper: Taper::Linear,
    smoothing_ms: 0.0,
}];

pub static CLOCK_INFO: ModuleInfo = ModuleInfo {
    kind: "clock",
    name: "Clock",
    category: Category::Sequencer,
    rate: Rate::Global,
    explain: "Ticks four times per beat at the tempo you set; sequencers step on each tick.",
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

const PULSES_PER_BEAT: f32 = 4.0;

pub struct Clock {
    phase: f32,
    sample_rate: f32,
}

impl Clock {
    pub fn new() -> Self {
        Clock {
            phase: 0.0,
            sample_rate: 48000.0,
        }
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Clock {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &CLOCK_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let inc = io.param(0).at(0) / 60.0 * PULSES_PER_BEAT / self.sample_rate;
        let block_len = io.block_len();
        let mut phase = self.phase;
        for s in io.output(0)[..block_len].iter_mut() {
            *s = if phase < 0.5 { 1.0 } else { 0.0 };
            phase = (phase + inc).fract();
        }
        self.phase = phase;
    }

    fn reset(&mut self) {
        self.phase = 0.0;
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        out.write_f32("phase", self.phase);
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        if let Some(p) = s.read_f32("phase") {
            self.phase = p;
        }
    }
}
