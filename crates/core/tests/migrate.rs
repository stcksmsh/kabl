//! Schema 7 to 8: the stereo half ports become `in` / `out`, with `join` / `split` where a
//! single stereo cable cannot say the same (docs/stereo/DESIGN.md section 4).

use std::collections::BTreeMap;

use kabl_core::{
    migrate_state, CableState, ModuleState, PatchState, PortRef, Vec2, CURRENT_SCHEMA_VERSION,
};

fn state(modules: &[(u64, &str)], cables: &[((u64, &str), (u64, &str))]) -> PatchState {
    let mut s = PatchState::default();
    for &(id, kind) in modules {
        s.modules.insert(
            id,
            ModuleState {
                kind: kind.into(),
                pos: Vec2 { x: 100.0, y: 0.0 },
                params: BTreeMap::new(),
            },
        );
    }
    for (i, &(f, t)) in cables.iter().enumerate() {
        s.cables.insert(i as u64 + 1, cable(f, t));
    }
    s
}

fn cable(f: (u64, &str), t: (u64, &str)) -> CableState {
    let p = |(id, port): (u64, &str)| PortRef::Module {
        id,
        port: port.into(),
    };
    CableState {
        from: p(f),
        to: p(t),
        params: BTreeMap::new(),
        steps: Vec::new(),
    }
}

/// Every cable as `(from kind.port, to kind.port)`, sorted: the wiring without ids.
fn wiring(s: &PatchState) -> Vec<(String, String)> {
    let name = |r: &PortRef| match r {
        PortRef::Module { id, port } => format!("{}.{port}", s.modules[id].kind),
        other => format!("{other:?}"),
    };
    let mut v: Vec<_> = s
        .cables
        .values()
        .map(|c| (name(&c.from), name(&c.to)))
        .collect();
    v.sort();
    v
}

fn pair(a: &str, b: &str) -> (String, String) {
    (a.into(), b.into())
}

#[test]
fn a_mono_source_into_both_sides_is_one_cable() {
    let mut s = state(
        &[(1, "gain"), (2, "reverb")],
        &[((1, "out"), (2, "in_l")), ((1, "out"), (2, "in_r"))],
    );
    assert!(migrate_state(&mut s));
    assert_eq!(wiring(&s), [pair("gain.out", "reverb.in")]);
}

#[test]
fn a_stereo_pair_into_a_pair_is_one_stereo_cable() {
    let mut s = state(
        &[(1, "chorus"), (2, "reverb"), (3, "out")],
        &[
            ((1, "left"), (2, "in_l")),
            ((1, "right"), (2, "in_r")),
            ((2, "left"), (3, "left")),
            ((2, "right"), (3, "right")),
        ],
    );
    migrate_state(&mut s);
    assert_eq!(
        wiring(&s),
        [
            pair("chorus.out", "reverb.in"),
            pair("reverb.out", "out.in")
        ]
    );
}

#[test]
fn only_the_left_side_patched_goes_through_a_join_so_the_right_stays_silent() {
    let mut s = state(&[(1, "gain"), (2, "out")], &[((1, "out"), (2, "left"))]);
    migrate_state(&mut s);
    assert_eq!(
        wiring(&s),
        [pair("gain.out", "join.left"), pair("join.out", "out.in")]
    );
}

#[test]
fn two_sources_go_through_a_join() {
    let mut s = state(
        &[(1, "osc.va"), (2, "osc.va"), (3, "chorus")],
        &[((1, "out"), (3, "in_l")), ((2, "out"), (3, "in_r"))],
    );
    migrate_state(&mut s);
    assert_eq!(
        wiring(&s),
        [
            pair("join.out", "chorus.in"),
            pair("osc.va.out", "join.left"),
            pair("osc.va.out", "join.right"),
        ]
    );
}

#[test]
fn crossed_sides_go_through_a_split_and_a_join() {
    let mut s = state(
        &[(1, "chorus"), (2, "reverb")],
        &[((1, "left"), (2, "in_r")), ((1, "right"), (2, "in_l"))],
    );
    migrate_state(&mut s);
    assert_eq!(
        wiring(&s),
        [
            pair("chorus.out", "split.in"),
            pair("join.out", "reverb.in"),
            pair("split.left", "join.right"),
            pair("split.right", "join.left"),
        ]
    );
}

#[test]
fn a_half_port_used_as_a_signal_source_leaves_a_split() {
    let mut s = state(
        &[(1, "pan"), (2, "filter.svf"), (3, "filter.svf"), (4, "out")],
        &[
            ((1, "left"), (2, "in")),
            ((1, "right"), (3, "in")),
            ((2, "lp"), (4, "left")),
        ],
    );
    migrate_state(&mut s);
    assert_eq!(
        wiring(&s),
        [
            pair("filter.svf.lp", "join.left"),
            pair("join.out", "out.in"),
            pair("pan.out", "split.in"),
            pair("split.left", "filter.svf.in"),
            pair("split.right", "filter.svf.in"),
        ]
    );
}

