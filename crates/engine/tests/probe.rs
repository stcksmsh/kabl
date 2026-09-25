//! D03 selected-signal tap (`probe.rs`, docs/signal-inspection/design.md): it measures the
//! selected output at the right point of the schedule, keeps voices apart, names the graph it
//! measured, never changes the sound and never allocates on the audio thread.

use std::collections::BTreeMap;

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::Collector;
use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::{Command, PatchEngine};
use kabl_engine::probe::{ProbeReport, ProbeStatus, ProbeTarget};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;

fn module(kind: &str, params: &[(&str, f32)]) -> ModuleState {
    ModuleState {
        kind: kind.to_string(),
        pos: Vec2 { x: 0.0, y: 0.0 },
        params: params.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
    }
}

fn cable(from: u64, fp: &str, to: u64, tp: &str) -> CableState {
    CableState {
        from: PortRef::Module {
            id: from,
            port: fp.into(),
        },
        to: PortRef::Module {
            id: to,
            port: tp.into(),
        },
        params: BTreeMap::new(),
        steps: Vec::new(),
    }
}

/// Macro m1 (0.3, constant) → VCA #2 (×0.5) → VCA #3 (×0.5) → Output: three different DC
/// levels along a chain whose buffers the compiler coalesces.
fn chain() -> PatchState {
    let mut p = PatchState::new();
    p.modules.insert(1, module("macro", &[("m1", 0.3)]));
    p.modules.insert(2, module("vca", &[("gain", 0.5)]));
    p.modules.insert(3, module("vca", &[("gain", 0.5)]));
    p.modules.insert(4, module("out", &[]));
    p.cables.insert(1, cable(1, "m1", 2, "in"));
    p.cables.insert(2, cable(2, "out", 3, "in"));
    p.cables.insert(3, cable(3, "out", 4, "left"));
    p
}

/// A keyboard voice: MIDI In → Osc → VCA ← ADSR, to Output.
fn keys() -> PatchState {
    let mut p = PatchState::new();
    p.modules.insert(1, module("midi.in", &[]));
    p.modules.insert(2, module("osc.va", &[]));
    p.modules
        .insert(3, module("env.adsr", &[("attack_ms", 1.0)]));
    p.modules.insert(4, module("vca", &[("gain", 0.0)]));
    p.modules.insert(5, module("out", &[]));
    p.cables.insert(1, cable(1, "pitch", 2, "pitch"));
    p.cables.insert(2, cable(1, "gate", 3, "gate"));
    p.cables.insert(3, cable(2, "out", 4, "in"));
    p.cables.insert(4, cable(3, "out", 4, "cv"));
    p.cables.insert(5, cable(4, "out", 5, "left"));
    p.cables.insert(6, cable(4, "out", 5, "right"));
    p
}

fn target(token: u64, module: u64, port: u8, kind: &'static str) -> ProbeTarget {
    ProbeTarget {
        token,
        module,
        port,
        kind,
    }
}

/// Runs blocks until a report comes (at most `max`).
fn next_report(e: &mut PatchEngine, max: usize) -> ProbeReport {
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    for _ in 0..max {
        e.process_block(&mut l, &mut r);
        if let Some(rep) = e.take_probe_report() {
            return rep;
        }
    }
    panic!("no report in {max} blocks");
}

fn engine(p: &PatchState, voices: usize) -> (Collector, PatchEngine) {
    let c = Collector::new();
    let e = PatchEngine::new(&c.handle(), p, SR, voices).unwrap();
    (c, e)
}

