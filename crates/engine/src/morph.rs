//! Engine morphs of a parameter. A launched cue can carry target values for macros and a morph
//! length in pulses (16ths); a snapshot recall carries start and target values for any params
//! (`MorphPart`). The engine, not the interface, moves the value: a morph starts when its
//! boundary on its reference clock arrives and its progress is the clock's own position, so a
//! tempo change stretches it, a stop freezes it and a restart or locate finishes it.
//!
//! Rules (the tests in `tests/cue_macros.rs` and `tests/param_morphs.rs` pin each one):
//! - A macro with no target in a cue is not touched.
//! - Length 0, or a clock that is not running when the morph starts: the value jumps.
//! - The next cue to land starts from where the macro is at that moment; the old morph ends.
//!   A recall starts from the `from` it carries.
//! - A hand, MIDI or host-automation write to the param ends its morph and the param stays
//!   where that write put it. The next morph that has a target for it takes over again.
//! - After a morph the engine holds the param at its target (a recompile starts from the
//!   document's stored value) until a write or a Load.
//! - Continuous recall targets move in knob travel; a stepped one holds `from` until half the
//!   length, then jumps. Cue macros move linearly in their own units.
//! - A recall morph always lands: the document already holds `to`, so every path that drops
//!   one (`Morphs::land`) writes `to` instead. Cue macros are not landed.
//!
//! Values are written once per block. A macro glides them over 20 ms; a recall target the
//! module does not smooth ramps to each block's value over `RAMP_MS`. Nothing here allocates.

use kabl_core::ModuleId;
use kabl_modules::{ParamInfo, Taper};

use crate::compile::CompiledPatch;
use crate::runtime::RuntimeTarget;

/// Macro targets one launch can carry.
pub const MAX_MACRO_TARGETS: usize = 16;
/// Params the engine drives at once, and pending morphs.
pub const MAX_DRIVEN: usize = 64;
/// Targets of one recall: one per exposed control of a composite.
pub const MAX_MORPH_TARGETS: usize = 32;
/// Targets one `MorphPart` carries.
pub const PART_TARGETS: usize = 8;
/// Every recall morph one `clear` can drop: pending, driven and staged.
const MAX_LANDS: usize = 2 * MAX_DRIVEN + MAX_MORPH_TARGETS;

/// One macro's target in a cue: macro module `id`, output `index` (0 = m1), value 0..1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MacroTarget {
    pub id: ModuleId,
    pub index: u8,
    pub value: f32,
}

/// One param of a recall: from `from` to `to`, in the param's own units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MorphTarget {
    pub target: RuntimeTarget,
    pub from: f32,
    pub to: f32,
}

/// Part of a recall (`Command::Morph`). Parts with the same `seq` are staged; the one with
/// `last` starts all of them in the next block, over `pulses` of `clock`. A part of another
/// `seq` replaces an unfinished set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MorphPart {
    pub seq: u32,
    pub last: bool,
    pub clock: ModuleId,
    pub pulses: f32,
    pub count: u8,
    pub targets: [MorphTarget; PART_TARGETS],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pending {
    pub target: RuntimeTarget,
    /// The start value of a recall; `None`: a cue macro, from where it is.
    pub from: Option<f32>,
    pub to: f32,
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
    target: RuntimeTarget,
    /// A recall's param, for its taper; `None`: a cue macro, linear.
    info: Option<&'static ParamInfo>,
    value: f32,
    to: f32,
    run: Option<Run>,
    /// A recall that has arrived: the graphs hold `to` (and the document does), so it is no
    /// longer written. Writing it again each block would restart its ramp.
    arrived: bool,
}

/// The runtime target of macro output `index` of `id`.
pub(crate) fn target(id: ModuleId, index: u8) -> RuntimeTarget {
    RuntimeTarget::Param {
        id,
        kind: "macro",
        index: index as u16,
    }
}

