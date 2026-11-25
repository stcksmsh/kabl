//! The composite design page: entry points, snapped drags, refused drops, draft history,
//! Apply, and the questions it asks before dropping a draft.
use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect};
use kabl_core::{
    panel::{Kind, Panel, Placement},
    CompositeId, PortRef,
};
use kabl_ui::{composites, design_page::Page, show, PatchEditor, UiState};

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    t: f64,
    events: Vec<Event>,
    /// The filter as a composite with no panel, and the oscillator as one with an authored face.
    plain: CompositeId,
    authored: CompositeId,
}

fn param(id: u64, param: &str) -> PortRef {
    PortRef::Param {
        id,
        param: param.into(),
    }
}

fn port(id: u64, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

impl H {
    fn new() -> Self {
        let mut e = PatchEditor::seed_from(&kabl_standalone::default_patch());
        let plain =
            composites::encapsulate(&mut e, [3].into(), Default::default(), "Voice").unwrap();
        for (target, control) in [
            (param(3, "cutoff_hz"), true),
            (port(3, "in"), false),
            (port(3, "lp"), false),
        ] {
            composites::add_exposure(&mut e, plain, target, control).unwrap();
        }
        let authored =
            composites::encapsulate(&mut e, [2].into(), Default::default(), "Osc").unwrap();
        composites::add_exposure(&mut e, authored, param(2, "base_hz"), true).unwrap();
        let mut c = e.state().composites[&authored].clone();
        let key = *c.controls.keys().next().unwrap();
        let mut panel = Panel::default();
        panel.placements.insert(
            key,
            Placement {
                kind: Kind::Knob,
                x: 12.0,
                y: 72.0,
                width: 84.0,
                height: 110.0,
                source: None,
                label: None,
                icons: Vec::new(),
            },
        );
        c.panel = Some(Box::new(panel));
        composites::set(&mut e, authored, c).unwrap();
        let mut h = H {
            ctx: egui::Context::default(),
            editor: e,
            ui: UiState::default(),
            t: 0.0,
            events: Vec::new(),
            plain,
            authored,
        };
        h.frames(3);
        h
    }

    fn frames(&mut self, n: usize) {
        let size = egui::vec2(1440.0, 900.0);
        for _ in 0..n {
            let raw = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                time: Some(self.t),
                events: std::mem::take(&mut self.events),
                ..Default::default()
            };
            self.t += 1.0 / 60.0;
            self.ctx.begin_pass(raw);
            let mut root = egui::Ui::new(
                self.ctx.clone(),
                egui::Id::new("root"),
                egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::ZERO, size)),
            );
            show(&mut self.editor, &mut self.ui, &mut root);
            // Nothing is painted: the tests read the page state and hit rects.
            let _ = self.ctx.end_pass();
        }
    }

    fn button(&mut self, p: Pos2, pressed: bool) {
        self.events.push(Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        });
        self.frames(1);
    }

    fn click(&mut self, key: &str) {
        let p = self.hit(key).center();
        self.events.push(Event::PointerMoved(p));
        self.frames(1);
        self.button(p, true);
        self.button(p, false);
        self.frames(2);
    }

    /// Presses at `from` and releases at `from + d` (screen points), moving in steps.
    fn drag(&mut self, from: Pos2, d: egui::Vec2) {
        self.events.push(Event::PointerMoved(from));
        self.frames(1);
        self.button(from, true);
        for i in 1..=8 {
            self.events
                .push(Event::PointerMoved(from + d * (i as f32 / 8.0)));
            self.frames(1);
        }
        self.button(from + d, false);
        self.frames(2);
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) {
        for pressed in [true, false] {
            self.events.push(Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            });
        }
        self.frames(2);
    }

    fn hit(&self, key: &str) -> Rect {
        *self
            .ui
            .hits
            .get(key)
            .unwrap_or_else(|| panic!("no hit rect {key}"))
    }

    fn page(&self) -> &Page {
        self.ui
            .panels
            .page
            .as_ref()
            .expect("the design page is open")
    }

    fn open(&mut self, id: CompositeId) {
        let c = self.editor.state().composites[&id].clone();
        kabl_ui::panels::open(&mut self.ui, id, &c);
        self.frames(2);
    }

    /// Exposure keys of `id`: controls, then ports.
    fn keys(&self, id: CompositeId) -> Vec<u64> {
        let c = &self.editor.state().composites[&id];
        c.controls.keys().chain(c.ports.keys()).copied().collect()
    }

    fn placed(&self, key: u64) -> [f32; 4] {
        let p = &self.page().draft.panel.placements[&key];
        [p.x, p.y, p.width, p.height]
    }

    /// Screen points per rack point on the canvas.
    fn zoom(&self, key: u64) -> f32 {
        self.hit(&format!("design:el:{key}")).width() / self.placed(key)[2]
    }

    /// The screen point of rack point `at`, measured from placed element `by`.
    fn screen(&self, by: u64, at: Pos2) -> Pos2 {
        let [x, y, ..] = self.placed(by);
        self.hit(&format!("design:el:{by}")).min + (at - egui::pos2(x, y)) * self.zoom(by)
    }

    /// Presses at `from` and moves to `to` in steps, the button still down.
    fn hold(&mut self, from: Pos2, to: Pos2) {
        self.events.push(Event::PointerMoved(from));
        self.frames(1);
        self.button(from, true);
        for i in 1..=8 {
            self.events
                .push(Event::PointerMoved(from + (to - from) * (i as f32 / 8.0)));
            self.frames(1);
        }
    }

    /// The plain composite's page with its last jack placed by a click; returns the knob and
    /// that jack.
    fn with_jack() -> (Self, u64, u64) {
        let mut h = H::new();
        h.open(h.plain);
        let keys = h.keys(h.plain);
        h.click(&format!("design:palette:{}", keys[2]));
        (h, keys[0], keys[2])
    }
}

