//! The `Module` trait (brief section 8) — "one module interface for built-in, composite and
//! code modules" (brief section 4.1, foundational decision 5). `QualityConfig`/`QualityTier`
//! and `StateWriter`/`StateReader` are minimal stand-ins: brief section 7 specifies the full
//! quality-tier lever table (oversampling, filter model, interpolation, ...) and section 7.5
//! specifies `ModuleId`-keyed save/load dispatched through a compiler that doesn't exist yet.
//! Building those out fully now would be speculating ahead of the compiler that will actually
//! drive them — kept minimal on purpose (brief section 17: "cheap now, rewrite later" is
//! section 4's framing for foundational decisions; this isn't one of the six, so treat it as
//! ordinary scope discipline, section 2: build what's needed to make `Module` real and testable,
//! not the whole quality-tier system speculatively).

use crate::info::ModuleInfo;
use crate::io::ProcessIo;

/// Brief section 7's three tiers. Per-lever user overrides ("plus user overrides per lever")
/// aren't modeled yet — no UI exists to set them, and no module needs to honour them until one
/// does; `QualitySupport` in `ModuleInfo` already says which levers a module touches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityTier {
    Live,
    Studio,
    Render,
}

#[derive(Debug, Clone, Copy)]
pub struct QualityConfig {
    pub tier: QualityTier,
}

/// Minimal key/value state carry-over (brief section 7.5: "carry module state across recompiles
/// by stable ModuleId"). A real serialization format is compiler/patch-format work, not
/// `Module`-trait work — this is deliberately just enough for a module to say "here's my float
/// state" and get it back, matching what S1's spike hand-rolled (`CompiledGraph::
/// recompiled_with_depth` cloning oscillator phase/filter state directly) but through a trait
/// instead of bespoke per-graph-shape code.
pub trait StateWriter {
    fn write_f32(&mut self, key: &str, value: f32);
}

pub trait StateReader {
    fn read_f32(&self, key: &str) -> Option<f32>;
}

/// Maximum entries and key length `StateBuf` holds. Every built-in writes at most 7 keys of at
/// most 13 bytes; `tests/registry.rs` checks each one fits.
pub const STATE_BUF_ENTRIES: usize = 12;
pub const STATE_BUF_KEY_LEN: usize = 16;

/// Fixed-capacity, non-allocating `StateWriter`/`StateReader`. Lets the audio thread carry
/// module state into a new graph at the moment its crossfade starts, instead of from a stale
/// control-thread snapshot. Writes past capacity (or keys longer than `STATE_BUF_KEY_LEN`) are
/// dropped and counted in `overflowed`.
pub struct StateBuf {
    keys: [[u8; STATE_BUF_KEY_LEN]; STATE_BUF_ENTRIES],
    key_lens: [u8; STATE_BUF_ENTRIES],
    values: [f32; STATE_BUF_ENTRIES],
    len: usize,
    pub overflowed: usize,
}

impl Default for StateBuf {
    fn default() -> Self {
        StateBuf {
            keys: [[0; STATE_BUF_KEY_LEN]; STATE_BUF_ENTRIES],
            key_lens: [0; STATE_BUF_ENTRIES],
            values: [0.0; STATE_BUF_ENTRIES],
            len: 0,
            overflowed: 0,
        }
    }
}

impl StateBuf {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn find(&self, key: &str) -> Option<usize> {
        let k = key.as_bytes();
        (0..self.len).find(|&i| &self.keys[i][..self.key_lens[i] as usize] == k)
    }
}

impl StateWriter for StateBuf {
    fn write_f32(&mut self, key: &str, value: f32) {
        if let Some(i) = self.find(key) {
            self.values[i] = value;
            return;
        }
        let k = key.as_bytes();
        if self.len == STATE_BUF_ENTRIES || k.len() > STATE_BUF_KEY_LEN {
            self.overflowed += 1;
            return;
        }
        self.keys[self.len][..k.len()].copy_from_slice(k);
        self.key_lens[self.len] = k.len() as u8;
        self.values[self.len] = value;
        self.len += 1;
    }
}

impl StateReader for StateBuf {
    fn read_f32(&self, key: &str) -> Option<f32> {
        self.find(key).map(|i| self.values[i])
    }
}

/// brief section 8. `prepare` may allocate (called on the control thread when a graph compiles);
/// `process` may not (audio thread, brief section 3's RT rules).
///
/// `Module: 'static` (via `Any`'s requirement) so `as_any`/`as_any_mut` can downcast a
/// `Box<dyn Module>` back to its concrete type — needed by anything that has to reach a
/// module-specific method through a compiled patch's type-erased storage: a MIDI router calling
/// `MidiIn::note_on` on the right instance, or the (unbuilt) audio callback reading `Out::left`/
/// `right`. Not default-implemented: a blanket `{ self }` body needs `Self: Sized`, which makes
/// the method uncallable through `dyn Module` — the entire point here — so every impl provides
/// its own two-line `{ self }`/`{ self }` body (mechanical, not meaningfully different per
/// module).
pub trait Module: Send + std::any::Any {
    fn info(&self) -> &'static ModuleInfo;
    fn prepare(&mut self, sample_rate: f32, max_block: usize, quality: &QualityConfig);
    fn process(&mut self, io: &mut ProcessIo);
    fn reset(&mut self);
    fn save_state(&self, _out: &mut dyn StateWriter) {}
    fn load_state(&mut self, _s: &dyn StateReader) {}

    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}
