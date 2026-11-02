//! Compact windows (below 1280 px, the plugin): the drawer starts shut and, when opened, floats
//! over the rack instead of splitting the window; the rack fits itself on resize until the user
//! zooms.
use egui::{Pos2, RawInput, Rect};
use kabl_ui::{show, PatchEditor, UiState};

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    size: egui::Vec2,
    t: f64,
}

impl H {
    fn new(w: f32, h: f32) -> Self {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/reference");
        let mut s = H {
            ctx: egui::Context::default(),
            editor: PatchEditor::from_log(kabl_core::load(&dir).expect("patch")),
            ui: UiState::default(),
            size: egui::vec2(w, h),
            t: 0.0,
        };
        s.frames(3);
        s
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            let raw = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
                time: Some(self.t),
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
}

#[test]
fn the_drawer_starts_shut_in_a_compact_window_only() {
    assert!(!H::new(1180.0, 680.0).ui.drawer_open);
    assert!(H::new(1440.0, 900.0).ui.drawer_open);
}

#[test]
fn an_open_drawer_floats_over_the_rack_in_a_compact_window() {
    let mut h = H::new(1180.0, 680.0);
    let zoom = h.ui.zoom;
    h.ui.drawer_open = true;
    h.frames(3);
    assert_eq!(h.ui.zoom, zoom, "the rack does not refit under the drawer");
    let routing = h.ui.hits["routing"];
    assert!(Rect::from_min_size(Pos2::ZERO, h.size).contains_rect(routing));
}

#[test]
fn a_compact_window_refits_the_rack_until_the_user_zooms() {
    let mut h = H::new(1180.0, 680.0);
    let before = h.ui.zoom;
    h.size = egui::vec2(1000.0, 560.0);
    h.frames(3);
    assert!(h.ui.zoom <= before, "a smaller window never zooms in");
    let fitted = h.ui.zoom;
    h.ui.zoom = 1.0;
    h.ui.auto_fit = false;
    h.size = egui::vec2(1180.0, 680.0);
    h.frames(3);
    assert_eq!(h.ui.zoom, 1.0, "after a manual zoom the rack stays put");
    assert!(fitted <= 1.0);
}
