//! `osc.wt`: a wavetable oscillator. One voice reads a band-limited table (`wavetable.rs`) at
//! the pitch given by `pitch` and `base_hz`, sweeping through the table's frames with
//! `position`. The `pos` input adds an audio-rate offset (scaled by `pos_mod`), so an LFO,
//! envelope or another oscillator morphs the timbre without block-rate stepping.
//!
//! `table` picks a factory table; `user` > 0 picks the patch's embedded table in that slot
//! instead (the compiler hands the decoded table over with `set_user`; a slot that is empty or
//! undecodable plays the factory table, never silence). Both are structural: a change compiles
//! a new graph, which the engine crossfades.

use std::sync::Arc;

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};
use crate::wavetable::{self, WaveTable};

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "pitch",
        port_type: PortType::Pitch,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "pos",
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
        name: "base_hz",
        min: 20.0,
        max: 20000.0,
        default: 261.63, // C4, as osc.va: the frequency a pitch of 0 semitones resolves to.
        unit: "Hz",
        taper: Taper::Exponential,
        smoothing_ms: 5.0,
    },
    ParamInfo {
        name: "table",
        min: 0.0,
        max: (wavetable::FACTORY_COUNT - 1) as f32,
        default: 0.0,
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "position",
        min: 0.0,
        max: 1.0,
        default: 0.0,
        unit: "",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "pos_mod",
        min: 0.0,
        max: 1.0,
        default: 1.0,
        unit: "",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "fine",
        min: -100.0,
        max: 100.0,
        default: 0.0,
        unit: "ct",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
    // 0 = the factory table; 1..=8 = the patch's embedded table in that slot.
    ParamInfo {
        name: "user",
        min: 0.0,
        max: 8.0,
        default: 0.0,
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    },
];

pub static OSC_WT_INFO: ModuleInfo = ModuleInfo {
    kind: "osc.wt",
    name: "Wavetable Oscillator",
    category: Category::Source,
    rate: Rate::Voice,
    explain: "A wavetable oscillator: sweeps through a table of waveforms to morph the timbre.",
    lesson: None,
    requires: &[],
    ports: PORTS,
    params: PARAMS,
    quality: QualitySupport {
        oversampling: false,
        anti_aliasing: true, // mipmapped tables, see wavetable.rs
        interpolation: true,
    },
    skin: None,
    width_units: 6,
    advanced: &["pos_mod", "fine", "user"],
};

const PITCH_IN: usize = 0;
const POS_IN: usize = 1;
const OUT: usize = 0;
const BASE_HZ_PARAM: usize = 0;
const TABLE_PARAM: usize = 1;
const POSITION_PARAM: usize = 2;
const POS_MOD_PARAM: usize = 3;
const FINE_PARAM: usize = 4;
const USER_PARAM: usize = 5;

pub struct OscWt {
    phase: f32,
    /// `position` at the end of the last block: the next block ramps from it. Negative before
    /// the first block.
    position: f32,
    sample_rate: f32,
    user: Option<Arc<WaveTable>>,
}

impl OscWt {
    /// Index of the `user` param, which the compiler reads to find the embedded table slot.
    pub const USER_PARAM: usize = USER_PARAM;

    pub fn new() -> Self {
        OscWt {
            phase: 0.0,
            position: -1.0,
            sample_rate: 48000.0,
            user: None,
        }
    }

    /// The embedded table this oscillator plays when `user` > 0.
    pub fn set_user(&mut self, table: Option<Arc<WaveTable>>) {
        self.user = table;
    }
}

impl Default for OscWt {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for OscWt {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &OSC_WT_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
        wavetable::init_factory();
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let pitch = io.input(PITCH_IN);
        let pos_in = io.input(POS_IN);
        let base_hz = io.param(BASE_HZ_PARAM);
        let table_index = io.param(TABLE_PARAM).at(0).round().max(0.0) as usize;
        let target = io.param(POSITION_PARAM).at(0).clamp(0.0, 1.0);
        let depth = io.param(POS_MOD_PARAM).at(0);
        let fine = 2f32.powf(io.param(FINE_PARAM).at(0) / 1200.0);
        let user = io.param(USER_PARAM).at(0).round() > 0.0;
        let n = io.block_len();
        let sr = self.sample_rate;

        let table: &WaveTable = match (&self.user, user) {
            (Some(t), true) => t,
            _ => wavetable::factory(table_index),
        };
        if self.position < 0.0 {
            self.position = target;
        }
        let step = (target - self.position) / n as f32;
        // The richest mipmap that stays under Nyquist at the block's highest pitch.
        let top = match pitch {
            crate::io::Signal::Scalar(v) => v,
            crate::io::Signal::Buffer(b) => b[..n].iter().fold(f32::MIN, |m, &v| m.max(v)),
        };
        let dt_max = base_hz.at(0).max(base_hz.at(n - 1)) * 2f32.powf(top / 12.0) * fine / sr;
        let level = table.level_for(dt_max);

        let out = &mut io.output(OUT)[..n];
        let mut position = self.position;
        let mut phase = self.phase;
        for (i, o) in out.iter_mut().enumerate() {
            position += step;
            let dt = base_hz.at(i) * 2f32.powf(pitch.at(i) / 12.0) * fine / sr;
            *o = table.read(level, position + pos_in.at(i) * depth, phase);
            phase += dt;
            phase -= phase.floor();
        }
        self.position = target;
        // A NaN pitch must not poison the phase for good.
        self.phase = if phase.is_finite() { phase } else { 0.0 };
    }

    fn reset(&mut self) {
        let (sr, user) = (self.sample_rate, self.user.take());
        *self = OscWt::new();
        self.sample_rate = sr;
        self.user = user;
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        out.write_f32("phase", self.phase);
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        if let Some(phase) = s.read_f32("phase") {
            self.phase = phase.rem_euclid(1.0);
        }
    }

    /// Phase and position only; the table belongs to the new graph.
    fn carry_from(&mut self, old: &dyn Module) {
        if let Some(o) = old.as_any().downcast_ref::<OscWt>() {
            self.phase = o.phase;
            self.position = o.position;
        }
    }
}
