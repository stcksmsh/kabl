//! UI frame time of `kabl_ui::show` (build, layout, tessellation; no GPU) on a simple and a
//! dense patch, with the Sounds browser and Routing drawer open.
//! `cargo run --release -p kabl-ui --example bench_chrome [frames]`
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
fn run(name: &str, state: &PatchState, w: f32, h: f32, dark: bool, frames: usize, perform: u8) {
    let user = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut editor = PatchEditor::seed_from(state);
    let mut ui = UiState::default();
    ui.browser_open = true;
    ui.drawer_open = true;
    ui.perform_open = perform > 0;
    ui.perform_tall = perform == 1;
    ui.dark = dark;
    ui.library = Some(Library::open(Some(patches()), user.path().to_path_buf()));
    let size = egui::vec2(w, h);
    let mut times = Vec::with_capacity(frames);
    for i in 0..frames + 20 {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            time: Some(i as f64 / 60.0),
            focused: true,
            ..Default::default()
        };
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
    let simple = kabl_standalone::default_patch();
    let dense = dense();
    for (w, h) in [(1440.0, 900.0), (1280.0, 800.0)] {
        for dark in [false, true] {
            run("simple", &simple, w, h, dark, frames, 0);
            run("dense", &dense, w, h, dark, frames, 0);
            run("d-perf", &dense, w, h, dark, frames, 1);
            run("d-dock", &dense, w, h, dark, frames, 2);
        }
    }
}