#[test]
fn the_tap_reads_each_module_output_before_its_buffer_is_reused() {
    let (_c, mut e) = engine(&chain(), 4);
    for (id, port, kind, level) in [
        (1, 0, "macro", 0.3f32),
        (2, 0, "vca", 0.15),
        (3, 0, "vca", 0.075),
    ] {
        e.inspect(Some(target(id, id, port, kind)));
        let r = next_report(&mut e, 100);
        assert_eq!(r.token, id);
        assert_eq!(r.status, ProbeStatus::Measured);
        assert_eq!((r.lanes, r.voiced), (1, false), "one shared instance");
        let s = r.lane[0];
        assert!(
            (s.min - level).abs() < 1e-6 && (s.max - level).abs() < 1e-6,
            "{id}: {s:?}"
        );
        assert_eq!(r.samples, 38 * BLOCK as u32, "about 50 ms of whole blocks");
    }
    // Macro m2 is unconnected but still an output: 0, measured (not "missing").
    e.inspect(Some(target(9, 1, 1, "macro")));
    let r = next_report(&mut e, 100);
    assert_eq!(r.status, ProbeStatus::Measured);
    assert_eq!(r.lane[0].peak, 0.0);
}

#[test]
fn voices_are_separate_lanes_and_gate_edges_and_pitch_are_exact() {
    let (_c, mut e) = engine(&keys(), 8);
    e.inspect(Some(target(1, 1, 0, "midi.in")));
    let r = next_report(&mut e, 100);
    assert!(r.voiced);
    assert_eq!((r.lanes, r.lanes_total), (8, 8));
    assert!(r.lane.iter().take(8).all(|s| s.rises == 0 && s.high == 0));

    e.key(KeyEvent::On {
        note: 67,
        velocity: 100,
    });
    let r = next_report(&mut e, 100);
    let active: Vec<usize> = (0..8).filter(|&l| r.lane[l].rises > 0).collect();
    assert_eq!(active.len(), 1, "one key, one voice lane: {active:?}");
    let lane = active[0];
    assert_eq!(r.lane[lane].rises, 1);
    // Pitch output of the same lane: +7 semitones from note 60.
    e.inspect(Some(target(2, 1, 1, "midi.in")));
    let r = next_report(&mut e, 100);
    assert!((r.lane[lane].last - 7.0).abs() < 1e-4, "{:?}", r.lane[lane]);
    // A held gate: high throughout, no new edge.
    e.inspect(Some(target(3, 1, 0, "midi.in")));
    let r = next_report(&mut e, 100);
    assert_eq!(r.lane[lane].rises, 0);
    assert_eq!(r.lane[lane].high, r.samples);
}

#[test]
fn clock_edges_match_its_rate() {
    // One-sample pulses are covered by `probe.rs`'s own unit test.
    let mut p = PatchState::new();
    p.modules.insert(1, module("clock", &[("bpm", 300.0)]));
    p.modules.insert(2, module("out", &[]));
    let (_c, mut e) = engine(&p, 2);
    e.inspect(Some(target(1, 1, 0, "clock")));
    let mut rises = 0;
    let mut high = 0;
    let mut samples = 0;
    for _ in 0..20 {
        let r = next_report(&mut e, 100);
        rises += r.lane[0].rises;
        high += r.lane[0].high;
        samples += r.samples;
    }
    // 300 BPM sixteenths = 20 pulses per second; 20 windows ≈ 1.01 s.
    let secs = samples as f32 / SR;
    assert!(
        (rises as f32 - 20.0 * secs).abs() <= 1.5,
        "{rises} in {secs} s"
    );
    assert!(high > 0 && high < samples, "pulses: {high} of {samples}");
}

