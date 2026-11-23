//! Every patch committed before the multi-operator work renders to exactly the audio it did
//! then. `golden_renders.txt` holds one hash per patch, recorded with the build of PR #17
//! (`d7980f5`) and, for the factory patches, unchanged since; this test renders 400 blocks of a
//! three-note chord through the real compiler and compares. Regenerate deliberately with
//! `KABL_WRITE_GOLDEN=1 cargo test -p kabl-engine --test golden_renders` (only when a sound is
//! meant to change).

use std::path::Path;

use kabl_engine::compile::compile;
use kabl_engine::golden::{chord_hash, patch_dirs};

/// FNV-1a over the bit patterns of the left and right output.
fn render_hash(dir: &Path) -> String {
    // Recorded with flush-to-zero on, like the audio callbacks.
    let _ftz = kabl_engine::ftz::FtzGuard::new();
    let log = kabl_core::load(dir).expect("loads");
    let Ok(mut c) = compile(log.state(), 48000.0, 8) else {
        return "compile-error".into();
    };
    chord_hash(&mut c, 400, 150, |_, _| {})
}

#[test]
fn committed_patches_render_as_they_did() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches");
    let golden_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden_renders.txt");
    if std::env::var_os("KABL_WRITE_GOLDEN").is_some() {
        let mut dirs = Vec::new();
        patch_dirs(&root, &mut dirs, 3);
        dirs.sort();
        let text: String = dirs
            .iter()
            .map(|d| {
                format!(
                    "{} {}\n",
                    d.strip_prefix(&root).unwrap().display(),
                    render_hash(d)
                )
            })
            .collect();
        std::fs::write(&golden_path, text).unwrap();
        return;
    }
    let golden = std::fs::read_to_string(&golden_path).unwrap();
    let mut checked = 0;
    for line in golden.lines() {
        let (name, want) = line.split_once(' ').unwrap();
        assert_eq!(
            render_hash(&root.join(name)),
            want,
            "{name} sounds different"
        );
        checked += 1;
    }
    assert!(checked >= 20, "golden list too short: {checked}");
}
