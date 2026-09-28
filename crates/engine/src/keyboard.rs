//! The keyboard: turns key events (note on/off, sustain pedal, bend, wheel, notes off, panic)
//! into per-voice play/release/expression actions for one `midi.in`, by its settings (POLY,
//! MONO, LEGATO; note priority; glide; channel). The rules are
//! docs/sound-palette-batch/keyboard.md and, for identity, routing and expression,
//! docs/midi-timing/design.md section 7; `tests/keyboard.rs` and `tests/expression.rs` check
//! them event by event. Fixed-size state, no allocation: it runs on the audio thread, inside
//! `PatchEngine`, outside any graph, so live edits neither lose nor replay a key.

use kabl_core::ModuleId;
use kabl_modules::builtins::KeySettings;

/// Most voices a keyboard assigns (the engine runs 8).
pub const MAX_VOICES: usize = 16;
/// MIDI note that plays `osc.va`'s `base_hz` (semitone 0).
pub const BASE_NOTE: i32 = 60;
/// Most keys a keyboard remembers as held; beyond, the oldest is forgotten.
pub const MAX_HELD: usize = 32;
/// Sources a keyboard tells apart (`Source`).
pub const MAX_SOURCES: usize = 3;

const POLY: u8 = 0;
const MONO: u8 = 1;
const LAST: u8 = 0;
const LOW: u8 = 1;
const ALWAYS: u8 = 1;
const GLIDE_LEGATO: u8 = 2;
const ALL_CHANNELS: u16 = 0xFFFF;

/// Where a key event comes from. Part of a note's identity: a source's release never
/// releases another source's key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Source {
    /// The standalone app's MIDI input.
    Controller = 0,
    /// The app's own audition (D01): reaches every keyboard, whatever its channel.
    Preview = 1,
    /// A plugin host (D07).
    Host = 2,
}

/// A key event, without its source and channel (`MidiEvent`).
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
    /// Panic: every note of every source releases, every pedal comes up (the panel's All
    /// notes off, a legacy `PatchEngine::key`).
    AllOff,
    /// CC 120 / CC 123: this source and channel's notes release, its pedal comes up.
    NotesOff,
    /// Pitch bend, 14 bit: 0..=16383, 8192 = center.
    Bend(u16),
    /// Modulation wheel, CC 1: 0..=127.
    Wheel(u8),
    /// CC 121: this source and channel's bend to center, wheel to 0, pedal up.
    ResetControllers,
    /// The source went away (a disconnect): its notes, pedals and expression end.
    SourceLost,
}

impl KeyEvent {
    /// Parses a raw MIDI message the way the app did before D06: note on/off (velocity 0 =
    /// off), CC 64, and CC 120/123 as a panic. Anything else is `None`. The channel is
    /// ignored; `MidiEvent::parse` keeps it.
    pub fn from_midi(data: &[u8]) -> Option<KeyEvent> {
        match MidiEvent::parse(Source::Controller, data)?.event {
            KeyEvent::NotesOff => Some(KeyEvent::AllOff),
            e @ (KeyEvent::On { .. } | KeyEvent::Off { .. } | KeyEvent::Sustain(_)) => Some(e),
            _ => None,
        }
    }
}

/// A key event with its source and channel (0..=15, MIDI channel 1..=16).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MidiEvent {
    pub source: Source,
    pub channel: u8,
    pub event: KeyEvent,
}

impl MidiEvent {
    pub fn new(source: Source, channel: u8, event: KeyEvent) -> Self {
        MidiEvent {
            source,
            channel: channel & 0x0F,
            event,
        }
    }

