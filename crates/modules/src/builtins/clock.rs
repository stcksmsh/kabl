//! `clock`: the shared tempo source. One global instance drives any number of sequencers, so
//! every pattern patched to it stays in step. Emits a 50 % duty gate, one pulse per 16th note
//! at `bpm` (four pulses per beat, the usual step length of a step sequencer).
//!
//! Transport (`Clock::command`) is a runtime command from the UI, not a param: it is never in
//! the op log, so undo, reload and graph swaps can't replay it. Stop holds `gate` low. Run starts
//! a fresh pulse at once (the next step). Restart arms `reset`: it goes high with the next pulse
//! that a (re)start begins and lasts that pulse, so a sequencer or divider patched to both
//! outputs lands on step 1 exactly on that tick. Running, that pulse starts now (after one low
//! sample if `gate` was high, so the edge is clean); stopped, it waits for Run.

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "gate",
        port_type: PortType::Gate,
        direction: PortDirection::Output,
    },
    PortInfo {
        name: "reset",
        port_type: PortType::Gate,
        direction: PortDirection::Output,
    },
];

const PARAMS: &[ParamInfo] = &[ParamInfo {
    name: "bpm",
    min: 20.0,
    max: 300.0,
    default: 120.0,
    unit: "bpm",
    taper: Taper::Linear,
    smoothing_ms: 0.0,
}];

pub static CLOCK_INFO: ModuleInfo = ModuleInfo {
    kind: "clock",
    name: "Clock",
    category: Category::Sequencer,
    rate: Rate::Global,
    explain: "Ticks four times per beat at the tempo you set; sequencers step on each tick. Restart sends a pulse on reset.",
    lesson: None,
    requires: &[],
    ports: PORTS,
    params: PARAMS,
    quality: QualitySupport {
        oversampling: false,
        anti_aliasing: false,
        interpolation: false,
    },
    skin: None,
    width_units: 5,
    advanced: &[],
};

const PULSES_PER_BEAT: f64 = 4.0;
/// Rounding slack on the phase thresholds, so a pulse whose length is a whole number of samples
/// (6000 at 120 bpm, 48 kHz) comes out exactly that long.
const EPS: f64 = 1e-9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    Run,
    Stop,
    Restart,
    /// Run when stopped, Stop when running: decided where the clock runs (a MIDI button needs
    /// no report of the run state).
    Toggle,
}

/// Pulses a block can start: one at most below ~750 bpm-equivalent rates, room to spare.
const MAX_BLOCK_TICKS: usize = 4;

#[derive(Clone, Copy)]
pub struct Clock {
    /// f64: in f32 the phase sum drifts by about one sample per pulse at 48 kHz.
    phase: f64,
    sample_rate: f32,
    running: bool,
    /// Restart pending: `reset` rises with the next pulse a (re)start begins.
    armed: bool,
    /// Emit one low sample before the restarted pulse (gate was high).
    gap: bool,
    reset_high: bool,
    gate_high: bool,
    /// Pulses started since the last Restart pulse (or the load); a Restart pulse starts a new
    /// epoch at tick 0. Launch boundaries are counted in these.
    epoch: u32,
    ticks: u64,
    /// `(epoch, tick, sample offset)` of each pulse started in the last block.
    block_ticks: [(u32, u64, u32); MAX_BLOCK_TICKS],
    n_block_ticks: usize,
}

impl Clock {
    pub fn new() -> Self {
        Clock {
            phase: 0.0,
            sample_rate: 48000.0,
            running: true,
            armed: false,
            gap: false,
            reset_high: false,
            gate_high: false,
            epoch: 0,
            ticks: 0,
            block_ticks: [(0, 0, 0); MAX_BLOCK_TICKS],
            n_block_ticks: 0,
        }
    }

    pub fn running(&self) -> bool {
        self.running
    }

    /// `(epoch, tick)` of the next pulse to start.
    pub fn next_tick(&self) -> (u32, u64) {
        (self.epoch, self.ticks)
    }

