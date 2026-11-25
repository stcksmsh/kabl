//! Composite faces, stored form (schema v10): new panel element kinds, element sources,
//! snapshots, their validation, and the schema a log is written with. v8 and v9 files written
//! before v10 existed load and re-save byte for byte.
use kabl_core::composite::{self, Snapshot};
use kabl_core::format::{self, schema_for};
use kabl_core::panel::{Artwork, Kind, Panel, Placement, Source};
use kabl_core::{
    Composite, CompositeId, Definition, Exposure, ModuleId, Op, PatchLog, PatchState, PortRef, Vec2,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const CLOCK: ModuleId = 1;
const CUES: ModuleId = 2;
const OSC: ModuleId = 3;
const FILTER: ModuleId = 4;
const OUTER: ModuleId = 5;

/// The parent has the smaller id, so a restore of every composite in id order puts the parent
/// back before its nested child.
const PARENT: CompositeId = 1;
const CHILD: CompositeId = 2;

/// Child keys: controls 1 (cutoff) and 2 (waveform), button 3 on snapshot 4, snapshot strip 5.
/// Parent keys: display 1 on the filter, button 2 on the child's button 3, cue pads 3.
fn placement(kind: Kind, x: f32, width: f32, height: f32, source: Option<Source>) -> Placement {
    Placement {
        kind,
        x,
        y: 72.0,
        width,
        height,
        source,
        label: None,
        icons: Vec::new(),
    }
}

fn control(id: ModuleId, param: &str) -> Exposure {
    Exposure {
        label: param.into(),
        target: PortRef::Param {
            id,
            param: param.into(),
        },
        also: Vec::new(),
    }
}

fn composite(name: &str, parent: Option<CompositeId>, members: &[ModuleId]) -> Composite {
    Composite {
        name: name.into(),
        help: String::new(),
        definition: Definition {
            id: format!("test.{name}"),
            version: 1,
        },
        parent,
        members: members.iter().copied().collect::<BTreeSet<_>>(),
        pos: Vec2::default(),
        ports: BTreeMap::new(),
        controls: BTreeMap::new(),
        next_interface: 1,
        panel: None,
        snapshots: Vec::new(),
    }
}

fn snapshot(id: u64) -> Snapshot {
    Snapshot {
        id,
        name: format!("Snap {id}"),
        values: BTreeMap::from([(1, 800.0), (2, 1.0)]),
        morph_beats: 4.0,
        clock: Some(CLOCK),
    }
}

fn child() -> Composite {
    let mut c = composite("child", Some(PARENT), &[OSC, FILTER]);
    c.controls = BTreeMap::from([
        (1, control(FILTER, "cutoff")),
        (2, control(OSC, "waveform")),
    ]);
    c.next_interface = 6;
    c.snapshots = vec![snapshot(4)];
    c.panel = Some(Box::new(Panel {
        placements: BTreeMap::from([
            (1, placement(Kind::Knob, 12.0, 84.0, 110.0, None)),
            (
                3,
                placement(
                    Kind::Button,
                    104.0,
                    54.0,
                    40.0,
                    Some(Source::Snapshot { id: 4 }),
                ),
            ),
            (5, placement(Kind::Snapshots, 170.0, 54.0, 40.0, None)),
        ]),
        ..Panel::default()
    }));
    c
}

fn parent() -> Composite {
    let mut c = composite("parent", None, &[CLOCK, CUES]);
    c.next_interface = 4;
    let module = |id| Some(Source::Module { id, action: None });
    c.panel = Some(Box::new(Panel {
        width: 600.0,
        placements: BTreeMap::from([
            (
                1,
                placement(Kind::Display, 12.0, 120.0, 60.0, module(FILTER)),
            ),
            (
                2,
                placement(
                    Kind::Button,
                    140.0,
                    54.0,
                    40.0,
                    Some(Source::Face {
                        composite: CHILD,
                        key: 3,
                    }),
                ),
            ),
            (
                3,
                placement(Kind::CuePads, 200.0, 120.0, 120.0, module(CUES)),
            ),
        ]),
        ..Panel::default()
    }));
    c
}

fn state() -> PatchState {
    let mut p = PatchState::new();
    for (id, kind) in [
        (CLOCK, "clock"),
        (CUES, "cues"),
        (OSC, "osc.va"),
        (FILTER, "filter.svf"),
        (OUTER, "osc.va"),
    ] {
        p.apply(&Op::AddModule {
            id,
            kind: kind.into(),
            pos: Vec2::default(),
        });
    }
    for (id, c) in [(PARENT, parent()), (CHILD, child())] {
        p.apply(&Op::SetComposite { id, value: Some(c) });
    }
    composite::validate(&p).unwrap();
    p
}

/// `state()` with `edit` applied to composite `id`; the validation result.
fn with(id: CompositeId, edit: impl FnOnce(&mut Composite)) -> Result<(), String> {
    let mut p = state();
    edit(p.composites.get_mut(&id).unwrap());
    composite::validate(&p)
}

fn refused(id: CompositeId, edit: impl FnOnce(&mut Composite)) -> String {
    with(id, edit).expect_err("refused")
}

fn placements(c: &mut Composite) -> &mut BTreeMap<u64, Placement> {
    &mut c.panel.as_mut().unwrap().placements
}

// --- v8 and v9 files ---

fn resaves_byte_identical(fixture: &str, schema: u32) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/faces")
        .join(fixture);
    let log = format::load(&dir).unwrap();
    assert_eq!(schema_for(&log), schema);
    let out = tempfile::tempdir().unwrap();
    format::save(out.path(), &log).unwrap();
    for file in ["meta.toml", "log.jsonl", "checkpoint.json"] {
        assert_eq!(
            std::fs::read(dir.join(file)).unwrap(),
            std::fs::read(out.path().join(file)).unwrap(),
            "{fixture}/{file}"
        );
    }
}

