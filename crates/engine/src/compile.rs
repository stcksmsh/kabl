//! The flat-schedule compiler (brief section 7): turns a `kabl_core::PatchState` into a
//! `CompiledPatch` that can be `process_block`-ed, generalizing what `patch_demo.rs` hand-wired
//! for one fixed topology. See docs/decisions.md for the full design writeup, including two
//! decisions made here that aren't in the brief verbatim:
//!
//! - **Voice-rate output feeding a global-rate input averages across voice instances** (sum,
//!   then divide by voice count — not a raw sum). Brief foundational decision 4 ("voice-rate
//!   subgraph instanced per voice; global-rate subgraph shared") implies *some* combination but
//!   doesn't spell out the mechanism; averaging (not summing) is the concrete rule chosen so a
//!   1-note and an 8-note chord of the same patch come out comparably loud instead of the chord
//!   being 8x louder — the same reasoning a real polyphonic synth's voice-sum stage uses. This
//!   is why `patch_demo.rs`'s 4-input `mixer` standing in for "voice bus" isn't what this
//!   compiler's test patch uses: a single voice-rate chain connected straight to `out` is the
//!   correct, simpler shape once the compiler averages automatically. `mixer` remains available
//!   for combining genuinely different signals, not for polyphony.
//! - **Cycles get brief section 7.1's implicit 1-block delay.** A DFS over the cable graph
//!   (`kabl_core::PatchState.cables`, module-id order for determinism) classifies each cable as a
//!   tree/forward/cross edge or a *back* edge (destination already on the current DFS stack).
//!   Removing every back edge from the schedule-ordering graph leaves a DAG (standard result: any
//!   cycle must contain a back edge w.r.t. any one DFS of it), so Kahn's algorithm always
//!   completes — `CompileError::Cycle` is now an unreachable safety net, not the normal outcome.
//!   Each back-edge cable instead reads a *delay buffer*: a separate buffer holding its source's
//!   output from the *previous* `process_block()` call, refreshed by a `Step::CopyToDelay` at the
//!   end of the schedule. First block after compile sees silence on that input (no history yet).
//!   Delay buffers are pinned live-forever in `coalesce_buffers` (their whole point is surviving
//!   past the block that writes them) and, like module state, are carried across `recompile()`:
//!   each delay buffer is keyed by what it holds (`DelaySlotKey` — its source port, plus a lane
//!   for a voice-rate source) rather than where it physically lives, so `recompile()` can find the
//!   same feedback loop's slot in the freshly-compiled patch and copy its last value forward. A
//!   cable that stops being a DFS back edge (or is removed) simply drops its slot; a newly-cyclic
//!   edge starts silent, same as any first compile. See decisions.md "Cycle handling: implicit
//!   1-block delay" and "Cycle handling: carrying delay-buffer memory across recompile".
//! - **Buffer-pool reuse** (brief section 7.4): `coalesce_buffers` remaps every buffer to a
//!   smaller set of physical slots via greedy linear-scan register allocation over each buffer's
//!   `[first_def, last_use]` schedule-step interval, run once at compile time after the schedule
//!   is built. See that function's own doc comment for the algorithm and its pinned-live-forever
//!   exceptions (`silence_buf`, `out_left`, `out_right`, every delay buffer).
//! - **Params are a compile-time base plus block-rate modulation.** Each param's base comes from
//!   `ModuleState.params` (or `ParamInfo.default`). A cable into `PortRef::Param` is a
//!   modulation route. Per block, the compiler normalizes the base into the param's taper
//!   (`ParamInfo::to_norm`), adds `source[0] × amount / source full scale` for every
//!   non-bypassed route, clamps once to 0..1, and converts back (`ParamInfo::from_norm`, which
//!   rounds `Stepped` params). Evaluation is block rate (one value per `BLOCK` samples) for every
//!   param, whatever the destination module does with it. Routes take part in scheduling and
//!   cycle handling like any cable: a feedback route reads its source one block late.
//!
//! `process_block` is now allocation-free (brief section 3), proven by
//! `tests/compile_rt_safety.rs` running it inside `assert_no_alloc!` the same way spike S1 does.
//! It got there by replacing the original correctness-first pass's per-call `Vec`s (copying
//! inputs out of the buffer pool, building `Signal`/output-slice arrays fresh each block) with
//! fixed-size stack arrays sized to `MAX_INPUTS`/`MAX_OUTPUTS`/`MAX_PARAMS` — constants derived
//! from the largest counts among the known built-ins (`mixer`: 4 inputs; `filter.svf`: 3
//! outputs; `seq`: 16 params). `compile()` (control-thread, allowed to allocate/return
//! errors) rejects any module whose port/param counts exceed those bounds with
//! `CompileError::TooManyPorts`, so a future built-in that needs more headroom fails loudly at
//! compile time instead of the audio thread silently truncating or panicking. `CompiledPatch` is
//! wired into a real audio thread via `patch_engine.rs::PatchEngine` and `crates/standalone`/
//! `crates/ui`'s live playback.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt;

use kabl_core::{CableId, ModuleId, PatchState, PortRef};
use kabl_modules::builtins::MidiIn;
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{
    registry, Module, ModuleInfo, ParamInfo, PortDirection, ProcessIo, Rate, Signal, StateBuf,
};

use crate::graph::BLOCK;

pub type BufIdx = usize;

