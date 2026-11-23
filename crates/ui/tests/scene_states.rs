//! Perform's scene pads: exactly one cue reads "Playing" and at most one "Queued", also when
//! every cue sets the same sequencer banks (the promo Lichen, whose scenes differ only in their
//! macros, showed every pad as Playing).
use egui::epaint::{ClippedShape, Shape};
use egui::{Pos2, RawInput, Rect};
use kabl_core::ModuleId;
use kabl_ui::{cues, show, PatchEditor, UiState};

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    t: f64,
}

impl H {
    /// The factory Lichen in Perform; with `banks`, every cue also sets bank A of every
    /// sequencer, as the promo patch does.
    fn lichen(banks: bool) -> Self {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/presets/lichen");
        let mut editor = PatchEditor::from_log(kabl_core::load(&dir).expect("lichen"));
        let state = editor.state().clone();
        let id = cues_id(&editor);
        if banks {
            let seqs: Vec<ModuleId> = state
                .modules
                .iter()
                .filter(|(_, m)| m.kind == "seq")
                .map(|(&s, _)| s)
                .collect();
            for c in cues::cues(&state, id) {
                for &s in &seqs {
                    cues::set_target(&mut editor, id, c.n, s, Some(0));
                }
            }
        }
        let mut ui = UiState::default();
        ui.perform_open = true;
        ui.drawer_open = false;
        // The MIDI connect guard has long passed.
        kabl_ui::perform::rearm(&mut ui, -60.0);
        H {
            ctx: egui::Context::default(),
            editor,
            ui,
            t: 0.0,
        }
    }

    /// Draws a frame; the scene pads' state words, in drawing order.
    fn states(&mut self) -> Vec<String> {
        let size = egui::vec2(1440.0, 900.0);
        self.ctx.begin_pass(RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            time: Some(self.t),
            ..Default::default()
        });
        self.t += 1.0 / 60.0;
        let mut root = egui::Ui::new(
            self.ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::ZERO, size)),
        );
        show(&mut self.editor, &mut self.ui, &mut root);
        let out = self.ctx.end_pass();
        let mut words = Vec::new();
        fn walk(s: &Shape, out: &mut Vec<String>) {
            match s {
                Shape::Text(t) if ["Playing", "Queued", "Ready"].contains(&t.galley.text()) => {
                    out.push(t.galley.text().to_string())
                }
                Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        out.shapes
            .iter()
            .for_each(|c: &ClippedShape| walk(&c.shape, &mut words));
        words
    }

    /// Presses a cue's MIDI button (Lichen maps cues 1-4 to CC 40-43).
    fn launch(&mut self, n: u8) {
        let t = self.t;
        kabl_ui::perform::apply_cc(&mut self.editor, &mut self.ui, &[(0, 39 + n, 127)], t);
        kabl_ui::perform::apply_cc(&mut self.editor, &mut self.ui, &[(0, 39 + n, 0)], t);
    }
}

fn cues_id(e: &PatchEditor) -> ModuleId {
    *e.state()
        .modules
        .iter()
        .find(|(_, m)| m.kind == "cues")
        .expect("cues module")
        .0
}

fn count(words: &[String], w: &str) -> usize {
    words.iter().filter(|x| *x == w).count()
}

#[test]
fn only_the_launched_lichen_scene_is_playing() {
    for banks in [false, true] {
        let mut h = H::lichen(banks);
        let words = h.states();
        assert_eq!(words.len(), 4, "four Lichen scenes ({banks}): {words:?}");
        assert_eq!(
            count(&words, "Playing"),
            0,
            "nothing launched ({banks}): {words:?}"
        );

        h.launch(2);
        let words = h.states();
        assert_eq!(
            words,
            ["Ready", "Playing", "Ready", "Ready"],
            "Colony launched ({banks})"
        );

        h.launch(3);
        let words = h.states();
        assert_eq!(
            words,
            ["Ready", "Ready", "Playing", "Ready"],
            "Bloom launched ({banks})"
        );
    }
}

#[test]
fn only_the_launched_scene_is_queued_when_scenes_share_banks() {
    let mut h = H::lichen(true);
    h.launch(2);
    // Every sequencer waits for bank A at the bar: all four scenes would land there.
    let id = cues_id(&h.editor);
    let state = h.editor.state().clone();
    for c in cues::cues(&state, id) {
        for &(s, b) in &c.targets {
            h.ui.seq_banks.insert(s, (b, Some(b)));
        }
    }
    let words = h.states();
    assert_eq!(words, ["Ready", "Queued", "Ready", "Ready"]);
}
