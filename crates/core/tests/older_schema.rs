//! A file written by an older development build (schema below 8) is refused with a clear
//! error, never migrated and never a panic.

use kabl_core::format::{self, FormatError};
use kabl_core::PatchLog;

fn msg(e: FormatError) -> String {
    match e {
        FormatError::Unsupported(m) => m,
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

#[test]
fn a_kabl_file_of_an_older_schema_is_refused() {
    let mut bytes = Vec::new();
    format::write_kabl(&mut bytes, &PatchLog::new()).unwrap();
    for old in [1u8, 7] {
        let mut b = bytes.clone();
        b[14] = old;
        let m = msg(format::read_kabl(&b[..]).unwrap_err());
        assert!(
            m.contains("older kabl development build; not supported"),
            "{m}"
        );
    }
}

#[test]
fn a_folder_of_an_older_schema_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    format::save(dir.path(), &PatchLog::new()).unwrap();
    std::fs::write(dir.path().join("meta.toml"), "schema_version = 7\n").unwrap();
    let m = msg(format::load(dir.path()).unwrap_err());
    assert!(m.contains("schema 7"), "{m}");
}

#[test]
fn a_newer_schema_is_still_unknown() {
    let dir = tempfile::tempdir().unwrap();
    format::save(dir.path(), &PatchLog::new()).unwrap();
    std::fs::write(dir.path().join("meta.toml"), "schema_version = 99\n").unwrap();
    assert!(matches!(
        format::load(dir.path()),
        Err(FormatError::UnknownSchemaVersion(99))
    ));
}