    /// Parses a raw channel message: note on/off (velocity 0 = off), pitch bend, CC 1, 64,
    /// 120, 121, 123. Anything else (other CCs included) is `None`.
    pub fn parse(source: Source, data: &[u8]) -> Option<MidiEvent> {
        if data.len() < 3 {
            return None;
        }
        let (d1, d2) = (data[1] & 0x7F, data[2] & 0x7F);
        let event = match data[0] & 0xF0 {
            0x90 if d2 > 0 => KeyEvent::On {
                note: d1,
                velocity: d2,
            },
            0x90 | 0x80 => KeyEvent::Off { note: d1 },
            0xE0 => KeyEvent::Bend(((d2 as u16) << 7) | d1 as u16),
            0xB0 => match d1 {
                1 => KeyEvent::Wheel(d2),
                64 => KeyEvent::Sustain(d2 >= 64),
                120 | 123 => KeyEvent::NotesOff,
                121 => KeyEvent::ResetControllers,
                _ => return None,
            },
            _ => return None,
        };
        Some(MidiEvent::new(source, data[0] & 0x0F, event))
    }

    /// An event that only ends something (a note, a pedal, a source): never dropped when a
    /// queue is full (docs/midi-timing/design.md, section 3).
    pub fn is_release(&self) -> bool {
        matches!(
            self.event,
            KeyEvent::Off { .. }
                | KeyEvent::Sustain(false)
                | KeyEvent::AllOff
                | KeyEvent::NotesOff
                | KeyEvent::ResetControllers
                | KeyEvent::SourceLost
        )
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
    /// Every voice of this keyboard: bend -1..1 (times the module's bend range), wheel 0..1.
    Expression {
        bend: f32,
        wheel: f32,
    },
}

/// A held key: its identity, velocity and when it went down.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Held {
    source: u8,
    channel: u8,
    note: u8,
    velocity: u8,
    at: u64,
}

#[derive(Clone, Copy, Debug, Default)]
struct Voice {
    /// (channel, note) sounding on it.
    key: Option<(u8, u8)>,
    /// Sources holding the key down (bit per `Source`); 0 while the pedal holds it.
    holders: u8,
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
    /// Keys down, oldest first.
    held: [Held; MAX_HELD],
    n_held: usize,
    /// Sustain pedal per source, bit n = channel n.
    pedal: [u16; MAX_SOURCES],
    clock: u64,
    /// Mono: the key sounding on voice 0 (source, channel, note), and whether its gate is high.
    sounding: Option<(u8, u8, u8)>,
    gate: bool,
    /// Bend -1..1 and wheel 0..1, with the (source, channel) that last set each.
    bend: f32,
    wheel: f32,
    bend_from: Option<(u8, u8)>,
    wheel_from: Option<(u8, u8)>,
}

fn pitch(note: u8) -> f32 {
    (note as i32 - BASE_NOTE) as f32
}

fn vel(v: u8) -> f32 {
    v as f32 / 127.0
}

/// 14-bit bend to -1..1: 8192 is exactly 0; each half scales to its own endpoint.
pub fn bend_norm(v: u16) -> f32 {
    let d = v.min(16383) as f32 - 8192.0;
    if d < 0.0 {
        d / 8192.0
    } else {
        d / 8191.0
    }
}

impl Keyboard {
    pub fn new(id: ModuleId, settings: KeySettings, voices: usize) -> Self {
        Keyboard {
            id,
            settings,
            voices: [Voice::default(); MAX_VOICES],
            n_voices: voices.clamp(1, MAX_VOICES),
            held: [Held::default(); MAX_HELD],
            n_held: 0,
            pedal: [0; MAX_SOURCES],
            clock: 0,
            sounding: None,
            gate: false,
            bend: 0.0,
            wheel: 0.0,
            bend_from: None,
            wheel_from: None,
        }
    }

    pub fn settings(&self) -> KeySettings {
        self.settings
    }

    /// Keys physically down.
    pub fn keys_held(&self) -> usize {
        self.n_held
    }

    /// Bend (-1..1) and wheel (0..1) now.
    pub fn expression(&self) -> (f32, f32) {
        (self.bend, self.wheel)
    }

    /// Voices with their gate high (keys down or held by the pedal).
    pub fn voices_sounding(&self) -> usize {
        if self.settings.mode == POLY {
            self.voices[..self.n_voices]
                .iter()
                .filter(|v| v.key.is_some())
                .count()
        } else {
            self.gate as usize
        }
    }

