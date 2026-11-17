//! Motion through real egui frames: the drawer, the view fade, the camera glide and the toasts
//! each reach their target and then ask for no repaints; a retargeted glide stays continuous.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use egui::{Event, Pos2, RawInput, Rect};
use kabl_core::Vec2;
use kabl_ui::{show, PatchEditor, UiState};

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    t: f64,
    delays: Arc<Mutex<Vec<Duration>>>,
}

impl H {
    fn new() -> Self {
        let mut editor = PatchEditor::new();
        editor.add_module("noise", Vec2 { x: 24.0, y: 20.0 });
        editor.add_module("out", Vec2 { x: 400.0, y: 20.0 });
        let ctx = egui::Context::default();
        let delays = Arc::new(Mutex::new(Vec::new()));
        let d = delays.clone();
        ctx.set_request_repaint_callback(move |i| d.lock().unwrap().push(i.delay));
        let mut h = H {
            ctx,
            editor,
            ui: UiState::default(),
            t: 0.0,
            delays,
        };
        h.settle();
        h
    }

    fn frame_with(&mut self, events: Vec<Event>) {
        let size = egui::vec2(1440.0, 900.0);
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events,
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
        let _ = self.ctx.end_pass();
    }

    fn frame(&mut self) {
        self.frame_with(vec![]);
    }

    /// Four seconds of frames: longer than any transition or toast lifetime.
    fn settle(&mut self) {
        for _ in 0..240 {
            self.frame();
        }
    }

    /// Shortest repaint delay requested over the next `n` frames.
    fn shortest_repaint(&mut self, n: usize) -> Option<Duration> {
        self.delays.lock().unwrap().clear();
        for _ in 0..n {
            self.frame();
        }
        let d = self.delays.lock().unwrap().iter().min().copied();
        d
    }

    fn idle(&mut self, what: &str) {
        let d = self.shortest_repaint(60);
        assert!(
            d.is_none_or(|d| d >= Duration::from_millis(60)),
            "{what}: idle after settling, got {d:?} (causes {:?})",
            self.ctx.repaint_causes()
        );
    }

    fn moving(&mut self, what: &str) {
        let d = self.shortest_repaint(3);
        assert!(
            d.is_some_and(|d| d < Duration::from_millis(40)),
            "{what}: repaints while moving, got {d:?}"
        );
    }
}

#[test]
fn a_resting_rack_is_idle() {
    let mut h = H::new();
    h.idle("rest");
}

#[test]
fn the_drawer_slides_then_stops_repainting() {
    let mut h = H::new();
    let open = h.ui.drawer_open;
    h.ui.drawer_open = !open;
    h.moving("drawer toggled");
    h.settle();
    h.idle("drawer");
    h.ui.drawer_open = open;
    h.moving("drawer toggled back");
    h.settle();
    h.idle("drawer back");
}

#[test]
fn the_view_fades_in_then_stops_repainting() {
    let mut h = H::new();
    h.ui.browser_open = true;
    h.moving("Sounds");
    h.settle();
    h.idle("Sounds");
    h.ui.browser_open = false;
    h.moving("Rack");
    h.settle();
    h.idle("Rack");
}

fn zoom_event(h: &mut H, f: f32) {
    let at = Pos2::new(700.0, 450.0);
    h.frame_with(vec![Event::PointerMoved(at)]);
    h.frame_with(vec![Event::Zoom(f)]);
    // The glide steps at the top of the next frame.
    h.frame();
}

#[test]
fn zoom_glides_to_its_target_then_idles() {
    let mut h = H::new();
    let z0 = h.ui.zoom;
    zoom_event(&mut h, 1.2);
    // Drawn zoom is part-way: not snapped, not still.
    assert!(
        h.ui.zoom > z0 && h.ui.zoom < z0 * 1.2 - 1e-3,
        "gliding, zoom {}",
        h.ui.zoom
    );
    h.moving("zoom");
    h.settle();
    assert!(
        (h.ui.zoom - z0 * 1.2).abs() < 1e-3,
        "arrived, {} vs {}",
        h.ui.zoom,
        z0 * 1.2
    );
    h.idle("zoom");
}

#[test]
fn retargeting_a_glide_keeps_the_camera_continuous() {
    let mut h = H::new();
    let z0 = h.ui.zoom;
    zoom_event(&mut h, 1.3);
    let before = h.ui.zoom;
    // Aim back down mid-glide: the drawn zoom carries on from where it is.
    h.frame_with(vec![Event::Zoom(1.0 / 1.6)]);
    let after = h.ui.zoom;
    assert!(
        (after - before).abs() < 0.08 * z0,
        "no jump: {before} -> {after}"
    );
    h.settle();
    assert!((h.ui.zoom - z0 * 1.3 / 1.6).abs() < 1e-3);
    h.idle("retargeted glide");
}

#[test]
fn a_toast_comes_and_goes_then_the_rack_idles() {
    let mut h = H::new();
    h.ui.last_message = Some("opened \"Evolving Pad\"".into());
    h.moving("toast in");
    assert_eq!(h.ui.toasts.len(), 1);
    h.settle(); // 4 s: past the 3 s lifetime of a success toast
    assert!(
        h.ui.toasts.is_empty(),
        "expired toast removed after its fade"
    );
    h.idle("toast");
}

#[test]
fn a_dismissed_toast_fades_before_it_goes() {
    let mut h = H::new();
    h.ui.last_message = Some("Audio device busy".into());
    for _ in 0..30 {
        h.frame();
    }
    h.frame_with(vec![Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert_eq!(h.ui.toasts.len(), 1, "still fading on the next frame");
    h.moving("toast out");
    for _ in 0..30 {
        h.frame();
    }
    assert!(h.ui.toasts.is_empty());
    h.idle("dismissed toast");
}
