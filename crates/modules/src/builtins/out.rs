//! `out` (brief section 8: "stereo output"). The graph's terminal sink. It has no `Module`
//! output ports of its own, but the compiler hands it two hidden output buffers (left, right):
//! `process` writes the input there times the Level (dB, smoothed over 20 ms), and the engine
//! reads those buffers as the patch's output. Level 0 dB (the default, and what a patch saved
//! before the parameter existed loads as) passes the input untouched.

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};

const PORTS: &[PortInfo] = &[PortInfo {
    name: "in",
    port_type: PortType::Audio,
    direction: PortDirection::Input,
    width: 2,
}];

pub const MIN_DB: f32 = -60.0;
pub const MAX_DB: f32 = 12.0;

const PARAMS: &[ParamInfo] = &[ParamInfo {
    name: "level_db",
    min: MIN_DB,
    max: MAX_DB,
    default: 0.0,
    unit: "dB",
    taper: Taper::Linear,
    smoothing_ms: 20.0,
}];

pub static OUT_INFO: ModuleInfo = ModuleInfo {
    kind: "out",
    name: "Output",
    category: Category::Utility,
    rate: Rate::Global,
    explain: "The patch's stereo output: what actually reaches your speakers or headphones. \
              Level trims it in dB; 0 dB leaves it as it is.",
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

const LEFT: usize = 0;
const RIGHT: usize = 1;
const TAU_S: f32 = 0.02;

/// Linear gain of a Level in dB, clamped to the parameter's range.
pub fn level_to_gain(db: f32) -> f32 {
    10f32.powf(db.clamp(MIN_DB, MAX_DB) / 20.0)
}

pub struct Out {
    sample_rate: f32,
    /// Smoothed linear gain; negative until the first block.
    g: f32,
}

impl Out {
    pub fn new() -> Self {
        Out {
            sample_rate: 48000.0,
            g: -1.0,
        }
    }
}

impl Default for Out {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Out {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &OUT_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let left_in = io.input(LEFT);
        let right_in = io.input(RIGHT);
        let target = level_to_gain(io.param(0).at(0));
        if self.g < 0.0 {
            self.g = target;
        }
        let k = 1.0 - (-1.0 / (TAU_S * self.sample_rate)).exp();
        let mut g = self.g;
        {
            let left = io.output(LEFT);
            let mut gl = g;
            for (i, o) in left[..n].iter_mut().enumerate() {
                gl += (target - gl) * k;
                *o = left_in.at(i) * gl;
            }
        }
        {
            let right = io.output(RIGHT);
            for (i, o) in right[..n].iter_mut().enumerate() {
                g += (target - g) * k;
                *o = right_in.at(i) * g;
            }
        }
        // Settles exactly, so an unchanged level is exact (0 dB passes the input untouched).
        self.g = if (g - target).abs() < 1e-6 { target } else { g };
    }

    fn reset(&mut self) {
        self.g = -1.0;
    }
}
