//! `env.adsr` (brief section 8: "exponential segments, gate in"). Wraps `dsp::FullAdsr`.
//!
//! Params (attack/decay/sustain/release) are read once per block, not per sample: they're knob
//! values in the overwhelming common case, not audio-rate CV, and re-deriving `FullAdsr`'s
//! exponential coefficients (each an `exp()` call) every sample for values that essentially
//! never change would repeat spike S3's exact finding — recomputing a transcendental for a
//! block-constant value burns cycles for nothing. If a real patch ever wants per-sample ADSR-time
//! modulation, that's a deliberate limitation to revisit then, not an oversight now.

use crate::dsp::FullAdsr;
use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "gate",
        port_type: PortType::Gate,
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
        name: "attack_ms",
        min: 0.1,
        max: 10000.0,
        default: 10.0,
        unit: "ms",
        taper: Taper::Exponential,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "decay_ms",
        min: 0.1,
        max: 10000.0,
        default: 100.0,
        unit: "ms",
        taper: Taper::Exponential,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "sustain",
        min: 0.0,
        max: 1.0,
        default: 0.7,
        unit: "",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "release_ms",
        min: 0.1,
        max: 10000.0,
        default: 200.0,
        unit: "ms",
        taper: Taper::Exponential,
        smoothing_ms: 0.0,
    },
];

pub static ENV_ADSR_INFO: ModuleInfo = ModuleInfo {
    kind: "env.adsr",
    name: "ADSR Envelope",
    category: Category::Modulator,
    rate: Rate::Voice,
    explain:
        "Shapes loudness (or anything else) over time: attack up, decay down, sustain, release.",
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

const GATE_IN: usize = 0;
const OUT: usize = 0;
const ATTACK_PARAM: usize = 0;
const DECAY_PARAM: usize = 1;
const SUSTAIN_PARAM: usize = 2;
const RELEASE_PARAM: usize = 3;

pub struct EnvAdsr {
    env: FullAdsr,
    sample_rate: f32,
}

impl EnvAdsr {
    pub fn new() -> Self {
        EnvAdsr {
            env: FullAdsr::new(10.0, 100.0, 0.7, 200.0, 48000.0),
            sample_rate: 48000.0,
        }
    }
}

impl Default for EnvAdsr {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for EnvAdsr {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &ENV_ADSR_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let gate = io.input(GATE_IN);
        let attack_ms = io.param(ATTACK_PARAM).at(0);
        let decay_ms = io.param(DECAY_PARAM).at(0);
        let sustain = io.param(SUSTAIN_PARAM).at(0);
        let release_ms = io.param(RELEASE_PARAM).at(0);
        let block_len = io.block_len();

        self.env
            .set_params(attack_ms, decay_ms, sustain, release_ms, self.sample_rate);

        let env = &mut self.env;
        let out = &mut io.output(OUT)[..block_len];
        for (i, sample) in out.iter_mut().enumerate() {
            *sample = env.next(gate.at(i));
        }
    }

    fn reset(&mut self) {
        self.env.reset();
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        out.write_f32("level", self.env.level);
        out.write_f32("stage", self.env.stage.to_u8() as f32);
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        if let Some(level) = s.read_f32("level") {
            self.env.level = level;
        }
        if let Some(stage) = s.read_f32("stage") {
            self.env.stage = crate::dsp::AdsrStage::from_u8(stage as u8);
        }
    }
}
