//! D06 timing (docs/midi-timing/design.md): one timestamped performance rendered through the
//! production `Timeline` adapter with host buffers of 1, 63, 64, 65, 127, 256, 512 frames and
//! irregular mixes gives the same samples, bit for bit, and equals the engine driven directly
//! on its 64-sample grid shifted by exactly `LATENCY` frames. Covers events at frame 0, inside
//! blocks, at block edges and final frames, simultaneous events, repeated and overlapping
//! notes, two channels and two sources, bend, wheel, sustain, notes off, reset, panic, a
//! routing change by graph replacement, an off-thread compile, a Load, feedback, noise,
//! sequences, effects tails and other sample rates; then late, out-of-range and overflowing
//! events, restart, and no allocation.

use std::path::Path;

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::{Collector, Owned};
use kabl_core::{CableState, ModuleId, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::compile::{compile, CompiledPatch};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::{KeyEvent, MidiEvent, Source};
use kabl_engine::patch_engine::PatchEngine;
use kabl_engine::timeline::{Timeline, LATENCY, QUEUE};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const VOICES: usize = 8;
const PARTITIONS: [usize; 7] = [1, 63, 64, 65, 127, 256, 512];

fn port(id: ModuleId, p: &str) -> PortRef {
    PortRef::Module { id, port: p.into() }
}

fn param(id: ModuleId, p: &str) -> PortRef {
    PortRef::Param {
        id,
        param: p.into(),
    }
}

fn module(kind: &str, params: &[(&str, f32)]) -> ModuleState {
    ModuleState {
        kind: kind.into(),
        pos: Vec2::default(),
        params: params.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
    }
}

fn cable(p: &mut PatchState, from: PortRef, to: PortRef, amount: Option<f32>) {
    let id = p.cables.len() as u64 + 1;
    let mut params = std::collections::BTreeMap::new();
    if let Some(a) = amount {
        params.insert("amount".to_string(), a);
    }
    p.cables.insert(
        id,
        CableState {
            from,
            to,
            params,
            steps: vec![],
        },
    );
}

/// A voice with a feedback loop (filter back into its own mixer: one block late), seeded
/// noise, the wheel on the cutoff and an LFO on the resonance: every grid rule at once. A
/// second keyboard on channel 2 plays its own oscillator when `split`.
fn synth(channel_a: f32, split: bool) -> PatchState {
    let mut p = PatchState::new();
    p.modules.insert(
        1,
        module("midi.in", &[("channel", channel_a), ("bend", 7.0)]),
    );
    p.modules.insert(2, module("osc.va", &[("waveform", 2.0)]));
    p.modules.insert(3, module("mixer", &[]));
    p.modules.insert(
        4,
        module("filter.svf", &[("cutoff_hz", 900.0), ("resonance", 0.4)]),
    );
    p.modules
        .insert(5, module("env.adsr", &[("release_ms", 80.0)]));
    p.modules.insert(6, module("vca", &[]));
    p.modules.insert(7, module("noise", &[("level_db", -18.0)]));
    p.modules.insert(8, module("lfo", &[("rate_hz", 3.3)]));
    p.modules.insert(9, module("out", &[]));
    cable(&mut p, port(1, "pitch"), port(2, "pitch"), None);
    cable(&mut p, port(2, "out"), port(3, "in1"), None);
    cable(&mut p, port(7, "out"), port(3, "in3"), None);
    cable(&mut p, port(3, "out"), port(4, "in"), None);
    // The feedback cable, through a VCA at 0.3 so the loop stays stable.
    p.modules.insert(13, module("vca", &[("gain", 0.3)]));
    cable(&mut p, port(4, "bp"), port(13, "in"), None);
    cable(&mut p, port(13, "out"), port(3, "in2"), None);
    cable(&mut p, port(1, "gate"), port(5, "gate"), None);
    cable(&mut p, port(4, "lp"), port(6, "in"), None);
    cable(&mut p, port(5, "out"), port(6, "cv"), None);
    cable(&mut p, port(6, "out"), port(9, "left"), None);
    cable(&mut p, port(1, "wheel"), param(4, "cutoff_hz"), Some(0.6));
    cable(
        &mut p,
        port(1, "velocity"),
        param(4, "resonance"),
        Some(0.3),
    );
    cable(&mut p, port(8, "out"), param(4, "resonance"), Some(0.2));
    if split {
        p.modules.insert(
            10,
            module(
                "midi.in",
                &[("channel", 2.0), ("mode", 2.0), ("glide", 1.0)],
            ),
        );
        p.modules.insert(11, module("osc.va", &[("waveform", 1.0)]));
        p.modules.insert(12, module("vca", &[]));
        cable(&mut p, port(10, "pitch"), port(11, "pitch"), None);
        cable(&mut p, port(11, "out"), port(12, "in"), None);
        cable(&mut p, port(10, "gate"), port(12, "cv"), None);
        cable(&mut p, port(12, "out"), port(9, "right"), None);
    } else {
        cable(&mut p, port(6, "out"), port(9, "right"), None);
    }
    p
}

fn factory(name: &str) -> PatchState {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../patches")
        .join(name);
    kabl_core::load(&dir)
        .expect("factory patch")
        .state()
        .clone()
}

fn ev(source: Source, channel: u8, event: KeyEvent) -> MidiEvent {
    MidiEvent::new(source, channel, event)
}

fn on(ch: u8, note: u8, velocity: u8) -> MidiEvent {
    ev(Source::Controller, ch, KeyEvent::On { note, velocity })
}

fn off(ch: u8, note: u8) -> MidiEvent {
    ev(Source::Controller, ch, KeyEvent::Off { note })
}

/// The timestamped performance: `(timeline sample, event)`, in time order, over `len` samples.
fn performance(len: u64) -> Vec<(u64, MidiEvent)> {
    let c = Source::Controller;
    let mut v = vec![
        (0, on(0, 60, 100)), // frame 0
        (63, on(0, 64, 90)), // last frame of block 0
        (64, on(0, 67, 80)), // first frame of block 1
        (65, off(0, 64)),    // second frame of block 1
        (127, on(1, 43, 110)),
        (5_000, on(0, 72, 70)), // simultaneous pair
        (5_000, on(0, 76, 70)),
        (7_001, on(1, 60, 99)),  // same key, other channel: its own voice
        (9_000, on(0, 60, 127)), // repeated note, same identity
        (
            9_000,
            ev(
                Source::Preview,
                0,
                KeyEvent::On {
                    note: 62,
                    velocity: 60,
                },
            ),
        ),
        (12_345, off(0, 60)),
        (15_000, off(1, 60)),
        (16_000, off(0, 67)),
        (16_000, off(0, 72)),
        (16_001, off(0, 76)),
        (16_002, off(1, 43)),
        (18_000, ev(Source::Preview, 0, KeyEvent::Off { note: 62 })),
    ];
    // Held notes under bend and wheel sweeps on both channels.
    v.push((19_000, on(0, 57, 100)));
    v.push((19_050, on(1, 38, 100)));
    for i in 0..200u64 {
        let t = 20_000 + i * 97;
        let b = ((i * 311) % 16384) as u16;
        v.push((t, ev(c, (i % 2) as u8, KeyEvent::Bend(b))));
        v.push((t + 31, ev(c, 0, KeyEvent::Wheel((i % 128) as u8))));
    }
    v.push((40_000, ev(c, 0, KeyEvent::ResetControllers)));
    // The pedal holds released notes; channel 2's pedal does not hold channel 1.
    v.push((42_000, ev(c, 0, KeyEvent::Sustain(true))));
    v.push((42_100, on(0, 69, 90)));
    v.push((42_200, off(0, 69)));
    v.push((42_300, off(0, 57)));
    v.push((43_000, ev(c, 1, KeyEvent::Sustain(true))));
    v.push((43_500, ev(c, 1, KeyEvent::Sustain(false))));
    v.push((50_000, ev(c, 0, KeyEvent::Sustain(false))));
    v.push((51_000, ev(c, 1, KeyEvent::NotesOff)));
    // Fast repeated notes inside one block, and a mono chain on channel 2.
    for k in 0..6u64 {
        v.push((52_000 + k * 9, on(0, 48 + k as u8, 100)));
        v.push((52_004 + k * 9, off(0, 48 + k as u8)));
    }
    for k in 0..5u64 {
        v.push((54_000 + k * 700, on(1, 50 + 2 * k as u8, 100)));
        v.push((54_350 + k * 700, off(1, 50 + 2 * k as u8)));
    }
    v.push((60_000, on(0, 65, 100)));
    v.push((61_000, on(1, 41, 100)));
    // After the routing change (64_000) the held notes are released by their own offs.
    v.push((66_000, off(0, 65)));
    v.push((66_001, off(1, 41)));
    v.push((70_000, on(0, 62, 100)));
    v.push((70_064, ev(c, 0, KeyEvent::AllOff))); // panic
    v.push((80_000, on(0, 64, 100)));
    // Final frames.
    v.push((len - LATENCY as u64 - 1, on(0, 71, 100)));
    v.push((len - 1, on(0, 72, 100)));
    v.sort_by_key(|e| e.0); // stable: equal times keep their order
    v
}

/// A graph change the performance applies at a block start: routing, then a Load.
struct Change {
    at: u64,
    graph: Option<Owned<CompiledPatch>>,
}

fn changes(handle: &basedrop::Handle, patch: &PatchState, sr: f32, routing: bool) -> Vec<Change> {
    if !routing {
        return Vec::new();
    }
    // Channel of keyboard 1 from ALL to 1, compiled on another thread like the app's worker.
    let mut rerouted = patch.clone();
    rerouted
        .modules
        .get_mut(&1)
        .unwrap()
        .params
        .insert("channel".into(), 1.0);
    let built = std::thread::spawn(move || compile(&rerouted, sr, VOICES).unwrap())
        .join()
        .unwrap();
    let mut loaded = compile(patch, sr, VOICES).unwrap();
    loaded.fresh = true;
    vec![
        Change {
            at: 64_000,
            graph: Some(Owned::new(handle, built)),
        },
        Change {
            at: 76_800,
            graph: Some(Owned::new(handle, loaded)),
        },
    ]
}

fn install(changes: &mut [Change], engine: &mut PatchEngine, start: u64) {
    for c in changes.iter_mut() {
        if c.at == start {
            if let Some(g) = c.graph.take() {
                engine.receive_swap(g);
            }
        }
    }
}

/// The engine alone on its grid: the reference the adapter must equal, `LATENCY` later.
fn direct(patch: &PatchState, sr: f32, len: u64, routing: bool) -> Vec<(f32, f32)> {
    let c = Collector::new();
    let mut e = PatchEngine::new(&c.handle(), patch, sr, VOICES).unwrap();
    let mut ch = changes(&c.handle(), patch, sr, routing);
    let events = performance(len);
    let mut next = 0;
    let mut out = Vec::with_capacity(len as usize);
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    let mut start = 0u64;
    while start < len {
        install(&mut ch, &mut e, start);
        while next < events.len() && events[next].0 < start + BLOCK as u64 {
            e.key_at(events[next].1, (events[next].0 - start) as usize);
            next += 1;
        }
        e.process_block(&mut l, &mut r);
        out.extend(l.iter().zip(&r).map(|(&a, &b)| (a, b)));
        start += BLOCK as u64;
    }
    out.truncate(len as usize);
    out
}

/// Through the adapter, host buffers from `sizes` (cycled).
fn adapter(
    patch: &PatchState,
    sr: f32,
    len: u64,
    sizes: &[usize],
    routing: bool,
) -> Vec<(f32, f32)> {
    let c = Collector::new();
    let mut e = PatchEngine::new(&c.handle(), patch, sr, VOICES).unwrap();
    let mut ch = changes(&c.handle(), patch, sr, routing);
    let mut tl = Box::new(Timeline::new());
    let events = performance(len);
    let mut next = 0;
    let mut out = Vec::with_capacity(len as usize);
    let mut host = 0u64;
    let mut k = 0;
    let (mut l, mut r) = (vec![0f32; 1024], vec![0f32; 1024]);
    let mut batch = Vec::new();
    while host < len {
        let n = sizes[k % sizes.len()].min((len - host) as usize);
        k += 1;
        batch.clear();
        while next < events.len() && events[next].0 < host + n as u64 {
            batch.push(((events[next].0 - host) as usize, events[next].1));
            next += 1;
        }
        tl.process(&mut e, &mut l[..n], &mut r[..n], &batch, |e, start| {
            install(&mut ch, e, start)
        });
        out.extend(l[..n].iter().zip(&r[..n]).map(|(&a, &b)| (a, b)));
        host += n as u64;
    }
    assert_eq!(
        tl.stats().clamped + tl.stats().reordered + tl.stats().dropped,
        0
    );
    out
}

/// Deterministic irregular host buffers, 1..=700 frames.
fn irregular(seed: u64, count: usize) -> Vec<usize> {
    let mut x = seed;
    (0..count)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x % 700) as usize + 1
        })
        .collect()
}

