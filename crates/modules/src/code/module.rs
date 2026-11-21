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
//! - a normal block and an out-of-fuel block allocate nothing (the wasmi backend reads running
//!   out of fuel from a resumable call instead of building an error). A real trap makes wasmi
//!   build its error value: **one allocation, once**, because the module is disabled right
//!   after and runs nothing further. Debug builds mark the call ([`in_code_call`]) so a checking
//!   allocator can assert all of this.

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
    sanitized: AtomicU32,
    overruns: AtomicU32,
    /// Overruns already told to the user (see [`Health::take_overrun_news`]).
    overruns_told: AtomicU32,
    /// The module's per-block fuel range: at the slowest and the fastest calibrated rate.
    floor_fuel: AtomicU64,
    ceil_fuel: AtomicU64,
    /// Where in that range the module's measured rate puts it, in 1/65536 of the ceiling.
    trust: AtomicU32,
    /// Fuel and run time of the calls since the trust last moved (see `observe`).
    acc_fuel: AtomicU64,
    acc_ns: AtomicU64,
    voices: AtomicU32,
    /// Places this module has in running patches (its lane-0 instances); each asks for a share.
    placements: AtomicU32,
    wall_cap_ns: AtomicU64,
}

/// `Health::trust` at the ceiling.
const FULL_TRUST: u32 = 1 << 16;
/// Run time summed before the measured rate moves the budget: a few dozen typical voice calls.
const RATE_WINDOW: Duration = Duration::from_micros(100);

