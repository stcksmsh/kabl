//! `slew`: limits how fast a signal may rise and fall, with separate times: a glide for pitch
//! and steps, a smoother for stepped values, and with `mode` FOLLOW (the input is rectified
//! first) an envelope follower whose attack is `rise` and release is `fall`. Each time is how
//! long the output takes to get within 1 % of a step (a one-pole, so big and small steps take
//! the same time); 0.1 ms is the shortest and passes the signal as it is. Follows its input:
//! one instance per voice behind a `midi.in`, otherwise one.

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "in",
        port_type: PortType::Cv,
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
        name: "rise_ms",
        min: 0.1,
        max: 10000.0,
        default: 100.0,
        unit: "ms",
        taper: Taper::Exponential,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "fall_ms",
        min: 0.1,
        max: 10000.0,
        default: 100.0,
        unit: "ms",
        taper: Taper::Exponential,
        smoothing_ms: 0.0,
    },
    // SLEW, FOLLOW.
    ParamInfo {
        name: "mode",
        min: 0.0,
        max: 1.0,
        default: 0.0,
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    },
];

pub const MODE_LABELS: [&str; 2] = ["SLEW", "FOLLOW"];

pub static SLEW_INFO: ModuleInfo = ModuleInfo {
    kind: "slew",
    name: "Slew",
    category: Category::Utility,
    rate: Rate::Voice,
    explain: "Smooths a signal with separate rise and fall times: glide between notes, round off steps, follow a loudness.",
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
    width_units: 6,
    advanced: &[],
};

pub struct Slew {
    sample_rate: f32,
    /// f64 so a long, slow slew still lands on its target instead of stalling a few ulps short.
    y: f64,
}

impl Slew {
    pub fn new() -> Self {
        Slew {
            sample_rate: 48000.0,
            y: 0.0,
        }
    }
}

impl Default for Slew {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-sample coefficient that gets within 1 % of a step in `ms` (1 at the 0.1 ms minimum).
fn coef(ms: f32, sr: f32) -> f32 {
    if ms <= 0.1 {
        1.0
    } else {
        1.0 - (-4.6 / (ms * 0.001 * sr)).exp()
    }
}

impl Module for Slew {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &SLEW_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let input = io.input(0);
        let rise = coef(io.param(0).at(0), self.sample_rate);
        let fall = coef(io.param(1).at(0), self.sample_rate);
        let follow = io.param(2).at(0) >= 0.5;
        let mut y = self.y;
        for (i, o) in io.output(0)[..n].iter_mut().enumerate() {
            let mut x = input.at(i) as f64;
            if !x.is_finite() {
                x = y;
            }
            if follow {
                x = x.abs();
            }
            y += (x - y) * if x > y { rise } else { fall } as f64;
            // Lands exactly, so a settled output is the input and never goes denormal.
            if (x - y).abs() < 1e-7 {
                y = x;
            }
            *o = y as f32;
        }
        self.y = y;
    }

    fn reset(&mut self) {
        self.y = 0.0;
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        out.write_f32("y", self.y as f32);
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        if let Some(v) = s.read_f32("y").filter(|v| v.is_finite()) {
            self.y = v as f64;
        }
    }
}
