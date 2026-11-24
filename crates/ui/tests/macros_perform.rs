//! Eight named, pinned macros on the Perform page (`patches/macros-eight`) fit one row inside
//! the plugin editor's default 1180 by 680 window and the standalone minimum, without overlap.
//! Each macro's explain and menu sit in the card's header, above the knobs.

use egui::{Pos2, RawInput, Rect};
use kabl_ui::{show, PatchEditor, UiState};
use std::collections::BTreeMap;

fn eight() -> PatchEditor {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/macros-eight");
    PatchEditor::from_log(kabl_core::load(&dir).expect("patch"))
}

/// The Perform page drawn at `w` by `h`: the macro module's id and every recorded hit rect.
fn perform(
    mut editor: PatchEditor,
    w: f32,
    h: f32,
) -> (kabl_core::ModuleId, BTreeMap<String, Rect>) {
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
    (id, ui.hits)
}

fn tiles(w: f32, h: f32) -> Vec<Rect> {
    let (id, hits) = perform(eight(), w, h);
    (1..=8)
        .map(|k| {
            *hits
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

#[test]
fn macro_tools_sit_in_the_header_above_the_knobs() {
    for (patch, n) in [
        (eight as fn() -> PatchEditor, 8),
        (kabl_ui::first_run::start_patch, 2),
    ] {
        for (w, h) in [(1180.0, 680.0), (1280.0, 800.0), (1440.0, 900.0)] {
            let (id, hits) = perform(patch(), w, h);
            let knob_top = (1..=n)
                .map(|k| hits[&format!("pslider:{id}.m{k}")].top())
                .fold(f32::INFINITY, f32::min);
            let mut tools: Vec<Rect> = Vec::new();
            for k in 1..=n {
                for what in ["pexplain", "pmenu"] {
                    let r = hits
                        .get(&format!("{what}:{id}.m{k}"))
                        .unwrap_or_else(|| panic!("no {what} for m{k} at {w}"));
                    assert!(
                        r.bottom() <= knob_top,
                        "{what} m{k} at {w}: {r:?} not above the knobs"
                    );
                    // A full-size button (the theme's control height), not the small one.
                    assert!(r.height() >= 28.0, "{what} m{k} at {w}: hit target {r:?}");
                    for o in &tools {
                        assert!(!r.intersects(*o), "{what} m{k} at {w} overlaps {o:?}");
                    }
                    tools.push(*r);
                }
            }
        }
    }
}

#[test]
fn the_cc_note_stays_visible_and_clear_of_the_tools() {
    for (w, h) in [(1180.0, 680.0), (1280.0, 800.0), (1440.0, 900.0)] {
        let (id, hits) = perform(eight(), w, h);
        let note = hits
            .get("pnote:macros")
            .unwrap_or_else(|| panic!("no CC note at {w}"));
        assert!(note.width() > 0.0, "empty CC note at {w}");
        for (k, r) in &hits {
            let ours = (1..=8).any(|m| {
                ["pexplain", "pmenu", "pslider"]
                    .iter()
                    .any(|what| *k == format!("{what}:{id}.m{m}"))
            });
            assert!(
                !ours || !note.intersects(*r),
                "CC note {note:?} at {w} overlaps {k} {r:?}"
            );
        }
    }
}
