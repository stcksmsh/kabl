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

/// The mockup's patch: sequencer, VA oscillator, SVF, ADSR, mixer, delay, reverb, output.
fn mockup_patch() -> PatchEditor {
    use kabl_core::PortRef;
    let mut e = PatchEditor::seed_from(&Default::default());
    let at = |x: f32, y: f32| kabl_core::Vec2 { x, y };
    let seq = e.add_module("seq", at(24.0, 20.0));
    let osc = e.add_module("osc.va", at(714.0, 20.0));
    let svf = e.add_module("filter.svf", at(964.0, 20.0));
    let adsr = e.add_module("env.adsr", at(24.0, 320.0));
    let mix = e.add_module("mixer", at(244.0, 320.0));
    let delay = e.add_module("delay", at(504.0, 320.0));
    let verb = e.add_module("reverb", at(744.0, 320.0));
    let out = e.add_module("out", at(984.0, 320.0));
    let m = |id, port: &str| PortRef::Module {
        id,
        port: port.to_string(),
    };
    e.connect(m(seq, "pitch"), m(osc, "pitch"));
    e.connect(m(osc, "out"), m(svf, "in"));
    e.connect(m(seq, "gate"), m(adsr, "gate"));
    e.connect_route(m(adsr, "out"), svf, "cutoff_hz");
    e.connect(m(svf, "lp"), m(mix, "in1"));
    e.connect(m(mix, "out"), m(delay, "in"));
    e.connect(m(delay, "out"), m(verb, "in"));
    e.connect(m(verb, "out"), m(out, "in"));
    e
}

/// After the fit, every cable rope (sag included) sits inside the rack viewport, at every
/// window size, with the drawer shut and open, in both modes.
#[test]
fn the_fit_keeps_every_cable_inside_the_viewport() {
    for (w, h) in [(1440.0, 900.0), (1280.0, 800.0), (1180.0, 680.0)] {
        for drawer in [false, true] {
            for dark in [true, false] {
                let mut s = H::new(w, h);
                s.editor = mockup_patch();
                s.ui = UiState::default();
                s.ui.dark = dark;
                s.ui.drawer_open = drawer;
                s.frames(1);
                // A compact window shuts the drawer on its first frame: open it after.
                s.ui.drawer_open = drawer;
                s.frames(8);
                let ropes = s.ui.cable_bounds().expect("cables are drawn");
                let view = s.ui.rack_canvas();
                assert!(
                    view.contains_rect(ropes),
                    "{w}x{h} drawer {drawer} dark {dark}: ropes {ropes:?} outside {view:?}"
                );
                // The rope layer is cut by nothing but the viewport and paints after all module
                // content (trays, plates, titles), so nothing hides a rope.
                let (clip, modules_end, cables_start, cables_end) = s.ui.cable_audit;
                assert!(
                    clip.contains_rect(view),
                    "{w}x{h}: the cable layer is clipped to {clip:?}, smaller than {view:?}"
                );
                assert!(
                    cables_start >= modules_end && cables_end > cables_start,
                    "{w}x{h}: cables must paint after the modules ({modules_end}, {cables_start}..{cables_end})"
                );
            }
        }
    }
}
