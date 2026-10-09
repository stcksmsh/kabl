//! Every patch that shipped before functional cables (listed in `golden/legacy_hashes.txt`,
//! rendered at 2195de0: `patches/` and the schema-v1 fixtures) still renders bit-identically.
//! Patches added later are not listed and not checked here.

use std::path::Path;

use kabl_engine::compile::compile;

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
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/legacy_hashes.txt");
    let text = std::fs::read_to_string(golden).unwrap();
    assert!(text.lines().count() >= 24);
    for line in text.lines() {
        let (name, want) = line.split_once(' ').unwrap();
        let got =
            render_hash(&repo.join(name)).map_or("unloadable".to_string(), |h| format!("{h:016x}"));
        assert_eq!(got, want, "{name} no longer renders as before");
    }
}