#[test]
fn a_v8_file_loads_and_resaves_byte_identical() {
    resaves_byte_identical("v8", 8);
}

#[test]
fn a_v9_file_loads_and_resaves_byte_identical() {
    resaves_byte_identical("v9", 9);
}

// --- schema_for ---

fn log_with(c: Composite) -> PatchLog {
    let mut log = PatchLog::new();
    for op in [
        Op::AddModule {
            id: OSC,
            kind: "osc.va".into(),
            pos: Vec2::default(),
        },
        Op::AddModule {
            id: FILTER,
            kind: "filter.svf".into(),
            pos: Vec2::default(),
        },
        Op::SetComposite {
            id: 1,
            value: Some(c),
        },
    ] {
        log.append(op, 0, kabl_core::Source::User);
    }
    log
}

fn plain() -> Composite {
    let mut c = composite("plain", None, &[OSC, FILTER]);
    c.controls = BTreeMap::from([(1, control(FILTER, "cutoff"))]);
    c.next_interface = 3;
    c.panel = Some(Box::new(Panel {
        placements: BTreeMap::from([(1, placement(Kind::Knob, 12.0, 84.0, 110.0, None))]),
        ..Panel::default()
    }));
    c
}

#[test]
fn a_log_with_a_snapshot_is_written_as_v10() {
    let mut c = plain();
    c.snapshots = vec![Snapshot {
        id: 2,
        name: "A".into(),
        values: BTreeMap::from([(1, 500.0)]),
        morph_beats: 0.0,
        clock: None,
    }];
    let log = log_with(c);
    composite::validate(log.state()).unwrap();
    assert_eq!(schema_for(&log), 10);
}

#[test]
fn a_log_with_a_new_element_kind_is_written_as_v10() {
    let mut c = plain();
    let p = placements(&mut c).get_mut(&1).unwrap();
    p.kind = Kind::Slider;
    p.width = 120.0;
    let log = log_with(c);
    composite::validate(log.state()).unwrap();
    assert_eq!(schema_for(&log), 10);
}

#[test]
fn a_log_with_only_a_fan_in_port_is_written_as_v9_and_otherwise_v8() {
    assert_eq!(schema_for(&log_with(plain())), 8);
    let mut c = plain();
    c.ports = BTreeMap::from([(
        2,
        Exposure {
            label: "In".into(),
            target: PortRef::Module {
                id: OSC,
                port: "pitch".into(),
            },
            also: vec![PortRef::Module {
                id: FILTER,
                port: "cutoff".into(),
            }],
        },
    )]);
    let log = log_with(c);
    composite::validate(log.state()).unwrap();
    assert_eq!(schema_for(&log), 9);
}

