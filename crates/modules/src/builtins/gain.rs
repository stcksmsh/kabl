//! `gain`: a trim for levels, in dB, above or below unity (a mixer channel tops out at unity).
//! Changes glide over 20 ms so a knob turn, a CC or a modulation route never steps.

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "in",
        port_type: PortType::Audio,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "out",
        port_type: PortType::Audio,
        direction: PortDirection::Output,
    },
];

pub const MIN_DB: f32 = -60.0;
pub const MAX_DB: f32 = 24.0;

const PARAMS: &[ParamInfo] = &[ParamInfo {
    name: "gain_db",
    min: MIN_DB,
    max: MAX_DB,
    default: 0.0,
    unit: "dB",
    taper: Taper::Linear,
    smoothing_ms: 20.0,
}];

pub static GAIN_INFO: ModuleInfo = ModuleInfo {
    kind: "gain",
    name: "Gain",
    category: Category::Utility,
    rate: Rate::Voice,
    explain: "Raises or lowers a signal's level, in dB. 0 dB leaves it as it is.",
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
    width_units: 4,
    advanced: &[],
};

const TAU_S: f32 = 0.02;

pub struct Gain {
    sample_rate: f32,
    /// Smoothed linear gain; negative until the first block.
    g: f32,
}

impl Gain {
    pub fn new() -> Self {
        Gain {
            sample_rate: 48000.0,
            g: -1.0,
        }
    }
}

impl Default for Gain {
    fn default() -> Self {
        Self::new()
    }
}

pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db.clamp(MIN_DB, MAX_DB) / 20.0)
}

impl Module for Gain {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &GAIN_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let input = io.input(0);
        let target = db_to_gain(io.param(0).at(0));
        if self.g < 0.0 {
            self.g = target;
        }
        let k = 1.0 - (-1.0 / (TAU_S * self.sample_rate)).exp();
        let mut g = self.g;
        for (i, o) in io.output(0)[..n].iter_mut().enumerate() {
            g += (target - g) * k;
            *o = input.at(i) * g;
        }
        // Settles exactly, so an unchanged gain is exact (0 dB passes the input untouched).
        self.g = if (g - target).abs() < 1e-6 { target } else { g };
    }

    fn reset(&mut self) {
        self.g = -1.0;
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        out.write_f32("g", self.g);
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        if let Some(g) = s.read_f32("g") {
            self.g = g;
        }
    }
}
