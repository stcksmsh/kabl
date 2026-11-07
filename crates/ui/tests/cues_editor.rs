//! The drawer's G4 cues editor through real egui input: Launch sets every cue's timing, a
//! card opens its settings (bank per sequencer, delete), "Add cue from what plays" adds one,
//! and the editor's controls fit the drawer without overlap at 1440x900 and 1180x680.

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect};
use kabl_core::{ModuleId, Vec2};
use kabl_engine::patch_engine::Timing;
use kabl_ui::{cues, show, PatchEditor, UiState};

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    size: egui::Vec2,
    events: Vec<Event>,
    t: f64,
    pointer: Pos2,
    seq: ModuleId,
    cues: ModuleId,
}

impl H {
    fn new(size: egui::Vec2) -> Self {
        let mut editor = PatchEditor::new();
        let seq = editor.add_module("seq", Vec2 { x: 24.0, y: 20.0 });
        let cues = editor.add_module("cues", Vec2 { x: 24.0, y: 320.0 });
        cues::add(&mut editor, cues, &|_| 0);
        cues::add(&mut editor, cues, &|_| 1);
        let mut h = H {
            ctx: egui::Context::default(),
            editor,
            ui: UiState::default(),
            size,
            events: Vec::new(),
            t: 0.0,
            pointer: Pos2::ZERO,
            seq,
            cues,
        };
        h.settle();
        // A compact window starts with the drawer shut: open it, as the user does.
        h.ui.drawer_open = true;
        h.ui.selected_module = Some(cues);
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

    fn click(&mut self, key: &str) {
        let p = self
            .ui
            .hits
            .get(key)
            .unwrap_or_else(|| panic!("no hit target {key}"))
            .center();
        self.pointer = p;
        self.events.push(Event::PointerMoved(p));
        self.frame();
        for pressed in [true, false] {
            self.events.push(Event::PointerButton {
                pos: self.pointer,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
            self.frame();
        }
        self.settle();
    }

    /// Presses on `from`, moves in steps to `to`'s centre, releases.
    fn drag(&mut self, from: &str, to: &str) {
        let (a, b) = (self.ui.hits[from].center(), self.ui.hits[to].center());
        self.events.push(Event::PointerMoved(a));
        self.frame();
        self.events.push(Event::PointerButton {
            pos: a,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        });
        self.frame();
        for k in 1..=8 {
            self.events
                .push(Event::PointerMoved(a + (b - a) * (k as f32 / 8.0)));
            self.frame();
        }
        self.events.push(Event::PointerButton {
            pos: b,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        });
        self.frame();
        self.settle();
    }

    fn all(&self) -> Vec<cues::Cue> {
        cues::cues(self.editor.state(), self.cues)
    }
}

#[test]
fn launch_cards_and_add_edit_the_cues() {
    let mut h = H::new(egui::vec2(1440.0, 900.0));
    let (id, seq) = (h.cues, h.seq);
    // Launch on the next step, for every cue.
    h.click(&format!("cue-launch:{id}.1"));
    assert!(h.all().iter().all(|c| c.timing == Timing::NextStep));
    // Settings stay closed until the card is tapped.
    assert!(!h.ui.hits.contains_key(&format!("cue-bank:{id}.1.{seq}.2")));
    h.click(&format!("cue-card:{id}.1"));
    h.click(&format!("cue-bank:{id}.1.{seq}.2"));
    assert_eq!(h.all()[0].targets, vec![(seq, 2)]);
    h.click(&format!("cue-bank:{id}.1.{seq}.keep"));
    assert!(h.all()[0].targets.is_empty());
    // Add from what plays, then delete the first.
    h.click(&format!("cue-add:{id}"));
    assert_eq!(h.all().len(), 3);
    h.click(&format!("cue-delete:{id}.1"));
    assert_eq!(h.all().iter().map(|c| c.n).collect::<Vec<_>>(), vec![2, 3]);
}

#[test]
fn the_cues_editor_fits_the_drawer_without_overlap_at_both_sizes() {
    for size in [egui::vec2(1440.0, 900.0), egui::vec2(1180.0, 680.0)] {
        let mut h = H::new(size);
        let id = h.cues;
        h.click(&format!("cue-card:{id}.1"));
        let screen = Rect::from_min_size(Pos2::ZERO, size);
        // The drawer: 320 px left of the 44 px rail.
        let drawer = Rect::from_min_max(
            Pos2::new(size.x - 44.0 - 320.0, 0.0),
            Pos2::new(size.x - 44.0, size.y),
        );
        let prefix = |k: &str| {
            [
                "cue-launch:",
                "cue-name:",
                "cue-capture:",
                "cue-delete:",
                "cue-clock:",
            ]
            .iter()
            .chain(&["cue-timing:", "cue-bank:", "cue-add:", "cue-card:"])
            .any(|p| k.starts_with(&format!("{p}{id}")))
        };
        let leaves: Vec<(&String, &Rect)> = h.ui.hits.iter().filter(|(k, _)| prefix(k)).collect();
        assert!(leaves.len() > 10, "{size:?}: cues editor not drawn");
        for (i, (ka, a)) in leaves.iter().enumerate() {
            // The drawer scrolls: only what is on screen has to fit.
            if a.bottom() > size.y {
                continue;
            }
            assert!(
                screen.contains_rect(**a),
                "{size:?}: {ka} {a:?} leaves the window"
            );
            assert!(
                drawer.contains_rect(**a),
                "{size:?}: {ka} {a:?} leaves the drawer"
            );
            for (kb, b) in &leaves[i + 1..] {
                // A card's tap area holds its own row of text, not controls.
                if ka.starts_with("cue-card:") || kb.starts_with("cue-card:") {
                    continue;
                }
                let inter = a.intersect(**b);
                assert!(
                    inter.width() <= 0.5 || inter.height() <= 0.5,
                    "{size:?}: {ka} {a:?} overlaps {kb} {b:?}"
                );
            }
        }
    }
}

/// The chips and text the editor draws read at 4.5:1 in both default themes.
#[test]
fn the_cues_editor_text_reads_at_4_5_to_1() {
    for dark in [true, false] {
        let st = kabl_ui::style::line_solid_default(dark);
        let r = &st.roles;
        for (name, fg, bg) in [
            ("text on surface", r.text, r.surface),
            ("text2 on surface", r.text2, r.surface),
            ("text2 on raised", r.text2, r.raised),
            ("on_accent on accent", r.on_accent, r.accent),
            ("accent_text on surface", r.accent_text, r.surface),
        ] {
            let c = kabl_ui::style::contrast(fg, bg);
            assert!(c >= 4.5, "dark={dark}: {name} {c:.2}:1");
        }
    }
}

#[test]
fn a_cards_grip_drags_it_to_another_place() {
    let mut h = H::new(egui::vec2(1440.0, 900.0));
    let id = h.cues;
    h.click(&format!("cue-add:{id}"));
    for (n, name) in [(1, "Intro"), (2, "Drop"), (3, "Break")] {
        cues::rename(&mut h.editor, id, n, name);
    }
    h.settle();
    let before = h.editor.log().entries().len();
    h.drag(&format!("cue-grip:{id}.3"), &format!("cue-card:{id}.1"));
    let names: Vec<String> = h.all().into_iter().map(|c| c.name).collect();
    assert_eq!(names, ["Break", "Intro", "Drop"]);
    assert_eq!(h.editor.log().entries().len(), before + 1, "one undo step");
}