fn check(name: &str, patch: &PatchState, sr: f32, secs: f32, routing: bool) {
    let len = (secs * sr) as u64;
    let reference = direct(patch, sr, len, routing);
    let energy: f64 = reference.iter().map(|&(a, b)| (a * a + b * b) as f64).sum();
    assert!(
        energy > 1e-3,
        "{name}: the performance is audible ({energy})"
    );
    let mut runs: Vec<(String, Vec<usize>)> = PARTITIONS
        .iter()
        .map(|&n| (format!("{n}"), vec![n]))
        .collect();
    runs.push(("irregular A".into(), irregular(0x9E37_79B9, 997)));
    runs.push(("irregular B".into(), irregular(0xDEAD_BEEF, 991)));
    runs.push(("alternating 1/511".into(), vec![1, 511, 2, 62, 64, 66, 300]));
    for (label, sizes) in &runs {
        let got = adapter(patch, sr, len, sizes, routing);
        assert!(
            got[..LATENCY].iter().all(|&s| s == (0.0, 0.0)),
            "{name} {label}: latency is silent"
        );
        for (i, (g, want)) in got[LATENCY..].iter().zip(&reference).enumerate() {
            assert!(
                g.0.to_bits() == want.0.to_bits() && g.1.to_bits() == want.1.to_bits(),
                "{name}, buffers {label}: sample {i} (+{LATENCY}) is {g:?}, the grid gives {want:?}"
            );
        }
    }
}