impl Health {
    pub fn new(fuel_budget: u64, wall_cap: Duration) -> Arc<Health> {
        Arc::new(Health {
            crashed: AtomicBool::new(false),
            fault: AtomicU8::new(0),
            notice: AtomicBool::new(false),
            nonfinite: AtomicU32::new(0),
            sanitized: AtomicU32::new(0),
            overruns: AtomicU32::new(0),
            overruns_told: AtomicU32::new(0),
            floor_fuel: AtomicU64::new(fuel_budget),
            ceil_fuel: AtomicU64::new(fuel_budget),
            trust: AtomicU32::new(FULL_TRUST),
            acc_fuel: AtomicU64::new(0),
            acc_ns: AtomicU64::new(0),
            voices: AtomicU32::new(0),
            placements: AtomicU32::new(0),
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
        self.overruns_told.store(0, Relaxed);
        self.crashed.store(false, Relaxed);
    }

    /// Blocks that produced NaN/Inf (and were therefore replaced by silence).
    pub fn nonfinite_count(&self) -> u32 {
        self.nonfinite.load(Relaxed)
    }

    /// Input samples that were NaN or infinite and were replaced by 0.0 before the module saw
    /// them (a counter, not a warning per sample). The module is never given non-finite input,
    /// so a non-finite output is its own fault.
    pub fn sanitized_inputs(&self) -> u32 {
        self.sanitized.load(Relaxed)
    }

    /// Overruns logged (fuel or wall clock), including one-off ones that did not crash anything.
    pub fn overrun_count(&self) -> u32 {
        self.overruns.load(Relaxed)
    }

    /// The overrun count when it has doubled since it was last told (or is first non-zero), so
    /// the user hears about a struggling module without a message per block.
    pub fn take_overrun_news(&self) -> Option<u32> {
        let n = self.overruns.load(Relaxed);
        let told = self.overruns_told.load(Relaxed);
        (n >= told.saturating_mul(2).max(1)).then(|| {
            self.overruns_told.store(n, Relaxed);
            n
        })
    }

    /// The module's per-block fuel now: its measured rate's place between floor and ceiling.
    pub fn fuel_budget(&self) -> u64 {
        let ceil = self.ceil_fuel.load(Relaxed);
        let fit = ((u128::from(ceil) * u128::from(self.trust.load(Relaxed))) >> 16) as u64;
        fit.max(self.floor_fuel.load(Relaxed))
    }

    /// The per-block fuel range set by the budget: (floor, ceiling).
    pub fn fuel_range(&self) -> (u64, u64) {
        (self.floor_fuel.load(Relaxed), self.ceil_fuel.load(Relaxed))
    }

    /// Feeds one call's cost back into the budget (audio thread; atomics only). Calls are
    /// summed until they add up to `RATE_WINDOW` of run time; the rate of the sum (total fuel
    /// over total time, so one jittery 5 µs call barely moves it), against the ceiling's rate,
    /// then moves the trust half the way down or a quarter of the way up.
    pub fn observe(&self, fuel: u64, took: Duration) {
        let ns = self.acc_ns.fetch_add(took.as_nanos() as u64, Relaxed) + took.as_nanos() as u64;
        let fuel = self.acc_fuel.fetch_add(fuel, Relaxed) + fuel;
        if ns < RATE_WINDOW.as_nanos() as u64 {
            return;
        }
        self.acc_ns.store(0, Relaxed);
        self.acc_fuel.store(0, Relaxed);
        let ceil = self.ceil_fuel.load(Relaxed);
        let cap = self.wall_cap_ns.load(Relaxed);
        if ceil == 0 || cap == 0 {
            return;
        }
        // The rate the window ran at, relative to the ceiling's rate (ceil fuel per cap).
        let sample = (u128::from(fuel) * u128::from(cap) * u128::from(FULL_TRUST)
            / (u128::from(ns) * u128::from(ceil)))
        .min(u128::from(FULL_TRUST)) as i64;
        let old = i64::from(self.trust.load(Relaxed));
        let step = if sample < old { 2 } else { 4 };
        let delta = match (sample - old) / step {
            0 => sample - old, // close enough: settle on it
            d => d,
        };
        self.trust.store((old + delta) as u32, Relaxed);
    }

    pub fn wall_cap(&self) -> Duration {
        Duration::from_nanos(self.wall_cap_ns.load(Relaxed))
    }

    /// Voice instances of this module that exist right now.
    pub fn voices(&self) -> u32 {
        self.voices.load(Relaxed)
    }

    /// Places this module has in running patches: the same code placed six times asks for six
    /// shares (within the 50 % total), not one share split six ways.
    pub fn placements(&self) -> u32 {
        self.placements.load(Relaxed)
    }

    /// What one voice's call may spend: the module's budget split across its voices, so that all
    /// voices together stay within the module's 25 % (and, via `Budget`, the global 50 %).
    pub fn voice_fuel(&self) -> u64 {
        (self.fuel_budget() / u64::from(self.voices().max(1))).max(1)
    }

    /// The wall-clock backstop for one voice's call: the module's whole share, not a slice of
    /// it. The measured rate already keeps the voices together within the share; the backstop
    /// only catches a call that alone takes all of it. A per-voice slice (10 µs for 32 voices)
    /// is within normal jitter of a 5 µs call, and eight jittery calls in a row crashed modules
    /// that fit easily (REAPER, 4 filters at 8 voices, while the host started up).
    pub fn voice_wall_cap(&self) -> Duration {
        self.wall_cap()
    }

    /// A fixed budget (floor = ceiling).
    pub fn set_budget(&self, fuel: u64, wall_cap: Duration) {
        self.set_budget_range(fuel, fuel, wall_cap);
    }

    /// The fuel range for the module's share (`wall_cap`), from the slowest and the fastest
    /// calibrated rate. The measured trust is kept: it is a rate, not an amount.
    pub fn set_budget_range(&self, floor: u64, ceil: u64, wall_cap: Duration) {
        self.floor_fuel.store(floor, Relaxed);
        self.ceil_fuel.store(ceil.max(floor), Relaxed);
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
    /// This instance stands for one place of the module in a patch (see `count_as_placement`).
    placed: bool,
    /// Where the song's settings for this instance are handed over (see `state::StateCell`).
    cell: Option<Arc<super::state::StateCell>>,
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
        if health.voices.fetch_add(1, Relaxed) == 0 {
            super::host::budget().rebalance();
        }
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
            placed: false,
            cell: None,
        }
    }

    /// Marks this instance as the one that stands for a place of the module in a patch (the
    /// compiler calls it on lane 0, channel 0), so the budget gives the module a share per
    /// place. Control thread.
    pub fn count_as_placement(&mut self) {
        if !self.placed {
            self.placed = true;
            self.health.placements.fetch_add(1, Relaxed);
            super::host::budget().rebalance();
        }
    }

    /// Size of the module's settings transfer buffer, if it has saved settings.
    pub fn state_cap(&self) -> Option<usize> {
        self.inst.state_xfer().map(|x| x.cap)
    }

    /// Serves the song's save and load requests for this instance from the next block on.
    pub fn set_state_cell(&mut self, cell: Arc<super::state::StateCell>) {
        self.cell = Some(cell);
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

thread_local! {
    static IN_CODE_CALL: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// True while this thread is inside a code module's audio-thread call (debug builds only; always
/// false in release). A debug binary's allocator checks it to catch allocation on the audio
/// thread while custom modules run; `tests/code_module.rs` installs such an allocator. No
/// dependency: the usual checker crate's licence is not on the allow list for the shipped tree.
pub fn in_code_call() -> bool {
    cfg!(debug_assertions) && IN_CODE_CALL.with(|c| c.get() > 0)
}

fn no_alloc<T>(f: impl FnOnce() -> T) -> T {
    if cfg!(debug_assertions) {
        IN_CODE_CALL.with(|c| c.set(c.get() + 1));
        let r = f();
        IN_CODE_CALL.with(|c| c.set(c.get() - 1));
        r
    } else {
        f()
    }
}

fn fade_len(sample_rate: f32) -> usize {
    ((sample_rate * FADE_MS / 1000.0) as usize).max(1)
}

/// Writes input samples, replacing NaN and infinity with 0.0; returns how many were replaced.
fn put_inputs(mem: &mut [u8], at: usize, vals: impl Iterator<Item = f32>) -> u32 {
    let mut bad = 0;
    for (k, v) in vals.enumerate() {
        let v = if v.is_finite() {
            v
        } else {
            bad += 1;
            0.0
        };
        mem[at + k * 4..at + k * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    bad
}

fn put_f32s(mem: &mut [u8], at: usize, vals: impl Iterator<Item = f32>) {
    for (k, v) in vals.enumerate() {
        mem[at + k * 4..at + k * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
}

impl Drop for CodeModule {
    fn drop(&mut self) {
        let last = self.health.voices.fetch_sub(1, Relaxed) == 1;
        if self.placed {
            self.health.placements.fetch_sub(1, Relaxed);
        }
        if last || self.placed {
            super::host::budget().rebalance();
        }
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
        if let Some(cell) = &self.cell {
            let fuel = self.health.voice_fuel();
            let inst = &mut *self.inst;
            if let Err(fault) = no_alloc(|| cell.service(inst, fuel)) {
                return self.die(fault, io, n);
            }
        }
        let layout = self.layout;
        let mem = self.inst.memory();
        for s in 0..self.in_slots {
            let at = layout.input_range(s).start;
            let bad = match io.input(s) {
                Signal::Scalar(v) => put_inputs(mem, at, std::iter::repeat_n(v, n)),
                Signal::Buffer(b) => put_inputs(mem, at, b[..n].iter().copied()),
            };
            if bad > 0 {
                self.health.sanitized.fetch_add(bad, Relaxed);
            }
        }
        for p in 0..self.n_params.min(io.param_count()) {
            let v = match io.param(p) {
                Signal::Scalar(v) => v,
                Signal::Buffer(b) => b[0],
            };
            put_f32s(mem, layout.param_range(p).start, std::iter::once(v));
        }

        let budget = self.health.voice_fuel();
        let cap = self.health.voice_wall_cap();
        self.inst.set_fuel(budget);
        let started = Instant::now();
        let inst = &mut self.inst;
        let result = no_alloc(|| inst.call_block(n as u32));
        let took = started.elapsed();
        match result {
            Ok(used) => self.health.observe(used, took),
            Err(Fault::OutOfFuel) => self.health.observe(budget, took),
            Err(_) => {}
        }

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

    /// Resets the module's DSP state (voice reuse). Does not revive a crashed module. The module's
    /// `reset()` runs under the same rules as a block: the voice's share of the fuel budget, the
    /// wall-clock backstop, no allocation, and the same overrun/crash outcome (an overrun is
    /// counted, eight in a row or a trap disables the module).
    fn reset(&mut self) {
        if self.health.is_crashed() {
            return;
        }
        let cap = self.health.voice_wall_cap();
        self.inst.set_fuel(self.health.voice_fuel());
        let started = Instant::now();
        let inst = &mut self.inst;
        let result = no_alloc(|| inst.reset());
        let took = started.elapsed();
        match result {
            Err(Fault::OutOfFuel) => {
                if self.overrun() {
                    self.health.crash(Fault::Overrun);
                }
            }
            Err(fault) => self.health.crash(fault),
            Ok(()) if took > cap => {
                if self.overrun() {
                    self.health.crash(Fault::Overrun);
                }
            }
            Ok(()) => {}
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
