//! Text-options stills (AccessKit slice R4a): the mockup patch in look B dark at a fractional
//! scale, with egui's stock text options (`before`) and the ones kabl sets (`after`), plus 2x
//! crops of the sequencer and the filter, where the small labels and readouts are.
//!
//! `xvfb-run -a -s "-screen 0 1920x1200x24" cargo run --release -p kabl-tools --example accesskit_capture -- DIR [scale,scale]`
//!
//! The window is 1920x1200 physical pixels; egui runs at `scale`, so the layout is 1536x960
//! points at 1.25 and 1280x800 at 1.5. Files: `DIR/s<125|150>__<before|after>__full.png` and
//! `..__seq-2x.png`, `..__filter-2x.png` (nearest-neighbour doubling, no smoothing).
use eframe::egui;
use egui::{Pos2, Rect};
use image::RgbaImage;
use kabl_ui::{show, PatchEditor, UiState};
use std::path::PathBuf;

#[path = "support/mockup.rs"]
mod mockup;

struct Job {
    scale: f32,
    after: bool,
}

struct App {
    jobs: Vec<Job>,
    i: usize,
    f: u32,
    dir: PathBuf,
    editor: PatchEditor,
    ui: UiState,
}

impl App {
    fn prepare(&mut self, ctx: &egui::Context) {
        let j = &self.jobs[self.i];
        ctx.set_pixels_per_point(j.scale);
        // kabl installs its style once per theme change; force it again so the `after` job
        // does not inherit the stock options the `before` job forced on top.
        ctx.data_mut(|d| d.remove_temp::<(String, bool)>(egui::Id::new("kabl-style-applied")));
        self.editor = mockup::mockup_patch(false);
        self.ui = UiState::default();
        self.ui.theme = "kabl.look_b".into();
        self.ui.dark = true;
        self.ui.drawer_open = false;
        self.ui.browser_open = false;
        self.f = 0;
    }

    fn name(&self, what: &str) -> String {
        let j = &self.jobs[self.i];
        format!(
            "s{}__{}__{what}.png",
            (j.scale * 100.0).round(),
            if j.after { "after" } else { "before" }
        )
    }

    fn write_stills(&self, img: &egui::ColorImage) {
        let [w, h] = img.size;
        let raw: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
        let full = RgbaImage::from_raw(w as u32, h as u32, raw).unwrap();
        full.save(self.dir.join(self.name("full"))).unwrap();
        let ppp = self.jobs[self.i].scale;
        let kind = |k: &str| {
            self.editor
                .state()
                .modules
                .iter()
                .find(|(_, m)| m.kind == k)
                .map(|(&id, _)| id)
                .unwrap()
        };
        for (what, k) in [("seq-2x", "seq"), ("filter-2x", "filter.svf")] {
            let r = self.ui.hits[&format!("module:{}", kind(k))].expand(4.0);
            let x = ((r.left() * ppp) as u32).min(w as u32 - 1);
            let y = ((r.top().max(0.0) * ppp) as u32).min(h as u32 - 1);
            let cw = ((r.width() * ppp) as u32).min(w as u32 - x);
            let ch = ((r.height() * ppp) as u32).min(h as u32 - y);
            let crop = image::imageops::crop_imm(&full, x, y, cw, ch).to_image();
            let big = image::imageops::resize(
                &crop,
                cw * 2,
                ch * 2,
                image::imageops::FilterType::Nearest,
            );
            big.save(self.dir.join(self.name(what))).unwrap();
        }
        println!("wrote {}", self.name("full"));
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
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
            self.write_stills(&img);
            self.i += 1;
            if self.i >= self.jobs.len() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
            self.prepare(&ctx);
        }
        let size = ctx.content_rect().size();
        let mut root = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::ZERO, size)),
        );
        show(&mut self.editor, &mut self.ui, &mut root);
        if !self.jobs[self.i].after {
            // Stock egui text options (hinting on, sub-pixel binning on), over kabl's.
            ctx.global_style_mut(|s| {
                s.visuals.text_options = egui::Visuals::dark().text_options;
                if std::env::var("KABL_TEXT_PROBE").is_ok() {
                    s.visuals.text_options.font_hinting = false;
                }
            });
        }
        self.f += 1;
        if self.f == 20 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        ctx.request_repaint();
    }
}

fn main() {
    let dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "target/accesskit".into()),
    );
    std::fs::create_dir_all(&dir).unwrap();
    let scales: Vec<f32> = std::env::args().nth(2).map_or_else(
        || vec![1.25, 1.5],
        |s| s.split(',').map(|v| v.parse().unwrap()).collect(),
    );
    let mut jobs = Vec::new();
    for &scale in &scales {
        for after in [false, true] {
            jobs.push(Job { scale, after });
        }
    }
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1920.0, 1200.0])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "accesskit capture",
        opts,
        Box::new(move |cc| {
            let mut app = App {
                jobs,
                i: 0,
                f: 0,
                dir,
                editor: PatchEditor::seed_from(&Default::default()),
                ui: UiState::default(),
            };
            app.prepare(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
    .unwrap();
}
