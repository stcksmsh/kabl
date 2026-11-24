//! Cable parameters on a composite boundary (gap G1): a route from outside into the amount of
//! an internal route becomes a port bound to that cable parameter, by interactive and by
//! curated encapsulation. Wrapping changes nothing the engine, a cue, a CC map or a runtime
//! write reads.
use kabl_core::{CableId, Definition, Exposure, ModuleId, PatchLog, PatchState, PortRef, Vec2};
use kabl_engine::compile::DEPTH;
use kabl_engine::runtime::RuntimeTarget;
use kabl_ui::{composites, cues, perform, PatchEditor};
use std::collections::BTreeSet;

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
fn amount(cable: CableId) -> PortRef {
    PortRef::CableParam {
        cable,
        param: DEPTH.into(),
    }
}
fn exposure(label: &str, target: PortRef) -> Exposure {
    Exposure {
        label: label.into(),
        also: vec![],
        target,
    }
}

struct Fixture {
    log: PatchLog,
    macros: ModuleId,
    cues: ModuleId,
    /// Envelope → filter cutoff.
    route: CableId,
    /// Macro m1 → the amount of `route`.
    depth: CableId,
}

/// The default voice with the corpus pattern: the envelope moves the filter cutoff through a
/// route whose amount a macro drives. A cue moves the macro and CC 20 is mapped onto it.
fn fixture() -> Fixture {
    let mut e = PatchEditor::seed_from(&kabl_standalone::default_patch());
    let route = e.connect_route(jack(ENV, "out"), FILTER, "cutoff_hz");
    e.set_route_amount(route, 0.4, true);
    let macros = e.add_module("macro", Vec2 { x: 24.0, y: 400.0 });
    let depth = e
        .connect_cable_route(jack(macros, "m1"), route, DEPTH)
        .unwrap();
    let cue_module = e.add_module("cues", Vec2 { x: 24.0, y: 700.0 });
    let n = cues::add(&mut e, cue_module, &|_| 0).unwrap();
    cues::set_macro(&mut e, cue_module, n, macros, 0, Some(0.9));
    e.set_presentation(&[(macros, "cc.m1".into(), Some(20.0))]);
    Fixture {
        log: e.log().clone(),
        macros,
        cues: cue_module,
        route,
        depth,
    }
}

fn voice() -> BTreeSet<ModuleId> {
    BTreeSet::from([OSC, FILTER, ENV, VCA])
}

fn render(
    p: &PatchState,
    before_each: impl Fn(&mut kabl_engine::compile::CompiledPatch),
) -> String {
    let _ftz = kabl_engine::ftz::FtzGuard::new();
    let mut c = kabl_engine::compile::compile(p, 48000.0, 8).unwrap();
    kabl_engine::golden::chord_hash(&mut c, 400, 200, |c, _| before_each(c))
}

fn with_macro(p: &PatchState, macros: ModuleId, value: f32) -> PatchState {
    let mut p = p.clone();
    p.modules
        .get_mut(&macros)
        .unwrap()
        .params
        .insert("m1".into(), value);
    p
}

/// Flat and wrapped render bit-identically, idle and with the macro moved, and the macro is
/// audible through the cable amount.
fn assert_same_sound(f: &Fixture, flat: &PatchState, wrapped: &PatchState) {
    assert_eq!(
        flat.cables, wrapped.cables,
        "wrapping leaves every cable alone"
    );
    assert_eq!(render(flat, |_| {}), render(wrapped, |_| {}));
    let moved = |p: &PatchState| render(&with_macro(p, f.macros, 0.9), |_| {});
    assert_eq!(moved(flat), moved(wrapped));
    assert_ne!(
        moved(wrapped),
        render(&with_macro(wrapped, f.macros, 0.0), |_| {}),
        "the macro must audibly move the envelope amount"
    );
}

fn ports_of(p: &PatchState, id: u64) -> Vec<PortRef> {
    p.composites[&id]
        .ports
        .values()
        .map(|e| e.target.clone())
        .collect()
}

#[test]
fn interactive_encapsulate_exposes_the_cable_amount_as_a_port() {
    let f = fixture();
    let mut e = PatchEditor::from_log(f.log.clone());
    let id = composites::encapsulate(&mut e, voice(), BTreeSet::new(), "Voice").unwrap();
    let p = e.state();
    let ports = ports_of(p, id);
    assert!(ports.contains(&amount(f.route)), "{ports:?}");
    assert_eq!(
        composites::direction(p, &amount(f.route)),
        Some(kabl_modules::PortDirection::Input)
    );
    // The macro route still drives the same cable's amount.
    assert_eq!(p.cables[&f.depth].from, jack(f.macros, "m1"));
    assert_eq!(p.cables[&f.depth].to, amount(f.route));
    assert_same_sound(&f, f.log.state(), p);
}

