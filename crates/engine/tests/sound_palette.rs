//! The sound palette modules (`osc.va` with pulse width, unison and hard sync, `noise`,
//! `filter.ladder`, `chorus`, `drive`) and the keyboard's `midi.in` settings through the real
//! compiler and `PatchEngine`: self-swap null tests (single and overlapping swaps, no
//! allocation), effect histories that stay with their own instance, noise streams per voice
//! and the single-instance (lane 0) stream, and the patch lifecycle (save/reload, undo, a
//! fresh Load, old patches unaffected).

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::Collector;
use kabl_core::{CableState, ModuleId, ModuleState, Op, ParamTarget, PatchLog, PatchState};
use kabl_core::{PortRef, Source, Vec2};
use kabl_engine::compile::compile;
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::PatchEngine;

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;
const VOICES: usize = 8;
/// Blocks per second.
const SEC: usize = 750;

const MIDI: ModuleId = 1;
const OSC: ModuleId = 2;
const SYNC_OSC: ModuleId = 3;
const NOISE: ModuleId = 4;
const MIX: ModuleId = 5;
const LADDER: ModuleId = 6;
const ENV: ModuleId = 7;
const VCA: ModuleId = 8;
const DRIVE: ModuleId = 9;
const CHORUS_A: ModuleId = 10;
const CHORUS_B: ModuleId = 11;
const DRIVE_B: ModuleId = 12;
const OUT: ModuleId = 13;

fn port(id: ModuleId, p: &str) -> PortRef {
    PortRef::Module { id, port: p.into() }
}

struct B(PatchState, u64);

impl B {
    fn m(&mut self, id: ModuleId, kind: &str, params: &[(&str, f32)]) -> &mut Self {
        self.0.modules.insert(
            id,
            ModuleState {
                kind: kind.into(),
                pos: Vec2::default(),
                params: params.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
            },
        );
        self
    }
    fn c(&mut self, from: PortRef, to: PortRef) -> &mut Self {
        self.1 += 1;
        self.0.cables.insert(
            self.1,
            CableState {
                from,
                to,
                params: Default::default(),
                steps: vec![],
            },
        );
        self
    }
}

/// Every new module at once: a 3-voice-unison pulse (PW 30 %) hard-synced to a second
/// oscillator, pink noise, a driven resonant ladder, an envelope, a drive per voice, two
/// choruses with different settings (left from one, right from the other through a second,
/// single-instance drive). MONO/LEGATO/glide on the `midi.in`.
fn palette(mode: f32) -> PatchState {
    let mut b = B(PatchState::new(), 0);
    b.m(
        MIDI,
        "midi.in",
        &[("mode", mode), ("glide", 1.0), ("glide_ms", 80.0)],
    )
    .m(
        OSC,
        "osc.va",
        &[
            ("waveform", 3.0),
            ("pw", 30.0),
            ("unison", 3.0),
            ("detune", 25.0),
            ("fine", 7.0),
        ],
    )
    .m(SYNC_OSC, "osc.va", &[("base_hz", 180.0), ("waveform", 0.0)])
    .m(NOISE, "noise", &[("color", 1.0), ("level_db", -18.0)])
    .m(MIX, "mixer", &[("level1", 0.7), ("level2", 0.5)])
    .m(
        LADDER,
        "filter.ladder",
        &[("cutoff_hz", 1400.0), ("resonance", 0.7), ("drive_db", 6.0)],
    )
    .m(
        ENV,
        "env.adsr",
        &[("attack_ms", 5.0), ("release_ms", 400.0)],
    )
    .m(VCA, "vca", &[("gain", 0.0)])
    .m(DRIVE, "drive", &[("drive_db", 12.0), ("mix", 70.0)])
    .m(
        CHORUS_A,
        "chorus",
        &[("rate_hz", 0.4), ("depth", 60.0), ("mix", 50.0)],
    )
    .m(
        CHORUS_B,
        "chorus",
        &[
            ("rate_hz", 1.7),
            ("depth", 90.0),
            ("mix", 80.0),
            ("width", 40.0),
        ],
    )
    .m(DRIVE_B, "drive", &[("drive_db", 18.0), ("trim_db", -6.0)])
    .m(OUT, "out", &[]);
    b.c(port(MIDI, "pitch"), port(OSC, "pitch"))
        .c(port(MIDI, "pitch"), port(SYNC_OSC, "pitch"))
        .c(port(SYNC_OSC, "out"), port(OSC, "sync"))
        .c(port(MIDI, "gate"), port(ENV, "gate"))
        .c(port(OSC, "out"), port(MIX, "in1"))
        .c(port(NOISE, "out"), port(MIX, "in2"))
        .c(port(MIX, "out"), port(LADDER, "in"))
        .c(port(LADDER, "out"), port(VCA, "in"))
        .c(port(ENV, "out"), port(VCA, "cv"))
        .c(port(VCA, "out"), port(DRIVE, "in"))
        .c(port(DRIVE, "out"), port(CHORUS_A, "in_l"))
        .c(port(DRIVE, "out"), port(CHORUS_A, "in_r"))
        .c(port(DRIVE, "out"), port(CHORUS_B, "in_l"))
        .c(port(DRIVE, "out"), port(CHORUS_B, "in_r"))
        .c(port(CHORUS_A, "left"), port(OUT, "left"))
        .c(port(CHORUS_B, "right"), port(DRIVE_B, "in"))
        .c(port(DRIVE_B, "out"), port(OUT, "right"));
    b.0
}

