//! The production timing adapter (docs/midi-timing/design.md): host buffers of any length and
//! timestamped key events in, the engine's fixed 64-sample blocks on one absolute sample grid
//! out, at a fixed latency of `LATENCY` frames. The engine sees the same sequence of events
//! (with their sample offsets) and `process_block` calls whatever the host partition, so the
//! output does not depend on it: feedback delay, block-rate modulation, ramps, clocks and random
//! state all advance per engine block, and engine blocks never move.
//!
//! Fixed-size storage; no allocation, locks or I/O: it runs in the audio callback.

use crate::graph::BLOCK;
use crate::keyboard::{KeyEvent, MidiEvent, Source, MAX_SOURCES};
use crate::patch_engine::PatchEngine;

/// Frames from an event's time (or an input frame's time) to the output frame that carries it.
pub const LATENCY: usize = BLOCK;
/// Most events waiting for their block.
pub const QUEUE: usize = 1024;

/// What the queue did with events that did not fit the contract (design.md, section 3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Events accepted.
    pub queued: u64,
    /// Offset past the end of its host buffer: moved to the last frame.
    pub clamped: u64,
    /// Earlier than an event already queued: moved to that event's time.
    pub reordered: u64,
    /// Queue full: a starting event dropped.
    pub dropped: u64,
    /// Queue full: a release turned into its source's notes off at the next block.
    pub forced: u64,
}

pub struct Timeline {
    /// Timeline sample of the next host frame.
    now: u64,
    /// The engine block being played out.
    left: [f32; BLOCK],
    right: [f32; BLOCK],
    /// Events and their timeline sample, in time order (a ring).
    queue: [(u64, MidiEvent); QUEUE],
    head: usize,
    len: usize,
    /// Time of the newest queued event.
    last: u64,
    /// Releases that did not fit: per source, notes off at the next block; or a panic.
    forced: [bool; MAX_SOURCES],
    forced_panic: bool,
    stats: Stats,
}

const EMPTY: (u64, MidiEvent) = (
    0,
    MidiEvent {
        source: Source::Controller,
        channel: 0,
        event: KeyEvent::AllOff,
    },
);

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Timeline {
    pub fn new() -> Self {
        Timeline {
            now: 0,
            left: [0.0; BLOCK],
            right: [0.0; BLOCK],
            queue: [EMPTY; QUEUE],
            head: 0,
            len: 0,
            last: 0,
            forced: [false; MAX_SOURCES],
            forced_panic: false,
            stats: Stats::default(),
        }
    }

    /// Back to sample 0: no queued events, silence, zeroed counters. The engine is the
    /// caller's (build a fresh one, or send a panic).
    pub fn reset(&mut self) {
        *self = Timeline::new();
    }

    /// Timeline sample of the next host frame.
    pub fn now(&self) -> u64 {
        self.now
    }

    pub fn stats(&self) -> Stats {
        self.stats
    }

    /// Events waiting for their block.
    pub fn queued(&self) -> usize {
        self.len
    }

    /// Queues `e` at frame `offset` of the host buffer of `frames` frames about to be
    /// rendered (call before `render`, in time order). Out of range, out of order and full
    /// queue are handled as design.md section 3 says. No allocation.
    pub fn push(&mut self, offset: usize, frames: usize, e: MidiEvent) {
        let mut offset = offset;
        if offset >= frames.max(1) {
            offset = frames.max(1) - 1;
            self.stats.clamped += 1;
        }
        let mut t = self.now + offset as u64;
        if self.len > 0 && t < self.last {
            t = self.last;
            self.stats.reordered += 1;
        }
        if self.len == QUEUE {
            if e.is_release() {
                self.stats.forced += 1;
                if e.event == KeyEvent::AllOff {
                    self.forced_panic = true;
                } else {
                    self.forced[e.source as usize] = true;
                }
            } else {
                self.stats.dropped += 1;
            }
            return;
        }
        self.queue[(self.head + self.len) % QUEUE] = (t, e);
        self.len += 1;
        self.last = t;
        self.stats.queued += 1;
    }

    /// Fills `left`/`right` (same length, any length) with the next host frames. At each
    /// 64-sample grid point it renders the engine block that ended there: `before_block(engine,
    /// block start)` first, then that block's events at their offsets, then the block. No
    /// allocation.
    pub fn render(
        &mut self,
        engine: &mut PatchEngine,
        left: &mut [f32],
        right: &mut [f32],
        mut before_block: impl FnMut(&mut PatchEngine, u64),
    ) {
        let n = left.len().min(right.len());
        for i in 0..n {
            let h = self.now + i as u64;
            let at = (h % BLOCK as u64) as usize;
            if at == 0 && h >= BLOCK as u64 {
                self.block(engine, h - BLOCK as u64, &mut before_block);
            }
            left[i] = self.left[at];
            right[i] = self.right[at];
        }
        self.now += n as u64;
    }

    /// `push` every `(offset, event)` of `events`, then `render`.
    pub fn process(
        &mut self,
        engine: &mut PatchEngine,
        left: &mut [f32],
        right: &mut [f32],
        events: &[(usize, MidiEvent)],
        before_block: impl FnMut(&mut PatchEngine, u64),
    ) {
        let frames = left.len().min(right.len());
        for &(offset, e) in events {
            self.push(offset, frames, e);
        }
        self.render(engine, left, right, before_block);
    }

    /// Renders the engine block starting at timeline sample `start`.
    fn block(
        &mut self,
        engine: &mut PatchEngine,
        start: u64,
        before_block: &mut impl FnMut(&mut PatchEngine, u64),
    ) {
        before_block(engine, start);
        if std::mem::take(&mut self.forced_panic) {
            engine.key_at(MidiEvent::new(Source::Controller, 0, KeyEvent::AllOff), 0);
        }
        for (s, source) in [Source::Controller, Source::Preview, Source::Host]
            .into_iter()
            .enumerate()
        {
            if std::mem::take(&mut self.forced[s]) {
                for ch in 0..16 {
                    engine.key_at(MidiEvent::new(source, ch, KeyEvent::NotesOff), 0);
                }
            }
        }
        let end = start + BLOCK as u64;
        while self.len > 0 {
            let (t, e) = self.queue[self.head];
            if t >= end {
                break;
            }
            self.head = (self.head + 1) % QUEUE;
            self.len -= 1;
            engine.key_at(e, t.saturating_sub(start).min(BLOCK as u64 - 1) as usize);
        }
        engine.process_block(&mut self.left, &mut self.right);
    }
}