#[test]
fn a_synth_with_feedback_noise_wheel_and_a_routing_change_is_partition_independent() {
    check("synth", &synth(0.0, true), 48000.0, 1.9, true);
}

#[test]
fn factory_voices_are_partition_independent() {
    for name in [
        "palette/lead",
        "palette/pad",
        "palette/bass",
        "init-keyboard",
    ] {
        check(name, &factory(name), 48000.0, 1.8, false);
    }
}

#[test]
fn sequences_effects_and_tails_are_partition_independent() {
    for name in ["performance", "composition", "echo"] {
        check(name, &factory(name), 48000.0, 1.8, false);
    }
}

#[test]
fn other_sample_rates_are_partition_independent() {
    check("synth @ 44.1 kHz", &synth(0.0, true), 44100.0, 1.9, true);
    check("synth @ 96 kHz", &synth(0.0, true), 96000.0, 1.0, false);
}

/// A gate edge lands on its own sample (plus the fixed latency), not on a block start.
#[test]
fn a_note_sounds_from_its_own_sample() {
    let mut p = PatchState::new();
    p.modules.insert(1, module("midi.in", &[]));
    p.modules.insert(2, module("out", &[]));
    cable(&mut p, port(1, "gate"), port(2, "right"), None);
    for (t, n) in [
        (1000u64, 1),
        (1000, 7),
        (1000, 1000),
        (1023, 64),
        (1024, 5),
        (1025, 3),
    ] {
        let c = Collector::new();
        let mut e = PatchEngine::new(&c.handle(), &p, 48000.0, 1).unwrap();
        let mut tl = Timeline::new();
        let (mut l, mut r) = (vec![0f32; n], vec![0f32; n]);
        let mut host = 0u64;
        let mut first = None;
        while host < 3000 {
            let ev: Vec<_> = if (host..host + n as u64).contains(&t) {
                vec![((t - host) as usize, on(0, 60, 100))]
            } else {
                vec![]
            };
            tl.process(&mut e, &mut l, &mut r, &ev, |_, _| {});
            if first.is_none() {
                first = r.iter().position(|&g| g > 0.5).map(|i| host + i as u64);
            }
            host += n as u64;
        }
        assert_eq!(
            first,
            Some(t + LATENCY as u64),
            "note at {t}, buffers of {n}"
        );
    }
}

