//! Research benchmark (docs/format/KABL-FILE-RESEARCH.md): time full-replay `format::load`
//! and a checkpoint.json parse for every patch folder under a root. Not shipped.
use std::path::{Path, PathBuf};
use std::time::Instant;

fn walk(d: &Path, out: &mut Vec<PathBuf>) {
    if d.join("meta.toml").exists() && d.join("log.jsonl").exists() {
        out.push(d.to_path_buf());
    }
    if let Ok(rd) = std::fs::read_dir(d) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() && p.file_name().is_some_and(|n| n != "target" && n != ".git") {
                walk(&p, out);
            }
        }
    }
}

fn main() {
    let root = std::env::args().nth(1).expect("root");
    let mut dirs = Vec::new();
    walk(Path::new(&root), &mut dirs);
    let mut times: Vec<(f64, PathBuf)> = Vec::new();
    for pass in 0..2 {
        let t0 = Instant::now();
        for d in &dirs {
            let t = Instant::now();
            let _ = kabl_core::format::load(d);
            if pass == 1 {
                times.push((t.elapsed().as_secs_f64(), d.clone()));
            }
        }
        println!("pass {pass}: {:.1} ms total", t0.elapsed().as_secs_f64() * 1e3);
    }
    times.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    for (t, d) in times.iter().take(5) {
        println!("warm {:.2} ms {}", t * 1e3, d.display());
    }
    let (mut replay, mut snap, mut ok) = (0.0f64, 0.0f64, 0);
    let mut worst = (0.0f64, PathBuf::new(), 0usize);
    for d in &dirs {
        let t = Instant::now();
        let Ok(log) = kabl_core::format::load(d) else {
            continue;
        };
        let r = t.elapsed().as_secs_f64();
        let n = log.entries().len();
        replay += r;
        if r > worst.0 {
            worst = (r, d.clone(), n);
        }
        if let Ok(text) = std::fs::read_to_string(d.join("checkpoint.json")) {
            let t = Instant::now();
            let _: serde_json::Value = serde_json::from_str(&text).unwrap();
            snap += t.elapsed().as_secs_f64();
        }
        ok += 1;
    }
    println!(
        "patches {ok}  replay total {:.1} ms (mean {:.2} ms)  checkpoint parse (Value, lower bound) total {:.1} ms (mean {:.2} ms)",
        replay * 1e3, replay * 1e3 / ok as f64, snap * 1e3, snap * 1e3 / ok as f64
    );
    println!(
        "slowest replay {:.2} ms: {} ({} entries)",
        worst.0 * 1e3,
        worst.1.display(),
        worst.2
    );
}