#[test]
fn swaps_name_the_graph_and_a_missing_target_says_so() {
    let c = Collector::new();
    let h = c.handle();
    let mut e = PatchEngine::new(&h, &chain(), SR, 4).unwrap();
    e.inspect(Some(target(5, 2, 0, "vca")));
    let r = next_report(&mut e, 100);
    assert_eq!(r.generation, 0);

    // Edit: VCA #2 gain 0.25, graph generation 7. During the fade the report is flagged and
    // measures the new graph.
    let mut p = chain();
    p.modules
        .get_mut(&2)
        .unwrap()
        .params
        .insert("gain".into(), 0.25);
    let mut g = e.build_swap(&h, &p).unwrap();
    g.generation = 7;
    e.receive_swap(g);
    let r = next_report(&mut e, 100);
    assert_eq!(r.generation, 7);
    assert!(r.fading);
    assert!(
        (r.lane[0].max - 0.075).abs() < 1e-6,
        "new graph: {:?}",
        r.lane[0]
    );
    let r = next_report(&mut e, 100);
    assert!(!r.fading);

    // Delete the module: the new graph reports it missing, never an old value.
    let mut p = chain();
    p.modules.remove(&2);
    p.cables
        .retain(|_, c| c.from.module_id() != 2 && c.to.module_id() != 2);
    let mut g = e.build_swap(&h, &p).unwrap();
    g.generation = 8;
    e.receive_swap(g);
    let r = next_report(&mut e, 100);
    assert_eq!((r.generation, r.status), (8, ProbeStatus::NotInGraph));

    // Same id, other kind (a reused id): not measured.
    let mut p = chain();
    p.modules.insert(2, module("macro", &[]));
    p.cables
        .retain(|_, c| c.to.module_id() != 2 && c.from.module_id() != 2);
    let mut g = e.build_swap(&h, &p).unwrap();
    g.generation = 9;
    e.receive_swap(g);
    let r = next_report(&mut e, 100);
    assert_eq!((r.generation, r.status), (9, ProbeStatus::NotInGraph));
}

#[test]
fn overlapping_swaps_report_the_newest_graph_and_never_go_back() {
    let c = Collector::new();
    let h = c.handle();
    let mut e = PatchEngine::new(&h, &chain(), SR, 4).unwrap();
    e.inspect(Some(target(1, 3, 0, "vca")));
    for gen in 1..=4 {
        let mut g = e.build_swap(&h, &chain()).unwrap();
        g.generation = gen;
        e.receive_swap(g); // 2..4 arrive during the first fade: queued, last wins
    }
    let mut last = 0;
    let mut seen = Vec::new();
    for _ in 0..40 {
        let r = next_report(&mut e, 100);
        assert!(
            r.generation >= last,
            "went back from {last} to {}",
            r.generation
        );
        last = r.generation;
        seen.push(r.generation);
    }
    assert_eq!(last, 4, "{seen:?}");
    assert!(
        !seen.contains(&2) && !seen.contains(&3),
        "superseded graphs never played"
    );
}

/// Renders `blocks` of the composition piece plus a held chord on the keyboard voice, with the
/// tap on `t` (or off).
fn render(p: &PatchState, t: Option<ProbeTarget>, blocks: usize) -> Vec<f32> {
    let (_c, mut e) = engine(p, 8);
    e.inspect(t);
    for note in [60, 64, 67] {
        e.key(KeyEvent::On { note, velocity: 90 });
    }
    let mut out = Vec::new();
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    for i in 0..blocks {
        if i == blocks / 2 {
            e.key(KeyEvent::AllOff);
        }
        e.process_block(&mut l, &mut r);
        out.extend_from_slice(&l);
        out.extend_from_slice(&r);
        let _ = e.take_probe_report();
    }
    out
}

#[test]
fn inspection_on_or_off_renders_the_same_samples() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches");
    for dir in ["composition", "palette/pad", "echo"] {
        let p = kabl_core::load(&root.join(dir)).unwrap().state().clone();
        let off = render(&p, None, 400);
        // Every output of every module, one at a time over a short render, then a long one on
        // a few.
        for (&id, m) in &p.modules {
            let info = kabl_modules::registry::info_for(&m.kind).unwrap();
            let outs = info
                .ports
                .iter()
                .filter(|p| p.direction == kabl_modules::PortDirection::Output)
                .count();
            for port in 0..outs {
                let on = render(&p, Some(target(1, id, port as u8, info.kind)), 400);
                assert!(on == off, "{dir}: tap on {id}.{port} changed the output");
            }
        }
    }
}

