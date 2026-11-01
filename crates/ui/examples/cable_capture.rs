//! Cable before/after captures (PLAN §7 Cables): the real rack (`kabl_ui::show`) in a window,
//! one PNG per patch x look x zoom. Looks: `classic` (Satin Studio, the cable as it was),
//! `themed` (Signal solid with the new cable paint tokens) and `solid-classic` (the same face
//! theme with `cable.style = classic`, to isolate the cable change). One run per window size.
//!
//! `xvfb-run -a -s "-screen 0 1440x900x24" cargo run --release -p kabl-ui --example cable_capture -- 1440x900 DIR [patch,..]`
//!
//! Patches: simple (default patch), composition, stereo (reference), functional (echo-throws).
//! Output names: `DIR/<patch>__<look>__<dark|light>__z<zoom>__<WxH>.png`.
use eframe::egui;
use egui::{Pos2, Rect};
use kabl_ui::{show, PatchEditor, UiState};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const LOOKS: [(&str, &str); 3] = [
    ("classic", "kabl.satin"),
    ("themed", "kabl.line_solid"),
    ("solid-classic", "kabl.line_solid"),
];
const ZOOMS: [f32; 2] = [1.0, 0.58];

struct Job {
    patch: String,
    look: &'static str,
    theme: &'static str,
    dark: bool,
    zoom: f32,
}

struct App {
    jobs: Vec<Job>,
    i: usize,
    f: u32,
    dir: PathBuf,
    size: (u32, u32),
    editor: PatchEditor,
    ui: UiState,
    loaded: Option<String>,
}

fn patch(name: &str) -> kabl_core::PatchState {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches");
    match name {
        "simple" => kabl_standalone::default_patch(),
        "composition" => kabl_core::load(&root.join("composition"))
            .unwrap()
            .state()
            .clone(),
        "stereo" => kabl_core::load(&root.join("reference"))
            .unwrap()
            .state()
            .clone(),
        "functional" => kabl_core::load(&root.join("functional-cables/echo-throws"))
            .unwrap()
            .state()
            .clone(),
        _ => panic!("unknown patch {name}"),
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        for e in ctx.input(|i| i.raw.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = e {
                let j = &self.jobs[self.i];
                let file = self.dir.join(format!(
                    "{}__{}__{}__z{}__{}x{}.png",
                    j.patch,
                    j.look,
                    if j.dark { "dark" } else { "light" },
                    (j.zoom * 100.0).round(),
                    self.size.0,
                    self.size.1
                ));
                let [w, h] = image.size;
                let raw: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                image::save_buffer(&file, &raw, w as u32, h as u32, image::ColorType::Rgba8)
                    .unwrap();
                println!("wrote {} ({w}x{h})", file.display());
                self.i += 1;
                self.f = 0;
            }
        }
        if self.i >= self.jobs.len() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        let j = &self.jobs[self.i];
        if self.f == 0 {
            if self.loaded.as_deref() != Some(j.patch.as_str()) {
                self.editor = PatchEditor::seed_from(&patch(&j.patch));
                self.loaded = Some(j.patch.clone());
            }
            self.ui.theme = j.theme.into();
            self.ui.dark = j.dark;
            self.ui.zoom = j.zoom;
        }
        let mut raw_ui = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(self.size.0 as f32, self.size.1 as f32),
            )),
        );
        show(&mut self.editor, &mut self.ui, &mut raw_ui);
        // `show` builds the style from the theme once; force the classic cable on top of it.
        if j.look == "solid-classic"
            && self.ui.style.slots.cable.style != kabl_ui::style::CableStyle::Classic
        {
            let mut st = (*self.ui.style).clone();
            st.slots.cable.style = kabl_ui::style::CableStyle::Classic;
            self.ui.style = Arc::new(st);
        }
        self.f += 1;
        if self.f == 12 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        ctx.request_repaint();
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (w, h) = a
        .first()
        .and_then(|s| s.split_once('x'))
        .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
        .unwrap_or((1440u32, 900u32));
    let dir = PathBuf::from(a.get(1).cloned().unwrap_or_else(|| "target/cables".into()));
    std::fs::create_dir_all(&dir).unwrap();
    let patches: Vec<String> = a.get(2).map_or_else(
        || {
            ["simple", "composition", "stereo", "functional"]
                .map(String::from)
                .to_vec()
        },
        |s| s.split(',').map(String::from).collect(),
    );
    for p in &patches {
        let st = patch(p);
        let ed = PatchEditor::seed_from(&st);
        println!(
            "{p}: {} modules, {} cables",
            ed.state().modules.len(),
            ed.state().cables.len()
        );
    }
    let mut jobs = Vec::new();
    for p in &patches {
        for dark in [false, true] {
            for z in ZOOMS {
                for (look, theme) in LOOKS {
                    jobs.push(Job {
                        patch: p.clone(),
                        look,
                        theme,
                        dark,
                        zoom: z,
                    });
                }
            }
        }
    }
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([w as f32, h as f32])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "cable capture",
        opts,
        Box::new(move |cc| {
            cc.egui_ctx.set_pixels_per_point(1.0);
            let user = tempfile::tempdir().unwrap().keep();
            let mut ui = UiState::default();
            ui.library = Some(kabl_ui::library::Library::open(
                Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches")),
                user,
            ));
            ui.drawer_open = false;
            ui.browser_open = false;
            Ok(Box::new(App {
                jobs,
                i: 0,
                f: 0,
                dir,
                size: (w, h),
                editor: PatchEditor::seed_from(&patch("simple")),
                ui,
                loaded: None,
            }))
        }),
    )
    .unwrap();
}
