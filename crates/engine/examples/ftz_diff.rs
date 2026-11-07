//! Renders every golden patch (the `golden_renders` recipe: 400 blocks, a three-note chord,
//! release at block 150) with flush-to-zero off and on, then prints the max |difference| in
//! dBFS and the render time of each run.
//!
//!     cargo run --release -p kabl-engine --example ftz_diff -- patches
//!
//! A third run holds the same chord for 27 s so decaying tails reach the subnormal range. Also renders the self-oscillation cases: a patch with no key held (so only the patch itself
//! can make sound) is compared the same way.

use std::path::{Path, PathBuf};
use std::time::Instant;

use kabl_engine::compile::compile;
use kabl_engine::ftz::FtzGuard;

fn patch_dirs(root: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if root.join("log.jsonl").is_file() && root.join("meta.toml").is_file() {
        out.push(root.to_path_buf());
        return;
    }
    if depth == 0 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(root) else {
        return;
    };
    for e in rd.flatten() {
        if e.path().is_dir() {
            patch_dirs(&e.path(), out, depth - 1);
        }
    }
}

fn render(dir: &Path, ftz: bool, notes: bool, blocks: usize) -> Option<(Vec<f32>, f64)> {
    let log = kabl_core::load(dir).ok()?;
    let mut c = compile(log.state(), 48000.0, 8).ok()?;
    let _guard = ftz.then(FtzGuard::new);
    let mut out = Vec::new();
    let start = Instant::now();
    for b in 0..blocks {
        if b == 0 && notes {
            for (v, n) in [(0usize, 0.0f32), (1, 4.0), (2, 7.0)] {
                c.note_on(v, n, 0.8);
            }
        }
        if b == 150 && notes {
            for v in 0..3 {
                c.note_off(v);
            }
        }
        c.process_block();
        out.extend_from_slice(c.left());
        out.extend_from_slice(c.right());
    }
    Some((out, start.elapsed().as_secs_f64() * 1e3))
}

fn dbfs(x: f32) -> f64 {
    if x == 0.0 {
        f64::NEG_INFINITY
    } else {
        20.0 * (x as f64).log10()
    }
}

fn main() {
    let root = PathBuf::from(std::env::args().nth(1).expect("patch root"));
    let mut dirs = Vec::new();
    patch_dirs(&root, &mut dirs, 3);
    dirs.sort();
    println!("patch\tmax_diff_dBFS\tpeak_dBFS\tms_off\tms_on\tmode");
    for d in dirs {
        for (notes, blocks) in [(true, 400), (false, 400), (true, 20000)] {
            // Warm up once so the first timed run is not paying for page faults.
            let _ = render(&d, false, notes, blocks);
            let (Some((a, ta)), Some((b, tb))) = (
                render(&d, false, notes, blocks),
                render(&d, true, notes, blocks),
            ) else {
                continue;
            };
            let diff = a
                .iter()
                .zip(&b)
                .map(|(x, y)| (x - y).abs())
                .fold(0f32, f32::max);
            let peak = a.iter().fold(0f32, |m, x| m.max(x.abs()));
            println!(
                "{}\t{:.1}\t{:.1}\t{ta:.2}\t{tb:.2}\t{}",
                d.strip_prefix(&root).unwrap().display(),
                dbfs(diff),
                dbfs(peak),
                match (notes, blocks) {
                    (true, 400) => "chord",
                    (false, _) => "no-input",
                    _ => "long",
                }
            );
        }
    }
}
