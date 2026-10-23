//! The functional-cable editor through real egui input on the reference patch: opened from a
//! route row and from a jack cable's menu, edits land as cable params, undo takes them back.

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect};
use kabl_core::{CableId, ParamTarget, PortRef};
use kabl_ui::{show, PatchEditor, UiState};

const ENV: u64 = 4;

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    events: Vec<Event>,
    t: f64,
    pointer: Pos2,
    size: egui::Vec2,
}

impl H {
    fn new() -> Self {
        Self::sized(egui::vec2(1440.0, 900.0))
    }

    fn sized(size: egui::Vec2) -> Self {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/reference");
        let mut h = H {
            ctx: egui::Context::default(),
            editor: PatchEditor::from_log(kabl_core::load(&dir).expect("patch")),
            ui: UiState::default(),
            events: Vec::new(),
            t: 0.0,
            pointer: Pos2::ZERO,
            size,
        };
        h.frame();
        h.frame();
        h
    }

    fn frame(&mut self) {
        let size = self.size;
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            events: std::mem::take(&mut self.events),
            time: Some(self.t),
            ..Default::default()
        };
        self.t += 1.0 / 60.0;
        self.ctx.begin_pass(raw);
        let mut root = egui::Ui::new(
            self.ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::ZERO, size)),
        );
        show(&mut self.editor, &mut self.ui, &mut root);
        let _ = self.ctx.end_pass();
    }

    fn at(&self, key: &str) -> Pos2 {
        self.ui
            .hits
            .get(key)
            .unwrap_or_else(|| panic!("no hit target {key}"))
            .center()
    }

    fn move_to(&mut self, p: Pos2) {
        self.pointer = p;
        self.events.push(Event::PointerMoved(p));
        self.frame();
    }

    fn button(&mut self, button: PointerButton, pressed: bool) {
        self.events.push(Event::PointerButton {
            pos: self.pointer,
            button,
            pressed,
            modifiers: Modifiers::NONE,
        });
        self.frame();
    }

    fn click_with(&mut self, key: &str, button: PointerButton) {
        let p = self.at(key);
        self.move_to(p);
        self.button(button, true);
        self.button(button, false);
        for _ in 0..3 {
            self.frame();
        }
    }

    fn click(&mut self, key: &str) {
        self.click_with(key, PointerButton::Primary);
    }

    fn drag_by(&mut self, key: &str, dx: f32) {
        let from = self.at(key);
        self.move_to(from);
        self.button(PointerButton::Primary, true);
        for i in 1..=6 {
            self.move_to(from + egui::vec2(dx * i as f32 / 6.0, 0.0));
        }
        self.button(PointerButton::Primary, false);
        self.frame();
    }

    fn drag_between(&mut self, from: Pos2, to: Pos2) {
        self.move_to(from);
        self.button(PointerButton::Primary, true);
        for i in 1..=6 {
            self.move_to(from + (to - from) * (i as f32 / 6.0));
        }
        self.button(PointerButton::Primary, false);
        self.frame();
    }

    fn cable_param(&self, cable: CableId, name: &str) -> Option<f32> {
        self.editor.state().param(&ParamTarget::Cable {
            id: cable,
            param: name.into(),
        })
    }
}

#[test]
fn a_route_gets_a_pattern_from_its_row_and_undo_takes_it_back() {
    let mut t = H::new();
    t.click(&format!("knob:{ENV}.attack_ms"));
    let route = *t
        .ui
        .hits
        .keys()
        .find_map(|k| k.strip_prefix("pattern:"))
        .map(|s| s.parse::<CableId>().unwrap())
        .iter()
        .next()
        .expect("a route row has a Pattern button");
    t.click(&format!("pattern:{route}"));
    assert_eq!(t.ui.cable_fn, Some(route));
    assert_eq!(t.cable_param(route, "length"), None);

    t.drag_by(&format!("fn-length:{route}"), 40.0);
    let len = t.cable_param(route, "length").expect("length stored");
    assert!(
        len >= 1.0,
        "dragging the steps value turns the pattern on: {len}"
    );
    t.frame();
    assert!(
        t.ui.hits.contains_key(&format!("fn-level:{route}:0")),
        "one row per step appears"
    );

    t.drag_by(&format!("fn-level:{route}:0"), -30.0);
    assert!(t.cable_param(route, "s1").is_some_and(|v| v < 1.0));
    t.drag_by(&format!("fn-prob:{route}"), -30.0);
    assert!(t.cable_param(route, "prob").is_some_and(|v| v < 100.0));

    while t.editor.can_undo() && t.cable_param(route, "prob").is_some() {
        t.editor.undo();
    }
    assert_eq!(t.cable_param(route, "prob"), None);
}

