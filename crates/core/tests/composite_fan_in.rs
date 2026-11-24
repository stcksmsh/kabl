//! One composite port may feed several internal targets (fan-in, `Exposure::also`, schema v9).
//! A file with neither fan-in nor a cable-parameter binding stays v8, and v9 files written
//! before fan-in existed load unchanged.
use kabl_core::format::{self, schema_for};
use kabl_core::{composite, Composite, Definition, Exposure, Op, PatchLog, PortRef, Source, Vec2};
use std::collections::{BTreeMap, BTreeSet};

const MACRO: u64 = 1;
const OSC_A: u64 = 2;
const OSC_B: u64 = 3;

fn jack(id: u64, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

/// A macro feeding the pitch input of two oscillators.
fn base() -> PatchLog {
    let mut log = PatchLog::new();
    for (id, kind) in [(MACRO, "macro"), (OSC_A, "osc.va"), (OSC_B, "osc.va")] {
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
    for (id, to) in [(10, OSC_A), (11, OSC_B)] {
        log.append(
            Op::Connect {
                id,
                from: jack(MACRO, "m1"),
                to: jack(to, "pitch"),
            },
            0,
            Source::User,
        );
    }
    log
}

fn exposure(target: PortRef, also: Vec<PortRef>) -> Exposure {
    Exposure {
        label: "Pitch".into(),
        target,
        also,
    }
}

fn set(log: &mut PatchLog, ports: Vec<Exposure>, controls: Vec<Exposure>) {
    let next = (ports.len() + controls.len()) as u64 + 1;
    let c = Composite {
        name: "Voice".into(),
        help: String::new(),
        definition: Definition {
            id: "voice".into(),
            version: 1,
        },
        parent: None,
        members: BTreeSet::from([OSC_A, OSC_B]),
        pos: Vec2::default(),
        ports: (1..).zip(ports).collect::<BTreeMap<_, _>>(),
        controls: (next - controls.len() as u64..).zip(controls).collect(),
        next_interface: next,
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

fn saved(log: &PatchLog) -> (String, PatchLog) {
    let dir = tempfile::tempdir().unwrap();
    format::save(dir.path(), log).unwrap();
    let meta = std::fs::read_to_string(dir.path().join("meta.toml")).unwrap();
    let text = std::fs::read_to_string(dir.path().join("log.jsonl")).unwrap();
    (meta + &text, format::load(dir.path()).unwrap())
}

#[test]
fn a_fan_in_port_saves_as_v9_and_loads_back() {
    let mut log = base();
    set(
        &mut log,
        vec![exposure(jack(OSC_A, "pitch"), vec![jack(OSC_B, "pitch")])],
        vec![],
    );
    composite::validate(log.state()).unwrap();
    assert_eq!(schema_for(&log), 9);
    let (text, back) = saved(&log);
    assert!(text.contains("schema_version = 9") && text.contains("\"also\""));
    assert_eq!(back.state(), log.state());
    let port = &back.state().composites[&1].ports[&1];
    assert_eq!(
        port.targets().cloned().collect::<Vec<_>>(),
        vec![jack(OSC_A, "pitch"), jack(OSC_B, "pitch")]
    );

    let mut bytes = Vec::new();
    format::write_kabl(&mut bytes, &log).unwrap();
    assert_eq!(bytes[14], 9);
    assert_eq!(format::read_kabl(&bytes[..]).unwrap().state(), log.state());

    // Undone, the fan-in is past the entries a save writes, so the file is v8 again.
    assert!(log.undo());
    assert_eq!(schema_for(&log), 8);
}

/// The layout of a file from before fan-in: one port per target, no `also` field anywhere.
/// Such a file stays v8 and loads as N ports.
#[test]
fn an_older_file_with_one_port_per_target_loads_as_n_ports() {
    let mut log = base();
    set(
        &mut log,
        vec![
            exposure(jack(OSC_A, "pitch"), vec![]),
            exposure(jack(OSC_B, "pitch"), vec![]),
        ],
        vec![],
    );
    assert_eq!(schema_for(&log), 8);
    let (text, back) = saved(&log);
    assert!(text.contains("schema_version = 8"), "{text}");
    assert!(!text.contains("\"also\""), "{text}");
    let ports = &back.state().composites[&1].ports;
    assert_eq!(ports.len(), 2);
    assert!(ports.values().all(|e| e.also.is_empty()));
}

/// A v9 file as #215 wrote it: a cable-parameter binding and no `also` key anywhere. The
/// writer still produces that layout, and an exposure without the key reads as one target.
#[test]
fn a_v9_file_from_before_fan_in_loads() {
    let target = serde_json::to_string(&jack(OSC_A, "pitch")).unwrap();
    let old: Exposure =
        serde_json::from_str(&format!(r#"{{"label":"Pitch","target":{target}}}"#)).unwrap();
    assert_eq!(old, exposure(jack(OSC_A, "pitch"), vec![]));

    let mut log = base();
    log.append(
        Op::Connect {
            id: 12,
            from: jack(OSC_B, "out"),
            to: PortRef::Param {
                id: OSC_A,
                param: "tune".into(),
            },
        },
        0,
        Source::User,
    );
    let amount = PortRef::CableParam {
        cable: 12,
        param: "amount".into(),
    };
    set(&mut log, vec![exposure(amount, vec![])], vec![]);
    assert_eq!(schema_for(&log), 9);
    let (text, back) = saved(&log);
    assert!(text.contains("schema_version = 9"), "{text}");
    assert!(!text.contains("\"also\""), "{text}");
    assert_eq!(back.state(), log.state());
}

#[test]
fn a_control_cannot_fan_in_and_a_target_appears_once() {
    let mut log = base();
    set(
        &mut log,
        vec![],
        vec![Exposure {
            label: "Tune".into(),
            target: PortRef::Param {
                id: OSC_A,
                param: "tune".into(),
            },
            also: vec![PortRef::Param {
                id: OSC_B,
                param: "tune".into(),
            }],
        }],
    );
    let err = composite::validate(log.state()).unwrap_err();
    assert!(err.contains("control must bind a parameter"), "{err}");

    let mut log = base();
    set(
        &mut log,
        vec![exposure(jack(OSC_A, "pitch"), vec![jack(OSC_A, "pitch")])],
        vec![],
    );
    let err = composite::validate(log.state()).unwrap_err();
    assert!(err.contains("invalid public binding"), "{err}");

    let mut log = base();
    set(
        &mut log,
        vec![exposure(jack(OSC_A, "pitch"), vec![jack(MACRO, "m1")])],
        vec![],
    );
    let err = composite::validate(log.state()).unwrap_err();
    assert!(err.contains("invalid public binding"), "{err}");
}

#[test]
fn removing_one_target_keeps_the_rest_and_the_last_drops_the_port() {
    let mut log = base();
    set(
        &mut log,
        vec![exposure(jack(OSC_A, "pitch"), vec![jack(OSC_B, "pitch")])],
        vec![],
    );
    let before = log.state().clone();
    log.append(Op::RemoveModule { id: OSC_A }, 0, Source::User);
    let port = &log.state().composites[&1].ports[&1];
    assert_eq!(port.target, jack(OSC_B, "pitch"));
    assert!(port.also.is_empty());
    log.append(Op::RemoveModule { id: OSC_B }, 0, Source::User);
    assert!(log
        .state()
        .composites
        .get(&1)
        .is_none_or(|c| c.ports.is_empty()));
    assert!(log.undo() && log.undo());
    assert_eq!(log.state(), &before);
    assert!(log.redo());
    assert_eq!(
        log.state().composites[&1].ports[&1].target,
        jack(OSC_B, "pitch")
    );
}

#[test]
fn undoing_an_edit_in_a_patch_without_composites_is_exact() {
    // The inverse of a plain edit is exact in a patch without composites.
    let mut log = base();
    let before = log.state().clone();
    log.append(Op::RemoveModule { id: OSC_A }, 0, Source::User);
    assert!(log.undo());
    assert_eq!(log.state(), &before);
}