    /// `(epoch, tick, sample offset)` of every pulse the last processed block started.
    pub fn block_ticks(&self) -> &[(u32, u64, u32)] {
        &self.block_ticks[..self.n_block_ticks]
    }

    /// Audio-thread call. No allocation.
    pub fn command(&mut self, t: Transport) {
        match t {
            Transport::Stop => self.running = false,
            Transport::Run if !self.running => {
                self.running = true;
                self.phase = 0.0;
            }
            Transport::Run => {}
            Transport::Toggle => self.command(if self.running {
                Transport::Stop
            } else {
                Transport::Run
            }),
            Transport::Restart => {
                self.armed = true;
                if self.running {
                    self.gap = self.gate_high;
                    self.phase = 0.0;
                }
            }
        }
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Clock {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &CLOCK_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let inc = io.param(0).at(0) as f64 / 60.0 * PULSES_PER_BEAT / self.sample_rate as f64;
        let n = io.block_len();
        // Yields (gate, reset) per sample from the stored state. Run once per output:
        // `ProcessIo` lends one output buffer at a time.
        let start = *self;
        let walk = move || {
            (0..n).scan(start, move |st, _| {
                let started = if !st.running || st.gap {
                    (st.gap, st.reset_high, st.gate_high) = (false, false, false);
                    false
                } else {
                    let g = st.phase < 0.5 - EPS;
                    let started = g && !st.gate_high;
                    if started && st.armed {
                        (st.reset_high, st.armed) = (true, false);
                        (st.epoch, st.ticks) = (st.epoch.wrapping_add(1), 0);
                    }
                    if started {
                        st.ticks += 1;
                    }
                    st.reset_high &= g;
                    st.gate_high = g;
                    st.phase += inc;
                    if st.phase >= 1.0 - EPS {
                        st.phase -= 1.0;
                    }
                    started
                };
                Some((*st, started))
            })
        };
        for (o, (st, _)) in io.output(0)[..n].iter_mut().zip(walk()) {
            *o = st.gate_high as u8 as f32;
        }
        let mut ticks = [(0, 0, 0); MAX_BLOCK_TICKS];
        let mut count = 0;
        for (i, (o, (st, started))) in io.output(1)[..n].iter_mut().zip(walk()).enumerate() {
            *o = st.reset_high as u8 as f32;
            if started && count < MAX_BLOCK_TICKS {
                ticks[count] = (st.epoch, st.ticks - 1, i as u32);
                count += 1;
            }
            *self = st;
        }
        (self.block_ticks, self.n_block_ticks) = (ticks, count);
    }

    fn reset(&mut self) {
        *self = Clock {
            sample_rate: self.sample_rate,
            ..Clock::new()
        };
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        // Split in two so a graph swap carries the f64 phase exactly.
        let hi = self.phase as f32;
        out.write_f32("phase", hi);
        out.write_f32("phase_lo", (self.phase - hi as f64) as f32);
        for (k, v) in [
            ("running", self.running),
            ("armed", self.armed),
            ("gap", self.gap),
            ("reset_high", self.reset_high),
            ("gate_high", self.gate_high),
        ] {
            out.write_f32(k, v as u8 as f32);
        }
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        if let Some(p) = s.read_f32("phase") {
            self.phase = p as f64 + s.read_f32("phase_lo").unwrap_or(0.0) as f64;
        }
        for (k, v) in [
            ("running", &mut self.running),
            ("armed", &mut self.armed),
            ("gap", &mut self.gap),
            ("reset_high", &mut self.reset_high),
            ("gate_high", &mut self.gate_high),
        ] {
            if let Some(x) = s.read_f32(k) {
                *v = x >= 0.5;
            }
        }
    }

    /// Tick count and epoch ride along exactly (`load_state` has the rest).
    fn carry_from(&mut self, old: &dyn Module) {
        if let Some(o) = old.as_any().downcast_ref::<Clock>() {
            (self.epoch, self.ticks) = (o.epoch, o.ticks);
        }
    }
}
