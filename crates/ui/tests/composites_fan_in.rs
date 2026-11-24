//! Fan-in (gap G3): one outside source feeding several targets inside a group becomes one
//! public port with every target, by interactive and by curated encapsulation. Wrapping
//! changes no cable, so the sound is bit-identical.
use kabl_core::format::schema_for;
use kabl_core::{CableId, Definition, Exposure, ModuleId, PatchLog, PatchState, PortRef, Vec2};
use kabl_engine::compile::DEPTH;
use kabl_ui::{composites, PatchEditor};
use std::collections::BTreeSet;

const MIDI: ModuleId = 1;
const OSC: ModuleId = 2;
const FILTER: ModuleId = 3;
const ENV: ModuleId = 4;
const VCA: ModuleId = 5;

fn jack(id: ModuleId, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}
fn param(id: ModuleId, name: &str) -> PortRef {
    PortRef::Param {
        id,
        param: name.into(),
    }
}

/// The default voice with a second oscillator and a second envelope: pitch feeds both
/// oscillators, gate both envelopes, and a mixer sums the oscillators into the filter.
struct Voice {
    editor: PatchEditor,
    osc_b: ModuleId,
    env_b: ModuleId,
    mixer: ModuleId,
    /// The second envelope's route into the filter cutoff.
    route: CableId,
}

fn voice() -> Voice {
    let mut e = PatchEditor::seed_from(&kabl_standalone::default_patch());
    let osc_b = e.add_module("osc.va", Vec2 { x: 200.0, y: 300.0 });
    let env_b = e.add_module("env.adsr", Vec2 { x: 400.0, y: 300.0 });
    let mixer = e.add_module("mixer", Vec2 { x: 600.0, y: 300.0 });
    e.connect(jack(MIDI, "pitch"), jack(osc_b, "pitch"));
    e.connect(jack(MIDI, "gate"), jack(env_b, "gate"));
    e.connect(jack(OSC, "out"), jack(mixer, "in1"));
    e.connect(jack(osc_b, "out"), jack(mixer, "in2"));
    e.connect(jack(mixer, "out"), jack(FILTER, "in"));
    let route = e.connect_route(jack(env_b, "out"), FILTER, "cutoff_hz");
    Voice {
        editor: e,
        osc_b,
        env_b,
        mixer,
        route,
    }
}

impl Voice {
    fn members(&self) -> BTreeSet<ModuleId> {
        BTreeSet::from([OSC, FILTER, ENV, VCA, self.osc_b, self.env_b, self.mixer])
    }
    /// Interactive encapsulation of the whole voice.
    fn wrap(&mut self) -> u64 {
        let members = self.members();
        composites::encapsulate(&mut self.editor, members, BTreeSet::new(), "Voice").unwrap()
    }
    fn pitch(&self) -> Vec<PortRef> {
        vec![jack(OSC, "pitch"), jack(self.osc_b, "pitch")]
    }
    fn gate(&self) -> Vec<PortRef> {
        vec![jack(ENV, "gate"), jack(self.env_b, "gate")]
    }
}

fn render(p: &PatchState) -> String {
    let _ftz = kabl_engine::ftz::FtzGuard::new();
    let mut c = kabl_engine::compile::compile(p, 48000.0, 8).unwrap();
    kabl_engine::golden::chord_hash(&mut c, 400, 200, |_, _| {})
}

fn targets(p: &PatchState, id: u64) -> Vec<Vec<PortRef>> {
    p.composites[&id]
        .ports
        .values()
        .map(|e| e.targets().cloned().collect())
        .collect()
}

fn exposure(label: &str, mut targets: Vec<PortRef>) -> Exposure {
    let target = targets.remove(0);
    Exposure {
        label: label.into(),
        target,
        also: targets,
    }
}

fn curated(log: &mut PatchLog, v: &Voice, ports: &[Exposure]) -> Result<u64, String> {
    composites::encapsulate_with(
        log,
        "Voice",
        Definition {
            id: "kabl.test-voice".into(),
            version: 1,
        },
        v.members(),
        ports,
        &[],
        None,
    )
}

fn curated_ports(v: &Voice) -> Vec<Exposure> {
    vec![
        exposure("Pitch", v.pitch()),
        exposure("Gate", v.gate()),
        exposure("Out", vec![jack(VCA, "out")]),
    ]
}

#[test]
fn interactive_encapsulate_gives_one_pitch_and_one_gate_port() {
    let mut v = voice();
    let flat = v.editor.state().clone();
    let id = v.wrap();
    let p = v.editor.state();
    assert_eq!(
        targets(p, id),
        vec![v.pitch(), v.gate(), vec![jack(VCA, "out")]]
    );
    let labels: Vec<&str> = p.composites[&id]
        .ports
        .values()
        .map(|e| e.label.as_str())
        .collect();
    assert_eq!(labels.len(), 3, "{labels:?}");
    assert_eq!(p.cables, flat.cables, "wrapping leaves every cable alone");
    assert_eq!(render(&flat), render(p));
    assert_eq!(schema_for(v.editor.log()), 9);
}