fn with(mut p: PatchState, id: ModuleId, param: &str, v: f32) -> PatchState {
    p.modules
        .get_mut(&id)
        .unwrap()
        .params
        .insert(param.into(), v);
    p
}

/// A phrase: a chord, overlapping legato notes, a release, a new note (block, event).
fn phrase() -> Vec<(usize, KeyEvent)> {
    let on = |note| KeyEvent::On {
        note,
        velocity: 100,
    };
    let off = |note| KeyEvent::Off { note };
    vec![
        (10, on(48)),
        (12, on(55)),
        (14, on(60)),
        (300, on(63)),
        (420, off(48)),
        (500, off(55)),
        (560, off(60)),
        (700, off(63)),
        (900, on(67)),
        (1300, off(67)),
    ]
}

/// Runs a fresh engine on `start` for `blocks`, playing `keys` and installing `swaps` (patch,
/// fresh) at their block indices. Every audio-thread call is under `assert_no_alloc`.
fn run(
    start: &PatchState,
    blocks: usize,
    keys: &[(usize, KeyEvent)],
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
            for (_, k) in keys.iter().filter(|(at, _)| *at == b) {
                e.key(*k);
            }
            e.process_block(&mut l, &mut r);
        });
        outl.extend_from_slice(&l);
        outr.extend_from_slice(&r);
    }
    (outl, outr)
}

fn rms(v: &[f32]) -> f32 {
    (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
}

fn max_diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f32::max)
}

fn db(v: f32) -> f32 {
    20.0 * v.max(1e-9).log10()
}

#[test]
fn self_swaps_are_null_with_every_new_module() {
    for mode in [0.0, 1.0, 2.0] {
        let p = palette(mode);
        let blocks = 2 * SEC;
        let (rl, rr) = run(&p, blocks, &phrase(), &[]);
        assert!(rl.iter().chain(&rr).all(|s| s.is_finite()));
        assert!(
            db(rms(&rl[20 * BLOCK..400 * BLOCK])) > -50.0,
            "the patch plays"
        );
        // Single swaps (mid-chord, mid-glide, mid-release) and bursts of overlapping ones.
        let mut swaps: Vec<_> = [40, 305, 430, 710, 905]
            .into_iter()
            .map(|b| (b, p.clone(), false))
            .collect();
        for b in [150, 151, 152, 600, 600, 601, 1000, 1001, 1003] {
            swaps.push((b, p.clone(), false));
        }
        let (l, r) = run(&p, blocks, &phrase(), &swaps);
        let d = max_diff(&l, &rl).max(max_diff(&r, &rr));
        assert!(d < 1e-4, "mode {mode}: self-swapped run differs by {d}");
    }
}

#[test]
fn effect_histories_stay_with_their_own_instance() {
    // Two choruses and two drives in one patch: removing chorus B's path must leave the left
    // channel (chorus A only) sample-identical, with and without swaps in between, so no
    // instance ever reads another's delay line or ADAA state.
    let p = palette(0.0);
    let mut only_a = p.clone();
    only_a.modules.remove(&CHORUS_B);
    only_a.modules.remove(&DRIVE_B);
    only_a.cables.retain(|_, c| {
        ![c.from.clone(), c.to.clone()]
            .iter()
            .any(|r| matches!(r, PortRef::Module { id, .. } if *id == CHORUS_B || *id == DRIVE_B))
    });
    let blocks = 2 * SEC;
    let (al, _) = run(&only_a, blocks, &phrase(), &[]);
    let (bl, br) = run(&p, blocks, &phrase(), &[]);
    assert!(max_diff(&al, &bl) < 1e-6, "chorus B disturbs chorus A");
    assert!(max_diff(&bl, &br) > 1e-3, "the two sides differ");
    // Swaps that edit only chorus B or drive B (and back, overlapping too): the left channel
    // (chorus A) stays the reference sample for sample, so each carry lands on its own
    // instance.
    let swaps = [
        (320, with(p.clone(), CHORUS_B, "depth", 20.0), false),
        (330, p.clone(), false),
        (700, with(p.clone(), DRIVE_B, "drive_db", 3.0), false),
        (700, p.clone(), false),
        (701, with(p.clone(), CHORUS_B, "rate_hz", 3.0), false),
    ];
    let (l, r) = run(&p, blocks, &phrase(), &swaps);
    assert!(max_diff(&l, &bl) < 1e-4, "an edit of B changed A");
    assert!(
        max_diff(&r, &br) > 1e-3,
        "the edits of B are heard on the right"
    );
}

