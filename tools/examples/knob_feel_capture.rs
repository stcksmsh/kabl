//! Knob-feel captures (M1a): the mockup rack at 100 %, look B dark, 1440x900.
//! `hover.png`: the pointer on the filter's Cutoff (ring and value bubble). `focus.png`: keyboard
//! focus on Resonance with Cutoff inspected, so the two rings can be compared. Each also as a 2x
//! crop (`*_2x.png`).
//!
//! `xvfb-run -a -s "-screen 0 1440x900x24" cargo run --release -p kabl-tools --example knob_feel_capture -- docs/knob-feel`
use eframe::egui;
use egui::{Pos2, Rect};
use image::RgbaImage;
use kabl_ui::{show, PatchEditor, UiState};
use std::path::PathBuf;

#[path = "support/mockup.rs"]
mod mockup;

struct App {
    dir: PathBuf,
    editor: PatchEditor,
    ui: UiState,
    f: u32,
    /// Next capture: 0 hover, 1 focus, 2 done.
    phase: u32,
    filter: u64,
}

impl App {
    fn knob(&self, param: &str) -> Pos2 {
        self.ui.hits[&format!("knob:{}.{param}", self.filter)].center()
    }

    fn write_shot(&self, name: &str, full: &RgbaImage, at: Pos2) {
        let crop = image::imageops::crop_imm(full, 0, 0, 1440, 900).to_image();
        crop.save(self.dir.join(format!("{name}.png"))).unwrap();
        let (x, y) = (
            (at.x as u32).saturating_sub(120),
            (at.y as u32).saturating_sub(110),
        );
        let part = image::imageops::crop_imm(&crop, x, y, 240, 200).to_image();
        let big = image::imageops::resize(&part, 480, 400, image::imageops::FilterType::Nearest);
        big.save(self.dir.join(format!("{name}_2x.png"))).unwrap();
    }
}

impl eframe::App for App {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        raw.focused = true;
        raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1440.0, 900.0)));
        raw.time = Some(f64::from(self.f) / 60.0);
        raw.predicted_dt = 1.0 / 60.0;
        if self.phase == 0 && self.f >= 20 {
            raw.events
                .push(egui::Event::PointerMoved(self.knob("cutoff_hz")));
        } else {
            raw.events.push(egui::Event::PointerGone);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ctx.input_mut(|i| i.focused = true);
        for e in ctx.input(|i| i.raw.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = e {
                let [w, h] = image.size;
                let raw: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                let full = RgbaImage::from_raw(w as u32, h as u32, raw).unwrap();
                if self.phase == 0 {
                    self.write_shot("hover", &full, self.knob("cutoff_hz"));
                    self.phase = 1;
                    self.f = 100;
                } else {
                    self.write_shot("focus", &full, self.knob("resonance"));
                    println!("wrote captures");
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
            }
        }
        let mut root = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new()
                .max_rect(Rect::from_min_size(Pos2::ZERO, egui::vec2(1440.0, 900.0))),
        );
        show(&mut self.editor, &mut self.ui, &mut root);
        if self.f == 6 {
            self.ui.zoom = 1.0;
            self.ui.drawer_open = false;
        }
        if self.phase == 1 && self.f == 101 {
            self.ui.inspected = Some((self.filter, "cutoff_hz".to_string()));
            let id = egui::Id::new(("kabl-knob", self.filter, "resonance"));
            ctx.memory_mut(|m| m.request_focus(id));
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("kabl-keyboard-nav"), true));
        }
        if (self.phase == 0 && self.f == 60) || (self.phase == 1 && self.f == 130) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        self.f += 1;
        ctx.request_repaint();
    }
}

fn main() {
    let dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "docs/knob-feel".into()),
    );
    std::fs::create_dir_all(&dir).unwrap();
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "knob feel capture",
        opts,
        Box::new(move |cc| {
            cc.egui_ctx.set_pixels_per_point(1.0);
            let mut ui = UiState::default();
            ui.theme = "kabl.look_b".into();
            ui.dark = true;
            ui.drawer_open = false;
            let editor = mockup::mockup_patch(false);
            let filter = *editor
                .state()
                .modules
                .iter()
                .find(|(_, m)| m.kind == "filter.svf")
                .map(|(id, _)| id)
                .unwrap();
            Ok(Box::new(App {
                dir,
                editor,
                ui,
                f: 0,
                phase: 0,
                filter,
            }))
        }),
    )
    .unwrap();
}
