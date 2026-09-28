//! Offline D05 baseline at 48 kHz, four 64-frame blocks per callback, eight voices.
use std::time::Instant;
use basedrop::Collector;
use kabl_engine::compile::compile;
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::PatchEngine;

fn main() {
    const CALLBACKS: usize = 1200;
    for dir in ["patches/init-keyboard", "patches/sound-palette", "patches/composition"] {
        let patch = kabl_core::load(std::path::Path::new(dir)).unwrap();
        let state = patch.state();
        let graph = compile(state, 48_000.0, 8).unwrap();
        let (instances, voice_instances, steps) = graph.profile_counts();
        let mut times = Vec::new();
        for run in 0..5 {
            let collector = Collector::new();
            let mut e = PatchEngine::new(&collector.handle(), state, 48_000.0, 8).unwrap();
            let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
            for n in 0..CALLBACKS {
                if n % 188 == 0 {
                    for note in [48, 55, 60, 64] { e.key(KeyEvent::On { note, velocity: 90 }); }
                } else if n % 188 == 150 { e.key(KeyEvent::AllOff); }
                let start = Instant::now();
                for _ in 0..4 { e.process_block(&mut l, &mut r); }
                if run > 0 { times.push(start.elapsed().as_nanos() as u64); }
            }
        }
        times.sort_unstable();
        let p = |v: f64| times[((times.len() - 1) as f64 * v).round() as usize] as f64 / 1000.0;
        println!("{dir}: modules={} instances={} voice_instances={} steps={} buffers={} callbacks={} p50={:.1}us p99={:.1}us max={:.1}us mean={:.1}us", state.modules.len(), instances, voice_instances, steps, graph.buffer_count(), times.len(), p(0.5), p(0.99), p(1.0), times.iter().sum::<u64>() as f64 / times.len() as f64 / 1000.0);
    }
}
