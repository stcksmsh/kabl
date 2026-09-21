//! `Patch`/`Voice`: the hand-wired 4-voice `Module`-trait patch shared by
//! `tests/patch_integration.rs` (correctness + WAV render) and `benches/patch_integration.rs`
//! (ns/block). See the test file's module doc for the full rationale — this is a stand-in for
//! the real compiler's graph assembly, not application code, kept in `engine`'s public surface
//! only so the test and bench binaries (separate crates) can both reach it. Expect it to be
//! deleted once the real compiler can build this same topology from ops instead of Rust code.

use kabl_modules::builtins::{EnvAdsr, FilterSvf, MidiIn, Mixer, OscVa, Out, Vca};
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{Module, ProcessIo, Signal};

pub const SAMPLE_RATE: f32 = 48000.0;
pub const BLOCK: usize = 64;
const BASE_HZ: f32 = 261.63; // C4
pub const CHORD_SEMITONES: [f32; 4] = [0.0, 4.0, 7.0, 12.0]; // C major: C4, E4, G4, C5

fn quality() -> QualityConfig {
    QualityConfig {
        tier: QualityTier::Live,
    }
}

pub struct Voice {
    midi: MidiIn,
    osc: OscVa,
    filter: FilterSvf,
    env: EnvAdsr,
    vca: Vca,
}

impl Voice {
    fn new() -> Self {
        Voice {
            midi: MidiIn::new(),
            osc: OscVa::new(),
            filter: FilterSvf::new(),
            env: EnvAdsr::new(),
            vca: Vca::new(),
        }
    }

    fn prepare(&mut self) {
        let q = quality();
        self.midi.prepare(SAMPLE_RATE, BLOCK, &q);
        self.osc.prepare(SAMPLE_RATE, BLOCK, &q);
        self.filter.prepare(SAMPLE_RATE, BLOCK, &q);
        self.env.prepare(SAMPLE_RATE, BLOCK, &q);
        self.vca.prepare(SAMPLE_RATE, BLOCK, &q);
    }

    #[inline]
    fn process_block(&mut self) -> [f32; BLOCK] {
        let mut gate = [0f32; BLOCK];
        let mut pitch = [0f32; BLOCK];
        let mut velocity = [0f32; BLOCK];
        {
            let mut outputs: [&mut [f32]; 3] = [&mut gate, &mut pitch, &mut velocity];
            let mut io = ProcessIo::new(&[], &mut outputs, &[], BLOCK);
            self.midi.process(&mut io);
        }

        let mut osc_out = [0f32; BLOCK];
        {
            // sync unconnected (Scalar(0.0), never crosses the >0.5 edge threshold); waveform
            // 2.0 = Saw, matching this hand-wired demo's original (pre-waveform-param) sound.
            let inputs = [Signal::Buffer(&pitch), Signal::Scalar(0.0)];
            let params = [Signal::Scalar(BASE_HZ), Signal::Scalar(2.0)];
            let mut outputs: [&mut [f32]; 1] = [&mut osc_out];
            let mut io = ProcessIo::new(&inputs, &mut outputs, &params, BLOCK);
            self.osc.process(&mut io);
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
            self.filter.process(&mut io);
        }

        let mut env_out = [0f32; BLOCK];
        {
            let inputs = [Signal::Buffer(&gate)];
            let params = [
                Signal::Scalar(10.0),  // attack_ms
                Signal::Scalar(80.0),  // decay_ms
                Signal::Scalar(0.7),   // sustain
                Signal::Scalar(300.0), // release_ms
            ];
            let mut outputs: [&mut [f32]; 1] = [&mut env_out];
            let mut io = ProcessIo::new(&inputs, &mut outputs, &params, BLOCK);
            self.env.process(&mut io);
        }

        let mut vca_out = [0f32; BLOCK];
        {
            // gain param = 0.0: amplitude comes entirely from the envelope via the cv input,
            // not a fixed knob — the envelope is the only thing shaping loudness here.
            let inputs = [Signal::Buffer(&lp), Signal::Buffer(&env_out)];
            let params = [Signal::Scalar(0.0), Signal::Scalar(0.0)];
            let mut outputs: [&mut [f32]; 1] = [&mut vca_out];
            let mut io = ProcessIo::new(&inputs, &mut outputs, &params, BLOCK);
            self.vca.process(&mut io);
        }

        vca_out
    }
}

pub struct Patch {
    voices: [Voice; 4],
    mixer: Mixer,
    out: Out,
}

impl Patch {
    pub fn new() -> Self {
        Patch {
            voices: std::array::from_fn(|_| Voice::new()),
            mixer: Mixer::new(),
            out: Out::new(),
        }
    }

    pub fn prepare(&mut self) {
        for voice in &mut self.voices {
            voice.prepare();
        }
        self.mixer.prepare(SAMPLE_RATE, BLOCK, &quality());
        self.out.prepare(SAMPLE_RATE, BLOCK, &quality());
    }

    pub fn note_on_chord(&mut self) {
        for (voice, &semitones) in self.voices.iter_mut().zip(CHORD_SEMITONES.iter()) {
            voice.midi.note_on(semitones, 0.9);
        }
    }

    pub fn note_off_chord(&mut self) {
        for voice in &mut self.voices {
            voice.midi.note_off();
        }
    }

    /// Single-voice control, for a melody rather than a held chord — same `midi.in` mechanism
    /// (`note_on_chord` in a loop is this, called on all 4 voices at once).
    pub fn note_on(&mut self, voice: usize, semitones: f32, velocity: f32) {
        self.voices[voice].midi.note_on(semitones, velocity);
    }

    pub fn note_off(&mut self, voice: usize) {
        self.voices[voice].midi.note_off();
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
            let params = [Signal::Scalar(0.25); 4]; // 1/4 per voice, simple headroom management
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

impl Default for Patch {
    fn default() -> Self {
        Self::new()
    }
}