#[test]
fn a_jack_cable_opens_the_editor_from_its_menu() {
    let mut t = H::new();
    let jack = *t
        .editor
        .state()
        .cables
        .iter()
        .find(|(_, c)| matches!(c.to, PortRef::Module { .. }))
        .map(|(id, _)| id)
        .expect("a jack cable");
    t.click_with(&format!("cable:{jack}"), PointerButton::Secondary);
    t.click("menu:cable-pattern");
    assert_eq!(t.ui.cable_fn, Some(jack));
    t.drag_by(&format!("fn-length:{jack}"), 30.0);
    assert!(t.cable_param(jack, "length").is_some_and(|v| v >= 1.0));
    t.click(&format!("fn-close:{jack}"));
    assert_eq!(t.ui.cable_fn, None);
}

#[test]
fn a_source_is_added_set_and_removed_for_a_cables_morph_from_its_editor() {
    let mut t = H::new();
    let jack = *t
        .editor
        .state()
        .cables
        .iter()
        .find(|(_, c)| matches!(c.to, PortRef::Module { .. }))
        .map(|(id, _)| id)
        .expect("a jack cable");
    let routes = |t: &H| -> Vec<CableId> {
        t.editor
            .state()
            .cables
            .iter()
            .filter(|(_, c)| matches!(&c.to, PortRef::CableParam { cable, param } if *cable == jack && param == "morph"))
            .map(|(&id, _)| id)
            .collect()
    };
    t.click_with(&format!("cable:{jack}"), PointerButton::Secondary);
    t.click("menu:cable-pattern");
    assert!(routes(&t).is_empty());

    t.click(&format!("croute-add:{jack}:morph"));
    let source =
        t.ui.hits
            .keys()
            .find(|k| k.starts_with(&format!("croute-source:{jack}:morph:")))
            .expect("the chooser lists the patch's outputs")
            .clone();
    t.click(&source);
    let route = *routes(&t).first().expect("the route was added");
    assert_eq!(
        t.cable_param(route, "amount"),
        None,
        "engine default amount"
    );

    t.drag_by(&format!("croute-amount:{route}"), 40.0);
    assert!(t.cable_param(route, "amount").is_some_and(|a| a > 0.25));
    t.click(&format!("croute-invert:{route}"));
    assert!(t.cable_param(route, "amount").is_some_and(|a| a < 0.0));
    kabl_engine::compile::compile(t.editor.state(), 48000.0, 1)
        .expect("the patch with a route into a cable compiles");

    // The routing drawer lists it with the others.
    t.frame();
    assert!(t.ui.hits.contains_key(&format!("croute-remove:{route}")));
    t.click(&format!("croute-remove:{route}"));
    assert!(routes(&t).is_empty());
    t.editor.undo();
    assert_eq!(routes(&t), vec![route], "undo brings the route back");
}

/// Opens the reference patch's first route-row pattern editor with a pattern of 4 or more steps.
fn stepped_editor() -> (H, CableId) {
    let mut t = H::new();
    t.click(&format!("knob:{ENV}.attack_ms"));
    let route = *t
        .ui
        .hits
        .keys()
        .find_map(|k| k.strip_prefix("pattern:"))
        .map(|s| s.parse::<CableId>().unwrap())
        .iter()
        .next()
        .expect("a route row has a Pattern button");
    t.click(&format!("pattern:{route}"));
    let r = t.ui.hits[&format!("fn-length:{route}")];
    t.drag_between(r.left_center(), r.lerp_inside(egui::vec2(0.5, 0.5)));
    assert!(t.cable_param(route, "length").is_some_and(|n| n >= 4.0));
    t.frame();
    (t, route)
}

#[test]
fn clicking_a_step_switches_it_off_and_back_to_its_level() {
    let (mut t, route) = stepped_editor();
    t.drag_between(
        t.ui.hits[&format!("fn-level:{route}:1")].lerp_inside(egui::vec2(0.5, 0.3)),
        t.ui.hits[&format!("fn-level:{route}:1")].lerp_inside(egui::vec2(0.5, 0.5)),
    );
    let level = t.cable_param(route, "s2").expect("level stored");
    assert!(
        (level - 0.5).abs() < 0.06,
        "dragged to half height: {level}"
    );
    t.click(&format!("fn-level:{route}:1"));
    assert_eq!(t.cable_param(route, "s2"), Some(0.0));
    t.click(&format!("fn-level:{route}:1"));
    assert_eq!(t.cable_param(route, "s2"), Some(level));
}