pub struct Morphs {
    pending: [Option<Pending>; MAX_DRIVEN],
    driven: [Option<Driven>; MAX_DRIVEN],
    staged: [Option<MorphTarget>; MAX_MORPH_TARGETS],
    staged_seq: Option<u32>,
    lands: [Option<(RuntimeTarget, f32)>; MAX_LANDS],
}

impl Default for Morphs {
    fn default() -> Self {
        Morphs {
            pending: [None; MAX_DRIVEN],
            driven: [None; MAX_DRIVEN],
            staged: [None; MAX_MORPH_TARGETS],
            staged_seq: None,
            lands: [None; MAX_LANDS],
        }
    }
}

impl Morphs {
    /// Drops every morph and staged part. Recall morphs land; the land table is kept.
    pub fn clear(&mut self) {
        for i in 0..MAX_DRIVEN {
            if let Some(Pending {
                target,
                from: Some(_),
                to,
                ..
            }) = self.pending[i]
            {
                self.land(target, to);
            }
            if let Some(d) = self.driven[i].filter(|d| d.info.is_some()) {
                self.land(d.target, d.to);
            }
        }
        self.drop_staged();
        self.pending.fill(None);
        self.driven.fill(None);
    }

    /// A recall morph that will not run: `to` is written in this block and anything older on
    /// `target` stops.
    fn land(&mut self, target: RuntimeTarget, to: f32) {
        for p in self.pending.iter_mut() {
            if p.is_some_and(|p| p.target == target) {
                *p = None;
            }
        }
        self.release(target);
        let slot = self
            .lands
            .iter()
            .position(|l| l.is_some_and(|l| l.0 == target))
            .or_else(|| self.lands.iter().position(|l| l.is_none()));
        if let Some(i) = slot {
            self.lands[i] = Some((target, to));
        }
    }

    /// `f(target, value)` for every landed value; empties the land table.
    pub fn take_lands(&mut self, mut f: impl FnMut(RuntimeTarget, f32)) {
        for l in self.lands.iter_mut() {
            if let Some((t, v)) = l.take() {
                f(t, v);
            }
        }
    }

    fn drop_staged(&mut self) {
        for i in 0..MAX_MORPH_TARGETS {
            if let Some(t) = self.staged[i].take() {
                self.land(t.target, t.to);
            }
        }
        self.staged_seq = None;
    }

    /// Stages a recall part; the `last` one schedules every staged target to start in the next
    /// block. A new `seq` lands the unfinished set it replaces.
    pub fn part(&mut self, p: &MorphPart) {
        if self.staged_seq != Some(p.seq) {
            self.drop_staged();
            self.staged_seq = Some(p.seq);
        }
        for t in &p.targets[..(p.count as usize).min(PART_TARGETS)] {
            match self.staged.iter().position(|s| s.is_none()) {
                Some(i) => self.staged[i] = Some(*t),
                None => self.land(t.target, t.to),
            }
        }
        if p.last {
            self.staged_seq = None;
            for i in 0..MAX_MORPH_TARGETS {
                if let Some(t) = self.staged[i].take() {
                    self.schedule(Pending {
                        target: t.target,
                        from: Some(t.from),
                        to: t.to,
                        pulses: p.pulses,
                        clock: p.clock,
                        at: None,
                    });
                }
            }
        }
    }

    /// Queues `p`, replacing a pending morph of the same target. A full table drops it (a
    /// recall lands).
    pub fn schedule(&mut self, p: Pending) {
        let slot = self
            .pending
            .iter()
            .position(|q| q.is_some_and(|q| q.target == p.target))
            .or_else(|| self.pending.iter().position(|q| q.is_none()));
        match slot {
            Some(i) => self.pending[i] = Some(p),
            None if p.from.is_some() => self.land(p.target, p.to),
            None => {}
        }
    }

    /// Drops every pending morph; recalls land.
    pub fn cancel_pending(&mut self) {
        for i in 0..MAX_DRIVEN {
            if let Some(Pending {
                target,
                from: Some(_),
                to,
                ..
            }) = self.pending[i].take()
            {
                self.land(target, to);
            }
        }
    }

