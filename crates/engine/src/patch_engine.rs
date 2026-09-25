//! Generalizes spike S1's `swap.rs::Engine` (crossfade + `basedrop` deferred-drop) to the real
//! compiler's `CompiledPatch`: arbitrary topology and voice count instead of S1's fixed 2-node
//! mono graph with a single `cable_depth` knob.
//!
//! A separate `PatchEngine` type rather than generalizing `swap::Engine` in place: the two wrap
//! genuinely different shapes, not just different type parameters of the same shape.
//! `CompiledGraph` is mono, takes `sample_rate` as a `process_block` argument, and rebuilds via
//! `recompiled_with_depth(new_depth: f32)` (one knob). `CompiledPatch` is stereo, owns its
//! `sample_rate` internally, and rebuilds via `recompile(&mut old, &PatchState, sample_rate,
//! voice_count)` (a whole patch, arbitrary changes). Forcing both through one generic/trait would
//! mean abstracting over a mono-vs-stereo output shape and a one-knob-vs-whole-patch rebuild
//! signature for exactly two call sites — the "don't design for hypothetical future
//! requirements" case, not a real shared shape. The crossfade is linear here, not S1's
//! equal-power curve: see `process_block` and decisions.md "Live-edit crossfade is linear".
//!
//! **State carry happens on the audio thread, when a fade starts.** `receive_swap` (or the
//! promotion of a queued `pending` graph) runs `compile::carry_state` from the graph that is
//! actually playing at that instant: `incoming` if a fade is in flight, else `active`.
//! `carry_state` is allocation-free, so this needs no lock. A control-thread snapshot taken at
//! build time would be stale by the time a queued graph starts: notes released in between would
//! come back, envelopes would jump back in time.
//!
//! **MIDI reaches every running graph.** `note_on`/`note_off` apply to `active` and `incoming`.
//! A `pending` graph gets them through `carry_state` when its fade starts.
//!
//! `process_block` is allocation-free — `CompiledPatch::process_block` already is (see
//! `compile.rs`), and this wrapper adds no allocation of its own. Proven the same way as
//! `compile.rs`'s own RT-safety claim and spike S1's: `tests/patch_engine_swap.rs` runs the
//! audio-thread path inside `assert_no_alloc!`.

use basedrop::{Handle, Owned};
use kabl_core::PatchState;

use crate::compile::{carry_state, compile, CompileError, CompiledPatch, PendingLaunch};
use crate::graph::BLOCK;
use crate::keyboard::{Action, KeyEvent, Keyboard};
use crate::probe::{ProbeReport, ProbeTarget, WINDOW_SECS};
use crate::swap::CROSSFADE_MS;
use kabl_modules::builtins::KeySettings;

pub struct PatchEngine {
    active: Owned<CompiledPatch>,
    incoming: Option<(Owned<CompiledPatch>, usize)>,
    /// A swap that arrived while another was still crossfading in (brief section 7.6's
    /// overlapping-swap gap, see `receive_swap`). Held until the in-flight fade completes, then
    /// started as an ordinary fade from the now-promoted `active` — never dropped, never
    /// discontinuous, just delayed by at most one crossfade's worth of latency. Only one slot: a
    /// second overlapping arrival replaces it (last-request-wins), since nothing here needs an
    /// unbounded queue of edits that are about to be superseded anyway.
    pending: Option<Owned<CompiledPatch>>,
    crossfade_samples: usize,
    voice_count: usize,
    /// Launches waiting for their clock boundary, at most one per sequencer. Lives here, not in
    /// a graph, so graph swaps neither lose nor replay it (docs/composition-batch/design.md).
    launches: [Option<PendingLaunch>; MAX_PENDING],
    /// `launches` packed, for `process_block_with`.
    packed: [PendingLaunch; MAX_PENDING],
    /// One keyboard per `midi.in` of the playing graph, outside the graphs like the launches
    /// (docs/sound-palette-batch/keyboard.md).
    keyboards: [Option<Keyboard>; MAX_KEYBOARDS],
    /// Who holds each key (bit n = MIDI note n): a key held by both the controller and the
    /// preview is released only when both have let go (docs/find-play-save/README.md).
    held_controller: u128,
    held_preview: u128,
    /// Blocks until the preview's notes release; 0 = no preview running.
    preview_blocks: u32,
    /// The selected signal (D03): measured in the graph fading in, else the active one.
    probe: Option<ProbeTarget>,
    probe_window: u32,
    probe_seq: u64,
    /// The last finished window, until the callback takes it.
    report: Option<ProbeReport>,
}