    /// New settings from the graph. A mode change releases every voice and forgets the keys
    /// (the pedal stays as it physically is); a channel change returns bend and wheel to rest;
    /// the rest apply from the next event.
    pub fn set_settings(&mut self, s: KeySettings, out: &mut impl FnMut(Action)) {
        if s.mode != self.settings.mode {
            self.release_everything(out);
        }
        if s.channel != self.settings.channel {
            (self.bend_from, self.wheel_from) = (None, None);
            self.set_expression(0.0, 0.0, out);
        }
        self.settings = s;
    }

    /// Does this keyboard take a note-on, bend, wheel or pedal-down from `e`? The preview
    /// reaches every keyboard; otherwise channel ALL or the one set.
    pub fn accepts(&self, e: &MidiEvent) -> bool {
        e.source == Source::Preview
            || self.settings.channel == 0
            || self.settings.channel == e.channel + 1
    }

    /// A controller event on channel 1 (the pre-D06 entry point).
    pub fn event(&mut self, e: KeyEvent, out: &mut impl FnMut(Action)) {
        self.midi(MidiEvent::new(Source::Controller, 0, e), out);
    }

    /// One event. Starts (note on, bend, wheel, pedal down) only if `accepts`; endings reach
    /// what this keyboard holds whatever its channel now (a changed route never strands a
    /// note).
    pub fn midi(&mut self, e: MidiEvent, out: &mut impl FnMut(Action)) {
        self.clock += 1;
        let (src, ch) = (e.source as u8, e.channel & 0x0F);
        let accepted = self.accepts(&e);
        match e.event {
            KeyEvent::On { note, velocity } if accepted => {
                let note = note & 0x7F;
                if self.settings.mode == POLY {
                    self.poly_on(src, ch, note, velocity, out);
                } else {
                    if self.find_held(src, ch, note).is_some() {
                        self.mono_off(src, ch, note, out);
                        self.clock += 1;
                    }
                    self.mono_on(src, ch, note, velocity, out);
                }
            }
            KeyEvent::Off { note } => {
                let note = note & 0x7F;
                if self.settings.mode == POLY {
                    self.poly_off(src, ch, note, out);
                } else {
                    self.mono_off(src, ch, note, out);
                }
            }
            KeyEvent::Sustain(true) if accepted => self.pedal[src as usize] |= 1 << ch,
            KeyEvent::Sustain(false) => self.pedal_up(src, ch, out),
            KeyEvent::AllOff => {
                self.release_everything(out);
                self.pedal = [0; MAX_SOURCES];
            }
            KeyEvent::NotesOff => self.end_keys(src, 1 << ch, out),
            KeyEvent::Bend(v) if accepted => {
                self.bend_from = Some((src, ch));
                self.set_expression(bend_norm(v), self.wheel, out);
            }
            KeyEvent::Wheel(v) if accepted => {
                self.wheel_from = Some((src, ch));
                self.set_expression(self.bend, (v & 0x7F) as f32 / 127.0, out);
            }
            KeyEvent::ResetControllers => {
                self.pedal_up(src, ch, out);
                self.reset_expression(|from| from == (src, ch), out);
            }
            KeyEvent::SourceLost => {
                self.end_keys(src, ALL_CHANNELS, out);
                self.reset_expression(|from| from.0 == src, out);
            }
            _ => {}
        }
    }

    fn set_expression(&mut self, bend: f32, wheel: f32, out: &mut impl FnMut(Action)) {
        if (bend, wheel) != (self.bend, self.wheel) {
            (self.bend, self.wheel) = (bend, wheel);
            out(Action::Expression { bend, wheel });
        }
    }

    /// Bend and/or wheel back to rest where `set_by` matches who set them.
    fn reset_expression(
        &mut self,
        set_by: impl Fn((u8, u8)) -> bool,
        out: &mut impl FnMut(Action),
    ) {
        let (mut bend, mut wheel) = (self.bend, self.wheel);
        if self.bend_from.is_some_and(&set_by) {
            (bend, self.bend_from) = (0.0, None);
        }
        if self.wheel_from.is_some_and(&set_by) {
            (wheel, self.wheel_from) = (0.0, None);
        }
        self.set_expression(bend, wheel, out);
    }

