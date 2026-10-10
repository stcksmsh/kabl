//! `crossfade` and `pan`.
//!
//! `crossfade` blends `a` into `b`: `mix` 0 is all `a`, 1 is all `b`, and the `fade` input adds
//! to `mix` (the sum is held between 0 and 1), so an LFO, envelope or sequencer can move the
//! blend. `law` LINEAR keeps a signal that is the same on both inputs unchanged at every
//! position (right for control signals); EQUAL POWER dips less in loudness at the middle (right
//! for two different sounds). Either way 0 and 1 are exactly `a` and exactly `b`.
//!
//! `pan` places a mono signal in the stereo field: `pan` -1 is all left, 0 centre, +1 all right,
//! the `pan` input adds to the knob. Equal power, so the centre is 3 dB down on each side and
//! the loudness stays the same across the field.
//!
//! Both follow their inputs: one instance per voice behind a `midi.in`, otherwise one.

use std::f32::consts::FRAC_PI_2;

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};

const NO_QUALITY: QualitySupport = QualitySupport {
    oversampling: false,
    anti_aliasing: false,
    interpolation: false,
};

const CROSSFADE_PORTS: &[PortInfo] = &[
    PortInfo {
        name: "a",
        port_type: PortType::Audio,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "b",
        port_type: PortType::Audio,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "fade",
        port_type: PortType::Cv,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "out",
        port_type: PortType::Audio,
        direction: PortDirection::Output,
    },
];

const CROSSFADE_PARAMS: &[ParamInfo] = &[
    ParamInfo {
        name: "mix",
        min: 0.0,
        max: 1.0,
        default: 0.5,
        unit: "",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
    // 0 = linear gains, 1 = equal-power gains.
    ParamInfo {
        name: "curve",
        min: 0.0,
        max: 1.0,
        default: 0.0,
        unit: "",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
];

pub static CROSSFADE_INFO: ModuleInfo = ModuleInfo {
    kind: "crossfade",
    name: "Crossfade",
    category: Category::Utility,
    rate: Rate::Voice,
    explain: "Blends between two signals; a control signal on the fade input moves the blend.",
    lesson: None,
    requires: &[],
    ports: CROSSFADE_PORTS,
    params: CROSSFADE_PARAMS,
    quality: NO_QUALITY,
    skin: None,
    width_units: 5,
    advanced: &["curve"],
};

#[derive(Default)]
pub struct Crossfade;

impl Crossfade {
    pub fn new() -> Self {
        Crossfade
    }
}

impl Module for Crossfade {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &CROSSFADE_INFO
    }

    fn prepare(&mut self, _sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {}

    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let (a, b, fade, mix) = (io.input(0), io.input(1), io.input(2), io.param(0));
        let curve = io.param(1);
        for (i, o) in io.output(0)[..n].iter_mut().enumerate() {
            let p = (mix.at(i) + fade.at(i)).clamp(0.0, 1.0);
            let p = if p.is_finite() { p } else { 0.0 };
            let c = curve.at(i).clamp(0.0, 1.0);
            let ga = (1.0 - p) + c * ((p * FRAC_PI_2).cos() - (1.0 - p));
            let gb = p + c * ((p * FRAC_PI_2).sin() - p);
            // The ends are exact, whatever the law's rounding.
            let y = if p == 0.0 {
                a.at(i)
            } else if p == 1.0 {
                b.at(i)
            } else {
                a.at(i) * ga + b.at(i) * gb
            };
            *o = if y.is_finite() { y } else { 0.0 };
        }
    }

    fn reset(&mut self) {}
}

const PAN_PORTS: &[PortInfo] = &[
    PortInfo {
        name: "in",
        port_type: PortType::Audio,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "pan",
        port_type: PortType::Cv,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "left",
        port_type: PortType::Audio,
        direction: PortDirection::Output,
    },
    PortInfo {
        name: "right",
        port_type: PortType::Audio,
        direction: PortDirection::Output,
    },
];

const PAN_PARAMS: &[ParamInfo] = &[ParamInfo {
    name: "pan",
    min: -1.0,
    max: 1.0,
    default: 0.0,
    unit: "",
    taper: Taper::Linear,
    smoothing_ms: 0.0,
}];

pub static PAN_INFO: ModuleInfo = ModuleInfo {
    kind: "pan",
    name: "Pan",
    category: Category::Utility,
    rate: Rate::Voice,
    explain: "Places a mono sound between the left and right speakers.",
    lesson: None,
    requires: &[],
    ports: PAN_PORTS,
    params: PAN_PARAMS,
    quality: NO_QUALITY,
    skin: None,
    width_units: 4,
    advanced: &[],
};

#[derive(Default)]
pub struct Pan;

impl Pan {
    pub fn new() -> Self {
        Pan
    }
}

impl Module for Pan {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &PAN_INFO
    }

    fn prepare(&mut self, _sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {}

    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let (input, cv, knob) = (io.input(0), io.input(1), io.param(0));
        for i in 0..n {
            let p = (knob.at(i) + cv.at(i)).clamp(-1.0, 1.0);
            let p = if p.is_finite() { p } else { 0.0 };
            let angle = (p + 1.0) * 0.5 * FRAC_PI_2;
            let x = input.at(i);
            let fin = |v: f32| if v.is_finite() { v } else { 0.0 };
            io.output(0)[i] = fin(x * angle.cos());
            io.output(1)[i] = fin(x * angle.sin());
        }
    }

    fn reset(&mut self) {}
}
