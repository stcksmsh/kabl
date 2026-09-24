//! Queued bank launches through the real compiler and `PatchEngine`: boundaries, divided
//! clocks, replace/cancel, and the transport/swap/Load rules of
//! docs/composition-batch/design.md. Every audio-thread call runs under `assert_no_alloc`.

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::{Collector, Handle};
use kabl_core::{ModuleId, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::{Launch, PatchEngine, Timing};
use kabl_modules::builtins::seq::{param_index, BANKS, STEPS};
use kabl_modules::builtins::Transport;

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;
const CLOCK: ModuleId = 1;
const SEQ_A: ModuleId = 2;
const DIV: ModuleId = 3;
const SEQ_B: ModuleId = 4;
const OUT: ModuleId = 5;
/// 16ths at 125 bpm: 5760 samples, 90 blocks, so ticks land on block starts; 131 bpm puts them
/// mid-block.
const BPM: f32 = 131.0;

fn module(kind: &str, params: &[(&str, f32)]) -> ModuleState {
    ModuleState {
        kind: kind.into(),
        pos: Vec2::default(),
        params: params.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
    }
}

/// clock → seq A (direct); clock → /3 divider → seq B; `clock.reset` → divider and both
/// sequencers. Left out = A's pitch (or gate), right = B's pitch. Bank b step k plays 10b + k.
fn patch(left: &str) -> PatchState {
    let mut p = PatchState::new();
    let mut seq = module("seq", &[]);
    for b in 0..BANKS {
        for k in 0..STEPS {
            let name = kabl_modules::builtins::SEQ_INFO.params[param_index(b, k)].name;
            seq.params.insert(name.into(), (10 * b + k) as f32);
        }
    }
    p.modules.insert(CLOCK, module("clock", &[("bpm", BPM)]));
    p.modules.insert(SEQ_A, seq.clone());
    p.modules.insert(DIV, module("clock.div", &[("div", 3.0)]));
    p.modules.insert(SEQ_B, seq);
    p.modules.insert(OUT, module("out", &[]));
    let m = |id, port: &str| PortRef::Module {
        id,
        port: port.into(),
    };
    let cables = [
        (m(CLOCK, "gate"), m(SEQ_A, "clock")),
        (m(CLOCK, "gate"), m(DIV, "clock")),
        (m(DIV, "gate"), m(SEQ_B, "clock")),
        (m(CLOCK, "reset"), m(DIV, "reset")),
        (m(CLOCK, "reset"), m(SEQ_A, "reset")),
        (m(CLOCK, "reset"), m(SEQ_B, "reset")),
        (m(SEQ_A, left), m(OUT, "left")),
        (m(SEQ_B, "pitch"), m(OUT, "right")),
    ];
    for (i, (from, to)) in cables.into_iter().enumerate() {
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

#[allow(clippy::large_enum_variant)]
enum Ev {
    Launch(Launch),
    Cancel(Option<ModuleId>),
    Transport(Transport),
    Swap(PatchState, bool),
}

struct Run {
    /// Per sample: (left, right).
    out: Vec<(f32, f32)>,
    /// Per block: queued bank of A and B as the engine reports it.
    queued: Vec<(Option<usize>, Option<usize>)>,
}

fn run(p: &PatchState, blocks: usize, events: Vec<(usize, Ev)>) -> Run {
    let collector = Collector::new();
    let handle: Handle = collector.handle();
    let mut e = PatchEngine::new(&handle, p, SR, 8).unwrap();
    let mut events: Vec<(usize, Ev, Option<_>)> = events
        .into_iter()
        .map(|(b, ev)| {
            let g = match &ev {
                Ev::Swap(p, fresh) => {
                    let mut g = e.build_swap(&handle, p).unwrap();
                    g.fresh = *fresh;
                    Some(g)
                }
                _ => None,
            };
            (b, ev, g)
        })
        .collect();
    let mut r = Run {
        out: Vec::with_capacity(blocks * BLOCK),
        queued: Vec::with_capacity(blocks),
    };
    let (mut l, mut rt) = ([0f32; BLOCK], [0f32; BLOCK]);
    for b in 0..blocks {
        let mut q = (None, None);
        assert_no_alloc(|| {
            for (at, ev, g) in events.iter_mut() {
                if *at != b {
                    continue;
                }
                match ev {
                    Ev::Launch(x) => e.launch(x),
                    Ev::Cancel(s) => e.cancel(*s),
                    Ev::Transport(t) => e.transport(CLOCK, *t),
                    Ev::Swap(..) => e.receive_swap(g.take().unwrap()),
                }
            }
            e.process_block(&mut l, &mut rt);
            e.seqs(|id, _, _, queued| match id {
                SEQ_A => q.0 = queued,
                SEQ_B => q.1 = queued,
                _ => {}
            });
        });
        r.queued.push(q);
        r.out.extend(l.iter().zip(&rt).map(|(&a, &b)| (a, b)));
    }
    r
}

/// (sample, value) wherever `f` of the output changes to another whole number (values between
/// whole numbers are a crossfade's blend, and rounding of identical graphs is ignored).
fn changes(r: &Run, f: impl Fn(&(f32, f32)) -> f32) -> Vec<(usize, f32)> {
    let mut v = Vec::new();
    let mut last = f32::NAN;
    for (i, s) in r.out.iter().enumerate() {
        let x = f(s);
        if (x - x.round()).abs() > 1e-3 {
            continue;
        }
        let x = x.round();
        if x != last {
            v.push((i, x));
            last = x;
        }
    }
    v
}

fn step_len() -> f64 {
    SR as f64 * 60.0 / BPM as f64 / 4.0
}

/// Sample at which clock tick `n` starts (the first tick starts at sample 0).
fn tick(n: usize) -> usize {
    (n as f64 * step_len()).ceil() as usize
}

fn block_of(sample: usize) -> usize {
    sample / BLOCK
}

fn launch(timing: Timing, bank: u8) -> Ev {
    Ev::Launch(Launch::new(CLOCK, timing, &[(SEQ_A, bank), (SEQ_B, bank)]))
}

#[test]
fn next_bar_starts_each_sequencer_on_its_first_edge_at_or_after_the_bar() {
    let r = run(
        &patch("pitch"),
        block_of(tick(24)),
        vec![(block_of(tick(5)), launch(Timing::NextBar, 1))],
    );
    let a = changes(&r, |s| s.0);
    // A plays A1..A8, A1..A8, then B from the bar (tick 16), at the exact sample.
    let first_b = a.iter().find(|c| c.1 >= 10.0).unwrap();
    assert_eq!(first_b.1, 10.0);
    assert!(
        first_b.0.abs_diff(tick(16)) <= 1,
        "{} vs {}",
        first_b.0,
        tick(16)
    );
    // The /3 sequencer ticks on 0, 3, ..., 15, 18: its first edge after the bar is tick 18.
    let b = changes(&r, |s| s.1);
    let first_b = b.iter().find(|c| c.1 >= 10.0).unwrap();
    assert_eq!(first_b.1, 10.0);
    assert!(
        first_b.0.abs_diff(tick(18)) <= 1,
        "{} vs {}",
        first_b.0,
        tick(18)
    );
    // Queued from the command until the switch, then nothing.
    assert_eq!(r.queued[block_of(tick(5)) + 1], (Some(1), Some(1)));
    assert_eq!(r.queued[block_of(tick(17))], (None, Some(1)));
    assert_eq!(*r.queued.last().unwrap(), (None, None));
}

#[test]
fn next_step_and_now() {
    let r = run(
        &patch("pitch"),
        block_of(tick(12)),
        vec![(block_of(tick(5)) + 3, launch(Timing::NextStep, 2))],
    );
    let a = changes(&r, |s| s.0);
    let first = a.iter().find(|c| c.1 >= 20.0).unwrap();
    assert!(first.0.abs_diff(tick(6)) <= 1);
    // Now: the next edge of each sequencer.
    let r = run(
        &patch("pitch"),
        block_of(tick(12)),
        vec![(block_of(tick(5)) + 3, launch(Timing::Now, 3))],
    );
    let a = changes(&r, |s| s.0);
    assert!(a.iter().find(|c| c.1 >= 30.0).unwrap().0.abs_diff(tick(6)) <= 1);
    let b = changes(&r, |s| s.1);
    assert!(b.iter().find(|c| c.1 >= 30.0).unwrap().0.abs_diff(tick(6)) <= 1);
}

#[test]
fn a_new_launch_replaces_the_pending_one_and_cancel_drops_it() {
    let r = run(
        &patch("pitch"),
        block_of(tick(20)),
        vec![
            (block_of(tick(2)), launch(Timing::NextBar, 1)),
            (block_of(tick(4)), launch(Timing::NextBar, 3)),
        ],
    );
    let a = changes(&r, |s| s.0);
    assert!(
        !a.iter().any(|c| (10.0..20.0).contains(&c.1)),
        "B never plays"
    );
    assert!(a.iter().any(|c| c.1 == 30.0));

    let r = run(
        &patch("pitch"),
        block_of(tick(20)),
        vec![
            (block_of(tick(2)), launch(Timing::NextBar, 1)),
            (block_of(tick(4)), Ev::Cancel(Some(SEQ_A))),
        ],
    );
    assert!(changes(&r, |s| s.0).iter().all(|c| c.1 < 10.0));
    assert!(changes(&r, |s| s.1).iter().any(|c| c.1 == 10.0));
}

#[test]
fn stop_turns_a_pending_launch_into_a_selection_played_from_run() {
    let stop = block_of(tick(6)) + 10;
    let go = stop + 300;
    let r = run(
        &patch("gate"),
        go + 400,
        vec![
            (block_of(tick(4)), launch(Timing::NextBar, 2)),
            (stop, Ev::Transport(Transport::Stop)),
            (go, Ev::Transport(Transport::Run)),
        ],
    );
    // Stopped: armed, no gate.
    assert_eq!(r.queued[stop + 1], (Some(2), Some(2)));
    assert!(r.out[(stop + 2) * BLOCK..go * BLOCK]
        .iter()
        .all(|s| s.0 == 0.0));
    // Run: A's gate opens at once on the new pulse (C1); B holds its pitch until its divider
    // passes a pulse, which then plays C1.
    assert_eq!(r.queued[go].0, None);
    assert!(r.out[go * BLOCK].0 > 0.5);
    let b = changes(&r, |s| s.1);
    let c1 = b.iter().find(|c| c.1 >= 20.0).unwrap();
    assert!(c1.0 >= go * BLOCK && c1.1 == 20.0, "{c1:?}");
    assert!(b.iter().all(|c| c.0 <= stop * BLOCK || c.0 >= go * BLOCK));
    assert_eq!(*r.queued.last().unwrap(), (None, None));
}

#[test]
fn a_launch_while_stopped_selects_without_a_note() {
    let stop = 50;
    let r = run(
        &patch("gate"),
        stop + 700,
        vec![
            (stop, Ev::Transport(Transport::Stop)),
            (stop + 100, launch(Timing::NextBar, 3)),
            (stop + 300, Ev::Transport(Transport::Run)),
        ],
    );
    assert!(r.out[(stop + 1) * BLOCK..(stop + 300) * BLOCK]
        .iter()
        .all(|s| s.0 == 0.0));
    assert_eq!(r.queued[stop + 100], (Some(3), Some(3)));
    assert!(r.out[(stop + 300) * BLOCK].0 > 0.5, "A's first edge on Run");
    let b = changes(&r, |s| s.1);
    let d1 = b.iter().find(|c| c.1 >= 30.0).unwrap();
    assert!(d1.0 >= (stop + 300) * BLOCK && d1.1 == 30.0);
}

#[test]
fn restart_lands_a_pending_launch_on_the_restart_pulse() {
    let at = block_of(tick(7)) + 20;
    let r = run(
        &patch("pitch"),
        at + 200,
        vec![
            (block_of(tick(3)), launch(Timing::NextBar, 1)),
            (at, Ev::Transport(Transport::Restart)),
        ],
    );
    let a = changes(&r, |s| s.0);
    let b1 = a.iter().find(|c| c.1 >= 10.0).unwrap();
    // The restart pulse starts one sample after the command (the gate was high) or at once.
    assert!(
        b1.0 >= at * BLOCK && b1.0 <= at * BLOCK + 1,
        "{b1:?} at {}",
        at * BLOCK
    );
    assert_eq!(b1.1, 10.0);
    let b = changes(&r, |s| s.1);
    assert_eq!(b.iter().find(|c| c.1 >= 10.0).unwrap().0, b1.0);
}

#[test]
fn a_live_swap_keeps_the_pending_launch_and_never_replays_it() {
    let p = patch("pitch");
    let mut edited = p.clone();
    // An unrelated edit (inactive bank D) mid-wait, and another after the switch.
    edited
        .modules
        .get_mut(&SEQ_A)
        .unwrap()
        .params
        .insert("d.p1".into(), 5.0);
    let r = run(
        &p,
        block_of(tick(40)),
        vec![
            (block_of(tick(3)), launch(Timing::NextBar, 1)),
            (block_of(tick(9)), Ev::Swap(edited.clone(), false)),
            (block_of(tick(20)), Ev::Swap(p.clone(), false)),
        ],
    );
    let a = changes(&r, |s| s.0);
    // B from the bar on, looping 10..17 with no second restart at tick 32.
    let from = a.iter().position(|c| c.1 == 10.0).unwrap();
    assert!(a[from].0.abs_diff(tick(16)) <= 1);
    let after: Vec<f32> = a[from..].iter().map(|c| c.1).collect();
    let want: Vec<f32> = (0..after.len()).map(|k| 10.0 + (k % 8) as f32).collect();
    assert_eq!(after, want);
}

#[test]
fn load_drops_pending_launches_and_starts_on_the_startup_bank() {
    let mut loaded = patch("pitch");
    loaded
        .modules
        .get_mut(&SEQ_A)
        .unwrap()
        .params
        .insert("bank".into(), 2.0);
    let r = run(
        &patch("pitch"),
        block_of(tick(30)),
        vec![
            (block_of(tick(3)), launch(Timing::NextBar, 1)),
            (block_of(tick(6)), Ev::Swap(loaded, true)),
        ],
    );
    assert_eq!(r.queued[block_of(tick(6)) + 1], (None, None));
    // After the 15 ms crossfade: C, the loaded startup bank, never B.
    let settled = (block_of(tick(6)) + 13) * BLOCK;
    let a: Vec<_> = changes(&r, |s| s.0)
        .into_iter()
        .filter(|c| c.0 >= settled)
        .collect();
    assert!(a.iter().all(|c| (20.0..30.0).contains(&c.1)), "{a:?}");
    assert!(a.iter().any(|c| c.1 == 20.0));
}

#[test]
fn deleting_the_reference_clock_drops_its_launches() {
    let mut no_clock = patch("pitch");
    no_clock.modules.remove(&CLOCK);
    no_clock.cables.retain(|_, c| c.from.module_id() != CLOCK);
    let r = run(
        &patch("pitch"),
        block_of(tick(10)),
        vec![
            (block_of(tick(3)), launch(Timing::NextBar, 1)),
            (block_of(tick(5)), Ev::Swap(no_clock, false)),
        ],
    );
    assert_eq!(*r.queued.last().unwrap(), (None, None));
}

#[test]
fn every_block_boundary_offset_lands_on_the_bar_sample() {
    // Tempos that put tick 16 at many offsets within a block.
    for bpm in [100.0f32, 117.0, 121.0, 131.0, 143.0, 157.0] {
        let mut p = patch("pitch");
        p.modules
            .get_mut(&CLOCK)
            .unwrap()
            .params
            .insert("bpm".into(), bpm);
        let step = SR as f64 * 60.0 / bpm as f64 / 4.0;
        let t16 = (16.0 * step).ceil() as usize;
        let r = run(&p, t16 / BLOCK + 20, vec![(3, launch(Timing::NextBar, 1))]);
        let a = changes(&r, |s| s.0);
        let first = a.iter().find(|c| c.1 == 10.0).unwrap();
        assert!(
            first.0.abs_diff(t16) <= 1,
            "{bpm} bpm: {} vs {t16}",
            first.0
        );
        assert!(a.iter().filter(|c| c.0 < first.0).all(|c| c.1 < 10.0));
    }
}
