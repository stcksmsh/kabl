//! D04 transition comparison, offline: one knob change in a playing patch, delivered the old
//! way (a compiled graph, crossfaded in with state carried: what every edit did before D04) and
//! the new way (a runtime value: ramped or set in the playing graph). Same keys, same seeds.
//! Prints the output before the change (must be identical), the largest difference in the
//! first 50 ms after it, and the RMS difference in later windows.
//!
//!     cargo run --release -p kabl-engine --example transition_compare
//!
//! Not a listening test: it shows where and for how long the two paths differ.

use basedrop::Collector;
use kabl_core::PatchState;
use kabl_engine::compile::compile;
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::PatchEngine;
use kabl_engine::runtime::{runtime_changes, ParamSet};

const SR: f32 = 48000.0;

fn render(patch: &PatchState, edited: &PatchState, runtime: bool, secs: f32) -> Vec<f32> {
    let collector = Collector::new();
    let h = collector.handle();
    let mut e = PatchEngine::new(&h, patch, SR, 8).unwrap();
    for n in [48, 55, 60, 64] {
        e.key(KeyEvent::On {
            note: n,
            velocity: 100,
        });
    }
    let blocks = (secs * SR / BLOCK as f32) as usize;
    let change = blocks / 2;
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    let mut out = Vec::with_capacity(blocks * BLOCK);
    for b in 0..blocks {
        if b == change {
            if runtime {
                for (k, (t, v)) in runtime_changes(patch, edited)
                    .expect("a runtime change")
                    .into_iter()
                    .enumerate()
                {
                    e.set(&ParamSet {
                        rev: 1 + k as u64,
                        target: t,
                        value: v,
                    });
                }
            } else {
                let g = compile(edited, SR, 8).unwrap();
                e.finish_swap(&h, g);
            }
        }
        e.process_block(&mut l, &mut r);
        out.extend(l.iter().zip(&r).map(|(a, b)| a + b));
    }
    out
}

fn main() {
    let cases: &[(&str, u64, &str, f32)] = &[
        ("patches/init-keyboard", 3, "cutoff_hz", 600.0),
        ("patches/init-keyboard", 6, "sustain", 0.3),
        ("patches/palette/pad", 0, "", 0.0),
        ("patches/composition", 3, "m1", 0.9),
        ("patches/composition", 32, "level1", 0.2),
        ("patches/echo", 16, "feedback", 80.0),
    ];
    for &(dir, id, param, value) in cases {
        let patch = kabl_core::load(std::path::Path::new(dir))
            .unwrap()
            .state()
            .clone();
        let (id, param, value) = if param.is_empty() {
            // The pad's filter cutoff, whatever its id.
            let (&fid, _) = patch
                .modules
                .iter()
                .find(|(_, m)| m.kind.starts_with("filter"))
                .unwrap();
            (fid, "cutoff_hz", 900.0)
        } else {
            (id, param, value)
        };
        let mut edited = patch.clone();
        edited
            .modules
            .get_mut(&id)
            .unwrap()
            .params
            .insert(param.into(), value);
        let a = render(&patch, &edited, false, 4.0);
        let b = render(&patch, &edited, true, 4.0);
        let change = a.len() / 2;
        let before = a[..change] == b[..change];
        let ms = |n: f32| (n / 1000.0 * SR) as usize;
        let max50 = (change..change + ms(50.0))
            .map(|i| (a[i] - b[i]).abs())
            .fold(0f32, f32::max);
        let peak = a.iter().fold(0f32, |m, x| m.max(x.abs()));
        let rms = |from: usize, to: usize| {
            let d: f64 = (from..to.min(a.len()))
                .map(|i| ((a[i] - b[i]) as f64).powi(2))
                .sum();
            (d / (to.min(a.len()) - from).max(1) as f64).sqrt()
        };
        println!(
            "{dir} #{id} {param} -> {value}: before identical {before}; after: max |diff| in \
             50 ms {max50:.5} (output peak {peak:.3}); rms diff 0-50 ms {:.6}, 50-500 ms {:.6}, \
             0.5-1.5 s {:.6}",
            rms(change, change + ms(50.0)),
            rms(change + ms(50.0), change + ms(500.0)),
            rms(change + ms(500.0), change + ms(1500.0)),
        );
    }
}
