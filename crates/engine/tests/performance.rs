//! `patches/performance` through the real compiler and `PatchEngine`: layer levels and
//! headroom, audible step velocity, the reverb and echo tails across transport stops, live
//! parameter edits and overlapping swaps, separate reverb histories, and a fresh Load. Every
//! audio-thread call runs under `assert_no_alloc`.

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::Collector;
use kabl_core::{CableState, ModuleId, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::compile::{compile, CompiledPatch};
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::PatchEngine;
use kabl_modules::builtins::Transport;

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;
const VOICES: usize = 8;
const CLOCK: ModuleId = 1;
const SEQ_A: ModuleId = 3;
const FILT_A: ModuleId = 6;
const DRY: ModuleId = 26;
const ATMOS: ModuleId = 27;
const ECHO: ModuleId = 28;
const DELAY: ModuleId = 29;
const REVERB: ModuleId = 32;
const MAIN_L: ModuleId = 33;
const MAIN_R: ModuleId = 34;
/// Blocks per second.
const SEC: usize = 750;

fn patch() -> PatchState {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/performance");
    kabl_core::load(&dir)
        .expect("performance patch")
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

fn port(id: ModuleId, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

fn db(v: f32) -> f32 {
    20.0 * v.log10()
}

fn rms(v: &[f32]) -> f32 {
    (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
}

fn peak(v: &[f32]) -> f32 {
    v.iter().fold(0.0, |m, x| m.max(x.abs()))
}

fn max_diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f32::max)
}

/// A short lead phrase (semitones from C4, block it starts), one note at a time on voice 0.
const PHRASE: [(f32, usize); 5] = [
    (4.0, 2 * SEC),
    (7.0, 3 * SEC),
    (11.0, 4 * SEC),
    (9.0, 5 * SEC + SEC / 2),
    (7.0, 6 * SEC),
];

fn play_lead(c: &mut CompiledPatch, b: usize) {
    for &(st, at) in &PHRASE {
        if b == at {
            c.note_off(0);
            c.note_on(0, st, 0.9);
        }
    }
    if b == 7 * SEC {
        c.note_off(0);
    }
}

/// Offline stereo render of `seconds` with the lead phrase.
fn render(p: &PatchState, seconds: f32) -> (Vec<f32>, Vec<f32>) {
    let mut c = compile(p, SR, VOICES).expect("compiles");
    let (mut l, mut r) = (Vec::new(), Vec::new());
    for b in 0..(seconds * SEC as f32) as usize {
        play_lead(&mut c, b);
        c.process_block();
        l.extend_from_slice(c.left());
        r.extend_from_slice(c.right());
    }
    (l, r)
}

fn solo(layer: &str) -> PatchState {
    let mut p = patch();
    for (id, ch, name) in [
        (DRY, "level1", "bass"),
        (ATMOS, "level1", "arp"),
        (ATMOS, "level2", "pad"),
        (ECHO, "level1", "lead"),
    ] {
        if name != layer {
            p = with(p, id, ch, 0.0);
        }
    }
    if layer != "arp" {
        p = with(p, ECHO, "level2", 0.0);
    }
    p
}

#[test]
fn layers_balance_and_the_mix_has_headroom() {
    let (l, r) = render(&patch(), 10.0);
    let all: Vec<f32> = l.iter().chain(&r).copied().collect();
    println!(
        "full mix: peak {:.1} dBFS, rms {:.1} dBFS",
        db(peak(&all)),
        db(rms(&all))
    );
    let mut levels = Vec::new();
    for layer in ["bass", "arp", "pad", "lead"] {
        let (l, r) = render(&solo(layer), 10.0);
        // The lead only plays from 2 s to 7 s.
        let span = if layer == "lead" {
            2 * SEC * BLOCK..8 * SEC * BLOCK
        } else {
            0..l.len()
        };
        let v: Vec<f32> = l[span.clone()].iter().chain(&r[span]).copied().collect();
        println!("{layer} alone: rms {:.1} dBFS", db(rms(&v)));
        levels.push(db(rms(&v)));
    }
    // No layer buries or swamps the others.
    let (lo, hi) = levels
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), &x| (a.min(x), b.max(x)));
    assert!(hi - lo < 12.0, "layers span {:.1} dB", hi - lo);
    assert!(db(peak(&all)) < -3.0, "peak {:.1} dBFS", db(peak(&all)));
    assert!(db(rms(&all)) > -30.0, "rms {:.1} dBFS", db(rms(&all)));
}

