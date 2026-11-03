//! The drawer's G4 sequencer editor through real egui input: the step grid writes the edited
//! bank's params (gate tap, ratchet tap, pitch drag), each edit is one undo step, and the
//! editor's controls fit the drawer without overlap at 1440x900 and 1180x680.

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect};
use kabl_core::{ModuleId, Vec2};
use kabl_ui::{show, PatchEditor, UiState};

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    size: egui::Vec2,
    events: Vec<Event>,
    t: f64,
    pointer: Pos2,
    seq: ModuleId,
}

impl H {
    fn new(size: egui::Vec2) -> Self {
        let mut editor = PatchEditor::new();
        let seq = editor.add_module("seq", Vec2 { x: 24.0, y: 20.0 });
        let mut h = H {
            ctx: egui::Context::default(),
            editor,
            ui: UiState::default(),
            size,
            events: Vec::new(),
            t: 0.0,
            pointer: Pos2::ZERO,
            seq,
        };
        h.settle();
        // A compact window starts with the drawer shut: open it, as the user does.
        h.ui.drawer_open = true;
        h.ui.selected_module = Some(seq);
        h.settle();
        h
    }

    fn frame(&mut self) {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            events: std::mem::take(&mut self.events),
            modifiers: Modifiers::NONE,
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

    fn settle(&mut self) {
        for _ in 0..4 {
            self.frame();
        }
    }

    fn rect(&self, key: &str) -> Rect {
        *self
            .ui
            .hits
            .get(key)
            .unwrap_or_else(|| panic!("no hit target {key}"))
    }

    fn move_to(&mut self, p: Pos2) {
        self.pointer = p;
        self.events.push(Event::PointerMoved(p));
        self.frame();
    }

    fn button(&mut self, pressed: bool) {
        self.events.push(Event::PointerButton {
            pos: self.pointer,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
        self.frame();
    }

    fn click_at(&mut self, p: Pos2) {
        self.move_to(p);
        self.button(true);
        self.button(false);
        self.settle();
    }

    fn param(&self, name: &str) -> Option<f32> {
        self.editor.state().modules[&self.seq]
            .params
            .get(name)
            .copied()
    }

    /// Centre of step `i` (0-based) in the lane whose top is `top` px below the grid and
    /// whose height is `h` (the grid's layout: 56 px labels, a 22 px number row, 8 px gaps).
    fn cell(&self, i: usize, top: f32, h: f32) -> Pos2 {
        let g = self.rect(&format!("seq-grid:{}", self.seq));
        let w = (g.width() - 56.0) / 8.0;
        Pos2::new(
            g.left() + 56.0 + w * (i as f32 + 0.5),
            g.top() + top + h / 2.0,
        )
    }
}

// Lane tops below the grid's top edge: numbers 22, pitch 86, gate 20, vel 36, prob 36, ratchet 20.
const PITCH: f32 = 22.0;
const GATE: f32 = PITCH + 86.0 + 8.0;
const RATCHET: f32 = GATE + 20.0 + 8.0 + 36.0 + 8.0 + 36.0 + 8.0;

#[test]
fn the_step_grid_writes_the_edited_bank_one_undo_step_per_edit() {
    let mut h = H::new(egui::vec2(1440.0, 900.0));
    // Gate 3 is on by default: a tap turns it off.
    let p = h.cell(2, GATE, 20.0);
    h.click_at(p);
    assert_eq!(h.param("g3"), Some(0.0));
    // Ratchet 1 steps from 1x to 2x.
    let p = h.cell(0, RATCHET, 20.0);
    h.click_at(p);
    assert_eq!(h.param("k1"), Some(2.0));
    // A press near the top of pitch 1 sets it high, in whole semitones.
    let p = h.cell(0, PITCH, 86.0) - egui::vec2(0.0, 30.0);
    h.click_at(p);
    let pitch = h.param("p1").unwrap();
    assert!(pitch > 6.0 && pitch.fract() == 0.0, "p1 = {pitch}");
    // Three edits, three undo steps.
    assert!(h.editor.undo());
    assert!(h.editor.undo());
    assert!(h.editor.undo());
    assert_eq!(h.param("g3"), None);
    // Editing bank B writes bank B's params.
    let b = h.rect(&format!("drawer-bank:{}.1", h.seq)).center();
    h.click_at(b);
    let p = h.cell(2, GATE, 20.0);
    h.click_at(p);
    assert_eq!(h.param("b.g3"), Some(0.0));
    assert_eq!(h.param("g3"), None);
}

#[test]
fn the_seq_editor_fits_the_drawer_without_overlap_at_both_sizes() {
    for size in [egui::vec2(1440.0, 900.0), egui::vec2(1180.0, 680.0)] {
        let h = H::new(size);
        let screen = Rect::from_min_size(Pos2::ZERO, size);
        let id = h.seq;
        let leaves: Vec<(&String, &Rect)> =
            h.ui.hits
                .iter()
                .filter(|(k, _)| {
                    (k.starts_with("seq-") || k.starts_with("drawer-bank:"))
                        && k.ends_with(&id.to_string())
                        || k.starts_with(&format!("drawer-bank:{id}."))
                        || k.starts_with(&format!("seq-direction:{id}."))
                        || k.starts_with(&format!("seq-gate_mode:{id}."))
                })
                .collect();
        assert!(leaves.len() > 8, "{size:?}: seq editor not drawn");
        // The drawer: 320 px left of the 44 px rail.
        let drawer = Rect::from_min_max(
            Pos2::new(size.x - 44.0 - 320.0, 0.0),
            Pos2::new(size.x - 44.0, size.y),
        );
        for (i, (ka, a)) in leaves.iter().enumerate() {
            assert!(
                screen.contains_rect(**a),
                "{size:?}: {ka} {a:?} leaves the window"
            );
            assert!(
                drawer.contains_rect(**a),
                "{size:?}: {ka} {a:?} leaves the drawer"
            );
            for (kb, b) in &leaves[i + 1..] {
                let inter = a.intersect(**b);
                assert!(
                    inter.width() <= 0.5 || inter.height() <= 0.5,
                    "{size:?}: {ka} {a:?} overlaps {kb} {b:?}"
                );
            }
        }
    }
}

/// Every text pair the editor draws reads at 4.5:1 in both default themes.
#[test]
fn the_seq_editor_text_reads_at_4_5_to_1() {
    for dark in [true, false] {
        let st = kabl_ui::style::line_solid_default(dark);
        let r = &st.roles;
        let well = if dark { r.inset } else { r.raised };
        for (name, fg, bg) in [
            ("text on surface", r.text, r.surface),
            ("text2 on surface", r.text2, r.surface),
            ("text2 on well", r.text2, well),
            ("accent_text on surface", r.accent_text, r.surface),
        ] {
            let c = kabl_ui::style::contrast(fg, bg);
            assert!(c >= 4.5, "dark={dark}: {name} {c:.2}:1");
        }
    }
}
