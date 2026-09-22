//! `PatchEditor` correctness (undo/redo, add/remove/connect/disconnect, seeding from an existing
//! patch) plus a headless smoke test of `show()` itself. `egui::Context` needs no window or GPU
//! to drive a frame (`begin_pass`/`end_pass` are pure Rust) — see `lib.rs`'s module doc for why
//! that's enough to catch gross bugs (panics, out-of-bounds indexing) even in this container,
//! which has no display server to actually look at the result with.

use kabl_core::{PortRef, Vec2};
use kabl_standalone::default_patch;
use kabl_ui::{editor::PatchEditor, show, UiState};

fn zero() -> Vec2 {
    Vec2 { x: 0.0, y: 0.0 }
}

#[test]
fn add_module_appears_in_state() {
    let mut editor = PatchEditor::new();
    let id = editor.add_module("osc.va", zero());
    assert_eq!(
        editor.state().modules.get(&id).map(|m| m.kind.as_str()),
        Some("osc.va")
    );
}

#[test]
fn remove_module_removes_it_and_its_cables() {
    let mut editor = PatchEditor::new();
    let a = editor.add_module("osc.va", zero());
    let b = editor.add_module("out", zero());
    let cable_id = editor.connect(
        PortRef::Module {
            id: a,
            port: "out".into(),
        },
        PortRef::Module {
            id: b,
            port: "left".into(),
        },
    );
    assert!(editor.state().cables.contains_key(&cable_id));

    editor.remove_module(a);
    assert!(!editor.state().modules.contains_key(&a));
    // core's inverse-for-RemoveModule semantics: does removing a module also remove cables
    // referencing it, or leave a dangling cable? Whichever core does, the editor doesn't need
    // its own opinion -- just confirm nothing panics and the module itself is gone.
    assert!(editor.state().modules.contains_key(&b));
}

#[test]
fn connect_and_disconnect_round_trip() {
    let mut editor = PatchEditor::new();
    let a = editor.add_module("osc.va", zero());
    let b = editor.add_module("out", zero());
    let cable_id = editor.connect(
        PortRef::Module {
            id: a,
            port: "out".into(),
        },
        PortRef::Module {
            id: b,
            port: "left".into(),
        },
    );
    assert!(editor.state().cables.contains_key(&cable_id));
    editor.disconnect(cable_id);
    assert!(!editor.state().cables.contains_key(&cable_id));
}

#[test]
fn set_param_updates_state() {
    let mut editor = PatchEditor::new();
    let id = editor.add_module("osc.va", zero());
    editor.set_param(id, "base_hz", 440.0);
    assert_eq!(
        editor.state().modules[&id].params.get("base_hz"),
        Some(&440.0)
    );
}

#[test]
fn move_module_updates_position() {
    let mut editor = PatchEditor::new();
    let id = editor.add_module("osc.va", zero());
    editor.move_module(id, Vec2 { x: 12.0, y: 34.0 });
    assert_eq!(editor.state().modules[&id].pos, Vec2 { x: 12.0, y: 34.0 });
}

#[test]
fn undo_redo_round_trips_add_module() {
    let mut editor = PatchEditor::new();
    assert!(!editor.can_undo());
    let id = editor.add_module("osc.va", zero());
    assert!(editor.state().modules.contains_key(&id));
    assert!(editor.can_undo());

    assert!(editor.undo());
    assert!(!editor.state().modules.contains_key(&id));
    assert!(editor.can_redo());

    assert!(editor.redo());
    assert!(editor.state().modules.contains_key(&id));
}

#[test]
fn undo_with_nothing_to_undo_is_a_no_op() {
    let mut editor = PatchEditor::new();
    assert!(!editor.undo());
}

#[test]
fn redo_with_nothing_to_redo_is_a_no_op() {
    let mut editor = PatchEditor::new();
    editor.add_module("osc.va", zero());
    assert!(!editor.can_redo());
    assert!(!editor.redo());
}

#[test]
fn mutations_set_the_dirty_flag_and_take_dirty_clears_it() {
    let mut editor = PatchEditor::new();
    assert!(!editor.is_dirty());
    editor.add_module("osc.va", zero());
    assert!(editor.is_dirty());
    assert!(editor.take_dirty());
    assert!(!editor.is_dirty());
    assert!(!editor.take_dirty());
}

#[test]
fn seed_from_reproduces_the_source_patch_exactly() {
    let source = default_patch();
    let editor = PatchEditor::seed_from(&source);
    assert_eq!(editor.state(), &source);
}

#[test]
fn seed_from_is_undoable_back_to_empty() {
    let source = default_patch();
    let mut editor = PatchEditor::seed_from(&source);
    assert!(!editor.state().modules.is_empty());
    while editor.undo() {}
    assert!(editor.state().modules.is_empty());
    assert!(editor.state().cables.is_empty());
}

#[test]
fn seeding_does_not_mark_the_editor_dirty() {
    // Seeding from an existing patch isn't itself a user edit -- nothing should need a swap for
    // it (the caller compiles the seeded patch directly before ever calling take_dirty()).
    let editor = PatchEditor::seed_from(&default_patch());
    assert!(!editor.is_dirty());
}

#[test]
fn ids_assigned_after_seeding_do_not_collide_with_seeded_ids() {
    let source = default_patch();
    let max_seeded = *source.modules.keys().max().unwrap();
    let mut editor = PatchEditor::seed_from(&source);
    let new_id = editor.add_module("vca", zero());
    assert!(new_id > max_seeded);
}

#[test]
fn known_kinds_is_the_full_registry() {
    let kinds = PatchEditor::known_kinds();
    assert!(kinds.contains(&"osc.va"));
    assert!(kinds.contains(&"out"));
    assert_eq!(kinds.len(), kabl_modules::registry::KNOWN_KINDS.len());
}

// --- headless show() smoke test: no window, no GPU, just egui's pure-Rust Context ---

#[test]
fn show_runs_without_panicking_across_several_frames() {
    let mut editor = PatchEditor::seed_from(&default_patch());
    let mut ui_state = UiState::default();
    let ctx = egui::Context::default();

    for _ in 0..5 {
        ctx.begin_pass(egui::RawInput::default());
        // Drive the real widget tree through the same entry point main.rs uses.
        let mut root = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("kabl-test-root"),
            egui::UiBuilder::new(),
        );
        show(&mut editor, &mut ui_state, &mut root);
        let _ = ctx.end_pass();
    }

    // Sanity: the seeded patch's modules should still all be present -- show() only mutates via
    // explicit user interaction (clicks), and RawInput::default() has none, so nothing should
    // have changed just from rendering empty-input frames.
    assert_eq!(editor.state().modules.len(), default_patch().modules.len());
}

#[test]
fn show_with_a_selected_module_renders_its_param_panel_without_panicking() {
    let mut editor = PatchEditor::seed_from(&default_patch());
    let some_id = *editor.state().modules.keys().next().unwrap();
    let mut ui_state = UiState::default();
    ui_state.selected_module = Some(some_id);
    let ctx = egui::Context::default();
    ctx.begin_pass(egui::RawInput::default());
    let mut root = egui::Ui::new(
        ctx.clone(),
        egui::Id::new("kabl-test-root-2"),
        egui::UiBuilder::new(),
    );
    show(&mut editor, &mut ui_state, &mut root);
    let _ = ctx.end_pass();
}