/// Most notes one preview plays (a chord).
pub const MAX_PREVIEW_NOTES: usize = 4;
/// Longest preview, in seconds: the audio thread releases it after this whatever the UI does.
pub const MAX_PREVIEW_SECS: f32 = 10.0;

/// Most `midi.in` modules with a keyboard of their own; any beyond get no notes.
pub const MAX_KEYBOARDS: usize = 8;

pub const MAX_PENDING: usize = 32;
/// Sequencers one launch command can name.
pub const MAX_TARGETS: usize = 16;

/// When a launch lands, on its reference clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Timing {
    Now,
    NextStep,
    NextBar,
}

/// Pulses per bar: 16 sixteenths, 4/4.
pub const BAR_TICKS: u64 = 16;

/// A launch command from the UI: bank per sequencer (`targets[..count]`), timed on `clock`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Launch {
    pub clock: kabl_core::ModuleId,
    pub timing: Timing,
    pub targets: [(kabl_core::ModuleId, u8); MAX_TARGETS],
    pub count: usize,
}

impl Launch {
    pub fn new(
        clock: kabl_core::ModuleId,
        timing: Timing,
        targets: &[(kabl_core::ModuleId, u8)],
    ) -> Self {
        let mut t = [(0, 0); MAX_TARGETS];
        let count = targets.len().min(MAX_TARGETS);
        t[..count].copy_from_slice(&targets[..count]);
        Launch {
            clock,
            timing,
            targets: t,
            count,
        }
    }
}

/// A runtime command from the UI to the audio thread. Never in the op log, so undo, redo,
/// reload and graph swaps cannot replay one.
/// Unboxed on purpose: the audio thread drops what it pops, and must not free memory.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    Launch(Launch),
    /// Cancel one sequencer's pending launch, or every one.
    Cancel(Option<kabl_core::ModuleId>),
    /// Audition: press `notes[..count]` at `velocity` through the keyboards, release them
    /// after `blocks` blocks (at most `MAX_PREVIEW_SECS`). Replaces a preview still running.
    Preview {
        notes: [u8; MAX_PREVIEW_NOTES],
        count: u8,
        velocity: u8,
        blocks: u32,
    },
    /// Release the preview's notes now (those the controller doesn't also hold).
    PreviewStop,
    /// Measure this output (`None`: stop measuring). View only: no effect on the sound.
    Inspect(Option<ProbeTarget>),
}

const NO_LAUNCH: PendingLaunch = PendingLaunch {
    seq: 0,
    bank: 0,
    clock: 0,
    at: (0, 0),
};

impl PatchEngine {
    /// Control-thread call: compiles `patch` and wraps it for deferred drop. Allocates.
    pub fn new(
        handle: &Handle,
        patch: &PatchState,
        sample_rate: f32,
        voice_count: usize,
    ) -> Result<Self, CompileError> {
        let compiled = compile(patch, sample_rate, voice_count)?;
        let mut e = PatchEngine {
            active: Owned::new(handle, compiled),
            incoming: None,
            pending: None,
            crossfade_samples: (CROSSFADE_MS / 1000.0 * sample_rate).round() as usize,
            voice_count,
            launches: [None; MAX_PENDING],
            packed: [NO_LAUNCH; MAX_PENDING],
            keyboards: Default::default(),
            held_controller: 0,
            held_preview: 0,
            preview_blocks: 0,
            probe: None,
            probe_window: 1,
            probe_seq: 0,
            report: None,
        };
        e.probe_window = window_blocks(sample_rate);
        e.sync_keyboards();
        Ok(e)
    }

