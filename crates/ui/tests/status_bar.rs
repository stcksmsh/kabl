//! The status bar at the smallest common window sizes: with a long MIDI port name and a long
//! audio message, every item stays inside the window and none overlaps another.
use egui::{Pos2, RawInput, Rect};
use kabl_ui::{kit, show_with, PatchEditor, UiState};

/// The standalone's own controls at the bar's right end (`app/frame.rs` `toolbar`), with the
/// same sizes and hit keys.
fn host_controls(ui: &mut egui::Ui, ui_state: &mut UiState) {
    let st = ui_state.style.clone();
    for (key, text, ghost) in [
        ("diagnostics", "Diagnostics", true),
        ("log-path", "Log", true),
        ("audio-retry", "Retry audio", false),
    ] {
        let b = kit::Button::new(&st, text).small();
        let r = ui.add(if ghost { b.ghost() } else { b });
        ui_state.record(key.into(), r.rect);
    }
    let r = kit::dropdown(ui, &st, "audio-output", "System default", 190.0, |_| {});
    ui_state.record("audio-output".into(), r.rect);
}

fn status_hits(w: f32, h: f32) -> Vec<(String, Rect)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/reference");
    let mut editor = PatchEditor::from_log(kabl_core::load(&dir).expect("patch"));
    let mut ui_state = UiState::default();
    ui_state.status_meter = true;
    ui_state.status_cpu = Some(12.0);
    // The reference patch has no clock; a playing one shows these.
    ui_state.status_bpm = Some(112.0);
    ui_state.status_pos = Some("26 . 3 . 2".into());
    ui_state.midi_input =
        Some("Arturia KeyStep 37 MIDI 1 (USB MIDI Interface, port 0, client 24) long name".into());
    ui_state.status_audio =
        "audio running · session 12 · current graph installed · stream restarted after an xrun \
         on the default device"
            .into();
    let ctx = egui::Context::default();
    let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(w, h));
    for i in 0..4 {
        ctx.begin_pass(RawInput {
            screen_rect: Some(screen),
            time: Some(i as f64 / 60.0),
            ..Default::default()
        });
        let mut root = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(screen),
        );
        show_with(&mut editor, &mut ui_state, &mut root, host_controls);
        // Only the layout is checked; the paint output is not needed.
        let _ = ctx.end_pass();
    }
    ui_state
        .hits
        .iter()
        .filter(|(k, _)| {
            k.starts_with("status:")
                || matches!(
                    k.as_str(),
                    "keys:toggle" | "diagnostics" | "log-path" | "audio-retry" | "audio-output"
                )
        })
        .map(|(k, r)| (k.clone(), *r))
        .collect()
}

#[test]
fn status_bar_items_fit_the_window_and_do_not_overlap() {
    for (w, h) in [(1280.0, 800.0), (1180.0, 680.0)] {
        let hits = status_hits(w, h);
        let keys: Vec<&str> = hits.iter().map(|(k, _)| k.as_str()).collect();
        for want in [
            "status:meter",
            "status:peak",
            "status:midi",
            "keys:toggle",
            "status:message",
            "status:diagnostics",
            "status:tempo",
            "status:position",
            "diagnostics",
            "log-path",
            "audio-retry",
            "audio-output",
        ] {
            assert!(keys.contains(&want), "{w}x{h}: {want} not drawn: {keys:?}");
        }
        let window = Rect::from_min_size(Pos2::ZERO, egui::vec2(w, h));
        for (k, r) in &hits {
            assert!(
                window.contains_rect(*r),
                "{w}x{h}: {k} {r:?} leaves the window"
            );
        }
        for (i, (a, ra)) in hits.iter().enumerate() {
            for (b, rb) in &hits[i + 1..] {
                let both = ra.intersect(*rb);
                assert!(
                    both.width() <= 0.0 || both.height() <= 0.0,
                    "{w}x{h}: {a} {ra:?} overlaps {b} {rb:?}"
                );
            }
        }
    }
}