// --- Panel::validate ---

#[test]
fn the_fixture_with_every_source_kind_is_valid() {
    composite::validate(&state()).unwrap();
}

#[test]
fn a_control_kind_needs_a_control_and_a_jack_a_port() {
    let e = refused(CHILD, |c| {
        placements(c).get_mut(&1).unwrap().kind = Kind::Jack
    });
    assert!(e.contains("wrong control kind"), "{e}");
    for kind in [Kind::Slider, Kind::Toggle, Kind::Icons] {
        with(CHILD, |c| {
            let p = placements(c).get_mut(&1).unwrap();
            p.kind = kind;
            p.height = 160.0;
            p.width = 84.0;
        })
        .unwrap();
    }
}

#[test]
fn a_composite_level_element_cannot_use_an_exposure_key() {
    let e = refused(CHILD, |c| {
        let p = placements(c).get_mut(&1).unwrap();
        p.kind = Kind::Button;
        p.source = Some(Source::Snapshot { id: 4 });
    });
    assert!(e.contains("wrong control kind"), "{e}");
}

#[test]
fn a_composite_level_element_key_comes_from_next_interface() {
    let e = refused(CHILD, |c| {
        let strip = placements(c).remove(&5).unwrap();
        placements(c).insert(6, strip);
    });
    assert!(e.contains("wrong control kind"), "{e}");
}

#[test]
fn an_exposure_bound_element_or_the_snapshot_strip_has_no_source() {
    let e = refused(CHILD, |c| {
        placements(c).get_mut(&1).unwrap().source = Some(Source::Snapshot { id: 4 })
    });
    assert!(e.contains("source does not fit"), "{e}");
    let e = refused(CHILD, |c| {
        placements(c).get_mut(&5).unwrap().source = Some(Source::Snapshot { id: 4 })
    });
    assert!(e.contains("source does not fit"), "{e}");
}

#[test]
fn a_display_button_or_block_needs_a_source() {
    for key in [1, 2, 3] {
        let e = refused(PARENT, |c| {
            placements(c).get_mut(&key).unwrap().source = None
        });
        assert!(e.contains("source does not fit"), "{key}: {e}");
    }
}

#[test]
fn only_a_button_names_a_module_action_and_a_module_button_needs_one() {
    let e = refused(PARENT, |c| {
        placements(c).get_mut(&1).unwrap().source = Some(Source::Module {
            id: FILTER,
            action: Some("run".into()),
        })
    });
    assert!(e.contains("source does not fit"), "{e}");
    let e = refused(PARENT, |c| {
        placements(c).get_mut(&2).unwrap().source = Some(Source::Module {
            id: CLOCK,
            action: None,
        })
    });
    assert!(e.contains("source does not fit"), "{e}");
    with(PARENT, |c| {
        placements(c).get_mut(&2).unwrap().source = Some(Source::Module {
            id: CLOCK,
            action: Some("run".into()),
        })
    })
    .unwrap();
}

#[test]
fn a_block_or_display_cannot_name_a_face_or_snapshot() {
    let e = refused(PARENT, |c| {
        placements(c).get_mut(&3).unwrap().source = Some(Source::Face {
            composite: CHILD,
            key: 3,
        })
    });
    assert!(e.contains("source does not fit"), "{e}");
}

#[test]
fn a_label_is_at_most_40_characters() {
    with(PARENT, |c| {
        placements(c).get_mut(&1).unwrap().label = Some("é".repeat(40))
    })
    .unwrap();
    let e = refused(PARENT, |c| {
        placements(c).get_mut(&1).unwrap().label = Some("é".repeat(41))
    });
    assert!(e.contains("label"), "{e}");
}

fn png(w: u32, h: u32) -> Artwork {
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    png.extend(w.to_be_bytes());
    png.extend(h.to_be_bytes());
    png.extend([8, 6, 0, 0, 0, 0, 0, 0, 0]);
    Artwork {
        name: "icon.png".into(),
        png,
    }
}

