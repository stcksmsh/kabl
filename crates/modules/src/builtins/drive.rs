//! `drive`: saturation. The curve is y = tanh(g·x) / tanh(g), g = 10^(drive/20) − 1: at 0 dB
//! drive it is a straight line (the module passes its input sample for sample), and at any
//! drive a full-scale input comes out at full scale while quieter parts are raised and rounded
//! (small-signal gain g/tanh(g): +2.4 dB at 6 dB drive, +9.5 dB at 12, about the drive itself
//! above 20). It is a fixed curve: no limiter, no automatic level. `trim` sets the output
//! level, `mix` blends the dry input back in.
//!
//! **Anti-aliasing.** First-order antiderivative anti-aliasing (ADAA: Parker, Zavalishin & Le
//! Bivic, "Reducing the aliasing of nonlinear waveshaping using continuous-time convolution",
//! DAFx 2016; Bilbao, Esqueda, Parker & Välimäki, "Antiderivative antialiasing for memoryless
//! nonlinearities", IEEE SPL 24(7), 2017): each output is the mean of the curve over the
//! straight line between the last two input samples, (F(xₙ) − F(xₙ₋₁)) / (xₙ − xₙ₋₁), with
//! F(x) = ln cosh(g·x) / (g·tanh g), computed in f64. No oversampling. It costs half a sample
//! of latency (10 µs at 48 kHz) and a gentle top-octave roll-off of the shaped signal (−0.5 dB
//! at 10 kHz, −3 dB at Nyquist); the dry path is not delayed, and mixing the two gives the same
//! gentle roll-off, not a comb. Measured aliasing: tests/drive.rs.
//!
//! **Neutral default.** Drive 0 dB, trim 0 dB, mix 100 %: the input passes exactly. Leaving
//! 0 dB fades the shaper in over 10 ms (and back out on return), so the half-sample latency
//! never switches in as a step.
//!
//! Voice rate and mono: per voice in a MIDI chain (each note distorts on its own, before the
//! voices are summed), one instance elsewhere; for a stereo bus, one per side.

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};

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

