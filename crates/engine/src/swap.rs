//! Spike S1: swap compiled graphs with a fixed-length equal-power crossfade (brief section 7,
//! "Swap"), holding old and new in parallel for the crossfade window, then handing the old one
//! off for deferred (not audio-thread) collection.
//!
//! `basedrop::Owned<CompiledGraph>` gives single-owner, mutable-access, deferred-drop semantics:
//! dropping an `Owned` queues its node on the collector instead of deallocating inline (see
//! `basedrop`'s `owned.rs`). `Engine::request_swap` (control-thread work: it allocates) hands a
//! freshly-built `Owned<CompiledGraph>` to `Engine::process_block` (audio-thread work: it must
//! not allocate). Swapping `self.active` for the finished `incoming` graph just moves a pointer
//! and queues the old node's drop — no allocation, no deallocation, on the audio thread.

use basedrop::{Handle, Owned};

use crate::graph::{CompiledGraph, BLOCK};

/// Equal-power crossfade length. Brief section 7: "Run old and new graphs in parallel for
/// 10-20 ms". 15 ms is the midpoint.
pub const CROSSFADE_MS: f32 = 15.0;

pub struct Engine {
    active: Owned<CompiledGraph>,
    incoming: Option<(Owned<CompiledGraph>, usize)>,
    crossfade_samples: usize,
}

impl Engine {
    pub fn new(handle: &Handle, sample_rate: f32, initial_depth: f32) -> Self {
        Engine {
            active: Owned::new(handle, CompiledGraph::new(initial_depth)),
            incoming: None,
            crossfade_samples: (CROSSFADE_MS / 1000.0 * sample_rate).round() as usize,
        }
    }

    pub fn crossfade_samples(&self) -> usize {
        self.crossfade_samples
    }

    pub fn is_swapping(&self) -> bool {
        self.incoming.is_some()
    }

    /// Control-thread call: builds the new graph, carrying state over from the current active
    /// graph. Allocates (`Owned::new`) — never call from the audio thread. Hand the result to
    /// the audio thread through a channel (brief section 7.6: "single-slot lock-free channel");
    /// this spike uses `rtrb` (see `spike_s1_swap.rs`). If a swap is already in flight, the new
    /// graph carries state from the current `active` graph, not the in-flight `incoming` one —
    /// the spike's test schedules swaps far enough apart that this doesn't come up.
    /// Overlapping-swap choreography belongs to the real compiler (v1 milestone).
    pub fn build_swap(&self, handle: &Handle, new_depth: f32) -> Owned<CompiledGraph> {
        let new_graph = self.active.recompiled_with_depth(new_depth);
        Owned::new(handle, new_graph)
    }

    /// Audio-thread call: installs a graph built by `build_swap` (received off the lock-free
    /// channel) and starts its crossfade-in. No allocation.
    pub fn receive_swap(&mut self, new_graph: Owned<CompiledGraph>) {
        self.incoming = Some((new_graph, 0));
    }

    /// Audio-thread call: must not allocate or deallocate. Runs the active graph, and — while a
    /// swap is in flight — the incoming graph too, blending with an equal-power curve.
    #[inline]
    pub fn process_block(&mut self, sample_rate: f32, out: &mut [f32; BLOCK]) {
        let mut old_buf = [0f32; BLOCK];
        self.active.process_block(sample_rate, &mut old_buf);

        let mut finished = false;
        if let Some((new_graph, elapsed)) = self.incoming.as_mut() {
            let mut new_buf = [0f32; BLOCK];
            new_graph.process_block(sample_rate, &mut new_buf);

            for i in 0..BLOCK {
                let pos = *elapsed + i;
                if pos >= self.crossfade_samples {
                    out[i] = new_buf[i];
                } else {
                    let t = pos as f32 / self.crossfade_samples as f32;
                    let (g_old, g_new) = equal_power(t);
                    out[i] = old_buf[i] * g_old + new_buf[i] * g_new;
                }
            }
            *elapsed += BLOCK;
            finished = *elapsed >= self.crossfade_samples;
        } else {
            *out = old_buf;
        }

        if finished {
            // Move, not clone: drops the old `active` in place. `Owned::drop` only queues the
            // node on the collector (atomic pointer swap) — no deallocation happens here.
            let (new_graph, _) = self.incoming.take().unwrap();
            self.active = new_graph;
        }
    }
}

/// Equal-power (constant-power) crossfade curve: `(cos, sin)` of a quarter turn, so
/// `g_old^2 + g_new^2 == 1` throughout — brief section 7 names this explicitly.
/// `pub(crate)`: reused by `patch_engine.rs`, which generalizes this same crossfade mechanism to
/// `CompiledPatch` — see that module's doc comment for why it's a separate `Engine`-shaped type
/// rather than a generalization of this one in place.
#[inline]
pub(crate) fn equal_power(t: f32) -> (f32, f32) {
    let angle = t * std::f32::consts::FRAC_PI_2;
    (angle.cos(), angle.sin())
}
