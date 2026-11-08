//! Custom code embedded in a `.kabl` (D11 decisions 2 and 7): round trip, one copy of identical
//! code, the 2 MiB cap, files without code, hash checking, and the old state kept.

use std::path::{Path, PathBuf};

use kabl_core::embedded::{hash_of, EmbedError, Embedded, MAX_CODE_BLOB_BYTES};
use kabl_core::format::{self, FormatError};
use kabl_core::PatchLog;

fn log() -> PatchLog {
    let dir: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/echo");
    format::load(&dir).unwrap()
}

fn wasm(tag: u8, len: usize) -> Vec<u8> {
    let mut v = b"\0asm\x01\0\0\0".to_vec();
    v.resize(len, tag);
    v
}

fn write(code: &Embedded) -> Vec<u8> {
    let mut b = Vec::new();
    format::write_kabl_full(&mut b, &log(), Some("name = \"x\"\n"), Some(code)).unwrap();
    b
}

#[test]
fn blobs_references_and_kept_state_round_trip() {
    let mut e = Embedded::new();
    let h = e.add_blob("demo.logic", 3, wasm(1, 5000)).unwrap();
    e.bind(10, h).unwrap();
    e.keep_state(10, 2, vec![9, 8, 7]).unwrap();
    let b = write(&e);
    let (back, meta, got) = format::read_kabl_full(&b[..]).unwrap();
    assert_eq!(back.state(), log().state());
    assert_eq!(meta.as_deref(), Some("name = \"x\"\n"));
    assert_eq!(got, e);
    assert_eq!(got.instance_blob(10).unwrap().version, 3);
    assert_eq!(got.instance_blob(10).unwrap().module_id, "demo.logic");
    assert_eq!(got.instance_hash(10), Some(&hash_of(&wasm(1, 5000))));
    assert_eq!(got.kept_state(10, 2), Some(&[9u8, 8, 7][..]));
}

#[test]
fn identical_code_is_stored_once() {
    let mut e = Embedded::new();
    let a = e.add_blob("demo.logic", 1, wasm(2, 4000)).unwrap();
    let b = e.add_blob("demo.logic", 1, wasm(2, 4000)).unwrap();
    assert_eq!(a, b);
    for i in 1..=4 {
        e.bind(i, a).unwrap();
    }
    assert_eq!(e.blobs().count(), 1);
    let bytes = write(&e);
    let single = bytes.len();
    let (_, _, got) = format::read_kabl_full(&bytes[..]).unwrap();
    assert_eq!(got.blobs().count(), 1);
    assert_eq!(got.refs().count(), 4);
    // Four instances cost four 40-byte references, not four copies of the code.
    let mut one = Embedded::new();
    let h = one.add_blob("demo.logic", 1, wasm(2, 4000)).unwrap();
    one.bind(1, h).unwrap();
    assert!(single < write(&one).len() + 4 * 48 + 8);
    // The same bytes under another id are a conflict, not a second copy.
    assert!(matches!(
        e.add_blob("other", 1, wasm(2, 4000)),
        Err(EmbedError::Conflict(_))
    ));
}

#[test]
fn oversize_blob_is_rejected_with_a_clear_error() {
    let mut e = Embedded::new();
    let err = e
        .add_blob("big", 1, wasm(3, MAX_CODE_BLOB_BYTES + 1))
        .unwrap_err();
    assert_eq!(
        err,
        EmbedError::TooLarge {
            size: MAX_CODE_BLOB_BYTES + 1,
            cap: MAX_CODE_BLOB_BYTES
        }
    );
    assert!(err.to_string().contains("2 MiB"), "{err}");
    // Exactly the cap is fine and survives a round trip.
    let h = e.add_blob("max", 1, wasm(3, MAX_CODE_BLOB_BYTES)).unwrap();
    e.bind(1, h).unwrap();
    let (_, _, got) = format::read_kabl_full(&write(&e)[..]).unwrap();
    assert_eq!(got.blob(&h).unwrap().bytes.len(), MAX_CODE_BLOB_BYTES);
}

