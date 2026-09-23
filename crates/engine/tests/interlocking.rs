//! `patches/interlocking` through the real compiler and `PatchEngine`: output headroom, both
//! layers audible, and musical position kept through transport commands and live graph swaps.

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::Collector;
use kabl_core::{ModuleId, PatchState};
use kabl_engine::compile::compile;
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::PatchEngine;
use kabl_modules::builtins::Transport;

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;
const VOICES: usize = 8;
const CLOCK: ModuleId = 1;
const BASS: ModuleId = 3;
const LEAD: ModuleId = 4;
const MIXER: ModuleId = 6;

fn patch() -> PatchState {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/interlocking");
    kabl_core::load(&dir)
        .expect("interlocking patch")
        .state()
        .clone()
}

fn with(mut p: PatchState, id: ModuleId, param: &str, v: f32) -> PatchState {
    p.modules
        .get_mut(&id)
        .unwrap()
        .params
        .insert(param.into(), v);
    p
}

/// Peak and RMS of the left output over `seconds`.
fn render(p: &PatchState, seconds: f32) -> (f32, f32) {
    let mut c = compile(p, SR, VOICES).expect("compiles");
    let (mut peak, mut sum, mut n) = (0f32, 0f64, 0usize);
    while (n as f32) < seconds * SR {
        c.process_block();
        for &s in c.left() {
            peak = peak.max(s.abs());
            sum += (s as f64).powi(2);
        }
        n += BLOCK;
    }
    (peak, (sum / n as f64).sqrt() as f32)
}

fn db(v: f32) -> f32 {
    20.0 * v.log10()
}

/// 20 s covers two full cycles of the 7-against-5 pattern (70 sixteenths, about 9 s at 116 bpm).
#[test]
fn both_layers_are_audible_and_the_mix_has_headroom() {
    let (peak, rms) = render(&patch(), 20.0);
    let (_, bass) = render(&with(patch(), MIXER, "level2", 0.0), 20.0);
    let (_, lead) = render(&with(patch(), MIXER, "level1", 0.0), 20.0);
    println!(
        "mix peak {:.1} dBFS, rms {:.1} dBFS; bass alone rms {:.1}, lead alone rms {:.1}",
        db(peak),
        db(rms),
        db(bass),
        db(lead)
    );
    assert!(peak < 0.5, "peak {:.1} dBFS, want below -6", db(peak));
    assert!(db(bass) > -40.0 && db(lead) > -40.0, "both layers audible");
    assert!(
        (db(bass) - db(lead)).abs() < 10.0,
        "neither layer buries the other"
    );
}

/// Per block: (bass step, lead step, clock running) as the engine reports them.
type Trace = Vec<(usize, usize, bool)>;

fn trace(e: &PatchEngine) -> (usize, usize, bool) {
    let (mut b, mut l, mut r) = (usize::MAX, usize::MAX, false);
    e.seq_steps(|id, s| match id {
        BASS => b = s,
        LEAD => l = s,
        _ => {}
    });
    e.clocks(|id, run| {
        if id == CLOCK {
            r = run
        }
    });
    (b, l, r)
}

/// Runs `blocks` blocks, sending `cmds` and swapping in `swaps` at their block indices. Every
/// audio-thread call runs under `assert_no_alloc`.
fn run(blocks: usize, cmds: &[(usize, Transport)], swaps: &[(usize, PatchState)]) -> Trace {
    let collector = Collector::new();
    let handle = collector.handle();
    let mut e = PatchEngine::new(&handle, &patch(), SR, VOICES).unwrap();
    let mut out = Vec::with_capacity(blocks);
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    for b in 0..blocks {
        let swap = swaps
            .iter()
            .find(|(at, _)| *at == b)
            .map(|(_, p)| e.build_swap(&handle, p).unwrap());
        let mut t = (0, 0, false);
        assert_no_alloc(|| {
            if let Some(g) = swap {
                e.receive_swap(g);
            }
            for (_, c) in cmds.iter().filter(|(at, _)| *at == b) {
                e.transport(CLOCK, *c);
            }
            e.process_block(&mut l, &mut r);
            t = trace(&e);
        });
        out.push(t);
    }
    out
}

/// Blocks per 16th at 116 bpm: 48000 * 60 / 116 / 4 / 64 ≈ 97.
const STEP_BLOCKS: usize = 97;

#[test]
fn unrelated_live_edits_keep_the_musical_position() {
    let blocks = 40 * STEP_BLOCKS;
    let reference = run(blocks, &[], &[]);
    // Edits while playing: levels, a transpose, a filter; some overlap (a swap arrives while
    // another is still fading in).
    let swaps = vec![
        (300, with(patch(), MIXER, "level2", 0.05)),
        (301, with(patch(), LEAD, "transpose", 5.0)),
        (900, with(patch(), BASS, "transpose", -12.0)),
        (1500, with(patch(), 9, "cutoff_hz", 900.0)),
        (2000, patch()),
    ];
    assert_eq!(run(blocks, &[], &swaps), reference);
}

#[test]
fn bpm_change_keeps_the_step_it_is_on() {
    let blocks = 20 * STEP_BLOCKS;
    let at = 5 * STEP_BLOCKS + 40;
    let t = run(blocks, &[], &[(at, with(patch(), CLOCK, "bpm", 90.0))]);
    let reference = run(at, &[], &[]);
    assert_eq!(t[..at], reference[..], "identical up to the edit");
    // After it: the bass keeps counting from where it was, one step at a time, never back.
    let mut prev = t[at - 1].0;
    for &(b, _, _) in &t[at..] {
        assert!(
            b == prev || b == (prev + 1) % 7,
            "bass jumped {prev} -> {b}"
        );
        prev = b;
    }
}

#[test]
fn transport_through_the_engine_and_through_a_fade() {
    let stop = 3 * STEP_BLOCKS + 20;
    // A swap starts a fade 2 blocks before the Stop, so both graphs must get it.
    let t = run(
        12 * STEP_BLOCKS,
        &[
            (stop, Transport::Stop),
            (stop + 200, Transport::Restart),
            (stop + 400, Transport::Run),
        ],
        &[(stop - 2, with(patch(), MIXER, "level1", 0.25))],
    );
    assert!(t[stop - 1].2 && !t[stop].2, "stopped at once");
    let parked = t[stop];
    assert!(
        t[stop..stop + 400]
            .iter()
            .all(|x| (x.0, x.1) == (parked.0, parked.1) && !x.2),
        "nothing moves while stopped, Restart included"
    );
    assert_eq!(
        (t[stop + 400].0, t[stop + 400].1),
        (0, 0),
        "Run after Restart: step 1"
    );
    assert!(t[stop + 400].2);
    // Then both count on from the shared downbeat: bass every 16th, lead every 8th.
    let b = &t[stop + 400..];
    assert_eq!(b[STEP_BLOCKS + 5].0, 1);
    assert_eq!(b[STEP_BLOCKS + 5].1, 0);
    assert_eq!(b[2 * STEP_BLOCKS + 5].1, 1);
}

/// A Restart already applied never comes back: a later graph swap (the same graph a reload or
/// an undo would build) carries the clock as it is, it doesn't restart it again.
#[test]
fn restart_is_not_replayed_by_a_graph_swap() {
    let restart = 4 * STEP_BLOCKS + 10;
    let later = restart + 3 * STEP_BLOCKS;
    let with_swap = run(
        12 * STEP_BLOCKS,
        &[(restart, Transport::Restart)],
        &[(later, patch())],
    );
    let without = run(12 * STEP_BLOCKS, &[(restart, Transport::Restart)], &[]);
    assert_eq!(with_swap, without);
}
