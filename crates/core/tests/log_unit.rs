use kabl_core::{format, Op, ParamTarget, PatchLog, PortRef, Source, Vec2};

#[test]
fn consecutive_set_param_within_window_coalesces() {
    let mut log = PatchLog::new();
    log.append(
        Op::AddModule {
            id: 1,
            kind: "osc.va".into(),
            pos: Vec2 { x: 0.0, y: 0.0 },
        },
        0,
        Source::User,
    );
    let target = ParamTarget::Module {
        id: 1,
        param: "freq".into(),
    };
    log.append(
        Op::SetParam {
            target: target.clone(),
            value: 100.0,
        },
        1000,
        Source::User,
    );
    log.append(
        Op::SetParam {
            target: target.clone(),
            value: 200.0,
        },
        1100,
        Source::User,
    );
    log.append(
        Op::SetParam {
            target: target.clone(),
            value: 300.0,
        },
        1200,
        Source::User,
    );

    // Three appends after AddModule: only one entry, holding the latest value.
    assert_eq!(
        log.entries().len(),
        2,
        "SetParam entries should have coalesced into one"
    );
    assert_eq!(
        log.state().modules[&1].params["freq"],
        300.0,
        "coalesced entry should hold the last value"
    );

    // Undo the coalesced SetParam entry: goes straight back to pre-SetParam state (the merged
    // entry's inverse is the value from *before the first* of the three calls), not one step
    // back through 300 -> 200 -> 100. The param didn't exist before, so undo removes it again
    // (`UnsetParam`) rather than inventing a value.
    log.undo();
    assert!(
        !log.state().modules[&1].params.contains_key("freq"),
        "single undo of the coalesced entry should restore the absent param"
    );
}

#[test]
fn set_param_outside_window_does_not_coalesce() {
    let mut log = PatchLog::new();
    log.append(
        Op::AddModule {
            id: 1,
            kind: "osc.va".into(),
            pos: Vec2 { x: 0.0, y: 0.0 },
        },
        0,
        Source::User,
    );
    let target = ParamTarget::Module {
        id: 1,
        param: "freq".into(),
    };
    log.append(
        Op::SetParam {
            target: target.clone(),
            value: 100.0,
        },
        0,
        Source::User,
    );
    log.append(
        Op::SetParam {
            target,
            value: 200.0,
        },
        1000, // 1000ms later, past the 250ms coalesce window
        Source::User,
    );

    assert_eq!(log.entries().len(), 3);
}

/// Documents a known, intentional simplification (see docs/decisions.md "core: op log inverse
/// simplifications"): `RemoveModule`'s inverse is a single `AddModule { id, kind, pos }` — it
/// cannot carry an arbitrary param map back in one `Op`. Undoing a remove restores the module
/// with its kind and position, but not the params it had. This does NOT break the undo/redo
/// round-trip property (see replay_proptest.rs) because forward `redo` always re-applies the
/// exact stored `Op`, not a value derived from the (possibly lossy) undo state.
#[test]
fn undo_of_remove_module_restores_position_and_params() {
    let mut log = PatchLog::new();
    log.append(
        Op::AddModule {
            id: 1,
            kind: "osc.va".into(),
            pos: Vec2 { x: 3.0, y: 4.0 },
        },
        0,
        Source::User,
    );
    log.append(
        Op::SetParam {
            target: ParamTarget::Module {
                id: 1,
                param: "freq".into(),
            },
            value: 440.0,
        },
        100,
        Source::User,
    );
    log.append(Op::RemoveModule { id: 1 }, 200, Source::User);
    assert!(!log.state().modules.contains_key(&1));

    log.undo(); // undoes RemoveModule
    let restored = &log.state().modules[&1];
    assert_eq!(
        restored.pos,
        Vec2 { x: 3.0, y: 4.0 },
        "position is restored"
    );
    assert_eq!(restored.params["freq"], 440.0, "params are restored");
}

#[test]
fn save_and_load_round_trips_the_full_log() {
    let dir = tempfile::tempdir().unwrap();

    let mut log = PatchLog::new();
    log.append(
        Op::AddModule {
            id: 1,
            kind: "osc.va".into(),
            pos: Vec2 { x: 1.0, y: 2.0 },
        },
        0,
        Source::User,
    );
    log.append(
        Op::AddModule {
            id: 2,
            kind: "out".into(),
            pos: Vec2 { x: 10.0, y: 2.0 },
        },
        10,
        Source::User,
    );
    log.append(
        Op::Connect {
            id: 1,
            from: PortRef::Module {
                id: 1,
                port: "out".into(),
            },
            to: PortRef::Module {
                id: 2,
                port: "in".into(),
            },
        },
        20,
        Source::User,
    );
    log.append(
        Op::SetParam {
            target: ParamTarget::Module {
                id: 1,
                param: "freq".into(),
            },
            value: 220.0,
        },
        1000, // past the coalesce window from the previous op, so it's its own entry
        Source::User,
    );

    format::save(dir.path(), &log).unwrap();
    assert!(dir.path().join("log.jsonl").exists());
    assert!(dir.path().join("checkpoint.json").exists());
    assert!(dir.path().join("meta.toml").exists());

    let loaded = format::load(dir.path()).unwrap();
    assert_eq!(loaded.state(), log.state());
    assert_eq!(loaded.entries().len(), log.entries().len());
}
