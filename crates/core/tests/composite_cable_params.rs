//! A composite port may bind a cable parameter (schema v9), and so may a control (v10, drawn as
//! a knob on the face). It belongs to the module the cable ends on, and only a file that holds
//! such a binding is written at that schema.
use kabl_core::format::{self, schema_for};
use kabl_core::panel::{Kind, Panel, Placement};
use kabl_core::{
    composite, composite::Snapshot, Composite, Definition, Exposure, Op, PatchLog, PortRef, Source,
    Vec2,
};
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
        also: vec![],
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
        snapshots: Vec::new(),
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
    let mut log = base();
    wrap(&mut log, &[ENV, FILTER], false);
    composite::validate(log.state()).unwrap();
    assert_eq!(composite::owner(log.state(), &amount()), Some(FILTER));

    // The route ends on the filter: wrapping only the envelope does not own its amount.
    let mut log = base();
    wrap(&mut log, &[ENV], false);
    let err = composite::validate(log.state()).unwrap_err();
    assert!(err.contains("invalid public binding 1"), "{err}");
}

#[test]
fn a_cable_param_inside_the_composite_can_be_a_control() {
    // A face draws a route's amount as a knob, so a control may bind it.
    let mut log = base();
    wrap(&mut log, &[ENV, FILTER], true);
    composite::validate(log.state()).unwrap();
}

#[test]
fn a_cable_param_outside_the_composite_cannot_be_a_control() {
    // The route ends on the filter, which is outside a voice of the envelope alone.
    let mut log = base();
    wrap(&mut log, &[ENV], true);
    let err = composite::validate(log.state()).unwrap_err();
    assert!(err.contains("invalid public binding 1"), "{err}");
}

fn ports(log: &PatchLog) -> Vec<PortRef> {
    log.state().composites[&1]
        .ports
        .values()
        .map(|e| e.target.clone())
        .collect()
}

/// Applies `op` to a wrapped voice: the binding goes and the composite stays valid; undo brings
/// the binding back, redo drops it again.
fn assert_binding_drops_until_undone(op: Op) {
    let mut log = base();
    wrap(&mut log, &[ENV, FILTER], false);
    let before = log.state().clone();
    log.append(op, 0, Source::User);
    assert!(ports(&log).is_empty());
    composite::validate(log.state()).unwrap();
    assert!(log.undo());
    assert_eq!(log.state(), &before);
    assert_eq!(ports(&log), vec![amount()]);
    assert!(log.redo());
    assert!(ports(&log).is_empty());
}

/// The voice's controls, the keys its face places and the keys its snapshots hold.
fn controls(log: &PatchLog) -> (Vec<PortRef>, Vec<u64>, Vec<u64>) {
    let c = &log.state().composites[&1];
    let placed = c
        .panel
        .as_ref()
        .map_or(Vec::new(), |p| p.placements.keys().copied().collect());
    let held = c
        .snapshots
        .iter()
        .flat_map(|s| s.values.keys().copied())
        .collect();
    (
        c.controls.values().map(|e| e.target.clone()).collect(),
        placed,
        held,
    )
}

/// As `assert_binding_drops_until_undone`, for a control placed on the voice's face and held by
/// a snapshot: the control, its knob and its snapshot value go together and come back together.
fn assert_control_drops_until_undone(op: Op) {
    let mut log = base();
    wrap(&mut log, &[ENV, FILTER], true);
    let mut c = log.state().composites[&1].clone();
    let mut panel = Panel::default();
    panel.placements.insert(
        1,
        Placement {
            kind: Kind::Knob,
            x: 8.0,
            y: 64.0,
            width: 90.0,
            height: 110.0,
            source: None,
            label: None,
            icons: Vec::new(),
        },
    );
    c.panel = Some(Box::new(panel));
    c.next_interface = 3;
    c.snapshots.push(Snapshot {
        id: 2,
        name: "Bright".into(),
        values: BTreeMap::from([(1, 0.5)]),
        morph_beats: 0.0,
        clock: None,
    });
    log.append(
        Op::SetComposite {
            id: 1,
            value: Some(c),
        },
        0,
        Source::User,
    );
    composite::validate(log.state()).unwrap();
    let before = log.state().clone();
    log.append(op, 0, Source::User);
    assert_eq!(controls(&log), (vec![], vec![], vec![]));
    composite::validate(log.state()).unwrap();
    assert!(log.undo());
    assert_eq!(log.state(), &before);
    assert_eq!(controls(&log), (vec![amount()], vec![1], vec![1]));
    assert!(log.redo());
    assert_eq!(controls(&log), (vec![], vec![], vec![]));
}

#[test]
fn removing_the_owner_drops_the_binding_until_undone() {
    assert_binding_drops_until_undone(Op::RemoveModule { id: FILTER });
    assert_control_drops_until_undone(Op::RemoveModule { id: FILTER });
}

#[test]
fn removing_the_source_module_drops_the_binding_until_undone() {
    // The envelope takes its route with it, so the amount has nothing to belong to.
    assert_binding_drops_until_undone(Op::RemoveModule { id: ENV });
    assert_control_drops_until_undone(Op::RemoveModule { id: ENV });
}

#[test]
fn disconnecting_the_route_drops_the_binding_until_undone() {
    assert_binding_drops_until_undone(Op::Disconnect { id: ROUTE });
    assert_control_drops_until_undone(Op::Disconnect { id: ROUTE });
}

#[test]
fn moving_the_route_out_of_the_composite_drops_the_binding_until_undone() {
    // The macro module 1 is outside the voice; reconnecting the route there moves its owner out.
    let moved = Op::Connect {
        id: ROUTE,
        from: PortRef::Module {
            id: ENV,
            port: "out".into(),
        },
        to: PortRef::Param {
            id: 1,
            param: "value".into(),
        },
    };
    assert_binding_drops_until_undone(moved.clone());
    assert_control_drops_until_undone(moved);
}

#[test]
fn moving_the_route_inside_the_composite_keeps_the_binding() {
    let mut log = base();
    wrap(&mut log, &[ENV, FILTER], false);
    log.append(
        Op::Connect {
            id: ROUTE,
            from: PortRef::Module {
                id: ENV,
                port: "out".into(),
            },
            to: PortRef::Param {
                id: FILTER,
                param: "q".into(),
            },
        },
        0,
        Source::User,
    );
    assert_eq!(ports(&log), vec![amount()]);
    composite::validate(log.state()).unwrap();
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

#[test]
fn a_cable_param_control_needs_faces_and_loads_back() {
    // The face schema is the first a reader that knows controls on cables can rely on.
    let mut log = base();
    wrap(&mut log, &[ENV, FILTER], true);
    assert_eq!(schema_for(&log), 10);
    let mut bytes = Vec::new();
    format::write_kabl(&mut bytes, &log).unwrap();
    assert_eq!(bytes[14], 10);
    assert_eq!(format::read_kabl(&bytes[..]).unwrap().state(), log.state());
    assert!(log.undo());
    assert_eq!(schema_for(&log), 8);
}
