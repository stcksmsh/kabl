//! `kabl cat`, `pack` and `unpack` (kabl_core::cli): pack then cat prints the folder's log, a
//! pack, unpack, pack cycle keeps the log, and bad input is an error message, not a panic.

use std::path::{Path, PathBuf};

use kabl_core::{cli, format};

fn factory_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/composition")
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn args(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| x.to_string()).collect()
}

fn cat(file: &Path) -> Vec<u8> {
    let mut out = Vec::new();
    cli::run(&args(&["cat", &s(file)]), &mut out).unwrap();
    out
}

fn folder_lines(dir: &Path) -> Vec<u8> {
    let mut v = Vec::new();
    for e in format::load(dir).unwrap().entries() {
        v.extend(serde_json::to_string(e).unwrap().bytes());
        v.push(b'\n');
    }
    v
}

fn run(a: &[&str]) -> Result<(), String> {
    cli::run(&args(a), &mut Vec::new())
}

#[test]
fn pack_then_cat_equals_the_folder_log() {
    let t = tempfile::tempdir().unwrap();
    let kabl = t.path().join("p.kabl");
    run(&["pack", &s(&factory_dir()), &s(&kabl)]).unwrap();
    assert_eq!(cat(&kabl), folder_lines(&factory_dir()));
}

#[test]
fn pack_unpack_pack_keeps_the_log() {
    let t = tempfile::tempdir().unwrap();
    let (a, dir, b) = (
        t.path().join("a.kabl"),
        t.path().join("d"),
        t.path().join("b.kabl"),
    );
    run(&["pack", &s(&factory_dir()), &s(&a)]).unwrap();
    run(&["unpack", &s(&a), &s(&dir)]).unwrap();
    run(&["pack", &s(&dir), &s(&b)]).unwrap();
    assert_eq!(cat(&a), cat(&b));
    assert_eq!(
        format::load(&dir).unwrap().state(),
        format::load(&factory_dir()).unwrap().state()
    );
}

#[test]
fn default_names_sit_next_to_the_input() {
    let t = tempfile::tempdir().unwrap();
    // pack DIR: DIR.kabl
    let tune = t.path().join("tune");
    run(&["pack", &s(&factory_dir()), &s(&t.path().join("src.kabl"))]).unwrap();
    run(&["unpack", &s(&t.path().join("src.kabl")), &s(&tune)]).unwrap();
    run(&["pack", &s(&tune)]).unwrap();
    assert!(t.path().join("tune.kabl").exists());
    // unpack FILE.kabl: the folder named like the file, without .kabl
    run(&["unpack", &s(&t.path().join("tune.kabl"))]).unwrap_err(); // "tune" exists
    std::fs::rename(t.path().join("tune.kabl"), t.path().join("song.kabl")).unwrap();
    run(&["unpack", &s(&t.path().join("song.kabl"))]).unwrap();
    assert!(t.path().join("song/log.jsonl").exists());
}

#[test]
fn errors_are_messages() {
    let t = tempfile::tempdir().unwrap();
    let junk = t.path().join("junk.kabl");
    std::fs::write(&junk, b"not a kabl file at all, really").unwrap();
    for bad in [
        vec!["cat"],
        vec!["cat", "a", "b"],
        vec!["pack"],
        vec!["unpack"],
        vec!["cat", "/nonexistent/x.kabl"],
        vec!["pack", "/nonexistent/dir"],
    ] {
        let e = run(&bad).unwrap_err();
        assert!(!e.is_empty(), "{bad:?}");
    }
    let e = run(&["cat", &s(&junk)]).unwrap_err();
    assert!(
        e.contains("junk.kabl") && e.contains("not a readable .kabl"),
        "{e}"
    );
    // Never overwrites.
    let k = t.path().join("k.kabl");
    run(&["pack", &s(&factory_dir()), &s(&k)]).unwrap();
    let e = run(&["pack", &s(&factory_dir()), &s(&k)]).unwrap_err();
    assert!(e.contains("already exists"), "{e}");
    let e = run(&["unpack", &s(&k), &s(t.path())]).unwrap_err();
    assert!(e.contains("already exists"), "{e}");
}
