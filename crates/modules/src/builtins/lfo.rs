//! `lfo` (brief section 8: "sine/tri/saw/square/S&H, rate in Hz, sync later"). Wraps
//! `dsp::FullLfo` — all five waveforms, "sync" (to host/internal clock) is out of scope until
//! there's a clock to sync to (brief section 9: clock is v3 scope).
//!
//! Waveform selection is a discrete choice, not a continuous value, but `ParamInfo` (brief
//! section 8) only models float ranges — no enum-param variant exists. Represented as a stepped
//! float param (`0..=4`, rounded in `process`), the standard modular-software convention for a
//! "pick one of N" knob. Documented here since it's not obvious from `ParamInfo`'s shape alone.

use crate::dsp::{FullLfo, LfoWaveform};
use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};

const PORTS: &[PortInfo] = &[PortInfo {
    name: "out",
    port_type: PortType::Cv,
    direction: PortDirection::Output,
}];

const PARAMS: &[ParamInfo] = &[
    ParamInfo {
        name: "rate_hz",
        min: 0.01,
        max: 100.0,
        default: 2.0,
        unit: "Hz",
        taper: Taper::Exponential,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "waveform",
        min: 0.0,
        max: 4.0,
        default: 0.0, // Sine
        unit: "",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
];

pub static LFO_INFO: ModuleInfo = ModuleInfo {
    kind: "lfo",
    name: "LFO",
    category: Category::Modulator,
    rate: Rate::Voice,
    explain: "A slow wave for making other things wobble, sweep, or pulse.",
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

const OUT: usize = 0;
const RATE_PARAM: usize = 0;
const WAVEFORM_PARAM: usize = 1;

fn waveform_from_param(v: f32) -> LfoWaveform {
    match v.round() as i32 {
        1 => LfoWaveform::Triangle,
        2 => LfoWaveform::Saw,
        3 => LfoWaveform::Square,
        4 => LfoWaveform::SampleAndHold,
        _ => LfoWaveform::Sine,
    }
}

pub struct Lfo {
    lfo: FullLfo,
    sample_rate: f32,
}

impl Lfo {
    pub fn new() -> Self {
        Lfo {
            lfo: FullLfo::new(),
            sample_rate: 48000.0,
        }
    }
}

impl Default for Lfo {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Lfo {
    fn info(&self) -> &'static ModuleInfo {
        &LFO_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let rate_hz = io.param(RATE_PARAM).at(0);
        let waveform = waveform_from_param(io.param(WAVEFORM_PARAM).at(0));
        let block_len = io.block_len();
        let sample_rate = self.sample_rate;

        let lfo = &mut self.lfo;
        let out = &mut io.output(OUT)[..block_len];
        for sample in out.iter_mut() {
            *sample = lfo.next(rate_hz, sample_rate, waveform);
        }
    }

    fn reset(&mut self) {
        self.lfo = FullLfo::new();
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        out.write_f32("phase", self.lfo.phase);
        out.write_f32("held_sample", self.lfo.held_sample());
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        if let Some(phase) = s.read_f32("phase") {
            self.lfo.phase = phase;
        }
        if let Some(held) = s.read_f32("held_sample") {
            self.lfo.set_held_sample(held);
        }
    }
}
