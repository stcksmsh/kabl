//! Eight named, pinned macros on the Perform page (`patches/macros-eight`) fit one row inside
//! the plugin editor's default 1180 by 680 window and the standalone minimum, without overlap.

use egui::{Pos2, RawInput, Rect};
use kabl_ui::{show, PatchEditor, UiState};

fn tiles(w: f32, h: f32) -> Vec<Rect> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/macros-eight");
    let mut editor = PatchEditor::from_log(kabl_core::load(&dir).expect("patch"));
    let id = editor
        .state()
        .modules
        .iter()
        .find(|(_, m)| m.kind == "macro")
        .map(|(&id, _)| id)
        .unwrap();
    let mut ui = UiState::default();
    ui.perform_open = true;
    ui.perform_tall = true;
    let ctx = egui::Context::default();
    let size = egui::vec2(w, h);
    for i in 0..3 {
        ctx.begin_pass(RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            time: Some(i as f64 / 60.0),
            ..Default::default()
        });
        let mut root = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::ZERO, size)),
        );
        show(&mut editor, &mut ui, &mut root);
        let _ = ctx.end_pass();
    }
    (1..=8)
        .map(|k| {
            *ui.hits
                .get(&format!("pslider:{id}.m{k}"))
                .unwrap_or_else(|| panic!("no tile for m{k}"))
        })
        .collect()
}

#[test]
fn eight_macros_share_one_row_and_stay_inside_the_window() {
    for (w, h) in [(1180.0, 680.0), (1280.0, 800.0), (1440.0, 900.0)] {
        let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(w, h));
        let t = tiles(w, h);
        for (k, r) in t.iter().enumerate() {
            assert!(
                screen.contains_rect(*r),
                "m{} outside {w}x{h}: {r:?}",
                k + 1
            );
            assert!(r.width() >= 40.0, "m{} too narrow at {w}: {r:?}", k + 1);
        }
        for pair in t.windows(2) {
            assert!(
                pair[0].max.x <= pair[1].min.x + 0.5,
                "overlap at {w}: {pair:?}"
            );
            assert!(
                (pair[0].center().y - pair[1].center().y).abs() < 1.0,
                "one row at {w}"
            );
        }
    }
}
