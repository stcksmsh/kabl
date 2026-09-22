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
//! - **Cycles are a compile error, not brief section 7.1's 1-block delay — yet.** Detecting and
//!   correctly breaking a cycle with an implicit delay is real, untested work with no existing
//!   spike to lean on; scoped out of this first correctness pass and left as an explicit `Err`
//!   rather than silently doing the wrong thing.
//! - **Buffer-pool reuse** (brief section 7.4): `coalesce_buffers` remaps every buffer to a
//!   smaller set of physical slots via greedy linear-scan register allocation over each buffer's
//!   `[first_def, last_use]` schedule-step interval, run once at compile time after the schedule
//!   is built. See that function's own doc comment for the algorithm and the three pinned-live-
//!   forever exceptions (`silence_buf`, `out_left`, `out_right`).
//! - **Params are compile-time constants.** v1 has no cable-to-param modulation (that's cable
//!   depth, v2) — every param is read once from `ModuleState.params` (or `ParamInfo.default` if
//!   unset) at compile time and passed as `Signal::Scalar` every block, exactly matching how
//!   `patch_demo.rs` already used literals for every param.
//!
//! `process_block` is now allocation-free (brief section 3), proven by
//! `tests/compile_rt_safety.rs` running it inside `assert_no_alloc!` the same way spike S1 does.
//! It got there by replacing the original correctness-first pass's per-call `Vec`s (copying
//! inputs out of the buffer pool, building `Signal`/output-slice arrays fresh each block) with
//! fixed-size stack arrays sized to `MAX_INPUTS`/`MAX_OUTPUTS`/`MAX_PARAMS` — constants derived
//! from the largest counts among the 9 known built-ins (`mixer`: 4 inputs; `filter.svf`: 3
//! outputs; `env.adsr`: 4 params). `compile()` (control-thread, allowed to allocate/return
//! errors) rejects any module whose port/param counts exceed those bounds with
//! `CompileError::TooManyPorts`, so a future built-in that needs more headroom fails loudly at
//! compile time instead of the audio thread silently truncating or panicking. `CompiledPatch` is
//! wired into a real audio thread via `patch_engine.rs::PatchEngine` and `crates/standalone`/
//! `crates/ui`'s live playback.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt;

use kabl_core::{ModuleId, PatchState, PortRef};
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{registry, Module, ModuleInfo, PortDirection, ProcessIo, Rate, Signal};

use crate::graph::BLOCK;

pub type BufIdx = usize;

