//! Composite face sliders: dragging sets the leaf param through its taper as one undo step per
//! gesture in both variants, the route badge shows, and the variant never reaches the patch.
use egui::{Event, PointerButton, Pos2, RawInput, Rect};
use kabl_core::{
    panel::{Kind, Panel, Placement},
    CompositeId, PortRef,
};
use kabl_ui::{composites, show, CableView, PatchEditor, UiState};

/// The default patch's filter.
const FILTER: u64 = 3;

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    t: f64,
    events: Vec<Event>,
    id: CompositeId,
    /// Every string painted by the last frame.
    texts: Vec<String>,
}

fn texts(shape: &egui::Shape, out: &mut Vec<String>) {
    match shape {
        egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
        egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
        _ => {}
    }
}

impl H {
    /// The filter as a composite whose face is one slider on its cutoff.
    fn new(vertical: bool) -> Self {
        let mut e = PatchEditor::seed_from(&kabl_standalone::default_patch());
        let id =
            composites::encapsulate(&mut e, [FILTER].into(), Default::default(), "Voice").unwrap();
        let target = PortRef::Param {
            id: FILTER,
            param: "cutoff_hz".into(),
        };
        composites::add_exposure(&mut e, id, target, true).unwrap();
        let mut c = e.state().composites[&id].clone();
        let key = *c.controls.keys().next().unwrap();
        let mut panel = Panel::default();
        panel.placements.insert(
            key,
            Placement {
                kind: Kind::Slider,
                x: 12.0,
                y: 72.0,
                width: 120.0,
                height: 168.0,
                source: None,
                label: None,
                icons: Vec::new(),
            },
        );
        c.panel = Some(Box::new(panel));
        composites::set(&mut e, id, c).unwrap();
        let mut ui = UiState::default();
        ui.vertical_sliders = vertical;
        let mut h = H {
            ctx: egui::Context::default(),
            editor: e,
            ui,
            t: 0.0,
            events: Vec::new(),
            id,
            texts: Vec::new(),
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
            self.texts.clear();
            for s in self.ctx.end_pass().shapes {
                texts(&s.shape, &mut self.texts);
            }
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

    /// Presses at `from` and releases at `to`, moving in steps.
    fn drag(&mut self, from: Pos2, to: Pos2) {
        self.events.push(Event::PointerMoved(from));
        self.frames(1);
        self.button(from, true);
        for i in 1..=8 {
            self.events
                .push(Event::PointerMoved(from + (to - from) * (i as f32 / 8.0)));
            self.frames(1);
        }
        self.button(to, false);
        self.frames(2);
    }

    /// The slider's track: where the handle's centre travels, from the minimum to the maximum.
    fn track(&self) -> Rect {
        *self
            .ui
            .hits
            .get(&format!("slider:{FILTER}.cutoff_hz"))
            .expect("the face slider records its track")
    }

    /// The track point at normalized position `t`.
    fn at(&self, t: f32) -> Pos2 {
        let r = self.track();
        if self.ui.vertical_sliders {
            egui::pos2(r.center().x, r.bottom() - r.height() * t)
        } else {
            egui::pos2(r.left() + r.width() * t, r.center().y)
        }
    }

    fn cutoff(&self) -> f32 {
        self.editor.state().modules[&FILTER].params["cutoff_hz"]
    }

    fn undo_depth(&self) -> usize {
        self.editor.log().entries().len()
    }

    fn saved(&self, dir: &std::path::Path, name: &str) -> Vec<u8> {
        let path = kabl_ui::library::save_patch(&dir.join(name), self.editor.log()).unwrap();
        std::fs::read(path).unwrap()
    }
}

fn info() -> &'static kabl_modules::ParamInfo {
    kabl_modules::registry::info_for("filter.svf")
        .unwrap()
        .params
        .iter()
        .find(|p| p.name == "cutoff_hz")
        .unwrap()
}

/// Drags a quarter of the way, then past each end, checking the value and the undo log after
/// each gesture; returns the page and the cutoff before the drags.
fn drag_through_the_taper(vertical: bool) -> (H, f32) {
    let mut h = H::new(vertical);
    let p = info();
    let before = h.cutoff();
    let depth = h.undo_depth();
    h.drag(h.at(0.5), h.at(0.25));
    let mid = h.cutoff();
    let want = p.from_norm(0.25);
    assert!(
        (mid - want).abs() <= want * 1e-3,
        "vertical {vertical}: {mid} is not the taper's value {want} at a quarter"
    );
    assert!(
        (want - (p.min + (p.max - p.min) * 0.25)).abs() > 1.0,
        "the taper is not linear"
    );
    assert_eq!(h.undo_depth(), depth + 1, "vertical {vertical}: one step");
    // Past either end clamps at the param's range.
    let past = h.track().size() * 2.0;
    h.drag(h.at(0.5), h.at(1.0) + egui::vec2(past.x, -past.y));
    assert_eq!(h.cutoff(), p.from_norm(1.0), "vertical {vertical}");
    assert!((h.cutoff() - p.max).abs() <= p.max * 1e-4);
    h.drag(h.at(0.5), h.at(0.0) + egui::vec2(-past.x, past.y));
    assert_eq!(h.cutoff(), p.from_norm(0.0), "vertical {vertical}");
    assert!((h.cutoff() - p.min).abs() <= p.min * 1e-4);
    assert_eq!(
        h.undo_depth(),
        depth + 3,
        "vertical {vertical}: one step each"
    );
    (h, before)
}

#[test]
fn horizontal_slider_drags_through_the_taper_one_undo_step_per_gesture() {
    let (mut h, before) = drag_through_the_taper(false);
    for _ in 0..3 {
        h.editor.undo();
    }
    assert_eq!(h.cutoff(), before);
}

#[test]
fn vertical_slider_drags_through_the_taper_one_undo_step_per_gesture() {
    let (mut h, before) = drag_through_the_taper(true);
    for _ in 0..3 {
        h.editor.undo();
    }
    assert_eq!(h.cutoff(), before);
}

#[test]
fn a_modulated_slider_shows_the_route_badge_in_both_variants() {
    for vertical in [false, true] {
        let mut h = H::new(vertical);
        h.ui.cable_view = CableView::Hidden;
        h.frames(2);
        assert!(!h.texts.iter().any(|t| t == "< 1 mod"));
        h.editor.connect_route(
            PortRef::Module {
                id: 4,
                port: "out".into(),
            },
            FILTER,
            "cutoff_hz",
        );
        h.frames(2);
        assert!(
            h.texts.iter().any(|t| t == "< 1 mod"),
            "vertical {vertical}: no route badge"
        );
    }
}

#[test]
fn the_slider_variant_is_a_view_setting_never_saved() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = H::new(false);
    let depth = h.undo_depth();
    let key = *h.editor.state().composites[&h.id]
        .controls
        .keys()
        .next()
        .unwrap();
    let hit = format!("composite:{}:control:{key}", h.id);
    let face = h.ui.hits[&hit];
    let track = h.track();
    let horizontal = h.saved(dir.path(), "a");
    h.ui.vertical_sliders = true;
    h.frames(2);
    assert_eq!(h.ui.hits[&hit], face, "same footprint");
    assert!(
        h.track().width() < track.width(),
        "redrawn as the vertical fader"
    );
    assert_eq!(h.undo_depth(), depth, "switching makes no undo entry");
    assert_eq!(horizontal, h.saved(dir.path(), "b"));
}
