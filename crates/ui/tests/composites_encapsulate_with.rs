//! `composites::encapsulate_with`: the curated encapsulation patch generators use. Wrapping
//! appends one `SetComposite` and changes nothing a sound, a route, a cue, a CC map or host
//! automation reads.
use kabl_core::{Definition, Exposure, ModuleId, Op, PatchLog, PatchState, PortRef, Vec2};
use kabl_engine::runtime::RuntimeTarget;
use kabl_ui::{composites, cues, perform, PatchEditor};
use std::collections::BTreeSet;

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

struct Fixture {
    log: PatchLog,
    filter: ModuleId,
    vca: ModuleId,
    delay: ModuleId,
    macros: ModuleId,
    cues: ModuleId,
}

/// The default voice (midi → osc → filter → VCA → out) with a delay before the output, a macro
/// on the filter cutoff, a cue that moves the macro and CC 20 mapped to the cutoff.
fn fixture() -> Fixture {
    let mut e = PatchEditor::seed_from(&kabl_standalone::default_patch());
    let id = |e: &PatchEditor, kind: &str| {
        *e.state()
            .modules
            .iter()
            .find(|(_, m)| m.kind == kind)
            .unwrap()
            .0
    };
    let (filter, vca, out) = (id(&e, "filter.svf"), id(&e, "vca"), id(&e, "out"));
    let delay = e.add_module("delay", Vec2 { x: 740.0, y: 200.0 });
    let to_out = *e
        .state()
        .cables
        .iter()
        .find(|(_, c)| c.to.module_id() == Some(out))
        .unwrap()
        .0;
    e.disconnect(to_out);
    e.connect(jack(vca, "out"), jack(delay, "in"));
    e.connect(jack(delay, "out"), jack(out, "in"));
    let macros = e.add_module("macro", Vec2 { x: 24.0, y: 400.0 });
    e.connect_route(jack(macros, "m1"), filter, "cutoff_hz");
    let cue_module = e.add_module("cues", Vec2 { x: 24.0, y: 700.0 });
    let n = cues::add(&mut e, cue_module, &|_| 0).unwrap();
    cues::set_macro(&mut e, cue_module, n, macros, 0, Some(0.9));
    e.set_presentation(&[(filter, "cc.cutoff_hz".into(), Some(20.0))]);
    Fixture {
        log: e.log().clone(),
        filter,
        vca,
        delay,
        macros,
        cues: cue_module,
    }
}

fn definition() -> Definition {
    Definition {
        id: "kabl.test-tone".into(),
        version: 1,
    }
}

fn exposures(f: &Fixture) -> Vec<Exposure> {
    vec![
        Exposure {
            label: "Input".into(),
            target: jack(f.filter, "in"),
        },
        Exposure {
            label: "Cutoff".into(),
            target: param(f.filter, "cutoff_hz"),
        },
    ]
}

fn wrap(log: &mut PatchLog, f: &Fixture, exposures: &[Exposure]) -> Result<u64, String> {
    composites::encapsulate_with(
        log,
        "Tone",
        definition(),
        BTreeSet::from([f.filter, f.vca, f.delay]),
        exposures,
        None,
    )
}

/// Bit-exact render (FNV over the output's bit patterns), with `before_each` run every block.
fn render(
    p: &PatchState,
    before_each: impl Fn(&mut kabl_engine::compile::CompiledPatch),
) -> String {
    let _ftz = kabl_engine::ftz::FtzGuard::new();
    let mut c = kabl_engine::compile::compile(p, 48000.0, 8).unwrap();
    kabl_engine::golden::chord_hash(&mut c, 400, 200, |c, _| before_each(c))
}

fn serialized(log: &PatchLog) -> Vec<String> {
    log.entries()
        .iter()
        .map(|e| serde_json::to_string(e).unwrap())
        .chain([serde_json::to_string(log.state()).unwrap()])
        .collect()
}

