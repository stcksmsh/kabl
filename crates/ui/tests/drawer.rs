//! The routing drawer's tabs and its patch-wide cable list.
use egui::{Event, PointerButton, Pos2, RawInput, Rect};
use kabl_core::PortRef;
use kabl_ui::{show, PatchEditor, UiState};

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    size: egui::Vec2,
    t: f64,
    events: Vec<Event>,
}

impl H {
    fn new() -> Self {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/reference");
        let mut h = H {
            ctx: egui::Context::default(),
            editor: PatchEditor::from_log(kabl_core::load(&dir).expect("patch")),
            ui: UiState::default(),
            size: egui::vec2(1440.0, 900.0),
            t: 0.0,
            events: Vec::new(),
        };
        h.frames(3);
        h
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            let raw = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
                time: Some(self.t),
                events: std::mem::take(&mut self.events),
                ..Default::default()
            };
            self.t += 1.0 / 60.0;
            self.ctx.begin_pass(raw);
            let mut root = egui::Ui::new(
                self.ctx.clone(),
                egui::Id::new("root"),
                egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::ZERO, self.size)),
            );
            show(&mut self.editor, &mut self.ui, &mut root);
            let _ = self.ctx.end_pass();
        }
    }

    fn click(&mut self, key: &str) {
        let p = self.ui.hits[key].center();
        self.events.push(Event::PointerMoved(p));
        self.frames(1);
        for pressed in [true, false] {
            self.events.push(Event::PointerButton {
                pos: p,
                button: PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            });
            self.frames(1);
        }
        self.frames(2);
    }
}

#[test]
fn the_cable_list_draws_rows_for_jack_cables() {
    let h = H::new();
    let jack_cables = h
        .editor
        .state()
        .cables
        .values()
        .filter(|c| {
            matches!(c.from, PortRef::Module { .. }) && matches!(c.to, PortRef::Module { .. })
        })
        .count();
    let rows =
        h.ui.hits
            .keys()
            .filter(|k| k.starts_with("cable-row:"))
            .count();
    assert!(jack_cables > 0);
    // Rows below the drawer's visible part are not drawn into hits, so count at most all.
    assert!(
        rows > 0 && rows <= jack_cables,
        "{rows} rows, {jack_cables} cables"
    );
}

#[test]
fn the_explain_view_belongs_to_routing_and_the_routing_tab_closes_it() {
    let mut h = H::new();
    let id = *h.editor.state().modules.keys().next().unwrap();
    h.ui.selected_module = Some(id);
    h.frames(2);
    kabl_ui::explain::open(&mut h.ui, kabl_ui::explain::Subject::Module(id), None);
    h.frames(1);
    assert!(h.ui.explain.is_open());
    h.click("routing-tab");
    assert!(!h.ui.explain.is_open());
}

#[test]
fn the_inspect_tab_toggles_the_inspector() {
    let mut h = H::new();
    h.click("inspect-open");
    assert!(h.ui.inspect.open);
    h.click("routing-tab");
    assert!(!h.ui.inspect.open);
}