    /// An engine playing an already compiled graph.
    pub fn with_compiled(handle: &Handle, compiled: CompiledPatch, voice_count: usize) -> Self {
        let crossfade_samples = (CROSSFADE_MS / 1000.0 * compiled.sample_rate()).round() as usize;
        let mut e = PatchEngine {
            active: Owned::new(handle, compiled),
            incoming: None,
            pending: None,
            crossfade_samples,
            voice_count,
            launches: [None; MAX_PENDING],
            packed: [NO_LAUNCH; MAX_PENDING],
            keyboards: Default::default(),
            held_controller: 0,
            held_preview: 0,
            preview_blocks: 0,
            probe: None,
            probe_window: 1,
            probe_seq: 0,
            report: None,
        };
        e.probe_window = window_blocks(e.active.sample_rate());
        e.sync_keyboards();
        e
    }

    /// Audio-thread call: a key event from the MIDI controller. A key-up is held back while the
    /// preview also holds that key. No allocation.
    pub fn key(&mut self, e: KeyEvent) {
        match e {
            KeyEvent::On { note, .. } => self.held_controller |= bit(note),
            KeyEvent::Off { note } => {
                self.held_controller &= !bit(note);
                if self.held_preview & bit(note) != 0 {
                    return;
                }
            }
            KeyEvent::AllOff => {
                self.held_controller = 0;
                self.held_preview = 0;
                self.preview_blocks = 0;
            }
            KeyEvent::Sustain(_) => {}
        }
        self.send_key(e);
    }

    /// Audio-thread call: starts a preview (replacing one still running). No allocation.
    pub fn preview(&mut self, notes: &[u8], velocity: u8, blocks: u32) {
        self.preview_stop();
        let max = (MAX_PREVIEW_SECS * self.active.sample_rate() / BLOCK as f32) as u32;
        self.preview_blocks = blocks.clamp(1, max.max(1));
        for &note in notes.iter().take(MAX_PREVIEW_NOTES) {
            let note = note & 0x7F;
            self.held_preview |= bit(note);
            self.send_key(KeyEvent::On {
                note,
                velocity: velocity.clamp(1, 127),
            });
        }
    }

    /// Audio-thread call: releases the preview's keys, except those the controller holds.
    pub fn preview_stop(&mut self) {
        self.preview_blocks = 0;
        let held = std::mem::take(&mut self.held_preview);
        for note in 0..128u8 {
            if held & bit(note) != 0 && self.held_controller & bit(note) == 0 {
                self.send_key(KeyEvent::Off { note });
            }
        }
    }

    /// Keys the preview holds right now (bit n = note n), for tests and the UI.
    pub fn preview_keys(&self) -> u128 {
        self.held_preview
    }

    fn send_key(&mut self, e: KeyEvent) {
        self.sync_keyboards();
        let PatchEngine {
            keyboards,
            active,
            incoming,
            ..
        } = self;
        for kb in keyboards.iter_mut().flatten() {
            let id = kb.id;
            kb.event(e, &mut |a| act(active, incoming, id, a));
        }
    }

    /// Keys physically down (the most any keyboard holds) and voices sounding (all keyboards).
    pub fn keys(&self) -> (usize, usize) {
        let kbs = self.keyboards.iter().flatten();
        (
            kbs.clone().map(Keyboard::keys_held).max().unwrap_or(0),
            kbs.map(Keyboard::voices_sounding).sum(),
        )
    }

    /// Matches the keyboards to the `midi.in` modules of the graph playing (fading in, else
    /// active): a new module gets a keyboard, a deleted one's keyboard releases and goes, and
    /// changed settings apply (a mode change releases). `fresh`: a Load, every keyboard
    /// starts over. No allocation.
    fn sync(&mut self, fresh: bool) {
        if fresh {
            // Every keyboard starts over, so nobody holds a key.
            self.held_controller = 0;
            self.held_preview = 0;
            self.preview_blocks = 0;
        }
        let PatchEngine {
            keyboards,
            active,
            incoming,
            voice_count,
            ..
        } = self;
        let mut found = [(0, KeySettings::default()); MAX_KEYBOARDS];
        let mut n = 0;
        {
            let graph: &CompiledPatch = match incoming {
                Some((g, _)) => g,
                None => active,
            };
            graph.keyboards(|id, s| {
                if n < MAX_KEYBOARDS {
                    found[n] = (id, s);
                    n += 1;
                }
            });
        }
        let found = &found[..n];
        for slot in keyboards.iter_mut() {
            if let Some(kb) = slot {
                if fresh || !found.iter().any(|&(id, _)| id == kb.id) {
                    let id = kb.id;
                    kb.event(KeyEvent::AllOff, &mut |a| act(active, incoming, id, a));
                    *slot = None;
                }
            }
        }
        for &(id, s) in found {
            match keyboards.iter_mut().flatten().find(|kb| kb.id == id) {
                Some(kb) => kb.set_settings(s, &mut |a| act(active, incoming, id, a)),
                None => {
                    if let Some(slot) = keyboards.iter_mut().find(|k| k.is_none()) {
                        *slot = Some(Keyboard::new(id, s, *voice_count));
                    }
                }
            }
        }
    }