/// `process_block`'s per-step scratch (inputs, params) is a fixed-size stack array sized to
/// these, not a `Vec`, so building it every block doesn't allocate. Set to the largest count any
/// of the 9 known built-ins actually has (`mixer`: 4 inputs; `env.adsr`: 4 params) — `compile()`
/// checks every module against these bounds and returns `CompileError::TooManyPorts` rather than
/// silently truncating if a future built-in needs more.
const MAX_INPUTS: usize = 4;
/// Output arity the audio-thread match in `process_block` handles directly (0/1/2/3 — no module
/// currently needs more; `filter.svf`'s 3 outputs is the largest). Bump alongside a new match arm
/// if a module ever needs more, not just this constant.
const MAX_OUTPUTS: usize = 3;
const MAX_PARAMS: usize = 4;

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
    /// Modules left over after topo-sorting everything with in-degree 0 repeatedly — a cycle.
    /// Brief section 7.1's 1-block-delay handling isn't implemented yet (see module doc).
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
            CompileError::Cycle(ids) => {
                write!(f, "cycle among modules {ids:?} (not yet supported)")
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

enum Step {
    Process {
        module_index: usize,
        inputs: Vec<InputSource>,
        output_bufs: Vec<BufIdx>,
        params: Vec<f32>,
    },
    /// Voice-rate output -> global-rate input: sum `sources` (one per voice) into `dest`.
    SumVoices { sources: Vec<BufIdx>, dest: BufIdx },
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
/// order. Three exceptions get pinned to "live forever" (never eligible for reuse, and nothing
/// else may ever be assigned their slot):
/// - `silence_buf`: a shared sentinel every unconnected input reads, potentially from any step at
///   any point, not something with a normal single-producer lifetime.
/// - `out_left`/`out_right`: read by the caller (`left()`/`right()`) *after* every step in
///   `process_block` has already run — a live range ending at "the last step" would still let
///   something else's buffer overwrite it before the caller gets to read it.
///
/// Reuse within a single step (a buffer last-read by a step that also defines the buffer taking
/// its slot) is deliberately *not* attempted, even though `process_block`'s copy-inputs-before-
/// writing-outputs ordering would make it safe — correctness here shouldn't depend on a subtle
/// invariant of a separate function that could change later. One buffer's worth of slack is a
/// trivial cost next to that fragility.
fn coalesce_buffers(
    steps: &[Step],
    buffer_count: usize,
    silence_buf: BufIdx,
    out_left: BufIdx,
    out_right: BufIdx,
) -> (Vec<BufIdx>, usize) {
    let mut first_def = vec![0usize; buffer_count];
    let mut last_use = vec![0usize; buffer_count];

    for (step_idx, step) in steps.iter().enumerate() {
        match step {
            Step::Process {
                inputs,
                output_bufs,
                ..
            } => {
                for src in inputs {
                    if let InputSource::Buffer(idx) = src {
                        last_use[*idx] = last_use[*idx].max(step_idx);
                    }
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
        }
    }

    for &pinned in &[silence_buf, out_left, out_right] {
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

pub struct CompiledPatch {
    modules: Vec<Box<dyn Module>>,
    /// Parallel to `modules`: which `(ModuleId, voice index)` each instance came from — `None`
    /// voice index for global-rate modules. Used for state carry-over on recompile.
    module_origin: Vec<(ModuleId, Option<usize>)>,
    steps: Vec<Step>,
    buffers: Vec<[f32; BLOCK]>,
    out_left: BufIdx,
    out_right: BufIdx,
    sample_rate: f32,
    voice_count: usize,
}

struct CableInfo {
    from_id: ModuleId,
    from_port: String,
    to_port: String,
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

    // Cables terminating at each module, keyed by destination id -> list of (dest port, source).
    let mut cables_by_dest: BTreeMap<ModuleId, Vec<CableInfo>> = BTreeMap::new();
    let mut edges: BTreeMap<ModuleId, Vec<ModuleId>> = BTreeMap::new();
    let mut indegree: BTreeMap<ModuleId, usize> = metas.keys().map(|&id| (id, 0)).collect();

    for cstate in patch.cables.values() {
        let PortRef::Module {
            id: from_id,
            port: from_port,
        } = &cstate.from;
        let PortRef::Module {
            id: to_id,
            port: to_port,
        } = &cstate.to;
        edges.entry(*from_id).or_default().push(*to_id);
        *indegree.entry(*to_id).or_insert(0) += 1;
        cables_by_dest.entry(*to_id).or_default().push(CableInfo {
            from_id: *from_id,
            from_port: from_port.clone(),
            to_port: to_port.clone(),
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

    let mut buffers: Vec<[f32; BLOCK]> = Vec::new();
    let silence_buf = {
        buffers.push([0.0; BLOCK]);
        buffers.len() - 1
    };

    let mut voice_output_buf: HashMap<(ModuleId, usize, usize), BufIdx> = HashMap::new();
    let mut global_output_buf: HashMap<(ModuleId, usize), BufIdx> = HashMap::new();
    let mut summed_buf: HashMap<(ModuleId, usize), BufIdx> = HashMap::new();

    let mut modules: Vec<Box<dyn Module>> = Vec::new();
    let mut module_origin: Vec<(ModuleId, Option<usize>)> = Vec::new();
    let mut steps: Vec<Step> = Vec::new();
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
            .map(|p| mstate.params.get(p.name).copied().unwrap_or(p.default))
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
        for cable in incoming {
            if input_port_index(meta.info, &cable.to_port).is_none() {
                return Err(CompileError::UnknownPort {
                    id,
                    kind: "destination",
                    port: cable.to_port.clone(),
                });
            }
        }

        for lane in 0..n_lanes {
            let mut lane_inputs = Vec::with_capacity(input_ports.len());
            for port in &input_ports {
                let cable = incoming.iter().find(|c| c.to_port == port.name);
                let source = match cable {
                    None => InputSource::Silence,
                    Some(cable) => {
                        let src_meta = &metas[&cable.from_id];
                        let src_out_idx = output_port_index(src_meta.info, &cable.from_port)
                            .ok_or_else(|| CompileError::UnknownPort {
                                id: cable.from_id,
                                kind: "source",
                                port: cable.from_port.clone(),
                            })?;
                        let src_is_voice = src_meta.info.rate == Rate::Voice;
                        let buf = if src_is_voice && is_voice {
                            voice_output_buf[&(cable.from_id, src_out_idx, lane)]
                        } else if src_is_voice && !is_voice {
                            *summed_buf
                                .entry((cable.from_id, src_out_idx))
                                .or_insert_with(|| {
                                    let sources: Vec<BufIdx> = (0..voice_count)
                                        .map(|v| voice_output_buf[&(cable.from_id, src_out_idx, v)])
                                        .collect();
                                    buffers.push([0.0; BLOCK]);
                                    let dest = buffers.len() - 1;
                                    steps.push(Step::SumVoices { sources, dest });
                                    dest
                                })
                        } else {
                            global_output_buf[&(cable.from_id, src_out_idx)]
                        };
                        InputSource::Buffer(buf)
                    }
                };
                lane_inputs.push(source);
            }

            let mut lane_outputs = Vec::with_capacity(output_ports.len());
            for (out_idx, _) in output_ports.iter().enumerate() {
                buffers.push([0.0; BLOCK]);
                let buf = buffers.len() - 1;
                lane_outputs.push(buf);
                if is_voice {
                    voice_output_buf.insert((id, out_idx, lane), buf);
                } else {
                    global_output_buf.insert((id, out_idx), buf);
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

            steps.push(Step::Process {
                module_index,
                inputs: lane_inputs,
                output_bufs: lane_outputs,
                params: params.clone(),
            });
        }
    }

    let quality = QualityConfig {
        tier: QualityTier::Live,
    };
    for m in &mut modules {
        m.prepare(sample_rate, BLOCK, &quality);
    }

    let (remap, physical_count) =
        coalesce_buffers(&steps, buffers.len(), silence_buf, out_left, out_right);
    for step in &mut steps {
        match step {
            Step::Process {
                inputs,
                output_bufs,
                ..
            } => {
                for src in inputs.iter_mut() {
                    if let InputSource::Buffer(idx) = src {
                        *idx = remap[*idx];
                    }
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
        }
    }
    out_left = remap[out_left];
    out_right = remap[out_right];
    let buffers = vec![[0.0; BLOCK]; physical_count];

    Ok(CompiledPatch {
        modules,
        module_origin,
        steps,
        buffers,
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
    let old_origins = old.module_origin.clone();
    for (id, voice) in old_origins {
        if let (Some(old_module), Some(new_module)) =
            (old.module_mut(id, voice), new_patch.module_mut(id, voice))
        {
            let mut state = HashMapState::default();
            old_module.save_state(&mut state);
            new_module.load_state(&state);
        }
    }
    Ok(new_patch)
}
