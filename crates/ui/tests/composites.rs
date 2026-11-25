use kabl_core::{load, save, PortRef, Vec2};
use kabl_ui::{composites, PatchEditor};
use std::collections::BTreeSet;
fn voice() -> PatchEditor {
    PatchEditor::seed_from(&kabl_standalone::default_patch())
}
fn group(e: &mut PatchEditor) -> u64 {
    let members = e
        .state()
        .modules
        .iter()
        .filter(|(_, m)| m.kind != "out" && m.kind != "midi.in")
        .map(|(&id, _)| id)
        .collect();
    composites::encapsulate(e, members, BTreeSet::new(), "Voice").unwrap()
}
#[test]
fn encapsulate_sound_graph_identity_history_and_save() {
    let mut e = voice();
    let flat = e.state().clone();
    let id = group(&mut e);
    assert_eq!(flat.modules, e.state().modules);
    assert_eq!(flat.cables, e.state().cables);
    assert!(kabl_engine::runtime::runtime_changes(&flat, e.state())
        .unwrap()
        .is_empty());
    let grouped = e.state().clone();
    assert!(e.undo());
    assert_eq!(e.state(), &flat);
    assert!(e.redo());
    assert_eq!(e.state(), &grouped);
    let dir = tempfile::tempdir().unwrap();
    save(dir.path(), e.log()).unwrap();
    let mut restored = PatchEditor::from_log(load(dir.path()).unwrap());
    assert_eq!(restored.state(), e.state());
    assert!(restored.undo());
    assert_eq!(restored.state(), &flat);
    let pkg = composites::package(e.state(), id).unwrap();
    let bytes = serde_json::to_vec(&pkg).unwrap();
    assert_eq!(composites::decode(&bytes).unwrap().patch, pkg.patch);
}
#[test]
fn duplicates_private_graph_and_stable_interfaces() {
    let mut e = voice();
    let id = group(&mut e);
    let original = e.state().clone();
    let copy = composites::duplicate(&mut e, id).unwrap();
    let a = kabl_core::composite::leaves(e.state(), id);
    let b = kabl_core::composite::leaves(e.state(), copy);
    assert!(a.is_disjoint(&b));
    for (&cid, c) in &original.cables {
        assert_eq!(e.state().cables.get(&cid), Some(c));
    }
    assert!(e.state().cables.values().all(
        |c| !b.contains(&c.from.module_id().unwrap()) || b.contains(&c.to.module_id().unwrap())
    ));
    let leaf = *b
        .iter()
        .find(|id| e.state().modules[id].kind == "filter.svf")
        .unwrap();
    e.set_param(leaf, "cutoff_hz", 800.0);
    for id in a {
        assert_eq!(e.state().modules[&id], original.modules[&id]);
    }
    let edited = e.state().clone();
    e.undo();
    e.redo();
    assert_eq!(e.state(), &edited);
    let mut c = e.state().composites[&copy].clone();
    let keys = c.ports.keys().copied().collect::<Vec<_>>();
    for p in c.ports.values_mut() {
        p.label = "Renamed".into();
    }
    composites::set(&mut e, copy, c).unwrap();
    assert_eq!(
        e.state().composites[&copy]
            .ports
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        keys
    );
}
#[test]
fn malformed_recursion_limits_and_missing_dependencies_are_atomic() {
    let mut e = voice();
    let id = group(&mut e);
    let before = e.state().clone();
    let entries = e.log().entries().len();
    let counters = e.reserved_ids();
    let mut c = before.composites[&id].clone();
    c.parent = Some(id);
    assert!(composites::set(&mut e, id, c)
        .unwrap_err()
        .contains("recursive"));
    assert_eq!(e.state(), &before);
    assert_eq!(entries, e.log().entries().len());
    assert_eq!(counters, e.reserved_ids());
    let pkg = composites::package(&before, id).unwrap();
    let mut bad = pkg.clone();
    bad.patch.modules.values_mut().next().unwrap().kind = "missing.dsp".into();
    assert!(composites::insert(&mut e, &bad).is_err());
    assert_eq!(e.state(), &before);
    let mut duplicate = serde_json::to_string(&pkg).unwrap();
    duplicate=duplicate.replacen("\"modules\":{","\"modules\":{\"999\":{\"kind\":\"out\",\"pos\":{\"x\":0,\"y\":0},\"params\":{}},\"999\":{\"kind\":\"out\",\"pos\":{\"x\":0,\"y\":0},\"params\":{}},",1);
    assert!(composites::decode(duplicate.as_bytes())
        .unwrap_err()
        .contains("Duplicate"));
    let mut current = id;
    for _ in 1..8 {
        current =
            composites::encapsulate(&mut e, BTreeSet::new(), BTreeSet::from([current]), "Nested")
                .unwrap();
    }
    let before = e.state().clone();
    assert!(composites::encapsulate(
        &mut e,
        BTreeSet::new(),
        BTreeSet::from([current]),
        "Too deep"
    )
    .unwrap_err()
    .contains("nesting"));
    assert_eq!(e.state(), &before);
}
#[test]
fn publish_immutable_versions_embedded_recall_and_controller_omissions() {
    let mut e = voice();
    let id = group(&mut e);
    let leaf = *kabl_core::composite::leaves(e.state(), id)
        .iter()
        .next()
        .unwrap();
    e.set_param(leaf, "cc.base_hz", 20.0);
    e.set_param(leaf, "btn.restart", 21.0);
    e.set_param(leaf, "pin.base_hz", 0.0);
    let dir = tempfile::tempdir().unwrap();
    let v1 = composites::publish(e.state(), id, dir.path()).unwrap();
    let old = std::fs::read(&v1).unwrap();
    e.set_param(leaf, "base_hz", 400.0);
    let v2 = composites::publish(e.state(), id, dir.path()).unwrap();
    assert_ne!(v1, v2);
    assert_eq!(std::fs::read(&v1).unwrap(), old);
    let copy = composites::duplicate(&mut e, id).unwrap();
    for leaf in kabl_core::composite::leaves(e.state(), copy) {
        assert!(e.state().modules[&leaf]
            .params
            .keys()
            .all(|key| !key.starts_with("cc.") && !key.starts_with("btn.")));
    }
    let state = e.state().clone();
    let bytes = serde_json::to_vec(&state).unwrap();
    std::fs::remove_dir_all(dir.path()).unwrap();
    let recalled: kabl_core::PatchState = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(state, recalled);
    kabl_engine::compile::compile(&recalled, 48000.0, 8).unwrap();
}
#[test]
fn historical_ids_and_deletion_undo_never_alias() {
    let mut e = voice();
    let old = e.add_module("lfo", Vec2 { x: 1000.0, y: 10.0 });
    e.remove_module(old);
    let dir = tempfile::tempdir().unwrap();
    save(dir.path(), e.log()).unwrap();
    let mut e = PatchEditor::from_log(load(dir.path()).unwrap());
    let fresh = e.add_module("lfo", Vec2 { x: 1000.0, y: 10.0 });
    assert!(fresh > old);
    let id = group(&mut e);
    let leaf = *kabl_core::composite::leaves(e.state(), id)
        .iter()
        .next()
        .unwrap();
    let param = registry_param(&e, leaf);
    composites::add_exposure(&mut e, id, PortRef::Param { id: leaf, param }, true).unwrap();
    let before = e.state().clone();
    e.remove_module(leaf);
    kabl_core::composite::validate(e.state()).unwrap();
    e.undo();
    assert_eq!(e.state(), &before);
}
fn e_kind(e: &PatchEditor, id: u64) -> String {
    e.state().modules[&id].kind.clone()
}
fn registry_param(e: &PatchEditor, id: u64) -> String {
    kabl_modules::registry::info_for(&e_kind(e, id))
        .unwrap()
        .params[0]
        .name
        .into()
}

