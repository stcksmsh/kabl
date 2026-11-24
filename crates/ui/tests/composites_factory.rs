//! The read-only factory composite bank (docs/composites/FACTORY-BANK.md).
use kabl_core::{load, save, Definition};
use kabl_ui::{composites, PatchEditor};
use std::collections::BTreeSet;
use std::path::Path;

/// The default voice, every module in one composite, written to `<bank>/space/v1.json` as
/// `kabl.space` v1 with the oscillator at `base_hz`.
fn write_space(bank: &Path, base_hz: f32) {
    let mut e = PatchEditor::seed_from(&kabl_standalone::default_patch());
    let members: BTreeSet<_> = e.state().modules.keys().copied().collect();
    let id = composites::encapsulate(&mut e, members, BTreeSet::new(), "Space").unwrap();
    let osc = *e
        .state()
        .modules
        .iter()
        .find(|(_, m)| m.params.contains_key("base_hz"))
        .unwrap()
        .0;
    e.set_param(osc, "base_hz", base_hz);
    let mut pkg = composites::package(e.state(), id).unwrap();
    pkg.patch.composites.get_mut(&id).unwrap().definition = Definition {
        id: "kabl.space".into(),
        version: 1,
    };
    std::fs::create_dir_all(bank.join("space")).unwrap();
    std::fs::write(
        bank.join("space/v1.json"),
        serde_json::to_vec_pretty(&pkg).unwrap(),
    )
    .unwrap();
}

fn space(bank: &Path) -> composites::FactoryEntry {
    let entries = composites::factory_entries(bank);
    assert_eq!(entries.len(), 1);
    entries.into_iter().next().unwrap()
}

fn render(p: &kabl_core::PatchState) -> Vec<f32> {
    let mut engine = kabl_engine::compile::compile(p, 48000.0, 8).unwrap();
    engine.note_on(0, 0.0, 0.8);
    let mut audio: Vec<f32> = Vec::new();
    for _ in 0..400 {
        engine.process_block();
        audio.extend(engine.left().iter().chain(engine.right()));
    }
    assert!(audio.iter().any(|v| v.abs() > 0.001));
    audio
}

fn loaded_id(bank: &Path) -> String {
    let entry = space(bank);
    let pkg = composites::read_factory(&entry).unwrap();
    let d = &pkg.patch.composites[&pkg.root].definition;
    assert_eq!(*d, entry.definition);
    format!("{} v{}", d.id, d.version)
}

/// Also run by `factory_ids_are_the_same_in_two_loads_and_two_processes` in a child process,
/// on the parent's bank.
#[test]
fn factory_id_child() {
    let own = tempfile::tempdir().unwrap();
    let bank = std::env::var_os("KABL_TEST_FACTORY_CHILD").map_or_else(
        || {
            write_space(own.path(), 220.0);
            own.path().to_path_buf()
        },
        Into::into,
    );
    let id = loaded_id(&bank);
    assert_eq!(id, "kabl.space v1");
    println!("FACTORY_ID={id}");
}

#[test]
fn factory_ids_are_the_same_in_two_loads_and_two_processes() {
    let dir = tempfile::tempdir().unwrap();
    write_space(dir.path(), 220.0);
    let first = loaded_id(dir.path());
    assert_eq!(first, "kabl.space v1");
    assert_eq!(loaded_id(dir.path()), first);
    for _ in 0..2 {
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "factory_id_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("KABL_TEST_FACTORY_CHILD", dir.path())
            .output()
            .unwrap();
        assert!(out.status.success());
        let stdout = String::from_utf8(out.stdout).unwrap();
        let child = stdout
            .lines()
            // libtest prints `test factory_id_child ... ` on the same line.
            .find_map(|l| l.split_once("FACTORY_ID=").map(|(_, id)| id))
            .unwrap();
        assert_eq!(child, first);
    }
}

#[test]
fn every_shipped_factory_definition_inserts() {
    let bank = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../composites");
    for entry in composites::factory_entries(&bank) {
        let mut e = PatchEditor::seed_from(&kabl_core::PatchState::default());
        let pkg = composites::read_factory(&entry)
            .unwrap_or_else(|err| panic!("{}: {err}", entry.path.display()));
        let id = composites::insert(&mut e, &pkg).unwrap();
        assert_eq!(e.state().composites[&id].definition, entry.definition);
    }
}

#[test]
fn bank_lists_versions_ignores_strays_and_rejects_mismatched_files() {
    let dir = tempfile::tempdir().unwrap();
    assert!(composites::factory_entries(&dir.path().join("missing")).is_empty());
    write_space(dir.path(), 220.0);
    std::fs::copy(
        dir.path().join("space/v1.json"),
        dir.path().join("space/v2.json"),
    )
    .unwrap();
    for stray in [
        "space/v01.json",
        "space/v0.json",
        "space/notes.txt",
        "Bad.Name/v1.json",
    ] {
        std::fs::create_dir_all(dir.path().join(stray).parent().unwrap()).unwrap();
        std::fs::write(dir.path().join(stray), b"{}").unwrap();
    }
    let entries = composites::factory_entries(dir.path());
    let listed: Vec<_> = entries
        .iter()
        .map(|e| (e.definition.id.as_str(), e.definition.version))
        .collect();
    assert_eq!(listed, [("kabl.space", 1), ("kabl.space", 2)]);
    // v2's file still says v1: refused rather than inserted under the wrong identity.
    assert!(composites::read_factory(&entries[1])
        .unwrap_err()
        .contains("kabl.space v1"));
}

#[test]
fn factory_definitions_are_read_only_through_the_library_api() {
    let dir = tempfile::tempdir().unwrap();
    let bank = dir.path().join("composites");
    write_space(&bank, 220.0);
    let before = std::fs::read(bank.join("space/v1.json")).unwrap();
    let mut e = PatchEditor::seed_from(&kabl_core::PatchState::default());
    let id = composites::insert(&mut e, &composites::read_factory(&space(&bank)).unwrap()).unwrap();
    // Publishing into a separate user root, and into a root whose library is the bank itself.
    let user = tempfile::tempdir().unwrap();
    for root in [user.path(), dir.path()] {
        let err = composites::publish(e.state(), id, root).unwrap_err();
        assert!(err.contains("read-only factory definition"), "{err}");
    }
    assert!(!user.path().join("composites").exists());
    assert_eq!(std::fs::read(bank.join("space/v1.json")).unwrap(), before);
    assert_eq!(composites::factory_entries(&bank), [space(&bank)]);
}

#[test]
fn instances_keep_their_snapshot_when_the_bank_changes() {
    let dir = tempfile::tempdir().unwrap();
    write_space(dir.path(), 220.0);
    let mut e = PatchEditor::seed_from(&kabl_core::PatchState::default());
    let id = composites::insert(
        &mut e,
        &composites::read_factory(&space(dir.path())).unwrap(),
    )
    .unwrap();
    let before = render(e.state());
    let song = tempfile::tempdir().unwrap();
    save(song.path(), e.log()).unwrap();

    write_space(dir.path(), 440.0);
    let mut fresh = PatchEditor::seed_from(&kabl_core::PatchState::default());
    composites::insert(
        &mut fresh,
        &composites::read_factory(&space(dir.path())).unwrap(),
    )
    .unwrap();
    assert_ne!(render(fresh.state()), before);

    let reopened = PatchEditor::from_log(load(song.path()).unwrap());
    assert_eq!(reopened.state(), e.state());
    assert_eq!(
        reopened.state().composites[&id].definition,
        Definition {
            id: "kabl.space".into(),
            version: 1
        }
    );
    assert_eq!(render(reopened.state()), before);
}