#[test]
fn an_oversize_blob_frame_in_a_file_is_refused() {
    // Hand-built file: a valid one with its blob frame's length field raised past the cap.
    let mut e = Embedded::new();
    let h = e.add_blob("m", 1, wasm(4, 100)).unwrap();
    e.bind(1, h).unwrap();
    let mut b = write(&e);
    let magic = 0x184D_2A52u32.to_le_bytes();
    let at = b.windows(4).position(|w| w == magic).unwrap();
    b[at + 4..at + 8].copy_from_slice(&((MAX_CODE_BLOB_BYTES as u32) + (1 << 20)).to_le_bytes());
    assert!(matches!(
        format::read_kabl_full(&b[..]),
        Err(FormatError::Code(EmbedError::TooLarge { .. }))
    ));
}

#[test]
fn a_blob_that_does_not_match_its_hash_is_corrupt() {
    let mut e = Embedded::new();
    let h = e.add_blob("m", 1, wasm(5, 100)).unwrap();
    e.bind(1, h).unwrap();
    let mut b = write(&e);
    let magic = 0x184D_2A52u32.to_le_bytes();
    let at = b.windows(4).position(|w| w == magic).unwrap();
    // Flip a byte of the wasm (after frame header 8, hash 32, version 4, len 2, id 1).
    b[at + 8 + 39 + 20] ^= 0xff;
    assert!(matches!(
        format::read_kabl_full(&b[..]),
        Err(FormatError::Corrupt(_))
    ));
}

#[test]
fn files_without_code_load_with_none() {
    let mut plain = Vec::new();
    format::write_kabl(&mut plain, &log()).unwrap();
    let (back, meta, code) = format::read_kabl_full(&plain[..]).unwrap();
    assert_eq!(back.state(), log().state());
    assert!(meta.is_none());
    assert!(code.is_empty());
    // And a build that only knows the metadata frame still reads a file that has code.
    let mut e = Embedded::new();
    let h = e.add_blob("m", 1, wasm(6, 100)).unwrap();
    e.bind(1, h).unwrap();
    let with_code = write(&e);
    let (back, meta) = format::read_kabl_meta(&with_code[..]).unwrap();
    assert_eq!(back.state(), log().state());
    assert!(meta.is_some());
    format::read_kabl(&with_code[..]).unwrap();
}

#[test]
fn committed_factory_files_are_unchanged_by_the_new_frames() {
    // Writing without code is byte-identical to the old writer path.
    let mut a = Vec::new();
    let mut b = Vec::new();
    format::write_kabl_meta(&mut a, &log(), Some("m = 1\n")).unwrap();
    format::write_kabl_full(&mut b, &log(), Some("m = 1\n"), None).unwrap();
    assert_eq!(a, b);
}

#[test]
fn the_old_state_is_kept_across_a_version_change_and_a_save() {
    let mut e = Embedded::new();
    let h = e.add_blob("demo.bitfilter", 3, wasm(7, 200)).unwrap();
    e.bind(5, h).unwrap();
    // The file's code is v3; v4 is installed now.
    let changes = e.version_changes(|id| (id == "demo.bitfilter").then_some(4));
    assert_eq!(changes.len(), 1);
    assert_eq!(
        (changes[0].from, changes[0].to, changes[0].instance),
        (3, 4, 5)
    );
    assert_eq!(
        changes[0].notice(),
        "Custom module demo.bitfilter updated from v3 to v4: settings reset"
    );
    e.keep_state(5, 3, b"v3 settings".to_vec()).unwrap();
    // Move to v4: new blob, instance rebinds, old blob and state stay.
    let h4 = e.add_blob("demo.bitfilter", 4, wasm(8, 200)).unwrap();
    e.bind(5, h4).unwrap();
    e.keep_state(5, 4, b"v4 settings".to_vec()).unwrap();
    let (_, _, got) = format::read_kabl_full(&write(&e)[..]).unwrap();
    assert_eq!(got.kept_state(5, 3), Some(&b"v3 settings"[..]));
    assert_eq!(got.kept_state(5, 4), Some(&b"v4 settings"[..]));
    // Reopening and saving again keeps the v3 state.
    let (_, _, again) = format::read_kabl_full(&write(&got)[..]).unwrap();
    assert_eq!(again.kept_state(5, 3), Some(&b"v3 settings"[..]));
    // Same version installed: no change, nothing reset.
    assert!(got.version_changes(|_| Some(4)).is_empty());
}

