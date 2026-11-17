//! M2: the cable settle spring, reduced motion and the system theme, through real egui frames.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use egui::{Event, Pos2, RawInput, Rect};
use kabl_core::{PortRef, Vec2};
use kabl_ui::desktop::{Hints, ReduceMotion, ThemeMode};
use kabl_ui::{show, PatchEditor, UiState};

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    t: f64,
    delays: Arc<Mutex<Vec<Duration>>>,
    noise: u64,
    out: u64,
}

impl H {
    fn new(reduce: ReduceMotion) -> Self {
        let mut editor = PatchEditor::new();
        let noise = editor.add_module("noise", Vec2 { x: 24.0, y: 20.0 });
        let out = editor.add_module("out", Vec2 { x: 400.0, y: 20.0 });
        let ctx = egui::Context::default();
        let delays = Arc::new(Mutex::new(Vec::new()));
        let d = delays.clone();
        ctx.set_request_repaint_callback(move |i| d.lock().unwrap().push(i.delay));
        let mut ui = UiState::default();
        ui.reduce_motion = reduce;
        let mut h = H {
            ctx,
            editor,
            ui,
            t: 0.0,
            delays,
            noise,
            out,
        };
        for _ in 0..240 {
            h.frame(vec![]);
        }
        h
    }

    fn frame(&mut self, events: Vec<Event>) {
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

    /// Shortest repaint delay over `n` frames.
    fn shortest(&mut self, n: usize) -> Option<Duration> {
        self.delays.lock().unwrap().clear();
        for _ in 0..n {
            self.frame(vec![]);
        }
        let d = self.delays.lock().unwrap().iter().min().copied();
        d
    }

    fn release(&mut self) {
        self.frame(vec![Event::PointerButton {
            pos: Pos2::new(700.0, 600.0),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
    }

    fn plug(&mut self) {
        self.release();
        self.editor.connect(
            PortRef::Module {
                id: self.noise,
                port: "out".into(),
            },
            PortRef::Module {
                id: self.out,
                port: "in".into(),
            },
        );
    }

    fn moving(&mut self, what: &str) {
        let d = self.shortest(3);
        assert!(
            d.is_some_and(|d| d < Duration::from_millis(40)),
            "{what}: expected frames while settling, got {d:?}"
        );
    }

    fn idle(&mut self, what: &str) {
        let d = self.shortest(60);
        assert!(
            d.is_none_or(|d| d >= Duration::from_millis(60)),
            "{what}: idle expected, got {d:?} (causes {:?})",
            self.ctx.repaint_causes()
        );
    }
}

#[test]
fn a_plugged_cable_settles_then_the_rack_is_idle() {
    let mut h = H::new(ReduceMotion::Off);
    h.idle("before");
    h.plug();
    h.moving("plugged");
    // Two seconds is far past the spring's sleep (about 1.2 s).
    for _ in 0..120 {
        h.frame(vec![]);
    }
    h.idle("after the settle");
}

#[test]
fn a_moved_module_settles_its_cables_then_sleeps() {
    let mut h = H::new(ReduceMotion::Off);
    h.plug();
    for _ in 0..180 {
        h.frame(vec![]);
    }
    h.idle("plugged");
    h.editor.move_module(h.out, Vec2 { x: 420.0, y: 60.0 });
    h.moving("module moved");
    for _ in 0..180 {
        h.frame(vec![]);
    }
    h.idle("moved");
}

#[test]
fn reduced_motion_has_no_settle_and_instant_slides() {
    let mut h = H::new(ReduceMotion::On);
    assert!(h.ui.reduced_motion());
    h.plug();
    h.frame(vec![]);
    h.idle("reduced: no spring after a plug");
    // The glow keeps working with a doubled release; the slides land at once.
    assert!(h.ui.life.slow_release && h.ui.pulses.slow_release);
    assert_eq!(h.ui.style.motion.drawer, kabl_ui::style::REDUCED_MS);
    assert_eq!(h.ui.style.motion.view, kabl_ui::style::REDUCED_MS);
    assert_eq!(h.ui.style.motion.glide, kabl_ui::style::REDUCED_MS);
    h.ui.drawer_open = !h.ui.drawer_open;
    for _ in 0..3 {
        h.frame(vec![]);
    }
    h.idle("reduced: drawer lands at once");
    // Back on: tokens return and cables settle again.
    h.ui.reduce_motion = ReduceMotion::Off;
    h.frame(vec![]);
    assert!(!h.ui.life.slow_release);
    assert!(h.ui.style.motion.drawer > 100.0);
}

#[test]
fn reduced_motion_follows_the_desktop_under_system_and_the_setting_overrides_it() {
    let mut h = H::new(ReduceMotion::System);
    assert!(!h.ui.reduced_motion());
    h.ui.desktop = Hints {
        reduce_motion: true,
        dark: None,
    };
    assert!(h.ui.reduced_motion());
    h.plug();
    h.frame(vec![]);
    h.idle("desktop asks: no spring");
    h.ui.reduce_motion = ReduceMotion::Off;
    assert!(!h.ui.reduced_motion());
}

#[test]
fn the_system_theme_follows_the_portal_and_falls_back_to_dark() {
    for (portal, system_dark) in [(None, true), (Some(true), true), (Some(false), false)] {
        let mut h = H::new(ReduceMotion::Off);
        h.ui.desktop = Hints {
            dark: portal,
            reduce_motion: false,
        };
        h.ui.set_theme_mode(ThemeMode::System);
        assert_eq!(h.ui.dark, system_dark, "System, portal {portal:?}");
        h.frame(vec![]);
        assert_eq!(h.ui.style.dark, system_dark, "style follows");
        h.ui.set_theme_mode(ThemeMode::Light);
        h.frame(vec![]);
        assert!(!h.ui.style.dark, "Light is light whatever the portal says");
        h.ui.set_theme_mode(ThemeMode::Dark);
        h.frame(vec![]);
        assert!(h.ui.style.dark, "Dark is dark whatever the portal says");
    }
}
