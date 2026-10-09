//! Every patch shipped under `patches/` and the schema-v1 fixtures render bit-identically to
//! the build before functional cables (hashes in `golden/legacy_hashes.txt`, made at 2195de0).
//! Regenerate only for an intended sound change: `KABL_BLESS=1 cargo test -p kabl-engine --test
//! legacy_patches`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use kabl_engine::compile::compile;

fn patch_dirs() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        if dir.join("meta.toml").exists() {
            out.push(dir.to_path_buf());
        }
        let mut subs: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        subs.sort();
        for s in subs {
            walk(&s, out);
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut out = Vec::new();
    walk(&root.join("../patches"), &mut out);
    walk(&root.join("core/tests/fixtures"), &mut out);
    out
}

/// FNV-1a over the f32 bit patterns of 400 blocks: a chord on three voices, released at 250.
fn render_hash(dir: &Path) -> Option<u64> {
    let log = kabl_core::load(dir).ok()?;
    let mut c = compile(log.state(), 48000.0, 8).ok()?;
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in 0..400 {
        if b == 0 {
            for (v, n) in [(0usize, 0.0f32), (1, 4.0), (2, 7.0)] {
                c.note_on(v, n, 0.8);
            }
        }
        if b == 250 {
            for v in 0..3 {
                c.note_off(v);
            }
        }
        c.process_block();
        for s in c.left().iter().chain(c.right().iter()) {
            for byte in s.to_bits().to_le_bytes() {
                h = (h ^ byte as u64).wrapping_mul(0x0100_0000_01b3);
            }
        }
    }
    Some(h)
}

#[test]
fn shipped_patches_sound_unchanged() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut now = String::new();
    for d in patch_dirs() {
        let rel = d
            .canonicalize()
            .unwrap()
            .strip_prefix(repo.canonicalize().unwrap())
            .unwrap()
            .to_string_lossy()
            .into_owned();
        match render_hash(&d) {
            Some(h) => writeln!(now, "{rel} {h:016x}").unwrap(),
            None => writeln!(now, "{rel} unloadable").unwrap(),
        }
    }
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/legacy_hashes.txt");
    if std::env::var_os("KABL_BLESS").is_some() {
        std::fs::write(&golden, &now).unwrap();
        return;
    }
    assert_eq!(now, std::fs::read_to_string(golden).unwrap());
}
