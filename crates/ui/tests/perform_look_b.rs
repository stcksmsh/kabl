//! Perform in look B (UI spec 9): the live dot on a modulated macro, the page dimming of a long
//! step lane, and the one-token ink of what you play.
use egui::epaint::{CircleShape, ClippedShape, Shape};
use egui::{Pos2, RawInput, Rect};
use kabl_core::PortRef;
use kabl_ui::displays::LiveTap;
use kabl_ui::perform::toggle_pin;
use kabl_ui::{show, PatchEditor, UiState};

/// A macro pinned to Perform, optionally modulated by an LFO. Returns the editor and the LFO.
fn patch(modulated: bool) -> (PatchEditor, kabl_core::ModuleId) {
    let mut e = PatchEditor::seed_from(&Default::default());
    let at = |x: f32, y: f32| kabl_core::Vec2 { x, y };
    let mac = e.add_module("macro", at(24.0, 20.0));
    let lfo = e.add_module("lfo", at(244.0, 20.0));
    e.set_param(mac, "m1", 0.5);
    if modulated {
        e.connect_route(
            PortRef::Module {
                id: lfo,
                port: "out".into(),
            },
            mac,
            "m1",
        );
    }
    toggle_pin(&mut e, mac, "m1");
    (e, lfo)
}

fn circles(shapes: &[ClippedShape], out: &mut Vec<CircleShape>) {
    fn walk(s: &Shape, out: &mut Vec<CircleShape>) {
        match s {
            Shape::Circle(c) => out.push(*c),
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    shapes.iter().for_each(|c| walk(&c.shape, out));
}

/// Draws a few Perform frames in look B; true when a live dot (a ring in the `cv` colour) was
/// painted in the last one.
fn live_dot_painted(modulated: bool, with_source: bool) -> bool {
    let (mut editor, lfo) = patch(modulated);
    let mut ui = UiState::default();
    ui.theme = "kabl.look_b".into();
    ui.perform_open = true;
    ui.perform_tall = true;
    ui.drawer_open = false;
    let size = egui::vec2(1440.0, 900.0);
    let ctx = egui::Context::default();
    let mut painted = Vec::new();
    for i in 0..4 {
        let t = i as f64 / 60.0;
        if with_source {
            ui.live.set_tap(
                lfo,
                0,
                LiveTap {
                    last: 0.2,
                    min: -0.2,
                    max: 0.2,
                    view: kabl_modules::ModuleView {
                        valid: false,
                        position: 0.0,
                        cycle: [0.0; kabl_modules::VIEW_CYCLE],
                    },
                    at: t,
                },
            );
        }
        ctx.begin_pass(RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            time: Some(t),
            ..Default::default()
        });
        let mut root = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::ZERO, size)),
        );
        show(&mut editor, &mut ui, &mut root);
        let out = ctx.end_pass();
        painted.clear();
        circles(&out.shapes, &mut painted);
    }
    let cv = ui.style.roles.cv;
    painted
        .iter()
        .any(|c| c.stroke.color == cv && (c.radius - 5.0).abs() < 0.01)
}

#[test]
fn a_modulated_macro_shows_a_live_dot_and_only_then() {
    assert!(live_dot_painted(true, true), "modulated, source reporting");
    assert!(!live_dot_painted(true, false), "no live source: no dot");
    assert!(!live_dot_painted(false, true), "not modulated: no dot");
}

#[test]
fn a_long_lane_dims_the_pages_the_playhead_is_not_on() {
    use kabl_ui::kit::lane_page_dim;
    // Eight steps or fewer: one page, never dimmed.
    assert_eq!(lane_page_dim(3, Some(5), 8), 1.0);
    // Sixteen: the playing page is bright, the other 55 %.
    assert_eq!(lane_page_dim(2, Some(0), 16), 1.0);
    assert_eq!(lane_page_dim(9, Some(0), 16), 0.55);
    assert_eq!(lane_page_dim(9, Some(12), 16), 1.0);
    assert_eq!(lane_page_dim(2, Some(12), 16), 0.55);
    // Stopped: the first page is the bright one.
    assert_eq!(lane_page_dim(2, None, 16), 1.0);
    assert_eq!(lane_page_dim(9, None, 16), 0.55);
}

#[test]
fn what_you_play_has_one_ink_token() {
    let ui = UiState::default();
    // Teal (the cv role), by the owner's choice; one line in `kit::perform_ink`.
    assert_eq!(kabl_ui::kit::perform_ink(&ui.style), ui.style.roles.cv);
}