#[test]
fn every_edit_face_entry_opens_the_page_for_its_composite() {
    let mut h = H::new();
    for id in [h.plain, h.authored] {
        h.click(&format!("composite:{id}:design"));
        assert_eq!(h.page().id, id, "rack face of #{id}");
        h.click("design:close");
        assert!(h.ui.panels.page.is_none());
    }
    h.ui.composites.panel = true;
    h.frames(2);
    for id in [h.plain, h.authored] {
        h.click(&format!("composite:{id}:tools"));
        // Past the collapsing animation.
        h.frames(20);
        h.click(&format!("composite:{id}:list-design"));
        assert_eq!(h.page().id, id, "composite list entry of #{id}");
        h.click("design:close");
        assert!(h.ui.panels.page.is_none());
        // Collapsed again, so the next entry is inside the window.
        h.click(&format!("composite:{id}:tools"));
        h.frames(20);
    }
}

#[test]
fn a_composite_with_no_panel_opens_on_the_default_face() {
    let mut h = H::new();
    h.open(h.plain);
    assert_eq!(h.page().draft.panel, Panel::default());
    assert!(!h.page().dirty());
}

#[test]
fn drags_move_and_resize_on_the_six_point_grid() {
    let mut h = H::new();
    h.open(h.plain);
    let key = h.keys(h.plain)[0];
    h.click(&format!("design:palette:{key}"));
    let [x, y, w, height] = h.placed(key);
    assert_eq!((x, y), (12.0, 66.0));
    let z = h.zoom(key);
    // 13 and 7 rack points snap to 12 and 6.
    let from = h.hit(&format!("design:el:{key}")).center();
    h.drag(from, egui::vec2(13.0, 7.0) * z);
    assert_eq!(h.placed(key), [24.0, 72.0, w, height]);
    // The bottom-right corner, 10 and 5 points out: the right edge snaps from 118 to 120, the
    // bottom edge from 187 to 186.
    let corner = h.hit(&format!("design:corner:{key}:se")).center();
    h.drag(corner, egui::vec2(10.0, 5.0) * z);
    assert_eq!(h.placed(key), [24.0, 72.0, 96.0, 114.0]);
}

#[test]
fn an_overlapping_or_off_canvas_drop_is_refused_and_the_draft_kept() {
    let mut h = H::new();
    h.open(h.plain);
    let keys = h.keys(h.plain);
    for &k in &keys {
        h.click(&format!("design:palette:{k}"));
    }
    let (knob, jack) = (keys[0], keys[2]);
    let before = h.page().draft.clone();
    let z = h.zoom(jack);
    let [kx, ky, ..] = h.placed(knob);
    let [jx, jy, ..] = h.placed(jack);
    // Onto the knob.
    let from = h.hit(&format!("design:el:{jack}")).center();
    h.drag(from, egui::vec2(kx + 12.0 - jx, ky + 12.0 - jy) * z);
    assert_eq!(h.page().draft, before);
    assert_eq!(h.page().refused.map(|(k, _)| k), Some(jack));
    // Off the face to the left.
    let from = h.hit(&format!("design:el:{jack}")).center();
    h.drag(from, egui::vec2(-jx - 60.0, 0.0) * z);
    assert_eq!(h.page().draft, before);
    let (k, [x, ..]) = h.page().refused.unwrap();
    assert_eq!(k, jack);
    assert!(x < 8.0);
}