/// Peak of each 16th-note step over one bar.
fn step_peaks(p: &PatchState) -> Vec<f32> {
    let (l, _) = render(p, 5.0);
    // 112 bpm: 16th = 60 / 112 / 4 s. From the start of the second bar.
    let step = (SR * 60.0 / 112.0 / 4.0) as usize;
    let start = 16 * step;
    (0..16)
        .map(|k| peak(&l[start + k * step..start + (k + 1) * step]))
        .collect()
}

#[test]
fn step_velocity_is_audible_and_flat_velocity_is_even() {
    let accented = step_peaks(&solo("bass"));
    let mut flat = solo("bass");
    for k in 1..=8 {
        flat = with(flat, SEQ_A, &format!("v{k}"), 100.0);
    }
    let flat = step_peaks(&flat);
    let spread = |v: &[f32]| {
        let (lo, hi) = v
            .iter()
            .fold((f32::MAX, 0f32), |(a, b), &x| (a.min(x), b.max(x)));
        db(hi / lo)
    };
    println!(
        "bass step peaks: accented spread {:.1} dB, flat {:.1} dB",
        spread(&accented),
        spread(&flat)
    );
    assert!(spread(&accented) > 5.0);
    assert!(spread(&flat) < spread(&accented) - 3.0);
}

/// Runs `blocks` blocks from a fresh engine on `start` with transport `cmds` and `swaps`
/// (block, patch, fresh). Returns the stereo output.
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
            if b == SEC {
                e.note_on(0, 7.0, 0.9);
            }
            if b == 2 * SEC {
                e.note_off(0);
            }
            e.process_block(&mut l, &mut r);
        });
        outl.extend_from_slice(&l);
        outr.extend_from_slice(&r);
    }
    (outl, outr)
}

#[test]
fn reverb_and_echo_tails_survive_stop_and_live_edits() {
    // Bass and the (never-ending) pad muted: after the stop only tails remain, and the bass
    // cutoff edit below is unrelated to what is heard.
    let p = with(with(patch(), DRY, "level1", 0.0), ATMOS, "level2", 0.0);
    let stop = 3 * SEC;
    let blocks = stop + 3 * SEC;
    let cmds = [(stop, Transport::Stop)];
    let (rl, rr) = run(&p, blocks, &cmds, &[]);
    let s = |b: usize| b * BLOCK;
    // Two seconds after the stop the reverb still rings, quieter than before it.
    let tail = rms(&rr[s(stop + 2 * SEC)..s(stop + 2 * SEC + SEC / 4)]);
    println!("tail 2 s after stop: {:.1} dBFS", db(tail));
    assert!(db(tail) > -60.0, "{:.1}", db(tail));
    assert!(db(tail) < db(rms(&rr[s(stop - SEC)..s(stop)])));

    let swaps = [
        (stop - 300, p.clone(), false),
        (
            stop + 100,
            with(p.clone(), FILT_A, "cutoff_hz", 900.0),
            false,
        ),
        (stop + 400, p.clone(), false),
        (stop + 400, p.clone(), false),
        (
            stop + 401,
            with(p.clone(), FILT_A, "cutoff_hz", 600.0),
            false,
        ),
        // A Restart-and-Stop's worth of unrelated edits deep in the tail.
        (
            stop + 2 * SEC,
            with(p.clone(), SEQ_A, "gate_len", 80.0),
            false,
        ),
    ];
    let (l, r) = run(&p, blocks, &cmds, &swaps);
    let d = max_diff(&l, &rl).max(max_diff(&r, &rr));
    assert!(d < 1e-4, "swapped run differs from the reference by {d}");
}

#[test]
fn a_reverb_edit_mid_tail_has_no_dip() {
    let p = with(with(patch(), DRY, "level1", 0.0), ATMOS, "level2", 0.0);
    let stop = 3 * SEC;
    let at = stop + SEC / 2;
    let blocks = at + 2 * SEC;
    let cmds = [(stop, Transport::Stop)];
    let (_, reference) = run(&p, blocks, &cmds, &[]);
    for (param, v) in [("decay_s", 15.0), ("damp_hz", 1500.0), ("mix", 60.0)] {
        let (_, r) = run(
            &p,
            blocks,
            &cmds,
            &[(at, with(p.clone(), REVERB, param, v), false)],
        );
        let s = |b: usize| b * BLOCK;
        let before = rms(&r[s(at - 40)..s(at)]);
        let during = rms(&r[s(at)..s(at + 12)]);
        println!("{param} → {v}: {before:.5} → {during:.5}");
        assert!(during > 0.6 * before, "{param}: dip {before} → {during}");
        assert_eq!(&r[..s(at)], &reference[..s(at)]);
    }
}