/// `process_block`'s per-step scratch (inputs, params) is a fixed-size stack array sized to
/// these, not a `Vec`, so building it every block doesn't allocate. Set to the largest count any
/// of the known built-ins actually has (`mixer`: 4 inputs; `seq`: 16 params) — `compile()`
/// checks every module against these bounds and returns `CompileError::TooManyPorts` rather than
/// silently truncating if a future built-in needs more.
const MAX_INPUTS: usize = 4;
/// Output arity the audio-thread match in `process_block` handles directly (0/1/2/3 — no module
/// currently needs more; `filter.svf`'s 3 outputs is the largest). Bump alongside a new match arm
/// if a module ever needs more, not just this constant.
const MAX_OUTPUTS: usize = 3;
const MAX_PARAMS: usize = 16;

/// Route amount when a `PortRef::Param` cable has no stored `amount` (+25 % of knob travel, the
/// UI's default on drop). A stored value always wins.
pub const DEFAULT_ROUTE_AMOUNT: f32 = 0.25;

#[derive(Debug)]
pub enum CompileError {
    UnknownKind {
        id: ModuleId,
        kind: String,
    },
    UnknownPort {
        id: ModuleId,
        kind: &'static str,
        port: String,
    },
    /// A modulation route names a param the destination module doesn't have.
    UnknownParam {
        id: ModuleId,
        param: String,
    },
    /// A cable references a module id that isn't in the patch.
    MissingModule {
        cable: CableId,
        id: ModuleId,
    },
    /// Modules left over after topo-sorting everything with in-degree 0 repeatedly. Should not
    /// happen in practice: every cycle's back edges (found via DFS) are excluded from this
    /// ordering graph before Kahn's algorithm runs, which is proven to leave a DAG (see module
    /// doc). Kept as a defensive fallback rather than an `unreachable!`/`panic!`, since a compiler
    /// bug should fail loudly with a `Result`, not take down the caller.
    Cycle(Vec<ModuleId>),
    /// A module's input/output/param count exceeds `MAX_INPUTS`/`MAX_OUTPUTS`/`MAX_PARAMS` — the
    /// fixed-size scratch `process_block` uses to stay allocation-free can't hold it. Caught here
    /// at compile time (control thread, safe to fail loudly) rather than truncating data or
    /// panicking on the audio thread.
    TooManyPorts {
        id: ModuleId,
        kind: String,
        what: &'static str,
        count: usize,
        max: usize,
    },
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::UnknownKind { id, kind } => {
                write!(f, "module {id} has unknown kind \"{kind}\"")
            }
            CompileError::UnknownPort { id, kind, port } => {
                write!(f, "module {id} (\"{kind}\") has no port named \"{port}\"")
            }
            CompileError::UnknownParam { id, param } => {
                write!(f, "module {id} has no param named \"{param}\"")
            }
            CompileError::MissingModule { cable, id } => {
                write!(f, "cable {cable} references missing module {id}")
            }
            CompileError::Cycle(ids) => {
                write!(
                    f,
                    "internal error: modules {ids:?} left over after topo sort despite back-edge \
                     removal — this should be unreachable, please report it"
                )
            }
            CompileError::TooManyPorts {
                id,
                kind,
                what,
                count,
                max,
            } => {
                write!(
                    f,
                    "module {id} (\"{kind}\") has {count} {what}, but the compiler's fixed \
                     scratch space only supports up to {max} — bump the relevant MAX_* constant \
                     in compile.rs"
                )
            }
        }
    }
}

impl std::error::Error for CompileError {}

#[derive(Clone, Copy)]
enum InputSource {
    Silence,
    Buffer(BufIdx),
}

/// One modulated param of one `Step::Process`: base in knob travel plus every active route.
struct ParamMod {
    index: usize,
    info: ParamInfo,
    base_norm: f32,
    /// (source buffer, signed amount / source full scale).
    routes: Vec<(BufIdx, f32)>,
}

enum Step {
    Process {
        module_index: usize,
        inputs: Vec<InputSource>,
        output_bufs: Vec<BufIdx>,
        params: Vec<f32>,
        mods: Vec<ParamMod>,
    },
    /// Voice-rate output -> global-rate input: sum `sources` (one per voice) into `dest`.
    SumVoices { sources: Vec<BufIdx>, dest: BufIdx },
    /// Cycle-breaking (brief section 7.1): copy `src`'s current-block content into `dest` (a
    /// pinned delay buffer) for a delayed cable's reader to pick up on the *next* `process_block`
    /// call. Placed at the end of the schedule so this block's own `Step::Process` reads (earlier
    /// in `steps`) still see last block's value.
    CopyToDelay { src: BufIdx, dest: BufIdx },
}