#[test]
fn a_palette_drag_places_the_element_snapped_where_it_drops() {
    let (mut h, knob, jack) = H::with_jack();
    let from = h.hit(&format!("design:palette:{knob}")).center();
    // The 84 x 110 knob is held by its middle: released over rack point (220.7, 201.9), its
    // top-left is (178.7, 146.9), which snaps to (180, 144).
    let to = h.screen(jack, egui::pos2(220.7, 201.9));
    h.drag(from, to - from);
    assert_eq!(h.placed(knob), [180.0, 144.0, 84.0, 110.0]);
    assert_eq!(h.page().selected, Some(knob));
    assert!(h.page().refused.is_none());
}

#[test]
fn a_palette_drop_overlapping_or_off_the_face_is_refused_and_the_draft_kept() {
    let (mut h, knob, jack) = H::with_jack();
    let before = h.page().draft.clone();
    let from = h.hit(&format!("design:palette:{knob}")).center();
    // Onto the jack at (12, 66).
    let to = h.screen(jack, egui::pos2(60.0, 130.0));
    h.drag(from, to - from);
    assert_eq!(h.page().draft, before);
    assert_eq!(h.page().refused.map(|(k, _)| k), Some(knob));
    // Off the face to the left, still over the canvas.
    let to = h.screen(jack, egui::pos2(-20.0, 200.0));
    h.drag(from, to - from);
    assert_eq!(h.page().draft, before);
    let (k, [x, ..]) = h.page().refused.unwrap();
    assert_eq!(k, knob);
    assert!(x < 0.0);
}

#[test]
fn esc_during_a_palette_drag_cancels_it() {
    let (mut h, knob, jack) = H::with_jack();
    let before = h.page().draft.clone();
    let from = h.hit(&format!("design:palette:{knob}")).center();
    let to = h.screen(jack, egui::pos2(220.0, 200.0));
    h.hold(from, to);
    h.key(Key::Escape, Modifiers::NONE);
    h.button(to, false);
    h.frames(2);
    assert_eq!(h.page().draft, before);
    assert!(h.page().refused.is_none());
    // The Esc went to the drag, not to the page.
    assert!(h.page().ask.is_none());
}

#[test]
fn a_palette_drag_released_over_the_palette_cancels_it() {
    let (mut h, knob, jack) = H::with_jack();
    let before = h.page().draft.clone();
    let from = h.hit(&format!("design:palette:{knob}")).center();
    let out = h.screen(jack, egui::pos2(220.0, 200.0));
    h.hold(from, out);
    let back = from + egui::vec2(20.0, 4.0);
    for i in 1..=8 {
        h.events
            .push(Event::PointerMoved(out + (back - out) * (i as f32 / 8.0)));
        h.frames(1);
    }
    h.button(back, false);
    h.frames(2);
    assert_eq!(h.page().draft, before);
    assert!(h.page().refused.is_none());
}

#[test]
fn a_placed_palette_item_does_not_drag() {
    let (mut h, _, jack) = H::with_jack();
    let before = h.page().draft.clone();
    let from = h.hit(&format!("design:palette:{jack}")).center();
    let to = h.screen(jack, egui::pos2(220.0, 200.0));
    h.drag(from, to - from);
    assert_eq!(h.page().draft, before);
}

#[test]
fn a_palette_drop_is_one_page_undo_step() {
    let (mut h, knob, jack) = H::with_jack();
    let before = h.page().draft.clone();
    let from = h.hit(&format!("design:palette:{knob}")).center();
    let to = h.screen(jack, egui::pos2(220.0, 200.0));
    h.drag(from, to - from);
    assert_eq!(h.page().draft.panel.placements.len(), 2);
    h.click("design:undo");
    assert_eq!(h.page().draft, before);
    // The step before it is the jack's click-add.
    h.click("design:undo");
    assert_eq!(h.page().draft.panel, Panel::default());
}

#[test]
fn page_undo_walks_the_draft_and_apply_is_one_project_undo_step() {
    let mut h = H::new();
    h.open(h.plain);
    let keys = h.keys(h.plain);
    h.click(&format!("design:palette:{}", keys[0]));
    h.click(&format!("design:palette:{}", keys[1]));
    assert_eq!(h.page().draft.panel.placements.len(), 2);
    h.click("design:undo");
    assert_eq!(h.page().draft.panel.placements.len(), 1);
    h.key(Key::Z, Modifiers::COMMAND);
    assert_eq!(h.page().draft.panel, Panel::default());
    h.click("design:redo");
    h.key(Key::Y, Modifiers::COMMAND);
    assert_eq!(h.page().draft.panel.placements.len(), 2);
    // Page undo leaves the project alone.
    assert!(h.editor.state().composites[&h.plain].panel.is_none());

    let before = h.editor.state().clone();
    h.click("design:apply");
    let applied = h.editor.state().composites[&h.plain].panel.clone().unwrap();
    assert_eq!(applied.placements.len(), 2);
    assert_eq!(h.page().draft.base, h.editor.state().composites[&h.plain]);
    assert!(h.editor.undo());
    assert_eq!(*h.editor.state(), before);
}