fn icons(c: &mut Composite, list: Vec<Artwork>) {
    let p = placements(c).get_mut(&1).unwrap();
    p.kind = Kind::Icons;
    p.width = 84.0;
    p.icons = list;
}

#[test]
fn icons_only_on_an_icon_selector() {
    with(CHILD, |c| icons(c, vec![png(16, 16), png(16, 16)])).unwrap();
    let e = refused(CHILD, |c| {
        placements(c).get_mut(&1).unwrap().icons = vec![png(16, 16)]
    });
    assert!(e.contains("icons only"), "{e}");
}

#[test]
fn each_icon_is_checked_as_panel_artwork() {
    let e = refused(CHILD, |c| icons(c, vec![png(16, 16), png(4096, 16)]));
    assert!(e.contains("Artwork dimensions"), "{e}");
}

#[test]
fn icon_pixels_count_toward_the_document_artwork_budget() {
    let e = refused(CHILD, |c| {
        icons(c, (0..9).map(|_| png(1024, 1024)).collect())
    });
    assert!(e.contains("Document artwork limit"), "{e}");
}

#[test]
fn new_kinds_have_minimum_sizes() {
    for (key, kind, w, h) in [
        (1, Kind::Slider, 119.0, 55.0),
        (1, Kind::Slider, 40.0, 159.0),
        (1, Kind::Toggle, 63.0, 55.0),
        (5, Kind::Snapshots, 53.0, 40.0),
        (3, Kind::Button, 54.0, 39.0),
    ] {
        let e = refused(CHILD, |c| {
            let p = placements(c).get_mut(&key).unwrap();
            p.kind = kind;
            p.width = w;
            p.height = h;
        });
        assert!(e.contains("too small"), "{kind:?}: {e}");
    }
    for (key, kind, w, h) in [
        (1, Kind::Display, 119.0, 60.0),
        (3, Kind::CuePads, 120.0, 119.0),
        (3, Kind::Transport, 119.0, 32.0),
        (3, Kind::Banks, 119.0, 18.0),
    ] {
        let e = refused(PARENT, |c| {
            let p = placements(c).get_mut(&key).unwrap();
            p.kind = kind;
            p.width = w;
            p.height = h;
        });
        assert!(e.contains("too small"), "{kind:?}: {e}");
    }
}

#[test]
fn a_compact_width_is_in_rack_units_from_180_and_below_the_width() {
    with(PARENT, |c| {
        c.panel.as_mut().unwrap().compact_width = Some(300.0)
    })
    .unwrap();
    for w in [150.0, 600.0, 310.0, f32::NAN] {
        let e = refused(PARENT, |c| {
            c.panel.as_mut().unwrap().compact_width = Some(w)
        });
        assert!(e.contains("Compact width"), "{w}: {e}");
    }
}

// --- composite::validate ---

#[test]
fn a_module_source_is_a_leaf_at_any_depth() {
    // The filter is the child's member, two levels below the parent's display: accepted.
    composite::validate(&state()).unwrap();
    let e = refused(PARENT, |c| {
        placements(c).get_mut(&1).unwrap().source = Some(Source::Module {
            id: OUTER,
            action: None,
        })
    });
    assert!(e.contains("missing source"), "{e}");
}

#[test]
fn a_face_target_on_a_non_descendant_is_refused() {
    // The child cannot press its parent's button, nor itself.
    for target in [PARENT, CHILD] {
        let e = refused(CHILD, |c| {
            placements(c).get_mut(&3).unwrap().source = Some(Source::Face {
                composite: target,
                key: 2,
            })
        });
        assert!(e.contains("missing source"), "{target}: {e}");
    }
}

#[test]
fn a_face_target_on_a_non_button_is_refused() {
    for key in [1, 5, 9] {
        let e = refused(PARENT, |c| {
            placements(c).get_mut(&2).unwrap().source = Some(Source::Face {
                composite: CHILD,
                key,
            })
        });
        assert!(e.contains("missing source"), "{key}: {e}");
    }
}

#[test]
fn a_snapshot_source_names_an_existing_snapshot() {
    let e = refused(CHILD, |c| c.snapshots.clear());
    assert!(e.contains("missing source"), "{e}");
}

#[test]
fn snapshot_ids_are_unique() {
    let e = refused(CHILD, |c| c.snapshots.push(snapshot(4)));
    assert!(e.contains("reuses an id"), "{e}");
}

