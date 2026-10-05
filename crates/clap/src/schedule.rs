//! Bounded raw host schedule. Parameter and transport changes land on next 64-frame boundary.
use crate::automation::{Bank, SLOTS};
use kabl_engine::{
    keyboard::{KeyEvent, MidiEvent, Source},
    patch_engine::PatchEngine,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct Position {
    pub beats: f64,
    pub seconds: f64,
    pub tempo: f64,
    pub playing: bool,
    pub valid: bool,
}
impl Position {
    pub fn from_clap(t: Option<&clap_sys::events::clap_event_transport>) -> Self {
        use clap_sys::events::*;
        let Some(t) = t else {
            return Self::default();
        };
        let valid = t.flags
            & (CLAP_TRANSPORT_HAS_TEMPO
                | CLAP_TRANSPORT_HAS_BEATS_TIMELINE
                | CLAP_TRANSPORT_HAS_SECONDS_TIMELINE)
            == (CLAP_TRANSPORT_HAS_TEMPO
                | CLAP_TRANSPORT_HAS_BEATS_TIMELINE
                | CLAP_TRANSPORT_HAS_SECONDS_TIMELINE)
            && t.tempo.is_finite()
            && (20.0..=300.0).contains(&t.tempo);
        Self {
            beats: t.song_pos_beats as f64 / (1u64 << 31) as f64,
            seconds: t.song_pos_seconds as f64 / (1u64 << 31) as f64,
            tempo: t.tempo,
            playing: valid && t.flags & CLAP_TRANSPORT_IS_PLAYING != 0,
            valid,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub enum Event {
    Position(Position),
    Value {
        slot: usize,
        value: f32,
        modulation: bool,
    },
}
const EMPTY: (u64, Event) = (
    0,
    Event::Position(Position {
        beats: 0.0,
        seconds: 0.0,
        tempo: 0.0,
        playing: false,
        valid: false,
    }),
);
pub struct Schedule {
    queue: [(u64, Event); 4096],
    head: usize,
    len: usize,
    anchor: Option<(u64, Position)>,
    host: bool,
    mode_changed: bool,
    values: [f32; SLOTS],
    modulation: [f32; SLOTS],
    pub dropped: u64,
}
impl Schedule {
    pub fn new(values: [f32; SLOTS]) -> Self {
        Self {
            queue: [EMPTY; 4096],
            head: 0,
            len: 0,
            anchor: None,
            host: false,
            mode_changed: false,
            values,
            modulation: [0.0; SLOTS],
            dropped: 0,
        }
    }
    pub fn clear_time(&mut self) {
        self.head = 0;
        self.len = 0;
        self.anchor = None;
    }
    pub fn mode(&mut self, host: bool) {
        if host != self.host {
            self.host = host;
            self.mode_changed = true;
            self.anchor = None;
        }
    }
    pub fn push(&mut self, time: u64, event: Event) {
        if self.len == self.queue.len() {
            self.dropped += 1;
            return;
        }
        // Stable insertion merges raw transport and parameter lists without allocating.
        let mut index = self.len;
        while index > 0 && self.queue[(self.head + index - 1) % self.queue.len()].0 > time {
            self.queue[(self.head + index) % self.queue.len()] =
                self.queue[(self.head + index - 1) % self.queue.len()];
            index -= 1;
        }
        self.queue[(self.head + index) % self.queue.len()] = (time, event);
        self.len += 1;
    }
    pub fn block(&mut self, engine: &mut PatchEngine, start: u64, bank: &Bank, rate: f32) {
        if self.mode_changed {
            engine.reset();
            self.mode_changed = false;
        }
        while self.len > 0 && self.queue[self.head].0 <= start {
            let (time, event) = self.queue[self.head];
            self.head = (self.head + 1) % self.queue.len();
            self.len -= 1;
            match event {
                Event::Value {
                    slot,
                    value,
                    modulation,
                } if slot < SLOTS && value.is_finite() => {
                    if modulation {
                        self.modulation[slot] = value;
                    } else {
                        self.values[slot] = value.clamp(0.0, 1.0);
                    }
                }
                Event::Position(position) => {
                    if self.host {
                        let previous = self.anchor;
                        let play_start =
                            position.playing && previous.is_none_or(|(_, p)| !p.playing);
                        let seek = previous.is_some_and(|(at, p)| {
                            p.playing
                                && position.playing
                                && ((position.seconds - p.seconds) * rate as f64
                                    - (time - at) as f64)
                                    .abs()
                                    > 2.0
                        });
                        if play_start || seek {
                            // Reset at transport boundary, never at each MIDI note.
                            engine.reset();
                        } else if !position.playing && previous.is_some_and(|(_, p)| p.playing) {
                            engine.key_at(MidiEvent::new(Source::Host, 0, KeyEvent::SourceLost), 0);
                            engine.cancel(None);
                        }
                    }
                    self.anchor = Some((time, position));
                }
                _ => {}
            }
        }
        let position = if self.host {
            let (time, p) = self.anchor.unwrap_or((start, Position::default()));
            let beats = p.beats
                + if p.playing {
                    (start - time) as f64 / rate as f64 * p.tempo / 60.0
                } else {
                    0.0
                };
            Some((beats, if p.valid { p.tempo } else { 120.0 }, p.playing))
        } else {
            None
        };
        engine.host_clock(position);
        for (i, resolved) in bank.iter().enumerate() {
            if let Some(r) = resolved {
                let normalized = (self.values[i] + self.modulation[i]).clamp(0.0, 1.0);
                engine.automate(i, r.target, r.param.from_norm(normalized));
            }
        }
    }
}