#[test]
fn late_out_of_range_and_overflowing_events_never_strand_a_note() {
    let c = Collector::new();
    let patch = synth(0.0, false);
    let mut e = PatchEngine::new(&c.handle(), &patch, 48000.0, VOICES).unwrap();
    let mut tl = Box::new(Timeline::new());
    let (mut l, mut r) = (vec![0f32; 256], vec![0f32; 256]);
    // Out of order and out of range: moved, counted, still played and released.
    tl.push(100, 256, on(0, 60, 100));
    tl.push(10, 256, on(0, 62, 100));
    tl.push(900, 256, on(0, 64, 100));
    tl.render(&mut e, &mut l, &mut r, |_, _| {});
    let s = tl.stats();
    assert_eq!((s.reordered, s.clamped), (1, 1));
    tl.render(&mut e, &mut l, &mut r, |_, _| {});
    assert_eq!(e.keys().0, 3);
    for n in [60, 62, 64] {
        tl.push(0, 256, off(0, n));
    }
    tl.render(&mut e, &mut l, &mut r, |_, _| {});
    tl.render(&mut e, &mut l, &mut r, |_, _| {});
    assert_eq!(e.keys(), (0, 0));

    // A full queue: note-ons beyond it are dropped, releases are forced, nothing sticks.
    for i in 0..QUEUE {
        tl.push(i % 256, 256, on(0, (i % 100) as u8, 100));
    }
    tl.push(255, 256, on(0, 120, 100));
    tl.push(255, 256, off(0, 3));
    let s = tl.stats();
    assert_eq!((s.dropped, s.forced), (1, 1));
    for _ in 0..8 {
        tl.render(&mut e, &mut l, &mut r, |_, _| {});
    }
    assert_eq!(tl.queued(), 0);
    assert_eq!(
        e.keys(),
        (0, 0),
        "the forced release ended every controller key"
    );

    // A reset that overflows still returns bend and wheel to rest (review R-01).
    tl.push(0, 256, ev(Source::Controller, 0, KeyEvent::Bend(0)));
    tl.push(0, 256, ev(Source::Controller, 0, KeyEvent::Wheel(127)));
    tl.render(&mut e, &mut l, &mut r, |_, _| {});
    tl.render(&mut e, &mut l, &mut r, |_, _| {});
    assert_eq!(e.expression(1), Some((-1.0, 1.0)));
    for i in 0..QUEUE {
        let wheel = KeyEvent::Wheel((i % 100) as u8);
        tl.push(i % 256, 256, ev(Source::Controller, 0, wheel));
    }
    tl.push(
        255,
        256,
        ev(Source::Controller, 0, KeyEvent::ResetControllers),
    );
    for _ in 0..8 {
        tl.render(&mut e, &mut l, &mut r, |_, _| {});
    }
    assert_eq!(e.expression(1), Some((0.0, 0.0)));
}

