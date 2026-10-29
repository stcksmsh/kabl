//! Not checks: clips for listening to the drum voices, `target/drum-clips/*.wav`. One clip per
//! voice with a few hits across its settings. Write them with
//! `cargo test -p kabl-engine --test drum_clips -- --ignored`.

use std::collections::BTreeMap;

use basedrop::Collector;
use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::{KeyEvent, MidiEvent, Source};
use kabl_engine::patch_engine::PatchEngine;

const SR: f32 = 48000.0;

fn module(kind: &str, params: &[(&str, f32)]) -> ModuleState {
    ModuleState {
        kind: kind.to_string(),
        pos: Vec2 { x: 0.0, y: 0.0 },
        params: params.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
    }
}

fn wire(p: &mut PatchState, a: (u64, &str), b: (u64, &str)) {
    let id = p.cables.len() as u64 + 1;
    let port = |(id, port): (u64, &str)| PortRef::Module {
        id,
        port: port.into(),
    };
    p.cables.insert(
        id,
        CableState {
            from: port(a),
            to: port(b),
            params: BTreeMap::new(),
            steps: Vec::new(),
        },
    );
}

/// One voice patched `midi.note` -> voice -> out (both sides), with the voice's `params` set.
fn voice_patch(kind: &str, gate: &str, note: f32, params: &[(&str, f32)]) -> PatchState {
    let mut p = PatchState::default();
    p.modules.insert(1, module("midi.note", &[("note", note)]));
    p.modules.insert(2, module(kind, params));
    p.modules.insert(3, module("out", &[]));
    wire(&mut p, (1, "gate"), (2, gate));
    wire(&mut p, (1, "velocity"), (2, "velocity"));
    wire(&mut p, (2, "out"), (3, "in"));
    p
}

/// Notes `(time s, note, velocity)` played into `p`, `secs` long, as mono samples.
fn play(p: &PatchState, hits: &[(f32, u8, u8)], secs: f32) -> Vec<f32> {
    let c = Collector::new();
    let mut e = PatchEngine::new(&c.handle(), p, SR, 8).unwrap();
    let mut out = Vec::new();
    for b in 0..(secs * SR) as usize / BLOCK {
        let t0 = (b * BLOCK) as f32 / SR;
        let t1 = ((b + 1) * BLOCK) as f32 / SR;
        for &(t, note, velocity) in hits {
            if t >= t0 && t < t1 {
                e.key_at(
                    MidiEvent::new(Source::Controller, 9, KeyEvent::On { note, velocity }),
                    ((t - t0) * SR) as usize,
                );
            }
        }
        let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
        e.process_block(&mut l, &mut r);
        out.extend_from_slice(&l);
    }
    out
}

fn write_wav(path: &std::path::Path, mono: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SR as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for s in mono {
        w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16)
            .unwrap();
    }
    w.finalize().unwrap();
}

type Set<'a> = &'a [(&'a str, f32)];

/// Each variant plays a loud hit and then a soft one, 1.4 s in all.
fn variants(dir: &std::path::Path, name: &str, kind: &str, note: u8, sets: &[Set]) {
    let mut clip = Vec::new();
    for s in sets {
        let p = voice_patch(kind, "gate", f32::from(note), s);
        clip.extend(play(&p, &[(0.05, note, 110), (0.75, note, 55)], 1.4));
    }
    write_wav(&dir.join(format!("{name}.wav")), &clip);
}

#[test]
#[ignore]
fn write_clips() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/drum-clips");
    std::fs::create_dir_all(&dir).unwrap();
    variants(
        &dir,
        "kick",
        "drum.kick",
        36,
        &[
            &[],
            &[("tune_hz", 38.0), ("decay_ms", 900.0)],
            &[("tune_hz", 62.0), ("sweep_ms", 12.0), ("decay_ms", 160.0)],
            &[("click", 0.9), ("tone", 0.9)],
            &[("drive_db", 24.0)],
            &[("sweep_st", 0.0), ("click", 0.0), ("drive_db", 0.0)],
        ],
    );
    variants(
        &dir,
        "snare",
        "drum.snare",
        38,
        &[
            &[],
            &[("snappy", 0.0)],
            &[("snappy", 1.0), ("tone", 0.9)],
            &[("tune_hz", 140.0), ("decay_ms", 600.0)],
            &[("tune_hz", 260.0), ("decay_ms", 120.0), ("drive_db", 20.0)],
        ],
    );
    variants(
        &dir,
        "clap",
        "drum.clap",
        39,
        &[
            &[],
            &[("bursts", 2.0), ("spread_ms", 5.0)],
            &[("bursts", 6.0), ("spread_ms", 14.0)],
            &[("decay_ms", 700.0), ("tone", 0.2)],
            &[("decay_ms", 90.0), ("tone", 0.9)],
        ],
    );
    variants(
        &dir,
        "tom",
        "drum.tom",
        45,
        &[
            &[("tune_hz", 90.0)],
            &[("tune_hz", 140.0)],
            &[("tune_hz", 220.0), ("decay_ms", 250.0)],
            &[("tune_hz", 330.0), ("decay_ms", 180.0), ("sweep_st", 16.0)],
        ],
    );
    // The hat: closed, open, open cut by a closed hit, tuned and toned, short closed roll.
    let hat = |params: Set, hits: &[(f32, u8, u8)], secs: f32| {
        let mut p = voice_patch("drum.hat", "closed", 42.0, params);
        p.modules.insert(4, module("midi.note", &[("note", 46.0)]));
        wire(&mut p, (4, "gate"), (2, "open"));
        play(&p, hits, secs)
    };
    let mut clip = hat(&[], &[(0.05, 42, 120), (0.25, 42, 60)], 0.6);
    clip.extend(hat(&[], &[(0.05, 46, 110)], 1.6));
    clip.extend(hat(&[], &[(0.05, 46, 110), (0.35, 42, 110)], 1.0));
    clip.extend(hat(
        &[("tune_st", -7.0), ("tone", 0.1)],
        &[(0.05, 46, 110), (0.6, 42, 100)],
        1.6,
    ));
    clip.extend(hat(
        &[("tune_st", 7.0), ("tone", 0.9), ("closed_ms", 25.0)],
        &[(0.05, 42, 110), (0.2, 42, 110), (0.4, 42, 110)],
        1.0,
    ));
    write_wav(&dir.join("hat.wav"), &clip);
}