/// Reassigns buffer indices so buffers whose live ranges don't overlap share the same physical
/// slot — the buffer-pool reuse this compiler originally shipped without (see the module doc's
/// "No buffer-pool reuse yet" note, now stale). Standard greedy linear-scan register allocation:
/// sort buffers by the step that first defines them, then for each one either reuse a physical
/// slot whose previous occupant's last use is already behind this buffer's first def, or hand out
/// a fresh slot. `steps.len()` buffers is small (tens to low hundreds for any real patch), so the
/// O(n log n) sort plus O(n) scan here is not worth agonizing over.
///
/// A buffer's live range is `[step that produces it, last step that reads it]`, both in schedule
/// order. `pinned` buffers get "live forever" instead (never eligible for reuse, and nothing else
/// may ever be assigned their slot):
/// - `silence_buf`: a shared sentinel every unconnected input reads, potentially from any step at
///   any point, not something with a normal single-producer lifetime.
/// - `out_left`/`out_right`: read by the caller (`left()`/`right()`) *after* every step in
///   `process_block` has already run — a live range ending at "the last step" would still let
///   something else's buffer overwrite it before the caller gets to read it.
/// - every delay buffer (brief section 7.1's cycle-breaking, see module doc): its value must
///   survive from one `process_block()` call to the *next* one, not just to the end of this
///   block's schedule — reuse within a single block's live-range analysis has no way to know
///   that.
///
/// Reuse within a single step (a buffer last-read by a step that also defines the buffer taking
/// its slot) is deliberately *not* attempted, even though `process_block`'s copy-inputs-before-
/// writing-outputs ordering would make it safe — correctness here shouldn't depend on a subtle
/// invariant of a separate function that could change later. One buffer's worth of slack is a
/// trivial cost next to that fragility.
fn coalesce_buffers(
    steps: &[Step],
    buffer_count: usize,
    pinned: &[BufIdx],
) -> (Vec<BufIdx>, usize) {
    let mut first_def = vec![0usize; buffer_count];
    let mut last_use = vec![0usize; buffer_count];

    for (step_idx, step) in steps.iter().enumerate() {
        match step {
            Step::Process {
                inputs,
                output_bufs,
                mods,
                ..
            } => {
                for src in inputs {
                    if let InputSource::Buffer(idx) = src {
                        last_use[*idx] = last_use[*idx].max(step_idx);
                    }
                }
                for &(idx, _) in mods.iter().flat_map(|m| &m.routes) {
                    last_use[idx] = last_use[idx].max(step_idx);
                }
                for &out_idx in output_bufs {
                    first_def[out_idx] = step_idx;
                    last_use[out_idx] = last_use[out_idx].max(step_idx);
                }
            }
            Step::SumVoices { sources, dest } => {
                for &src in sources {
                    last_use[src] = last_use[src].max(step_idx);
                }
                first_def[*dest] = step_idx;
                last_use[*dest] = last_use[*dest].max(step_idx);
            }
            Step::CopyToDelay { src, dest } => {
                last_use[*src] = last_use[*src].max(step_idx);
                first_def[*dest] = step_idx;
                last_use[*dest] = last_use[*dest].max(step_idx);
            }
        }
    }

    for &pinned in pinned {
        first_def[pinned] = 0;
        last_use[pinned] = usize::MAX;
    }

    let mut order: Vec<BufIdx> = (0..buffer_count).collect();
    order.sort_by_key(|&i| (first_def[i], i));

    let mut remap = vec![0 as BufIdx; buffer_count];
    let mut active: Vec<(BufIdx, usize)> = Vec::new(); // (physical slot, last_use)
    let mut free: Vec<BufIdx> = Vec::new();
    let mut next_physical: BufIdx = 0;

    for old_idx in order {
        let def = first_def[old_idx];
        active.retain(|&(phys, lu)| {
            if lu < def {
                free.push(phys);
                false
            } else {
                true
            }
        });
        let phys = free.pop().unwrap_or_else(|| {
            let p = next_physical;
            next_physical += 1;
            p
        });
        remap[old_idx] = phys;
        active.push((phys, last_use[old_idx]));
    }

    (remap, next_physical)
}

fn input_port_index(info: &ModuleInfo, name: &str) -> Option<usize> {
    info.ports
        .iter()
        .filter(|p| p.direction == PortDirection::Input)
        .position(|p| p.name == name)
}

fn output_port_index(info: &ModuleInfo, name: &str) -> Option<usize> {
    info.ports
        .iter()
        .filter(|p| p.direction == PortDirection::Output)
        .position(|p| p.name == name)
}

/// Identifies one delay buffer (brief section 7.1's cycle-breaking) by what it holds, independent
/// of its physical buffer index — a delayed cable's source port, plus a lane for a voice-rate
/// source (`None` for global-rate/summed). Stable across recompile as long as the same cable stays
/// a DFS back edge, which is what lets `recompile()` carry a feedback loop's one-block memory
/// forward instead of resetting it to silence. Summed (voice-source-into-global-sink) delay
/// buffers are intentionally not carried: they're a derived sum recomputed by `Step::SumVoices`
/// every block from the per-lane delay buffers, which are themselves already carried.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct DelaySlotKey {
    from_id: ModuleId,
    from_port_idx: usize,
    lane: Option<usize>,
}

pub struct CompiledPatch {
    modules: Vec<Box<dyn Module>>,
    /// `(module index, voice)` of every `midi.in` instance. Every `midi.in` in the patch
    /// receives every note, voice for voice (see `note_on`).
    midi_ins: Vec<(usize, usize)>,
    /// Parallel to `modules`: which `(ModuleId, voice index)` each instance came from — `None`
    /// voice index for global-rate modules. Used for state carry-over on recompile.
    module_origin: Vec<(ModuleId, Option<usize>)>,
    steps: Vec<Step>,
    buffers: Vec<[f32; BLOCK]>,
    /// Every delay buffer's physical index, keyed by what it holds rather than where it lives —
    /// lets `recompile()` find "the same" delay buffer in the new compile and copy its value
    /// across, instead of every feedback loop resetting to silence on every edit. See
    /// `DelaySlotKey`'s doc and module doc's "Cycle handling" section.
    delay_slots: HashMap<DelaySlotKey, BufIdx>,
    out_left: BufIdx,
    out_right: BufIdx,
    sample_rate: f32,
    voice_count: usize,
}

enum CableTo {
    Port(String),
    /// Modulation route into a param: `scale` is the signed amount already divided by the
    /// source's nominal full scale.
    Param {
        index: usize,
        amount: f32,
    },
}

struct CableInfo {
    from_id: ModuleId,
    from_port: String,
    to: CableTo,
    /// This cable is a DFS back edge (brief section 7.1): its reader gets last block's value from
    /// a delay buffer instead of this block's live value, and it's excluded from the
    /// topo-ordering graph. See module doc.
    delayed: bool,
}

/// A route with `bypass` set contributes nothing and is left out of the compiled graph; its
/// settings stay in the patch.
fn route_bypassed(c: &kabl_core::CableState) -> bool {
    matches!(c.to, PortRef::Param { .. }) && c.params.get("bypass").is_some_and(|&b| b >= 0.5)
}

