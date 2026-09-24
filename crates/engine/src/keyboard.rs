//! The keyboard: turns key events (note on/off, sustain pedal, All Notes Off) into per-voice
//! play/release actions for one `midi.in`, by its settings (POLY, MONO, LEGATO; note priority;
//! glide). The rules are docs/sound-palette-batch/keyboard.md; `tests/keyboard.rs` checks them
//! event by event. Fixed-size state, no allocation: it runs on the audio thread, inside
//! `PatchEngine`, outside any graph, so live edits neither lose nor replay a key.

use kabl_core::ModuleId;
use kabl_modules::builtins::KeySettings;

/// Most voices a keyboard assigns (the engine runs 8).
pub const MAX_VOICES: usize = 16;
/// MIDI note that plays `osc.va`'s `base_hz` (semitone 0).
pub const BASE_NOTE: i32 = 60;

const POLY: u8 = 0;
const MONO: u8 = 1;
const LAST: u8 = 0;
const LOW: u8 = 1;
const ALWAYS: u8 = 1;
const GLIDE_LEGATO: u8 = 2;

/// A key event from a MIDI input (channel ignored).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyEvent {
    On {
        note: u8,
        velocity: u8,
    },
    Off {
        note: u8,
    },
    /// CC 64: down at ≥ 64.
    Sustain(bool),
    /// CC 123 / CC 120, the panel's All notes off, a disconnect.
    AllOff,
}

impl KeyEvent {
    /// Parses a raw MIDI message: note on/off (velocity 0 = off), CC 64, 120, 123. Anything
    /// else is `None`.
    pub fn from_midi(data: &[u8]) -> Option<KeyEvent> {
        if data.len() < 3 {
            return None;
        }
        let (note, value) = (data[1] & 0x7F, data[2] & 0x7F);
        match data[0] & 0xF0 {
            0x90 if value > 0 => Some(KeyEvent::On {
                note,
                velocity: value,
            }),
            0x90 | 0x80 => Some(KeyEvent::Off { note }),
            0xB0 if data[1] == 64 => Some(KeyEvent::Sustain(value >= 64)),
            0xB0 if data[1] == 120 || data[1] == 123 => Some(KeyEvent::AllOff),
            _ => None,
        }
    }
}

/// What a keyboard asks of its `midi.in` voices.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    /// `MidiIn::play`: pitch in semitones from note 60, velocity 0..1.
    Play {
        voice: usize,
        pitch: f32,
        velocity: f32,
        glide: bool,
        retrigger: bool,
    },
    Release {
        voice: usize,
    },
}

#[derive(Clone, Copy, Debug, Default)]
struct Voice {
    note: Option<u8>,
    /// The key is down (else the pedal holds it).
    key_down: bool,
    /// Event count when it last started a note, and when it last released.
    started: u64,
    released: u64,
    /// It has played a pitch (the first note of a voice never glides).
    played: bool,
}

pub struct Keyboard {
    pub id: ModuleId,
    settings: KeySettings,
    voices: [Voice; MAX_VOICES],
    n_voices: usize,
    /// Per note: 0 = up, else the event count when it went down.
    held: [u64; 128],
    velocity: [u8; 128],
    pedal: bool,
    clock: u64,
    /// Mono: the note sounding on voice 0, and whether its gate is high.
    sounding: Option<u8>,
    gate: bool,
}

fn pitch(note: u8) -> f32 {
    (note as i32 - BASE_NOTE) as f32
}

fn vel(v: u8) -> f32 {
    v as f32 / 127.0
}

impl Keyboard {
    pub fn new(id: ModuleId, settings: KeySettings, voices: usize) -> Self {
        Keyboard {
            id,
            settings,
            voices: [Voice::default(); MAX_VOICES],
            n_voices: voices.clamp(1, MAX_VOICES),
            held: [0; 128],
            velocity: [0; 128],
            pedal: false,
            clock: 0,
            sounding: None,
            gate: false,
        }
    }

    pub fn settings(&self) -> KeySettings {
        self.settings
    }

    /// Keys physically down.
    pub fn keys_held(&self) -> usize {
        self.held.iter().filter(|&&h| h > 0).count()
    }

    /// Voices with their gate high (keys down or held by the pedal).
    pub fn voices_sounding(&self) -> usize {
        if self.settings.mode == POLY {
            self.voices[..self.n_voices]
                .iter()
                .filter(|v| v.note.is_some())
                .count()
        } else {
            self.gate as usize
        }
    }

    /// New settings from the graph. A mode change releases every voice and forgets the keys
    /// (the pedal stays as it physically is); the rest apply from the next event.
    pub fn set_settings(&mut self, s: KeySettings, out: &mut impl FnMut(Action)) {
        if s.mode != self.settings.mode {
            self.release_everything(out);
        }
        self.settings = s;
    }

    pub fn event(&mut self, e: KeyEvent, out: &mut impl FnMut(Action)) {
        self.clock += 1;
        match e {
            KeyEvent::On { note, velocity } => {
                let note = note & 0x7F;
                if self.settings.mode == POLY {
                    self.poly_on(note, velocity, out);
                } else {
                    if self.held[note as usize] > 0 {
                        self.mono_off(note, out);
                        self.clock += 1;
                    }
                    self.mono_on(note, velocity, out);
                }
            }
            KeyEvent::Off { note } => {
                let note = note & 0x7F;
                if self.settings.mode == POLY {
                    self.poly_off(note, out);
                } else {
                    self.mono_off(note, out);
                }
            }
            KeyEvent::Sustain(down) => {
                self.pedal = down;
                if !down {
                    self.pedal_up(out);
                }
            }
            KeyEvent::AllOff => {
                self.release_everything(out);
                self.pedal = false;
            }
        }
    }

