//! The compiled, audio-thread-owned graph for spike S1: a 4-voice chord (`Saw` per voice)
//! summed, scaled by a cable's `depth`, into one shared `Svf` lowpass. This stands in for the
//! general flat-schedule compiler (brief section 7), which is v1 milestone scope, not spike
//! scope — the spike only needs to prove a graph can be swapped with state carried over.

use kabl_modules::dsp::{Saw, Svf};

pub const BLOCK: usize = 64;
pub const NUM_VOICES: usize = 4;
pub const CUTOFF_HZ: f32 = 2000.0;
pub const RESONANCE: f32 = 0.3;

/// A-minor-ish chord: A2, C3, E3, A3.
pub const CHORD_HZ: [f32; NUM_VOICES] = [110.0, 130.81, 164.81, 220.0];

/// Owns real heap memory (not just for realism — this is also what proves `basedrop`'s deferred
/// drop actually matters: without it, dropping this on the audio thread would deallocate).
pub struct CompiledGraph {
    pub cable_depth: f32,
    voices: [Saw; NUM_VOICES],
    filter: Svf,
    /// Unused by processing; exists so the graph owns a heap allocation, exercising deferred
    /// drop via `basedrop::Shared` (see `swap.rs`) instead of testing an all-Copy struct.
    _scratch_pool: Vec<f32>,
}

impl CompiledGraph {
    pub fn new(cable_depth: f32) -> Self {
        CompiledGraph {
            cable_depth,
            voices: [Saw::new(); NUM_VOICES],
            filter: Svf::new(),
            _scratch_pool: vec![0.0; BLOCK * 8],
        }
    }

    /// Builds a new graph with a different `cable_depth`, carrying over oscillator phase and
    /// filter state from `self` — brief section 7.5, "carry module state across recompiles by
    /// stable ModuleId, so a ringing filter survives a repatch." Hand-written for this fixed
    /// graph shape; the general `ModuleId`-keyed save/load mechanism is v1 milestone work.
    pub fn recompiled_with_depth(&self, new_depth: f32) -> CompiledGraph {
        CompiledGraph {
            cable_depth: new_depth,
            voices: self.voices,
            filter: self.filter,
            _scratch_pool: vec![0.0; BLOCK * 8],
        }
    }

    /// No allocation, no locks, no syscalls — the RT rule (brief section 3). Callers wrap this
    /// in `assert_no_alloc!` in test/debug builds to prove it.
    #[inline]
    pub fn process_block(&mut self, sample_rate: f32, out: &mut [f32; BLOCK]) {
        for sample in out.iter_mut() {
            let mut mix = 0.0f32;
            for (voice, freq) in self.voices.iter_mut().zip(CHORD_HZ.iter()) {
                mix += voice.next(*freq, sample_rate);
            }
            mix *= self.cable_depth / NUM_VOICES as f32;
            *sample = self
                .filter
                .process_lowpass(mix, CUTOFF_HZ, RESONANCE, sample_rate);
        }
    }
}
