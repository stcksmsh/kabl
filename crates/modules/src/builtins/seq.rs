//! `seq`: an 8-step pitch/gate sequencer. Each rising edge on `clock` advances one step; `pitch`
//! holds that step's pitch (whole semitones, same reference as `midi.in`'s `pitch`) and
//! `velocity` its velocity (0..1, independent of the step's gate, so a rest stays a rest).
//! `transpose` shifts every step by whole semitones. The pattern loops over the first `length`
//! steps. A rising edge on `reset` jumps to step 1 if the clock is high, otherwise it makes the
//! next clock tick play step 1.
//!
//! Gate timing (`gate_mode`):
//! - CLOCK (default, the original behaviour): `gate` follows the clock pulse while the step is
//!   on, so consecutive on-steps retrigger an envelope.
//! - LENGTH: `gate` rises on the step's clock edge and stays high for `gate_len` % of the step
//!   period. The period is the interval between rising edges, so a divided clock works. An
//!   interval counts when it agrees within 10 % with the one before it (the delay's sync rule):
//!   a steady or slowly moving tempo is followed at once, a jump after two agreeing intervals,
//!   and the long interval across a Stop or the short one of a Restart never sets it. Until
//!   the first period is known (the first step after a load), the gate follows the clock. A gate
//!   still high when the next step starts (100 %, or the tempo sped up) drops for that one
//!   sample so the next note retriggers. Stop lets the playing gate finish its length.
//!
//! An input counts as high above 0, not at the usual 0.5: a `midi.in` gate reaching this global
//! module is averaged over the voices, so one held key arrives as 1 / voice count.
//!
//! Global rate, like `clock`: every voice of a voice-rate chain it feeds plays the same notes,
//! and the compiler's voice average keeps that as loud as one voice.
//! ponytail: the chain runs once per voice for one line of notes; skip voice instancing for
//! global-driven chains if that cost ever shows up on the Pi.

use crate::info::{
    Category, ModuleInfo, ParamInfo, PortDirection, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig, StateReader, StateWriter};

pub const STEPS: usize = 8;

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
        name: "gate",
        port_type: PortType::Gate,
        direction: PortDirection::Output,
    },
    PortInfo {
        name: "pitch",
        port_type: PortType::Pitch,
        direction: PortDirection::Output,
    },
    PortInfo {
        name: "velocity",
        port_type: PortType::UnipolarCv,
        direction: PortDirection::Output,
    },
];

const fn pitch(name: &'static str, default: f32) -> ParamInfo {
    ParamInfo {
        name,
        min: -24.0,
        max: 24.0,
        default,
        unit: "st",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    }
}

const fn gate(name: &'static str) -> ParamInfo {
    ParamInfo {
        name,
        min: 0.0,
        max: 1.0,
        default: 1.0,
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    }
}

const fn velocity(name: &'static str) -> ParamInfo {
    ParamInfo {
        name,
        min: 0.0,
        max: 100.0,
        default: 100.0,
        unit: "%",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    }
}

/// Pitches `p1..p8`, gates `g1..g8`, `length`, `transpose`, velocities `v1..v8`, `gate_len`,
/// then `gate_mode`; `process` relies on that order.
const PARAMS: &[ParamInfo] = &[
    pitch("p1", 0.0),
    pitch("p2", 3.0),
    pitch("p3", 7.0),
    pitch("p4", 10.0),
    pitch("p5", 12.0),
    pitch("p6", 10.0),
    pitch("p7", 7.0),
    pitch("p8", 3.0),
    gate("g1"),
    gate("g2"),
    gate("g3"),
    gate("g4"),
    gate("g5"),
    gate("g6"),
    gate("g7"),
    gate("g8"),
    ParamInfo {
        name: "length",
        min: 1.0,
        max: STEPS as f32,
        default: STEPS as f32,
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    },
    pitch("transpose", 0.0),
    velocity("v1"),
    velocity("v2"),
    velocity("v3"),
    velocity("v4"),
    velocity("v5"),
    velocity("v6"),
    velocity("v7"),
    velocity("v8"),
    ParamInfo {
        name: "gate_len",
        min: 1.0,
        max: 100.0,
        default: 50.0,
        unit: "%",
        taper: Taper::Linear,
        smoothing_ms: 0.0,
    },
    // CLOCK, LENGTH.
    ParamInfo {
        name: "gate_mode",
        min: 0.0,
        max: 1.0,
        default: 0.0,
        unit: "",
        taper: Taper::Stepped,
        smoothing_ms: 0.0,
    },
];

pub static SEQ_INFO: ModuleInfo = ModuleInfo {
    kind: "seq",
    name: "Sequencer",
    category: Category::Sequencer,
    rate: Rate::Global,
    explain:
        "Plays a looping pattern of 8 notes, one step per clock tick; switch steps off for rests.",
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
    width_units: 22,
    advanced: &[
        "length",
        "transpose",
        "v1",
        "v2",
        "v3",
        "v4",
        "v5",
        "v6",
        "v7",
        "v8",
        "gate_len",
        "gate_mode",
    ],
};

const LENGTH_PARAM: usize = 2 * STEPS;
const TRANSPOSE_PARAM: usize = 2 * STEPS + 1;
const VELOCITY_PARAM: usize = 2 * STEPS + 2;
const GATE_LEN_PARAM: usize = 3 * STEPS + 2;
const GATE_MODE_PARAM: usize = 3 * STEPS + 3;

