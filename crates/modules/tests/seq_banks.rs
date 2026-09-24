//! Pattern banks, direction, probability and arming on `seq`, sample by sample.

use kabl_modules::builtins::seq::{bank_of, bank_param, param_index, BANKS, SLOTS, STEPS};
use kabl_modules::builtins::{Clock, Seq, SEQ_INFO};
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{Module, ProcessIo, Signal};

const SR: f32 = 48000.0;
const BLOCK: usize = 64;
const STEP: usize = 6000; // samples per 16th at 120 bpm

struct Rig {
    clock: Clock,
    seq: Seq,
    params: Vec<f32>,
    t: usize,
    high: bool,
    /// Per clock edge: the pitch output and whether the gate opened in that step.
    steps: Vec<(f32, bool)>,
}

impl Rig {
    fn new() -> Self {
        let q = QualityConfig {
            tier: QualityTier::Live,
        };
        let (mut clock, mut seq) = (Clock::new(), Seq::new());
        clock.prepare(SR, BLOCK, &q);
        seq.prepare(SR, BLOCK, &q);
        let mut params: Vec<f32> = SEQ_INFO.params.iter().map(|p| p.default).collect();
        // Every bank's step k plays pitch 10 * bank + k, so the output names bank and step.
        for b in 0..BANKS {
            for k in 0..STEPS {
                params[param_index(b, k)] = (10 * b + k) as f32;
            }
        }
        Rig {
            clock,
            seq,
            params,
            t: 0,
            high: false,
            steps: Vec::new(),
        }
    }

    fn set(&mut self, name: &str, v: f32) -> &mut Self {
        let i = SEQ_INFO.params.iter().position(|p| p.name == name).unwrap();
        self.params[i] = v;
        self
    }

    fn block(&mut self, reset_at: Option<usize>) {
        let (mut tick, mut clock_reset) = ([0f32; BLOCK], [0f32; BLOCK]);
        let mut outs: [&mut [f32]; 2] = [&mut tick, &mut clock_reset];
        self.clock.process(&mut ProcessIo::new(
            &[],
            &mut outs,
            &[Signal::Scalar(120.0)],
            BLOCK,
        ));
        let mut reset = [0f32; BLOCK];
        if let Some(r) = reset_at {
            for (i, s) in reset.iter_mut().enumerate() {
                let t = self.t + i;
                *s = (t >= r && t < r + 100) as u8 as f32;
            }
        }
        let params: Vec<Signal> = self.params.iter().map(|&v| Signal::Scalar(v)).collect();
        let (mut gate, mut pitch, mut vel) = ([0f32; BLOCK], [0f32; BLOCK], [0f32; BLOCK]);
        let mut outs: [&mut [f32]; 3] = [&mut gate, &mut pitch, &mut vel];
        let ins = [Signal::Buffer(&tick), Signal::Buffer(&reset)];
        self.seq
            .process(&mut ProcessIo::new(&ins, &mut outs, &params, BLOCK));
        for i in 0..BLOCK {
            if tick[i] > 0.5 && !self.high {
                self.steps.push((pitch[i], false));
            }
            self.high = tick[i] > 0.5;
            if gate[i] > 0.5 {
                if let Some(s) = self.steps.last_mut() {
                    s.1 = true;
                }
            }
        }
        self.t += BLOCK;
    }

    /// Runs whole blocks up to (not past) `steps` 16ths from now.
    fn run(&mut self, steps: usize) -> &mut Self {
        let end = self.t + steps * STEP;
        self.run_to(end)
    }

    fn run_to(&mut self, end: usize) -> &mut Self {
        while self.t + BLOCK <= end {
            self.block(None);
        }
        self
    }

    fn pitches(&self) -> Vec<f32> {
        self.steps.iter().map(|s| s.0).collect()
    }
}

#[test]
fn bank_a_keeps_the_historic_names_and_indices() {
    let names: Vec<&str> = SEQ_INFO.params.iter().map(|p| p.name).collect();
    assert_eq!(&names[..3], ["p1", "p2", "p3"]);
    assert_eq!(names[16], "length");
    assert_eq!(names[17], "transpose");
    assert_eq!(names[18], "v1");
    assert_eq!(names[26], "gate_len");
    assert_eq!(names[27], "gate_mode");
    assert_eq!(names.len(), 4 * SLOTS + 3);
    assert_eq!(bank_of("p3"), Some((0, 2)));
    assert_eq!(bank_of("c.v2"), Some((2, 18)));
    assert_eq!(bank_of("transpose"), None);
    assert_eq!(bank_param(3, 16), "d.length");
    for (i, p) in SEQ_INFO.params.iter().enumerate() {
        if let Some((b, s)) = bank_of(p.name) {
            assert_eq!(param_index(b, s), i, "{}", p.name);
        }
    }
}

#[test]
fn directions_play_their_documented_order() {
    let run = |dir: f32| {
        let mut r = Rig::new();
        r.set("length", 4.0).set("direction", dir).run(12);
        r.pitches()
    };
    assert_eq!(run(0.0), [0., 1., 2., 3., 0., 1., 2., 3., 0., 1., 2., 3.]);
    assert_eq!(run(1.0), [3., 2., 1., 0., 3., 2., 1., 0., 3., 2., 1., 0.]);
    // Endpoints once per turn: period 2L - 2.
    assert_eq!(run(2.0), [0., 1., 2., 3., 2., 1., 0., 1., 2., 3., 2., 1.]);
}