    fn sync_keyboards(&mut self) {
        self.sync(false);
    }

    pub fn crossfade_samples(&self) -> usize {
        self.crossfade_samples
    }

    /// True while any fade is in flight or queued — includes a `pending` swap waiting on the
    /// current one to finish, not just the one actively blending.
    pub fn is_swapping(&self) -> bool {
        self.incoming.is_some() || self.pending.is_some()
    }

    /// Direct access to the active graph — for control-thread setup (e.g. triggering MIDI notes
    /// via `as_any_mut` downcasting) before the audio thread starts pulling blocks. Not meant to
    /// be called once real-time processing has started; nothing here prevents it, but nothing
    /// needs it to be prevented yet either (no standalone binary/audio callback exists to misuse
    /// it from).
    pub fn active_mut(&mut self) -> &mut CompiledPatch {
        &mut self.active
    }

    /// Control-thread call: compiles `patch` and wraps it for deferred drop. Allocates — never
    /// call from the audio thread. Hand the result to the audio thread through a channel
    /// (`rtrb`, brief section 7.6) and install it with `receive_swap`, which carries state.
    pub fn build_swap(
        &self,
        handle: &Handle,
        patch: &PatchState,
    ) -> Result<Owned<CompiledPatch>, CompileError> {
        let new_patch = compile(patch, self.active.sample_rate(), self.voice_count)?;
        Ok(Owned::new(handle, new_patch))
    }

    /// Same-thread convenience for tests and offline rendering: wrap and install `new_patch`.
    pub fn finish_swap(&mut self, handle: &Handle, new_patch: CompiledPatch) {
        self.receive_swap(Owned::new(handle, new_patch));
    }

    /// Audio-thread call: installs a graph built by `build_swap`. No allocation.
    ///
    /// No fade in flight: carries state from `active` and starts the crossfade now. Fade in
    /// flight (brief section 7.6's overlapping swaps): queues `new_patch` in `pending` instead of
    /// restarting the fade (which would click). A newer arrival replaces an older pending one
    /// (last request wins; the replaced graph is dropped through `basedrop`, not freed here).
    pub fn receive_swap(&mut self, mut new_patch: Owned<CompiledPatch>) {
        // An edit that replaces a queued fresh load is built on the loaded patch, so it must not
        // carry from the old one either.
        if let Some(p) = &self.pending {
            new_patch.fresh |= p.fresh;
        }
        // A Load drops pending launches: the loaded sequencers start on their startup banks.
        if new_patch.fresh {
            self.launches = [None; MAX_PENDING];
        }
        if self.incoming.is_some() {
            self.pending = Some(new_patch);
        } else {
            carry_state(&mut self.active, &mut new_patch);
            let fresh = new_patch.fresh;
            self.move_probe(&mut new_patch);
            self.incoming = Some((new_patch, 0));
            self.sync(fresh);
        }
    }

    /// Audio-thread call: the measurement moves to `new`, the graph fading in. No allocation.
    fn move_probe(&mut self, new: &mut CompiledPatch) {
        if self.probe.is_some() {
            self.active.set_probe(None);
            new.set_probe(self.probe);
        }
    }

    /// Audio-thread call: measure `target` from now on (`None`: stop). No allocation.
    pub fn inspect(&mut self, target: Option<ProbeTarget>) {
        self.probe = target;
        self.report = None;
        match self.incoming.as_mut() {
            Some((g, _)) => {
                self.active.set_probe(None);
                g.set_probe(target);
            }
            None => self.active.set_probe(target),
        }
    }

    /// Audio-thread call: the last finished measurement window, if one is waiting.
    pub fn take_probe_report(&mut self) -> Option<ProbeReport> {
        self.report.take()
    }

