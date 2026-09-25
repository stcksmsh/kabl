//! `noise`: white or pink noise, for breath, wind, percussive attacks and filtered texture.
//!
//! White is uniform in ±1 from a xorshift32 generator (Marsaglia 2003). Pink is that white
//! through Paul Kellet's "refined" pink filter (musicdsp.org, 2000, public domain): six
//! one-pole sections plus a direct term, within ±0.05 dB of −3 dB/octave above 9.2 Hz at
//! 44.1 kHz; at 48 and 96 kHz the poles sit slightly lower in relative frequency, which moves
//! the slope's low corner down, not the audible slope (measured in tests/noise.rs). Both
//! colours are scaled to the same RMS, −14 dBFS at 0 dB level (pink peaks reach about −2 dBFS), so switching colour changes the
//! tone, not the loudness.
//!
//! **Random state.** Each instance draws its own stream: the compiler seeds it from the module
//! id and the voice lane (`seed`), so two noise modules, or two voices of one, never play the
//! same samples, and a render is reproducible. A live edit carries the generator and the pink
//! filter into the new graph (`carry_from`) instead of reseeding, so the stream just continues.
//! `noise` is a voice-rate module. It runs once per voice (each voice its own stream, so a
//! chord adds in power) when a `midi.in` reaches it through a route, or when everything it
//! feeds is a MIDI voice chain (`compile.rs`: a noise chain whose every consumer is voiced is
//! voiced too). Anywhere else, including a noise that also feeds a path no MIDI reaches, it
//! compiles to one instance, seeded as lane 0, which is also the stream voice 0 would play, so
//! a chain switching between the two keeps lane 0's. (Use two noise modules for a per-voice
//! noise and a separate noise bed.)

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};

const PORTS: &[PortInfo] = &[PortInfo {
    name: "out",
    port_type: PortType::Audio,
    direction: PortDirection::Output,
}];

const PARAMS: &[ParamInfo] = &[
    // WHITE, PINK.
    ParamInfo {
        name: "color",
        min: 0.0,
        max: 1.0,
        default: 0.0,
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "level_db",
        min: -60.0,
        max: 0.0,
        default: 0.0,
        unit: "dB",
        taper: Taper::Linear,
        smoothing_ms: 20.0,
    },
];

pub static NOISE_INFO: ModuleInfo = ModuleInfo {
    kind: "noise",
    name: "Noise",
    category: Category::Source,
    rate: Rate::Voice,
    explain: "Random hiss: white (bright, even) or pink (softer, like wind or surf). Filter it for breath and air, gate it for hits.",
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

/// RMS of the output at 0 dB level.
const TARGET_RMS: f32 = 0.2;
/// Uniform white in ±1 has RMS 1/√3.
const WHITE_GAIN: f32 = TARGET_RMS * 1.732_050_8;
/// Kellet's filter output RMS for that white input, 1.760 (2·10⁷ samples; tests/noise.rs
/// checks the result).
const PINK_GAIN: f32 = TARGET_RMS / 1.760;
const TAU_S: f32 = 0.02;

#[derive(Debug, Clone, Copy)]
struct Core {
    rng: u32,
    /// Kellet's filter state.
    b: [f32; 7],
    /// Smoothed linear level; negative before the first block.
    g: f32,
}

pub struct Noise {
    c: Core,
    /// The seed this instance was given: its identity for `carry_from`.
    seed: u32,
    sample_rate: f32,
}

/// A nonzero xorshift32 state from any 64-bit seed (splitmix64 finalizer).
fn scramble(seed: u64) -> u32 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z as u32 ^ (z >> 32) as u32).max(1)
}

impl Noise {
    pub fn new() -> Self {
        let seed = scramble(0);
        Noise {
            c: Core {
                rng: seed,
                b: [0.0; 7],
                g: -1.0,
            },
            seed,
            sample_rate: 48000.0,
        }
    }

    /// Seeds the stream from the module id and voice lane (0 for one instance). Called by the
    /// compiler on a fresh instance, before any state is carried into it.
    pub fn seed(&mut self, module: u64, lane: usize) {
        self.seed = scramble(module.wrapping_mul(0x1_0000_0001) ^ ((lane as u64) << 48));
        self.c.rng = self.seed;
        self.c.b = [0.0; 7];
    }
}

impl Default for Noise {
    fn default() -> Self {
        Self::new()
    }
}

#[inline]
fn white(rng: &mut u32) -> f32 {
    *rng ^= *rng << 13;
    *rng ^= *rng >> 17;
    *rng ^= *rng << 5;
    // Top 24 bits → [-1, 1).
    (*rng >> 8) as f32 * (2.0 / 16_777_216.0) - 1.0
}

impl Module for Noise {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &NOISE_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let pink = io.param(0).at(0) >= 0.5;
        let target = 10f32.powf(io.param(1).at(0).clamp(-60.0, 0.0) / 20.0);
        let c = &mut self.c;
        if c.g < 0.0 {
            c.g = target;
        }
        let k = 1.0 - (-1.0 / (TAU_S * self.sample_rate)).exp();
        let mut g = c.g;
        for o in io.output(0)[..n].iter_mut() {
            g += (target - g) * k;
            let w = white(&mut c.rng);
            // The pink filter always runs, so switching colour never starts it from rest.
            let b = &mut c.b;
            b[0] = 0.99886 * b[0] + w * 0.055_517_9;
            b[1] = 0.99332 * b[1] + w * 0.075_075_9;
            b[2] = 0.969 * b[2] + w * 0.153_852;
            b[3] = 0.8665 * b[3] + w * 0.310_485_6;
            b[4] = 0.55 * b[4] + w * 0.532_952_2;
            b[5] = -0.7616 * b[5] - w * 0.016_898;
            let p = b[0] + b[1] + b[2] + b[3] + b[4] + b[5] + b[6] + w * 0.5362;
            b[6] = w * 0.115_926;
            *o = g * if pink { p * PINK_GAIN } else { w * WHITE_GAIN };
        }
        c.g = if (g - target).abs() < 1e-6 { target } else { g };
    }

    fn reset(&mut self) {
        self.c = Core {
            rng: self.seed,
            b: [0.0; 7],
            g: -1.0,
        };
    }

    /// Continues the stream of the same instance (same seed: same module, same lane). Another
    /// lane's stream is never copied, so voices stay independent.
    fn carry_from(&mut self, old: &dyn Module) {
        if let Some(o) = old.as_any().downcast_ref::<Noise>() {
            if o.seed == self.seed {
                self.c = o.c;
            }
        }
    }
}
