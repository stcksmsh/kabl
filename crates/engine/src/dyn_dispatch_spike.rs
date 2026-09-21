//! Spike, per `docs/STATUS.md`'s handover: measure `dyn Module` dispatch cost before the real
//! compiler gets built around it. `patch_demo.rs`'s `Voice` holds concrete typed fields (`MidiIn`,
//! `OscVa`, ...) — static dispatch, inlinable. The real compiler needs a heterogeneous collection
//! of module instances (a patch mixes arbitrary module kinds), which means `Box<dyn Module>` and
//! virtual dispatch. This is the same topology, same wiring, same params — the only difference is
//! `Voice`'s fields are `Box<dyn Module>` and every `process()` call goes through a vtable. Kept
//! side-by-side with `patch_demo.rs` (not replacing it) so `benches/dyn_dispatch_spike.rs` can
//! compare them under identical conditions. Expect this file to be deleted once the real compiler
//! exists and uses `Box<dyn Module>` for real, not as a spike.

use kabl_modules::builtins::{EnvAdsr, FilterSvf, MidiIn, Mixer, OscVa, Out, Vca};
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{Module, ProcessIo, Signal};

use crate::patch_demo::{BLOCK, CHORD_SEMITONES, SAMPLE_RATE};

const BASE_HZ: f32 = 261.63; // C4

fn quality() -> QualityConfig {
    QualityConfig {
        tier: QualityTier::Live,
    }
}

/// Same 5 modules as `patch_demo::Voice`, boxed as trait objects instead of concrete fields.
pub struct DynVoice {
    modules: [Box<dyn Module>; 5],
}

const MIDI: usize = 0;
const OSC: usize = 1;
const FILTER: usize = 2;
const ENV: usize = 3;
const VCA: usize = 4;

impl DynVoice {
    /// `note_on` isn't part of the `Module` trait (brief section 8 doesn't put it there — it's
    /// midi.in-specific control-thread plumbing, same as `patch_demo.rs`). Rather than adding a
    /// downcast path to `Module` just for this spike, the note is set on the concrete `MidiIn`
    /// before it's boxed — matches how a real compiler would set initial state on the control
    /// thread before handing the graph to the audio thread. `process_block` below is the part
    /// that actually goes through `dyn Module` dispatch on every call, which is what this spike
    /// measures.
    fn new(semitones: f32, velocity: f32) -> Self {
        let mut midi = MidiIn::new();
        midi.note_on(semitones, velocity);
        DynVoice {
            modules: [
                Box::new(midi),
                Box::new(OscVa::new()),
                Box::new(FilterSvf::new()),
                Box::new(EnvAdsr::new()),
                Box::new(Vca::new()),
            ],
        }
    }

    fn prepare(&mut self) {
        let q = quality();
        for m in &mut self.modules {
            m.prepare(SAMPLE_RATE, BLOCK, &q);
        }
    }

    #[inline]
    fn process_block(&mut self) -> [f32; BLOCK] {
        let mut gate = [0f32; BLOCK];
        let mut pitch = [0f32; BLOCK];
        let mut velocity = [0f32; BLOCK];
        {
            let mut outputs: [&mut [f32]; 3] = [&mut gate, &mut pitch, &mut velocity];
            let mut io = ProcessIo::new(&[], &mut outputs, &[], BLOCK);
            self.modules[MIDI].process(&mut io);
        }

        let mut osc_out = [0f32; BLOCK];
        {
            // sync unconnected; waveform 2.0 = Saw, matching patch_demo.rs's identical topology.
            let inputs = [Signal::Buffer(&pitch), Signal::Scalar(0.0)];
            let params = [Signal::Scalar(BASE_HZ), Signal::Scalar(2.0)];
            let mut outputs: [&mut [f32]; 1] = [&mut osc_out];
            let mut io = ProcessIo::new(&inputs, &mut outputs, &params, BLOCK);
            self.modules[OSC].process(&mut io);
        }

        let mut lp = [0f32; BLOCK];
        let mut bp = [0f32; BLOCK];
        let mut hp = [0f32; BLOCK];
        {
            let inputs = [
                Signal::Buffer(&osc_out),
                Signal::Scalar(0.0),
                Signal::Scalar(0.0),
            ];
            let params = [Signal::Scalar(2000.0), Signal::Scalar(0.3)];
            let mut outputs: [&mut [f32]; 3] = [&mut lp, &mut bp, &mut hp];
            let mut io = ProcessIo::new(&inputs, &mut outputs, &params, BLOCK);
            self.modules[FILTER].process(&mut io);
        }

        let mut env_out = [0f32; BLOCK];
        {
            let inputs = [Signal::Buffer(&gate)];
            let params = [
                Signal::Scalar(10.0),
                Signal::Scalar(80.0),
                Signal::Scalar(0.7),
                Signal::Scalar(300.0),
            ];
            let mut outputs: [&mut [f32]; 1] = [&mut env_out];
            let mut io = ProcessIo::new(&inputs, &mut outputs, &params, BLOCK);
            self.modules[ENV].process(&mut io);
        }

        let mut vca_out = [0f32; BLOCK];
        {
            let inputs = [Signal::Buffer(&lp), Signal::Buffer(&env_out)];
            let params = [Signal::Scalar(0.0), Signal::Scalar(0.0)];
            let mut outputs: [&mut [f32]; 1] = [&mut vca_out];
            let mut io = ProcessIo::new(&inputs, &mut outputs, &params, BLOCK);
            self.modules[VCA].process(&mut io);
        }

        vca_out
    }
}

pub struct DynPatch {
    voices: [DynVoice; 4],
    mixer: Box<dyn Module>,
    out: Box<dyn Module>,
}

impl DynPatch {
    pub fn new() -> Self {
        DynPatch {
            voices: std::array::from_fn(|i| DynVoice::new(CHORD_SEMITONES[i], 0.9)),
            mixer: Box::new(Mixer::new()),
            out: Box::new(Out::new()),
        }
    }

    pub fn prepare(&mut self) {
        for voice in &mut self.voices {
            voice.prepare();
        }
        self.mixer.prepare(SAMPLE_RATE, BLOCK, &quality());
        self.out.prepare(SAMPLE_RATE, BLOCK, &quality());
    }

    #[inline]
    pub fn process_block(&mut self) -> [f32; BLOCK] {
        let voice_outs: [[f32; BLOCK]; 4] = std::array::from_fn(|i| self.voices[i].process_block());

        let mut mix_out = [0f32; BLOCK];
        {
            let inputs = [
                Signal::Buffer(&voice_outs[0]),
                Signal::Buffer(&voice_outs[1]),
                Signal::Buffer(&voice_outs[2]),
                Signal::Buffer(&voice_outs[3]),
            ];
            let params = [Signal::Scalar(0.25); 4];
            let mut outputs: [&mut [f32]; 1] = [&mut mix_out];
            let mut io = ProcessIo::new(&inputs, &mut outputs, &params, BLOCK);
            self.mixer.process(&mut io);
        }

        let inputs = [Signal::Buffer(&mix_out), Signal::Buffer(&mix_out)];
        let mut outputs: [&mut [f32]; 0] = [];
        let mut io = ProcessIo::new(&inputs, &mut outputs, &[], BLOCK);
        self.out.process(&mut io);

        mix_out
    }
}

impl Default for DynPatch {
    fn default() -> Self {
        Self::new()
    }
}