    /// Audio-thread call: note-on for `voice` in every running graph. No allocation.
    pub fn note_on(&mut self, voice: usize, semitones: f32, velocity: f32) {
        self.active.note_on(voice, semitones, velocity);
        if let Some((g, _)) = self.incoming.as_mut() {
            g.note_on(voice, semitones, velocity);
        }
    }

    /// Audio-thread call: `CompiledPatch::seq_steps` of the graph fading in, else the active one.
    pub fn seq_steps(&self, f: impl FnMut(kabl_core::ModuleId, usize)) {
        match &self.incoming {
            Some((g, _)) => g.seq_steps(f),
            None => self.active.seq_steps(f),
        }
    }

    /// Audio-thread call: `CompiledPatch::delays` of the graph fading in, else the active one.
    pub fn delays(
        &self,
        f: impl FnMut(kabl_core::ModuleId, kabl_modules::builtins::DelayLock, f32),
    ) {
        match &self.incoming {
            Some((g, _)) => g.delays(f),
            None => self.active.delays(f),
        }
    }

    /// Audio-thread call: a clock transport command to every running graph (a `pending` graph
    /// gets it through `carry_state`). Stop turns the launches pending on that clock into
    /// selections: armed now, they start on the sequencers' next edges (Run). No allocation.
    pub fn transport(&mut self, id: kabl_core::ModuleId, t: kabl_modules::builtins::Transport) {
        self.active.transport(id, t);
        if let Some((g, _)) = self.incoming.as_mut() {
            g.transport(id, t);
        }
        if t == kabl_modules::builtins::Transport::Stop {
            for slot in self.launches.iter_mut() {
                if let Some(l) = slot.filter(|l| l.clock == id) {
                    *slot = None;
                    arm_all(&mut self.active, &mut self.incoming, l.seq, l.bank as usize);
                }
            }
        }
    }

    /// Audio-thread call: a bank launch. Each target replaces its sequencer's pending launch.
    /// Now, or a stopped reference clock: armed at once (a selection while stopped). Otherwise
    /// queued for the clock's next step or bar. An unknown clock drops the command. No
    /// allocation.
    pub fn launch(&mut self, l: &Launch) {
        let graph = match &self.incoming {
            Some((g, _)) => g,
            None => &self.active,
        };
        let Some(clock) = graph.clock(l.clock) else {
            return;
        };
        let (epoch, next) = clock.next_tick();
        let at = match l.timing {
            _ if !clock.running() => None,
            Timing::Now => None,
            Timing::NextStep => Some((epoch, next)),
            Timing::NextBar => Some((epoch, next.div_ceil(BAR_TICKS) * BAR_TICKS)),
        };
        for &(seq, bank) in &l.targets[..l.count.min(MAX_TARGETS)] {
            self.cancel(Some(seq));
            match at {
                None => arm_all(&mut self.active, &mut self.incoming, seq, bank as usize),
                Some(at) => {
                    if let Some(slot) = self.launches.iter_mut().find(|s| s.is_none()) {
                        *slot = Some(PendingLaunch {
                            seq,
                            bank,
                            clock: l.clock,
                            at,
                        });
                    }
                }
            }
        }
    }

    /// Audio-thread call: applies a UI command. No allocation.
    pub fn command(&mut self, c: &Command) {
        match c {
            Command::Launch(l) => self.launch(l),
            Command::Cancel(s) => self.cancel(*s),
            Command::Preview {
                notes,
                count,
                velocity,
                blocks,
            } => {
                let n = (*count as usize).min(MAX_PREVIEW_NOTES);
                self.preview(&notes[..n], *velocity, *blocks);
            }
            Command::PreviewStop => self.preview_stop(),
            Command::Inspect(t) => self.inspect(*t),
        }
    }

    /// Audio-thread call: drops the pending launch and the arm of sequencer `seq`, or of every
    /// sequencer. No allocation.
    pub fn cancel(&mut self, seq: Option<kabl_core::ModuleId>) {
        for slot in self.launches.iter_mut() {
            if slot.is_some_and(|l| seq.is_none_or(|s| s == l.seq)) {
                *slot = None;
            }
        }
        let disarm = |g: &mut CompiledPatch| match seq {
            Some(id) => g.with_seq(id, |s| s.cancel()),
            None => g.for_each_seq(|s| s.cancel()),
        };
        disarm(&mut self.active);
        if let Some((g, _)) = self.incoming.as_mut() {
            disarm(g);
        }
    }

