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

/// brief section 8. `prepare` may allocate (called on the control thread when a graph compiles);
/// `process` may not (audio thread, brief section 3's RT rules).
pub trait Module: Send {
    fn info(&self) -> &'static ModuleInfo;
    fn prepare(&mut self, sample_rate: f32, max_block: usize, quality: &QualityConfig);
    fn process(&mut self, io: &mut ProcessIo);
    fn reset(&mut self);
    fn save_state(&self, _out: &mut dyn StateWriter) {}
    fn load_state(&mut self, _s: &dyn StateReader) {}
}
