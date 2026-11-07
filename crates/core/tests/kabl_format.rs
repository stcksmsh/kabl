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

/// What ships: `kabl pack-tree` over `patches/` gives one .kabl per folder, and each loads to the
/// folder's state, entries and `sound.toml` text. The app reads only these files.
#[test]
fn the_packed_factory_equals_its_folders() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches");
    let out = tempfile::tempdir().unwrap();
    let n = format::pack_tree(&src, out.path()).unwrap();
    assert_eq!(n, factory().len());
    for dir in factory() {
        let rel = dir.strip_prefix(&src).unwrap();
        let file = out.path().join(rel).with_extension("kabl");
        let (log, meta) = format::load_kabl_meta(&file).unwrap();
        let folder = format::load(&dir).unwrap();
        assert_eq!(log.state(), folder.state(), "{}", dir.display());
        assert_eq!(log.entries(), folder.entries(), "{}", dir.display());
        assert_eq!(
            meta,
            std::fs::read_to_string(dir.join("sound.toml")).ok(),
            "{}",
            dir.display()
        );
    }
}

#[test]
fn metadata_round_trips_and_a_plain_reader_skips_it() {
    let (log, _) = one();
    let meta = "name = \"Warm Pad\"\ntags = [\"a\"]\n";
    let mut b = Vec::new();
    format::write_kabl_meta(&mut b, &log, Some(meta)).unwrap();
    let (back, got) = format::read_kabl_meta(&b[..]).unwrap();
    assert_eq!((back.state(), got.as_deref()), (log.state(), Some(meta)));
    // read_kabl ignores the frame; a file without one reads as no metadata.
    assert_eq!(format::read_kabl(&b[..]).unwrap().state(), log.state());
    let plain = bytes(&log);
    assert_eq!(format::read_kabl_meta(&plain[..]).unwrap().1, None);
    // The metadata frame is a skippable zstd frame: zstd -d style readers pass over it.
    assert!(b.len() > plain.len());
    // A cut metadata frame is refused.
    assert!(matches!(
        format::read_kabl_meta(&b[..24]),
        Err(FormatError::Corrupt(_))
    ));
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
    d[16] = 9;
    assert!(matches!(err(&d), FormatError::UnknownDictionary(9)));
}

#[test]
fn the_embedded_dictionary_has_the_id_the_writer_writes() {
    // A zstd dictionary starts with magic 0xEC30A437 and its id, both little-endian.
    let d = format::KABL_DICT_1;
    assert_eq!(&d[..4], &0xEC30_A437u32.to_le_bytes());
    assert_eq!(
        u32::from_le_bytes(d[4..8].try_into().unwrap()),
        format::KABL_DICT_ID
    );
    let (log, b) = one();
    assert_eq!(u32::from_le_bytes(b[16..20].try_into().unwrap()), 1);
    assert_eq!(format::read_kabl(&b[..]).unwrap().entries(), log.entries());
    // The frame really uses the dictionary: the same bytes under id 0 do not decode.
    let mut none = b;
    none[16] = 0;
    assert!(matches!(err(&none), FormatError::Corrupt(_)));
}

#[test]
fn an_id_0_file_without_a_dictionary_still_loads() {
    let log = format::load(&factory()[0]).unwrap();
    let mut lines = String::new();
    for e in log.entries() {
        lines.push_str(&serde_json::to_string(e).unwrap());
        lines.push('\n');
    }
    let (_, current) = one();
    let mut old = current[..16].to_vec();
    old.extend_from_slice(&0u32.to_le_bytes());
    old.extend(zstd::stream::encode_all(lines.as_bytes(), 19).unwrap());
    let back = format::read_kabl(&old[..]).unwrap();
    assert_eq!(back.state(), log.state());
    assert_eq!(back.entries(), log.entries());
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