    fn pedal_down(&self, channel: u8) -> bool {
        self.pedal.iter().any(|p| p & (1 << channel) != 0)
    }

    fn find_held(&self, src: u8, ch: u8, note: u8) -> Option<usize> {
        self.held[..self.n_held]
            .iter()
            .position(|h| (h.source, h.channel, h.note) == (src, ch, note))
    }

    fn remove_held(&mut self, i: usize) {
        self.held.copy_within(i + 1..self.n_held, i);
        self.n_held -= 1;
    }

    /// Remembers a key as the newest held one (full: the oldest is forgotten).
    fn push_held(&mut self, src: u8, ch: u8, note: u8, velocity: u8) {
        if let Some(i) = self.find_held(src, ch, note) {
            self.remove_held(i);
        }
        if self.n_held == MAX_HELD {
            self.remove_held(0);
        }
        self.held[self.n_held] = Held {
            source: src,
            channel: ch,
            note,
            velocity,
            at: self.clock,
        };
        self.n_held += 1;
    }

    /// Releases every sounding voice and forgets the keys.
    fn release_everything(&mut self, out: &mut impl FnMut(Action)) {
        let clock = self.clock;
        for (i, v) in self.voices[..self.n_voices].iter_mut().enumerate() {
            if v.key.take().is_some() {
                v.released = clock;
                out(Action::Release { voice: i });
            }
            v.holders = 0;
        }
        if std::mem::take(&mut self.gate) {
            out(Action::Release { voice: 0 });
        }
        self.sounding = None;
        self.n_held = 0;
    }

    /// Notes off for source `src` on the channels in `channels` (by identity, whatever the
    /// routing): its keys there are forgotten, its pedals there come up, and what nothing
    /// else holds releases.
    fn end_keys(&mut self, src: u8, channels: u16, out: &mut impl FnMut(Action)) {
        let ends = |s: u8, ch: u8| s == src && channels & (1 << ch) != 0;
        self.pedal[src as usize] &= !channels;
        let mut i = 0;
        while i < self.n_held {
            let h = self.held[i];
            if ends(h.source, h.channel) {
                self.remove_held(i);
            } else {
                i += 1;
            }
        }
        let clock = self.clock;
        if self.settings.mode == POLY {
            let pedal = self.pedal;
            let down = |ch: u8| pedal.iter().any(|p| p & (1 << ch) != 0);
            for (i, v) in self.voices[..self.n_voices].iter_mut().enumerate() {
                let Some((ch, _)) = v.key else { continue };
                if !ends(src, ch) {
                    continue;
                }
                v.holders &= !(1 << src);
                if v.holders == 0 && !down(ch) {
                    (v.key, v.released) = (None, clock);
                    out(Action::Release { voice: i });
                }
            }
        } else if let Some(s) = self.sounding.filter(|_| self.gate) {
            if self.find_held(s.0, s.1, s.2).is_none() {
                if let Some(p) = self.pick() {
                    self.sound(p, true, out);
                } else if !self.pedal_down(s.1) {
                    self.gate = false;
                    self.sounding = None;
                    out(Action::Release { voice: 0 });
                }
            }
        }
    }

