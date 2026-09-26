//! D04 control layer: turns document changes into what the audio thread needs, without an
//! editor frame. Rules: docs/runtime-controls/design.md.
//!
//! - **One document.** `PatchEditor` stays the only authority. `Delivery` remembers the state
//!   the audio side converges to (`sent`) and diffs the document against it
//!   (`kabl_engine::runtime::runtime_changes`): runtime values when only those changed, else a
//!   compile. Mouse, CC, undo/redo, recipes and comparison restore all edit the document, so
//!   they all arrive the same way.
//! - **Order and bounds.** Graphs and values go through one FIFO in revision order. What does
//!   not fit waits here: at most one graph (a newer one replaces it) and one value per target
//!   (the newest), behind that graph. A graph subsumes every value before it. At most
//!   `MAX_GRAPHS_QUEUED` graphs are in the queue at once. Nothing waiting is lost: `flush`
//!   retries it, and past `MAX_HELD` targets a compile replaces them.
//! - **MIDI without the editor.** `midi` applies CC mappings, pickup and buttons and delivers
//!   the result; `main.rs` calls it from its control thread, never from a frame.

use std::collections::BTreeMap;
use std::sync::Arc;

use basedrop::{Handle, Owned};
use kabl_core::{ModuleId, PatchState};
use kabl_engine::compile::{compile, CompiledPatch};
use kabl_engine::patch_engine::Command;
use kabl_engine::runtime::{runtime_changes, Feedback, ParamSet, RuntimeTarget, ToAudio};
use kabl_modules::builtins::Transport;

use crate::{perform, PatchEditor, UiState};

/// Messages the control queue holds.
pub const QUEUE: usize = 256;
/// Graphs in the queue at once (the audio thread has not taken them yet).
pub const MAX_GRAPHS_QUEUED: u64 = 2;
/// Targets whose newest value may wait here; past it a compile carries them all.
pub const MAX_HELD: usize = 1024;

/// What one `sync` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The document changed nothing the engine reads.
    Nothing,
    /// Runtime values, this many.
    Values(usize),
    /// A graph was compiled (it may still be waiting).
    Compiled,
    /// The compile failed: the audio side keeps its graph.
    Failed,
}

/// Running totals, for the stats file and the evidence.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub graphs: u64,
    pub values: u64,
    /// Values replaced by a newer one for the same target before they were sent.
    pub coalesced: u64,
    /// Compiles made because too many values waited.
    pub fallbacks: u64,
    pub failed: u64,
    /// Launch, preview, inspect or transport commands the queue refused (audio not running).
    pub dropped_actions: u64,
}

/// The control side of the audio queues.
pub struct Delivery {
    tx: Option<rtrb::Producer<ToAudio>>,
    commands: Option<rtrb::Producer<Command>>,
    transport: Option<rtrb::Producer<(ModuleId, Transport)>>,
    handle: Handle,
    pub feedback: Arc<Feedback>,
    sample_rate: f32,
    voice_count: usize,
    /// Last revision handed out.
    rev: u64,
    /// Compile attempts (probe reports name the graph they measured).
    pub generation: u64,
    /// The document state the audio side converges to; `None` = compile next.
    sent: Option<PatchState>,
    held_graph: Option<Owned<CompiledPatch>>,
    held: BTreeMap<RuntimeTarget, ParamSet>,
    graphs_sent: u64,
    values_sent: u64,
    pub compile_error: Option<String>,
    pub counts: Counts,
}