fn curated_ports(f: &Fixture) -> Vec<Exposure> {
    vec![
        exposure("Pitch", jack(OSC, "pitch")),
        exposure("Gate", jack(ENV, "gate")),
        exposure("Out", jack(VCA, "out")),
        exposure("Env Amount", amount(f.route)),
    ]
}

fn curated(log: &mut PatchLog, f: &Fixture, controls: &[Exposure]) -> Result<u64, String> {
    composites::encapsulate_with(
        log,
        "Voice",
        Definition {
            id: "kabl.test-voice".into(),
            version: 1,
        },
        voice(),
        &curated_ports(f),
        controls,
        None,
    )
}

#[test]
fn encapsulate_with_takes_the_cable_amount_as_a_port() {
    let f = fixture();
    let mut log = f.log.clone();
    let id = curated(&mut log, &f, &[]).unwrap();
    let c = &log.state().composites[&id];
    assert_eq!(c.ports[&4], exposure("Env Amount", amount(f.route)));
    assert_same_sound(&f, f.log.state(), log.state());
    kabl_engine::compile::validate_composites(log.state()).unwrap();

    // The crossing must still be covered.
    let mut log = f.log.clone();
    let err = composites::encapsulate_with(
        &mut log,
        "Voice",
        Definition {
            id: "kabl.test-voice".into(),
            version: 1,
        },
        voice(),
        &curated_ports(&f)[..3],
        &[],
        None,
    )
    .unwrap_err();
    assert!(
        err.contains(&format!(
            "cable {} (cable #{}.amount inside, #{}.m1 outside)",
            f.depth, f.route, f.macros
        )),
        "{err}"
    );

    // Composite faces only draw knobs for module parameters, so it is not a control yet.
    let mut log = f.log.clone();
    let err = curated(&mut log, &f, &[exposure("Env Knob", amount(f.route))]).unwrap_err();
    assert!(err.contains("is not a parameter"), "{err}");
}

#[test]
fn the_amount_of_a_cable_that_leaves_the_group_does_not_cross() {
    // The route itself crosses outward (envelope inside, filter outside), so its amount belongs
    // to the filter side and the macro route stays outside the boundary.
    let f = fixture();
    let mut e = PatchEditor::from_log(f.log.clone());
    let id =
        composites::encapsulate(&mut e, BTreeSet::from([ENV]), BTreeSet::new(), "Env").unwrap();
    let ports = ports_of(e.state(), id);
    assert!(ports.contains(&jack(ENV, "out")), "{ports:?}");
    assert!(
        !ports
            .iter()
            .any(|t| matches!(t, PortRef::CableParam { .. })),
        "{ports:?}"
    );
    assert_same_sound(&f, f.log.state(), e.state());

    // A curated port on that amount is outside the wrapped module.
    let mut log = f.log.clone();
    let err = composites::encapsulate_with(
        &mut log,
        "Env",
        Definition {
            id: "kabl.test-env".into(),
            version: 1,
        },
        BTreeSet::from([ENV]),
        &[
            exposure("Gate", jack(ENV, "gate")),
            exposure("Out", jack(ENV, "out")),
            exposure("Env Amount", amount(f.route)),
        ],
        &[],
        None,
    )
    .unwrap_err();
    assert!(
        err.contains("\"Env Amount\"") && err.contains("not inside"),
        "{err}"
    );
}

#[test]
fn the_amount_of_a_cable_that_enters_the_group_crosses_with_it() {
    // The route crosses inward (envelope outside, filter inside): its amount belongs to the
    // filter, so the outside macro reaches it through a port.
    let f = fixture();
    let mut e = PatchEditor::from_log(f.log.clone());
    let id = composites::encapsulate(&mut e, BTreeSet::from([FILTER]), BTreeSet::new(), "Filter")
        .unwrap();
    let ports = ports_of(e.state(), id);
    assert!(ports.contains(&param(FILTER, "cutoff_hz")), "{ports:?}");
    assert!(ports.contains(&amount(f.route)), "{ports:?}");
    assert_same_sound(&f, f.log.state(), e.state());

    // Macro inside, owner outside: the route leaves through the macro's output.
    let mut e = PatchEditor::from_log(f.log.clone());
    let id = composites::encapsulate(&mut e, BTreeSet::from([f.macros]), BTreeSet::new(), "Macro")
        .unwrap();
    assert_eq!(ports_of(e.state(), id), vec![jack(f.macros, "m1")]);
    assert_same_sound(&f, f.log.state(), e.state());
}