#[test]
fn exact_pcm_voice_stereo_effect_feedback_and_no_callback_allocation() {
    fn render(p: &kabl_core::PatchState) -> Vec<[f32; 2]> {
        let mut engine = kabl_engine::compile::compile(p, 48000.0, 8).unwrap();
        engine.note_on(0, 0.0, 0.8);
        engine.note_on(1, 7.0, 0.65);
        let mut audio = Vec::new();
        for n in 0..1800 {
            if n == 600 {
                engine.note_off(0);
                engine.note_off(1);
            }
            assert_no_alloc::assert_no_alloc(|| engine.process_block());
            for (&l, &r) in engine.left().iter().zip(engine.right()) {
                audio.push([l, r]);
            }
        }
        assert!(audio.iter().flatten().all(|v| v.is_finite()));
        assert!(audio.iter().flatten().any(|v| v.abs() > 0.001));
        audio
    }
    let mut e = voice();
    let voice_id = group(&mut e);
    let chorus = e.add_module("chorus", Vec2 { x: 240.0, y: 780.0 });
    let reverb = e.add_module("reverb", Vec2 { x: 480.0, y: 780.0 });
    let original = e.state().clone();
    let out = *original
        .modules
        .iter()
        .find(|(_, m)| m.kind == "out")
        .unwrap()
        .0;
    let src = original
        .cables
        .values()
        .find(|c| c.to.module_id() == Some(out))
        .unwrap()
        .from
        .clone();
    e.connect(
        src,
        PortRef::Module {
            id: chorus,
            port: "in".into(),
        },
    );
    for (from, to) in [(chorus, reverb), (reverb, out)] {
        e.connect(
            PortRef::Module {
                id: from,
                port: "out".into(),
            },
            PortRef::Module {
                id: to,
                port: "in".into(),
            },
        );
    }
    let flat = e.state().clone();
    let fx = composites::encapsulate(
        &mut e,
        BTreeSet::from([chorus, reverb]),
        BTreeSet::new(),
        "Stereo room",
    )
    .unwrap();
    let nested = composites::encapsulate(
        &mut e,
        BTreeSet::new(),
        BTreeSet::from([voice_id, fx]),
        "Voice with stereo room",
    )
    .unwrap();
    assert_eq!(render(&flat), render(e.state()));
    let lfo = e.add_module("lfo", Vec2 { x: 10.0, y: 1150.0 });
    let filter = *e
        .state()
        .modules
        .iter()
        .find(|(_, m)| m.kind == "filter.svf")
        .unwrap()
        .0;
    e.connect(
        PortRef::Module {
            id: lfo,
            port: "out".into(),
        },
        PortRef::Param {
            id: filter,
            param: "cutoff_hz".into(),
        },
    );
    e.connect(
        PortRef::Module {
            id: filter,
            port: "lp".into(),
        },
        PortRef::Param {
            id: lfo,
            param: "rate_hz".into(),
        },
    );
    let cycle = e.state().clone();
    let _ = composites::encapsulate(
        &mut e,
        BTreeSet::from([lfo]),
        BTreeSet::from([nested]),
        "Feedback boundary",
    )
    .unwrap();
    assert_eq!(render(&cycle), render(e.state()));
}
#[global_allocator]
static ALLOCATOR: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

