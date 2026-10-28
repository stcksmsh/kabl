//! A cue launch can switch a `drum.grid` pattern like it switches a `seq` bank: on the
//! reference clock's next bar (or step), on the grid's first clock edge at or after that
//! boundary. A grid the cue does not name keeps its pattern; a new `pattern` param value
//! (the face's tabs, automation) takes the grid back. Audio-thread calls run under
//! `assert_no_alloc`.

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::Collector;
use kabl_core::{ModuleId, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::{Command, Launch, PatchEngine, Timing};
use kabl_engine::runtime::{ParamSet, RuntimeTarget};
use kabl_modules::builtins::{make_code, pattern_index, with_code};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;
const CLOCK: ModuleId = 1;
const GRID: ModuleId = 2;
const OUT: ModuleId = 3;
const OTHER: ModuleId = 4;
/// 120 bpm: a 16th is 6000 samples, a bar 96000.
const STEP: usize = 6000;
const BAR: usize = 16 * STEP;

fn module(kind: &str, params: &[(String, f32)]) -> ModuleState {
    ModuleState {
        kind: kind.into(),
        pos: Vec2::default(),
        params: params.iter().cloned().collect(),
    }
}

/// Every step of row `row` (1 to 8) of pattern `pat` ("a" to "d") on at full velocity.
fn full_row(pat: &str, row: usize) -> Vec<(String, f32)> {
    let chunk = (0..4).fold(0.0, |c, s| with_code(c, s, make_code(3, 0)));
    (1..=4).map(|c| (format!("{pat}{row}{c}"), chunk)).collect()
}

/// Pattern A silent in every row but `keep`: the default beat would put a snare in A.
fn only_row(keep: usize) -> Vec<(String, f32)> {
    let mut v: Vec<_> = (1..=8)
        .filter(|&r| r != keep)
        .flat_map(|r| (1..=4).map(move |c| (format!("a{r}{c}"), 0.0)))
        .collect();
    v.extend(full_row("a", keep));
    v
}

/// clock, a grid with kick on every step in A, snare on every step in B, and a second grid
/// the cue never names. kick goes to the left output, snare to the right.
fn patch() -> PatchState {
    let mut p = PatchState::new();
    p.modules
        .insert(CLOCK, module("clock", &[("bpm".into(), 120.0)]));
    let mut params = only_row(1);
    params.extend(full_row("b", 2));
    p.modules.insert(GRID, module("drum.grid", &params));
    let mut other = only_row(1);
    other.extend(full_row("b", 2));
    p.modules.insert(OTHER, module("drum.grid", &other));
    p.modules.insert(OUT, module("out", &[]));
    let m = |id, port: &str| PortRef::Module {
        id,
        port: port.into(),
    };
    for (i, (from, to)) in [
        (m(CLOCK, "gate"), m(GRID, "clock")),
        (m(CLOCK, "gate"), m(OTHER, "clock")),
        (m(GRID, "kick"), m(OUT, "in")),
        (m(GRID, "snare"), m(OUT, "in")),
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
    left: Vec<f32>,
    right: Vec<f32>,
}

impl Rig {
    fn new() -> Self {
        let c = Collector::new();
        let e = PatchEngine::new(&c.handle(), &patch(), SR, 8).unwrap();
        Rig {
            e,
            _c: c,
            left: Vec::new(),
            right: Vec::new(),
        }
    }

    /// Runs until `samples` have been rendered in total.
    fn run_to(&mut self, samples: usize) {
        let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
        while self.left.len() < samples {
            assert_no_alloc(|| self.e.process_block(&mut l, &mut r));
            self.left.extend_from_slice(&l);
            self.right.extend_from_slice(&r);
        }
    }

    fn launch(&mut self, timing: Timing, targets: &[(ModuleId, u8)]) {
        let l = Launch::new(CLOCK, timing, targets);
        assert_no_alloc(|| self.e.command(&Command::Launch(l)));
    }
}

fn first_above(x: &[f32]) -> Option<usize> {
    x.iter().position(|&v| v > 0.1)
}

fn last_above(x: &[f32]) -> Option<usize> {
    x.iter().rposition(|&v| v > 0.1)
}

/// Kick before the boundary, snare from it on; nothing cut mid-bar.
#[test]
fn next_bar_switches_on_the_bar_line() {
    let mut r = Rig::new();
    r.run_to(BAR / 2 + 300);
    r.launch(Timing::NextBar, &[(GRID, 1)]);
    r.run_to(2 * BAR);
    let snare = first_above(&r.right).expect("pattern B plays");
    assert!(
        snare.abs_diff(BAR) <= 2 * BLOCK,
        "first snare at {snare}, bar line at {BAR}"
    );
    let kick = last_above(&r.left).expect("pattern A played");
    assert!(
        kick < BAR + BLOCK && kick > BAR - 2 * STEP,
        "kick stops at {kick}"
    );
    assert!(
        first_above(&r.right[..BAR - 2 * BLOCK]).is_none(),
        "no snare before the bar line"
    );
}

#[test]
fn next_step_switches_on_the_next_step() {
    let mut r = Rig::new();
    r.run_to(3 * STEP + 1000);
    r.launch(Timing::NextStep, &[(GRID, 1)]);
    r.run_to(8 * STEP);
    let snare = first_above(&r.right).expect("pattern B plays");
    assert!(
        snare.abs_diff(4 * STEP) <= 2 * BLOCK,
        "first snare at {snare}, step line at {}",
        4 * STEP
    );
}

/// A grid the launch does not name keeps its pattern: the cue's grid is on B, the other on A.
#[test]
fn unnamed_grid_keeps_its_pattern() {
    let mut r = Rig::new();
    r.launch(Timing::Now, &[(GRID, 1)]);
    r.run_to(2 * STEP);
    let mut seen = Vec::new();
    r.e.seqs(|id, _, pattern, _| seen.push((id, pattern)));
    seen.sort();
    assert_eq!(seen, vec![(GRID, 1), (OTHER, 0)]);
}

/// A new value on the `pattern` param takes the grid back from the cue.
#[test]
fn hand_takes_the_grid_back() {
    let mut r = Rig::new();
    r.launch(Timing::Now, &[(GRID, 1)]);
    r.run_to(2 * STEP);
    let target = RuntimeTarget::Param {
        id: GRID,
        kind: "drum.grid",
        index: pattern_index() as u16,
    };
    let s = ParamSet {
        rev: 1,
        target,
        value: 2.0,
    };
    assert_no_alloc(|| r.e.set(&s));
    r.run_to(4 * STEP);
    let mut seen = Vec::new();
    r.e.seqs(|id, _, pattern, _| seen.push((id, pattern)));
    assert!(seen.contains(&(GRID, 2)), "{seen:?}");
}

/// A cue queued for the next bar can be dropped before it lands.
#[test]
fn cancel_keeps_the_pattern() {
    let mut r = Rig::new();
    r.run_to(STEP);
    r.launch(Timing::NextBar, &[(GRID, 1)]);
    assert_no_alloc(|| r.e.command(&Command::Cancel(Some(GRID))));
    r.run_to(BAR + 4 * STEP);
    assert!(first_above(&r.right).is_none(), "pattern B never started");
}