/// Noise into the output through a VCA per voice, gated by `midi.in`.
fn noise_voices() -> PatchState {
    let mut b = B(PatchState::new(), 0);
    b.m(MIDI, "midi.in", &[])
        .m(NOISE, "noise", &[("color", 0.0)])
        .m(ENV, "env.adsr", &[("attack_ms", 0.1), ("sustain", 1.0)])
        .m(VCA, "vca", &[("gain", 0.0)])
        .m(OUT, "out", &[]);
    b.c(port(MIDI, "gate"), port(ENV, "gate"))
        .c(port(NOISE, "out"), port(VCA, "in"))
        .c(port(ENV, "out"), port(VCA, "cv"))
        .c(port(VCA, "out"), port(OUT, "left"))
        .c(port(VCA, "out"), port(OUT, "right"));
    b.0
}

#[test]
fn each_voice_plays_its_own_noise_stream() {
    let p = noise_voices();
    let on = |note| KeyEvent::On {
        note,
        velocity: 127,
    };
    let (one, _) = run(&p, SEC, &[(0, on(60))], &[]);
    let (two, _) = run(&p, SEC, &[(0, on(60)), (0, on(64))], &[]);
    let (four, _) = run(
        &p,
        SEC,
        &[(0, on(60)), (0, on(64)), (0, on(67)), (0, on(71))],
        &[],
    );
    let win = |v: &[f32]| rms(&v[50 * BLOCK..]);
    // Independent streams add in power (+3 dB per doubling); identical ones would add in
    // amplitude (+6 dB).
    let d2 = db(win(&two)) - db(win(&one));
    let d4 = db(win(&four)) - db(win(&one));
    assert!((d2 - 3.01).abs() < 0.5, "2 voices: {d2:+.2} dB");
    assert!((d4 - 6.02).abs() < 0.5, "4 voices: {d4:+.2} dB");

    // The same noise also patched straight to the right output (a path no MIDI reaches):
    // it stays one instance, so the right keeps its full level, and the voices share the
    // stream (a chord adds in amplitude).
    let mut q = p.clone();
    q.cables.retain(|_, c| c.to != port(OUT, "right"));
    q.cables.insert(
        99,
        CableState {
            from: port(NOISE, "out"),
            to: port(OUT, "right"),
            params: Default::default(),
            steps: vec![],
        },
    );
    let (one, r1) = run(&q, SEC, &[(0, on(60))], &[]);
    let (two, _) = run(&q, SEC, &[(0, on(60)), (0, on(64))], &[]);
    let d2 = db(win(&two)) - db(win(&one));
    assert!((d2 - 6.02).abs() < 0.2, "shared: {d2:+.2} dB");
    assert!(
        (db(win(&r1)) + 14.0).abs() < 0.5,
        "direct path: {:.1} dBFS",
        db(win(&r1))
    );
}

#[test]
fn single_instance_noise_plays_the_lane_zero_stream() {
    // Noise no `midi.in` reaches compiles to one instance, seeded as voice lane 0: it is the
    // stream voice 0 of the same module plays in a voiced patch, so adding or removing a MIDI
    // cable never changes what an unvoiced noise sounds like at its start.
    let mut b = B(PatchState::new(), 0);
    b.m(NOISE, "noise", &[("color", 1.0)]).m(OUT, "out", &[]);
    b.c(port(NOISE, "out"), port(OUT, "left"));
    let alone = b.0;
    let mut single = compile(&alone, SR, VOICES).unwrap();
    // The voiced patch with the gate held from the first sample (attack 0.1 ms, full sustain)
    // is compared after the attack, through a unity VCA on voice 0 only.
    let p = with(noise_voices(), NOISE, "color", 1.0);
    let p = with(p, ENV, "attack_ms", 0.1);
    let (voiced, _) = run(
        &p,
        20,
        &[(
            0,
            KeyEvent::On {
                note: 60,
                velocity: 127,
            },
        )],
        &[],
    );
    let mut alone_out = Vec::new();
    for _ in 0..20 {
        single.process_block();
        alone_out.extend_from_slice(single.left());
    }
    // The voiced output is the voice average (1/8 of voice 0 with one voice sounding).
    let scale = alone_out[BLOCK..]
        .iter()
        .zip(&voiced[BLOCK..])
        .map(|(a, v)| v / a)
        .filter(|r| r.is_finite())
        .fold(0.0f32, |m, r| m.max(r));
    let d = alone_out[BLOCK..]
        .iter()
        .zip(&voiced[BLOCK..])
        .map(|(a, v)| (a * scale - v).abs())
        .fold(0.0f32, f32::max);
    assert!(scale > 0.05 && d < 1e-4, "scale {scale}, diff {d}");
}

