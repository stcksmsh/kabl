//! `demo.logic` on the SDK: a control-rate logic module. Same behaviour as
//! `crates/modules/code-examples/logic`, with no `unsafe` and no layout code.
//!
//! Inputs `a`, `b` (CV), `gate`; params `mode`, `threshold`, `offset`; outputs `out` (CV) and
//! `trig` (gate). While `gate` is high the output holds its last value (a latch); otherwise it
//! follows the selected function of `a` and `b`.
#![no_std]
#![forbid(unsafe_code)]

use kabl_module_sdk::*;

struct Logic {
    latch: f32,
}

// Port and param indices. The const checks below make a rename or reorder a compile error.
const A: usize = 0;
const B: usize = 1;
const GATE: usize = 2;
const OUT: usize = 0;
const TRIG: usize = 1;
const MODE: usize = 0;
const THRESHOLD: usize = 1;
const OFFSET: usize = 2;

impl Module for Logic {
    const META: Meta = Meta::new("demo.logic", 1)
        .name("Logic")
        .explain("Combines two control signals: min, max, AND, XOR or a threshold test. Hold the gate to freeze the output.")
        .category(Category::Utility)
        .rate(Rate::Global)
        .width(4)
        .port_in(PortType::Cv, "a", 1)
        .port_in(PortType::Cv, "b", 1)
        .port_in(PortType::Gate, "gate", 1)
        .port_out(PortType::Cv, "out", 1)
        .port_out(PortType::Gate, "trig", 1)
        .param("mode", "0", "4", "0", Taper::Stepped, 0, "mode")
        .param("threshold", "-1", "1", "0.5", Taper::Linear, 5, "cv")
        .param("offset", "-1", "1", "0", Taper::Linear, 5, "cv");
    const STATE_CAP: usize = 4;

    fn new(_: &mut Init) -> Self {
        Logic { latch: 0.0 }
    }

    fn reset(&mut self) {
        self.latch = 0.0;
    }

    fn process(&mut self, io: &mut Io) {
        let mode = (io.param(MODE) + 0.5) as i32;
        let thr = io.param(THRESHOLD);
        let offset = io.param(OFFSET);
        let mut latch = self.latch;
        for i in 0..io.n {
            let a = io.input[A][i];
            let b = io.input[B][i];
            let held = io.input[GATE][i] > 0.5;
            let value = match mode {
                0 => a.min(b),
                1 => a.max(b),
                2 => f32::from(a > thr && b > thr),
                3 => f32::from((a > thr) != (b > thr)),
                _ => f32::from(a > thr),
            };
            if !held {
                latch = value + offset;
            }
            io.output[OUT][i] = latch;
            io.output[TRIG][i] = f32::from(latch > thr);
        }
        self.latch = latch;
    }

    fn save(&self, w: &mut StateWriter) {
        w.f32(self.latch);
    }

    fn load(&mut self, _version: u32, bytes: &[u8]) -> bool {
        match StateReader::new(bytes).f32() {
            Some(v) => {
                self.latch = v;
                true
            }
            None => false,
        }
    }
}

const _: () = {
    Logic::META.assert_in(A, "a");
    Logic::META.assert_in(B, "b");
    Logic::META.assert_in(GATE, "gate");
    Logic::META.assert_out(OUT, "out");
    Logic::META.assert_out(TRIG, "trig");
    Logic::META.assert_param(MODE, "mode");
    Logic::META.assert_param(THRESHOLD, "threshold");
    Logic::META.assert_param(OFFSET, "offset");
};

kabl_module!(Logic);
