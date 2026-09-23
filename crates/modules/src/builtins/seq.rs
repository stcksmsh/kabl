//! `seq`: an 8-step pitch/gate sequencer. Each rising edge on `clock` advances one step; `pitch`
//! holds that step's pitch (whole semitones, same reference as `midi.in`'s `pitch`), and `gate`
//! follows the clock pulse while the step is on, so consecutive on-steps retrigger an envelope.
//! The pattern loops over the first `length` steps. A rising edge on `reset` jumps to step 1 if
//! the clock is high, otherwise it makes the next clock tick play step 1.
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

/// Pitches `p1..p8`, gates `g1..g8`, then `length`; `process` relies on that order.
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
    advanced: &["length"],
};

const LENGTH_PARAM: usize = 2 * STEPS;

pub struct Seq {
    step: usize,
    clock_high: bool,
    reset_high: bool,
}

impl Seq {
    pub fn new() -> Self {
        // Parked on the last step so the first tick plays step 1.
        Seq {
            step: STEPS - 1,
            clock_high: false,
            reset_high: false,
        }
    }

    /// The step playing now, 0-based (for the UI's step light).
    pub fn step(&self) -> usize {
        self.step
    }
}

impl Default for Seq {
    fn default() -> Self {
        Self::new()
    }
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
        let pitches: [f32; STEPS] = std::array::from_fn(|k| io.param(k).at(0).round());
        let gates: [bool; STEPS] = std::array::from_fn(|k| io.param(STEPS + k).at(0) >= 0.5);
        let length = (io.param(LENGTH_PARAM).at(0).round() as usize).clamp(1, STEPS);
        let (clock, reset) = (io.input(0), io.input(1));
        let n = io.block_len();
        // Walks the inputs from the stored state, yielding (clock high, reset high, step) per sample. Run
        // once per output: `ProcessIo` lends one output buffer at a time.
        let start = (self.step, self.clock_high, self.reset_high);
        let walk = move || {
            (0..n).scan(start, move |(step, c_high, r_high), i| {
                let c = clock.at(i) > 0.0;
                if c && !*c_high {
                    *step = if *step + 1 >= length { 0 } else { *step + 1 };
                }
                // During a clock pulse, reset restarts that pulse's note on step 1 (two clocks
                // rarely tick on the same sample); between pulses it arms step 1 for the next.
                let r = reset.at(i) > 0.0;
                if r && !*r_high {
                    *step = if c { 0 } else { length - 1 };
                }
                (*c_high, *r_high) = (c, r);
                Some((c, r, *step))
            })
        };
        for (g, (c, _, step)) in io.output(0)[..n].iter_mut().zip(walk()) {
            *g = if c && gates[step] { 1.0 } else { 0.0 };
        }
        for (p, (c, r, step)) in io.output(1)[..n].iter_mut().zip(walk()) {
            *p = pitches[step];
            (self.step, self.clock_high, self.reset_high) = (step, c, r);
        }
    }

    fn reset(&mut self) {
        *self = Seq::new();
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        out.write_f32("step", self.step as f32);
        out.write_f32("clock_high", self.clock_high as u8 as f32);
        out.write_f32("reset_high", self.reset_high as u8 as f32);
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        if let Some(v) = s.read_f32("step") {
            self.step = (v as usize).min(STEPS - 1);
        }
        if let Some(v) = s.read_f32("clock_high") {
            self.clock_high = v >= 0.5;
        }
        if let Some(v) = s.read_f32("reset_high") {
            self.reset_high = v >= 0.5;
        }
    }
}