#[test]
fn unreferenced_blobs_can_be_pruned_before_saving() {
    let mut e = Embedded::new();
    let used = e.add_blob("a", 1, wasm(9, 50)).unwrap();
    e.add_blob("b", 1, wasm(10, 50)).unwrap();
    e.bind(1, used).unwrap();
    e.prune_unreferenced();
    let (_, _, got) = format::read_kabl_full(&write(&e)[..]).unwrap();
    assert_eq!(got.blobs().count(), 1);
}

fn frame(magic: u32, payload: &[u8]) -> Vec<u8> {
    let mut f = magic.to_le_bytes().to_vec();
    f.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    f.extend_from_slice(payload);
    f
}

#[test]
fn a_file_with_many_kept_state_frames_is_rejected_fast() {
    let mut e = Embedded::new();
    let h = e.add_blob("m", 1, wasm(11, 100)).unwrap();
    e.bind(1, h).unwrap();
    let b = write(&e);
    let mut flood = Vec::new();
    for i in 0..200_000u64 {
        let mut p = i.to_le_bytes().to_vec();
        p.extend_from_slice(&1u32.to_le_bytes());
        flood.extend(frame(0x184D_2A54, &p));
    }
    let mut bomb = b[..20].to_vec();
    bomb.extend(flood);
    bomb.extend_from_slice(&b[20..]);
    let t = std::time::Instant::now();
    assert!(matches!(
        format::read_kabl_full(&bomb[..]),
        Err(FormatError::Corrupt(_))
    ));
    assert!(t.elapsed().as_secs() < 5, "took {:?}", t.elapsed());
    // Up to the cap loads.
    let cap = kabl_core::embedded::MAX_KEPT_STATES;
    let mut e = Embedded::new();
    for i in 0..cap as u64 {
        e.keep_state(i, 1, vec![1]).unwrap();
    }
    assert_eq!(
        e.keep_state(u64::MAX, 1, vec![1]),
        Err(EmbedError::TooManyStates)
    );
    let (_, _, got) = format::read_kabl_full(&write(&e)[..]).unwrap();
    assert_eq!(got.kept_states().count(), cap);
}

#[test]
fn a_write_over_the_file_cap_fails_with_a_clear_error() {
    let mut e = Embedded::new();
    // 33 distinct 2 MiB blobs: each is within its cap, together they pass MAX_KABL_BYTES.
    for i in 0..33u8 {
        e.add_blob(&format!("m{i}"), 1, wasm(i, MAX_CODE_BLOB_BYTES))
            .unwrap();
    }
    let mut out = Vec::new();
    let err = format::write_kabl_full(&mut out, &log(), None, Some(&e)).unwrap_err();
    assert!(matches!(err, FormatError::TooLarge(n) if n == format::MAX_KABL_BYTES));
    assert!(out.is_empty(), "nothing is written");
}

#[test]
fn references_may_come_before_their_blobs() {
    let mut e = Embedded::new();
    let h = e.add_blob("m", 1, wasm(12, 100)).unwrap();
    e.bind(7, h).unwrap();
    let b = write(&e);
    let at = |magic: u32| b.windows(4).position(|w| w == magic.to_le_bytes()).unwrap();
    let (blob_at, ref_at) = (at(0x184D_2A52), at(0x184D_2A53));
    assert!(blob_at < ref_at);
    // Swap the two frames: reference first.
    let mut swapped = b[..blob_at].to_vec();
    swapped.extend_from_slice(&b[ref_at..ref_at + 48]);
    swapped.extend_from_slice(&b[blob_at..ref_at]);
    swapped.extend_from_slice(&b[ref_at + 48..]);
    let (_, _, got) = format::read_kabl_full(&swapped[..]).unwrap();
    assert_eq!(got, e);
    // A reference to a blob that is nowhere in the file is still corrupt.
    let mut orphan = b[..blob_at].to_vec();
    orphan.extend_from_slice(&b[ref_at..]);
    assert!(matches!(
        format::read_kabl_full(&orphan[..]),
        Err(FormatError::Corrupt(_))
    ));
}
