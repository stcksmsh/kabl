//! Cue macro morphs through the real compiler and `PatchEngine` (`morph.rs`): landing on the
//! bar, length in clock pulses, the next cue arriving early, a hand or the host taking the
//! macro, transport, restart and tempo changes. The audio-thread calls run under
//! `assert_no_alloc`.

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::Collector;
use kabl_core::{ModuleId, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::morph::MacroTarget;
use kabl_engine::patch_engine::{Launch, PatchEngine, Timing};
use kabl_engine::runtime::{ParamSet, RuntimeTarget};
use kabl_modules::builtins::Transport;

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;
const CLOCK: ModuleId = 1;
const MACRO: ModuleId = 2;
const OUT: ModuleId = 3;
/// Joins the two signals the tests read on the left and right output.
const JOIN: ModuleId = 90;
/// 120 bpm: a 16th is 6000 samples, a bar 96000 samples = 1500 blocks.
const BAR_BLOCKS: usize = 1500;
const PULSE_BLOCKS: usize = 93; // 6000 / 64 rounded down, for stepping near a pulse

fn module(kind: &str, params: &[(&str, f32)]) -> ModuleState {
    ModuleState {
        kind: kind.into(),
        pos: Vec2::default(),
        params: params.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
    }
}

/// clock, macro (m1 = 0.2, m2 = 0.7) with m1 and m2 as the left and right output.
fn patch() -> PatchState {
    let mut p = PatchState::new();
    p.modules.insert(CLOCK, module("clock", &[("bpm", 120.0)]));
    p.modules
        .insert(MACRO, module("macro", &[("m1", 0.2), ("m2", 0.7)]));
    p.modules.insert(OUT, module("out", &[]));
    p.modules.insert(JOIN, module("join", &[]));
    let m = |id, port: &str| PortRef::Module {
        id,
        port: port.into(),
    };
    for (i, (from, to)) in [
        (m(MACRO, "m1"), m(JOIN, "left")),
        (m(MACRO, "m2"), m(JOIN, "right")),
        (m(JOIN, "out"), m(OUT, "in")),
    ]
    .into_iter()
    .enumerate()
    {
        p.cables.insert(
            i as u64 + 1,
            kabl_core::CableState {
                from,
                to,
                params: Default::default(),
                steps: Vec::new(),
            },
        );
    }
    p
}

struct Rig {
    e: PatchEngine,
    _c: Collector,
    /// Left and right of the last block, per block.
    last: (f32, f32),
}

fn t(index: u16) -> RuntimeTarget {
    RuntimeTarget::Param {
        id: MACRO,
        kind: "macro",
        index,
    }
}

impl Rig {
    fn new() -> Self {
        let c = Collector::new();
        let e = PatchEngine::new(&c.handle(), &patch(), SR, 8).unwrap();
        let mut r = Rig {
            e,
            _c: c,
            last: (0.0, 0.0),
        };
        r.run(50); // the macro starts at its knob, the clock at pulse 0
        r
    }

    fn run(&mut self, blocks: usize) -> f32 {
        let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
        let mut worst: f32 = 0.0;
        for _ in 0..blocks {
            assert_no_alloc(|| self.e.process_block(&mut l, &mut r));
            for &x in &l {
                worst = worst.max((x - self.last.0).abs());
                self.last.0 = x;
            }
            self.last.1 = r[BLOCK - 1];
        }
        worst
    }

    /// Runs to the start of the next bar, plus `extra` blocks.
    fn run_to_bar(&mut self, extra: usize) {
        let mut pos = 0.0;
        self.e.clocks(|_, _, p| pos = p);
        let left = (16.0 - pos.rem_euclid(16.0)) * 6000.0 / BLOCK as f64;
        self.run(left as usize + extra);
    }

    fn launch(&mut self, to: &[(u8, f32)], pulses: f32, timing: Timing) {
        let ms: Vec<_> = to
            .iter()
            .map(|&(index, value)| MacroTarget {
                id: MACRO,
                index,
                value,
            })
            .collect();
        let l = Launch::new(CLOCK, timing, &[]).with_macros(&ms, pulses);
        assert_no_alloc(|| {
            self.e
                .command(&kabl_engine::patch_engine::Command::Launch(l))
        });
    }

    fn value(&mut self, index: u16) -> f32 {
        self.e.active_mut().runtime_value(t(index)).unwrap()
    }

    fn set(&mut self, index: u16, v: f32) {
        let s = ParamSet {
            rev: 1,
            target: t(index),
            value: v,
        };
        assert_no_alloc(|| self.e.set(&s));
    }
}

/// The output the listener hears (left = m1): settles 20 ms after the param moved.
fn heard(r: &mut Rig) -> f32 {
    r.run(300);
    r.last.0
}

#[test]
fn lands_on_the_bar_and_jumps_with_zero_length() {
    let mut r = Rig::new();
    r.launch(&[(0, 0.9)], 0.0, Timing::NextBar);
    r.run(BAR_BLOCKS / 2);
    assert_eq!(r.value(0), 0.2, "still waiting for the bar");
    r.run_to_bar(2);
    assert_eq!(r.value(0), 0.9);
    assert!((heard(&mut r) - 0.9).abs() < 1e-3);
}

#[test]
fn macro_without_a_target_is_left_alone() {
    let mut r = Rig::new();
    r.launch(&[(0, 0.9)], 16.0, Timing::NextBar);
    r.run_to_bar(0);
    r.run(BAR_BLOCKS + 20);
    assert_eq!(r.value(1), 0.7);
    assert!((r.last.1 - 0.7).abs() < 1e-4);
}

#[test]
fn morph_follows_the_clock_and_has_no_steps() {
    let mut r = Rig::new();
    r.launch(&[(0, 0.8)], 16.0, Timing::NextBar);
    r.run_to_bar(0);
    let mut worst: f32 = 0.0;
    let mut prev = 0.2;
    // Half a bar in: half way.
    worst = worst.max(r.run(BAR_BLOCKS / 2));
    let mid = r.value(0);
    assert!((mid - 0.5).abs() < 0.02, "mid {mid}");
    for _ in 0..BAR_BLOCKS / 2 + 5 {
        worst = worst.max(r.run(1));
        let v = r.value(0);
        assert!(v >= prev - 1e-6, "monotonic");
        prev = v;
    }
    assert_eq!(r.value(0), 0.8);
    // 0.6 over 96000 samples: any step above a few 1e-5 per sample would be a zipper.
    assert!(worst < 1e-4, "step {worst}");
}

#[test]
fn next_cue_before_the_end_starts_from_where_the_macro_is() {
    let mut r = Rig::new();
    r.launch(&[(0, 1.0)], 16.0, Timing::NextBar);
    r.run_to_bar(0);
    r.run(BAR_BLOCKS / 4);
    r.launch(&[(0, 0.0)], 0.0, Timing::Now);
    r.run(1);
    assert_eq!(r.value(0), 0.0);
    r.run(BAR_BLOCKS);
    assert_eq!(r.value(0), 0.0, "the old morph is gone");
    // A second morph that lands half way through the first starts from where it had got to.
    r.launch(&[(0, 1.0)], 16.0, Timing::NextBar);
    r.run_to_bar(0);
    r.run(BAR_BLOCKS / 2);
    r.launch(&[(0, 0.0)], 16.0, Timing::NextStep);
    r.run(PULSE_BLOCKS + 5);
    let v = r.value(0);
    assert!((0.4..0.65).contains(&v), "still on the first morph: {v}");
    r.run(BAR_BLOCKS / 2);
    let w = r.value(0);
    assert!(w < v, "now heading down: {v} {w}");
    r.run(BAR_BLOCKS * 2);
    assert_eq!(r.value(0), 0.0);
}

#[test]
fn a_hand_wins_and_the_next_cue_takes_over_again() {
    let mut r = Rig::new();
    r.launch(&[(0, 1.0)], 16.0, Timing::NextBar);
    r.run_to_bar(0);
    r.run(BAR_BLOCKS / 4);
    r.set(0, 0.1);
    r.run(BAR_BLOCKS);
    assert_eq!(r.value(0), 0.1, "the morph ended at the grab");
    r.e.morphs(|_, _, _, _, _| panic!("nothing is driven after a grab"));
    r.launch(&[(0, 0.6)], 0.0, Timing::NextBar);
    r.run_to_bar(2);
    assert_eq!(r.value(0), 0.6);
}

#[test]
fn host_automation_wins_like_a_hand() {
    let mut r = Rig::new();
    r.launch(&[(0, 1.0)], 16.0, Timing::NextBar);
    r.run_to_bar(0);
    r.run(BAR_BLOCKS / 4);
    r.e.automate(0, t(0), 0.3);
    r.run(BAR_BLOCKS);
    assert_eq!(r.value(0), 0.3);
    // A cue morph does not make a steady host value take the macro back.
    r.launch(&[(0, 0.9)], 0.0, Timing::NextBar);
    r.run_to_bar(2);
    r.e.automate(0, t(0), 0.3);
    r.run(3);
    assert_eq!(r.value(0), 0.9);
}

#[test]
fn stop_freezes_run_resumes_restart_finishes() {
    let mut r = Rig::new();
    r.launch(&[(0, 1.0)], 16.0, Timing::NextBar);
    r.run_to_bar(0);
    r.run(BAR_BLOCKS / 4);
    r.e.transport(CLOCK, Transport::Stop);
    r.run(2);
    let frozen = r.value(0);
    r.run(BAR_BLOCKS);
    assert_eq!(r.value(0), frozen);
    r.e.transport(CLOCK, Transport::Run);
    r.run(BAR_BLOCKS / 4);
    assert!(r.value(0) > frozen + 0.1);
    r.e.transport(CLOCK, Transport::Restart);
    r.run(4);
    assert_eq!(r.value(0), 1.0, "a restart finishes the morph");
}

#[test]
fn cue_on_a_stopped_clock_jumps() {
    let mut r = Rig::new();
    r.e.transport(CLOCK, Transport::Stop);
    r.run(2);
    r.launch(&[(0, 0.9)], 16.0, Timing::NextBar);
    r.run(2);
    assert_eq!(r.value(0), 0.9);
}

#[test]
fn tempo_change_stretches_the_morph() {
    let mut r = Rig::new();
    r.launch(&[(0, 1.0)], 16.0, Timing::NextBar);
    r.run_to_bar(0);
    r.run(BAR_BLOCKS / 4);
    let s = ParamSet {
        rev: 1,
        target: RuntimeTarget::Param {
            id: CLOCK,
            kind: "clock",
            index: 0,
        },
        value: 60.0,
    };
    r.e.set(&s);
    // Doubled pulse length: 3/4 of the bar is left, 1.5 bars of time.
    r.run(BAR_BLOCKS);
    assert!(r.value(0) < 1.0);
    r.run(BAR_BLOCKS / 2 + 30);
    assert_eq!(r.value(0), 1.0);
}

#[test]
fn a_morph_survives_a_recompile_and_holds_after_it_ends() {
    let mut r = Rig::new();
    r.launch(&[(0, 0.9)], 0.0, Timing::NextBar);
    r.run_to_bar(2);
    // An edit elsewhere compiles a new graph with the document's macro value (0.2).
    let mut p = patch();
    p.modules.insert(99, module("gain", &[("gain", 0.5)]));
    let g = kabl_engine::compile::compile(&p, SR, 8).unwrap();
    let mut g = basedrop::Owned::new(&r._c.handle(), g);
    g.rev = 5;
    r.e.receive_swap(g);
    r.run(200);
    assert_eq!(r.value(0), 0.9);
    assert!((heard(&mut r) - 0.9).abs() < 1e-3);
}

#[test]
fn a_steady_host_value_does_not_fight_a_morph_but_a_new_one_wins() {
    let mut r = Rig::new();
    r.e.automate(0, t(0), 0.3);
    r.run(2);
    r.launch(&[(0, 1.0)], 16.0, Timing::NextBar);
    r.run_to_bar(0);
    // The host sends its unchanged lane value every block, through a recompile too.
    for b in 0..BAR_BLOCKS / 2 {
        r.e.automate(0, t(0), 0.3);
        if b == 100 {
            let g = kabl_engine::compile::compile(&patch(), SR, 8).unwrap();
            let mut g = basedrop::Owned::new(&r._c.handle(), g);
            g.rev = 7;
            r.e.receive_swap(g);
        }
        r.run(1);
    }
    let v = r.value(0);
    assert!(v > 0.5 && v < 0.7, "the morph went on: {v}");
    r.e.automate(0, t(0), 0.45);
    r.run(BAR_BLOCKS);
    assert_eq!(r.value(0), 0.45, "a new host value ends it");
}