#[test]
fn snapshot_ids_are_below_next_interface() {
    let e = refused(CHILD, |c| c.snapshots.push(snapshot(6)));
    assert!(e.contains("not from next_interface"), "{e}");
}

#[test]
fn snapshot_names_are_1_to_40_characters() {
    with(CHILD, |c| c.snapshots[0].name = "x".repeat(40)).unwrap();
    for name in [String::new(), "x".repeat(41)] {
        let e = refused(CHILD, |c| c.snapshots[0].name = name);
        assert!(e.contains("name"), "{e}");
    }
}

#[test]
fn at_most_16_snapshots() {
    let fill = |c: &mut Composite, n: u64| {
        c.next_interface = 100;
        c.snapshots.extend((10..10 + n).map(snapshot));
    };
    with(CHILD, |c| fill(c, 15)).unwrap();
    let e = refused(CHILD, |c| fill(c, 16));
    assert!(e.contains("at most 16"), "{e}");
}

#[test]
fn a_snapshot_key_that_is_not_a_control_is_refused() {
    let e = refused(CHILD, |c| {
        c.snapshots[0].values.insert(3, 1.0);
    });
    assert!(e.contains("not a control"), "{e}");
}

#[test]
fn snapshot_values_are_finite() {
    let e = refused(CHILD, |c| {
        c.snapshots[0].values.insert(1, f32::INFINITY);
    });
    assert!(e.contains("out of range"), "{e}");
}

#[test]
fn a_snapshot_morph_is_0_to_16_beats() {
    with(CHILD, |c| c.snapshots[0].morph_beats = 16.0).unwrap();
    for beats in [-1.0, 16.5, f32::NAN] {
        let e = refused(CHILD, |c| c.snapshots[0].morph_beats = beats);
        assert!(e.contains("out of range"), "{beats}: {e}");
    }
}

#[test]
fn a_snapshot_clock_is_a_clock_module() {
    for m in [OSC, 99] {
        let e = refused(CHILD, |c| c.snapshots[0].clock = Some(m));
        assert!(e.contains("not a clock"), "{m}: {e}");
    }
}

// --- apply never prunes ---

#[test]
fn a_lone_set_composite_never_changes_another_composite() {
    let mut p = state();
    let before = p.composites[&PARENT].clone();
    // Dropping the child's button leaves the parent's face target dangling; `apply` writes
    // exactly the value it carries and leaves the parent alone.
    let mut c = child();
    placements(&mut c).remove(&3);
    p.apply(&Op::SetComposite {
        id: CHILD,
        value: Some(c.clone()),
    });
    assert_eq!(p.composites[&PARENT], before);
    assert_eq!(p.composites[&CHILD], c);
    p.apply(&Op::SetComposite {
        id: CHILD,
        value: None,
    });
    assert_eq!(p.composites[&PARENT], before);
    assert!(composite::validate(&p).is_err());
}

#[test]
fn removing_a_module_keeps_composite_level_elements_for_the_prune() {
    let mut p = state();
    p.apply(&Op::RemoveModule { id: FILTER });
    // The knob on the removed filter's control goes with its exposure; the snapshot strip and
    // the button stay (their keys are not exposures), and the dangling parent display waits
    // for `composite::prune`.
    let child = &p.composites[&CHILD];
    let keys: Vec<_> = child
        .panel
        .as_ref()
        .unwrap()
        .placements
        .keys()
        .copied()
        .collect();
    assert_eq!(keys, [3, 5]);
    assert!(p.composites[&PARENT]
        .panel
        .as_ref()
        .unwrap()
        .placements
        .contains_key(&1));
    let pruned = composite::prune(&p);
    for op in &pruned {
        p.apply(op);
    }
    composite::validate(&p).unwrap();
    let parent = &p.composites[&PARENT];
    let keys: Vec<_> = parent
        .panel
        .as_ref()
        .unwrap()
        .placements
        .keys()
        .copied()
        .collect();
    assert_eq!(keys, [2, 3]);
    assert_eq!(
        p.composites[&CHILD].snapshots[0].values,
        BTreeMap::from([(2, 1.0)])
    );
}
