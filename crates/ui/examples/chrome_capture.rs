//! Scene captures for the G5 chrome PRs: the real app (`kabl_ui::show`) with the factory sound
//! Acid Line, in the default Signal solid (dark) and Drafting solid (light) looks, at the three
//! window sizes of PLAN §8, for the Rack, Perform and Sounds scenes.
//!
//! `xvfb-run -a -s "-screen 0 1440x900x24" cargo run --release -p kabl-ui --example chrome_capture -- OUT_DIR [scene,scene|all] [theme]`
//!
//! Files: `OUT/<scene>__<dark|light>__<W>x<H>.png`. `theme` is a theme id (default
//! `kabl.line_solid`; `kabl.satin` for a before/after). Silent: no audio device is opened.
use eframe::egui;
use egui::{Pos2, Rect};
use image::RgbaImage;
use kabl_ui::library::Library;
use kabl_ui::{show, PatchEditor, UiState};
use std::path::PathBuf;

const SIZES: [(u32, u32); 3] = [(1440, 900), (1280, 800), (1180, 680)];
const SCENES: [&str; 3] = ["rack", "perform", "sounds"];

struct Job {
    scene: &'static str,
    dark: bool,
    size: (u32, u32),
}

struct App {
    jobs: Vec<Job>,
    i: usize,
    f: u32,
    dir: PathBuf,
    theme: String,
    editor: PatchEditor,
    ui: UiState,
    _user: tempfile::TempDir,
}

fn patches() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../patches")
}

impl App {
    fn prepare(&mut self) {
        let j = &self.jobs[self.i];
        let dir = patches().join("presets/acid-line");
        let state = kabl_core::load(&dir).unwrap().state().clone();
        self.editor = PatchEditor::seed_from(&state);
        let mut ui = UiState::default();
        ui.theme = self.theme.clone();
        ui.dark = j.dark;
        ui.status_meter = true;
        ui.library = Some(Library::open(
            Some(patches()),
            self._user.path().to_path_buf(),
        ));
        let mut doc = kabl_ui::browser::Doc::new(
            "Acid Line",
            kabl_ui::browser::DocOrigin::Library("factory:acid-line".into()),
            &state,
        );
        doc.meta.category = "Bass".into();
        ui.doc = Some(doc);
        // An unsaved edit, so the sound dropdown shows its dot.
        let edit = self.editor.state().modules.iter().find_map(|(&id, m)| {
            m.params
                .iter()
                .next()
                .map(|(k, &v)| (id, k.clone(), v as f32))
        });
        if let Some((id, k, v)) = edit {
            self.editor.set_param(id, &k, v + 1.0);
        }
        ui.browser_open = j.scene == "sounds";
        ui.drawer_open = j.scene == "rack";
        ui.perform_open = j.scene == "perform";
        ui.perform_tall = j.scene == "perform";
        self.ui = ui;
        self.f = 0;
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.i >= self.jobs.len() {
            return;
        }
        let mut shot = None;
        for e in ctx.input(|i| i.raw.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = e {
                shot = Some(image);
            }
        }
        if let Some(img) = shot {
            let j = &self.jobs[self.i];
            let [w, h] = img.size;
            let raw: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            let full = RgbaImage::from_raw(w as u32, h as u32, raw).unwrap();
            let crop = image::imageops::crop_imm(
                &full,
                0,
                0,
                j.size.0.min(w as u32),
                j.size.1.min(h as u32),
            )
            .to_image();
            let name = format!(
                "{}__{}__{}x{}.png",
                j.scene,
                if j.dark { "dark" } else { "light" },
                j.size.0,
                j.size.1
            );
            crop.save(self.dir.join(&name)).unwrap();
            println!("wrote {name}");
            self.i += 1;
            if self.i >= self.jobs.len() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
            self.prepare();
        }
        let size = self.jobs[self.i].size;
        let mut root = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(size.0 as f32, size.1 as f32),
            )),
        );
        show(&mut self.editor, &mut self.ui, &mut root);
        // The mockup's rack is drawn at 95 %: set it once the first fit has run.
        if self.f == 6 && self.jobs[self.i].scene == "rack" {
            self.ui.zoom = 0.95;
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
    let dir = PathBuf::from(a.first().cloned().unwrap_or_else(|| "target/chrome".into()));
    std::fs::create_dir_all(&dir).unwrap();
    let want = a.get(1).cloned().unwrap_or_else(|| "all".into());
    let theme = a
        .get(2)
        .cloned()
        .unwrap_or_else(|| "kabl.line_solid".into());
    let mut jobs = Vec::new();
    for scene in SCENES {
        if want != "all" && !want.split(',').any(|w| w == scene) {
            continue;
        }
        for dark in [true, false] {
            for size in SIZES {
                jobs.push(Job { scene, dark, size });
            }
        }
    }
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "chrome capture",
        opts,
        Box::new(move |cc| {
            cc.egui_ctx.set_pixels_per_point(1.0);
            let mut app = App {
                jobs,
                i: 0,
                f: 0,
                dir,
                theme,
                editor: PatchEditor::seed_from(&Default::default()),
                ui: UiState::default(),
                _user: tempfile::tempdir().unwrap(),
            };
            app.prepare();
            Ok(Box::new(app))
        }),
    )
    .unwrap();
}
