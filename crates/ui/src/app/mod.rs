//! `kabl-ui` binary: the patchbay editor with live audio. Combines `PatchEditor` (patch editing
//! over the real op log) with the same `cpal`/`midir` wiring `kabl-standalone` uses, so editing a
//! patch here and hearing the result are the same window, not a separate player process.
//!
//! **Audio handoff is lock-free.** The audio callback owns the `PatchEngine` outright. Document
//! changes go through `kabl_ui::control::Delivery`: runtime values when only knob values or
//! route amounts changed, else a compiled graph wrapped for deferred drop (`basedrop::Owned`),
//! both through one bounded `rtrb` queue in revision order. The callback takes a bounded number
//! of messages per call (`PatchEngine::drain`); state carry and value changes are
//! allocation-free. No mutex on the audio thread. Retired graphs are freed on the UI thread by
//! `Collector::collect()` every frame.
//!
//! **Control without the editor (D04).** The document, the UI state and the delivery live in
//! one `Core` behind a mutex that only non-audio threads take: the UI for a frame, and the
//! control thread (`pump`) for MIDI CCs and retries. MIDI mappings, pickup and buttons run
//! there, so they reach the audio whether or not a frame is drawn.

#[cfg(test)]
mod alloc_count;
mod audio;
mod callback;
mod frame;
mod health;
mod midi;
mod retry;
mod session;
mod startup;
#[cfg(test)]
mod tests;
mod timing;

#[cfg(test)]
#[global_allocator]
static CALLBACK_ALLOCATOR: alloc_count::Counter = alloc_count::Counter;

pub(crate) use audio::*;
pub(crate) use callback::*;
pub(crate) use frame::*;
pub(crate) use health::*;
pub(crate) use midi::*;
pub(crate) use retry::*;
pub(crate) use session::*;
pub(crate) use startup::*;
pub(crate) use timing::*;
