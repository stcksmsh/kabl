//! Proves `default_patch()` round-trips through `kabl_core::save`/`load` exactly — the logic
//! behind `kabl`'s `--save-default`/`--patch` flags, tested directly rather than only by manually
//! running the binary. No audio device needed: this is pure file I/O + the compiler.

use kabl_core::{Op, ParamTarget, PatchLog, Source};
use kabl_engine::compile::compile;
use kabl_standalone::{default_patch, DEFAULT_VOICE_COUNT};

/// Mirrors `main.rs`'s `save_default_patch` (kept in sync manually -- this is deliberately a
/// small enough function that a shared-crate abstraction for it would be overkill, see that
/// function's own doc comment for why `standalone` doesn't depend on `kabl-ui` to reuse
/// `PatchEditor::seed_from` instead).
fn patch_to_log(patch: &kabl_core::PatchState) -> PatchLog {
    let mut log = PatchLog::new();
    for (&id, m) in &patch.modules {
        log.append(
            Op::AddModule {
                id,
                kind: m.kind.clone(),
                pos: m.pos,
            },
            0,
            Source::User,
        );
        for (name, &value) in &m.params {
            log.append(
                Op::SetParam {
                    target: ParamTarget::Module {
                        id,
                        param: name.clone(),
                    },
                    value,
                },
                0,
                Source::User,
            );
        }
    }
    for (cable_id, c) in (1u64..).zip(patch.cables.values()) {
        log.append(
            Op::Connect {
                id: cable_id,
                from: c.from.clone(),
                to: c.to.clone(),
            },
            0,
            Source::User,
        );
    }
    log
}

#[test]
fn default_patch_round_trips_through_save_and_load_exactly() {
    let dir = tempfile::tempdir().expect("tempdir");
    let original = default_patch();
    let log = patch_to_log(&original);

    kabl_core::save(dir.path(), &log).expect("save should succeed");
    let loaded = kabl_core::load(dir.path()).expect("load should succeed");

    assert_eq!(
        loaded.state(),
        &original,
        "loading a saved default_patch() should reproduce it exactly"
    );
}

#[test]
fn a_loaded_patch_compiles_and_plays_identically_to_the_original() {
    let dir = tempfile::tempdir().expect("tempdir");
    let original = default_patch();
    kabl_core::save(dir.path(), &patch_to_log(&original)).expect("save should succeed");
    let loaded = kabl_core::load(dir.path()).expect("load should succeed");

    let mut compiled_original =
        compile(&original, 48000.0, DEFAULT_VOICE_COUNT).expect("original should compile");
    let mut compiled_loaded = compile(loaded.state(), 48000.0, DEFAULT_VOICE_COUNT)
        .expect("loaded patch should compile identically");

    for _ in 0..20 {
        compiled_original.process_block();
        compiled_loaded.process_block();
        assert_eq!(
            compiled_original.left(),
            compiled_loaded.left(),
            "a round-tripped patch must produce bit-identical output to the original"
        );
    }
}

#[test]
fn loaded_log_preserves_undo_history_not_just_a_snapshot() {
    let dir = tempfile::tempdir().expect("tempdir");
    let original = default_patch();
    let log = patch_to_log(&original);
    let entry_count = log.entries().len();
    assert!(
        entry_count > 1,
        "default_patch() should produce multiple ops"
    );

    kabl_core::save(dir.path(), &log).expect("save should succeed");
    let loaded = kabl_core::load(dir.path()).expect("load should succeed");

    assert_eq!(loaded.entries().len(), entry_count);
    assert!(loaded.can_undo());
}

/// Guards against `default_patch()` losing its params (and this test suite silently stopping
/// short of exercising `SetParam` ops) if it's ever simplified down to bare defaults.
#[test]
fn default_patch_has_params_worth_round_tripping() {
    let patch = default_patch();
    let total_params: usize = patch.modules.values().map(|m| m.params.len()).sum();
    assert!(total_params > 0);
}
