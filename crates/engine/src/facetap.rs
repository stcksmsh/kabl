//! Face taps: what the live displays on module faces need from the audio thread.
//!
//! The selected-signal tap (`probe.rs`) measures one output at a time. A face display needs
//! several at once: each visible oscillator's cycle, each envelope's level and stage, each
//! source that moves a knob. The UI names up to `MAX_FACE_TAPS` outputs; the engine keeps, per
//! output, the last, lowest and highest sample of a short window from the instance that was
//! loudest in it, and, where asked, that instance's `Module::view` (one cycle, a table position,
//! a stage). Everything is fixed-size and `Copy`: the audio thread allocates, formats and waits
//! for nothing, and a graph carries one slot byte per instance, filled when it is compiled.

use kabl_core::ModuleId;
use kabl_modules::{ModuleView, VIEW_CYCLE};

use crate::graph::BLOCK;

/// Most outputs one set of targets names.
pub const MAX_FACE_TAPS: usize = 16;
/// Points of a reported cycle.
pub const FACE_CYCLE: usize = VIEW_CYCLE;
/// Instances with no tap carry this slot.
pub(crate) const NO_SLOT: u8 = u8::MAX;

/// One output a face display reads. `kind` guards against another module reusing the id.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceTarget {
    pub module: ModuleId,
    /// Index among the module's output ports.
    pub port: u8,
    pub kind: &'static str,
    /// Also report the module's `view`.
    pub view: bool,
}

/// The outputs to read (`t[..n]`). `token` changes with every new set, so older reports are
/// recognisable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceTargets {
    pub token: u64,
    pub n: u8,
    pub t: [FaceTarget; MAX_FACE_TAPS],
}

impl FaceTargets {
    pub fn new(token: u64, targets: &[FaceTarget]) -> Self {
        let n = targets.len().min(MAX_FACE_TAPS);
        let mut t = [FaceTarget {
            module: 0,
            port: 0,
            kind: "",
            view: false,
        }; MAX_FACE_TAPS];
        t[..n].copy_from_slice(&targets[..n]);
        FaceTargets {
            token,
            n: n as u8,
            t,
        }
    }
}

/// What one tap saw in a window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceEntry {
    pub module: ModuleId,
    pub port: u8,
    /// The target exists in the measured graph.
    pub found: bool,
    /// The newest sample of the loudest instance, and the extremes over all instances.
    pub last: f32,
    pub min: f32,
    pub max: f32,
    /// `view` was asked for and the module had something to show.
    pub view_valid: bool,
    /// `ModuleView::position` (table position, LFO phase, envelope stage).
    pub position: f32,
    /// `ModuleView::cycle`, as signed bytes.
    pub cycle: [i8; FACE_CYCLE],
}

impl FaceEntry {
    const EMPTY: FaceEntry = FaceEntry {
        module: 0,
        port: 0,
        found: false,
        last: 0.0,
        min: 0.0,
        max: 0.0,
        view_valid: false,
        position: 0.0,
        cycle: [0; FACE_CYCLE],
    };

    /// The cycle as floats in -1..1.
    pub fn cycle_f32(&self) -> [f32; FACE_CYCLE] {
        std::array::from_fn(|i| f32::from(self.cycle[i]) / 127.0)
    }
}

/// One window of face measurements.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceReport {
    pub token: u64,
    /// `CompiledPatch::generation` of the graph measured.
    pub generation: u64,
    /// Samples the engine had rendered when this window ended.
    pub end_sample: u64,
    pub n: u8,
    pub entries: [FaceEntry; MAX_FACE_TAPS],
}

/// The taps inside one graph and what they have gathered.
#[derive(Clone, Debug)]
pub(crate) struct FaceTaps {
    pub targets: Option<FaceTargets>,
    /// Per instance: the target slot reading it.
    slot: Box<[u8]>,
    port: [u8; MAX_FACE_TAPS],
    found: [bool; MAX_FACE_TAPS],
    best_mod: [u32; MAX_FACE_TAPS],
    best_peak: [f32; MAX_FACE_TAPS],
    last: [f32; MAX_FACE_TAPS],
    min: [f32; MAX_FACE_TAPS],
    max: [f32; MAX_FACE_TAPS],
    pub blocks: u32,
    pub fading: bool,
}

impl FaceTaps {
    /// Allocates the slot table for a graph of `instances` modules (at compile time, off the
    /// audio thread).
    pub fn new(instances: usize) -> Self {
        FaceTaps {
            targets: None,
            slot: vec![NO_SLOT; instances].into_boxed_slice(),
            port: [0; MAX_FACE_TAPS],
            found: [false; MAX_FACE_TAPS],
            best_mod: [u32::MAX; MAX_FACE_TAPS],
            best_peak: [-1.0; MAX_FACE_TAPS],
            last: [0.0; MAX_FACE_TAPS],
            min: [f32::INFINITY; MAX_FACE_TAPS],
            max: [f32::NEG_INFINITY; MAX_FACE_TAPS],
            blocks: 0,
            fading: false,
        }
    }

