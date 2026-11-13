//! `demo.bitfilter` on the SDK: a stateful stereo voice filter. Bit-exact with
//! `crates/modules/code-examples/bitfilter`, with no `unsafe` and no layout code.
//!
//! Two cascaded RBJ low-pass biquads per channel (24 dB/oct), a soft-clip drive in front, and a
//! cutoff that follows `cutoff_cv` at one volt per octave (the block average). `resonance` sets
//! the Q of the first section. State is the four sections' two delay values per channel,
//! flushed of denormals after every block.
#![no_std]
#![forbid(unsafe_code)]

use kabl_module_sdk::*;

struct BitFilter {
    sample_rate: f32,
    /// `z[channel][section] = [z1, z2]`
    z: [[[f32; 2]; 2]; 2],
}

/// Butterworth Q. Deliberately 0.7071, not `FRAC_1_SQRT_2`, to stay bit-exact with the v0 blob.
#[allow(clippy::approx_constant)]
const Q: f32 = 0.7071;

const CUTOFF_CV: usize = 2;
const CUTOFF: usize = 0;
const RESONANCE: usize = 1;
const DRIVE: usize = 2;

impl Module for BitFilter {
    const META: Meta = Meta::new("demo.bitfilter", 1)
        .name("Bit Filter")
        .explain("A steep stereo low-pass with resonance and a soft-clip drive. The cutoff follows the cutoff input at one octave per unit.")
        .category(Category::Filter)
        .rate(Rate::Voice)
        .width(6)
        .port_in(PortType::Audio, "in", 2)
        .port_in(PortType::Cv, "cutoff_cv", 1)
        .port_out(PortType::Audio, "out", 2)
        .param("cutoff", "20", "20000", "2000", Taper::Exp, 5, "Hz")
        .param("resonance", "0", "1", "0.2", Taper::Linear, 5, "amt")
        .param("drive", "0", "1", "0", Taper::Linear, 5, "amt")
        .advanced("drive");
    const STATE_CAP: usize = 32;

    fn new(init: &mut Init) -> Self {
        BitFilter {
            sample_rate: init.sample_rate.clamp(8_000.0, 384_000.0),
            z: [[[0.0; 2]; 2]; 2],
        }
    }

    fn reset(&mut self) {
        self.z = [[[0.0; 2]; 2]; 2];
    }

    fn process(&mut self, io: &mut Io) {
        let n = io.n;
        if n == 0 {
            return;
        }
        let sr = self.sample_rate;
        let (cutoff, resonance, drive) = (io.param(CUTOFF), io.param(RESONANCE), io.param(DRIVE));
        let cv: f32 = io.input[CUTOFF_CV][..n].iter().sum();
        let fc = (cutoff * exp2(cv / n as f32)).clamp(5.0, 0.45 * sr);
        let q1 = Q + resonance.clamp(0.0, 1.0) * 9.0;
        let sections = [lowpass(fc, q1, sr), lowpass(fc, Q, sr)];
        let pre = 1.0 + 3.0 * drive.clamp(0.0, 1.0);
        let mix = drive.clamp(0.0, 1.0);
        for ch in 0..2 {
            let mut z = self.z[ch];
            for i in 0..n {
                let dry = io.input[ch][i];
                let mut x = dry * (1.0 - mix) + soft(dry * pre) * mix * 1.5;
                for (s, c) in z.iter_mut().zip(&sections) {
                    let y = c[0] * x + s[0];
                    s[0] = c[1] * x - c[3] * y + s[1];
                    s[1] = c[2] * x - c[4] * y;
                    x = y;
                }
                io.output[ch][i] = x;
            }
            for s in z.iter_mut() {
                s[0] = flush(s[0]);
                s[1] = flush(s[1]);
            }
            self.z[ch] = z;
        }
    }

    fn save(&self, w: &mut StateWriter) {
        for v in self.z.iter().flatten().flatten() {
            w.f32(*v);
        }
    }

    fn load(&mut self, _version: u32, bytes: &[u8]) -> bool {
        let mut r = StateReader::new(bytes);
        let mut z = [[[0.0; 2]; 2]; 2];
        for v in z.iter_mut().flatten().flatten() {
            match r.f32() {
                Some(x) if x.is_finite() => *v = x,
                _ => return false,
            }
        }
        self.z = z;
        true
    }
}

const _: () = {
    BitFilter::META.assert_in(0, "in");
    BitFilter::META.assert_in(CUTOFF_CV, "cutoff_cv");
    BitFilter::META.assert_out(0, "out");
    BitFilter::META.assert_param(CUTOFF, "cutoff");
    BitFilter::META.assert_param(RESONANCE, "resonance");
    BitFilter::META.assert_param(DRIVE, "drive");
};

/// Low-pass biquad coefficients `[b0, b1, b2, a1, a2]`, already divided by a0.
fn lowpass(fc: f32, q: f32, sr: f32) -> [f32; 5] {
    let w0 = core::f32::consts::TAU * fc / sr;
    let (sin, cos) = sin_cos(w0);
    let alpha = sin / (2.0 * q);
    let a0 = 1.0 + alpha;
    let b1 = (1.0 - cos) / a0;
    [b1 * 0.5, b1, b1 * 0.5, -2.0 * cos / a0, (1.0 - alpha) / a0]
}

fn soft(x: f32) -> f32 {
    x / (1.0 + x.abs())
}

kabl_module!(BitFilter);
