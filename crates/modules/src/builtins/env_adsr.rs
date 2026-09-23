//! `env.adsr` (brief section 8: "exponential segments, gate in"). Wraps `dsp::FullAdsr`.
//!
//! Params (attack/decay/sustain/release) are read once per block, not per sample: they're knob
//! values in the overwhelming common case, not audio-rate CV, and re-deriving `FullAdsr`'s
//! exponential coefficients (each an `exp()` call) every sample for values that essentially
//! never change would repeat spike S3's exact finding — recomputing a transcendental for a
//! block-constant value burns cycles for nothing. Modulated times therefore take effect at
//! block resolution (64 samples).
//!
//! `timing` selects how Attack, Decay and Release follow changes (owner decision, see
//! decisions.md "Envelope timing modes"):
//! - 0 = CONTINUOUS (default): every block uses the current times. The level and stage in
//!   progress are kept; only the rate of the running segment changes.
//! - 1 = KEY-TRIGGER: Attack, Decay and Release are captured at the gate's rising edge (the
//!   sample the note starts) and used for the whole note, release included. A retrigger
//!   captures fresh values. Sustain is a level, not a time, so it stays live in both modes.
//!
//! Switching CONTINUOUS -> KEY-TRIGGER while a note is held captures the times at that block;
//! the next note-on captures again. Switching back follows the live times from the next block.
//! Captured times are module state, so they survive recompiles.

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
        port_type: PortType::UnipolarCv,
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
    ParamInfo {
        name: "timing",
        min: 0.0,
        max: 1.0,
        default: 0.0,
        unit: "",
        taper: Taper::Stepped,
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
    skin: None,
    width_units: 8,
    advanced: &["timing"],
};

const GATE_IN: usize = 0;
const OUT: usize = 0;
const ATTACK_PARAM: usize = 0;
const DECAY_PARAM: usize = 1;
const SUSTAIN_PARAM: usize = 2;
const RELEASE_PARAM: usize = 3;
const TIMING_PARAM: usize = 4;

pub struct EnvAdsr {
    env: FullAdsr,
    sample_rate: f32,
    /// KEY-TRIGGER mode's captured (attack, decay, release) in ms. `None` in CONTINUOUS mode,
    /// or before the first capture.
    latched: Option<(f32, f32, f32)>,
}

impl EnvAdsr {
    pub fn new() -> Self {
        EnvAdsr {
            env: FullAdsr::new(10.0, 100.0, 0.7, 200.0, 48000.0),
            sample_rate: 48000.0,
            latched: None,
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
        let key_trigger = io.param(TIMING_PARAM).at(0) >= 0.5;
        let block_len = io.block_len();
        let sr = self.sample_rate;

        if !key_trigger {
            self.latched = None;
            self.env
                .set_params(attack_ms, decay_ms, sustain, release_ms, sr);
            let env = &mut self.env;
            let out = &mut io.output(OUT)[..block_len];
            for (i, sample) in out.iter_mut().enumerate() {
                *sample = env.next(gate.at(i));
            }
            return;
        }

        let (a, d, r) = *self
            .latched
            .get_or_insert((attack_ms, decay_ms, release_ms));
        self.env.set_params(a, d, sustain, r, sr);
        let out = &mut io.output(OUT)[..block_len];
        for (i, sample) in out.iter_mut().enumerate() {
            let g = gate.at(i);
            if g > 0.5 && !self.env.gate_was_high {
                self.latched = Some((attack_ms, decay_ms, release_ms));
                self.env
                    .set_params(attack_ms, decay_ms, sustain, release_ms, sr);
            }
            *sample = self.env.next(g);
        }
    }

    fn reset(&mut self) {
        self.env.reset();
        self.latched = None;
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        out.write_f32("level", self.env.level);
        out.write_f32("stage", self.env.stage.to_u8() as f32);
        out.write_f32("gate_was_high", self.env.gate_was_high as u8 as f32);
        if let Some((a, d, r)) = self.latched {
            out.write_f32("latched_a", a);
            out.write_f32("latched_d", d);
            out.write_f32("latched_r", r);
        }
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        if let Some(level) = s.read_f32("level") {
            self.env.level = level;
        }
        if let Some(stage) = s.read_f32("stage") {
            self.env.stage = crate::dsp::AdsrStage::from_u8(stage as u8);
        }
        // Without this, a continuously-held gate looks like a fresh note-on to a freshly
        // constructed FullAdsr (whose `new()` always starts `gate_was_high: false`), re-entering
        // Attack from the current `level` every recompile instead of continuing Sustain/Decay —
        // caught by `tests/patch_engine_swap.rs` diverging further from a never-recompiled
        // reference after every one of 20 repeated swaps of an identical, gate-held patch.
        if let Some(g) = s.read_f32("gate_was_high") {
            self.env.gate_was_high = g != 0.0;
        }
        if let (Some(a), Some(d), Some(r)) = (
            s.read_f32("latched_a"),
            s.read_f32("latched_d"),
            s.read_f32("latched_r"),
        ) {
            self.latched = Some((a, d, r));
        }
    }
}
