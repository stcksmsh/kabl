//! UI frame time of `kabl_ui::show` (build, layout, tessellation; no GPU) on a simple and a
//! dense patch, with the Sounds browser and Routing drawer open.
//! `cargo run --release -p kabl-ui --example bench_chrome [frames] [live] [editor|draw]`
//! With `live`: rack only (no browser, drawer), audio running, and every module's face taps
//! report new made-up values each frame, so every display and modulated knob animates.
//! With `editor` (after `live`): the wavetable editor is open on the patch's first `osc.wt`
//! (one is added when it has none). With `draw`: the same, and a pen stroke is down on the
//! single-frame view, moving every frame, so tables are written during the measured frames.
use egui::{Pos2, RawInput, Rect};
use kabl_core::PatchState;
use kabl_ui::library::Library;
use kabl_ui::{show, PatchEditor, UiState};
use std::path::Path;
use std::time::Instant;

fn patches() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches")
}

fn dense() -> PatchState {
    kabl_core::load(&patches().join("composition"))
        .unwrap()
        .state()
        .clone()
}

/// `perform`: 0 closed, 1 the full Perform view, 2 Perform docked under the rack.
#[allow(clippy::too_many_arguments)]
fn run(
    name: &str,
    state: &PatchState,
    w: f32,
    h: f32,
    dark: bool,
    frames: usize,
    perform: u8,
    live: bool,
    edit: u8,
) {
    let user = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut editor = PatchEditor::seed_from(state);
    let mut ui = UiState::default();
    ui.browser_open = !live;
    ui.drawer_open = !live;
    ui.inspect.audio = live;
    if let Some(z) = std::env::var("KABL_BENCH_ZOOM")
        .ok()
        .and_then(|s| s.parse().ok())
    {
        ui.zoom = z;
    }
    ui.perform_open = perform > 0;
    ui.perform_tall = perform == 1;
    ui.dark = dark;
    if let Ok(id) = std::env::var("KABL_BENCH_THEME") {
        ui.theme = id;
    }
    ui.library = Some(Library::open(Some(patches()), user.path().to_path_buf()));
    let size = egui::vec2(w, h);
    let mut times = Vec::with_capacity(frames);
    for i in 0..frames + 20 {
        let mut raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            time: Some(i as f64 / 60.0),
            focused: true,
            ..Default::default()
        };
        if edit > 0 && i == 5 {
            let wt = editor
                .state()
                .modules
                .iter()
                .find(|(_, m)| m.kind == "osc.wt")
                .map(|(&id, _)| id)
                .unwrap_or_else(|| {
                    editor.add_module("osc.wt", kabl_core::Vec2 { x: 24.0, y: 20.0 })
                });
            kabl_ui::tables::open(&editor, &mut ui, wt);
        }
        if edit > 1 && i >= 18 {
            if let Some(r) = ui.hits.get("wt:wave").copied() {
                let at = r.center()
                    + egui::vec2(
                        (i as f32 * 0.7).sin() * r.width() * 0.4,
                        (i as f32 * 0.31).sin() * r.height() * 0.4,
                    );
                raw.events.push(egui::Event::PointerMoved(at));
                if i == 18 {
                    raw.events.push(egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    });
                }
            }
        }
        if live {
            let now = i as f64 / 60.0;
            for (&id, m) in &state.modules {
                for port in 0..3u8 {
                    let ph = (now * 0.7).fract() as f32;
                    let mut view = kabl_modules::ModuleView {
                        valid: true,
                        position: if m.kind == "env.adsr" {
                            1.0 + (now % 3.0).floor() as f32
                        } else {
                            ph
                        },
                        ..Default::default()
                    };
                    for (k, c) in view.cycle.iter_mut().enumerate() {
                        *c = (std::f32::consts::TAU * (k as f32 / 128.0 + ph)).sin();
                    }
                    ui.live.taps.insert(
                        (id, port),
                        kabl_ui::displays::LiveTap {
                            last: (now * 2.0).sin() as f32 * 0.5 + 0.5,
                            min: 0.0,
                            max: 1.0,
                            view,
                            at: now,
                        },
                    );
                }
            }
        }
        let t = Instant::now();
        ctx.begin_pass(raw);
        let mut root = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::ZERO, size)),
        );
        show(&mut editor, &mut ui, &mut root);
        let out = ctx.end_pass();
        let _ = ctx.tessellate(out.shapes, out.pixels_per_point);
        if i >= 20 {
            times.push(t.elapsed().as_secs_f64() * 1e6);
        }
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |f: f64| times[((times.len() - 1) as f64 * f) as usize];
    println!(
        "{name:<7} {w:.0}x{h:.0} {:<5} median {:>7.0} µs  p95 {:>7.0} µs  max {:>7.0} µs",
        if dark { "dark" } else { "light" },
        q(0.5),
        q(0.95),
        q(1.0)
    );
}

fn main() {
    let frames = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);
    let live = std::env::args().nth(2).as_deref() == Some("live");
    let edit = match std::env::args().nth(3).as_deref() {
        Some("editor") => 1,
        Some("draw") => 2,
        _ => 0,
    };
    let simple = kabl_standalone::default_patch();
    let dense = dense();
    for (w, h) in [(1440.0, 900.0), (1280.0, 800.0)] {
        for dark in [false, true] {
            run("simple", &simple, w, h, dark, frames, 0, live, edit);
            run("dense", &dense, w, h, dark, frames, 0, live, edit);
            if !live {
                run("d-perf", &dense, w, h, dark, frames, 1, false, 0);
                run("d-dock", &dense, w, h, dark, frames, 2, false, 0);
            }
        }
    }
}
