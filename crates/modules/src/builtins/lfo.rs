//! `lfo` (brief section 8: "sine/tri/saw/square/S&H, rate in Hz"). Wraps `dsp::FullLfo`.
//!
//! Waveform selection is a discrete choice, not a continuous value, but `ParamInfo` (brief
//! section 8) only models float ranges — no enum-param variant exists. Represented as a stepped
//! float param (`0..=4`, rounded in `process`), the standard modular-software convention for a
//! "pick one of N" knob. Documented here since it's not obvious from `ParamInfo`'s shape alone.
//!
//! Clock sync (`sync` other than FREE): one cycle lasts `SYNC_TICKS` clock pulses (16ths).
//! - The pulse period comes from the interval between rising `clock` edges, taken once it
//!   agrees within 10 % with the one before it (the delay's rule): the long interval across a
//!   Stop or the short one of a Restart never counts as tempo. Until one is accepted
//!   (acquiring) the LFO runs at `rate_hz`.
//! - Locked, each edge also says where the phase should be: pulse count / cycle length plus
//!   `phase`. The error is spread over the next pulse, at most half the synced rate faster or
//!   slower, so acquisition glides instead of jumping (a big error takes up to a cycle).
//! - The clock stopping keeps the last accepted rate: the modulation goes on.
//! - `reset` (only the input: a transport Restart does nothing unless patched) jumps to
//!   `phase` and restarts the pulse count: at once if the clock is high (the Restart pulse),
//!   else on the next clock edge. Without sync it jumps at once.
//!
//! `status` reports free / acquiring / synced / held for the UI.

use crate::dsp::{FullLfo, LfoWaveform};
use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};

const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "clock",
        port_type: PortType::Gate,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "reset",
        port_type: PortType::Gate,
        direction: PortDirection::Input,
    },
    PortInfo {
        name: "out",
        port_type: PortType::Cv,
        direction: PortDirection::Output,
    },
];

/// Cycle length in clock pulses (16ths) per `sync` step after FREE: 1/16 … 8 bars.
pub const SYNC_TICKS: [u32; 8] = [1, 2, 4, 8, 16, 32, 64, 128];
pub const SYNC_LABELS: [&str; 9] = [
    "FREE", "1/16", "1/8", "1/4", "1/2", "1BAR", "2BAR", "4BAR", "8BAR",
];

