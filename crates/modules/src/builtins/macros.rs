//! `macro`: four performance controls, `m1`..`m4` (0..1), each with its own CV output. A macro
//! reaches its destinations through ordinary modulation routes (signed amounts, several per
//! output), so one move can shape many controls; the destinations keep their own base values.
//! Names are labels (`name.m1`), presentation only. Outputs glide over 20 ms, so a MIDI CC or a
//! quick drag never steps.

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};

pub const MACROS: usize = 4;

const fn out(name: &'static str) -> PortInfo {
    PortInfo {
        name,
        port_type: PortType::UnipolarCv,
        direction: PortDirection::Output,
    }
}

const fn knob(name: &'static str) -> ParamInfo {
    ParamInfo {
        name,
        min: 0.0,
        max: 1.0,
        default: 0.0,
        unit: "",
        taper: Taper::Linear,
        smoothing_ms: 20.0,
    }
}

const PORTS: &[PortInfo] = &[out("m1"), out("m2"), out("m3"), out("m4")];
const PARAMS: &[ParamInfo] = &[knob("m1"), knob("m2"), knob("m3"), knob("m4")];

pub static MACRO_INFO: ModuleInfo = ModuleInfo {
    kind: "macro",
    name: "Macros",
    category: Category::Modulator,
    rate: Rate::Global,
    explain: "Four named performance knobs. Route each one to as many controls as you like, \
              with its own depth and direction, and play them from the Perform panel or MIDI.",
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
    width_units: 10,
    advanced: &[],
};

const TAU_S: f32 = 0.02;

pub struct Macro {
    sample_rate: f32,
    /// Smoothed outputs; NaN until the first block (which starts at the knob).
    v: [f32; MACROS],
}

impl Macro {
    pub fn new() -> Self {
        Macro {
            sample_rate: 48000.0,
            v: [f32::NAN; MACROS],
        }
    }
}

impl Default for Macro {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Macro {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &MACRO_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let k = 1.0 - (-1.0 / (TAU_S * self.sample_rate)).exp();
        for m in 0..MACROS {
            let target = io.param(m).at(0).clamp(0.0, 1.0);
            let mut v = if self.v[m].is_nan() {
                target
            } else {
                self.v[m]
            };
            for o in io.output(m)[..n].iter_mut() {
                v += (target - v) * k;
                *o = v;
            }
            // f32 stalls a few 1e-5 short of the target; settle exactly there.
            self.v[m] = if (v - target).abs() < 1e-4 { target } else { v };
        }
    }

    fn reset(&mut self) {
        self.v = [f32::NAN; MACROS];
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        for (k, v) in ["m1", "m2", "m3", "m4"].iter().zip(self.v) {
            out.write_f32(k, v);
        }
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        for (k, v) in ["m1", "m2", "m3", "m4"].iter().zip(self.v.iter_mut()) {
            if let Some(x) = s.read_f32(k) {
                *v = x;
            }
        }
    }
}
