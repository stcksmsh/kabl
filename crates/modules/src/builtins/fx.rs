//! Small helpers shared by the 1.0 effects and filters (phaser, flanger, compressor, eq,
//! limiter, bitcrusher, filter.comb, filter.formant): const constructors for the info tables,
//! a control glide that follows knob steps but passes audio-rate signals straight through, and
//! the equal-gain dry/wet blend.

use crate::info::{ParamInfo, PortDirection, PortInfo, PortType, Taper};
use crate::io::Signal;

pub(super) const fn param(
    name: &'static str,
    min: f32,
    max: f32,
    default: f32,
    unit: &'static str,
    taper: Taper,
) -> ParamInfo {
    ParamInfo {
        name,
        min,
        max,
        default,
        unit,
        taper,
        smoothing_ms: 0.0,
    }
}

pub(super) const fn port(name: &'static str, port_type: PortType, input: bool) -> PortInfo {
    PortInfo {
        name,
        port_type,
        direction: if input {
            PortDirection::Input
        } else {
            PortDirection::Output
        },
        width: 1,
    }
}

/// The stereo jacks every effect here shares: `in` and `out`, width 2 (io slots 0 and 1).
pub(super) const STEREO_IN: PortInfo = PortInfo {
    width: 2,
    ..port("in", PortType::Audio, true)
};
pub(super) const STEREO_OUT: PortInfo = PortInfo {
    width: 2,
    ..port("out", PortType::Audio, false)
};

/// 20/ln 10 and ln 10/20: dB per neper and back.
pub(super) const DB_PER_NEPER: f32 = 20.0 * std::f32::consts::LOG10_E;
pub(super) const NEPER_PER_DB: f32 = std::f32::consts::LN_10 / 20.0;

/// One-pole smoothing coefficient for time constant `tau_s`.
#[inline]
pub(super) fn coef(tau_s: f32, sr: f32) -> f32 {
    1.0 - (-1.0 / (tau_s * sr)).exp()
}

/// Follows a control. A held (scalar) value glides to its target, so a knob step or a
/// block-rate edit never zippers; a genuine per-sample buffer (a cable into the param) is used
/// as it is, so audio-rate modulation is not smoothed away. Snaps onto the target once within
/// float noise, so endpoints are exact and unchanged controls compare equal.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Glide {
    v: f32,
    set: bool,
}

impl Glide {
    #[inline]
    pub fn next(&mut self, s: Signal, i: usize, k: f32) -> f32 {
        self.to(s.at(i), s.is_scalar(), k)
    }

    #[inline]
    pub fn to(&mut self, t: f32, held: bool, k: f32) -> f32 {
        if !self.set || !held {
            self.v = t;
            self.set = true;
        } else {
            self.v += (t - self.v) * k;
            if (t - self.v).abs() <= 1e-6 * (1.0 + t.abs()) {
                self.v = t;
            }
        }
        self.v
    }
}

/// States this small are flushed to 0 (a decaying loop must not go subnormal).
#[inline]
pub(super) fn flush(x: f32) -> f32 {
    if x.abs() < 1e-20 {
        0.0
    } else {
        x
    }
}

/// Dry/wet: exactly `dry` at 0, exactly `wet` at 1.
#[inline]
pub(super) fn blend(dry: f32, wet: f32, mix: f32) -> f32 {
    dry * (1.0 - mix) + wet * mix
}

/// Sine of a 0..1 phase.
#[inline]
pub(super) fn sin1(p: f32) -> f32 {
    (std::f32::consts::TAU * p).sin()
}

/// A time constant in ms turned into a per-sample coefficient, recomputed only when the time
/// changes (an exp per sample otherwise).
#[derive(Debug, Clone, Copy)]
pub(super) struct TimeCoef {
    ms: f32,
    pub k: f32,
}

impl TimeCoef {
    pub const fn new() -> Self {
        TimeCoef { ms: -1.0, k: 0.0 }
    }

    /// Coefficient `exp(-1/(ms·sr))`: the per-sample retention of a one-pole of that time.
    #[inline]
    pub fn get(&mut self, ms: f32, sr: f32) -> f32 {
        if ms != self.ms {
            self.ms = ms;
            self.k = (-1.0 / (ms.max(0.01) * 0.001 * sr)).exp();
        }
        self.k
    }
}