#[test]
fn pendulum_of_one_and_two_steps() {
    let mut r = Rig::new();
    r.set("length", 2.0).set("direction", 2.0).run(5);
    assert_eq!(r.pitches(), [0., 1., 0., 1., 0.]);
    let mut r = Rig::new();
    r.set("length", 1.0).set("direction", 2.0).run(3);
    assert_eq!(r.pitches(), [0., 0., 0.]);
}

#[test]
fn reset_restarts_on_the_directions_start_step() {
    let mut r = Rig::new();
    r.set("length", 4.0).set("direction", 1.0);
    r.run(2);
    // A reset between pulses: the next tick plays step L (reverse start).
    let at = 2 * STEP + STEP / 2 + 500; // the third pulse ended at 15000
    while r.t < at + 200 {
        r.block(Some(at));
    }
    r.run(3);
    assert_eq!(r.pitches(), [3., 2., 1., 3., 2., 1.]);
}

#[test]
fn startup_bank_is_read_by_a_fresh_instance() {
    let mut r = Rig::new();
    r.set("bank", 2.0).run(3);
    assert_eq!(r.pitches(), [20., 21., 22.]);
    assert_eq!(r.seq.bank(), 2);
}

#[test]
fn an_armed_bank_starts_on_the_first_edge_at_or_after_the_offset() {
    // The third edge is at 12000: offset 32 of the block starting at 11968.
    let mut r = Rig::new();
    r.run_to(11968);
    r.seq.arm(1, 33);
    r.run_to(4 * STEP + 100);
    assert_eq!(r.pitches(), [0., 1., 2., 10., 11.]);
    assert_eq!((r.seq.bank(), r.seq.armed()), (1, None));

    let mut r = Rig::new();
    r.run_to(11968);
    r.seq.arm(3, 32);
    r.run_to(3 * STEP + 100);
    assert_eq!(r.pitches(), [0., 1., 30., 31.]);
}

#[test]
fn editing_an_inactive_bank_changes_nothing_audible() {
    let mut a = Rig::new();
    let mut b = Rig::new();
    b.set("b.p1", 24.0).set("c.g3", 0.0).set("d.length", 2.0);
    a.run(10);
    b.run(10);
    assert_eq!(a.steps, b.steps);
}

#[test]
fn a_rejected_step_is_a_rest_and_keeps_its_time() {
    let mut r = Rig::new();
    r.set("r2", 0.0).run(10);
    let played: Vec<bool> = r.steps.iter().map(|s| s.1).collect();
    assert_eq!(
        played,
        [true, false, true, true, true, true, true, true, true, false]
    );
    // The pitch still moved on: step 2 is there, silent.
    assert_eq!(r.pitches()[..3], [0., 1., 2.]);
}

#[test]
fn probability_is_reproducible_and_seeded_per_module() {
    let run = |seed: u64| {
        let mut r = Rig::new();
        r.seq.seed(seed);
        for k in 1..=8 {
            r.set(&format!("r{k}"), 50.0);
        }
        r.run(64);
        r.steps.iter().map(|s| s.1).collect::<Vec<_>>()
    };
    let (a, b, c) = (run(7), run(7), run(8));
    assert_eq!(a, b);
    assert_ne!(a, c);
    let hits = a.iter().filter(|&&p| p).count();
    assert!((20..=44).contains(&hits), "{hits} of 64 at 50 %");
}

#[test]
fn a_reset_replays_the_same_decisions() {
    let mut r = Rig::new();
    r.seq.seed(3);
    for k in 1..=8 {
        r.set(&format!("r{k}"), 50.0);
    }
    // Reset during the first pulse of each 16-step pass.
    let mut passes = Vec::new();
    for _ in 0..2 {
        let at = r.t + 10;
        let end = r.t + 16 * STEP;
        r.steps.clear();
        while r.t < end {
            r.block(Some(at));
        }
        passes.push(r.steps.iter().map(|s| s.1).collect::<Vec<_>>());
    }
    assert_eq!(passes[0], passes[1]);
}

#[test]
fn changing_a_probability_does_not_shift_the_stream() {
    let run = |r3: f32| {
        let mut r = Rig::new();
        r.seq.seed(11);
        for k in 1..=8 {
            r.set(&format!("r{k}"), 50.0);
        }
        r.set("r3", r3).run(32);
        r.steps.iter().map(|s| s.1).collect::<Vec<_>>()
    };
    let (a, b) = (run(50.0), run(100.0));
    for (i, (x, y)) in a.iter().zip(&b).enumerate() {
        if i % 8 != 2 {
            assert_eq!(x, y, "step {i}");
        }
    }
}

#[test]
fn carried_state_keeps_bank_arm_heading_and_stream() {
    let mut r = Rig::new();
    r.seq.seed(5);
    r.set("direction", 2.0).set("r4", 50.0).run(5);
    r.seq.arm(2, 0);
    let mut copy = Seq::new();
    copy.carry_from(&r.seq);
    assert_eq!(
        (copy.bank(), copy.armed(), copy.step()),
        (0, Some(2), r.seq.step())
    );
    let mut twin = Rig::new();
    twin.params = r.params.clone();
    twin.clock = r.clock;
    twin.seq = copy;
    twin.t = r.t;
    r.run(12);
    twin.run(12);
    assert_eq!(r.steps[5..], twin.steps[..]);
}
