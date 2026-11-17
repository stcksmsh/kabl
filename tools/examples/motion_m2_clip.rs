//! The M2 captures: the mockup rack at 100 %, look B dark, 1440x900 on a fixed 60 fps clock.
//! `settle` plugs a cable and moves a module; `settle-reduced` is the same with reduced motion
//! on; `settings` stills the settings menu with the Theme and Reduce motion rows.
//!
//! `xvfb-run -a -s "-screen 0 1440x900x24" cargo run --release -p kabl-tools --example motion_m2_clip -- SCENARIO OUT_DIR`
//! writes `OUT_DIR/frame_000.png ..`; `ffmpeg -framerate 60 -i OUT_DIR/frame_%03d.png -c:v libvpx-vp9 -pix_fmt yuv420p out.webm`.
use eframe::egui;
use egui::{Pos2, Rect};
use image::RgbaImage;
use kabl_core::{PortRef, Vec2};
use kabl_ui::desktop::ReduceMotion;
use kabl_ui::{show, PatchEditor, UiState};
use std::path::PathBuf;

#[path = "support/mockup.rs"]
mod mockup;

/// Frames before the first captured one: the app settles at rest.
const SETTLE: u32 = 12;

struct App {
    scenario: String,
    frames: u32,
    dir: PathBuf,
    editor: PatchEditor,
    ui: UiState,
    f: u32,
    saved: u32,
    inject: Vec<egui::Event>,
    /// The cable taken out at k = 20 and plugged back at k = 60.
    unplugged: Option<(PortRef, PortRef)>,
}

impl App {
    fn module(&self, kind: &str) -> u64 {
        *self
            .editor
            .state()
            .modules
            .iter()
            .find(|(_, m)| m.kind == kind)
            .map(|(id, _)| id)
            .unwrap()
    }

    fn click(&mut self, at: Pos2, down: bool) {
        self.inject.push(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: down,
            modifiers: egui::Modifiers::NONE,
        });
    }

    /// The script, by captured-frame index `k` (60 fps).
    fn script(&mut self, k: u32) {
        if self.scenario.starts_with("settle") {
            let away = Pos2::new(700.0, 880.0);
            match k {
                20 => {
                    // Pull the reverb -> output cable.
                    let out = self.module("out");
                    let found = self
                        .editor
                        .state()
                        .cables
                        .iter()
                        .find(|(_, c)| c.to.module_id() == Some(out))
                        .map(|(id, c)| (*id, c.from.clone(), c.to.clone()));
                    if let Some((id, from, to)) = found {
                        self.editor.disconnect(id);
                        self.unplugged = Some((from, to));
                    }
                }
                59 => {
                    self.inject.push(egui::Event::PointerMoved(away));
                    self.click(away, true);
                }
                60 => self.click(away, false),
                61 => {
                    if let Some((from, to)) = self.unplugged.take() {
                        self.editor.connect(from, to);
                    }
                }
                // Drop a module a little way off, pointer up: the move settles its cables.
                160 => {
                    let mix = self.module("mixer");
                    self.editor.move_module(mix, Vec2 { x: 280.0, y: 380.0 });
                }
                _ => {}
            }
        } else if self.scenario == "settings" {
            match k {
                10 => {
                    if let Some(r) = self.ui.hits.get("view-menu").copied() {
                        self.inject.push(egui::Event::PointerMoved(r.center()));
                    }
                }
                12 => {
                    if let Some(r) = self.ui.hits.get("view-menu").copied() {
                        self.click(r.center(), true);
                    }
                }
                14 => {
                    if let Some(r) = self.ui.hits.get("view-menu").copied() {
                        self.click(r.center(), false);
                    }
                }
                _ => {}
            }
        }
    }
}

impl eframe::App for App {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        raw.focused = true;
        raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1440.0, 900.0)));
        raw.time = Some(f64::from(self.f) / 60.0);
        raw.predicted_dt = 1.0 / 60.0;
        if self.inject.is_empty() && self.scenario != "settings" {
            raw.events.push(egui::Event::PointerGone);
        }
        raw.events.append(&mut self.inject);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ctx.input_mut(|i| i.focused = true);
        for e in ctx.input(|i| i.raw.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = e {
                let [w, h] = image.size;
                let raw: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                let full = RgbaImage::from_raw(w as u32, h as u32, raw).unwrap();
                let crop =
                    image::imageops::crop_imm(&full, 0, 0, 1440.min(w as u32), 900.min(h as u32))
                        .to_image();
                crop.save(self.dir.join(format!("frame_{:03}.png", self.saved)))
                    .unwrap();
                self.saved += 1;
                if self.saved == self.frames {
                    println!("wrote {} frames", self.saved);
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
        }
        if self.f >= SETTLE {
            self.script(self.f - SETTLE);
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        self.f += 1;
        ctx.request_repaint();
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let scenario = args
        .next()
        .expect("scenario: settle|settle-reduced|settings");
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "target/motion-m2".into()));
    let frames = match scenario.as_str() {
        "settle" | "settle-reduced" => 330,
        "settings" => 24,
        other => panic!("unknown scenario {other}"),
    };
    std::fs::create_dir_all(&dir).unwrap();
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "motion m2 clip",
        opts,
        Box::new(move |cc| {
            cc.egui_ctx.set_pixels_per_point(1.0);
            let mut ui = UiState::default();
            ui.theme = "kabl.look_b".into();
            ui.dark = true;
            ui.status_meter = true;
            ui.drawer_open = false;
            if scenario == "settle-reduced" {
                ui.reduce_motion = ReduceMotion::On;
            }
            Ok(Box::new(App {
                scenario,
                frames,
                dir,
                editor: mockup::mockup_patch(false),
                ui,
                f: 0,
                saved: 0,
                inject: Vec::new(),
                unplugged: None,
            }))
        }),
    )
    .unwrap();
}