#[test]
fn two_reverbs_keep_separate_histories() {
    // A second, fully wet reverb on the dry bass, returned on channel 3 of both mains.
    let mut p = patch();
    let r2: ModuleId = 100;
    p.modules.insert(
        r2,
        ModuleState {
            kind: "reverb".into(),
            pos: Vec2 { x: 0.0, y: 0.0 },
            params: [("mix".to_string(), 100.0), ("decay_s".to_string(), 2.0)].into(),
        },
    );
    let cable = |from, to| CableState {
        from,
        to,
        params: Default::default(),
        steps: vec![],
    };
    p.cables
        .insert(1000, cable(port(DRY, "out"), port(r2, "in_l")));
    p.cables
        .insert(1001, cable(port(DRY, "out"), port(r2, "in_r")));
    p.cables
        .insert(1002, cable(port(r2, "left"), port(MAIN_L, "in3")));
    p.cables
        .insert(1003, cable(port(r2, "right"), port(MAIN_R, "in3")));
    let stop = 2 * SEC;
    let blocks = stop + 2 * SEC;
    let cmds = [(stop, Transport::Stop)];
    let (rl, rr) = run(&p, blocks, &cmds, &[]);
    let swaps: Vec<_> = (0..6)
        .map(|k| (stop + 50 + k * 60, p.clone(), false))
        .collect();
    let (l, r) = run(&p, blocks, &cmds, &swaps);
    assert!(max_diff(&l, &rl).max(max_diff(&r, &rr)) < 1e-4);
}

#[test]
fn a_fresh_load_starts_with_empty_effects() {
    let stop = 2 * SEC;
    let at = stop + SEC / 4;
    let blocks = at + SEC;
    let cmds = [(stop, Transport::Stop)];
    let (_, r) = run(&patch(), blocks, &cmds, &[(at, patch(), true)]);
    let collector = Collector::new();
    let mut e = PatchEngine::new(&collector.handle(), &patch(), SR, VOICES).unwrap();
    let (mut fl, mut fr) = ([0f32; BLOCK], [0f32; BLOCK]);
    let mut fresh = Vec::new();
    for _ in 0..blocks - at {
        e.process_block(&mut fl, &mut fr);
        fresh.extend_from_slice(&fr);
    }
    let fade = 12;
    let s = |b: usize| b * BLOCK;
    assert_eq!(&r[s(at + fade)..], &fresh[s(fade)..]);
}

/// Not a check: 12 s listening clips of the demo (with the lead phrase) to
/// `target/performance-clips/*.wav`, each scaled to the full mix's RMS so loudness doesn't
/// decide the comparison. `cargo test -p kabl-engine --test performance write_listening_clips
/// -- --ignored --nocapture`.
#[test]
#[ignore]
fn write_listening_clips() {
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/performance-clips");
    std::fs::create_dir_all(&dir).unwrap();
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR as u32,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let clips = [
        ("01-full", patch()),
        ("02-no-reverb", with(patch(), REVERB, "mix", 0.0)),
        ("03-no-echo", with(patch(), DELAY, "mix", 0.0)),
        (
            "04-dry",
            with(with(patch(), REVERB, "mix", 0.0), DELAY, "mix", 0.0),
        ),
        ("05-long-dark-reverb", {
            let p = with(patch(), REVERB, "decay_s", 14.0);
            with(p, REVERB, "damp_hz", 2000.0)
        }),
        ("06-clock-gates", {
            let p = with(patch(), SEQ_A, "gate_mode", 0.0);
            with(p, 4, "gate_mode", 0.0)
        }),
        ("07-flat-velocity", {
            let mut p = patch();
            for k in 1..=8 {
                p = with(p, SEQ_A, &format!("v{k}"), 100.0);
                p = with(p, 4, &format!("v{k}"), 100.0);
            }
            p
        }),
    ];
    let target = {
        let (l, r) = render(&clips[0].1, 12.0);
        rms(&[l, r].concat())
    };
    for (name, p) in clips {
        let (l, r) = render(&p, 12.0);
        let g = target / rms(&[l.clone(), r.clone()].concat());
        println!("{name}: gain {:+.1} dB", db(g));
        let mut w = hound::WavWriter::create(dir.join(format!("{name}.wav")), spec).unwrap();
        for (a, b) in l.iter().zip(&r) {
            w.write_sample(a * g).unwrap();
            w.write_sample(b * g).unwrap();
        }
        w.finalize().unwrap();
    }
}
