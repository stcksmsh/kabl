//! Every patch committed before the multi-operator work renders to exactly the audio it did
//! then. `golden_renders.txt` holds one hash per patch, recorded with the build of PR #17
//! (`d7980f5`) and, for the factory patches, unchanged since; this test renders 400 blocks of a
//! three-note chord through the real compiler and compares. Regenerate deliberately with
//! `KABL_WRITE_GOLDEN=1 cargo test -p kabl-engine --test golden_renders` (only when a sound is
//! meant to change).

use std::path::{Path, PathBuf};

use kabl_engine::compile::compile;

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

/// FNV-1a over the bit patterns of the left and right output.
fn render_hash(dir: &Path) -> String {
    // Recorded with flush-to-zero on, like the audio callbacks.
    let _ftz = kabl_engine::ftz::FtzGuard::new();
    let log = kabl_core::load(dir).expect("loads");
    let Ok(mut c) = compile(log.state(), 48000.0, 8) else {
        return "compile-error".into();
    };
    let mut h = 0xcbf29ce484222325u64;
    let mut mix = |v: f32| {
        for b in v.to_bits().to_le_bytes() {
            h = (h ^ b as u64).wrapping_mul(0x100000001b3);
        }
    };
    for b in 0..400 {
        if b == 0 {
            for (v, n) in [(0usize, 0.0f32), (1, 4.0), (2, 7.0)] {
                c.note_on(v, n, 0.8);
            }
        }
        if b == 150 {
            for v in 0..3 {
                c.note_off(v);
            }
        }
        c.process_block();
        c.left().iter().chain(c.right()).for_each(|&s| mix(s));
    }
    format!("{h:016x}")
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

/// Every patch the Sounds browser lists: patch folders one or two levels under `patches/`
/// (the browser reads the packed `.kabl` files at the top and one folder down; deeper folders
/// such as `sound-engines/demos/*` are not listed).
fn browser_corpus(root: &Path) -> Vec<String> {
    let mut dirs = Vec::new();
    patch_dirs(root, &mut dirs, 2);
    let mut names: Vec<String> = dirs
        .iter()
        .map(|d| d.strip_prefix(root).unwrap().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// The browser corpus renders to the recorded hashes. The render is the standard chord only:
/// cues and macros are never triggered, so a change that only affects them is not caught here.
/// Regenerate with
/// `KABL_WRITE_GOLDEN=1 cargo test -p kabl-engine --test golden_renders browser_corpus`
/// (run only that test: the other writer overwrites the curated list).
#[test]
fn browser_corpus_renders_as_it_did() {
    const COUNT: usize = 129;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches");
    let golden_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden_corpus.txt");
    let names = browser_corpus(&root);
    if std::env::var_os("KABL_WRITE_GOLDEN").is_some() {
        let text: String = names
            .iter()
            .map(|n| format!("{n} {}\n", render_hash(&root.join(n))))
            .collect();
        std::fs::write(&golden_path, text).unwrap();
        return;
    }
    assert_eq!(names.len(), COUNT, "browser corpus size changed");
    let golden = std::fs::read_to_string(&golden_path).unwrap();
    let recorded: Vec<(&str, &str)> = golden.lines().map(|l| l.split_once(' ').unwrap()).collect();
    assert_eq!(
        recorded.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
        names,
        "golden_corpus.txt and the patches folder list different patches"
    );
    for (name, want) in recorded {
        assert_eq!(
            render_hash(&root.join(name)),
            want,
            "{name} sounds different"
        );
    }
}
