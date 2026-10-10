//! Schema-v1 patches written by the baseline build (commit e6db377, `tests/fixtures/`) load
//! unchanged and gain exact undo: the v1 files store lossy inverses (`0.0` for absent params),
//! which the loader now recomputes from the replayed state.

use std::path::Path;

use kabl_core::{load, save, PatchState};

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn v1_fixtures_load_to_their_saved_checkpoint() {
    for name in ["v1_default", "v1_edited"] {
        let log = load(&fixture(name)).expect("v1 loads");
        let checkpoint: PatchState = serde_json::from_str(
            &std::fs::read_to_string(fixture(name).join("checkpoint.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            log.state(),
            &checkpoint,
            "{name}: replay equals baseline checkpoint"
        );
    }
}

#[test]
fn v1_history_undoes_to_empty_exactly() {
    let mut log = load(&fixture("v1_edited")).unwrap();
    let n = log.entries().len();
    for k in (0..n).rev() {
        assert!(log.undo());
        let mut replay = PatchState::new();
        for e in &log.all_entries()[..k] {
            replay.apply(&e.op);
        }
        assert_eq!(log.state(), &replay, "undo step to prefix {k}");
    }
    assert_eq!(log.state(), &PatchState::new());
}

#[test]
fn resaving_a_v1_patch_writes_the_current_version_that_loads_identically() {
    let log = load(&fixture("v1_edited")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    save(dir.path(), &log).unwrap();
    let meta = std::fs::read_to_string(dir.path().join("meta.toml")).unwrap();
    assert!(meta.contains(&format!("schema_version = {}", kabl_core::CURRENT_SCHEMA_VERSION)));
    assert_eq!(load(dir.path()).unwrap().state(), log.state());
}
