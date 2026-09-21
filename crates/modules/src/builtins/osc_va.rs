//! `osc.va` (brief section 8's v1 built-in table: "saw/square/tri/sine, PolyBLEP, hard sync
//! input"). **Only the saw waveform and no hard sync yet** — this is the first real `Module`
//! impl, meant to prove the trait/`ProcessIo` design works end-to-end, not to finish the table
//! entry. Square/tri/sine waveform selection and the hard-sync input are real gaps, tracked in
//! docs/decisions.md, not silently dropped.

use crate::dsp::Saw;
use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "pitch",
        port_type: PortType::Pitch,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "out",
        port_type: PortType::Audio,
        direction: PortDirection::Output,
    },
];

const PARAMS: &[ParamInfo] = &[ParamInfo {
    name: "base_hz",
    min: 20.0,
    max: 20000.0,
    default: 261.63, // C4 — the frequency a "pitch" input of 0 semitones resolves to.
    unit: "Hz",
    taper: Taper::Exponential,
    smoothing_ms: 5.0,
}];

pub static OSC_VA_INFO: ModuleInfo = ModuleInfo {
    kind: "osc.va",
    name: "VA Oscillator",
    category: Category::Source,
    rate: Rate::Voice,
    explain: "A virtual-analog oscillator: turns a pitch into a buzzy, harmonically rich wave.",
    lesson: None,
    requires: &[],
    ports: PORTS,
    params: PARAMS,
    quality: QualitySupport {
        oversampling: false,
        anti_aliasing: true, // PolyBLEP
        interpolation: false,
    },
};

/// Input port index for `pitch` — matches `PORTS` above. Named constants because `ProcessIo`'s
/// indices are position-based (brief section 8: "indices ... match the module's `ModuleInfo`
/// order"), and a bare `0`/`1` at every call site would be easy to get wrong once modules have
/// more than one or two ports.
const PITCH_IN: usize = 0;
const OUT: usize = 0;
const BASE_HZ_PARAM: usize = 0;

pub struct OscVa {
    osc: Saw,
    sample_rate: f32,
}

impl OscVa {
    pub fn new() -> Self {
        OscVa {
            osc: Saw::new(),
            sample_rate: 48000.0,
        }
    }
}

impl Default for OscVa {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for OscVa {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &OSC_VA_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let pitch = io.input(PITCH_IN);
        let base_hz = io.param(BASE_HZ_PARAM);
        let block_len = io.block_len();
        let sample_rate = self.sample_rate;
        let osc = &mut self.osc;
        let out = &mut io.output(OUT)[..block_len];
        for (i, sample) in out.iter_mut().enumerate() {
            let semitones = pitch.at(i);
            let freq = base_hz.at(i) * 2f32.powf(semitones / 12.0);
            *sample = osc.next(freq, sample_rate);
        }
    }

    fn reset(&mut self) {
        self.osc = Saw::new();
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        out.write_f32("phase", self.osc.phase);
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        if let Some(phase) = s.read_f32("phase") {
            self.osc.phase = phase;
        }
    }
}
