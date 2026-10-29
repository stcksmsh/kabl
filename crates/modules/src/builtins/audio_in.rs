//! `audio.in`: sound from outside the patch: the standalone's input device, or the host's
//! audio input to the plugin. Two outputs, `left` and `right`; a mono source plays on both.
//!
//! The host hands each block over before the graph runs (`feed`, called by the engine, which
//! keeps the last block it was given). Nothing arrives until a host feeds it: the outputs are
//! silent.
//!
//! **Untrusted input.** A device or host can deliver anything. Each sample passes `gain_db`
//! and is then read as 0 when not finite and clamped to ±16, so silence, clipping, NaN or an
//! unplugged device never reach the patch as anything worse than a loud, finite signal.
//!
//! One instance, shared by all voices.

use super::fx::{param, port, Glide, NEPER_PER_DB};
use crate::info::{
    Category, ModuleInfo, ParamInfo, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};

const PORTS: &[PortInfo] = &[PortInfo {
    width: 2,
    ..port("out", PortType::Audio, false)
}];

const PARAMS: &[ParamInfo] = &[param("gain_db", -24.0, 36.0, 0.0, "dB", Taper::Linear)];

pub static AUDIO_IN_INFO: ModuleInfo = ModuleInfo {
    kind: "audio.in",
    name: "Audio in",
    category: Category::Source,
    rate: Rate::Global,
    explain: "Brings in sound from outside: a microphone in the standalone, the host's audio input in the plugin. Patch it to a vocoder's mod to make a synth speak.",
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

const LIMIT: f32 = 16.0;
const GLIDE_TAU_S: f32 = 0.02;

pub struct AudioIn {
    buf: [Vec<f32>; 2],
    gain: Glide,
    sample_rate: f32,
}

impl AudioIn {
    pub fn new() -> Self {
        AudioIn {
            buf: [Vec::new(), Vec::new()],
            gain: Glide::default(),
            sample_rate: 48000.0,
        }
    }

    /// Takes the next block (the shorter of `left`, `right` and the prepared block; a missing
    /// right side repeats the left). Audio thread: copies, never allocates.
    pub fn feed(&mut self, left: &[f32], right: &[f32]) {
        let [l, r] = &mut self.buf;
        let n = l.len().min(left.len());
        l[..n].copy_from_slice(&left[..n]);
        l[n..].fill(0.0);
        let right = if right.is_empty() { left } else { right };
        let n = r.len().min(right.len());
        r[..n].copy_from_slice(&right[..n]);
        r[n..].fill(0.0);
    }
}

impl Default for AudioIn {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for AudioIn {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &AUDIO_IN_INFO
    }

    fn prepare(&mut self, sample_rate: f32, max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
        for b in &mut self.buf {
            b.resize(max_block, 0.0);
        }
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len().min(self.buf[0].len());
        let k = super::fx::coef(GLIDE_TAU_S, self.sample_rate);
        let gain = io.param(0);
        for i in 0..n {
            let g = (self.gain.next(gain, i, k).clamp(-24.0, 36.0) * NEPER_PER_DB).exp();
            for ch in 0..2 {
                let x = self.buf[ch][i] * g;
                io.output(ch)[i] = if x.is_finite() {
                    x.clamp(-LIMIT, LIMIT)
                } else {
                    0.0
                };
            }
        }
        for ch in 0..2 {
            io.output(ch)[n..].fill(0.0);
        }
    }

    fn reset(&mut self) {
        for b in &mut self.buf {
            b.fill(0.0);
        }
        self.gain = Glide::default();
    }
}