#[test]
fn curated_encapsulate_takes_one_port_per_source() {
    let v = voice();
    let flat = v.editor.state().clone();
    let mut log = v.editor.log().clone();
    let id = curated(&mut log, &v, &curated_ports(&v)).unwrap();
    assert_eq!(
        targets(log.state(), id),
        vec![v.pitch(), v.gate(), vec![jack(VCA, "out")]]
    );
    assert_eq!(render(&flat), render(log.state()));
    assert_eq!(schema_for(&log), 9);

    // A port that names only one of the fanned targets leaves the other cable uncovered.
    let mut log = v.editor.log().clone();
    let mut short = curated_ports(&v);
    short[0] = exposure("Pitch", vec![jack(OSC, "pitch")]);
    let err = curated(&mut log, &v, &short).unwrap_err();
    assert!(err.contains("No curated port covers cable"), "{err}");
}

#[test]
fn one_source_into_a_port_and_a_parameter_is_one_port() {
    let mut v = voice();
    let macros = v.editor.add_module("macro", Vec2 { x: 24.0, y: 600.0 });
    v.editor.connect(jack(macros, "m1"), jack(v.mixer, "in3"));
    v.editor
        .connect_route(jack(macros, "m1"), FILTER, "resonance");
    let amount = PortRef::CableParam {
        cable: v.route,
        param: DEPTH.into(),
    };
    v.editor
        .connect_cable_route(jack(macros, "m1"), v.route, DEPTH)
        .unwrap();
    let flat = v.editor.state().clone();
    let id = v.wrap();
    let p = v.editor.state();
    let mixed = vec![
        jack(v.mixer, "in3"),
        param(FILTER, "resonance"),
        amount.clone(),
    ];
    assert!(targets(p, id).contains(&mixed), "{:?}", targets(p, id));
    assert_eq!(render(&flat), render(p));

    // Removing the route drops only its amount from the port; undo restores it exactly.
    let before = p.composites[&id].clone();
    v.editor.disconnect(v.route);
    assert!(targets(v.editor.state(), id).contains(&mixed[..2].to_vec()));
    assert!(v.editor.undo());
    assert_eq!(v.editor.state().composites[&id], before);
}

#[test]
fn only_inside_targets_join_a_port() {
    let mut v = voice();
    let outside = v.editor.add_module("osc.va", Vec2 { x: 24.0, y: 800.0 });
    let cable = v
        .editor
        .connect(jack(MIDI, "pitch"), jack(outside, "pitch"));
    let id = v.wrap();
    let p = v.editor.state();
    assert!(targets(p, id).contains(&v.pitch()));
    assert_eq!(p.cables[&cable].from, jack(MIDI, "pitch"));
    assert_eq!(p.cables[&cable].to, jack(outside, "pitch"));
}

#[test]
fn removing_targets_keeps_the_rest_until_the_last_and_undo_restores() {
    let mut v = voice();
    let id = v.wrap();
    let wrapped = v.editor.state().clone();
    v.editor.remove_module(v.osc_b);
    assert!(targets(v.editor.state(), id).contains(&vec![jack(OSC, "pitch")]));
    v.editor.remove_module(OSC);
    let left = targets(v.editor.state(), id);
    assert!(
        !left.iter().flatten().any(|t| t.module_id() == Some(OSC)),
        "{left:?}"
    );
    assert_eq!(left.len(), 2, "the pitch port is gone: {left:?}");
    assert!(v.editor.undo() && v.editor.undo());
    assert_eq!(v.editor.state(), &wrapped);
    assert!(v.editor.redo());
    assert!(targets(v.editor.state(), id).contains(&vec![jack(OSC, "pitch")]));
}