#[test]
fn the_audio_thread_path_never_allocates_with_the_tap_on() {
    let c = Collector::new();
    let h = c.handle();
    let p = kabl_core::load(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/composition"),
    )
    .unwrap()
    .state()
    .clone();
    let mut e = PatchEngine::new(&h, &p, SR, 8).unwrap();
    let ids: Vec<(u64, &'static str)> = p
        .modules
        .iter()
        .map(|(&id, m)| (id, kabl_modules::registry::info_for(&m.kind).unwrap().kind))
        .collect();
    let (mut tx, mut rx) = rtrb::RingBuffer::<ProbeReport>::new(2);
    let mut graphs: Vec<_> = (0..6).map(|_| e.build_swap(&h, &p).unwrap()).collect();
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    for i in 0..600 {
        let g = (i % 100 == 0).then(|| graphs.pop()).flatten();
        let (id, kind) = ids[i % ids.len()];
        assert_no_alloc(|| {
            if let Some(g) = g {
                e.receive_swap(g);
            }
            if i % 97 == 0 {
                e.command(&Command::Inspect(Some(target(i as u64, id, 0, kind))));
            }
            if i % 250 == 0 {
                e.command(&Command::Inspect(None));
            }
            e.process_block(&mut l, &mut r);
            if let Some(rep) = e.take_probe_report() {
                // Never drained: the queue fills and later reports are dropped.
                let _ = tx.push(rep);
            }
        });
    }
    assert!(rx.pop().is_ok());
}

#[test]
fn a_knob_being_turned_does_not_stop_the_measurement() {
    // The UI rebuilds on every frame of a drag: a swap every 8–32 ms, shorter than a window.
    let c = Collector::new();
    let h = c.handle();
    let mut e = PatchEngine::new(&h, &chain(), SR, 4).unwrap();
    e.inspect(Some(target(1, 3, 0, "vca")));
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    for every in [6, 12, 24] {
        let mut reports = Vec::new();
        for i in 0..1500 {
            if i % every == 0 {
                let mut p = chain();
                let g = 0.5 + (i % 7) as f32 * 0.01;
                p.modules
                    .get_mut(&3)
                    .unwrap()
                    .params
                    .insert("gain".into(), g);
                e.receive_swap(e.build_swap(&h, &p).unwrap());
            }
            e.process_block(&mut l, &mut r);
            if let Some(rep) = e.take_probe_report() {
                reports.push(rep);
            }
        }
        // 1500 blocks = 2 s: about 39 windows of 38 blocks.
        assert!(
            reports.len() >= 35,
            "every {every} blocks: {} reports",
            reports.len()
        );
        assert!(reports.iter().all(|r| r.status == ProbeStatus::Measured));
        assert!(reports.iter().any(|r| r.fading));
        // Report times follow the audio clock.
        assert!(reports
            .windows(2)
            .all(|w| w[1].end_sample > w[0].end_sample));
    }
}

#[test]
fn a_wiring_change_mid_window_starts_the_measurement_over() {
    // Tap on VCA #3 (0.075); the cable into it goes 20 blocks into a window: the first
    // report of the new graph must not carry the old level.
    let c = Collector::new();
    let h = c.handle();
    let mut e = PatchEngine::new(&h, &chain(), SR, 4).unwrap();
    e.inspect(Some(target(1, 3, 0, "vca")));
    let r0 = next_report(&mut e, 100);
    assert!((r0.lane[0].peak - 0.075).abs() < 1e-6);
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    for _ in 0..20 {
        e.process_block(&mut l, &mut r);
        assert!(e.take_probe_report().is_none());
    }
    let mut p = chain();
    p.cables.remove(&2);
    let mut g = e.build_swap(&h, &p).unwrap();
    g.generation = 2;
    e.receive_swap(g);
    let r1 = next_report(&mut e, 100);
    assert_eq!(r1.generation, 2);
    assert_eq!(r1.lane[0].peak, 0.0, "old level carried: {:?}", r1.lane[0]);
}