    /// Releases every sounding voice and forgets the keys.
    fn release_everything(&mut self, out: &mut impl FnMut(Action)) {
        let clock = self.clock;
        for (i, v) in self.voices[..self.n_voices].iter_mut().enumerate() {
            if v.note.take().is_some() {
                v.released = clock;
                out(Action::Release { voice: i });
            }
            v.key_down = false;
        }
        if std::mem::take(&mut self.gate) {
            out(Action::Release { voice: 0 });
        }
        self.sounding = None;
        self.held = [0; 128];
    }

    fn poly_on(&mut self, note: u8, velocity: u8, out: &mut impl FnMut(Action)) {
        self.held[note as usize] = self.clock;
        let n = self.n_voices;
        let voices = &mut self.voices[..n];
        // The same key still sounding: same voice, envelope restart.
        if let Some(i) = voices.iter().position(|v| v.note == Some(note)) {
            voices[i].key_down = true;
            out(Action::Play {
                voice: i,
                pitch: pitch(note),
                velocity: vel(velocity),
                glide: false,
                retrigger: true,
            });
            return;
        }
        let oldest = |pred: &dyn Fn(&Voice) -> bool, key: &dyn Fn(&Voice) -> u64| {
            (0..n)
                .filter(|&i| pred(&voices[i]))
                .min_by_key(|&i| (key(&voices[i]), i))
        };
        let i = oldest(&|v| v.note.is_none(), &|v| v.released)
            .or_else(|| oldest(&|v| !v.key_down, &|v| v.started))
            .or_else(|| oldest(&|_| true, &|v| v.started))
            .unwrap_or(0);
        let v = &mut voices[i];
        let glide = self.settings.glide == ALWAYS && v.played;
        *v = Voice {
            note: Some(note),
            key_down: true,
            started: self.clock,
            released: v.released,
            played: true,
        };
        out(Action::Play {
            voice: i,
            pitch: pitch(note),
            velocity: vel(velocity),
            glide,
            // A stolen voice, or one released earlier in this block, restarts its envelope
            // (`MidiIn::play` dips only a gate that is or was just high).
            retrigger: true,
        });
    }

    fn poly_off(&mut self, note: u8, out: &mut impl FnMut(Action)) {
        self.held[note as usize] = 0;
        let clock = self.clock;
        let pedal = self.pedal;
        for (i, v) in self.voices[..self.n_voices].iter_mut().enumerate() {
            if v.note == Some(note) {
                if pedal {
                    v.key_down = false;
                } else {
                    (v.note, v.key_down, v.released) = (None, false, clock);
                    out(Action::Release { voice: i });
                }
            }
        }
    }

    fn pedal_up(&mut self, out: &mut impl FnMut(Action)) {
        let clock = self.clock;
        if self.settings.mode == POLY {
            for (i, v) in self.voices[..self.n_voices].iter_mut().enumerate() {
                if v.note.is_some() && !v.key_down {
                    (v.note, v.released) = (None, clock);
                    out(Action::Release { voice: i });
                }
            }
        } else if self.gate && self.keys_held() == 0 {
            self.gate = false;
            self.sounding = None;
            out(Action::Release { voice: 0 });
        }
    }

    /// The held key that sounds under the priority rule.
    fn pick(&self) -> Option<u8> {
        let held = (0..128u8).filter(|&n| self.held[n as usize] > 0);
        match self.settings.priority {
            LAST => held.max_by_key(|&n| self.held[n as usize]),
            LOW => held.min(),
            _ => held.max(),
        }
    }

    fn mono_on(&mut self, note: u8, velocity: u8, out: &mut impl FnMut(Action)) {
        let others = self.keys_held() > 0;
        self.held[note as usize] = self.clock;
        self.velocity[note as usize] = velocity;
        if self.pick() == Some(note) {
            self.sound(note, others, out);
        }
    }

    fn mono_off(&mut self, note: u8, out: &mut impl FnMut(Action)) {
        if self.held[note as usize] == 0 {
            return;
        }
        self.held[note as usize] = 0;
        if self.sounding != Some(note) {
            return;
        }
        if let Some(p) = self.pick() {
            self.sound(p, true, out);
        } else if !self.pedal {
            self.gate = false;
            self.sounding = None;
            out(Action::Release { voice: 0 });
        }
    }

    /// `note` sounds on voice 0. `legato`: another key was down at this change.
    fn sound(&mut self, note: u8, legato: bool, out: &mut impl FnMut(Action)) {
        // LEGATO keeps the envelope running between overlapping notes; anything else restarts
        // it (a no-op for a gate that is and was low: `MidiIn::play`).
        let retrigger = !(self.gate && self.settings.mode != MONO && legato);
        let glide = match self.settings.glide {
            ALWAYS => true,
            GLIDE_LEGATO => legato,
            _ => false,
        };
        self.gate = true;
        self.sounding = Some(note);
        out(Action::Play {
            voice: 0,
            pitch: pitch(note),
            velocity: vel(self.velocity[note as usize]),
            glide,
            retrigger,
        });
    }
}