#[test]
fn apply_replaces_only_the_panel_snapshots_and_next_interface() {
    let mut h = H::new();
    h.open(h.plain);
    let key = h.keys(h.plain)[0];
    h.click(&format!("design:palette:{key}"));
    let draft = h.page().draft.clone();
    let before = h.editor.state().clone();
    h.click("design:apply");
    let after = h.editor.state().clone();
    assert_eq!(
        after.composites[&h.plain].panel.as_deref(),
        Some(&draft.panel)
    );
    // Every other field, and everything else in the project, is as it was.
    let mut restored = after.clone();
    let c = restored.composites.get_mut(&h.plain).unwrap();
    let b = &before.composites[&h.plain];
    c.panel = b.panel.clone();
    c.snapshots = b.snapshots.clone();
    c.next_interface = b.next_interface;
    assert_eq!(restored, before);
}

#[test]
fn draft_history_keeps_each_image_once() {
    let mut h = H::new();
    h.open(h.plain);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("face.png");
    image::RgbaImage::from_pixel(32, 32, image::Rgba([120, 140, 110, 255]))
        .save(&path)
        .unwrap();
    h.ui.panels.path = path.to_string_lossy().into_owned();
    h.frames(1);
    h.click("design:import-light");
    h.click("design:import-dark");
    assert_eq!(h.page().pool_len(), 1);
    // The same PNG again, under both slots and after a clear, adds nothing to the pool.
    h.click("design:clear-art");
    h.click("design:import-light");
    h.click("design:import-dark");
    assert!(h.page().draft.panel.dark.is_some());
    assert_eq!(h.page().pool_len(), 1);
    h.click("design:undo");
    h.click("design:undo");
    h.click("design:undo");
    assert!(h.page().draft.panel.light.is_some() && h.page().draft.panel.dark.is_some());
}

#[test]
fn closing_asks_only_when_the_draft_is_unapplied() {
    let mut h = H::new();
    h.open(h.plain);
    h.click("design:close");
    assert!(h.ui.panels.page.is_none(), "no changes: no question");

    h.open(h.plain);
    let key = h.keys(h.plain)[0];
    h.click(&format!("design:palette:{key}"));
    h.click("design:close");
    assert_eq!(h.page().ask, Some(kabl_ui::design_page::Ask::Close));
    h.click("design:ask:keep");
    assert!(h.page().ask.is_none());
    h.click("design:close");
    h.click("design:ask:discard");
    assert!(h.ui.panels.page.is_none());
    assert!(h.editor.state().composites[&h.plain].panel.is_none());
}

#[test]
fn a_composite_changed_under_the_page_blocks_apply_until_reload() {
    let mut h = H::new();
    h.open(h.plain);
    let key = h.keys(h.plain)[0];
    h.click(&format!("design:palette:{key}"));
    let mut c = h.editor.state().composites[&h.plain].clone();
    c.name = "Renamed".into();
    composites::set(&mut h.editor, h.plain, c).unwrap();
    h.frames(2);
    h.hit("design:changed");
    h.click("design:apply");
    assert!(h.editor.state().composites[&h.plain].panel.is_none());
    h.click("design:reload");
    assert_eq!(h.page().ask, Some(kabl_ui::design_page::Ask::Reload));
    h.click("design:ask:discard");
    assert_eq!(h.page().draft.base.name, "Renamed");
    assert!(!h.page().dirty());
    assert!(!h.ui.hits.contains_key("design:changed"));
}

#[test]
fn a_composite_removed_under_the_page_keeps_the_draft_and_blocks_apply() {
    let mut h = H::new();
    h.open(h.authored);
    h.click("design:width-up");
    let draft = h.page().draft.clone();
    assert!(h.page().dirty());
    // Project undo (the toolbar's Undo stays live) back past the encapsulation.
    while h.editor.state().composites.contains_key(&h.authored) {
        assert!(h.editor.undo());
    }
    h.frames(2);
    assert_eq!(h.page().draft, draft);
    h.hit("design:changed");
    h.click("design:apply");
    assert_eq!(h.page().draft, draft);
    assert!(!h.editor.state().composites.contains_key(&h.authored));
    h.click("design:close");
    assert_eq!(h.page().ask, Some(kabl_ui::design_page::Ask::Close));
    h.click("design:ask:keep");
    h.click("design:reload");
    assert_eq!(h.page().ask, Some(kabl_ui::design_page::Ask::Reload));
    h.click("design:ask:discard");
    assert!(h.ui.panels.page.is_none());
}