#[derive(Default)]
struct BufMaps {
    voice: HashMap<(ModuleId, usize, usize), BufIdx>,
    global: HashMap<(ModuleId, usize), BufIdx>,
    summed: HashMap<(ModuleId, usize), BufIdx>,
}

/// Buffer and step bookkeeping while `compile` wires modules. `live` holds this block's output
/// buffers, `delay` the one-block-late copies read by feedback cables.
struct Wiring {
    buffers: Vec<[f32; BLOCK]>,
    steps: Vec<Step>,
    voice_count: usize,
    live: BufMaps,
    delay: BufMaps,
}

impl Wiring {
    fn new_buf(&mut self) -> BufIdx {
        self.buffers.push([0.0; BLOCK]);
        self.buffers.len() - 1
    }

    /// The buffer a reader in `lane` sees for source output `(from_id, out_idx)`. Voice → voice
    /// is per lane, global → anything is shared, voice → global is the average over voices
    /// (a `SumVoices` step, created once per source).
    fn source_buf(
        &mut self,
        from_id: ModuleId,
        out_idx: usize,
        src_is_voice: bool,
        dest_is_voice: bool,
        lane: usize,
        delayed: bool,
    ) -> BufIdx {
        let maps = if delayed {
            &mut self.delay
        } else {
            &mut self.live
        };
        if !src_is_voice {
            return maps.global[&(from_id, out_idx)];
        }
        if dest_is_voice {
            return maps.voice[&(from_id, out_idx, lane)];
        }
        if let Some(&b) = maps.summed.get(&(from_id, out_idx)) {
            return b;
        }
        let sources: Vec<BufIdx> = (0..self.voice_count)
            .map(|v| maps.voice[&(from_id, out_idx, v)])
            .collect();
        self.buffers.push([0.0; BLOCK]);
        let dest = self.buffers.len() - 1;
        self.steps.push(Step::SumVoices { sources, dest });
        let maps = if delayed {
            &mut self.delay
        } else {
            &mut self.live
        };
        maps.summed.insert((from_id, out_idx), dest);
        dest
    }
}

