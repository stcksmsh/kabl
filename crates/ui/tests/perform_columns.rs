//! With one sequencer and few other controls the Perform view is two columns: the transport
//! over the scenes at the left, the macros over the sequences at the right, every card inside
//! the window.
use egui::{Pos2, RawInput, Rect};
use kabl_ui::perform::{toggle_pin, BANKS};
use kabl_ui::{show, PatchEditor, UiState};

fn frames(editor: &mut PatchEditor, ui: &mut UiState, size: egui::Vec2, n: usize) {
    let ctx = egui::Context::default();
    for i in 0..n {
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
        show(editor, ui, &mut root);
        let _ = ctx.end_pass();
    }
}

#[test]
fn transport_over_scenes_and_macros_over_sequences() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/cue-macros");
    for (w, h) in [(1180.0, 680.0), (1280.0, 800.0), (1440.0, 900.0)] {
        let mut editor = PatchEditor::from_log(kabl_core::load(&dir).unwrap());
        let seq = editor
            .state()
            .modules
            .iter()
            .find(|(_, m)| m.kind == "seq")
            .map(|(&id, _)| id)
            .unwrap();
        toggle_pin(&mut editor, seq, BANKS);
        let mut ui = UiState::default();
        ui.perform_open = true;
        ui.perform_tall = true;
        ui.drawer_open = false;
        frames(&mut editor, &mut ui, egui::vec2(w, h), 4);
        let card = |suffix: &str| {
            *ui.hits
                .iter()
                .find(|(k, _)| k.starts_with("pcard:") && k.ends_with(suffix))
                .map(|(_, r)| r)
                .unwrap_or_else(|| panic!("{w}: no {suffix} card"))
        };
        let (transport, scenes, banks, macros) = (
            card(".transport"),
            card(".cues"),
            card(".banks"),
            card(".m1"),
        );
        let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(w, h));
        for (name, r) in [
            ("transport", transport),
            ("scenes", scenes),
            ("banks", banks),
        ] {
            assert!(screen.contains_rect(r), "{w}x{h}: {name} {r:?} outside");
        }
        assert!(
            transport.bottom() <= scenes.top() + 0.5,
            "{w}: transport over scenes"
        );
        assert!(
            macros.bottom() <= banks.top() + 0.5,
            "{w}: macros over sequences"
        );
        assert!(
            transport.right() <= macros.left() + 0.5,
            "{w}: left column before right"
        );
        assert!(
            !transport.intersects(banks) && !scenes.intersects(banks),
            "{w}: no overlap"
        );
    }
}
