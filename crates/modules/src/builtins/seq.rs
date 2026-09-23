//! `seq`: an 8-step pitch/gate sequencer. Each rising edge on `clock` advances one step; `pitch`
//! holds that step's pitch (whole semitones, same reference as `midi.in`'s `pitch`), and `gate`
//! follows the clock pulse while the step is on, so consecutive on-steps retrigger an envelope.
//! The pattern loops over all 8 steps.
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

/// Pitches `p1..p8` first, then gates `g1..g8`; `process` relies on that order.
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
    advanced: &[],
};

pub struct Seq {
    step: usize,
    clock_high: bool,
}

impl Seq {
    pub fn new() -> Self {
        // Parked on the last step so the first tick plays step 1.
        Seq {
            step: STEPS - 1,
            clock_high: false,
        }
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
        let clock = io.input(0);
        let n = io.block_len();
        // Walks the clock from the stored state, yielding (clock high, step) per sample. Run once
        // per output: `ProcessIo` lends one output buffer at a time.
        let (start, was_high) = (self.step, self.clock_high);
        let walk = move || {
            (0..n).scan((start, was_high), move |(step, high), i| {
                let c = clock.at(i) >= 0.5;
                if c && !*high {
                    *step = (*step + 1) % STEPS;
                }
                *high = c;
                Some((c, *step))
            })
        };
        for (g, (c, step)) in io.output(0)[..n].iter_mut().zip(walk()) {
            *g = if c && gates[step] { 1.0 } else { 0.0 };
        }
        for (p, (c, step)) in io.output(1)[..n].iter_mut().zip(walk()) {
            *p = pitches[step];
            (self.step, self.clock_high) = (step, c);
        }
    }

    fn reset(&mut self) {
        *self = Seq::new();
    }

    fn save_state(&self, out: &mut dyn StateWriter) {
        out.write_f32("step", self.step as f32);
        out.write_f32("clock_high", self.clock_high as u8 as f32);
    }

    fn load_state(&mut self, s: &dyn StateReader) {
        if let Some(v) = s.read_f32("step") {
            self.step = (v as usize).min(STEPS - 1);
        }
        if let Some(v) = s.read_f32("clock_high") {
            self.clock_high = v >= 0.5;
        }
    }
}