#[test]
fn dragging_across_steps_paints_their_levels_and_chances() {
    let (mut t, route) = stepped_editor();
    let first = t.ui.hits[&format!("fn-level:{route}:0")];
    let last = t.ui.hits[&format!("fn-level:{route}:2")];
    t.drag_between(
        first.lerp_inside(egui::vec2(0.5, 0.25)),
        last.lerp_inside(egui::vec2(0.5, 0.25)),
    );
    for s in ["s1", "s2", "s3"] {
        let v = t.cable_param(route, s).expect("painted");
        assert!((v - 0.75).abs() < 0.06, "{s} painted to 75 %: {v}");
    }
    assert_eq!(t.cable_param(route, "s4"), None, "step 4 untouched");
    let odds = t.ui.hits[&format!("fn-chance:{route}:3")];
    t.drag_between(
        odds.lerp_inside(egui::vec2(0.5, 0.9)),
        odds.lerp_inside(egui::vec2(0.5, 0.5)),
    );
    let chance = t.cable_param(route, "r4").expect("chance stored");
    assert!(
        (chance - 50.0).abs() < 12.0,
        "chance dragged to half: {chance}"
    );
}

#[test]
fn one_step_undoes_by_itself() {
    let (mut t, route) = stepped_editor();
    let cell = t.ui.hits[&format!("fn-level:{route}:0")];
    t.drag_between(
        cell.lerp_inside(egui::vec2(0.5, 0.2)),
        cell.lerp_inside(egui::vec2(0.5, 0.6)),
    );
    assert!(t.cable_param(route, "s1").is_some());
    t.editor.undo();
    assert_eq!(t.cable_param(route, "s1"), None);
    assert!(t.cable_param(route, "length").is_some_and(|n| n >= 4.0));
}

#[test]
fn the_step_grids_fit_the_drawer_at_the_maximum_step_count() {
    for size in [egui::vec2(1440.0, 900.0), egui::vec2(1180.0, 680.0)] {
        let mut t = H::sized(size);
        t.click(&format!("knob:{ENV}.attack_ms"));
        let route = *t
            .ui
            .hits
            .keys()
            .find_map(|k| k.strip_prefix("pattern:"))
            .map(|s| s.parse::<CableId>().unwrap())
            .iter()
            .next()
            .expect("a route row has a Pattern button");
        t.click(&format!("pattern:{route}"));
        for param in ["length", "b.length"] {
            t.editor.set_param_gesture(
                ParamTarget::Cable {
                    id: route,
                    param: param.into(),
                },
                kabl_cables::MAX_STEPS as f32,
                true,
            );
        }
        t.frame();
        t.frame();
        let edge = t.ui.hits[&format!("fn-prob:{route}")].right();
        for tag in ["", "-b"] {
            for k in [0, kabl_cables::MAX_STEPS - 1] {
                for lane in ["level", "chance"] {
                    let r = t.ui.hits[&format!("fn-{lane}{tag}:{route}:{k}")];
                    assert!(
                        r.right() <= edge + 0.5,
                        "{size:?} {lane}{tag} step {k} ends at {} past the drawer edge {edge}",
                        r.right()
                    );
                }
            }
        }
    }
}

#[test]
fn a_routes_depth_gets_a_source_from_the_drawer_and_the_patch_still_compiles() {
    let mut t = H::new();
    let (route, id, param) = t
        .editor
        .state()
        .cables
        .iter()
        .find_map(|(&r, c)| match &c.to {
            PortRef::Param { id, param } => Some((r, *id, param.clone())),
            _ => None,
        })
        .expect("a route into a knob");
    t.ui.inspected = Some((id, param));
    t.ui.selected_route = Some(route);
    t.frame();
    t.click(&format!("croute-add:{route}:amount"));
    let source =
        t.ui.hits
            .keys()
            .find(|k| k.starts_with(&format!("croute-source:{route}:amount:")))
            .expect("the chooser lists the patch's outputs")
            .clone();
    t.click(&source);
    let depth = t
        .editor
        .state()
        .cables
        .iter()
        .find(|(_, c)| matches!(&c.to, PortRef::CableParam { cable, param } if *cable == route && param == "amount"))
        .map(|(&id, _)| id)
        .expect("the depth route was added");
    kabl_engine::compile::compile(t.editor.state(), 48000.0, 1)
        .expect("the patch with a depth route compiles");
    t.click(&format!("croute-remove:{depth}"));
    assert!(!t.editor.state().cables.contains_key(&depth));
}
