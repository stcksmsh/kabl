//! `midi.note`: one MIDI note number as a trigger. The engine hands it every note-on (like
//! `arp`); a note-on of its `note` (on `channel`, 0 = all; General MIDI drums are channel 10)
//! raises `gate` and sets `velocity` (0..1). One module per drum: 36 kick, 38 snare, 39 clap,
//! 42 closed hat, 46 open hat, 45 tom in the General MIDI map. The module is also a plain
//! "when this key is pressed" trigger for anything with a gate input.
//!
//! It is a global module and takes no part in voices, so its gate is not averaged over voice
//! lanes (a `midi.in` gate would be). Each hit is a 10 ms pulse starting on the sample of the
//! note-on (the host's or the controller's offset inside the block); the length is fixed because
//! a drum voice reacts to the rising edge only. A second hit while the pulse is still high drops
//! the gate for one sample first, so every hit has an edge. `velocity` takes the note's velocity
//! on that sample and holds it.
//!
//! The engine pushes events from the audio thread into a fixed queue of 32 per block (more are
//! dropped). A note that lands beyond the end of the block it was pushed for plays in the next.

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};

const QUEUE: usize = 32;
const PULSE_S: f32 = 0.010;

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "gate",
        port_type: PortType::Gate,
        direction: PortDirection::Output,
    },
    PortInfo {
        name: "velocity",
        port_type: PortType::UnipolarCv,
        direction: PortDirection::Output,
    },
];

const PARAMS: &[ParamInfo] = &[
    ParamInfo {
        name: "note",
        min: 0.0,
        max: 127.0,
        default: 36.0,
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    },
    // 0 = ALL, else MIDI channel 1..16.
    ParamInfo {
        name: "channel",
        min: 0.0,
        max: 16.0,
        default: 0.0,
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    },
];

pub static MIDI_NOTE_INFO: ModuleInfo = ModuleInfo {
    kind: "midi.note",
    name: "MIDI Note",
    category: Category::Sequencer,
    rate: Rate::Global,
    explain: "Fires a gate when one MIDI note is played, with its velocity. Set Note to the drum's key (36 kick, 38 snare, 39 clap, 42 closed hat, 46 open hat) and patch the gate into a drum voice.",
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
    advanced: &["channel"],
};

#[derive(Debug, Clone, Copy)]
struct Hit {
    offset: u32,
    velocity: f32,
}

pub struct MidiNote {
    /// Pending hits, ordered by offset.
    queue: [Hit; QUEUE],
    n: usize,
    /// Gate samples left.
    high: u32,
    velocity: f32,
    /// The params last seen by `process`, for `note_on`.
    note: u8,
    channel: u8,
    sample_rate: f32,
}

impl MidiNote {
    pub fn new() -> Self {
        MidiNote {
            queue: [Hit {
                offset: 0,
                velocity: 0.0,
            }; QUEUE],
            n: 0,
            high: 0,
            velocity: 0.0,
            note: 36,
            channel: 0,
            sample_rate: 48000.0,
        }
    }

    /// Audio thread: a note-on on MIDI channel `ch` (0..=15) at `offset` samples into the next
    /// block. No allocation.
    pub fn note_on(&mut self, offset: usize, ch: u8, note: u8, velocity: u8) {
        if note != self.note
            || (self.channel != 0 && self.channel != (ch & 0x0F) + 1)
            || self.n == QUEUE
        {
            return;
        }
        let hit = Hit {
            offset: offset as u32,
            velocity: f32::from(velocity.min(127)) / 127.0,
        };
        // After every hit at or before this offset (arrival order within an offset).
        let at = self.queue[..self.n].partition_point(|h| h.offset <= hit.offset);
        self.queue.copy_within(at..self.n, at + 1);
        self.queue[at] = hit;
        self.n += 1;
    }
}

impl Default for MidiNote {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for MidiNote {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn info(&self) -> &'static ModuleInfo {
        &MIDI_NOTE_INFO
    }
    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _q: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        self.note = io.param(0).at(0).round().max(0.0).min(127.0) as u8;
        self.channel = io.param(1).at(0).round().max(0.0).min(16.0) as u8;
        let pulse = ((PULSE_S * self.sample_rate) as u32).max(2);
        let mut next = 0;
        for i in 0..n {
            // A hit on this sample: a retrigger drops the gate for it, the pulse runs on after.
            let mut dip = false;
            while next < self.n && self.queue[next].offset as usize <= i {
                dip |= self.high > 0;
                self.high = pulse + u32::from(dip);
                self.velocity = self.queue[next].velocity;
                next += 1;
            }
            io.output(0)[i] = f32::from(u8::from(self.high > 0 && !dip));
            io.output(1)[i] = self.velocity;
            self.high = self.high.saturating_sub(1);
        }
        // Whatever lands beyond this block plays in the next one.
        let mut kept = 0;
        for j in next..self.n {
            let mut h = self.queue[j];
            h.offset = h.offset.saturating_sub(n as u32);
            self.queue[kept] = h;
            kept += 1;
        }
        self.n = kept;
    }

    fn reset(&mut self) {
        let sample_rate = self.sample_rate;
        *self = MidiNote::new();
        self.sample_rate = sample_rate;
    }

    fn carry_from(&mut self, old: &dyn Module) {
        if let Some(o) = old.as_any().downcast_ref::<MidiNote>() {
            self.high = o.high;
            self.velocity = o.velocity;
            self.note = o.note;
            self.channel = o.channel;
        }
    }
}
