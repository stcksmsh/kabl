//! Face widths are frozen (PLAN §4): at every zoom a drawn face is exactly its layout width x
//! zoom, in every theme, or saved racks re-flow. For every module kind, in Satin Studio and in
//! Signal solid, the face rect the app records equals `layout.face.width() * zoom`.

use egui::{Pos2, RawInput, Rect};
use kabl_core::Vec2;
use kabl_modules::registry;
use kabl_ui::{show, PatchEditor, UiState};

const ZOOMS: [f32; 5] = [0.5, 0.58, 0.75, 1.0, 1.5];

fn drawn_width(kind: &str, theme: &str, dark: bool, zoom: f32) -> Option<f32> {
    let ctx = egui::Context::default();
    let mut editor = PatchEditor::seed_from(&Default::default());
    let id = editor.add_module(kind, Vec2 { x: 24.0, y: 20.0 });
    let mut ui = UiState::default();
    ui.theme = theme.into();
    ui.dark = dark;
    ui.browser_open = false;
    ui.drawer_open = false;
    let size = egui::vec2(1440.0, 900.0);
    for i in 0..4 {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            time: Some(i as f64 / 60.0),
            focused: true,
            ..Default::default()
        };
        ctx.begin_pass(raw);
        let mut root = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::ZERO, size)),
        );
        ui.zoom = zoom;
        ui.pan = egui::vec2(12.0, 12.0);
        show(&mut editor, &mut ui, &mut root);
        let _ = ctx.end_pass();
    }
    ui.hits.get(&format!("module:{id}")).map(|r| r.width())
}

#[test]
fn face_width_is_the_same_in_every_theme_and_zoom() {
    let mut bad = Vec::new();
    let mut checked = 0;
    for info in registry::all_infos() {
        for zoom in ZOOMS {
            let a = drawn_width(info.kind, "kabl.satin", true, zoom);
            let b = drawn_width(info.kind, "kabl.line_solid", true, zoom);
            let c = drawn_width(info.kind, "kabl.line_solid", false, zoom);
            let (Some(a), Some(b), Some(c)) = (a, b, c) else {
                continue;
            };
            checked += 1;
            let w1 = drawn_width(info.kind, "kabl.satin", true, 1.0).unwrap();
            if (a - w1 * zoom).abs() > 0.5 {
                bad.push(format!(
                    "{} z{zoom}: {a:.1} is not {w1:.1} x zoom",
                    info.kind
                ));
            }
            if (a - b).abs() > 0.5 || (a - c).abs() > 0.5 {
                bad.push(format!(
                    "{} z{zoom}: satin {a:.1} line_solid dark {b:.1} light {c:.1}",
                    info.kind
                ));
            }
        }
    }
    assert!(checked > 100, "only {checked} faces measured");
    assert!(
        bad.is_empty(),
        "{} mismatches:\n{}",
        bad.len(),
        bad.join("\n")
    );
}
