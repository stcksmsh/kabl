//! Where the piece's callback time goes: `patches/sound-palette` steady (a 6-note chord, all
//! layers up) with one part removed at a time. Not a benchmark of record; a breakdown.
//! `cargo run --release -p kabl-ui --example bench_palette_parts`
use basedrop::Collector;
use kabl_core::{ModuleId, PatchState};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::PatchEngine;
use std::time::Instant;

fn median(p: &PatchState) -> f64 {
    let c = Collector::new();
    let h = c.handle();
    let mut e = PatchEngine::new(&h, p, 48000.0, 8).unwrap();
    for n in [40, 52, 59, 62, 66, 71] {
        e.key(KeyEvent::On { note: n, velocity: 100 });
    }
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    for _ in 0..3000 {
        e.process_block(&mut l, &mut r);
    }
    let mut v: Vec<f64> = (0..3000)
        .map(|_| {
            let t = Instant::now();
            for _ in 0..4 {
                e.process_block(&mut l, &mut r);
            }
            t.elapsed().as_secs_f64() * 1e6
        })
        .collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/sound-palette");
    let mut p = kabl_core::load(&dir).unwrap().state().clone();
    for m in p.modules.values_mut() {
        if m.kind == "seq" {
            m.params.insert("bank".into(), 1.0);
        }
    }
    println!("whole piece: {:.0} µs per 256 frames", median(&p));
    // Each keyboard layer's chain: remove its midi.in (the chain then runs once, silent).
    for (name, midi) in [("breath", 36), ("strings", 47), ("pad", 58), ("lead", 70)] {
        let mut q = p.clone();
        q.modules.remove(&(midi as ModuleId));
        q.cables.retain(|_, c| c.from.module_id() != midi && c.to.module_id() != midi);
        println!("without the {name} layer's voices: {:.0} µs", median(&q));
    }
    for (name, ids) in [
        ("drum room", &[91u64][..]),
        ("plate", &[88]),
        ("chorus", &[84]),
        ("echo", &[85]),
        ("drums", &[4, 5, 6]),
        ("bass sequencer", &[27]),
    ] {
        let mut q = p.clone();
        for id in ids {
            q.modules.remove(id);
            q.cables.retain(|_, c| c.from.module_id() != *id && c.to.module_id() != *id);
        }
        println!("without the {name}: {:.0} µs", median(&q));
    }
}