pub fn compile(
    patch: &PatchState,
    sample_rate: f32,
    voice_count: usize,
) -> Result<CompiledPatch, CompileError> {
    struct ModuleMeta {
        kind: String,
        info: &'static ModuleInfo,
    }

    let mut metas: BTreeMap<ModuleId, ModuleMeta> = BTreeMap::new();
    for (&id, mstate) in &patch.modules {
        let info = registry::info_for(&mstate.kind).ok_or_else(|| CompileError::UnknownKind {
            id,
            kind: mstate.kind.clone(),
        })?;
        metas.insert(
            id,
            ModuleMeta {
                kind: mstate.kind.clone(),
                info,
            },
        );
    }

    // Brief section 7.1: find a feedback-arc set via DFS over the cable graph (cable-id order —
    // `patch.cables` is a `BTreeMap`, so this is deterministic) so a cycle gets an implicit
    // 1-block delay instead of failing to compile. Any cycle must contain at least one DFS back
    // edge (destination still on the current recursion stack); removing every back edge found by
    // one DFS run always leaves the graph acyclic, so Kahn's algorithm below is guaranteed to
    // consume every module. See module doc.
    let mut adj_by_cable: BTreeMap<ModuleId, Vec<(CableId, ModuleId)>> = BTreeMap::new();
    for (&cable_id, cstate) in &patch.cables {
        if route_bypassed(cstate) {
            continue;
        }
        let (from_id, to_id) = (cstate.from.module_id(), cstate.to.module_id());
        for id in [from_id, to_id] {
            if !metas.contains_key(&id) {
                return Err(CompileError::MissingModule {
                    cable: cable_id,
                    id,
                });
            }
        }
        adj_by_cable
            .entry(from_id)
            .or_default()
            .push((cable_id, to_id));
    }

    #[derive(Clone, Copy, PartialEq)]
    enum Color {
        White,
        Gray,
        Black,
    }
    fn mark_back_edges(
        id: ModuleId,
        adj: &BTreeMap<ModuleId, Vec<(CableId, ModuleId)>>,
        color: &mut BTreeMap<ModuleId, Color>,
        delayed: &mut std::collections::BTreeSet<CableId>,
    ) {
        color.insert(id, Color::Gray);
        if let Some(children) = adj.get(&id) {
            for &(cable_id, to_id) in children {
                match color[&to_id] {
                    Color::Gray => {
                        delayed.insert(cable_id);
                    }
                    Color::White => mark_back_edges(to_id, adj, color, delayed),
                    Color::Black => {}
                }
            }
        }
        color.insert(id, Color::Black);
    }
    let mut color: BTreeMap<ModuleId, Color> = metas.keys().map(|&id| (id, Color::White)).collect();
    let mut delayed_cables: std::collections::BTreeSet<CableId> = Default::default();
    for &id in metas.keys() {
        if color[&id] == Color::White {
            mark_back_edges(id, &adj_by_cable, &mut color, &mut delayed_cables);
        }
    }

    // Cables terminating at each module, keyed by destination id. Jack cables and modulation
    // routes both order the schedule (a route's source must run before the param is read).
    let mut cables_by_dest: BTreeMap<ModuleId, Vec<CableInfo>> = BTreeMap::new();
    let mut edges: BTreeMap<ModuleId, Vec<ModuleId>> = BTreeMap::new();
    let mut indegree: BTreeMap<ModuleId, usize> = metas.keys().map(|&id| (id, 0)).collect();

    for (&cable_id, cstate) in &patch.cables {
        if route_bypassed(cstate) {
            continue;
        }
        let PortRef::Module {
            id: from_id,
            port: from_port,
        } = &cstate.from
        else {
            return Err(CompileError::UnknownPort {
                id: cstate.from.module_id(),
                kind: "source",
                port: "<param used as a cable source>".into(),
            });
        };
        let (to_id, to) = match &cstate.to {
            PortRef::Module { id, port } => (*id, CableTo::Port(port.clone())),
            PortRef::Param { id, param } => {
                let info = metas[id].info;
                let index = info
                    .params
                    .iter()
                    .position(|p| {
                        p.name == param
                            || registry::legacy_param(info.kind, p.name) == Some(param.as_str())
                    })
                    .ok_or_else(|| CompileError::UnknownParam {
                        id: *id,
                        param: param.clone(),
                    })?;
                let amount = cstate
                    .params
                    .get("amount")
                    .copied()
                    .unwrap_or(DEFAULT_ROUTE_AMOUNT);
                (*id, CableTo::Param { index, amount })
            }
        };
        let delayed = delayed_cables.contains(&cable_id);
        if !delayed {
            edges.entry(*from_id).or_default().push(to_id);
            *indegree.entry(to_id).or_insert(0) += 1;
        }
        cables_by_dest.entry(to_id).or_default().push(CableInfo {
            from_id: *from_id,
            from_port: from_port.clone(),
            to,
            delayed,
        });
    }

    // Kahn's algorithm, BTreeMap/sorted throughout for a deterministic schedule.
    let mut queue: VecDeque<ModuleId> = indegree
        .iter()
        .filter(|&(_, &d)| d == 0)
        .map(|(&id, _)| id)
        .collect();
    let mut order = Vec::with_capacity(metas.len());
    while let Some(id) = queue.pop_front() {
        order.push(id);
        if let Some(succs) = edges.get(&id) {
            for &succ in succs {
                let d = indegree.get_mut(&succ).unwrap();
                *d -= 1;
                if *d == 0 {
                    queue.push_back(succ);
                }
            }
        }
    }
    if order.len() != metas.len() {
        let remaining: Vec<ModuleId> = metas
            .keys()
            .copied()
            .filter(|id| !order.contains(id))
            .collect();
        return Err(CompileError::Cycle(remaining));
    }

    let mut w = Wiring {
        buffers: Vec::new(),
        steps: Vec::new(),
        voice_count,
        live: BufMaps::default(),
        delay: BufMaps::default(),
    };
    let silence_buf = w.new_buf();

    // Delay buffers for every delayed cable's source port (brief section 7.1), pre-created before
    // the topo-order wiring loop below: a delayed cable's source can be scheduled *after* its
    // reader now that the edge no longer constrains ordering, so the reader can't wait for the
    // source's normal output-buffer bookkeeping to exist yet.
    let mut delayed_sources: std::collections::BTreeSet<(ModuleId, String)> = Default::default();
    for cables in cables_by_dest.values() {
        for c in cables {
            if c.delayed {
                delayed_sources.insert((c.from_id, c.from_port.clone()));
            }
        }
    }
    for (from_id, from_port) in delayed_sources {
        let src_meta = &metas[&from_id];
        let src_out_idx = output_port_index(src_meta.info, &from_port).ok_or_else(|| {
            CompileError::UnknownPort {
                id: from_id,
                kind: "source",
                port: from_port.clone(),
            }
        })?;
        if src_meta.info.rate == Rate::Voice {
            for lane in 0..voice_count {
                let b = w.new_buf();
                w.delay.voice.insert((from_id, src_out_idx, lane), b);
            }
        } else {
            let b = w.new_buf();
            w.delay.global.insert((from_id, src_out_idx), b);
        }
    }

    let mut modules: Vec<Box<dyn Module>> = Vec::new();
    let mut module_origin: Vec<(ModuleId, Option<usize>)> = Vec::new();
    let mut midi_ins: Vec<(usize, usize)> = Vec::new();
    let mut out_left = silence_buf;
    let mut out_right = silence_buf;

    for &id in &order {
        let meta = &metas[&id];
        let mstate = &patch.modules[&id];
        let is_voice = meta.info.rate == Rate::Voice;
        let n_lanes = if is_voice { voice_count } else { 1 };

        let params: Vec<f32> = meta
            .info
            .params
            .iter()
            .map(|p| {
                mstate
                    .params
                    .get(p.name)
                    .or_else(|| {
                        registry::legacy_param(meta.info.kind, p.name)
                            .and_then(|old| mstate.params.get(old))
                    })
                    .copied()
                    .unwrap_or(p.default)
            })
            .collect();

        let input_ports: Vec<_> = meta
            .info
            .ports
            .iter()
            .filter(|p| p.direction == PortDirection::Input)
            .collect();
        let output_ports: Vec<_> = meta
            .info
            .ports
            .iter()
            .filter(|p| p.direction == PortDirection::Output)
            .collect();

        if input_ports.len() > MAX_INPUTS {
            return Err(CompileError::TooManyPorts {
                id,
                kind: meta.kind.clone(),
                what: "input ports",
                count: input_ports.len(),
                max: MAX_INPUTS,
            });
        }
        if output_ports.len() > MAX_OUTPUTS {
            return Err(CompileError::TooManyPorts {
                id,
                kind: meta.kind.clone(),
                what: "output ports",
                count: output_ports.len(),
                max: MAX_OUTPUTS,
            });
        }
        if params.len() > MAX_PARAMS {
            return Err(CompileError::TooManyPorts {
                id,
                kind: meta.kind.clone(),
                what: "params",
                count: params.len(),
                max: MAX_PARAMS,
            });
        }

        let empty = Vec::new();
        let incoming = cables_by_dest.get(&id).unwrap_or(&empty);
        // Resolve every incoming cable's source port once (validates names too).
        let mut sources = Vec::with_capacity(incoming.len());
        for cable in incoming {
            if let CableTo::Port(port) = &cable.to {
                if input_port_index(meta.info, port).is_none() {
                    return Err(CompileError::UnknownPort {
                        id,
                        kind: "destination",
                        port: port.clone(),
                    });
                }
            }
            let src_info = metas[&cable.from_id].info;
            let src_out_idx = output_port_index(src_info, &cable.from_port).ok_or_else(|| {
                CompileError::UnknownPort {
                    id: cable.from_id,
                    kind: "source",
                    port: cable.from_port.clone(),
                }
            })?;
            let port_type = src_info
                .ports
                .iter()
                .filter(|p| p.direction == PortDirection::Output)
                .nth(src_out_idx)
                .expect("index from output_port_index")
                .port_type;
            let (lo, hi) = port_type.nominal_range();
            let full_scale = lo.abs().max(hi.abs());
            sources.push((src_out_idx, src_info.rate == Rate::Voice, full_scale));
        }

        for lane in 0..n_lanes {
            let mut lane_inputs = Vec::with_capacity(input_ports.len());
            for port in &input_ports {
                let found = incoming
                    .iter()
                    .position(|c| matches!(&c.to, CableTo::Port(p) if p == port.name));
                lane_inputs.push(match found {
                    None => InputSource::Silence,
                    Some(k) => {
                        let (src_out_idx, src_is_voice, _) = sources[k];
                        let c = &incoming[k];
                        InputSource::Buffer(w.source_buf(
                            c.from_id,
                            src_out_idx,
                            src_is_voice,
                            is_voice,
                            lane,
                            c.delayed,
                        ))
                    }
                });
            }

            let mut mods: Vec<ParamMod> = Vec::new();
            for (k, c) in incoming.iter().enumerate() {
                let CableTo::Param { index, amount } = c.to else {
                    continue;
                };
                let (src_out_idx, src_is_voice, full_scale) = sources[k];
                let buf = w.source_buf(
                    c.from_id,
                    src_out_idx,
                    src_is_voice,
                    is_voice,
                    lane,
                    c.delayed,
                );
                let route = (buf, amount / full_scale);
                match mods.iter_mut().find(|m| m.index == index) {
                    Some(m) => m.routes.push(route),
                    None => {
                        let info = meta.info.params[index];
                        mods.push(ParamMod {
                            index,
                            info,
                            base_norm: info.to_norm(params[index]),
                            routes: vec![route],
                        });
                    }
                }
            }

            let mut lane_outputs = Vec::with_capacity(output_ports.len());
            for (out_idx, _) in output_ports.iter().enumerate() {
                let buf = w.new_buf();
                lane_outputs.push(buf);
                if is_voice {
                    w.live.voice.insert((id, out_idx, lane), buf);
                } else {
                    w.live.global.insert((id, out_idx), buf);
                }
            }

            if meta.kind == "out" {
                out_left = match lane_inputs.first() {
                    Some(InputSource::Buffer(b)) => *b,
                    _ => silence_buf,
                };
                out_right = match lane_inputs.get(1) {
                    Some(InputSource::Buffer(b)) => *b,
                    _ => silence_buf,
                };
            }

            let instance =
                registry::create(&meta.kind).expect("kind already validated when building `metas`");
            let module_index = modules.len();
            modules.push(instance);
            module_origin.push((id, if is_voice { Some(lane) } else { None }));
            if meta.kind == "midi.in" {
                midi_ins.push((module_index, lane));
            }

            w.steps.push(Step::Process {
                module_index,
                inputs: lane_inputs,
                output_bufs: lane_outputs,
                params: params.clone(),
                mods,
            });
        }
    }

    let Wiring {
        buffers,
        mut steps,
        live,
        delay,
        ..
    } = w;
    let voice_output_buf = live.voice;
    let global_output_buf = live.global;
    let voice_delay_buf = delay.voice;
    let global_delay_buf = delay.global;

    // Refresh every delay buffer from this block's live value, for the *next* process_block()
    // call to read — sorted keys, not raw `HashMap` iteration order, to keep the schedule
    // deterministic (matters for reproducing `buffer_count()` across identical compiles).
    let mut voice_delay_keys: Vec<_> = voice_delay_buf.keys().copied().collect();
    voice_delay_keys.sort();
    for key @ (from_id, out_idx, lane) in voice_delay_keys {
        let live_buf = voice_output_buf[&(from_id, out_idx, lane)];
        steps.push(Step::CopyToDelay {
            src: live_buf,
            dest: voice_delay_buf[&key],
        });
    }
    let mut global_delay_keys: Vec<_> = global_delay_buf.keys().copied().collect();
    global_delay_keys.sort();
    for key @ (from_id, out_idx) in global_delay_keys {
        let live_buf = global_output_buf[&(from_id, out_idx)];
        steps.push(Step::CopyToDelay {
            src: live_buf,
            dest: global_delay_buf[&key],
        });
    }

    let quality = QualityConfig {
        tier: QualityTier::Live,
    };
    for m in &mut modules {
        m.prepare(sample_rate, BLOCK, &quality);
    }

    let mut delay_slots: HashMap<DelaySlotKey, BufIdx> = HashMap::new();
    for (&(from_id, from_port_idx, lane), &buf) in &voice_delay_buf {
        delay_slots.insert(
            DelaySlotKey {
                from_id,
                from_port_idx,
                lane: Some(lane),
            },
            buf,
        );
    }
    for (&(from_id, from_port_idx), &buf) in &global_delay_buf {
        delay_slots.insert(
            DelaySlotKey {
                from_id,
                from_port_idx,
                lane: None,
            },
            buf,
        );
    }

    let mut pinned = vec![silence_buf, out_left, out_right];
    pinned.extend(voice_delay_buf.values().copied());
    pinned.extend(global_delay_buf.values().copied());
    let (remap, physical_count) = coalesce_buffers(&steps, buffers.len(), &pinned);
    for step in &mut steps {
        match step {
            Step::Process {
                inputs,
                output_bufs,
                mods,
                ..
            } => {
                for src in inputs.iter_mut() {
                    if let InputSource::Buffer(idx) = src {
                        *idx = remap[*idx];
                    }
                }
                for route in mods.iter_mut().flat_map(|m| m.routes.iter_mut()) {
                    route.0 = remap[route.0];
                }
                for out_idx in output_bufs.iter_mut() {
                    *out_idx = remap[*out_idx];
                }
            }
            Step::SumVoices { sources, dest } => {
                for src in sources.iter_mut() {
                    *src = remap[*src];
                }
                *dest = remap[*dest];
            }
            Step::CopyToDelay { src, dest } => {
                *src = remap[*src];
                *dest = remap[*dest];
            }
        }
    }
    out_left = remap[out_left];
    out_right = remap[out_right];
    for buf in delay_slots.values_mut() {
        *buf = remap[*buf];
    }
    let buffers = vec![[0.0; BLOCK]; physical_count];

    Ok(CompiledPatch {
        modules,
        midi_ins,
        module_origin,
        steps,
        buffers,
        delay_slots,
        out_left,
        out_right,
        sample_rate,
        voice_count,
    })
}