    /// A write to `t` from outside (hand, MIDI, host): its morph ends here.
    pub fn release(&mut self, t: RuntimeTarget) {
        for d in self.driven.iter_mut() {
            if d.is_some_and(|d| d.target == t) {
                *d = None;
            }
        }
    }

    /// Starts the morphs whose boundary has come and moves the running ones. `graph` is the
    /// one the clocks are read from. `out` gets `(target, value, ramp)` for every param to
    /// write into the graphs; returns how many.
    pub fn step(
        &mut self,
        graph: &CompiledPatch,
        out: &mut [(RuntimeTarget, f32, bool); MAX_DRIVEN],
    ) -> usize {
        for k in 0..MAX_DRIVEN {
            let Some(p) = self.pending[k] else { continue };
            let Some(clock) = graph.clock(p.clock) else {
                self.pending[k] = None;
                if p.from.is_some() {
                    self.land(p.target, p.to);
                }
                continue;
            };
            if !p.at.is_none_or(|at| reached(at, clock.next_tick())) {
                continue;
            }
            self.pending[k] = None;
            let existing = self
                .driven
                .iter()
                .position(|d| d.is_some_and(|d| d.target == p.target));
            let info = match p.from {
                Some(_) => match graph.runtime_info(p.target) {
                    Some(info) => Some(info),
                    None => {
                        self.land(p.target, p.to);
                        continue;
                    }
                },
                None => None,
            };
            let Some(from) = p.from.or_else(|| {
                existing
                    .and_then(|i| self.driven[i].map(|d| d.value))
                    .or_else(|| graph.runtime_value(p.target))
            }) else {
                continue;
            };
            let Some(i) = existing.or_else(|| self.driven.iter().position(|d| d.is_none())) else {
                if p.from.is_some() {
                    self.land(p.target, p.to);
                }
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
                target: p.target,
                info,
                value: if run.is_some() { from } else { p.to },
                to: p.to,
                run,
                arrived: false,
            });
        }
        let mut n = 0;
        for slot in self.driven.iter_mut() {
            let Some(d) = slot else { continue };
            if d.arrived {
                continue;
            }
            let running = d.run.is_some();
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
                            d.value = at(d.info, r.from, d.to, r.elapsed / r.pulses);
                        }
                    }
                    _ => {
                        d.value = d.to;
                        d.run = None;
                    }
                }
            }
            // A recall ramps to each block's value while it runs, and to `to` from where the
            // last ramp got; one that starts without a run jumps.
            out[n] = (d.target, d.value, d.info.is_some() && running);
            n += 1;
            d.arrived = d.info.is_some() && d.run.is_none();
        }
        n
    }

    /// `f(target, value, to, progress 0..1)` for each driven param.
    pub fn report(&self, mut f: impl FnMut(RuntimeTarget, f32, f32, f32)) {
        for d in self.driven.iter().flatten() {
            let progress = d.run.map_or(1.0, |r| r.elapsed / r.pulses);
            f(d.target, d.value, d.to, progress);
        }
    }
}

/// The value `x` (0..1) of the way from `from` to `to`: in knob travel for a recall, holding
/// `from` until half way for a stepped param, linear for a cue macro.
fn at(info: Option<&ParamInfo>, from: f32, to: f32, x: f32) -> f32 {
    match info {
        None => from + (to - from) * x,
        Some(p) if p.taper == Taper::Stepped => {
            if x >= 0.5 {
                to
            } else {
                from
            }
        }
        Some(p) => {
            let (a, b) = (p.to_norm(from), p.to_norm(to));
            p.from_norm(a + (b - a) * x)
        }
    }
}

/// `next_tick` counts pulses started, so pulse `at.1` has started once it is past it.
fn reached(at: (u32, u64), (epoch, tick): (u32, u64)) -> bool {
    epoch != at.0 || tick > at.1
}
