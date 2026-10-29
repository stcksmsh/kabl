//! Rewrites patch folders below schema 8 as schema 8: `load` migrates, `save` writes
//! `meta.toml`, `log.jsonl` and `checkpoint.json` again. Folders already at schema 8 are left alone.
//!
//!     cargo run -p kabl-core --example rewrite_v8 -- patches
//!
//! Each argument is a patch folder or a directory searched recursively for `meta.toml`.

use std::path::Path;

fn walk(dir: &Path, done: &mut usize) {
    if let Ok(meta) = std::fs::read_to_string(dir.join("meta.toml")) {
        if !meta.contains(&format!(
            "schema_version = {}",
            kabl_core::CURRENT_SCHEMA_VERSION
        )) {
            let log = kabl_core::load(dir).unwrap_or_else(|e| panic!("{}: {e:?}", dir.display()));
            kabl_core::save(dir, &log).unwrap_or_else(|e| panic!("{}: {e:?}", dir.display()));
            *done += 1;
            println!("rewrote {}", dir.display());
        }
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut subs: Vec<_> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    subs.sort();
    for sub in subs {
        walk(&sub, done);
    }
}

fn main() {
    let mut done = 0;
    for arg in std::env::args().skip(1) {
        walk(Path::new(&arg), &mut done);
    }
    println!("{done} folder(s) rewritten");
}