    #[inline]
    pub fn active(&self) -> bool {
        self.targets.is_some()
    }

    pub fn reset_window(&mut self) {
        self.best_mod = [u32::MAX; MAX_FACE_TAPS];
        self.best_peak = [-1.0; MAX_FACE_TAPS];
        self.last = [0.0; MAX_FACE_TAPS];
        self.min = [f32::INFINITY; MAX_FACE_TAPS];
        self.max = [f32::NEG_INFINITY; MAX_FACE_TAPS];
        self.blocks = 0;
        self.fading = false;
    }

    /// Points the taps at `targets` (`None`: off) and starts a new window. `instances` yields
    /// each instance's (module id, kind, output count). Linear in the instances, no allocation.
    pub fn set(
        &mut self,
        targets: Option<FaceTargets>,
        instances: impl Iterator<Item = (ModuleId, &'static str, usize)>,
    ) {
        self.slot.fill(NO_SLOT);
        self.found = [false; MAX_FACE_TAPS];
        self.targets = targets;
        self.reset_window();
        let Some(t) = targets else { return };
        for (i, (id, kind, outputs)) in instances.enumerate() {
            if let Some(s) = t.t[..t.n as usize]
                .iter()
                .position(|x| x.module == id && x.kind == kind && (x.port as usize) < outputs)
            {
                self.slot[i] = s as u8;
                self.port[s] = t.t[s].port;
                self.found[s] = true;
            }
        }
    }

    /// Slot reading instance `index`, or `NO_SLOT`.
    #[inline]
    pub fn slot_of(&self, index: usize) -> u8 {
        self.slot.get(index).copied().unwrap_or(NO_SLOT)
    }

    #[inline]
    pub fn port_of(&self, slot: u8) -> usize {
        self.port[slot as usize] as usize
    }

    /// Adds one block of instance `index`'s output to slot `slot`.
    #[inline]
    pub fn feed(&mut self, slot: u8, index: usize, buf: &[f32; BLOCK]) {
        let s = slot as usize;
        let (mut lo, mut hi, mut peak) = (self.min[s], self.max[s], 0.0f32);
        let mut last = 0.0;
        for &x in buf {
            if !x.is_finite() {
                continue;
            }
            lo = lo.min(x);
            hi = hi.max(x);
            peak = peak.max(x.abs());
            last = x;
        }
        self.min[s] = lo;
        self.max[s] = hi;
        // The instance that peaked highest this window owns `last` (a held voice, not a
        // released one).
        if peak >= self.best_peak[s] {
            self.best_peak[s] = peak;
            self.best_mod[s] = index as u32;
            self.last[s] = last;
        }
    }

    /// Keeps `old`'s open window when only values changed (same targets, same instances), so a
    /// run of swaps (a knob being turned rebuilds the graph every frame) never restarts it.
    pub fn continue_from(&mut self, old: &FaceTaps, same_topology: bool) {
        if same_topology && old.targets == self.targets && old.slot == self.slot {
            self.best_mod = old.best_mod;
            self.best_peak = old.best_peak;
            self.last = old.last;
            self.min = old.min;
            self.max = old.max;
            self.blocks = old.blocks;
            self.fading = true;
        }
    }

    /// Counts a block; when the window is full, builds its report (`view_of` fills a module's
    /// view) and starts the next.
    pub fn block_end(
        &mut self,
        window_blocks: u32,
        fading: bool,
        generation: u64,
        mut view_of: impl FnMut(usize, &mut ModuleView),
    ) -> Option<FaceReport> {
        let t = self.targets?;
        self.blocks += 1;
        self.fading |= fading;
        if self.blocks < window_blocks.max(1) {
            return None;
        }
        let mut entries = [FaceEntry::EMPTY; MAX_FACE_TAPS];
        for (s, e) in entries.iter_mut().enumerate().take(t.n as usize) {
            let tg = t.t[s];
            let seen = self.found[s] && self.best_mod[s] != u32::MAX;
            *e = FaceEntry {
                module: tg.module,
                port: tg.port,
                found: self.found[s],
                last: if seen { self.last[s] } else { 0.0 },
                min: if seen { self.min[s] } else { 0.0 },
                max: if seen { self.max[s] } else { 0.0 },
                ..FaceEntry::EMPTY
            };
            if seen && tg.view {
                let mut v = ModuleView::default();
                view_of(self.best_mod[s] as usize, &mut v);
                if v.valid {
                    e.view_valid = true;
                    e.position = v.position;
                    for (o, &c) in e.cycle.iter_mut().zip(v.cycle.iter()) {
                        *o = (c.clamp(-1.0, 1.0) * 127.0).round() as i8;
                    }
                }
            }
        }
        let report = FaceReport {
            token: t.token,
            generation,
            end_sample: 0,
            n: t.n,
            entries,
        };
        self.reset_window();
        Some(report)
    }
}