const PARAMS: &[ParamInfo] = &[
    ParamInfo {
        name: "drive_db",
        min: 0.0,
        max: 36.0,
        default: 0.0,
        unit: "dB",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "trim_db",
        min: -24.0,
        max: 12.0,
        default: 0.0,
        unit: "dB",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "mix",
        min: 0.0,
        max: 100.0,
        default: 100.0,
        unit: "%",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
];

pub static DRIVE_INFO: ModuleInfo = ModuleInfo {
    kind: "drive",
    name: "Drive",
    category: Category::Effect,
    rate: Rate::Voice,
    explain: "Saturation: rounds and thickens a sound as you turn it up. Trim sets the level after it; mix blends the clean signal back.",
    lesson: None,
    requires: &[],
    ports: PORTS,
    params: PARAMS,
    quality: QualitySupport {
        oversampling: false,
        anti_aliasing: true,
        interpolation: false,
    },
    skin: None,
    width_units: 6,
    advanced: &["mix"],
};

const CTRL_TAU_S: f32 = 0.02;
const ENGAGE_TAU_S: f32 = 0.01;

#[derive(Debug, Clone, Copy)]
struct Core {
    /// Last input sample.
    x1: f64,
    /// Smoothed curve parameter g, engage (0 = exact bypass), trim gain, mix; g < 0 before the
    /// first block.
    g: f64,
    engage: f32,
    trim: f32,
    mix: f32,
}

pub struct Drive {
    c: Core,
    sample_rate: f32,
}

impl Drive {
    pub fn new() -> Self {
        Drive {
            c: Core {
                x1: 0.0,
                g: -1.0,
                engage: 0.0,
                trim: 1.0,
                mix: 1.0,
            },
            sample_rate: 48000.0,
        }
    }
}

impl Default for Drive {
    fn default() -> Self {
        Self::new()
    }
}

/// ln cosh(u), without overflow for large |u|.
#[inline]
fn ln_cosh(u: f64) -> f64 {
    let a = u.abs();
    a + (-2.0 * a).exp().ln_1p() - std::f64::consts::LN_2
}

/// The curve and its antiderivative for one g (g > 0).
#[derive(Clone, Copy)]
struct Curve {
    g: f64,
    norm: f64,
}

impl Curve {
    fn new(g: f64) -> Curve {
        Curve {
            g,
            norm: 1.0 / g.tanh(),
        }
    }
    #[inline]
    fn f(&self, x: f64) -> f64 {
        (self.g * x).tanh() * self.norm
    }
    #[inline]
    fn big_f(&self, x: f64) -> f64 {
        ln_cosh(self.g * x) * self.norm / self.g
    }
}

/// g for a drive in dB.
pub fn drive_g(db: f32) -> f64 {
    10f64.powf(db.clamp(0.0, 36.0) as f64 / 20.0) - 1.0
}

impl Module for Drive {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &DRIVE_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let sr = self.sample_rate;
        let input = io.input(0);
        let g_target = drive_g(io.param(0).at(0));
        let trim = 10f32.powf(io.param(1).at(0).clamp(-24.0, 12.0) / 20.0);
        let mix = (io.param(2).at(0) / 100.0).clamp(0.0, 1.0);
        let c = &mut self.c;
        if c.g < 0.0 {
            c.g = g_target;
            c.engage = (g_target > 0.0) as u8 as f32;
            (c.trim, c.mix) = (trim, mix);
        }
        let engage_target = (g_target > 0.0) as u8 as f32;
        let k = 1.0 - (-1.0 / (CTRL_TAU_S * sr)).exp();
        let k_engage = 1.0 - (-1.0 / (ENGAGE_TAU_S * sr)).exp();
        // While engaging from 0 dB the curve starts at a small g, never 0 (no curve there).
        let floor = drive_g(0.5);
        let mut curve = Curve::new(c.g.max(floor));
        // F at the last input, for the current curve; recomputed only when needed.
        let mut f1 = None;
        for (i, o) in io.output(0)[..n].iter_mut().enumerate() {
            let x = input.at(i);
            let x0 = x as f64;
            if c.g != g_target {
                c.g = settle64(c.g + (g_target - c.g) * k as f64, g_target);
                curve = Curve::new(c.g.max(floor));
                f1 = None;
            }
            if c.engage != engage_target {
                c.engage = settle(
                    c.engage + (engage_target - c.engage) * k_engage,
                    engage_target,
                );
            }
            c.trim = settle(c.trim + (trim - c.trim) * k, trim);
            c.mix = settle(c.mix + (mix - c.mix) * k, mix);
            let shaped = if c.engage == 0.0 {
                f1 = None;
                x
            } else {
                let f_prev = f1.unwrap_or_else(|| curve.big_f(c.x1));
                let f0 = curve.big_f(x0);
                let dx = x0 - c.x1;
                let y = if dx.abs() < 1e-6 {
                    curve.f(0.5 * (x0 + c.x1))
                } else {
                    (f0 - f_prev) / dx
                } as f32;
                f1 = Some(f0);
                c.engage * y + (1.0 - c.engage) * x
            };
            c.x1 = x0;
            let wet = if c.mix >= 1.0 {
                shaped
            } else {
                x + c.mix * (shaped - x)
            };
            *o = if c.trim == 1.0 { wet } else { c.trim * wet };
        }
    }

    fn reset(&mut self) {
        self.c = Drive::new().c;
    }

    fn carry_from(&mut self, old: &dyn Module) {
        if let Some(o) = old.as_any().downcast_ref::<Drive>() {
            self.c = o.c;
        }
    }
}

#[inline]
fn settle(v: f32, target: f32) -> f32 {
    if (v - target).abs() < 1e-6 {
        target
    } else {
        v
    }
}

#[inline]
fn settle64(v: f64, target: f64) -> f64 {
    if (v - target).abs() < 1e-9 {
        target
    } else {
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adaa_of_the_curve_matches_its_mean_over_the_segment() {
        let c = Curve::new(3.0);
        let (a, b) = (0.2, 0.7);
        let steps = 100_000;
        let mean: f64 = (0..steps)
            .map(|i| c.f(a + (b - a) * (i as f64 + 0.5) / steps as f64))
            .sum::<f64>()
            / steps as f64;
        let adaa = (c.big_f(b) - c.big_f(a)) / (b - a);
        assert!((adaa - mean).abs() < 1e-9);
        assert!((c.f(1.0) - 1.0).abs() < 1e-12);
    }
}