    /// Audio-thread call: `f(id, step, playing bank, queued bank)` for every sequencer of the
    /// graph fading in, else the active one; queued = a pending launch or an arm. No allocation.
    pub fn seqs(&self, mut f: impl FnMut(kabl_core::ModuleId, usize, usize, Option<usize>)) {
        let graph = match &self.incoming {
            Some((g, _)) => g,
            None => &self.active,
        };
        graph.seqs(|id, step, bank, armed| {
            let queued = self
                .launches
                .iter()
                .flatten()
                .find(|l| l.seq == id)
                .map(|l| l.bank as usize)
                .or(armed);
            f(id, step, bank, queued)
        });
    }

    /// Audio-thread call: `CompiledPatch::lfos` of the graph fading in, else the active one.
    pub fn lfos(&self, f: impl FnMut(kabl_core::ModuleId, kabl_modules::builtins::LfoSync)) {
        match &self.incoming {
            Some((g, _)) => g.lfos(f),
            None => self.active.lfos(f),
        }
    }

    /// Audio-thread call: `CompiledPatch::clocks` of the graph fading in, else the active one.
    pub fn clocks(&self, f: impl FnMut(kabl_core::ModuleId, bool)) {
        match &self.incoming {
            Some((g, _)) => g.clocks(f),
            None => self.active.clocks(f),
        }
    }

    /// Audio-thread call: note-off for `voice` in every running graph. No allocation.
    pub fn note_off(&mut self, voice: usize) {
        self.active.note_off(voice);
        if let Some((g, _)) = self.incoming.as_mut() {
            g.note_off(voice);
        }
    }

    /// Audio-thread call: must not allocate or deallocate. Runs the active graph, and — while a
    /// swap is in flight — the incoming graph too, blending with an equal-power curve. Stereo,
    /// unlike `swap::Engine::process_block` (S1's graph was mono).
    #[inline]
    pub fn process_block(&mut self, out_left: &mut [f32; BLOCK], out_right: &mut [f32; BLOCK]) {
        if self.preview_blocks > 0 {
            self.preview_blocks -= 1;
            if self.preview_blocks == 0 {
                self.preview_stop();
            }
        }
        let mut n = 0;
        for l in self.launches.iter().flatten() {
            self.packed[n] = *l;
            n += 1;
        }
        let launches = &self.packed[..n];
        self.active.process_block_with(launches);
        let old_left = *self.active.left();
        let old_right = *self.active.right();

        let mut finished = false;
        if let Some((new_patch, elapsed)) = self.incoming.as_mut() {
            new_patch.process_block_with(launches);
            let new_left = *new_patch.left();
            let new_right = *new_patch.right();

            for i in 0..BLOCK {
                let pos = *elapsed + i;
                if pos >= self.crossfade_samples {
                    out_left[i] = new_left[i];
                    out_right[i] = new_right[i];
                } else {
                    // Linear (gains sum to 1), not equal-power: both graphs start from the same
                    // carried state, so they are strongly correlated, and equal-power would
                    // swell an unchanged signal by up to +41 % mid-fade on every edit.
                    let g_new = pos as f32 / self.crossfade_samples as f32;
                    let g_old = 1.0 - g_new;
                    out_left[i] = old_left[i] * g_old + new_left[i] * g_new;
                    out_right[i] = old_right[i] * g_old + new_right[i] * g_new;
                }
            }
            *elapsed += BLOCK;
            finished = *elapsed >= self.crossfade_samples;
        } else {
            *out_left = old_left;
            *out_right = old_right;
        }

        // A launch whose boundary passed was armed in every running graph this block. One whose
        // clock is gone from the playing graph is dropped.
        if n > 0 {
            let graph = match &self.incoming {
                Some((g, _)) => g,
                None => &self.active,
            };
            for slot in self.launches.iter_mut() {
                if let Some(l) = slot {
                    let done = graph
                        .clock(l.clock)
                        .is_none_or(|c| l.reached_by(c.next_tick()) && c.next_tick() != l.at);
                    if done {
                        *slot = None;
                    }
                }
            }
        }

        if self.probe.is_some() {
            let fading = self.incoming.is_some();
            let g: &mut CompiledPatch = match self.incoming.as_mut() {
                Some((g, _)) => g,
                None => &mut self.active,
            };
            if let Some(r) = g.probe_block_end(self.probe_window, fading, self.probe_seq) {
                self.report = Some(r);
                self.probe_seq += 1;
            }
        }

        if finished {
            // Move, not clone: drops the old `active` in place. `Owned::drop` only queues the
            // node on the collector (atomic pointer swap) — no deallocation happens here.
            let (new_patch, _) = self.incoming.take().unwrap();
            self.active = new_patch;
            // A swap queued while this fade was in flight (see `receive_swap`) starts now, from
            // the graph that was just promoted — same as any other fresh `receive_swap`, just
            // deferred instead of dropped or stepped on.
            if let Some(mut pending) = self.pending.take() {
                carry_state(&mut self.active, &mut pending);
                let fresh = pending.fresh;
                self.move_probe(&mut pending);
                self.incoming = Some((pending, 0));
                self.sync(fresh);
            }
        }
    }
}

