//! The custom-code notice's gate and its remembered answer per content hash (D11 decision 2).

use kabl_core::embedded::Embedded;
use kabl_ui::code_notice::{self, Gate};
use kabl_ui::library::Library;

fn code() -> Embedded {
    let mut e = Embedded::new();
    let h = e
        .add_blob("demo.logic", 1, b"\0asm\x01\0\0\0aaaa".to_vec())
        .unwrap();
    e.bind(1, h).unwrap();
    e
}

#[test]
fn a_patch_without_code_never_asks() {
    let none = code_notice::entries(&Embedded::new());
    assert_eq!(code_notice::gate(&none, &Default::default()), Gate::Open);
}

#[test]
fn the_choice_is_remembered_per_hash_across_sessions() {
    let root = tempfile::tempdir().unwrap();
    let entries = code_notice::entries(&code());
    let mut lib = Library::open(None, root.path().to_path_buf());
    assert_eq!(
        code_notice::gate(&entries, &lib.prefs.code_trust),
        Gate::Ask
    );

    assert!(lib.remember_code_trust(&entries, true).is_none());
    assert_eq!(
        code_notice::gate(&entries, &lib.prefs.code_trust),
        Gate::Open
    );

    // A new session reads the same answer back from library.json.
    let lib = Library::open(None, root.path().to_path_buf());
    assert_eq!(
        code_notice::gate(&entries, &lib.prefs.code_trust),
        Gate::Open
    );

    // Different code (another hash) asks again.
    let mut other = Embedded::new();
    other
        .add_blob("demo.logic", 2, b"\0asm\x01\0\0\0bbbb".to_vec())
        .unwrap();
    let other = code_notice::entries(&other);
    assert_eq!(code_notice::gate(&other, &lib.prefs.code_trust), Gate::Ask);
}

#[test]
fn cancel_is_remembered_too() {
    let root = tempfile::tempdir().unwrap();
    let entries = code_notice::entries(&code());
    let mut lib = Library::open(None, root.path().to_path_buf());
    lib.remember_code_trust(&entries, false);
    let lib = Library::open(None, root.path().to_path_buf());
    assert_eq!(
        code_notice::gate(&entries, &lib.prefs.code_trust),
        Gate::Refuse
    );
}
