//! A composite port or control may bind a cable parameter (schema v9). It belongs to the module
//! the cable ends on, and only a file that holds such a binding is written as v9.
use kabl_core::format::{self, schema_for};
use kabl_core::{composite, Composite, Definition, Exposure, Op, PatchLog, PortRef, Source, Vec2};
use std::collections::{BTreeMap, BTreeSet};

const ENV: u64 = 2;
const FILTER: u64 = 3;
const ROUTE: u64 = 10;

fn amount() -> PortRef {
    PortRef::CableParam {
        cable: ROUTE,
        param: "amount".into(),
    }
}

/// Macro, envelope and filter; the envelope moves the cutoff and the macro moves that route's
/// amount.
fn base() -> PatchLog {
    let mut log = PatchLog::new();
    for (id, kind) in [(1, "macro"), (ENV, "env.adsr"), (FILTER, "filter.svf")] {
        log.append(
            Op::AddModule {
                id,
                kind: kind.into(),
                pos: Vec2::default(),
            },
            0,
            Source::User,
        );
    }
    let jack = |id, port: &str| PortRef::Module {
        id,
        port: port.into(),
    };
    log.append(
        Op::Connect {
            id: ROUTE,
            from: jack(ENV, "out"),
            to: PortRef::Param {
                id: FILTER,
                param: "cutoff_hz".into(),
            },
        },
        0,
        Source::User,
    );
    log.append(
        Op::Connect {
            id: 11,
            from: jack(1, "m1"),
            to: amount(),
        },
        0,
        Source::User,
    );
    log
}

fn wrap(log: &mut PatchLog, members: &[u64], control: bool) {
    let e = Exposure {
        label: "Env Amount".into(),
        target: amount(),
    };
    let (mut ports, mut controls) = (BTreeMap::new(), BTreeMap::new());
    if control {
        controls.insert(1, e);
    } else {
        ports.insert(1, e);
    }
    let c = Composite {
        name: "Voice".into(),
        help: String::new(),
        definition: Definition {
            id: "voice".into(),
            version: 1,
        },
        parent: None,
        members: members.iter().copied().collect::<BTreeSet<_>>(),
        pos: Vec2::default(),
        ports,
        controls,
        next_interface: 2,
        panel: None,
    };
    log.append(
        Op::SetComposite {
            id: 1,
            value: Some(c),
        },
        0,
        Source::User,
    );
}

#[test]
fn the_owner_is_the_module_the_cable_ends_on() {
    for control in [false, true] {
        let mut log = base();
        wrap(&mut log, &[ENV, FILTER], control);
        composite::validate(log.state()).unwrap();
        assert_eq!(composite::owner(log.state(), &amount()), Some(FILTER));

        // The route ends on the filter: wrapping only the envelope does not own its amount.
        let mut log = base();
        wrap(&mut log, &[ENV], control);
        let err = composite::validate(log.state()).unwrap_err();
        assert!(err.contains("invalid public binding 1"), "{err}");
    }

    // A binding of a cable that is gone has no owner.
    let mut log = base();
    wrap(&mut log, &[ENV, FILTER], false);
    log.append(Op::Disconnect { id: ROUTE }, 0, Source::User);
    assert!(composite::validate(log.state()).is_err());
}

#[test]
fn a_cable_param_binding_saves_as_v9_and_loads_back() {
    let mut log = base();
    assert_eq!(schema_for(&log), 8);
    wrap(&mut log, &[ENV, FILTER], false);
    assert_eq!(schema_for(&log), 9);

    let dir = tempfile::tempdir().unwrap();
    format::save(dir.path(), &log).unwrap();
    let meta = std::fs::read_to_string(dir.path().join("meta.toml")).unwrap();
    assert!(meta.contains("schema_version = 9"), "{meta}");
    assert_eq!(format::load(dir.path()).unwrap().state(), log.state());

    let mut bytes = Vec::new();
    format::write_kabl(&mut bytes, &log).unwrap();
    assert_eq!(bytes[14], 9);
    assert_eq!(format::read_kabl(&bytes[..]).unwrap().state(), log.state());

    // Undone, the binding is past the entries a save writes, so the file is v8 again.
    assert!(log.undo());
    assert_eq!(schema_for(&log), 8);
}
