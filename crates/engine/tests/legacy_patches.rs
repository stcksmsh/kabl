//! Every patch that shipped before functional cables (listed in `golden/legacy_hashes.txt`,
//! rendered at 2195de0: `patches/`) still renders bit-identically.
//! Patches added later are not listed and not checked here.

use std::path::Path;

use kabl_engine::compile::compile;
use kabl_engine::golden::chord_hash;

/// FNV-1a over the f32 bit patterns of 400 blocks: a chord on three voices, released at 250.
fn render_hash(dir: &Path) -> Option<String> {
    let log = kabl_core::load(dir).ok()?;
    let mut c = compile(log.state(), 48000.0, 8).ok()?;
    Some(chord_hash(&mut c, 400, 250, |_, _| {}))
}

#[test]
fn shipped_patches_sound_unchanged() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/legacy_hashes.txt");
    let text = std::fs::read_to_string(golden).unwrap();
    assert!(text.lines().count() >= 22);
    for line in text.lines() {
        let (name, want) = line.split_once(' ').unwrap();
        let got = render_hash(&repo.join(name)).unwrap_or_else(|| "unloadable".to_string());
        assert_eq!(got, want, "{name} no longer renders as before");
    }
}
