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
fn a_quiet_rack_with_an_output_idles_and_a_loud_master_meter_wakes_it() {
    let mut h = H::new();
    // No tap reports at all: the Output's meter has nothing to animate.
    h.shortest_repaint(90, None);
    let idle = h.shortest_repaint(60, None);
    assert!(
        idle.is_none_or(|d| d >= Duration::from_millis(60)),
        "an Output at rest asks for no 33 ms repaints, got {idle:?}"
    );
    // Sound reaches the master meter: frames at the live rate.
    h.ui.live.master = [0.4, 0.4];
    let live = h.shortest_repaint(30, None);
    assert!(
        live.is_some_and(|d| d <= Duration::from_millis(40)),
        "a loud master meter repaints at 33 ms, got {live:?}"
    );
}

/// A rack of the live displays: filter, envelope, mixer, delay and an Output, patched in a row.
fn displays_rack() -> (H, Vec<kabl_core::ModuleId>) {
    let mut h = H::new();
    let ids: Vec<_> = ["filter.svf", "env.adsr", "mixer", "delay"]
        .iter()
        .enumerate()
        .map(|(i, k)| {
            h.editor.add_module(
                k,
                Vec2 {
                    x: 24.0 + 220.0 * i as f32,
                    y: 220.0,
                },
            )
        })
        .collect();
    h.frame(None);
    (h, ids)
}

impl H {
    /// Every module of `ids` reports `level` on its first output.
    fn report(&mut self, ids: &[kabl_core::ModuleId], level: f32) {
        for &id in ids {
            let tap = LiveTap {
                last: level,
                min: -level,
                max: level,
                view: Default::default(),
                at: self.t,
            };
            // As the engine's reports arrive: the peak hold follows them.
            self.ui.live.set_tap(id, 0, tap);
        }
    }
}

#[test]
fn a_rack_of_live_displays_idles_when_silent_and_wakes_on_signal() {
    let (mut h, ids) = displays_rack();
    // Silent taps for a while (the mixer's peak hold has nothing to fall from): idle.
    for _ in 0..90 {
        h.report(&ids, 0.0);
        h.frame(None);
    }
    h.delays.lock().unwrap().clear();
    for _ in 0..60 {
        h.report(&ids, 0.0);
        h.frame(None);
    }
    let idle = h.delays.lock().unwrap().iter().min().copied();
    assert!(
        idle.is_none_or(|d| d >= Duration::from_millis(60)),
        "silent displays ask for no 33 ms repaints, got {idle:?}"
    );
    // Signal on the taps: frames at the live rate.
    h.delays.lock().unwrap().clear();
    for _ in 0..30 {
        h.report(&ids, 0.5);
        h.frame(None);
    }
    let live = h.delays.lock().unwrap().iter().min().copied();
    assert!(
        live.is_some_and(|d| d <= Duration::from_millis(40)),
        "moving displays repaint at 33 ms, got {live:?}"
    );
    // Silence again: after the hold has fallen (1 s + 1.4 s) the rack idles once more.
    for _ in 0..240 {
        h.report(&ids, 0.0);
        h.frame(None);
    }
    h.delays.lock().unwrap().clear();
    for _ in 0..60 {
        h.report(&ids, 0.0);
        h.frame(None);
    }
    let idle = h.delays.lock().unwrap().iter().min().copied();
    assert!(
        idle.is_none_or(|d| d >= Duration::from_millis(60)),
        "after the signal stops the displays idle again, got {idle:?}"
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
