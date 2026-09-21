//! Spike S2 (brief section 11): "Does block-held scalar classification meet the potato gate?"
//!
//! A stand-in for the brief's `potato` benchmark patch (section 13) — 20 modules, 4 voices —
//! built from five of the nine v1 built-ins (brief section 8) per voice: `osc.va` (`Saw`),
//! `filter.svf` (`Svf`), `env.adsr` (`Adsr`), `vca` and `ringmod` (both a single multiply, no
//! dedicated struct needed). 5 modules × 4 voices = 20 voice-rate modules, plus one global `lfo`
//! feeding both the filter's cutoff modulation and the ring-mod depth, a `mixer`, and `out` — 3
//! global-rate modules on top, matching brief section 7's voice-rate/global-rate split.
//!
//! This is a hand-rolled fixed topology, not the general flat-schedule compiler (still v1
//! milestone scope) — same simplification S1 made for its 2-node graph. What's genuinely under
//! test here is the control-rate tier itself: `process_naive` computes the LFO and the ADSR at
//! full per-sample rate regardless of how slowly they're actually changing; `process_optimized`
//! classifies them exactly as brief section 7 describes — "LFOs below ~100 Hz" and "envelopes in
//! sustain" — and holds them as block-scalars instead.

use crate::dsp::{Adsr, Lfo, Saw, Svf};
use crate::graph::BLOCK;

pub const NUM_VOICES: usize = 4;
pub const VOICE_HZ: [f32; NUM_VOICES] = [110.0, 130.81, 164.81, 220.0];
pub const BASE_CUTOFF_HZ: f32 = 1500.0;
pub const CUTOFF_MOD_HZ: f32 = 800.0;
pub const RESONANCE: f32 = 0.3;
/// Well under the brief's named "~100 Hz" control-rate threshold.
pub const LFO_RATE_HZ: f32 = 2.0;
pub const ATTACK_MS: f32 = 20.0;

struct Voice {
    osc: Saw,
    filter: Svf,
    adsr: Adsr,
}

impl Voice {
    fn new(sample_rate: f32) -> Self {
        Voice {
            osc: Saw::new(),
            filter: Svf::new(),
            adsr: Adsr::new(ATTACK_MS, sample_rate),
        }
    }
}

pub struct PotatoPatch {
    voices: [Voice; NUM_VOICES],
    lfo: Lfo,
    sample_rate: f32,
}

impl PotatoPatch {
    pub fn new(sample_rate: f32) -> Self {
        PotatoPatch {
            voices: std::array::from_fn(|_| Voice::new(sample_rate)),
            lfo: Lfo::new(),
            sample_rate,
        }
    }

    /// Every signal treated as if it needed full audio-rate handling: the LFO expanded into a
    /// per-sample buffer (64 `sin()` calls/block instead of 1), the ADSR's one-pole recurrence
    /// re-run every sample even once it's settled at the sustain level. No allocation either way
    /// — "naive" here means "no control-rate classification," not "no buffer pool."
    #[inline]
    pub fn process_naive(&mut self, out: &mut [f32; BLOCK]) {
        let mut lfo_buf = [0f32; BLOCK];
        for slot in lfo_buf.iter_mut() {
            *slot = self.lfo.next(LFO_RATE_HZ, self.sample_rate);
        }

        for s in out.iter_mut() {
            *s = 0.0;
        }
        for (voice, freq) in self.voices.iter_mut().zip(VOICE_HZ.iter()) {
            for (sample, &lfo_v) in out.iter_mut().zip(lfo_buf.iter()) {
                let osc_v = voice.osc.next(*freq, self.sample_rate);
                let cutoff = BASE_CUTOFF_HZ + CUTOFF_MOD_HZ * lfo_v;
                let filt_v =
                    voice
                        .filter
                        .process_lowpass(osc_v, cutoff, RESONANCE, self.sample_rate);
                let env = voice.adsr.next(1.0); // always recompute, even once settled
                let ring_gain = 0.5 + 0.5 * lfo_v;
                *sample += filt_v * env * ring_gain;
            }
        }
        for s in out.iter_mut() {
            *s /= NUM_VOICES as f32;
        }
    }

    /// Control-rate tier applied: the LFO is classified as a block-held scalar (one `sin()`
    /// call/block, shared by all 4 voices' cutoff modulation and ring-mod depth), and each
    /// voice's ADSR is classified per block from its *current* state — once settled at the
    /// sustain level, its recurrence is skipped for the block and the held `level` reused.
    #[inline]
    pub fn process_optimized(&mut self, out: &mut [f32; BLOCK]) {
        let lfo_scalar = self
            .lfo
            .next_block_held(LFO_RATE_HZ, self.sample_rate, BLOCK);
        let cutoff = BASE_CUTOFF_HZ + CUTOFF_MOD_HZ * lfo_scalar;
        let ring_gain = 0.5 + 0.5 * lfo_scalar;

        for s in out.iter_mut() {
            *s = 0.0;
        }
        for (voice, freq) in self.voices.iter_mut().zip(VOICE_HZ.iter()) {
            let settled = voice.adsr.is_settled(1.0);
            let held_env = voice.adsr.level;
            for sample in out.iter_mut() {
                let osc_v = voice.osc.next(*freq, self.sample_rate);
                let filt_v =
                    voice
                        .filter
                        .process_lowpass(osc_v, cutoff, RESONANCE, self.sample_rate);
                let env = if settled {
                    held_env
                } else {
                    voice.adsr.next(1.0)
                };
                *sample += filt_v * env * ring_gain;
            }
        }
        for s in out.iter_mut() {
            *s /= NUM_VOICES as f32;
        }
    }
}
