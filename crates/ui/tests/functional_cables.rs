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
}

impl H {
    fn new() -> Self {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/reference");
        let mut h = H {
            ctx: egui::Context::default(),
            editor: PatchEditor::from_log(kabl_core::load(&dir).expect("patch")),
            ui: UiState::default(),
            events: Vec::new(),
            t: 0.0,
            pointer: Pos2::ZERO,
        };
        h.frame();
        h.frame();
        h
    }

    fn frame(&mut self) {
        let size = egui::vec2(1440.0, 900.0);
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
