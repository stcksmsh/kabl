//! Perform card styling: the transport's run button is a round, large-size control, and the
//! scene pads are drawn inside their cards.
use egui::{Pos2, RawInput, Rect};
use kabl_ui::{show, PatchEditor, UiState};

#[test]
fn the_run_button_is_a_large_disc_and_the_scene_pads_fit_their_card() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/cue-macros");
    let size = egui::vec2(1440.0, 900.0);
    let mut editor = PatchEditor::from_log(kabl_core::load(&dir).unwrap());
    let mut ui = UiState::default();
    ui.perform_open = true;
    ui.perform_tall = true;
    ui.drawer_open = false;
    let ctx = egui::Context::default();
    for i in 0..4 {
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
    let run = ui
        .hits
        .iter()
        .find(|(k, _)| k.starts_with("prun:"))
        .map(|(_, r)| *r)
        .expect("the run button");
    let large = ui.style.metrics.large_h;
    assert!(
        (run.width() - large).abs() < 0.5 && (run.height() - large).abs() < 0.5,
        "run button {run:?}, large size {large}"
    );
    let card = ui
        .hits
        .iter()
        .find(|(k, _)| k.starts_with("pcard:") && k.ends_with(".cues"))
        .map(|(_, r)| *r)
        .expect("the scenes card");
    let pads: Vec<Rect> = ui
        .hits
        .iter()
        .filter(|(k, _)| k.starts_with("pcue:"))
        .map(|(_, r)| *r)
        .collect();
    assert!(!pads.is_empty());
    for p in pads {
        assert!(card.contains_rect(p), "pad {p:?} outside card {card:?}");
    }
}
