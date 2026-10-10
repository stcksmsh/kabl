//! Embedded wavetables: the `Table` asset, `Op::SetTable` with its exact inverse, and the file
//! format (replay, schema gate, validation on load).

use kabl_core::{Op, PatchLog, PatchState, Source, Table};

fn wav(n: usize) -> Vec<u8> {
    let mut b = b"RIFF".to_vec();
    b.extend(((36 + n) as u32).to_le_bytes());
    b.extend(b"WAVE");
    b.resize(44 + n, 7);
    b
}

fn table(name: &str, n: usize) -> Table {
    Table {
        name: name.into(),
        wav: wav(n),
    }
}

#[test]
fn json_round_trips_every_padding_case() {
    for n in 0..8 {
        let t = table("a.wav", n);
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("\"wav\":\""), "{json}");
        assert_eq!(serde_json::from_str::<Table>(&json).unwrap(), t);
    }
    let mut all = table("all.wav", 0);
    all.wav = (0..=255u8).cycle().take(1000).collect();
    let back: Table = serde_json::from_str(&serde_json::to_string(&all).unwrap()).unwrap();
    assert_eq!(back, all);
}

#[test]
fn bad_base64_is_rejected() {
    for bad in ["A", "AAA", "AA=A", "A===", "AAAA====", "AA!A", "A=AA"] {
        let json = format!(r#"{{"name":"a.wav","wav":"{bad}"}}"#);
        assert!(serde_json::from_str::<Table>(&json).is_err(), "{bad}");
    }
    assert!(serde_json::from_str::<Table>(r#"{"name":"a.wav","wav":"AAAA","extra":1}"#).is_err());
}

#[test]
fn validation_refuses_paths_odd_names_and_non_wav_bytes() {
    assert!(table("ok-name_1.wav", 4).validate().is_ok());
    for name in [
        "",
        ".",
        "..",
        "a/b.wav",
        "..\\x",
        "x y",
        "é.wav",
        &"a".repeat(81),
    ] {
        assert!(table(name, 4).validate().is_err(), "{name:?}");
    }
    let mut t = table("a.wav", 4);
    t.wav[0] = b'X';
    assert!(t.validate().is_err());
    let mut t = table("a.wav", 4);
    t.wav.truncate(20);
    assert!(t.validate().is_err());
    assert!(table("a.wav", kabl_core::table::MAX_TABLE_BYTES)
        .validate()
        .is_err());
}

#[test]
fn set_table_has_an_exact_inverse() {
    let mut s = PatchState::new();
    let (a, b) = (table("a.wav", 4), table("b.wav", 8));
    for op in [
        Op::SetTable {
            slot: 1,
            value: Some(a.clone()),
        },
        Op::SetTable {
            slot: 1,
            value: Some(b.clone()),
        },
        Op::SetTable {
            slot: 1,
            value: None,
        },
    ] {
        let before = s.clone();
        let inverse = s.inverse_for(&op);
        s.apply(&op);
        assert_ne!(s, before);
        s.apply(&inverse);
        assert_eq!(s, before);
        s.apply(&op);
    }
    assert!(s.tables.is_empty());
}

#[test]
fn tables_survive_save_and_load_and_old_files_still_load() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = PatchLog::new();
    log.append(
        Op::SetTable {
            slot: 2,
            value: Some(table("pad.wav", 100)),
        },
        0,
        Source::User,
    );
    kabl_core::save(dir.path(), &log).unwrap();
    let back = kabl_core::load(dir.path()).unwrap();
    assert_eq!(back.state(), log.state());
    assert_eq!(back.state().tables[&2].wav.len(), 144);
    // The checkpoint carries them too, as base64, not number arrays.
    let checkpoint = std::fs::read_to_string(dir.path().join("checkpoint.json")).unwrap();
    assert!(checkpoint.len() < 400, "{} bytes", checkpoint.len());

    // A state without tables serialises without the key, as before.
    let json = serde_json::to_string(&PatchState::new()).unwrap();
    assert!(!json.contains("tables"));
    assert!(serde_json::from_str::<PatchState>(&json)
        .unwrap()
        .tables
        .is_empty());
}

#[test]
fn a_schema_4_file_cannot_hold_tables_and_bad_slots_fail_validation() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = PatchLog::new();
    log.append(
        Op::SetTable {
            slot: 1,
            value: Some(table("a.wav", 4)),
        },
        0,
        Source::User,
    );
    kabl_core::save(dir.path(), &log).unwrap();
    std::fs::write(dir.path().join("meta.toml"), "schema_version = 4\n").unwrap();
    assert!(kabl_core::load(dir.path()).is_err());

    for slot in [0u64, 9] {
        let mut log = PatchLog::new();
        log.append(
            Op::SetTable {
                slot,
                value: Some(table("a.wav", 4)),
            },
            0,
            Source::User,
        );
        kabl_core::save(dir.path(), &log).unwrap();
        assert!(kabl_core::load(dir.path()).is_err(), "slot {slot}");
    }
}