const PARAMS: &[ParamInfo] = &[
    ParamInfo {
        name: "rate_hz",
        min: 0.01,
        max: 100.0,
        default: 2.0,
        unit: "Hz",
        taper: Taper::Exponential,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "waveform",
        min: 0.0,
        max: 4.0,
        default: 0.0, // Sine
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "sync",
        min: 0.0,
        max: SYNC_TICKS.len() as f32,
        default: 0.0, // FREE
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    },
    ParamInfo {
        name: "phase",
        min: 0.0,
        max: 360.0,
        default: 0.0,
        unit: "°",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
];

pub static LFO_INFO: ModuleInfo = ModuleInfo {
    kind: "lfo",
    name: "LFO",
    category: Category::Modulator,
    rate: Rate::Voice,
    explain: "A slow wave for making other things wobble, sweep, or pulse. Patch a clock to \
              sync one cycle to a musical length.",
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
    width_units: 7,
    advanced: &["sync", "phase"],
};

const OUT: usize = 0;
const RATE_PARAM: usize = 0;
const WAVEFORM_PARAM: usize = 1;
const SYNC_PARAM: usize = 2;
const PHASE_PARAM: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LfoSync {
    /// `sync` is FREE.
    Free,
    /// Synced but no tempo accepted yet, or still gliding onto the beat.
    Acquiring,
    Synced,
    /// No clock edge for over two pulses: running on at the last tempo.
    Held,
}

fn waveform_from_param(v: f32) -> LfoWaveform {
    match v.round() as i32 {
        1 => LfoWaveform::Triangle,
        2 => LfoWaveform::Saw,
        3 => LfoWaveform::Square,
        4 => LfoWaveform::SampleAndHold,
        _ => LfoWaveform::Sine,
    }
}

#[derive(Clone, Copy)]
pub struct Lfo {
    lfo: FullLfo,
    /// Synced phase in f64: f32 drifts a few samples per pulse at slow rates.
    phase: f64,
    sample_rate: f32,
    clock_high: bool,
    reset_high: bool,
    /// A reset between pulses: the next edge jumps to `phase` and counts from 0.
    reset_armed: bool,
    seen_edge: bool,
    /// Samples since the last rising clock edge.
    since: u32,
    /// Accepted pulse period in samples; 0 = none.
    period: f32,
    cand: f32,
    /// Pulses since the last reset (or the first edge).
    count: u32,
    /// Phase correction in cycles per sample, applied until `since` reaches `period`.
    correction: f32,
    /// Phase error at the last edge, cycles.
    error: f32,
    status: LfoSync,
}

impl Lfo {
    pub fn new() -> Self {
        Lfo {
            lfo: FullLfo::new(),
            phase: 0.0,
            sample_rate: 48000.0,
            clock_high: false,
            reset_high: false,
            reset_armed: false,
            seen_edge: false,
            since: 0,
            period: 0.0,
            cand: 0.0,
            count: 0,
            correction: 0.0,
            error: 0.0,
            status: LfoSync::Free,
        }
    }

    pub fn status(&self) -> LfoSync {
        self.status
    }
}

impl Default for Lfo {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Lfo {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &LFO_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let rate_hz = io.param(RATE_PARAM).at(0);
        let waveform = waveform_from_param(io.param(WAVEFORM_PARAM).at(0));
        let sync = io.param(SYNC_PARAM).at(0).round() as usize;
        let ticks = SYNC_TICKS.get(sync.wrapping_sub(1)).copied();
        let offset = (io.param(PHASE_PARAM).at(0) / 360.0).rem_euclid(1.0);
        let n = io.block_len();
        let sr = self.sample_rate;
        let (clock, reset) = (io.input(0), io.input(1));
        let out = &mut io.output(OUT)[..n];
        for (i, o) in out.iter_mut().enumerate() {
            let c = clock.at(i) > 0.0;
            let r = reset.at(i) > 0.0;
            let edge = c && !self.clock_high;
            self.since = self.since.saturating_add(1);
            if r && !self.reset_high {
                if c || ticks.is_none() {
                    (self.lfo.phase, self.phase) = (offset, offset as f64);
                    self.count = 0;
                } else {
                    self.reset_armed = true;
                }
            }
            if edge {
                if self.seen_edge {
                    let interval = self.since as f32;
                    if self.period == 0.0 || (interval - self.cand).abs() <= 0.1 * self.cand {
                        self.period = interval;
                    }
                    self.cand = interval;
                }
                (self.seen_edge, self.since) = (true, 0);
                if self.reset_armed {
                    self.reset_armed = false;
                    (self.lfo.phase, self.phase) = (offset, offset as f64);
                    self.count = 0;
                } else if !r || self.reset_high {
                    self.count = self.count.wrapping_add(1);
                }
                if let Some(t) = ticks.filter(|_| self.period > 0.0) {
                    let target = ((self.count % t) as f32 / t as f32 + offset).fract();
                    let e = (target - self.phase as f32 + 1.5).rem_euclid(1.0) - 0.5;
                    let base = 1.0 / (t as f32 * self.period);
                    self.error = e;
                    self.correction = (e / self.period).clamp(-0.5 * base, 0.5 * base);
                }
            }
            (self.clock_high, self.reset_high) = (c, r);
            *o = match ticks {
                Some(t) if self.period > 0.0 => {
                    let corr = if (self.since as f32) < self.period {
                        self.correction as f64
                    } else {
                        0.0
                    };
                    let inc = 1.0 / (t as f64 * self.period as f64) + corr;
                    self.lfo.phase = self.phase as f32;
                    let v = self.lfo.next((inc * sr as f64) as f32, sr, waveform);
                    self.phase += inc;
                    if self.phase >= 1.0 {
                        self.phase -= 1.0;
                    }
                    v
                }
                _ => {
                    let v = self.lfo.next(rate_hz, sr, waveform);
                    self.phase = self.lfo.phase as f64;
                    v
                }
            };
        }
        self.status = match ticks {
            None => LfoSync::Free,
            Some(_) if self.period == 0.0 => LfoSync::Acquiring,
            Some(_) if self.since as f32 > 2.0 * self.period => LfoSync::Held,
            Some(_) if self.error.abs() > 0.01 => LfoSync::Acquiring,
            Some(_) => LfoSync::Synced,
        };
    }

    fn reset(&mut self) {
        *self = Lfo {
            sample_rate: self.sample_rate,
            ..Lfo::new()
        };
    }

    /// The sync state (tempo, count, correction) rides along exactly.
    fn carry_from(&mut self, old: &dyn Module) {
        if let Some(o) = old.as_any().downcast_ref::<Lfo>() {
            *self = Lfo {
                sample_rate: self.sample_rate,
                ..*o
            };
        }
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        out.write_f32("phase", self.lfo.phase);
        out.write_f32("held_sample", self.lfo.held_sample());
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        if let Some(phase) = s.read_f32("phase") {
            self.lfo.phase = phase;
        }
        if let Some(held) = s.read_f32("held_sample") {
            self.lfo.set_held_sample(held);
        }
    }
}