impl CompiledPatch {
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn voice_count(&self) -> usize {
        self.voice_count
    }

    /// Physical buffer-pool size after `coalesce_buffers`' reuse pass — exposed so tests (and
    /// anyone curious) can observe that reuse is actually happening, not just trust it silently.
    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }

    /// Module instance at `(id, voice)` — `voice` ignored for global-rate modules. `None` if no
    /// module with that origin exists in this compiled patch (id not in the source `PatchState`,
    /// or `voice` out of range).
    pub fn module_mut(&mut self, id: ModuleId, voice: Option<usize>) -> Option<&mut dyn Module> {
        let index = self
            .module_origin
            .iter()
            .position(|&(origin_id, origin_voice)| origin_id == id && origin_voice == voice)?;
        Some(self.modules[index].as_mut())
    }

    #[inline]
    pub fn process_block(&mut self) {
        for step in &mut self.steps {
            match step {
                Step::Process {
                    module_index,
                    inputs,
                    output_bufs,
                    params,
                    mods,
                } => {
                    // Fixed-size stack scratch, not `Vec` — `compile()` already rejected any
                    // module whose input/param count exceeds MAX_INPUTS/MAX_PARAMS, so these
                    // never need to grow. Copy inputs out (Copy type, cheap) before taking any
                    // mutable borrow below, so there's no aliasing between input reads and
                    // output writes.
                    let mut input_data = [[0.0f32; BLOCK]; MAX_INPUTS];
                    for (slot, src) in input_data.iter_mut().zip(inputs.iter()) {
                        *slot = match src {
                            InputSource::Buffer(idx) => self.buffers[*idx],
                            InputSource::Silence => [0.0; BLOCK],
                        };
                    }
                    let mut input_signals = [Signal::Scalar(0.0); MAX_INPUTS];
                    for (slot, data) in input_signals.iter_mut().zip(input_data.iter()) {
                        *slot = Signal::Buffer(&data[..]);
                    }
                    let input_signals = &input_signals[..inputs.len()];

                    let mut param_signals = [Signal::Scalar(0.0); MAX_PARAMS];
                    for (slot, &v) in param_signals.iter_mut().zip(params.iter()) {
                        *slot = Signal::Scalar(v);
                    }
                    // Block-rate modulation in knob-travel space: base + Σ source × scale,
                    // clamped once inside `from_norm`. A non-finite source leaves the base.
                    for m in mods.iter() {
                        let mut n = m.base_norm;
                        for &(buf, scale) in &m.routes {
                            n += self.buffers[buf][0] * scale;
                        }
                        if !n.is_finite() {
                            n = m.base_norm;
                        }
                        param_signals[m.index] = Signal::Scalar(m.info.from_norm(n));
                    }
                    let param_signals = &param_signals[..params.len()];

                    // Disjoint mutable output slices, built straight into a stack array sized to
                    // the exact arity (no `Vec`). `get_disjoint_mut` returns an error (not UB) if
                    // two indices in `output_bufs` were somehow equal — a genuine safety net, not
                    // just an assumption, since every output buffer is freshly allocated per port
                    // and never reused across steps in this compiler pass. `compile()` already
                    // rejected any module with more than MAX_OUTPUTS output ports, so the `_` arm
                    // below is unreachable, not a latent panic.
                    match output_bufs.as_slice() {
                        [] => {
                            let mut outputs: [&mut [f32]; 0] = [];
                            let mut io =
                                ProcessIo::new(input_signals, &mut outputs, param_signals, BLOCK);
                            self.modules[*module_index].process(&mut io);
                        }
                        &[a] => {
                            let [x] = self.buffers.get_disjoint_mut([a]).expect("disjoint");
                            let mut outputs = [&mut x[..]];
                            let mut io =
                                ProcessIo::new(input_signals, &mut outputs, param_signals, BLOCK);
                            self.modules[*module_index].process(&mut io);
                        }
                        &[a, b] => {
                            let [x, y] = self.buffers.get_disjoint_mut([a, b]).expect("disjoint");
                            let mut outputs = [&mut x[..], &mut y[..]];
                            let mut io =
                                ProcessIo::new(input_signals, &mut outputs, param_signals, BLOCK);
                            self.modules[*module_index].process(&mut io);
                        }
                        &[a, b, c] => {
                            let [x, y, z] =
                                self.buffers.get_disjoint_mut([a, b, c]).expect("disjoint");
                            let mut outputs = [&mut x[..], &mut y[..], &mut z[..]];
                            let mut io =
                                ProcessIo::new(input_signals, &mut outputs, param_signals, BLOCK);
                            self.modules[*module_index].process(&mut io);
                        }
                        _ => unreachable!(
                            "compile() rejects modules with more than MAX_OUTPUTS output ports"
                        ),
                    }
                }
                Step::SumVoices { sources, dest } => {
                    let mut sum = [0f32; BLOCK];
                    for &src in sources.iter() {
                        let buf = self.buffers[src];
                        for i in 0..BLOCK {
                            sum[i] += buf[i];
                        }
                    }
                    // Average, not raw sum: a 1-voice and an N-voice chord playing the same
                    // patch should be comparably loud, not N times louder — see module doc.
                    // `sources.len()` is a power-of-2-friendly divide only incidentally; the
                    // point is "don't scale with polyphony," not a specific curve.
                    let scale = 1.0 / sources.len() as f32;
                    for s in sum.iter_mut() {
                        *s *= scale;
                    }
                    self.buffers[*dest] = sum;
                }
                Step::CopyToDelay { src, dest } => {
                    self.buffers[*dest] = self.buffers[*src];
                }
            }
        }
    }

    /// Starts a note on `voice` in every `midi.in` of this patch. No allocation. With several
    /// `midi.in` modules, each one plays every note (deterministic, like MIDI thru); a patch
    /// with none ignores notes.
    pub fn note_on(&mut self, voice: usize, semitones: f32, velocity: f32) {
        for &(index, v) in &self.midi_ins {
            if v == voice {
                if let Some(m) = self.modules[index].as_any_mut().downcast_mut::<MidiIn>() {
                    m.note_on(semitones, velocity);
                }
            }
        }
    }

    /// Releases `voice` in every `midi.in`. No allocation.
    pub fn note_off(&mut self, voice: usize) {
        for &(index, v) in &self.midi_ins {
            if v == voice {
                if let Some(m) = self.modules[index].as_any_mut().downcast_mut::<MidiIn>() {
                    m.note_off();
                }
            }
        }
    }

    pub fn left(&self) -> &[f32; BLOCK] {
        &self.buffers[self.out_left]
    }

    pub fn right(&self) -> &[f32; BLOCK] {
        &self.buffers[self.out_right]
    }
}

