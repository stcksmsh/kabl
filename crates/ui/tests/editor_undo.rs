//! `PatchEditor` through the real op log: which edits rebuild audio, and that every user action
//! undoes/redoes as one coherent step back to the exact previous state (including absent
//! params, removed modules with their cables, and route settings), across save/load.

use kabl_core::{ParamTarget, PatchState, PortRef, Vec2};
use kabl_engine::compile::compile;
use kabl_ui::PatchEditor;

fn out(id: u64, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

/// midi.in -> vca -> out, plus an LFO and an ADSR for routes.
fn editor() -> (PatchEditor, [u64; 5]) {
    let mut e = PatchEditor::new();
    let at = Vec2 { x: 0.0, y: 0.0 };
    let midi = e.add_module("midi.in", at);
    let vca = e.add_module("vca", at);
    let o = e.add_module("out", at);
    let lfo = e.add_module("lfo", at);
    let env = e.add_module("env.adsr", at);
    e.connect(out(midi, "gate"), out(vca, "in"));
    e.connect(out(vca, "out"), out(o, "left"));
    e.connect(out(midi, "gate"), out(env, "gate"));
    e.take_dirty();
    (e, [midi, vca, o, lfo, env])
}

#[test]
fn layout_edits_do_not_rebuild_audio() {
    let (mut e, [_, vca, ..]) = editor();
    e.move_module(vca, Vec2 { x: 100.0, y: 20.0 });
    assert!(!e.take_dirty(), "move is layout only");
    e.undo();
    assert!(!e.take_dirty(), "undo of a move is layout only");
    e.redo();
    assert!(!e.take_dirty(), "redo of a move is layout only");
    e.set_param(vca, "gain", 0.5);
    assert!(e.take_dirty(), "a param edit rebuilds");
    e.undo();
    assert!(e.take_dirty(), "undoing it rebuilds");
}

#[test]
fn undo_of_a_first_param_edit_restores_the_default_not_zero() {
    let (mut e, [_, vca, ..]) = editor();
    e.set_param(vca, "gain", 0.3);
    e.undo();
    assert!(!e.state().modules[&vca].params.contains_key("gain"));
}

#[test]
fn deleting_a_module_takes_its_cables_and_undo_restores_everything() {
    let (mut e, [_, vca, _, lfo, env]) = editor();
    e.set_param(env, "attack_ms", 40.0);
    let route = e.connect_route(out(lfo, "out"), env, "attack_ms");
    e.set_route_amount(route, -0.4, true);
    e.set_route_bypass(route, true);
    let before: PatchState = e.state().clone();

    e.remove_module(env);
    let after = e.state().clone();
    assert!(
        after
            .cables
            .values()
            .all(|c| c.from.module_id() != env && c.to.module_id() != env),
        "no dangling cables"
    );
    assert!(after.cables.values().any(|c| c.to == out(vca, "in")));
    compile(&after, 48000.0, 2).expect("patch compiles after deletion");

    assert!(e.undo());
    assert_eq!(
        e.state(),
        &before,
        "one undo restores module, params, cables, route"
    );
    assert_eq!(e.state().cables[&route].params["amount"], -0.4);
    assert!(e.redo());
    assert_eq!(e.state(), &after);
}

#[test]
fn repatching_an_occupied_jack_replaces_and_undoes_as_one_step() {
    let (mut e, [midi, vca, _, lfo, _]) = editor();
    let before = e.state().clone();
    e.connect(out(lfo, "out"), out(vca, "in"));
    let into_vca: Vec<_> = e
        .state()
        .cables
        .values()
        .filter(|c| c.to == out(vca, "in"))
        .collect();
    assert_eq!(into_vca.len(), 1, "jack holds one cable");
    assert_eq!(into_vca[0].from, out(lfo, "out"));
    e.undo();
    assert_eq!(e.state(), &before);
    assert!(e
        .state()
        .cables
        .values()
        .any(|c| c.from == out(midi, "gate") && c.to == out(vca, "in")));
}

#[test]
fn knobs_accept_several_routes() {
    let (mut e, [midi, _, _, lfo, env]) = editor();
    let a = e.connect_route(out(lfo, "out"), env, "attack_ms");
    let b = e.connect_route(out(midi, "velocity"), env, "attack_ms");
    assert_ne!(a, b);
    assert!(e.state().cables.contains_key(&a) && e.state().cables.contains_key(&b));
    compile(e.state(), 48000.0, 2).expect("two routes compile");
}

#[test]
fn a_slow_drag_is_one_undo_step() {
    let (mut e, [_, vca, ..]) = editor();
    let t = ParamTarget::Module {
        id: vca,
        param: "gain".into(),
    };
    e.set_param_gesture(t.clone(), 0.9, true);
    e.set_param_gesture(t.clone(), 0.8, false);
    // Longer than the 250 ms coalescing window: still the same gesture.
    std::thread::sleep(std::time::Duration::from_millis(300));
    e.set_param_gesture(t.clone(), 0.7, false);
    assert_eq!(e.state().modules[&vca].params["gain"], 0.7);
    e.undo();
    assert!(!e.state().modules[&vca].params.contains_key("gain"));

    // A second gesture right after the first is its own step.
    e.redo();
    e.set_param_gesture(t.clone(), 0.2, true);
    e.undo();
    assert_eq!(e.state().modules[&vca].params["gain"], 0.7);
}

#[test]
fn route_amount_drag_edits_only_that_route() {
    let (mut e, [midi, _, _, lfo, env]) = editor();
    let a = e.connect_route(out(lfo, "out"), env, "attack_ms");
    let b = e.connect_route(out(midi, "velocity"), env, "attack_ms");
    e.set_route_amount(b, 0.6, true);
    e.set_route_amount(b, 0.7, false);
    assert!(!e.state().cables[&a].params.contains_key("amount"));
    assert_eq!(e.state().cables[&b].params["amount"], 0.7);
    e.undo();
    assert!(!e.state().cables[&b].params.contains_key("amount"));
}

#[test]
fn save_load_preserves_routes_timing_and_undo() {
    let (mut e, [midi, _, _, lfo, env]) = editor();
    let a = e.connect_route(out(lfo, "out"), env, "attack_ms");
    let b = e.connect_route(out(midi, "velocity"), env, "attack_ms");
    e.set_route_amount(a, -0.3, true);
    e.set_route_bypass(b, true);
    e.set_param(env, "timing", 1.0);
    let dir = tempfile::tempdir().unwrap();
    kabl_core::save(dir.path(), e.log()).unwrap();

    let mut loaded = PatchEditor::from_log(kabl_core::load(dir.path()).unwrap());
    assert_eq!(
        loaded.state(),
        e.state(),
        "routes, ids, settings, timing mode"
    );
    assert_eq!(
        loaded.state().cables[&a].to,
        PortRef::Param {
            id: env,
            param: "attack_ms".into()
        }
    );
    // Undo history survives: timing, then bypass, then amount.
    loaded.undo();
    assert!(!loaded.state().modules[&env].params.contains_key("timing"));
    loaded.undo();
    assert!(!loaded.state().cables[&b].params.contains_key("bypass"));
    loaded.undo();
    assert!(!loaded.state().cables[&a].params.contains_key("amount"));
    // New ids continue past loaded ones.
    let c = loaded.connect_route(out(lfo, "out"), env, "decay_ms");
    assert!(c > b);
}
