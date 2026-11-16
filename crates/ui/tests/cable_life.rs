//! Living cables through real egui frames: a cable tapped by the engine lights from what the
//! tap reports, and a rack whose cables are all quiet asks for no 33 ms repaints.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use egui::{Pos2, RawInput, Rect};
use kabl_core::{PortRef, Vec2};
use kabl_ui::displays::LiveTap;
use kabl_ui::{show, PatchEditor, UiState};

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    t: f64,
    delays: Arc<Mutex<Vec<Duration>>>,
    noise: u64,
}

impl H {
    fn new() -> Self {
        let mut editor = PatchEditor::new();
        let noise = editor.add_module("noise", Vec2 { x: 24.0, y: 20.0 });
        let out = editor.add_module("out", Vec2 { x: 400.0, y: 20.0 });
        editor.connect(
            PortRef::Module {
                id: noise,
                port: "out".into(),
            },
            PortRef::Module {
                id: out,
                port: "in".into(),
            },
        );
        let ctx = egui::Context::default();
        let delays = Arc::new(Mutex::new(Vec::new()));
        let d = delays.clone();
        ctx.set_request_repaint_callback(move |i| d.lock().unwrap().push(i.delay));
        let mut ui = UiState::default();
        ui.inspect.audio = true;
        let mut h = H {
            ctx,
            editor,
            ui,
            t: 0.0,
            delays,
            noise,
        };
        h.frame(None);
        h
    }

    /// One frame at 60 Hz; `level` is what the engine reports for the noise output.
    fn frame(&mut self, level: Option<f32>) {
        if let Some(l) = level {
            let tap = LiveTap {
                last: l,
                min: -l,
                max: l,
                view: Default::default(),
                at: self.t,
            };
            self.ui.live.taps.insert((self.noise, 0), tap);
        }
        let size = egui::vec2(900.0, 500.0);
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            time: Some(self.t),
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

    /// Shortest repaint delay requested over the next `n` frames.
    fn shortest_repaint(&mut self, n: usize, level: Option<f32>) -> Option<Duration> {
        self.delays.lock().unwrap().clear();
        for _ in 0..n {
            self.frame(level);
        }
        let d = self.delays.lock().unwrap().iter().min().copied();
        d
    }
}

#[test]
fn a_loud_cable_keeps_frames_coming_and_a_quiet_one_does_not() {
    let mut h = H::new();
    // Loud noise for a while: frames at the live rate.
    let live = h.shortest_repaint(30, Some(0.5));
    assert!(
        live.is_some_and(|d| d <= Duration::from_millis(40)),
        "a lit cable repaints at 33 ms, got {live:?}"
    );
    // The engine reports silence: after the quiet time the cable is static.
    h.shortest_repaint(150, Some(0.0));
    let idle = h.shortest_repaint(60, Some(0.0));
    assert!(
        idle.is_none_or(|d| d >= Duration::from_millis(60)),
        "a quiet rack asks for no 33 ms repaints, got {idle:?} (causes {:?})",
        h.ctx.repaint_causes()
    );
}

#[test]
fn cable_life_off_asks_for_no_taps_and_no_frames() {
    let mut h = H::new();
    h.ui.cable_life = kabl_ui::cable_life::CableLife::Off;
    h.shortest_repaint(60, Some(0.5));
    let d = h.shortest_repaint(60, Some(0.5));
    assert!(d.is_none_or(|d| d >= Duration::from_millis(60)), "{d:?}");
}