#[test]
fn cues_cc_and_runtime_writes_still_reach_the_internal_amount() {
    let f = fixture();
    let flat = f.log.state();
    let mut log = f.log.clone();
    curated(&mut log, &f, &[]).unwrap();
    let wrapped = log.state();

    assert_eq!(
        cues::cues(wrapped, f.cues)[0].macros,
        vec![(f.macros, 0, 0.9)]
    );
    assert_eq!(perform::mapping(wrapped, f.macros, "m1"), Some((0, 20)));
    assert_eq!(perform::mapping(flat, f.macros, "m1"), Some((0, 20)));

    // A runtime write (CC, automation) addresses the route by its cable id.
    let write = |c: &mut kabl_engine::compile::CompiledPatch| {
        assert!(c.set_runtime(RuntimeTarget::Route { cable: f.route }, 0.9, false));
    };
    assert_eq!(render(flat, write), render(wrapped, write));
    assert_ne!(render(wrapped, write), render(wrapped, |_| {}));
}

#[test]
fn opening_the_composite_shows_and_edits_the_cable_amount() {
    let f = fixture();
    let mut e = PatchEditor::from_log(f.log.clone());
    let id = composites::encapsulate(&mut e, voice(), BTreeSet::new(), "Voice").unwrap();
    assert!(composites::hidden(e.state(), &BTreeSet::new(), ENV));
    let open = BTreeSet::from([id]);
    assert!(!composites::hidden(e.state(), &open, ENV));
    assert!(!composites::hidden(e.state(), &open, FILTER));

    let before = render(e.state(), |_| {});
    e.set_route_amount(f.route, -0.5, true);
    assert_eq!(e.state().cables[&f.route].params["amount"], -0.5);
    assert!(e.state().composites.contains_key(&id));
    kabl_engine::compile::validate_composites(e.state()).unwrap();
    assert_ne!(render(e.state(), |_| {}), before);
}

#[test]
fn duplicating_keeps_the_port_on_the_copied_cable() {
    let f = fixture();
    let mut e = PatchEditor::from_log(f.log.clone());
    let id = composites::encapsulate(&mut e, voice(), BTreeSet::new(), "Voice").unwrap();
    let copy = composites::duplicate(&mut e, id).unwrap();
    let p = e.state();
    let bound: Vec<CableId> = p.composites[&copy]
        .ports
        .values()
        .filter_map(|e| match &e.target {
            PortRef::CableParam { cable, .. } => Some(*cable),
            _ => None,
        })
        .collect();
    assert_eq!(bound.len(), 1);
    let cable = &p.cables[&bound[0]];
    assert_ne!(bound[0], f.route, "the copy binds its own cable");
    let leaves = kabl_core::composite::leaves(p, copy);
    assert!(cable.from.module_id().is_some_and(|m| leaves.contains(&m)));
    assert!(p.end_module(bound[0]).is_some_and(|m| leaves.contains(&m)));
}

#[test]
fn duplicating_drops_the_port_on_a_cable_that_stays_outside() {
    // The route enters the filter from the envelope outside, so its amount is a port of the
    // filter group, but the route itself is not copied: the copy has no such port.
    let f = fixture();
    let mut e = PatchEditor::from_log(f.log.clone());
    let id = composites::encapsulate(&mut e, BTreeSet::from([FILTER]), BTreeSet::new(), "Filter")
        .unwrap();
    assert!(ports_of(e.state(), id).contains(&amount(f.route)));

    let pkg = composites::package(e.state(), id).unwrap();
    let root = &pkg.patch.composites[&id];
    assert!(root.ports.values().all(|x| x.target != amount(f.route)));
    assert!(root
        .ports
        .values()
        .any(|x| x.target == param(FILTER, "cutoff_hz")));
    assert_eq!(pkg.schema, 1, "no cable parameter left to bind");

    let copy = composites::duplicate(&mut e, id).unwrap();
    let ports = ports_of(e.state(), copy);
    assert!(
        !ports
            .iter()
            .any(|t| matches!(t, PortRef::CableParam { .. })),
        "{ports:?}"
    );
    assert_eq!(ports.len(), ports_of(e.state(), id).len() - 1);
}

#[test]
fn a_package_binding_a_cable_param_is_schema_2() {
    let f = fixture();
    let mut e = PatchEditor::from_log(f.log.clone());
    let id = composites::encapsulate(&mut e, voice(), BTreeSet::new(), "Voice").unwrap();
    let pkg = composites::package(e.state(), id).unwrap();
    assert_eq!(pkg.schema, 2);
    let bytes = serde_json::to_vec(&pkg).unwrap();
    assert_eq!(composites::decode(&bytes).unwrap().schema, 2);

    // A package from a newer build is refused as unsupported, not opened.
    let mut newer = pkg.clone();
    newer.schema = 3;
    let err = composites::decode(&serde_json::to_vec(&newer).unwrap()).unwrap_err();
    assert!(err.contains("Unsupported"), "{err}");
}