    fn poly_on(&mut self, src: u8, ch: u8, note: u8, velocity: u8, out: &mut impl FnMut(Action)) {
        self.push_held(src, ch, note, velocity);
        let n = self.n_voices;
        let voices = &mut self.voices[..n];
        // The same key still sounding (held by any source, or sustained): same voice,
        // envelope restart.
        if let Some(i) = voices.iter().position(|v| v.key == Some((ch, note))) {
            voices[i].holders |= 1 << src;
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
        let i = oldest(&|v| v.key.is_none(), &|v| v.released)
            .or_else(|| oldest(&|v| v.holders == 0, &|v| v.started))
            .or_else(|| oldest(&|_| true, &|v| v.started))
            .unwrap_or(0);
        let v = &mut voices[i];
        let glide = self.settings.glide == ALWAYS && v.played;
        // A stolen voice forgets its old holders: their later releases find nothing.
        *v = Voice {
            key: Some((ch, note)),
            holders: 1 << src,
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

    fn poly_off(&mut self, src: u8, ch: u8, note: u8, out: &mut impl FnMut(Action)) {
        if let Some(i) = self.find_held(src, ch, note) {
            self.remove_held(i);
        }
        let clock = self.clock;
        let pedal = self.pedal_down(ch);
        for (i, v) in self.voices[..self.n_voices].iter_mut().enumerate() {
            if v.key == Some((ch, note)) && v.holders & (1 << src) != 0 {
                v.holders &= !(1 << src);
                if v.holders == 0 && !pedal {
                    (v.key, v.released) = (None, clock);
                    out(Action::Release { voice: i });
                }
            }
        }
    }

    fn pedal_up(&mut self, src: u8, ch: u8, out: &mut impl FnMut(Action)) {
        self.pedal[src as usize] &= !(1 << ch);
        if self.pedal_down(ch) {
            return;
        }
        let clock = self.clock;
        if self.settings.mode == POLY {
            for (i, v) in self.voices[..self.n_voices].iter_mut().enumerate() {
                if v.key.is_some_and(|k| k.0 == ch) && v.holders == 0 {
                    (v.key, v.released) = (None, clock);
                    out(Action::Release { voice: i });
                }
            }
        } else if self.gate && self.keys_held() == 0 && self.sounding.is_some_and(|s| s.1 == ch) {
            self.gate = false;
            self.sounding = None;
            out(Action::Release { voice: 0 });
        }
    }

    /// The held key that sounds under the priority rule.
    fn pick(&self) -> Option<(u8, u8, u8)> {
        let held = self.held[..self.n_held].iter();
        let h = match self.settings.priority {
            LAST => held.max_by_key(|h| h.at),
            LOW => held.min_by_key(|h| h.note),
            _ => held.max_by_key(|h| h.note),
        };
        h.map(|h| (h.source, h.channel, h.note))
    }

    fn mono_on(&mut self, src: u8, ch: u8, note: u8, velocity: u8, out: &mut impl FnMut(Action)) {
        let others = self.keys_held() > 0;
        self.push_held(src, ch, note, velocity);
        if self.pick() == Some((src, ch, note)) {
            self.sound((src, ch, note), others, out);
        }
    }

    fn mono_off(&mut self, src: u8, ch: u8, note: u8, out: &mut impl FnMut(Action)) {
        let Some(i) = self.find_held(src, ch, note) else {
            return;
        };
        self.remove_held(i);
        if self.sounding != Some((src, ch, note)) {
            return;
        }
        if let Some(p) = self.pick() {
            self.sound(p, true, out);
        } else if !self.pedal_down(ch) {
            self.gate = false;
            self.sounding = None;
            out(Action::Release { voice: 0 });
        }
    }

    /// `key` sounds on voice 0. `legato`: another key was down at this change.
    fn sound(&mut self, key: (u8, u8, u8), legato: bool, out: &mut impl FnMut(Action)) {
        // LEGATO keeps the envelope running between overlapping notes; anything else restarts
        // it (a no-op for a gate that is and was low: `MidiIn::play`).
        let retrigger = !(self.gate && self.settings.mode != MONO && legato);
        let glide = match self.settings.glide {
            ALWAYS => true,
            GLIDE_LEGATO => legato,
            _ => false,
        };
        let velocity = self
            .find_held(key.0, key.1, key.2)
            .map_or(0, |i| self.held[i].velocity);
        self.gate = true;
        self.sounding = Some(key);
        out(Action::Play {
            voice: 0,
            pitch: pitch(key.2),
            velocity: vel(velocity),
            glide,
            retrigger,
        });
    }
}