impl Delivery {
    /// `playing`: the document the audio thread's first graph was compiled from, at revision
    /// and generation 1.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tx: Option<rtrb::Producer<ToAudio>>,
        commands: Option<rtrb::Producer<Command>>,
        transport: Option<rtrb::Producer<(ModuleId, Transport)>>,
        handle: Handle,
        feedback: Arc<Feedback>,
        sample_rate: f32,
        voice_count: usize,
        playing: Option<&PatchState>,
    ) -> Self {
        Delivery {
            tx,
            commands,
            transport,
            handle,
            feedback,
            sample_rate,
            voice_count,
            rev: 1,
            generation: 1,
            sent: playing.cloned(),
            held_graph: None,
            held: BTreeMap::new(),
            graphs_sent: 0,
            values_sent: 0,
            compile_error: None,
            counts: Counts::default(),
        }
    }

    /// Brings the audio side to `doc`. `fresh`: a Load (compiled, no state carried);
    /// `stopped`: its clocks start stopped.
    pub fn sync(&mut self, doc: &PatchState, fresh: bool, stopped: bool) -> Outcome {
        let changes = match &self.sent {
            Some(sent) if !fresh => runtime_changes(sent, doc),
            _ => None,
        };
        let outcome = match changes {
            Some(v) if v.is_empty() => Outcome::Nothing,
            Some(v) if self.held.len() + v.len() <= MAX_HELD => {
                for (target, value) in &v {
                    self.rev += 1;
                    let s = ParamSet {
                        rev: self.rev,
                        target: *target,
                        value: *value,
                    };
                    if self.held.insert(*target, s).is_some() {
                        self.counts.coalesced += 1;
                    }
                }
                self.counts.values += v.len() as u64;
                Outcome::Values(v.len())
            }
            other => {
                if other.is_some() {
                    self.counts.fallbacks += 1;
                }
                self.compile(doc, fresh, stopped)
            }
        };
        self.sent = Some(doc.clone());
        self.flush();
        outcome
    }

    fn compile(&mut self, doc: &PatchState, fresh: bool, stopped: bool) -> Outcome {
        self.generation += 1;
        self.rev += 1;
        let started = std::time::Instant::now();
        let mut g = match compile(doc, self.sample_rate, self.voice_count) {
            Ok(g) => g,
            Err(err) => {
                log::warn!(target: "graph", "compile failed generation={} error={err}; audio keeps the previous graph", self.generation);
                self.compile_error = Some(err.to_string());
                self.counts.failed += 1;
                return Outcome::Failed;
            }
        };
        log::debug!(
            target: "graph",
            "compiled generation={} rev={} fresh={fresh} modules={} took_us={}",
            self.generation,
            self.rev,
            doc.modules.len(),
            started.elapsed().as_micros()
        );
        self.compile_error = None;
        g.generation = self.generation;
        g.rev = self.rev;
        g.fresh = fresh;
        if stopped {
            let mut clocks = Vec::new();
            g.clocks(|id, _| clocks.push(id));
            for id in clocks {
                g.transport(id, Transport::Stop);
            }
        }
        // The graph carries every value before it; a replaced waiting graph is freed here.
        self.held.clear();
        self.held_graph = Some(Owned::new(&self.handle, g));
        self.counts.graphs += 1;
        Outcome::Compiled
    }

    /// Sends what waits, in order, as far as the queue takes it. Returns how many messages
    /// still wait.
    pub fn flush(&mut self) -> usize {
        let queued = self.graphs_queued();
        let Some(tx) = self.tx.as_mut() else {
            // No audio: nothing will take them.
            self.held_graph = None;
            self.held.clear();
            return 0;
        };
        if let Some(g) = self.held_graph.take() {
            if queued >= MAX_GRAPHS_QUEUED {
                self.held_graph = Some(g);
                return self.waiting();
            }
            match tx.push(ToAudio::Graph(g)) {
                Ok(()) => self.graphs_sent += 1,
                Err(rtrb::PushError::Full(ToAudio::Graph(g))) => {
                    self.held_graph = Some(g);
                    return self.waiting();
                }
                Err(_) => unreachable!("pushed a graph"),
            }
        }
        if !self.held.is_empty() {
            let mut order: Vec<ParamSet> = self.held.values().copied().collect();
            order.sort_by_key(|s| s.rev);
            for s in order {
                if tx.push(ToAudio::Set(s)).is_err() {
                    break;
                }
                self.values_sent += 1;
                self.held.remove(&s.target);
            }
        }
        self.waiting()
    }

    /// The last revision handed out (a value or a graph).
    pub fn rev(&self) -> u64 {
        self.rev
    }

    /// Graphs in the queue the audio thread has not taken yet.
    pub fn graphs_queued(&self) -> u64 {
        self.graphs_sent - Feedback::get(&self.feedback.graphs_taken)
    }

    /// Messages waiting to be sent.
    pub fn waiting(&self) -> usize {
        self.held_graph.is_some() as usize + self.held.len()
    }

    /// Messages requested and not yet taken by the audio thread: waiting here or in the
    /// queue (0 = the audio side has everything).
    pub fn pending(&self) -> u64 {
        self.waiting() as u64 + self.graphs_queued() + self.values_sent
            - Feedback::get(&self.feedback.sets_taken)
    }

    /// Sends a runtime command; false when the queue refused it.
    pub fn command(&mut self, c: Command) -> bool {
        self.commands.as_mut().is_some_and(|q| q.push(c).is_ok())
    }

    /// Sends a transport command; false when the queue refused it.
    pub fn transport(&mut self, t: (ModuleId, Transport)) -> bool {
        self.transport.as_mut().is_some_and(|q| q.push(t).is_ok())
    }
}

/// Delivers what the editor and the UI state ask for: document changes (when the editor is
/// dirty), then queued launches, previews, inspection and transport commands. A command the
/// queue refuses is reported, never counted as done.
pub fn deliver(editor: &mut PatchEditor, ui_state: &mut UiState, d: &mut Delivery) -> Outcome {
    let out = if editor.take_dirty() {
        let fresh = std::mem::take(&mut ui_state.loaded);
        let stopped = std::mem::take(&mut ui_state.load_stopped);
        d.sync(editor.state(), fresh, stopped)
    } else {
        d.flush();
        Outcome::Nothing
    };
    let mut refused = 0;
    for c in std::mem::take(&mut ui_state.launches) {
        if !d.command(c) {
            refused += 1;
        }
    }
    for t in std::mem::take(&mut ui_state.transport) {
        if !d.transport(t) {
            refused += 1;
        }
    }
    if refused > 0 && d.tx.is_some() {
        d.counts.dropped_actions += refused;
        log::warn!(target: "control", "{refused} command(s) not delivered: the audio queue is full");
        ui_state.last_message = Some(format!(
            "{refused} action(s) not delivered: audio is not responding"
        ));
    }
    out
}

/// MIDI CC messages `(channel, controller, value)`, with no editor frame: mappings with soft
/// takeover, buttons, a pending learn, then delivery. `now` in seconds on one steady clock.
pub fn midi(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    d: &mut Delivery,
    events: &[(u8, u8, u8)],
    now: f64,
) -> Outcome {
    perform::apply_cc(editor, ui_state, events, now);
    deliver(editor, ui_state, d)
}