/// Everything `process` carries from sample to sample.
#[derive(Clone, Copy)]
struct St {
    step: usize,
    clock_high: bool,
    reset_high: bool,
    gate_high: bool,
    seen_edge: bool,
    /// Samples since the last rising clock edge.
    since: u32,
    /// Accepted step period in samples; 0 = none yet.
    period: f32,
    /// The last measured interval, accepted or not; 0 = none.
    cand: f32,
}

pub struct Seq {
    s: St,
}

impl Seq {
    pub fn new() -> Self {
        // Parked on the last step so the first tick plays step 1.
        Seq {
            s: St {
                step: STEPS - 1,
                clock_high: false,
                reset_high: false,
                gate_high: false,
                seen_edge: false,
                since: 0,
                period: 0.0,
                cand: 0.0,
            },
        }
    }

    /// The step playing now, 0-based (for the UI's step light).
    pub fn step(&self) -> usize {
        self.s.step
    }
}

impl Default for Seq {
    fn default() -> Self {
        Self::new()
    }
}

/// An interval between rising clock edges becomes the period when it agrees within 10 % with
/// the interval before it (or when there is no period yet), the delay's sync rule.
fn accept(s: &mut St, interval: f32) {
    if s.period == 0.0 || (interval - s.cand).abs() <= 0.1 * s.cand {
        s.period = interval;
    }
    s.cand = interval;
}

impl Module for Seq {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &SEQ_INFO
    }

    fn prepare(&mut self, _sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {}

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let transpose = io.param(TRANSPOSE_PARAM).at(0).round();
        let pitches: [f32; STEPS] = std::array::from_fn(|k| io.param(k).at(0).round() + transpose);
        let gates: [bool; STEPS] = std::array::from_fn(|k| io.param(STEPS + k).at(0) >= 0.5);
        let velocities: [f32; STEPS] =
            std::array::from_fn(|k| (io.param(VELOCITY_PARAM + k).at(0) / 100.0).clamp(0.0, 1.0));
        let length = (io.param(LENGTH_PARAM).at(0).round() as usize).clamp(1, STEPS);
        let timed = io.param(GATE_MODE_PARAM).at(0) >= 0.5;
        let gate_len = (io.param(GATE_LEN_PARAM).at(0) / 100.0).clamp(0.01, 1.0);
        let (clock, reset) = (io.input(0), io.input(1));
        let n = io.block_len();
        // Walks the inputs from the stored state, yielding the state after each sample. Run once
        // per output: `ProcessIo` lends one output buffer at a time.
        let start = self.s;
        let walk = move || {
            (0..n).scan(start, move |s, i| {
                let c = clock.at(i) > 0.0;
                let edge = c && !s.clock_high;
                s.since = s.since.saturating_add(1);
                if edge {
                    s.step = if s.step + 1 >= length { 0 } else { s.step + 1 };
                    if s.seen_edge {
                        accept(s, s.since as f32);
                    }
                    (s.seen_edge, s.since) = (true, 0);
                }
                // During a clock pulse, reset restarts that pulse's note on step 1 (two clocks
                // rarely tick on the same sample); between pulses it arms step 1 for the next.
                let r = reset.at(i) > 0.0;
                if r && !s.reset_high {
                    s.step = if c { 0 } else { length - 1 };
                }
                let on = gates[s.step];
                s.gate_high = if timed && s.period > 0.0 {
                    let len = (gate_len * s.period).max(2.0);
                    on && (s.since as f32) < len && !(edge && s.gate_high)
                } else {
                    on && c
                };
                (s.clock_high, s.reset_high) = (c, r);
                Some(*s)
            })
        };
        for (g, s) in io.output(0)[..n].iter_mut().zip(walk()) {
            *g = s.gate_high as u8 as f32;
        }
        for (p, s) in io.output(1)[..n].iter_mut().zip(walk()) {
            *p = pitches[s.step];
        }
        for (v, s) in io.output(2)[..n].iter_mut().zip(walk()) {
            *v = velocities[s.step];
            self.s = s;
        }
    }

    fn reset(&mut self) {
        *self = Seq::new();
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        let s = &self.s;
        out.write_f32("step", s.step as f32);
        for (k, v) in [
            ("clock_high", s.clock_high),
            ("reset_high", s.reset_high),
            ("gate_high", s.gate_high),
            ("seen_edge", s.seen_edge),
        ] {
            out.write_f32(k, v as u8 as f32);
        }
        // f32 holds whole sample counts exactly up to 2^24 (~5.8 min at 48 kHz); beyond that
        // only "a long time ago" matters.
        out.write_f32("since", s.since as f32);
        out.write_f32("period", s.period);
        out.write_f32("cand", s.cand);
    }

    fn load_state(&mut self, r: &dyn StateReader) {
        let s = &mut self.s;
        if let Some(v) = r.read_f32("step") {
            s.step = (v as usize).min(STEPS - 1);
        }
        for (k, v) in [
            ("clock_high", &mut s.clock_high),
            ("reset_high", &mut s.reset_high),
            ("gate_high", &mut s.gate_high),
            ("seen_edge", &mut s.seen_edge),
        ] {
            if let Some(x) = r.read_f32(k) {
                *v = x >= 0.5;
            }
        }
        if let Some(v) = r.read_f32("since") {
            s.since = v as u32;
        }
        if let Some(v) = r.read_f32("period") {
            s.period = v;
        }
        if let Some(v) = r.read_f32("cand") {
            s.cand = v;
        }
    }
}