/// Blocks per probe window at `sample_rate` (at least one).
fn window_blocks(sample_rate: f32) -> u32 {
    ((WINDOW_SECS * sample_rate / BLOCK as f32).ceil() as u32).max(1)
}

fn bit(note: u8) -> u128 {
    1u128 << (note & 0x7F)
}

/// A keyboard action in every running graph.
fn act(
    active: &mut CompiledPatch,
    incoming: &mut Option<(Owned<CompiledPatch>, usize)>,
    id: kabl_core::ModuleId,
    a: Action,
) {
    active.key_action(id, a);
    if let Some((g, _)) = incoming.as_mut() {
        g.key_action(id, a);
    }
}

/// Arms `seq` with `bank` from the start of the next block in the running graphs.
fn arm_all(
    active: &mut CompiledPatch,
    incoming: &mut Option<(Owned<CompiledPatch>, usize)>,
    seq: kabl_core::ModuleId,
    bank: usize,
) {
    active.with_seq(seq, |s| s.arm(bank, 0));
    if let Some((g, _)) = incoming.as_mut() {
        g.with_seq(seq, |s| s.arm(bank, 0));
    }
}

/// Control-thread end of the graph handoff queue (see `swap_channel`).
pub struct SwapSender {
    tx: rtrb::Producer<Owned<CompiledPatch>>,
    /// Newest graph that didn't fit in the queue yet. A newer `send` replaces it: only the
    /// latest edit matters, and the replaced graph is dropped here, on the control thread.
    unsent: Option<Owned<CompiledPatch>>,
}

/// Bounded single-producer/single-consumer queue of compiled graphs from the control thread to
/// the audio thread. The audio side calls `PatchEngine::drain_swaps` every callback.
pub fn swap_channel(capacity: usize) -> (SwapSender, rtrb::Consumer<Owned<CompiledPatch>>) {
    let (tx, rx) = rtrb::RingBuffer::new(capacity);
    (SwapSender { tx, unsent: None }, rx)
}

impl SwapSender {
    /// Queues `graph`, or holds it (replacing any older held graph) if the queue is full.
    /// Returns true if a graph is still held; call `flush` again later.
    pub fn send(&mut self, graph: Owned<CompiledPatch>) -> bool {
        self.unsent = Some(graph);
        self.flush()
    }

    /// Retries the held graph. Returns true if it is still held.
    pub fn flush(&mut self) -> bool {
        if let Some(graph) = self.unsent.take() {
            if let Err(rtrb::PushError::Full(graph)) = self.tx.push(graph) {
                self.unsent = Some(graph);
            }
        }
        self.unsent.is_some()
    }
}

impl PatchEngine {
    /// Audio-thread call: installs every graph waiting in the queue, in order (each one after
    /// the first replaces the previous as `pending`). No allocation.
    pub fn drain_swaps(&mut self, rx: &mut rtrb::Consumer<Owned<CompiledPatch>>) {
        while let Ok(graph) = rx.pop() {
            self.receive_swap(graph);
        }
    }
}
