//! `CodeModule`: a `Module` whose DSP is a sandboxed instance behind `WasmInstance`.
//!
//! Failure rules (D11 decisions 4, 5, 9):
//! - a trap, memory over cap, or NaN/Inf in any output buffer is a **crash**: this module's
//!   outputs fade to zero over ~5 ms and then stay silent, in **every voice** (the voices of one
//!   module share a [`Health`]), until the user resets or reloads it ([`Health::clear`]). No
//!   automatic retry. `Module::reset` (voice steal, note off) never revives a crashed module.
//! - running out of fuel, or a block taking longer than the wall-clock cap (the backstop), is an
//!   **overrun**: that block is silent (same short fade), and eight in a row make a crash. A
//!   one-off overrun is only counted.
//! - nothing allocates while a block runs; a debug build asserts it (`assert_no_alloc` is active
//!   when the binary installs its `AllocDisabler`).

use super::abi::Layout;
use super::engine::{Fault, InstanceConfig, WasmInstance};
use crate::info::{ModuleInfo, PortDirection};
use crate::io::{ProcessIo, Signal};
use crate::module::{Module, QualityConfig};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering::Relaxed};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const FADE_MS: f32 = 5.0;
pub const OVERRUN_LIMIT: u32 = 8;

/// State shared by every voice of one code module, and read by the UI (badge, toast).
pub struct Health {
    crashed: AtomicBool,
    fault: AtomicU8,
    notice: AtomicBool,
    nonfinite: AtomicU32,
    overruns: AtomicU32,
    fuel_budget: AtomicU64,
    wall_cap_ns: AtomicU64,
}

impl Health {
    pub fn new(fuel_budget: u64, wall_cap: Duration) -> Arc<Health> {
        Arc::new(Health {
            crashed: AtomicBool::new(false),
            fault: AtomicU8::new(0),
            notice: AtomicBool::new(false),
            nonfinite: AtomicU32::new(0),
            overruns: AtomicU32::new(0),
            fuel_budget: AtomicU64::new(fuel_budget),
            wall_cap_ns: AtomicU64::new(wall_cap.as_nanos() as u64),
        })
    }

    /// Marks the module crashed (all voices). Only the first fault is kept and raises the notice.
    pub fn crash(&self, fault: Fault) {
        if !self.crashed.swap(true, Relaxed) {
            self.fault.store(fault.code(), Relaxed);
            self.notice.store(true, Relaxed);
        }
    }

    pub fn is_crashed(&self) -> bool {
        self.crashed.load(Relaxed)
    }

    pub fn fault(&self) -> Option<Fault> {
        Fault::from_code(self.fault.load(Relaxed))
    }

    /// True once per crash, for the toast.
    pub fn take_notice(&self) -> bool {
        self.notice.swap(false, Relaxed)
    }

    /// The user's reset/reload: brings the module back. The only way out of a crash.
    pub fn clear(&self) {
        self.fault.store(0, Relaxed);
        self.nonfinite.store(0, Relaxed);
        self.overruns.store(0, Relaxed);
        self.crashed.store(false, Relaxed);
    }

    /// Blocks that produced NaN/Inf (and were therefore replaced by silence).
    pub fn nonfinite_count(&self) -> u32 {
        self.nonfinite.load(Relaxed)
    }

    /// Overruns logged (fuel or wall clock), including one-off ones that did not crash anything.
    pub fn overrun_count(&self) -> u32 {
        self.overruns.load(Relaxed)
    }

    /// Per-block fuel and per-call wall-clock cap, rescaled by the controller (calibration and
    /// the 50 % total cap come in a later PR).
    pub fn set_budget(&self, fuel: u64, wall_cap: Duration) {
        self.fuel_budget.store(fuel, Relaxed);
        self.wall_cap_ns.store(wall_cap.as_nanos() as u64, Relaxed);
    }
}

pub struct CodeModule {
    info: &'static ModuleInfo,
    inst: Box<dyn WasmInstance>,
    health: Arc<Health>,
    layout: Layout,
    in_slots: usize,
    n_params: usize,
    /// Last valid output sample per output slot: where a fade starts.
    last: Vec<f32>,
    fade_from: Vec<f32>,
    fade_total: usize,
    fade_left: usize,
    consecutive_overruns: u32,
    fading_dead: bool,
}

impl CodeModule {
    pub fn new(
        info: &'static ModuleInfo,
        inst: Box<dyn WasmInstance>,
        health: Arc<Health>,
        cfg: &InstanceConfig,
    ) -> CodeModule {
        let out_slots = info.slots(PortDirection::Output);
        let layout = inst.layout();
        CodeModule {
            info,
            inst,
            health,
            layout,
            in_slots: info.slots(PortDirection::Input),
            n_params: info.params.len(),
            last: vec![0.0; out_slots],
            fade_from: vec![0.0; out_slots],
            fade_total: fade_len(cfg.sample_rate),
            fade_left: 0,
            consecutive_overruns: 0,
            fading_dead: false,
        }
    }