#[test]
fn duplicating_and_publishing_keep_every_target() {
    let mut v = voice();
    let id = v.wrap();
    let pkg = composites::package(v.editor.state(), id).unwrap();
    assert_eq!(pkg.schema, 2);
    let dir = tempfile::tempdir().unwrap();
    let path = composites::publish(v.editor.state(), id, dir.path()).unwrap();
    let back = composites::decode(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(back.schema, 2);
    assert_eq!(targets(&back.patch, id), targets(v.editor.state(), id));

    let copy = composites::duplicate(&mut v.editor, id).unwrap();
    let p = v.editor.state();
    let leaves = kabl_core::composite::leaves(p, copy);
    let copied = targets(p, copy);
    assert_eq!(
        copied.iter().map(Vec::len).collect::<Vec<_>>(),
        vec![2, 2, 1]
    );
    assert!(copied
        .iter()
        .flatten()
        .all(|t| t.module_id().is_some_and(|m| leaves.contains(&m))));
    kabl_core::composite::validate(p).unwrap();
}

#[test]
fn an_older_group_with_one_port_per_target_keeps_n_ports() {
    let v = voice();
    let mut log = v.editor.log().clone();
    let mut ports = curated_ports(&v);
    ports.splice(
        0..2,
        v.pitch()
            .into_iter()
            .chain(v.gate())
            .enumerate()
            .map(|(i, t)| exposure(&format!("In {i}"), vec![t])),
    );
    let id = curated(&mut log, &v, &ports).unwrap();
    assert_eq!(schema_for(&log), 8);
    let dir = tempfile::tempdir().unwrap();
    kabl_core::format::save(dir.path(), &log).unwrap();
    let back = kabl_core::format::load(dir.path()).unwrap();
    assert_eq!(targets(back.state(), id).len(), 5);
    assert_eq!(back.state(), log.state());
}

#[test]
fn a_cable_onto_a_face_port_wires_every_target_and_disconnect_removes_all() {
    let mut v = voice();
    let id = v.wrap();
    let lfo = v.editor.add_module("macro", Vec2 { x: 24.0, y: 600.0 });
    let closed = BTreeSet::new();
    let before = v.editor.state().clone();

    let cable = composites::connect(&mut v.editor, &closed, jack(lfo, "m1"), jack(OSC, "pitch"));
    let fed = |p: &PatchState| -> Vec<PortRef> {
        let mut t: Vec<PortRef> = p
            .cables
            .values()
            .filter(|c| c.from == jack(lfo, "m1"))
            .map(|c| c.to.clone())
            .collect();
        t.sort_by_key(|t| format!("{t:?}"));
        t
    };
    let mut want = v.pitch();
    want.sort_by_key(|t| format!("{t:?}"));
    assert_eq!(fed(v.editor.state()), want);
    assert!(
        !v.editor
            .state()
            .cables
            .values()
            .any(|c| c.from == jack(MIDI, "pitch")),
        "the jacks held one cable each; the new one replaces it"
    );
    assert_eq!(v.editor.state().composites[&id], before.composites[&id]);

    composites::disconnect(&mut v.editor, &closed, cable);
    assert!(fed(v.editor.state()).is_empty());
    assert!(v.editor.undo() && v.editor.undo());
    assert_eq!(v.editor.state(), &before);

    // Opened, the composite shows its leaves and a cable patches one jack only.
    let open = BTreeSet::from([id]);
    composites::connect(&mut v.editor, &open, jack(lfo, "m1"), jack(OSC, "pitch"));
    assert_eq!(fed(v.editor.state()), vec![jack(OSC, "pitch")]);
}

#[test]
fn a_cable_dragged_onto_a_closed_face_port_wires_every_target() {
    let mut v = voice();
    v.wrap();
    let lfo = v.editor.add_module("macro", Vec2 { x: 24.0, y: 600.0 });
    let ctx = egui::Context::default();
    let mut view = kabl_ui::UiState::default();
    let size = egui::vec2(1440.0, 900.0);
    let mut time = 0.0;
    let mut frame = |e: &mut PatchEditor, view: &mut kabl_ui::UiState, events| {
        ctx.begin_pass(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            time: Some(time),
            events,
            ..Default::default()
        });
        time += 1.0 / 60.0;
        let mut root = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        );
        kabl_ui::show(e, view, &mut root);
        drop(ctx.end_pass());
    };
    for _ in 0..3 {
        frame(&mut v.editor, &mut view, vec![]);
    }
    let from = view.hits[&format!("out:{lfo}.m1")].center();
    let to = view.hits[&format!("in:{OSC}.pitch")].center();
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(
        &mut v.editor,
        &mut view,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    for i in 1..=6 {
        let p = from + (to - from) * (i as f32 / 6.0);
        frame(&mut v.editor, &mut view, vec![egui::Event::PointerMoved(p)]);
    }
    frame(&mut v.editor, &mut view, vec![button(to, false)]);
    frame(&mut v.editor, &mut view, vec![]);
    let mut fed: Vec<PortRef> = v
        .editor
        .state()
        .cables
        .values()
        .filter(|c| c.from == jack(lfo, "m1"))
        .map(|c| c.to.clone())
        .collect();
    fed.sort_by_key(|t| format!("{t:?}"));
    let mut want = v.pitch();
    want.sort_by_key(|t| format!("{t:?}"));
    assert_eq!(fed, want);
}