#[test]
fn functional_cables_keep_their_own_legs() {
    let mut s = state(
        &[(1, "gain"), (2, "reverb")],
        &[((1, "out"), (2, "in_l")), ((1, "out"), (2, "in_r"))],
    );
    s.cables.get_mut(&1).unwrap().steps = vec![1.0, 0.0];
    migrate_state(&mut s);
    assert_eq!(
        wiring(&s),
        [
            pair("gain.out", "join.left"),
            pair("gain.out", "join.right"),
            pair("join.out", "reverb.in"),
        ]
    );
    assert_eq!(
        s.cables[&1].steps,
        [1.0, 0.0],
        "the leg keeps its cable and its pattern"
    );
}

#[test]
fn a_second_cable_into_one_port_was_never_read_and_goes() {
    let mut s = state(
        &[(1, "gain"), (2, "gain"), (3, "reverb")],
        &[
            ((1, "out"), (3, "in_l")),
            ((2, "out"), (3, "in_l")),
            ((1, "out"), (3, "in_r")),
        ],
    );
    migrate_state(&mut s);
    assert_eq!(wiring(&s), [pair("gain.out", "reverb.in")]);
}

#[test]
fn migrating_is_deterministic_and_a_second_pass_changes_nothing() {
    let build = || {
        state(
            &[(1, "osc.va"), (2, "osc.va"), (3, "chorus"), (4, "out")],
            &[
                ((1, "out"), (3, "in_l")),
                ((2, "out"), (3, "in_r")),
                ((3, "left"), (4, "left")),
            ],
        )
    };
    let (mut a, mut b) = (build(), build());
    migrate_state(&mut a);
    migrate_state(&mut b);
    assert_eq!(a, b);
    let once = a.clone();
    assert!(!migrate_state(&mut a));
    assert_eq!(a, once);
}

#[test]
fn a_cycle_through_a_pair_keeps_both_legs() {
    let mut s = state(
        &[(1, "gain"), (2, "chorus")],
        &[
            ((1, "out"), (2, "in_l")),
            ((1, "out"), (2, "in_r")),
            ((2, "left"), (1, "in")),
        ],
    );
    migrate_state(&mut s);
    assert_eq!(
        wiring(&s),
        [
            pair("chorus.out", "split.in"),
            pair("gain.out", "chorus.in"),
            pair("split.left", "gain.in"),
        ]
    );
}

#[test]
fn a_v7_file_loads_migrated_and_the_migration_is_not_undoable() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = kabl_core::PatchLog::new();
    let s = state(
        &[(1, "gain"), (2, "reverb"), (3, "out")],
        &[
            ((1, "out"), (2, "in_l")),
            ((1, "out"), (2, "in_r")),
            ((2, "left"), (3, "left")),
        ],
    );
    for (&id, m) in &s.modules {
        log.append_new(
            kabl_core::Op::AddModule {
                id,
                kind: m.kind.clone(),
                pos: m.pos,
            },
            0,
            kabl_core::Source::User,
        );
    }
    for (&id, c) in &s.cables {
        log.append_new(
            kabl_core::Op::Connect {
                id,
                from: c.from.clone(),
                to: c.to.clone(),
            },
            0,
            kabl_core::Source::User,
        );
    }
    kabl_core::save(dir.path(), &log).unwrap();
    // The file says schema 7, as master wrote it.
    let meta = std::fs::read_to_string(dir.path().join("meta.toml")).unwrap();
    let old = meta.replace(
        &format!("schema_version = {CURRENT_SCHEMA_VERSION}"),
        "schema_version = 7",
    );
    assert_ne!(meta, old);
    std::fs::write(dir.path().join("meta.toml"), old).unwrap();

    let mut back = kabl_core::load(dir.path()).unwrap();
    assert_eq!(
        wiring(back.state()),
        [
            pair("gain.out", "reverb.in"),
            pair("reverb.out", "split.in"),
            pair("split.left", "join.left"),
            pair("join.out", "out.in"),
        ]
    );
    let migrated = back.state().clone();
    assert!(!back.can_undo());
    assert!(!back.undo());
    assert_eq!(back.state(), &migrated, "undo stops at the migration");
    // Saving writes schema 8, which loads without migrating again.
    let out = tempfile::tempdir().unwrap();
    kabl_core::save(out.path(), &back).unwrap();
    let again = kabl_core::load(out.path()).unwrap();
    assert_eq!(again.state(), &migrated);
}
