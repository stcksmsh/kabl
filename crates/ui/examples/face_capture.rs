//! Face captures for the G3 family PRs: the real rack (`kabl_ui::show`) with one module per
//! kind in the default Signal solid (dark) and Drafting solid (light) looks, at zoom 100, 75 and
//! 58 %. Writes one cropped PNG per face and one contact sheet per kind.
//!
//! `xvfb-run -a -s "-screen 0 1000x520x24" cargo run --release -p kabl-ui --example face_capture -- OUT_DIR [kind,kind|all] [theme]`
//!
//! `theme` is a theme id (default `kabl.line_solid`; `kabl.satin` shows the old look for a
//! before/after). Files: `OUT/<kind>__<dark|light>__z<zoom>.png` and `OUT/<kind>__sheet.png`
//! (rows dark, light; columns 100, 75, 58 %). Side by side with the G1b target: crop the R1
//! face from `docs/design/redesign/g1b/sheet-{graphite,paper}.png` and place it beside the sheet.
use eframe::egui;
use egui::{Pos2, Rect};
use image::{imageops, RgbaImage};
use kabl_ui::{show, PatchEditor, UiState};
use std::path::PathBuf;

const ZOOMS: [f32; 3] = [1.0, 0.75, 0.58];
const WINDOW: (u32, u32) = (1000, 520);
const PAD: f32 = 8.0;

struct Job {
    kind: &'static str,
    dark: bool,
    zoom: f32,
}

struct App {
    jobs: Vec<Job>,
    i: usize,
    f: u32,
    dir: PathBuf,
    theme: String,
    editor: PatchEditor,
    id: Option<kabl_core::ModuleId>,
    ui: UiState,
    shots: Vec<(String, bool, f32, RgbaImage)>,
}

impl App {
    fn sheet(&self, kind: &str) {
        let cells: Vec<_> = self.shots.iter().filter(|s| s.0 == kind).collect();
        let cw = cells.iter().map(|c| c.3.width()).max().unwrap_or(1);
        let ch = cells.iter().map(|c| c.3.height()).max().unwrap_or(1);
        let g = 8u32;
        let mut out = RgbaImage::from_pixel(
            3 * cw + 4 * g,
            2 * ch + 3 * g,
            image::Rgba([40, 40, 40, 255]),
        );
        for (_, dark, z, img) in cells {
            let col = ZOOMS.iter().position(|v| v == z).unwrap() as u32;
            let row = u32::from(!*dark);
            imageops::overlay(
                &mut out,
                img,
                i64::from(g + col * (cw + g)),
                i64::from(g + row * (ch + g)),
            );
        }
        out.save(self.dir.join(format!("{kind}__sheet.png")))
            .unwrap();
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let mut shot = None;
        for e in ctx.input(|i| i.raw.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = e {
                shot = Some(image);
            }
        }
        if let (Some(img), Some(id)) = (shot, self.id) {
            let j = &self.jobs[self.i];
            let r = self
                .ui
                .hits
                .get(&format!("module:{id}"))
                .copied()
                .unwrap_or(Rect::NOTHING)
                .expand(PAD);
            let [w, h] = img.size;
            let raw: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            let full = RgbaImage::from_raw(w as u32, h as u32, raw).unwrap();
            let (x, y) = (r.min.x.max(0.0) as u32, r.min.y.max(0.0) as u32);
            let (cw, ch) = (
                (r.width() as u32).min(w as u32 - x),
                (r.height() as u32).min(h as u32 - y),
            );
            let crop = imageops::crop_imm(&full, x, y, cw, ch).to_image();
            let name = format!(
                "{}__{}__z{}.png",
                j.kind,
                if j.dark { "dark" } else { "light" },
                (j.zoom * 100.0).round()
            );
            crop.save(self.dir.join(&name)).unwrap();
            println!("wrote {name} ({cw}x{ch})");
            self.shots.push((j.kind.to_string(), j.dark, j.zoom, crop));
            self.i += 1;
            self.f = 0;
            self.id = None;
            let done_kind = self.jobs.get(self.i).is_none_or(|n| n.kind != j.kind);
            if done_kind {
                self.sheet(j.kind);
            }
        }
        if self.i >= self.jobs.len() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        let j = &self.jobs[self.i];
        if self.id.is_none() {
            self.editor = PatchEditor::seed_from(&Default::default());
            self.id = Some(
                self.editor
                    .add_module(j.kind, kabl_core::Vec2 { x: 24.0, y: 20.0 }),
            );
            self.ui.theme = self.theme.clone();
        }
        self.ui.dark = j.dark;
        self.ui.zoom = j.zoom;
        self.ui.pan = egui::vec2(12.0, 12.0);
        let mut root = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(WINDOW.0 as f32, WINDOW.1 as f32),
            )),
        );
        show(&mut self.editor, &mut self.ui, &mut root);
        self.f += 1;
        if self.f == 8 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        ctx.request_repaint();
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let dir = PathBuf::from(a.first().cloned().unwrap_or_else(|| "target/faces".into()));
    std::fs::create_dir_all(&dir).unwrap();
    let want = a.get(1).cloned().unwrap_or_else(|| "all".into());
    let theme = a
        .get(2)
        .cloned()
        .unwrap_or_else(|| "kabl.line_solid".into());
    let kinds: Vec<&'static str> = kabl_modules::registry::KNOWN_KINDS
        .iter()
        .copied()
        .filter(|k| want == "all" || want.split(',').any(|w| w == *k))
        .collect();
    let mut jobs = Vec::new();
    for kind in kinds {
        for dark in [true, false] {
            for zoom in ZOOMS {
                jobs.push(Job { kind, dark, zoom });
            }
        }
    }
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([WINDOW.0 as f32, WINDOW.1 as f32])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "face capture",
        opts,
        Box::new(move |cc| {
            cc.egui_ctx.set_pixels_per_point(1.0);
            let mut ui = UiState::default();
            ui.browser_open = false;
            ui.drawer_open = false;
            Ok(Box::new(App {
                jobs,
                i: 0,
                f: 0,
                dir,
                theme,
                editor: PatchEditor::seed_from(&Default::default()),
                id: None,
                ui,
                shots: Vec::new(),
            }))
        }),
    )
    .unwrap();
}
