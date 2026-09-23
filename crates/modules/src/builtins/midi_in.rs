//! `midi.in` (brief section 8: "per-voice gate, pitch, velocity"). A source module with no
//! `Module` input ports — its state is set by `note_on`/`note_off`, called by whatever actually
//! reads MIDI (`crates/standalone`'s `midir` integration, not built yet — this is a stand-in
//! that lets a patch be driven programmatically until real MIDI plumbing exists, same relation
//! S1's spike graphs had to the real compiler). Monophonic-per-instance, last-note priority; a
//! voice allocator (brief section 7's "voice allocator," not built yet) is what would eventually
//! route MIDI notes to instances of this across a polyphonic patch.
//!
//! Outputs are held constant across each block (no ramping) — a note-on is a discrete event, and
//! nothing here yet has the timing information to know it should land mid-block rather than at a
//! block boundary; that's the real MIDI/scheduling plumbing's job once it exists.

use crate::info::{Category, ModuleInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "gate",
        port_type: PortType::Gate,
        direction: PortDirection::Output,
    },
    PortInfo {
        name: "pitch",
        port_type: PortType::Pitch,
        direction: PortDirection::Output,
    },
    PortInfo {
        name: "velocity",
        port_type: PortType::UnipolarCv,
        direction: PortDirection::Output,
    },
];

pub static MIDI_IN_INFO: ModuleInfo = ModuleInfo {
    kind: "midi.in",
    name: "MIDI In",
    category: Category::Source,
    rate: Rate::Voice,
    explain:
        "Turns a played note into gate, pitch, and velocity signals the rest of the patch can use.",
    lesson: None,
    requires: &[],
    ports: PORTS,
    params: &[],
    quality: QualitySupport {
        oversampling: false,
        anti_aliasing: false,
        interpolation: false,
    },
    skin: None,
    width_units: 5,
    advanced: &[],
};

const GATE: usize = 0;
const PITCH: usize = 1;
const VELOCITY: usize = 2;

pub struct MidiIn {
    gate: f32,
    /// Semitones from the same reference `osc.va`'s `pitch` input expects (brief section 8:
    /// "float semitones, 1V/oct semantics") — 0.0 here corresponds to `osc.va`'s `base_hz` param.
    pitch: f32,
    velocity: f32,
}

impl MidiIn {
    pub fn new() -> Self {
        MidiIn {
            gate: 0.0,
            pitch: 0.0,
            velocity: 0.0,
        }
    }

    pub fn note_on(&mut self, pitch_semitones: f32, velocity: f32) {
        self.gate = 1.0;
        self.pitch = pitch_semitones;
        self.velocity = velocity.clamp(0.0, 1.0);
    }

    pub fn note_off(&mut self) {
        self.gate = 0.0;
    }
}

impl Default for MidiIn {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for MidiIn {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &MIDI_IN_INFO
    }

    fn prepare(&mut self, _sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {}

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let block_len = io.block_len();
        let (gate, pitch, velocity) = (self.gate, self.pitch, self.velocity);
        io.output(GATE)[..block_len].fill(gate);
        io.output(PITCH)[..block_len].fill(pitch);
        io.output(VELOCITY)[..block_len].fill(velocity);
    }

    fn reset(&mut self) {
        self.gate = 0.0;
        self.pitch = 0.0;
        self.velocity = 0.0;
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        out.write_f32("gate", self.gate);
        out.write_f32("pitch", self.pitch);
        out.write_f32("velocity", self.velocity);
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        if let Some(v) = s.read_f32("gate") {
            self.gate = v;
        }
        if let Some(v) = s.read_f32("pitch") {
            self.pitch = v;
        }
        if let Some(v) = s.read_f32("velocity") {
            self.velocity = v;
        }
    }
}
