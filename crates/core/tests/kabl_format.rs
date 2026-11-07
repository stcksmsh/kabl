//! The `.kabl` container: every factory patch round-trips to the state its folder loads to,
//! and damaged, foreign or oversize files are refused with a `FormatError`.

use std::path::{Path, PathBuf};

use kabl_core::format::{self, FormatError};
use kabl_core::PatchLog;

fn patch_dirs(dir: &Path, out: &mut Vec<PathBuf>) {
    if dir.join("meta.toml").exists() {
        out.push(dir.to_path_buf());
    }
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        if e.path().is_dir() {
            patch_dirs(&e.path(), out);
        }
    }
}

fn factory() -> Vec<PathBuf> {
    let mut v = Vec::new();
    patch_dirs(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches"),
        &mut v,
    );
    v.sort();
    assert!(v.len() > 100, "factory patches not found");
    v
}

fn bytes(log: &PatchLog) -> Vec<u8> {
    let mut b = Vec::new();
    format::write_kabl(&mut b, log).unwrap();
    b
}

fn one() -> (PatchLog, Vec<u8>) {
    let log = format::load(&factory()[0]).unwrap();
    let b = bytes(&log);
    (log, b)
}

#[test]
fn every_factory_patch_round_trips() {
    let (mut before, mut after) = (0, 0);
    for dir in factory() {
        let folder = format::load(&dir).unwrap();
        let b = bytes(&folder);
        let kabl = format::read_kabl(&b[..]).unwrap();
        assert_eq!(folder.state(), kabl.state(), "{}", dir.display());
        assert_eq!(folder.entries(), kabl.entries(), "{}", dir.display());
        assert_eq!(folder.can_undo(), kabl.can_undo(), "{}", dir.display());
        for f in ["log.jsonl", "checkpoint.json", "meta.toml"] {
            before += std::fs::metadata(dir.join(f)).unwrap().len();
        }
        after += b.len() as u64;
    }
    println!("corpus bytes: folder {before}, kabl {after}");
}

#[test]
fn save_and_load_use_a_path_and_leave_no_tmp() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.kabl");
    let (log, _) = one();
    format::save_kabl(&path, &log).unwrap();
    assert_eq!(format::load_kabl(&path).unwrap().state(), log.state());
    assert!(!dir.path().join("a.kabl.tmp").exists());
}

/// Accepts every write, fails on flush: what a full disk does to a buffered writer.
struct FailsOnFlush;
impl std::io::Write for FailsOnFlush {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::other("disk full"))
    }
}

#[test]
fn a_failed_flush_fails_the_write() {
    let (log, _) = one();
    assert!(matches!(
        format::write_kabl(FailsOnFlush, &log),
        Err(FormatError::Io(_))
    ));
}

fn err(b: &[u8]) -> FormatError {
    format::read_kabl(b).expect_err("must be refused")
}

#[test]
fn a_truncated_file_is_refused() {
    let (_, b) = one();
    for n in [0, 10, 20, b.len() / 2, b.len() - 1] {
        assert!(format::read_kabl(&b[..n]).is_err(), "cut at {n}");
    }
}

#[test]
fn a_flipped_byte_is_refused() {
    let (_, mut b) = one();
    let i = b.len() - 6; // inside the checksum or the last block
    b[i] ^= 0x40;
    assert!(matches!(err(&b), FormatError::Corrupt(_)));
}

#[test]
fn a_wrong_magic_is_refused() {
    let (_, mut b) = one();
    b[8] = b'X';
    assert!(matches!(err(&b), FormatError::BadMagic));
    assert!(matches!(err(b"{\"not\":\"kabl\"}"), FormatError::BadMagic));
}

#[test]
fn a_future_version_is_refused() {
    let (_, b) = one();
    let mut c = b.clone();
    c[12] = 2;
    assert!(matches!(err(&c), FormatError::UnknownContainerVersion(2)));
    let mut s = b.clone();
    s[14] = 200;
    assert!(matches!(err(&s), FormatError::UnknownSchemaVersion(200)));
    let mut d = b;
    d[16] = 1;
    assert!(matches!(err(&d), FormatError::UnknownDictionary(1)));
}

#[test]
fn an_oversize_decode_is_refused() {
    let big = factory()
        .into_iter()
        .max_by_key(|d| std::fs::metadata(d.join("log.jsonl")).unwrap().len())
        .unwrap();
    let b = bytes(&format::load(&big).unwrap());
    // Decoded log over the cap, encoded file just under it.
    assert!(matches!(
        format::read_kabl_limited(&b[..], b.len()),
        Err(FormatError::TooLarge(_))
    ));
    // Encoded file over the cap.
    assert!(matches!(
        format::read_kabl_limited(&b[..], 30),
        Err(FormatError::TooLarge(30))
    ));
}
