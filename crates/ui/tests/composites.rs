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
    assert!(e
        .state()
        .cables
        .values()
        .all(|c| !b.contains(&c.from.module_id()) || b.contains(&c.to.module_id())));
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
        .find(|c| c.to.module_id() == out)
        .unwrap()
        .from
        .clone();
    e.connect(
        src.clone(),
        PortRef::Module {
            id: chorus,
            port: "in_l".into(),
        },
    );
    e.connect(
        src,
        PortRef::Module {
            id: chorus,
            port: "in_r".into(),
        },
    );
    for (a, b) in [("left", "in_l"), ("right", "in_r")] {
        e.connect(
            PortRef::Module {
                id: chorus,
                port: a.into(),
            },
            PortRef::Module {
                id: reverb,
                port: b.into(),
            },
        );
    }
    for port in ["left", "right"] {
        e.connect(
            PortRef::Module {
                id: reverb,
                port: port.into(),
            },
            PortRef::Module {
                id: out,
                port: port.into(),
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