    pub fn health(&self) -> &Arc<Health> {
        &self.health
    }

    fn begin_fade(&mut self) {
        self.fade_from.copy_from_slice(&self.last);
        self.fade_left = self.fade_total;
    }

    /// Writes the fade (then silence) into every output for `n` frames.
    fn emit_fade(&mut self, io: &mut ProcessIo, n: usize) {
        let start = self.fade_left;
        for (s, from) in self.fade_from.iter().enumerate() {
            let out = io.output(s);
            for (i, o) in out[..n].iter_mut().enumerate() {
                *o = if i < start {
                    from * (start - i) as f32 / self.fade_total as f32
                } else {
                    0.0
                };
            }
        }
        self.fade_left = start.saturating_sub(n);
        // After a fade the next block starts from silence.
        self.last.iter_mut().for_each(|l| *l = 0.0);
        if self.fade_left == 0 {
            self.fade_from.iter_mut().for_each(|f| *f = 0.0);
        }
    }

    fn die(&mut self, fault: Fault, io: &mut ProcessIo, n: usize) {
        self.health.crash(fault);
        self.fading_dead = true;
        self.begin_fade();
        self.emit_fade(io, n);
    }

    fn overrun(&mut self) -> bool {
        self.health.overruns.fetch_add(1, Relaxed);
        self.consecutive_overruns += 1;
        self.consecutive_overruns >= OVERRUN_LIMIT
    }
}

fn fade_len(sample_rate: f32) -> usize {
    ((sample_rate * FADE_MS / 1000.0) as usize).max(1)
}

fn put_f32s(mem: &mut [u8], at: usize, vals: impl Iterator<Item = f32>) {
    for (k, v) in vals.enumerate() {
        mem[at + k * 4..at + k * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
}

impl Module for CodeModule {
    fn info(&self) -> &'static ModuleInfo {
        self.info
    }

    fn prepare(&mut self, sample_rate: f32, max_block: usize, _quality: &QualityConfig) {
        self.fade_total = fade_len(sample_rate);
        if max_block > self.layout.max_block as usize {
            self.health.crash(Fault::BadLayout);
        }
    }

    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        if self.health.is_crashed() {
            if !self.fading_dead {
                self.fading_dead = true;
                self.begin_fade();
            }
            self.emit_fade(io, n);
            return;
        }
        if n > self.layout.max_block as usize {
            return self.die(Fault::BadLayout, io, n);
        }
        let layout = self.layout;
        let mem = self.inst.memory();
        for s in 0..self.in_slots {
            let at = layout.input_range(s).start;
            match io.input(s) {
                Signal::Scalar(v) => put_f32s(mem, at, std::iter::repeat_n(v, n)),
                Signal::Buffer(b) => put_f32s(mem, at, b[..n].iter().copied()),
            }
        }
        for p in 0..self.n_params.min(io.param_count()) {
            let v = match io.param(p) {
                Signal::Scalar(v) => v,
                Signal::Buffer(b) => b[0],
            };
            put_f32s(mem, layout.param_range(p).start, std::iter::once(v));
        }

        let budget = self.health.fuel_budget.load(Relaxed);
        let cap = Duration::from_nanos(self.health.wall_cap_ns.load(Relaxed));
        self.inst.set_fuel(budget);
        let started = Instant::now();
        let inst = &mut self.inst;
        let result = assert_no_alloc::assert_no_alloc(|| inst.call_block(n as u32));
        let took = started.elapsed();

        match result {
            Err(Fault::OutOfFuel) => {
                if self.overrun() {
                    return self.die(Fault::Overrun, io, n);
                }
                self.begin_fade();
                return self.emit_fade(io, n);
            }
            Err(fault) => return self.die(fault, io, n),
            Ok(_) => {}
        }
        if took > cap {
            // The wall-clock backstop: the output is valid, but the module is too slow.
            if self.overrun() {
                return self.die(Fault::Overrun, io, n);
            }
        } else {
            self.consecutive_overruns = 0;
        }

        let mem = self.inst.memory();
        let mut finite = true;
        for s in 0..self.last.len() {
            let r = layout.output_range(s);
            let out = io.output(s);
            for (i, o) in out[..n].iter_mut().enumerate() {
                let a = r.start + i * 4;
                let v = f32::from_le_bytes([mem[a], mem[a + 1], mem[a + 2], mem[a + 3]]);
                finite &= v.is_finite();
                *o = v;
            }
        }
        if !finite {
            self.health.nonfinite.fetch_add(1, Relaxed);
            return self.die(Fault::NonFinite, io, n);
        }
        self.fade_left = 0;
        for (s, l) in self.last.iter_mut().enumerate() {
            *l = io.output(s)[n - 1];
        }
    }

    /// Resets the module's DSP state (voice reuse). Does not revive a crashed module.
    fn reset(&mut self) {
        self.consecutive_overruns = 0;
        if !self.health.is_crashed() {
            if let Err(f) = self.inst.reset() {
                self.health.crash(f);
            }
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