#[test]
fn real_widgets_open_and_edit_aliases_at_both_sizes_and_themes() {
    for (w, h) in [(1440.0, 900.0), (1280.0, 800.0)] {
        for dark in [false, true] {
            let mut e = voice();
            let id = group(&mut e);
            composites::add_exposure(
                &mut e,
                id,
                PortRef::Param {
                    id: 3,
                    param: "cutoff_hz".into(),
                },
                true,
            )
            .unwrap();
            let ctx = egui::Context::default();
            let mut view = kabl_ui::UiState::default();
            view.dark = dark;
            let mut time = 0.0;
            fn frame(
                ctx: &egui::Context,
                e: &mut PatchEditor,
                view: &mut kabl_ui::UiState,
                w: f32,
                h: f32,
                time: &mut f64,
                events: Vec<egui::Event>,
            ) {
                ctx.begin_pass(egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(w, h),
                    )),
                    time: Some(*time),
                    events,
                    ..Default::default()
                });
                *time += 0.02;
                let mut root = egui::Ui::new(
                    ctx.clone(),
                    egui::Id::new("test-root"),
                    egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(w, h),
                    )),
                );
                kabl_ui::show(e, view, &mut root);
                let _ = ctx.end_pass();
            }
            for _ in 0..3 {
                frame(&ctx, &mut e, &mut view, w, h, &mut time, vec![]);
            }
            let key = format!("composite:{id}:control:4"); // three boundary exposures precede this control
            let r = *view.hits.get(&key).expect("public alias drawn");
            let before = e.state().modules[&3].params["cutoff_hz"];
            let at = egui::pos2(r.left() + 18.0, r.center().y);
            for pressed in [true, false] {
                frame(
                    &ctx,
                    &mut e,
                    &mut view,
                    w,
                    h,
                    &mut time,
                    vec![
                        egui::Event::PointerMoved(at),
                        egui::Event::PointerButton {
                            pos: at,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
            assert_ne!(before, e.state().modules[&3].params["cutoff_hz"]);
            assert_eq!(
                e.state().composites[&id].controls[&4].target,
                PortRef::Param {
                    id: 3,
                    param: "cutoff_hz".into()
                }
            );
            let at = view.hits[&format!("composite:{id}:open")].center();
            for pressed in [true, false] {
                frame(
                    &ctx,
                    &mut e,
                    &mut view,
                    w,
                    h,
                    &mut time,
                    vec![
                        egui::Event::PointerMoved(at),
                        egui::Event::PointerButton {
                            pos: at,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
            frame(&ctx, &mut e, &mut view, w, h, &mut time, vec![]);
            assert!(view.composites.open.contains(&id));
            assert!(view.hits.contains_key("module:3"));
        }
    }
}

#[test]
fn clearing_last_leaf_label_preserves_ownership_and_public_ids() {
    let mut e = voice();
    let id = group(&mut e);
    let leaf = *kabl_core::composite::leaves(e.state(), id)
        .iter()
        .next()
        .unwrap();
    let composite = e.state().composites[&id].clone();
    e.set_label(leaf, "name", Some("Named".into()));
    e.set_label(leaf, "name", None);
    assert_eq!(e.state().composites[&id], composite);
    assert!(!e.state().labels.contains_key(&leaf));
    e.undo();
    assert_eq!(e.state().composites[&id], composite);
}

#[test]
fn packed_faces_and_copied_binding_labels_identify_correct_leaves() {
    let mut e = voice();
    // Packed rack allows every raw position to match; cards must still avoid ungrouped leaves.
    for id in e.state().modules.keys().copied().collect::<Vec<_>>() {
        e.move_module(id, Vec2 { x: 24.0, y: 10.0 });
    }
    let id = group(&mut e);
    let lay = kabl_ui::rack::layout(e.state(), &kabl_ui::rack::View::default());
    let faces = composites::faces(e.state(), &BTreeSet::new(), &lay);
    let card = faces[&id];
    for module in &lay.mods {
        if !composites::hidden(e.state(), &BTreeSet::new(), module.id) {
            assert!(!card.shrink(0.1).intersects(module.rect));
        }
    }
    let copy = composites::duplicate(&mut e, id).unwrap();
    for exposure in e.state().composites[&copy].ports.values() {
        assert_eq!(exposure.label, composites::binding_label(&exposure.target));
    }
    let pkg = composites::package(e.state(), id).unwrap();
    let mut value = serde_json::to_value(&pkg).unwrap();
    let leaf = *pkg.patch.modules.keys().next().unwrap();
    value["patch"]["labels"] = serde_json::json!({leaf.to_string(): {"name": "first"}});
    let bytes = serde_json::to_string(&value).unwrap().replace(
        "\"name\":\"first\"",
        "\"name\":\"first\",\"name\":\"second\"",
    );
    let before = e.state().clone();
    let entries = e.log().entries().len();
    let ids = e.reserved_ids();
    assert!(composites::decode(bytes.as_bytes())
        .unwrap_err()
        .contains("Duplicate"));
    assert_eq!(e.state(), &before);
    assert_eq!(e.log().entries().len(), entries);
    assert_eq!(e.reserved_ids(), ids);
}

#[test]
fn cue_clock_dependencies_preserve_external_ids_or_require_inclusion() {
    let mut e = voice();
    let clock = e.add_module("clock", Vec2 { x: 24.0, y: 750.0 });
    let seq = e.add_module("seq", Vec2 { x: 270.0, y: 750.0 });
    let cues = e.add_module("cues", Vec2 { x: 24.0, y: 1120.0 });
    e.set_param(cues, "cue0.clock", clock as f32);
    e.set_param(cues, &format!("cue0.seq.{seq}"), 1.0);
    let root = composites::encapsulate(
        &mut e,
        BTreeSet::from([cues]),
        BTreeSet::new(),
        "Cue controller",
    )
    .unwrap();
    let copy = composites::duplicate(&mut e, root).unwrap();
    let leaf = *e.state().composites[&copy].members.iter().next().unwrap();
    assert_eq!(e.state().modules[&leaf].params["cue0.clock"], clock as f32);
    assert_eq!(
        e.state().modules[&leaf].params[&format!("cue0.seq.{seq}")],
        1.0
    );
    assert!(composites::package(e.state(), root)
        .unwrap_err()
        .contains("include referenced module"));
    let closed = composites::encapsulate(
        &mut e,
        BTreeSet::from([clock, seq]),
        BTreeSet::from([root]),
        "Portable cues",
    )
    .unwrap();
    let pkg = composites::package(e.state(), closed).unwrap();
    let mut destination = voice();
    let inserted = composites::insert(&mut destination, &pkg).unwrap();
    let leaves = kabl_core::composite::leaves(destination.state(), inserted);
    let find = |kind| {
        *leaves
            .iter()
            .find(|id| destination.state().modules[id].kind == kind)
            .unwrap()
    };
    let fresh_clock = find("clock");
    let fresh_seq = find("seq");
    let fresh_cues = find("cues");
    assert_eq!(
        destination.state().modules[&fresh_cues].params["cue0.clock"],
        fresh_clock as f32
    );
    assert_eq!(
        destination.state().modules[&fresh_cues].params[&format!("cue0.seq.{fresh_seq}")],
        1.0
    );
}

#[test]
fn malformed_project_load_and_seeded_nested_undo_are_atomic() {
    let mut e = voice();
    let child = group(&mut e);
    composites::encapsulate(&mut e, BTreeSet::new(), BTreeSet::from([child]), "Parent").unwrap();
    let p = e.state().clone();
    let mut seeded = PatchEditor::seed_from(&p);
    assert!(seeded.undo());
    assert!(seeded.state().composites.is_empty());
    assert_eq!(seeded.state().modules, p.modules);
    assert!(seeded.redo());
    assert_eq!(seeded.state(), &p);
    let mut broken = p.composites[&child].clone();
    broken.members.insert(999);
    e.restore_to(vec![kabl_core::Op::SetComposite {
        id: child,
        value: Some(broken),
    }]);
    let dir = tempfile::tempdir().unwrap();
    save(dir.path(), e.log()).unwrap();
    assert!(format!("{:?}", load(dir.path()).unwrap_err())
        .contains("missing or multiply owned module 999"));
}

#[test]
fn chronological_project_undo_keeps_interleaved_instance_state_independent() {
    let mut e = voice();
    let a = group(&mut e);
    let b = composites::duplicate(&mut e, a).unwrap();
    let filter = |group| {
        *kabl_core::composite::leaves(e.state(), group)
            .iter()
            .find(|id| e.state().modules[id].kind == "filter.svf")
            .unwrap()
    };
    let (a_filter, b_filter) = (filter(a), filter(b));
    let before = e.state().clone();
    e.set_param(a_filter, "cutoff_hz", 800.0);
    let after_a = e.state().clone();
    assert_eq!(after_a.modules[&b_filter], before.modules[&b_filter]);
    e.set_param(b_filter, "cutoff_hz", 1200.0);
    let after_b = e.state().clone();
    assert_eq!(after_b.modules[&a_filter], after_a.modules[&a_filter]);
    e.set_param(a_filter, "cutoff_hz", 1000.0);
    let after_a2 = e.state().clone();
    assert_eq!(after_a2.modules[&b_filter], after_b.modules[&b_filter]);
    assert_eq!(after_a2.composites, before.composites);
    assert_eq!(after_a2.cables, before.cables);
    // Loading retains the same chronological project history, not one stack per instance.
    let dir = tempfile::tempdir().unwrap();
    save(dir.path(), e.log()).unwrap();
    let mut restored = PatchEditor::from_log(load(dir.path()).unwrap());
    assert!(restored.undo());
    assert_eq!(restored.state(), &after_b);
    assert!(restored.undo());
    assert_eq!(restored.state(), &after_a);
    assert!(restored.undo());
    assert_eq!(restored.state(), &before);
    assert!(restored.redo());
    assert_eq!(restored.state(), &after_a);
    assert!(restored.redo());
    assert_eq!(restored.state(), &after_b);
    assert!(restored.redo());
    assert_eq!(restored.state(), &after_a2);
}

mod faces {
    use kabl_core::composite::{self, Snapshot};
    use kabl_core::panel::{Kind, Panel, Placement, Source};
    use kabl_core::{Composite, Definition, Exposure, ModuleId, Op, PatchState, PortRef, Vec2};
    use kabl_ui::{compare, composites, PatchEditor};
    use std::collections::BTreeMap;

    /// The parent's id is below its nested child's, so restoring composites in id order puts
    /// the parent back first.
    const PARENT: u64 = 1;
    const CHILD: u64 = 2;

    struct Ids {
        clock: ModuleId,
        cues: ModuleId,
        osc: ModuleId,
        filter: ModuleId,
    }

    fn placement(kind: Kind, x: f32, w: f32, h: f32, source: Option<Source>) -> Placement {
        Placement {
            kind,
            x,
            y: 72.0,
            width: w,
            height: h,
            source,
            label: None,
            icons: Vec::new(),
        }
    }

    fn composite(name: &str, parent: Option<u64>, members: &[ModuleId]) -> Composite {
        Composite {
            name: name.into(),
            help: String::new(),
            definition: Definition {
                id: format!("test.{name}"),
                version: 1,
            },
            parent,
            members: members.iter().copied().collect(),
            pos: Vec2::default(),
            ports: BTreeMap::new(),
            controls: BTreeMap::new(),
            next_interface: 1,
            panel: None,
            snapshots: Vec::new(),
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

    fn snapshot(id: u64, clock: ModuleId) -> Snapshot {
        Snapshot {
            id,
            name: format!("S{id}"),
            values: BTreeMap::from([(1, 800.0), (2, 1.0)]),
            morph_beats: 2.0,
            clock: Some(clock),
        }
    }

    fn placements(c: &mut Composite) -> &mut BTreeMap<u64, Placement> {
        &mut c.panel.as_mut().unwrap().placements
    }

    fn keys(p: &PatchState, id: u64) -> Vec<u64> {
        p.composites[&id]
            .panel
            .as_ref()
            .unwrap()
            .placements
            .keys()
            .copied()
            .collect()
    }

    /// Parent (clock, cues): display on the child's filter (key 1), a button pressing the
    /// child's button 3 (key 2), transport on the clock (key 3), cue pads (key 4).
    /// Child (osc, filter): knob on cutoff (key 1), button on snapshot 4 (key 3), button on
    /// snapshot 6 (key 7), snapshot strip (key 5); snapshots 4 and 6 morph on the clock.
    fn fixture() -> (PatchEditor, Ids) {
        let mut e = PatchEditor::new();
        let pos = Vec2::default();
        let ids = Ids {
            clock: e.add_module("clock", pos),
            cues: e.add_module("cues", pos),
            osc: e.add_module("osc.va", pos),
            filter: e.add_module("filter.svf", pos),
        };
        e.set_param(ids.cues, "cue1.clock", ids.clock as f32);
        let module = |id| Some(Source::Module { id, action: None });
        let mut parent = composite("parent", None, &[ids.clock, ids.cues]);
        parent.next_interface = 5;
        parent.panel = Some(Box::new(Panel {
            width: 600.0,
            placements: BTreeMap::from([
                (
                    1,
                    placement(Kind::Display, 12.0, 120.0, 60.0, module(ids.filter)),
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
                    placement(Kind::Transport, 200.0, 120.0, 32.0, module(ids.clock)),
                ),
                (
                    4,
                    placement(Kind::CuePads, 330.0, 120.0, 120.0, module(ids.cues)),
                ),
            ]),
            ..Panel::default()
        }));
        let mut child = composite("child", Some(PARENT), &[ids.osc, ids.filter]);
        child.controls = BTreeMap::from([
            (1, control(ids.filter, "cutoff_hz")),
            (2, control(ids.osc, "waveform")),
        ]);
        child.next_interface = 8;
        child.snapshots = vec![snapshot(4, ids.clock), snapshot(6, ids.clock)];
        let snap = |id| Some(Source::Snapshot { id });
        child.panel = Some(Box::new(Panel {
            placements: BTreeMap::from([
                (1, placement(Kind::Knob, 12.0, 84.0, 110.0, None)),
                (3, placement(Kind::Button, 104.0, 54.0, 40.0, snap(4))),
                (7, placement(Kind::Button, 104.0, 54.0, 40.0, snap(6))),
                (5, placement(Kind::Snapshots, 170.0, 54.0, 40.0, None)),
            ]),
            ..Panel::default()
        }));
        placements(&mut child).get_mut(&7).unwrap().y = 200.0;
        e.edit(vec![
            Op::SetComposite {
                id: PARENT,
                value: Some(parent),
            },
            Op::SetComposite {
                id: CHILD,
                value: Some(child),
            },
        ]);
        composite::validate(e.state()).unwrap();
        (e, ids)
    }

    /// The step left a valid patch, and one undo puts back exactly `before`.
    fn valid_and_undoes_to(e: &mut PatchEditor, before: &PatchState) {
        composite::validate(e.state()).unwrap();
        assert!(e.undo());
        assert_eq!(e.state(), before);
    }

    #[test]
    fn removing_a_module_with_cue_references_prunes_its_face_elements_and_snapshot_clocks() {
        let (mut e, ids) = fixture();
        let before = e.state().clone();
        e.remove_module(ids.clock);
        assert_eq!(keys(e.state(), PARENT), [1, 2, 4]);
        assert!(e.state().composites[&CHILD]
            .snapshots
            .iter()
            .all(|s| s.clock.is_none()));
        valid_and_undoes_to(&mut e, &before);
    }

    #[test]
    fn an_edit_removing_a_button_drops_face_buttons_pressing_it() {
        let (mut e, _) = fixture();
        let before = e.state().clone();
        let mut child = before.composites[&CHILD].clone();
        placements(&mut child).remove(&3);
        e.edit(vec![Op::SetComposite {
            id: CHILD,
            value: Some(child),
        }]);
        assert_eq!(keys(e.state(), PARENT), [1, 3, 4]);
        valid_and_undoes_to(&mut e, &before);
    }

    #[test]
    fn a_restore_prunes_what_it_leaves_dangling() {
        let (mut e, _) = fixture();
        let before = e.state().clone();
        let mut target = before.clone();
        target.composites.get_mut(&CHILD).unwrap().snapshots.clear();
        e.restore_to(compare::diff(e.state(), &target).unwrap());
        // The snapshot buttons go, and with them the parent's button pressing one.
        assert_eq!(keys(e.state(), CHILD), [1, 5]);
        assert_eq!(keys(e.state(), PARENT), [1, 3, 4]);
        valid_and_undoes_to(&mut e, &before);
    }

    #[test]
    fn removing_a_composite_drops_dependents_in_its_parent() {
        let (mut e, _) = fixture();
        let before = e.state().clone();
        e.edit(vec![Op::SetComposite {
            id: CHILD,
            value: None,
        }]);
        // The display's filter is no longer a leaf of the parent, and the pressed button is gone.
        assert_eq!(keys(e.state(), PARENT), [3, 4]);
        valid_and_undoes_to(&mut e, &before);
    }

    #[test]
    fn deleting_a_snapshot_keeps_other_buttons_on_their_snapshots() {
        let (mut e, _) = fixture();
        let before = e.state().clone();
        let mut child = before.composites[&CHILD].clone();
        child.snapshots.retain(|s| s.id != 4);
        e.edit(vec![Op::SetComposite {
            id: CHILD,
            value: Some(child),
        }]);
        let p = e.state();
        assert_eq!(keys(p, CHILD), [1, 5, 7]);
        assert_eq!(
            p.composites[&CHILD].panel.as_ref().unwrap().placements[&7].source,
            Some(Source::Snapshot { id: 6 })
        );
        assert_eq!(keys(p, PARENT), [1, 3, 4]);
        valid_and_undoes_to(&mut e, &before);
    }

    #[test]
    fn shrinking_members_drops_dependents_in_every_parent() {
        let (mut e, ids) = fixture();
        let before = e.state().clone();
        let mut child = before.composites[&CHILD].clone();
        child.members.remove(&ids.filter);
        child.controls.remove(&1);
        e.edit(vec![Op::SetComposite {
            id: CHILD,
            value: Some(child),
        }]);
        let p = e.state();
        assert_eq!(keys(p, CHILD), [3, 5, 7]);
        assert!(p.composites[&CHILD]
            .snapshots
            .iter()
            .all(|s| s.values == BTreeMap::from([(2, 1.0)])));
        assert_eq!(keys(p, PARENT), [2, 3, 4]);
        valid_and_undoes_to(&mut e, &before);
    }

    #[test]
    fn removing_an_exposure_drops_its_key_from_every_snapshot() {
        let (mut e, ids) = fixture();
        // The face holds only kinds that draw today, so the edit commits.
        let mut child = e.state().composites[&CHILD].clone();
        placements(&mut child).retain(|_, p| p.kind == Kind::Knob);
        let mut lone = composite("lone", None, &[ids.osc, ids.filter]);
        lone.controls = child.controls;
        lone.next_interface = child.next_interface;
        lone.snapshots = child.snapshots;
        lone.panel = child.panel;
        e.edit(vec![
            Op::SetComposite {
                id: CHILD,
                value: None,
            },
            Op::SetComposite {
                id: PARENT,
                value: None,
            },
            Op::SetComposite {
                id: 3,
                value: Some(lone),
            },
        ]);
        composites::remove_exposure(&mut e, 3, 1, true).unwrap();
        for s in &e.state().composites[&3].snapshots {
            assert_eq!(s.values, BTreeMap::from([(2, 1.0)]), "{}", s.id);
        }
    }
}