#[test]
fn wrapping_appends_one_set_composite_and_keeps_ids_sound_and_references() {
    let f = fixture();
    let flat = f.log.clone();
    let mut log = f.log.clone();
    let id = wrap(&mut log, &f, &exposures(&f)).unwrap();

    // Earlier entries untouched; one SetComposite appended after them.
    assert_eq!(log.entries().len(), flat.entries().len() + 1);
    assert_eq!(
        serialized(&log)[..flat.entries().len()],
        serialized(&flat)[..flat.entries().len()]
    );
    let Op::SetComposite { id: set, value } = &log.entries().last().unwrap().op else {
        panic!("last op is not SetComposite");
    };
    assert_eq!(*set, id);
    let c = value.as_ref().unwrap();
    assert_eq!(c.definition, definition());
    assert_eq!(c.members, BTreeSet::from([f.filter, f.vca, f.delay]));

    // Leaf modules, cables and their order unchanged.
    let (a, b) = (flat.state(), log.state());
    assert_eq!(a.modules, b.modules);
    assert_eq!(a.cables, b.cables);
    assert!(a.modules.keys().eq(b.modules.keys()));
    assert!(a.cables.keys().eq(b.cables.keys()));

    // Curated exposures get keys 1, 2 in order; the uncovered crossings follow as ports.
    assert_eq!(c.ports[&1].label, "Input");
    assert_eq!(c.ports[&1].target, jack(f.filter, "in"));
    assert_eq!(c.controls[&2].label, "Cutoff");
    assert_eq!(c.controls[&2].target, param(f.filter, "cutoff_hz"));
    let crossing: Vec<_> = c.ports.values().map(|e| e.target.clone()).collect();
    assert!(crossing.contains(&param(f.filter, "cutoff_hz")));
    assert!(crossing.contains(&jack(f.vca, "cv")));
    assert!(crossing.contains(&jack(f.delay, "out")));

    // Sound: bit-identical, idle and with the macro moved to the cue's value.
    assert_eq!(render(a, |_| {}), render(b, |_| {}));
    let moved = |p: &PatchState| {
        let mut p = p.clone();
        p.modules
            .get_mut(&f.macros)
            .unwrap()
            .params
            .insert("m1".into(), 0.9);
        render(&p, |_| {})
    };
    assert_eq!(moved(a), moved(b));
    assert_ne!(
        moved(b),
        render(b, |_| {}),
        "the macro must audibly move the cutoff"
    );

    // The macro route still lands on the same leaf param.
    let route = |p: &PatchState| {
        p.cables
            .iter()
            .find(|(_, c)| c.from == jack(f.macros, "m1"))
            .map(|(&id, c)| (id, c.to.clone()))
            .unwrap()
    };
    assert_eq!(route(a), route(b));
    assert_eq!(route(b).1, param(f.filter, "cutoff_hz"));

    // The cue still moves the same macro.
    assert_eq!(cues::cues(b, f.cues)[0].macros, vec![(f.macros, 0, 0.9)]);
    assert_eq!(
        cues::cues(a, f.cues)[0].macros,
        cues::cues(b, f.cues)[0].macros
    );

    // CC 20 still maps to the filter cutoff.
    assert_eq!(perform::mapping(b, f.filter, "cutoff_hz"), Some((0, 20)));

    // Host automation addresses (module id, param): the same runtime write resolves and sounds
    // the same.
    let index = kabl_modules::registry::info_for("filter.svf")
        .unwrap()
        .params
        .iter()
        .position(|p| p.name == "cutoff_hz")
        .unwrap() as u16;
    let automate = |c: &mut kabl_engine::compile::CompiledPatch| {
        let target = RuntimeTarget::Param {
            id: f.filter,
            kind: "filter.svf",
            index,
        };
        assert!(c.set_runtime(target, 400.0, false));
    };
    assert_eq!(render(a, automate), render(b, automate));
    assert_ne!(render(b, automate), render(b, |_| {}));
}

#[test]
fn the_same_call_twice_gives_byte_identical_logs() {
    let f = fixture();
    let mut one = f.log.clone();
    let mut two = f.log.clone();
    wrap(&mut one, &f, &exposures(&f)).unwrap();
    wrap(&mut two, &f, &exposures(&f)).unwrap();
    assert_eq!(serialized(&one), serialized(&two));
}

#[test]
fn edges_fail_with_a_named_error() {
    let f = fixture();
    let refused = |log: &mut PatchLog, exposures: &[Exposure]| {
        let before = serialized(log);
        let err = wrap(log, &f, exposures).unwrap_err();
        assert_eq!(serialized(log), before, "a refusal appends nothing");
        err
    };

    // Target outside the wrapped set.
    let mut log = f.log.clone();
    let mut outside = exposures(&f);
    outside.push(Exposure {
        label: "Macro".into(),
        target: jack(f.macros, "m1"),
    });
    let err = refused(&mut log, &outside);
    assert!(
        err.contains("\"Macro\"") && err.contains("not inside"),
        "{err}"
    );

    // Two exposures with the same label.
    let mut twice = exposures(&f);
    twice.push(Exposure {
        label: "Cutoff".into(),
        target: param(f.filter, "resonance"),
    });
    let err = refused(&mut log, &twice);
    assert!(
        err.contains("Two exposures are labelled \"Cutoff\""),
        "{err}"
    );

    // G1: a route moving a cable parameter across the boundary.
    let mut e = PatchEditor::from_log(f.log.clone());
    let route = *e
        .state()
        .cables
        .iter()
        .find(|(_, c)| c.from == jack(f.macros, "m1"))
        .unwrap()
        .0;
    let g1 = e
        .connect_cable_route(jack(f.macros, "m2"), route, kabl_engine::compile::DEPTH)
        .unwrap();
    let mut log = e.log().clone();
    let err = refused(&mut log, &exposures(&f));
    assert!(
        err.contains(&format!("Route {g1} moves a cable parameter")),
        "{err}"
    );

    // G3: one outside source feeding two inside targets.
    let mut e = PatchEditor::from_log(f.log.clone());
    let fan = e.connect_route(jack(f.macros, "m1"), f.delay, "mix");
    let mut log = e.log().clone();
    let err = refused(&mut log, &exposures(&f));
    assert!(
        err.contains(&format!("{fan} carry m1 · #{}", f.macros)) && err.contains("several targets"),
        "{err}"
    );
}