/// Restart (D05 Retry, a new sample rate): a fresh engine and a reset timeline replay the
/// performance exactly like the first run.
#[test]
fn a_restart_replays_identically() {
    let patch = synth(0.0, true);
    let len = 30_000u64;
    let first = adapter(&patch, 48000.0, len, &[256], false);
    let c = Collector::new();
    let mut e = PatchEngine::new(&c.handle(), &patch, 48000.0, VOICES).unwrap();
    let mut tl = Box::new(Timeline::new());
    let (mut l, mut r) = (vec![0f32; 256], vec![0f32; 256]);
    tl.process(&mut e, &mut l, &mut r, &[(3, on(0, 50, 100))], |_, _| {});
    tl.reset();
    let mut e = PatchEngine::new(&c.handle(), &patch, 48000.0, VOICES).unwrap();
    let events = performance(len);
    let (mut next, mut host, mut again) = (0, 0u64, Vec::new());
    while host < len {
        let n = 256.min((len - host) as usize);
        let mut batch = Vec::new();
        while next < events.len() && events[next].0 < host + n as u64 {
            batch.push(((events[next].0 - host) as usize, events[next].1));
            next += 1;
        }
        tl.process(&mut e, &mut l[..n], &mut r[..n], &batch, |_, _| {});
        again.extend(l[..n].iter().zip(&r[..n]).map(|(&a, &b)| (a, b)));
        host += n as u64;
    }
    assert!(first
        .iter()
        .zip(&again)
        .all(|(a, b)| a.0.to_bits() == b.0.to_bits() && a.1.to_bits() == b.1.to_bits()));
}

#[test]
fn the_adapter_does_not_allocate() {
    let c = Collector::new();
    let patch = synth(0.0, true);
    let mut e = PatchEngine::new(&c.handle(), &patch, 48000.0, VOICES).unwrap();
    let mut tl = Box::new(Timeline::new());
    let (mut l, mut r) = (vec![0f32; 512], vec![0f32; 512]);
    let events: Vec<(usize, MidiEvent)> = (0..40)
        .map(|i| {
            let e = match i % 5 {
                0 => on((i % 2) as u8, 50 + i as u8, 100),
                1 => ev(Source::Controller, 0, KeyEvent::Bend(i as u16 * 400)),
                2 => ev(Source::Controller, 1, KeyEvent::Wheel(i as u8 * 3)),
                3 => ev(Source::Controller, 0, KeyEvent::Sustain(i % 2 == 0)),
                _ => off((i % 2) as u8, 50 + i as u8 - 4),
            };
            (i * 12, e)
        })
        .collect();
    tl.process(&mut e, &mut l, &mut r, &events, |_, _| {});
    assert_no_alloc(|| {
        for n in [1usize, 63, 65, 127, 512] {
            tl.process(
                &mut e,
                &mut l[..n],
                &mut r[..n],
                &events[..n.min(40)],
                |_, _| {},
            );
        }
        tl.process(
            &mut e,
            &mut l[..64],
            &mut r[..64],
            &[(0, ev(Source::Controller, 0, KeyEvent::SourceLost))],
            |_, _| {},
        );
        tl.render(&mut e, &mut l[..128], &mut r[..128], |_, _| {});
    });
}
