//! `patches/echo` through the real compiler and `PatchEngine`: headroom, and the delay's echo
//! tail across transport stops, live graph swaps (overlapping ones too), two delays at once,
//! and a fresh Load. Every audio-thread call runs under `assert_no_alloc`.

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::Collector;
use kabl_core::{CableState, ModuleId, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::compile::compile;
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::PatchEngine;
use kabl_modules::builtins::{DelayLock, Transport};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;
const VOICES: usize = 8;
const CLOCK: ModuleId = 1;
const FILTER_A: ModuleId = 9;
const MIXER: ModuleId = 6;
const VCA_A: ModuleId = 11;
const DELAY: ModuleId = 16;
const MIXER_R: ModuleId = 17;

fn patch() -> PatchState {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/echo");
    kabl_core::load(&dir).expect("echo patch").state().clone()
}

fn with(mut p: PatchState, id: ModuleId, param: &str, v: f32) -> PatchState {
    p.modules
        .get_mut(&id)
        .unwrap()
        .params
        .insert(param.into(), v);
    p
}

fn port(id: ModuleId, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

fn db(v: f32) -> f32 {
    20.0 * v.log10()
}

/// Stereo peak and RMS over `seconds`.
fn render(p: &PatchState, seconds: f32) -> (f32, f32) {
    let mut c = compile(p, SR, VOICES).expect("compiles");
    let (mut peak, mut sum, mut n) = (0f32, 0f64, 0usize);
    while (n as f32) < seconds * SR {
        c.process_block();
        for &s in c.left().iter().chain(c.right()) {
            peak = peak.max(s.abs());
            sum += (s as f64).powi(2);
        }
        n += BLOCK;
    }
    (peak, (sum / (2 * n) as f64).sqrt() as f32)
}

#[test]
fn echo_mix_has_headroom_and_no_loudness_jump() {
    let (peak, rms) = render(&patch(), 20.0);
    let (dry_peak, dry) = render(&with(patch(), DELAY, "mix", 0.0), 20.0);
    let (_, bass) = render(
        &with(with(patch(), MIXER, "level2", 0.0), MIXER_R, "level2", 0.0),
        20.0,
    );
    println!(
        "echo: peak {:.1} dBFS, rms {:.1} dBFS; dry (mix 0 %): peak {:.1}, rms {:.1}; bass alone rms {:.1}",
        db(peak),
        db(rms),
        db(dry_peak),
        db(dry),
        db(bass)
    );
    assert!(peak < 0.5, "peak {:.1} dBFS, want below -6", db(peak));
    assert!(
        (db(rms) - db(dry)).abs() < 2.0,
        "dry/echo comparison within 2 dB"
    );
}

/// Runs `blocks` blocks from a fresh engine on `start`, sending transport `cmds` and swapping
/// in `swaps` (patch, fresh) at their block indices. Returns the stereo output.
fn run(
    start: &PatchState,
    blocks: usize,
    cmds: &[(usize, Transport)],
    swaps: &[(usize, PatchState, bool)],
) -> (Vec<f32>, Vec<f32>) {
    let collector = Collector::new();
    let handle = collector.handle();
    let mut e = PatchEngine::new(&handle, start, SR, VOICES).unwrap();
    let (mut outl, mut outr) = (
        Vec::with_capacity(blocks * BLOCK),
        Vec::with_capacity(blocks * BLOCK),
    );
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    for b in 0..blocks {
        let mut incoming: Vec<_> = swaps
            .iter()
            .filter(|(at, _, _)| *at == b)
            .map(|(_, p, fresh)| {
                let mut g = e.build_swap(&handle, p).unwrap();
                g.fresh = *fresh;
                g
            })
            .collect();
        assert_no_alloc(|| {
            for g in incoming.drain(..) {
                e.receive_swap(g);
            }
            for (_, c) in cmds.iter().filter(|(at, _)| *at == b) {
                e.transport(CLOCK, *c);
            }
            e.process_block(&mut l, &mut r);
        });
        outl.extend_from_slice(&l);
        outr.extend_from_slice(&r);
    }
    (outl, outr)
}

fn rms(v: &[f32]) -> f32 {
    (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt()
}

fn max_diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f32::max)
}

/// Blocks per second.
const SEC: usize = 750;

#[test]
fn echo_tail_survives_stop_and_live_edits() {
    // Bass muted, so the bass cutoff edit below is truly unrelated to what is heard (its own
    // release is still ringing 100 blocks after the stop).
    let p = with(with(patch(), MIXER, "level1", 0.0), MIXER_R, "level1", 0.0);
    let stop = 4 * SEC;
    let blocks = stop + 2 * SEC;
    let cmds = [(stop, Transport::Stop)];
    let (rl, rr) = run(&p, blocks, &cmds, &[]);

    // After the stop the lead's envelopes release, and only echoes remain: they decay but are
    // still clearly there a second later.
    let s = |b: usize| b * BLOCK;
    let tail = rms(&rr[s(stop + SEC)..s(stop + SEC + SEC / 4)]);
    assert!(db(tail) > -55.0, "tail after 1 s: {:.1} dBFS", db(tail));
    assert!(db(tail) < db(rms(&rr[s(stop - SEC)..s(stop)])));

    // The same run with graph swaps: the same patch (an undo/redo or a no-op rebuild), an
    // unrelated edit (the bass cutoff) during the tail, and three overlapping swaps.
    let swaps = [
        (stop - 300, p.clone(), false),
        (
            stop + 100,
            with(p.clone(), FILTER_A, "cutoff_hz", 900.0),
            false,
        ),
        (stop + 400, p.clone(), false),
        (stop + 400, p.clone(), false),
        (
            stop + 401,
            with(p.clone(), FILTER_A, "cutoff_hz", 600.0),
            false,
        ),
    ];
    let (l, r) = run(&p, blocks, &cmds, &swaps);
    let d = max_diff(&l, &rl).max(max_diff(&r, &rr));
    assert!(d < 1e-4, "swapped run differs from the reference by {d}");
}

#[test]
fn a_delay_edit_keeps_the_tail_and_two_delays_stay_apart() {
    // A second delay, fully wet and slower, on the bass into the left mixer only.
    let mut p = patch();
    let d2: ModuleId = 100;
    p.modules.insert(
        d2,
        ModuleState {
            kind: "delay".into(),
            pos: Vec2 { x: 0.0, y: 0.0 },
            params: [
                ("sync".to_string(), 0.0),
                ("time_ms".to_string(), 700.0),
                ("feedback".to_string(), 60.0),
                ("mix".to_string(), 100.0),
            ]
            .into(),
        },
    );
    let to_in1 = *p
        .cables
        .iter()
        .find(|(_, c)| c.to == port(MIXER, "in1"))
        .unwrap()
        .0;
    p.cables.get_mut(&to_in1).unwrap().from = port(d2, "left");
    let cable = |from, to| CableState {
        from,
        to,
        params: Default::default(),
        steps: vec![],
    };
    p.cables
        .insert(1000, cable(port(VCA_A, "out"), port(d2, "in")));

    let stop = 3 * SEC;
    let blocks = stop + 2 * SEC;
    let cmds = [(stop, Transport::Stop)];
    let (rl, rr) = run(&p, blocks, &cmds, &[]);
    let swaps: Vec<_> = (0..6)
        .map(|k| (stop + 50 + k * 60, p.clone(), false))
        .collect();
    let (l, r) = run(&p, blocks, &cmds, &swaps);
    assert!(
        max_diff(&l, &rl).max(max_diff(&r, &rr)) < 1e-4,
        "each delay keeps its own history"
    );

    // Raising the lead delay's feedback during the tail: the tail goes on (more of it), with
    // no gap at the swap.
    let at = stop + SEC / 2;
    let (_, r) = run(
        &p,
        blocks,
        &cmds,
        &[(at, with(p.clone(), DELAY, "feedback", 70.0), false)],
    );
    let s = |b: usize| b * BLOCK;
    let before = rms(&r[s(at - 40)..s(at)]);
    let during = rms(&r[s(at)..s(at + 12)]);
    assert!(
        during > 0.5 * before,
        "no dip at the swap: {before} → {during}"
    );
    assert!(rms(&r[s(at + SEC / 2)..s(at + SEC)]) > rms(&rr[s(at + SEC / 2)..s(at + SEC)]));
}

#[test]
fn a_fresh_load_starts_with_empty_delay_lines() {
    let stop = 3 * SEC;
    let at = stop + SEC / 4;
    let blocks = at + SEC;
    let cmds = [(stop, Transport::Stop)];
    let (_, r) = run(&patch(), blocks, &cmds, &[(at, patch(), true)]);
    // The loaded graph starts like a startup load: running from step 1, empty delay lines. After
    // the crossfade the output is exactly a fresh engine's from its first block.
    let (_, fresh) = run(&patch(), blocks - at, &[], &[]);
    let fade = 12; // blocks: 15 ms crossfade
    let s = |b: usize| b * BLOCK;
    assert_eq!(&r[s(at + fade)..], &fresh[s(fade)..]);

    // The same swap as a live edit (not fresh) keeps the stopped clock and the tail instead.
    let (_, live) = run(&patch(), blocks, &cmds, &[(at, patch(), false)]);
    let (_, reference) = run(&patch(), blocks, &cmds, &[]);
    assert!(max_diff(&live, &reference) < 1e-4);
}

#[test]
fn the_engine_reports_the_lock_state() {
    let collector = Collector::new();
    let handle = collector.handle();
    let mut e = PatchEngine::new(&handle, &patch(), SR, VOICES).unwrap();
    let status = |e: &PatchEngine| {
        let mut s = None;
        e.delays(|id, lock, ms| {
            if id == DELAY {
                s = Some((lock, ms));
            }
        });
        s.unwrap()
    };
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    e.process_block(&mut l, &mut r);
    // Free time (420 ms, the LFO route moves it a little) until locked.
    let (lock, ms) = status(&e);
    assert_eq!(lock, DelayLock::Unlocked);
    assert!((380.0..470.0).contains(&ms), "{ms}");
    for _ in 0..SEC / 2 {
        e.process_block(&mut l, &mut r);
    }
    // 116 bpm: a 16th is 6207 samples (rounded per pulse); dotted eighth = 3 of them.
    let (lock, ms) = status(&e);
    assert_eq!(lock, DelayLock::Locked);
    assert!((ms - 3.0 * 60_000.0 / 116.0 / 4.0).abs() < 0.1, "{ms} ms");
    e.transport(CLOCK, Transport::Stop);
    for _ in 0..SEC / 2 {
        e.process_block(&mut l, &mut r);
    }
    assert_eq!(status(&e).0, DelayLock::Held);
}

/// Saved patches hold configuration only: the committed demo is a few kilobytes of op log.
#[test]
fn saved_patch_has_no_audio_history() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/echo");
    let bytes: u64 = std::fs::read_dir(&dir)
        .unwrap()
        .map(|f| f.unwrap().metadata().unwrap().len())
        .sum();
    assert!(bytes < 64 * 1024, "{bytes} bytes");
}
