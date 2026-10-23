//! A route into another route's amount (`PortRef::CableParam` named `"amount"`): a macro, LFO or
//! envelope moves the depth of a routing. Probe: a square `lfo` (a constant +1) feeds an
//! `attenuverter` input and, through route `ROUTE`, its `amount` knob; the attenuverter goes to
//! `out.left`. A depth source adds `source * depth` to the route's amount, so the output with the
//! macro at 1 must equal a plain route whose amount is `base + depth`.

use std::collections::BTreeMap;

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::compile::{compile, CompileError};
use kabl_engine::runtime::RuntimeTarget;

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;
const LFO: u64 = 1;
const ATT: u64 = 2;
const OUT: u64 = 3;
const MACRO: u64 = 4;
const ROUTE: u64 = 10;
const DEPTH: u64 = 11;

fn module(kind: &str, kv: &[(&str, f32)]) -> ModuleState {
    ModuleState {
        kind: kind.into(),
        pos: Vec2 { x: 0.0, y: 0.0 },
        params: kv.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
    }
}

fn cable(from: PortRef, to: PortRef, amount: Option<f32>) -> CableState {
    CableState {
        from,
        to,
        params: amount
            .map(|a| ("amount".to_string(), a))
            .into_iter()
            .collect::<BTreeMap<_, _>>(),
        steps: Vec::new(),
    }
}

fn port(id: u64, p: &str) -> PortRef {
    PortRef::Module {
        id,
        port: p.into(),
    }
}

/// The probe with route `ROUTE` at `base`; the macro and its depth route are added by callers.
fn probe(base: f32) -> PatchState {
    let mut p = PatchState::new();
    p.modules
        .insert(LFO, module("lfo", &[("rate_hz", 0.01), ("waveform", 3.0)]));
    p.modules.insert(ATT, module("attenuverter", &[]));
    p.modules.insert(OUT, module("out", &[]));
    p.cables
        .insert(1, cable(port(LFO, "out"), port(ATT, "in"), None));
    p.cables
        .insert(2, cable(port(ATT, "out"), port(OUT, "left"), None));
    p.cables.insert(
        ROUTE,
        cable(
            port(LFO, "out"),
            PortRef::Param {
                id: ATT,
                param: "amount".into(),
            },
            Some(base),
        ),
    );
    p
}

fn with_depth(base: f32, macro_value: f32, depth: f32) -> PatchState {
    let mut p = probe(base);
    p.modules
        .insert(MACRO, module("macro", &[("m1", macro_value)]));
    p.cables.insert(
        DEPTH,
        cable(
            port(MACRO, "m1"),
            PortRef::CableParam {
                cable: ROUTE,
                param: "amount".into(),
            },
            Some(depth),
        ),
    );
    p
}

fn render(p: &PatchState) -> Vec<f32> {
    let mut c = compile(p, SR, 1).unwrap();
    let mut out = Vec::new();
    for _ in 0..200 {
        c.process_block();
        out.extend_from_slice(c.left());
    }
    out
}

fn tail(v: &[f32]) -> f32 {
    *v.last().unwrap()
}

#[test]
fn a_macro_adds_its_depth_to_the_route_amount() {
    let at_zero = render(&with_depth(0.1, 0.0, 0.5));
    let at_one = render(&with_depth(0.1, 1.0, 0.5));
    assert_eq!(tail(&at_zero), tail(&render(&probe(0.1))));
    assert_eq!(tail(&at_one), tail(&render(&probe(0.6))));
    assert_ne!(tail(&at_zero), tail(&at_one));
}

#[test]
fn a_negative_depth_pulls_the_amount_down() {
    assert_eq!(
        tail(&render(&with_depth(0.6, 1.0, -0.5))),
        tail(&render(&probe(0.1)))
    );
}

#[test]
fn the_depth_amount_is_a_runtime_value() {
    let mut c = compile(&with_depth(0.1, 1.0, 0.0), SR, 1).unwrap();
    for _ in 0..50 {
        c.process_block();
    }
    let before = c.left()[0];
    assert!(c.set_runtime(RuntimeTarget::Route { cable: DEPTH }, 0.5, false));
    for _ in 0..50 {
        c.process_block();
    }
    assert_ne!(c.left()[0], before);
    assert_eq!(c.left()[0], tail(&render(&probe(0.6))));
    assert_eq!(c.route_amount(DEPTH), Some(0.5));
}

#[test]
fn a_bypassed_depth_route_does_nothing() {
    let mut p = with_depth(0.1, 1.0, 0.5);
    p.cables
        .get_mut(&DEPTH)
        .unwrap()
        .params
        .insert("bypass".into(), 1.0);
    assert_eq!(tail(&render(&p)), tail(&render(&probe(0.1))));
}

#[test]
fn only_a_route_into_a_knob_has_an_amount_to_move() {
    let mut p = with_depth(0.1, 1.0, 0.5);
    p.cables.get_mut(&DEPTH).unwrap().to = PortRef::CableParam {
        cable: 1,
        param: "amount".into(),
    };
    let Err(CompileError::CableRoute { cable, reason }) = compile(&p, SR, 1) else {
        panic!("a route into a plain cable's amount must not compile");
    };
    assert_eq!(cable, DEPTH);
    assert!(reason.contains("no amount to move"), "{reason}");
}

#[test]
fn a_route_cannot_move_its_own_amount() {
    let mut p = with_depth(0.1, 1.0, 0.5);
    p.cables.get_mut(&DEPTH).unwrap().to = PortRef::CableParam {
        cable: DEPTH,
        param: "amount".into(),
    };
    assert!(matches!(
        compile(&p, SR, 1),
        Err(CompileError::CableRoute { .. })
    ));
}

#[test]
fn the_audio_path_does_not_allocate_with_a_depth_route_edited() {
    let mut c = compile(&with_depth(0.1, 0.5, 0.5), SR, 1).unwrap();
    for b in 0..300 {
        assert_no_alloc(|| {
            if b % 3 == 0 {
                c.set_runtime(
                    RuntimeTarget::Route { cable: DEPTH },
                    (b % 7) as f32 / 7.0 - 0.3,
                    b % 2 == 0,
                );
            }
            c.process_block();
        });
    }
    assert!(c.left().iter().all(|x| x.is_finite()));
}
