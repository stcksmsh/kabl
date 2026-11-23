//! Prints `<patch dir> <fnv1a hash of a fixed render>` for every patch under a directory, sorted.
//! Same render as `tests/legacy_patches.rs`: a chord on three voices, released at block 250,
//! 400 blocks. Used to prove a refactor leaves every factory patch sounding the same.
//!
//!     cargo run -p kabl-engine --example render_hashes -- patches

use std::path::{Path, PathBuf};

use kabl_engine::compile::compile;
use kabl_engine::golden::chord_hash;

fn dirs(dir: &Path, out: &mut Vec<PathBuf>) {
    if dir.join("meta.toml").exists() {
        out.push(dir.to_path_buf());
    }
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        if e.path().is_dir() {
            dirs(&e.path(), out);
        }
    }
}

fn hash(dir: &Path) -> String {
    let Ok(log) = kabl_core::load(dir) else {
        return "unloadable".into();
    };
    let Ok(mut c) = compile(log.state(), 48000.0, 8) else {
        return "uncompilable".into();
    };
    chord_hash(&mut c, 400, 250, |_, _| {})
}

fn main() {
    let root = PathBuf::from(std::env::args().nth(1).expect("patches dir"));
    let mut v = Vec::new();
    dirs(&root, &mut v);
    v.sort();
    for d in v {
        println!("{} {}", d.strip_prefix(&root).unwrap().display(), hash(&d));
    }
}
