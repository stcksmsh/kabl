//! Runtime control values (D04): a knob, macro, pin or mapped CC changes the playing graph in
//! place instead of recompiling it. Rules and the full classification:
//! docs/runtime-controls/design.md.
//!
//! - **Identity.** A value names module id + kind + param index, or a route's cable id, and
//!   carries the document revision it belongs to (`ParamSet::rev`). A graph applies it only
//!   when it is newer than the revision the graph was compiled from (`CompiledPatch::rev`), so
//!   a delayed value cannot overwrite a newer graph, and a graph of another document (a higher
//!   revision) never takes an old one. The kind check stops a reused id of another kind.
//! - **Order.** Values and graphs share one FIFO queue (`ToAudio`), filled by one producer in
//!   revision order; a graph subsumes every value before it.
//! - **Classification.** `runtime_changes` diffs two document states the way the compiler
//!   reads them (effective values: stored, legacy alias, else default). Only values of
//!   runtime params and route amounts changed: those values. Anything else (modules, cables,
//!   bypass, keyboard configuration, a load): `None`, compile.

use std::sync::atomic::{AtomicU64, Ordering};

use basedrop::Owned;
use kabl_core::{CableId, ModuleId, PatchState, PortRef};
use kabl_modules::{registry, ModuleInfo};

use crate::compile::{CompiledPatch, DEFAULT_ROUTE_AMOUNT};

/// What a runtime value changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RuntimeTarget {
    /// The stored base of param `index` (`ModuleInfo::params`) of module `id`, of kind `kind`.
    Param {
        id: ModuleId,
        kind: &'static str,
        index: u16,
    },
    /// The signed amount of the route `cable`.
    Route { cable: CableId },
}

/// One runtime value: `target` becomes `value` at document revision `rev`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParamSet {
    pub rev: u64,
    pub target: RuntimeTarget,
    pub value: f32,
}

/// The control thread's queue to the audio thread: graphs and runtime values, in revision
/// order. The audio thread drops what it pops without freeing: a graph goes to the collector.
pub enum ToAudio {
    Graph(Owned<CompiledPatch>),
    Set(ParamSet),
}

/// What the audio thread reports back (relaxed atomics, written by `PatchEngine::drain`).
#[derive(Default)]
pub struct Feedback {
    /// Graphs taken from the queue (installed, queued behind a fade, or refused as stale).
    pub graphs_taken: AtomicU64,
    /// Newest revision applied: a graph installed or a value that resolved in a running graph.
    pub applied_rev: AtomicU64,
    /// Values taken from the queue.
    pub sets_taken: AtomicU64,
    /// Values that resolved in no running graph (their module is not in it yet or any more).
    pub unresolved: AtomicU64,
    /// Graphs refused because a newer one had already arrived.
    pub stale_graphs: AtomicU64,
}

impl Feedback {
    pub fn get(a: &AtomicU64) -> u64 {
        a.load(Ordering::Relaxed)
    }
}

/// Whether a param changes in place or needs a compile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamClass {
    /// Read by the module every block: a new value applies in place.
    Runtime,
    /// Configures something outside the per-block params: compile.
    Structural,
}

/// The class of param `name` of `kind`. Structural: `midi.in` mode, priority and glide (the
/// keyboard outside the graph reads them when a graph is installed: voice assignment and the
/// release a mode change makes). Every other param of every built-in is read each block in
/// `process`; design.md lists each one with its reason.
pub fn param_class(kind: &str, name: &str) -> ParamClass {
    match (kind, name) {
        ("midi.in", "mode" | "priority" | "glide") => ParamClass::Structural,
        _ => ParamClass::Runtime,
    }
}

/// Whether a runtime value of this param ramps (`compile::RAMP_MS`) in a playing graph, or is
/// set at once: stepped choices (never through invalid states), params the module smooths
/// itself (no double smoothing) and sequencer step data (read when a step plays: a ramp could
/// play a note between two values). design.md, "Smoothing".
pub fn ramped(kind: &str, p: &kabl_modules::ParamInfo) -> bool {
    p.taper != kabl_modules::Taper::Stepped && kind != "seq" && !smoothed_by_module(kind, p.name)
}

/// Params whose module smooths a change itself, so a runtime value is set, not ramped (no
/// double smoothing). Read from each module's `process` (design.md, "Smoothing").
pub fn smoothed_by_module(kind: &str, name: &str) -> bool {
    match kind {
        "gain" | "macro" | "drive" | "filter.ladder" => true,
        "chorus" => matches!(name, "depth" | "mix" | "width"),
        "delay" => matches!(name, "time_ms" | "feedback" | "mix"),
        "reverb" => true,
        "noise" => name == "level_db",
        "osc.va" => name == "pw",
        _ => false,
    }
}

/// The value the compiler uses for param `p` of a module with stored `params`.
fn effective(
    info: &ModuleInfo,
    params: &std::collections::BTreeMap<String, f32>,
    p: &kabl_modules::ParamInfo,
) -> f32 {
    params
        .get(p.name)
        .or_else(|| registry::legacy_param(info.kind, p.name).and_then(|old| params.get(old)))
        .copied()
        .unwrap_or(p.default)
}

fn bypassed(c: &kabl_core::CableState) -> bool {
    c.params.get("bypass").is_some_and(|&b| b >= 0.5)
}

/// The runtime values that take a graph compiled from `old` to what `new` compiles to, or
/// `None` when that needs a compile. Values the compiler does not read (presentation params,
/// labels, positions, a jack cable's params, a bypassed route's amount) are ignored. The
/// comparison is bitwise, so a value that did not change sends nothing.
pub fn runtime_changes(old: &PatchState, new: &PatchState) -> Option<Vec<(RuntimeTarget, f32)>> {
    if old.modules.len() != new.modules.len() || old.cables.len() != new.cables.len() {
        return None;
    }
    let mut out = Vec::new();
    for ((&id, a), (&id2, b)) in old.modules.iter().zip(&new.modules) {
        if id != id2 || a.kind != b.kind {
            return None;
        }
        if a.params == b.params {
            continue;
        }
        let info = registry::info_for(&b.kind)?;
        for (index, p) in info.params.iter().enumerate() {
            let (x, y) = (effective(info, &a.params, p), effective(info, &b.params, p));
            if x.to_bits() == y.to_bits() {
                continue;
            }
            if param_class(info.kind, p.name) == ParamClass::Structural {
                return None;
            }
            out.push((
                RuntimeTarget::Param {
                    id,
                    kind: info.kind,
                    index: index as u16,
                },
                y,
            ));
        }
    }
    for ((&id, a), (&id2, b)) in old.cables.iter().zip(&new.cables) {
        if id != id2 || a.from != b.from || a.to != b.to || bypassed(a) != bypassed(b) {
            return None;
        }
        if !matches!(b.to, PortRef::Param { .. }) || bypassed(b) {
            continue;
        }
        let amount = |c: &kabl_core::CableState| {
            c.params
                .get("amount")
                .copied()
                .unwrap_or(DEFAULT_ROUTE_AMOUNT)
        };
        if amount(a).to_bits() != amount(b).to_bits() {
            out.push((RuntimeTarget::Route { cable: id }, amount(b)));
        }
    }
    Some(out)
}
