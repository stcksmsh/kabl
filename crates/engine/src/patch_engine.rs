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
//! requirements" case, not a real shared shape. The crossfade math itself (`equal_power`, brief
//! section 7's "10-20ms equal-power blend") is shared via `swap::equal_power` (`pub(crate)`),
//! since that's genuinely the same formula, not just structurally similar code.
//!
//! `process_block` is allocation-free — `CompiledPatch::process_block` already is (see
//! `compile.rs`), and this wrapper adds no allocation of its own. Proven the same way as
//! `compile.rs`'s own RT-safety claim and spike S1's: `tests/patch_engine_swap.rs` runs the
//! audio-thread path inside `assert_no_alloc!`.

use basedrop::{Handle, Owned};
use kabl_core::PatchState;

use crate::compile::{compile, recompile, CompileError, CompiledPatch};
use crate::graph::BLOCK;
use crate::swap::{equal_power, CROSSFADE_MS};

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
}

impl PatchEngine {
    /// Control-thread call: compiles `patch` and wraps it for deferred drop. Allocates.
    pub fn new(
        handle: &Handle,
        patch: &PatchState,
        sample_rate: f32,
        voice_count: usize,
    ) -> Result<Self, CompileError> {
        let compiled = compile(patch, sample_rate, voice_count)?;
        Ok(PatchEngine {
            active: Owned::new(handle, compiled),
            incoming: None,
            pending: None,
            crossfade_samples: (CROSSFADE_MS / 1000.0 * sample_rate).round() as usize,
            voice_count,
        })
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

    /// Control-thread call: recompiles `patch` against the current active graph, carrying state
    /// over (brief section 7.5, via `compile::recompile`), and wraps the result for deferred
    /// drop. Allocates — never call from the audio thread. Hand the result to the audio thread
    /// through a channel (`rtrb`, brief section 7.6) and install it with `receive_swap`.
    ///
    /// State is always carried from `active`, never from an in-flight `incoming` — if a second
    /// `build_swap` runs before the first fade finishes, its state snapshot is up to one
    /// crossfade behind what's about to become active (see `receive_swap`'s queueing). That's a
    /// real staleness, not a new category of one: `active`'s state is already only ever a
    /// control-thread snapshot of "as of whenever this ran," not synced to the audio thread's
    /// exact position either way.
    pub fn build_swap(
        &mut self,
        handle: &Handle,
        patch: &PatchState,
    ) -> Result<Owned<CompiledPatch>, CompileError> {
        let sample_rate = self.active.sample_rate();
        let new_patch = recompile(&mut self.active, patch, sample_rate, self.voice_count)?;
        Ok(Owned::new(handle, new_patch))
    }

    /// Audio-thread call: installs a graph built by `build_swap` and starts its crossfade-in. No
    /// allocation.
    ///
    /// Brief section 7.6's overlapping-swap case: if a fade is already in flight, this doesn't
    /// restart it (which would snap the blended output straight to a different signal — the exact
    /// click the crossfade exists to prevent). It queues `new_patch` in `pending` instead;
    /// `process_block` starts it as a normal fade the moment the current one finishes.
    pub fn receive_swap(&mut self, new_patch: Owned<CompiledPatch>) {
        if self.incoming.is_some() {
            self.pending = Some(new_patch);
        } else {
            self.incoming = Some((new_patch, 0));
        }
    }

    /// Audio-thread call: must not allocate or deallocate. Runs the active graph, and — while a
    /// swap is in flight — the incoming graph too, blending with an equal-power curve. Stereo,
    /// unlike `swap::Engine::process_block` (S1's graph was mono).
    #[inline]
    pub fn process_block(&mut self, out_left: &mut [f32; BLOCK], out_right: &mut [f32; BLOCK]) {
        self.active.process_block();
        let old_left = *self.active.left();
        let old_right = *self.active.right();

        let mut finished = false;
        if let Some((new_patch, elapsed)) = self.incoming.as_mut() {
            new_patch.process_block();
            let new_left = *new_patch.left();
            let new_right = *new_patch.right();

            for i in 0..BLOCK {
                let pos = *elapsed + i;
                if pos >= self.crossfade_samples {
                    out_left[i] = new_left[i];
                    out_right[i] = new_right[i];
                } else {
                    let t = pos as f32 / self.crossfade_samples as f32;
                    let (g_old, g_new) = equal_power(t);
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

        if finished {
            // Move, not clone: drops the old `active` in place. `Owned::drop` only queues the
            // node on the collector (atomic pointer swap) — no deallocation happens here.
            let (new_patch, _) = self.incoming.take().unwrap();
            self.active = new_patch;
            // A swap queued while this fade was in flight (see `receive_swap`) starts now, from
            // the graph that was just promoted — same as any other fresh `receive_swap`, just
            // deferred instead of dropped or stepped on.
            if let Some(pending) = self.pending.take() {
                self.incoming = Some((pending, 0));
            }
        }
    }
}
