//! `kabl cat|pack|unpack` on the real binary: a bad file is a message on stderr and exit 1, and
//! a good one prints the log, all without starting audio.
use std::process::Command;

fn kabl(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_kabl"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn a_bad_file_exits_non_zero_with_a_message() {
    let t = tempfile::tempdir().unwrap();
    let junk = t.path().join("junk.kabl");
    std::fs::write(&junk, b"junk junk junk junk junk junk").unwrap();
    let o = kabl(&["cat", junk.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(1));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(
        err.starts_with("kabl: ") && err.contains("not a readable .kabl"),
        "{err}"
    );
    assert_eq!(kabl(&["pack"]).status.code(), Some(1));
}

#[test]
fn pack_then_cat_prints_json_lines() {
    let t = tempfile::tempdir().unwrap();
    let k = t.path().join("p.kabl");
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../patches/composition");
    assert!(kabl(&["pack", dir, k.to_str().unwrap()]).status.success());
    let o = kabl(&["cat", k.to_str().unwrap()]);
    assert!(o.status.success());
    let text = String::from_utf8(o.stdout).unwrap();
    assert!(text.lines().count() > 10);
    assert!(text.lines().all(|l| l.starts_with("{\"seq\":")));
}