/// Minimal `HashMap`-backed `StateWriter`/`StateReader`, for state carry-over between two
/// `CompiledPatch`es on recompile (brief section 7.5). Same pattern every built-in module's own
/// tests already use locally; factored out here since the compiler needs it generically across
/// arbitrary module kinds, not just to test one module in isolation.
#[derive(Default)]
pub struct HashMapState(pub HashMap<String, f32>);

impl kabl_modules::StateWriter for HashMapState {
    fn write_f32(&mut self, key: &str, value: f32) {
        self.0.insert(key.to_string(), value);
    }
}

impl kabl_modules::StateReader for HashMapState {
    fn read_f32(&self, key: &str) -> Option<f32> {
        self.0.get(key).copied()
    }
}

/// Compiles `patch` fresh, then carries state over from `old` for every `(ModuleId, voice)`
/// present in both — brief section 7.5, generalizing what S1's spike hand-rolled
/// (`CompiledGraph::recompiled_with_depth` cloning phase/filter state directly) across arbitrary
/// module kinds via each module's own `save_state`/`load_state`.
pub fn recompile(
    old: &mut CompiledPatch,
    patch: &PatchState,
    sample_rate: f32,
    voice_count: usize,
) -> Result<CompiledPatch, CompileError> {
    let mut new_patch = compile(patch, sample_rate, voice_count)?;
    carry_state(old, &mut new_patch);
    Ok(new_patch)
}

