//! D04 compatibility: renders each patch offline at a fixed setting (no edits) with the same
//! scripted keys and prints a hash of every output sample, so two builds can be compared bit
//! for bit. Uses only API that predates D04, so the same file builds on the baseline.
//!
//!     cargo run --release -p kabl-engine --example render_hash -- 20 patches/composition ...
//!
//! Arguments: seconds per patch, then patch folders. Keys: a chord held for the first half,
//! then single notes; everything released for the last quarter (release, delay and reverb
//! tails). Patches with clocks and sequencers run them from the start.

use basedrop::Collector;
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::PatchEngine;

const SR: f32 = 48000.0;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let secs: f32 = args[0].parse().expect("seconds");
    for dir in &args[1..] {
        let log = kabl_core::load(std::path::Path::new(dir)).expect("patch");
        let collector = Collector::new();
        let mut e = PatchEngine::new(&collector.handle(), log.state(), SR, 8).expect("compiles");
        let blocks = (secs * SR / BLOCK as f32) as usize;
        let (mut h, mut peak, mut energy) = (0xcbf2_9ce4_8422_2325u64, 0f32, 0f64);
        let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
        for b in 0..blocks {
            let at = |f: f32| b == (f * blocks as f32) as usize;
            if b == 0 {
                for n in [48, 55, 60, 64] {
                    e.key(KeyEvent::On {
                        note: n,
                        velocity: 100,
                    });
                }
            }
            if at(0.5) {
                for n in [48, 55, 60, 64] {
                    e.key(KeyEvent::Off { note: n });
                }
            }
            for (k, n) in [67u8, 62, 69, 57].iter().enumerate() {
                if at(0.5 + k as f32 * 0.06) {
                    e.key(KeyEvent::On {
                        note: *n,
                        velocity: 70 + 10 * k as u8,
                    });
                }
                if at(0.53 + k as f32 * 0.06) {
                    e.key(KeyEvent::Off { note: *n });
                }
            }
            e.process_block(&mut l, &mut r);
            for s in l.iter().chain(r.iter()) {
                h = (h ^ s.to_bits() as u64).wrapping_mul(0x100_0000_01b3);
                peak = peak.max(s.abs());
                energy += (*s as f64) * (*s as f64);
            }
        }
        println!(
            "{dir}: {secs} s, hash {h:016x}, peak {peak:.6}, rms {:.6}",
            (energy / (blocks * BLOCK * 2) as f64).sqrt()
        );
    }
}
