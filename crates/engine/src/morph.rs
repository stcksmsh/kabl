//! Cue macro morphs: a launched cue can carry target values for macros and a morph length in
//! pulses (16ths). The engine, not the interface, moves the macro: a morph starts when the
//! cue's boundary on its reference clock arrives and its progress is the clock's own position,
//! so a tempo change stretches it, a stop freezes it and a restart or locate finishes it.
//!
//! Rules (the tests in `tests/cue_macros.rs` pin each one):
//! - A macro with no target in a cue is not touched.
//! - Length 0, or a clock that is not running when the cue lands: the macro jumps.
//! - The next cue to land starts from where the macro is at that moment; the old morph ends.
//! - A hand, MIDI or host-automation write to the macro ends its morph and the macro stays
//!   where that write put it. The next cue that has a target for it takes over again.
//! - After a morph the engine holds the macro at its target (a recompile starts from the
//!   document's stored value, which the morph never edited) until a write or a Load.
//!
//! Values are written once per block into the macro's param; the macro module glides them over
//! 20 ms, so there is no zipper noise. Nothing here allocates.

use kabl_core::ModuleId;

use crate::compile::CompiledPatch;
use crate::runtime::RuntimeTarget;

/// Macro targets one launch can carry.
pub const MAX_MACRO_TARGETS: usize = 8;
/// Macros the engine drives at once.
pub const MAX_DRIVEN: usize = 16;

/// One macro's target in a cue: macro module `id`, output `index` (0 = m1), value 0..1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MacroTarget {
    pub id: ModuleId,
    pub index: u8,
    pub value: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pending {
    pub target: MacroTarget,
    pub pulses: f32,
    pub clock: ModuleId,
    /// Boundary `(epoch, tick)` on `clock`; `None`: next block.
    pub at: Option<(u32, u64)>,
}

#[derive(Clone, Copy, Debug)]
struct Run {
    from: f32,
    pulses: f32,
    elapsed: f32,
    clock: ModuleId,
    last: (u32, f64),
}

#[derive(Clone, Copy, Debug)]
struct Driven {
    id: ModuleId,
    index: u8,
    value: f32,
    to: f32,
    run: Option<Run>,
}

fn target(id: ModuleId, index: u8) -> RuntimeTarget {
    RuntimeTarget::Param {
        id,
        kind: "macro",
        index: index as u16,
    }
}

#[derive(Default)]
pub struct Morphs {
    pending: [Option<Pending>; MAX_DRIVEN],
    driven: [Option<Driven>; MAX_DRIVEN],
}

impl Morphs {
    pub fn clear(&mut self) {
        *self = Morphs::default();
    }

    /// Queues `p`, replacing a pending morph of the same macro. Dropped when the table is full.
    pub fn schedule(&mut self, p: Pending) {
        let same = |q: &Pending| (q.target.id, q.target.index) == (p.target.id, p.target.index);
        let slot = self
            .pending
            .iter()
            .position(|q| q.as_ref().is_some_and(same))
            .or_else(|| self.pending.iter().position(|q| q.is_none()));
        if let Some(i) = slot {
            self.pending[i] = Some(p);
        }
    }

    /// Drops every pending morph, or those of reference clock `clock`.
    pub fn cancel_pending(&mut self) {
        self.pending.fill(None);
    }

    /// A write to macro `index` of `id` from outside (hand, MIDI, host): its morph ends here.
    pub fn release(&mut self, t: RuntimeTarget) {
        let RuntimeTarget::Param {
            id,
            kind: "macro",
            index,
        } = t
        else {
            return;
        };
        for d in self.driven.iter_mut() {
            if d.is_some_and(|d| (d.id, d.index as u16) == (id, index)) {
                *d = None;
            }
        }
    }

    /// Starts the morphs whose boundary has come and moves the running ones. `graph` is the
    /// one the clocks are read from. `out` gets `(target, value)` for every driven macro, to
    /// write into the graphs; returns how many.
    pub fn step(
        &mut self,
        graph: &CompiledPatch,
        out: &mut [(RuntimeTarget, f32); MAX_DRIVEN],
    ) -> usize {
        for slot in self.pending.iter_mut() {
            let Some(p) = *slot else { continue };
            let Some(clock) = graph.clock(p.clock) else {
                *slot = None;
                continue;
            };
            if !p.at.is_none_or(|at| reached(at, clock.next_tick())) {
                continue;
            }
            *slot = None;
            let t = target(p.target.id, p.target.index);
            let existing = self
                .driven
                .iter()
                .position(|d| d.is_some_and(|d| (d.id, d.index) == (p.target.id, p.target.index)));
            let Some(from) = existing
                .and_then(|i| self.driven[i].map(|d| d.value))
                .or_else(|| graph.runtime_value(t))
            else {
                continue;
            };
            let Some(i) = existing.or_else(|| self.driven.iter().position(|d| d.is_none())) else {
                continue;
            };
            let (epoch, pos) = (clock.next_tick().0, clock.position());
            let run = (clock.running() && p.pulses > 0.0).then(|| Run {
                from,
                pulses: p.pulses,
                // The boundary was passed inside the last block: count from it.
                elapsed: p.at.map_or(0.0, |at| {
                    if at.0 == epoch {
                        (pos - at.1 as f64).max(0.0) as f32
                    } else {
                        0.0
                    }
                }),
                clock: p.clock,
                last: (epoch, pos),
            });
            self.driven[i] = Some(Driven {
                id: p.target.id,
                index: p.target.index,
                value: if run.is_some() { from } else { p.target.value },
                to: p.target.value,
                run,
            });
        }
        let mut n = 0;
        for slot in self.driven.iter_mut() {
            let Some(d) = slot else { continue };
            if let Some(r) = &mut d.run {
                let now = graph
                    .clock(r.clock)
                    .map(|c| (c.next_tick().0, c.position()));
                match now {
                    // A restart or a locate (a jump of more than a pulse and a half either way)
                    // finishes. The reported position itself steps by up to a pulse at a
                    // stop or start, which counts as no progress.
                    Some((e, p)) if e == r.last.0 && (p - r.last.1).abs() <= 1.5 => {
                        r.elapsed += (p - r.last.1).max(0.0) as f32;
                        r.last.1 = p;
                        if r.elapsed >= r.pulses {
                            d.value = d.to;
                            d.run = None;
                        } else {
                            let x = r.elapsed / r.pulses;
                            d.value = r.from + (d.to - r.from) * x;
                        }
                    }
                    _ => {
                        d.value = d.to;
                        d.run = None;
                    }
                }
            }
            out[n] = (target(d.id, d.index), d.value);
            n += 1;
        }
        n
    }

    /// `f(macro module, output index, value, target, progress 0..1)` for each driven macro.
    pub fn report(&self, mut f: impl FnMut(ModuleId, u8, f32, f32, f32)) {
        for d in self.driven.iter().flatten() {
            let progress = d.run.map_or(1.0, |r| r.elapsed / r.pulses);
            f(d.id, d.index, d.value, d.to, progress);
        }
    }
}

/// `next_tick` counts pulses started, so pulse `at.1` has started once it is past it.
fn reached(at: (u32, u64), (epoch, tick): (u32, u64)) -> bool {
    epoch != at.0 || tick > at.1
}