/// The state-transfer half of `recompile`: for every `(ModuleId, voice)` present in both
/// graphs with the same kind, copies module state from `old` into `new_patch`, plus every
/// feedback delay slot present in both. Allocation-free (fixed-size `StateBuf`, linear origin
/// search, `HashMap` lookups only), so `PatchEngine` runs it on the audio thread at the moment a
/// new graph starts its crossfade, from the graph that is actually playing.
pub fn carry_state(old: &mut CompiledPatch, new_patch: &mut CompiledPatch) {
    for (new_index, &origin) in new_patch.module_origin.iter().enumerate() {
        let Some(old_index) = old.module_origin.iter().position(|&o| o == origin) else {
            continue;
        };
        let old_module = &old.modules[old_index];
        let new_module = &mut new_patch.modules[new_index];
        if old_module.info().kind != new_module.info().kind {
            continue;
        }
        let mut state = StateBuf::default();
        old_module.save_state(&mut state);
        new_module.load_state(&state);
    }
    // Carry a feedback loop's one-block memory across too (brief section 7.1's delay buffers) —
    // for every delay slot present in both the old and new compile (same source port still a DFS
    // back edge), copy its last-known value forward instead of the new compile's default silence.
    // A slot only in `old` (the cable stopped being delayed, or was removed) is simply dropped; a
    // slot only in `new` (a newly-cyclic edge) starts silent, same as any other fresh compile.
    for (key, &new_buf) in &new_patch.delay_slots {
        if let Some(&old_buf) = old.delay_slots.get(key) {
            new_patch.buffers[new_buf] = old.buffers[old_buf];
        }
    }
}