#[test]
fn save_reload_undo_and_a_fresh_load() {
    let dir = tempfile::tempdir().unwrap();
    let p = palette(2.0);
    // Build the patch through the op log, save, reload.
    let mut log = PatchLog::new();
    for (&id, m) in &p.modules {
        log.append(
            Op::AddModule {
                id,
                kind: m.kind.clone(),
                pos: m.pos,
            },
            0,
            Source::User,
        );
        for (k, &v) in &m.params {
            log.append(
                Op::SetParam {
                    target: ParamTarget::Module {
                        id,
                        param: k.clone(),
                    },
                    value: v,
                },
                0,
                Source::User,
            );
        }
    }
    for (&id, c) in &p.cables {
        log.append(
            Op::Connect {
                id,
                from: c.from.clone(),
                to: c.to.clone(),
            },
            0,
            Source::User,
        );
    }
    assert_eq!(log.state(), &p);
    kabl_core::save(dir.path(), &log).unwrap();
    let back = kabl_core::load(dir.path()).unwrap();
    assert_eq!(back.state(), &p);
    let (rl, rr) = run(&p, SEC, &phrase(), &[]);
    let (bl, br) = run(back.state(), SEC, &phrase(), &[]);
    assert_eq!(
        (rl.clone(), rr),
        (bl, br),
        "reloaded patch renders identically"
    );

    // An edit and its undo: the undone state is the original, and swapping it in while
    // playing is null outside the two crossfades.
    let mut log = back;
    log.append(
        Op::SetParam {
            target: ParamTarget::Module {
                id: CHORUS_A,
                param: "depth".into(),
            },
            value: 10.0,
        },
        0,
        Source::User,
    );
    let edited = log.state().clone();
    assert!(log.undo());
    assert_eq!(log.state(), &p);
    let swaps = [(200, edited, false), (260, log.state().clone(), false)];
    let (l, _) = run(&p, SEC, &phrase(), &swaps);
    let s = |b: usize| b * BLOCK;
    assert!(max_diff(&l[..s(200)], &rl[..s(200)]) < 1e-6);
    // After the undo the chorus glides its depth back (50 ms time constant); 0.4 s later it
    // is the original sound.
    let settled = s(260 + 300);
    assert!(
        max_diff(&l[settled..s(700)], &rl[settled..s(700)]) < 1e-3,
        "undo returns to the original sound"
    );

    // A fresh Load while the chorus and drive ring: after the crossfade the output is a fresh
    // engine's from its first block (no carried history, no held keys).
    let at = 440;
    let (_, r) = run(&p, at + SEC / 2, &phrase(), &[(at, p.clone(), true)]);
    let (_, fresh) = run(&p, SEC / 2, &[], &[]);
    let tail = &r[s(at + 12)..];
    assert!(
        max_diff(tail, &fresh[s(12)..s(12) + tail.len()]) < 1e-6,
        "a Load carries nothing"
    );
}

#[test]
fn old_patches_are_unchanged_by_the_new_params() {
    // A pre-batch oscillator/midi.in (no pw, fine, unison, mode, glide params stored) renders
    // exactly like one with the new params at their defaults.
    let mut b = B(PatchState::new(), 0);
    b.m(MIDI, "midi.in", &[])
        .m(OSC, "osc.va", &[("waveform", 2.0)])
        .m(ENV, "env.adsr", &[])
        .m(VCA, "vca", &[("gain", 0.0)])
        .m(OUT, "out", &[]);
    b.c(port(MIDI, "pitch"), port(OSC, "pitch"))
        .c(port(MIDI, "gate"), port(ENV, "gate"))
        .c(port(OSC, "out"), port(VCA, "in"))
        .c(port(ENV, "out"), port(VCA, "cv"))
        .c(port(VCA, "out"), port(OUT, "left"));
    let old = b.0;
    let mut new = old.clone();
    for (k, v) in [
        ("pw", 50.0),
        ("fine", 0.0),
        ("unison", 1.0),
        ("detune", 15.0),
    ] {
        new = with(new, OSC, k, v);
    }
    for (k, v) in [
        ("mode", 0.0),
        ("priority", 0.0),
        ("glide", 0.0),
        ("glide_ms", 120.0),
    ] {
        new = with(new, MIDI, k, v);
    }
    let (a, _) = run(&old, SEC, &phrase(), &[]);
    let (b, _) = run(&new, SEC, &phrase(), &[]);
    assert_eq!(a, b);
}
