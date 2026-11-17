//! The transition clips (M1b): the mockup rack at 100 %, look B dark, 1440x900 on a fixed 60 fps
//! clock. One scenario per run: `drawer` (open, then close), `view` (Rack -> Sounds -> Rack),
//! `zoom` (a zoom-in glide, then fit) or `toast` (a success toast in and out).
//!
//! `xvfb-run -a -s "-screen 0 1440x900x24" cargo run --release -p kabl-tools --example transitions_clip -- SCENARIO OUT_DIR`
//! writes `OUT_DIR/frame_000.png ..`; `ffmpeg -framerate 60 -i OUT_DIR/frame_%03d.png -c:v libvpx-vp9 -pix_fmt yuv420p out.webm`.
use eframe::egui;
use egui::{Pos2, Rect};
use image::RgbaImage;
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
    /// Events to inject on the next frame's input.
    inject: Vec<egui::Event>,
}

impl App {
    /// The script, by captured-frame index `k` (60 fps).
    fn script(&mut self, k: u32) {
        let center = Pos2::new(720.0, 450.0);
        match (self.scenario.as_str(), k) {
            ("drawer", 30) => self.ui.drawer_open = true,
            ("drawer", 150) => self.ui.drawer_open = false,
            ("view", 30) => self.ui.browser_open = true,
            ("view", 150) => self.ui.browser_open = false,
            ("zoom", 29) => self.inject.push(egui::Event::PointerMoved(center)),
            ("zoom", 30) => self.inject.push(egui::Event::Zoom(1.4)),
            ("zoom", 120) => {
                // Fit: a real click on the toolbar's fit button.
                if let Some(r) = self.ui.hits.get("zoom:fit").copied() {
                    self.inject.push(egui::Event::PointerMoved(r.center()));
                }
            }
            ("zoom", 124) => self.press(true),
            ("zoom", 126) => self.press(false),
            ("zoom", 200) => self.inject.push(egui::Event::PointerGone),
            ("toast", 30) => self.ui.last_message = Some("opened \"Evolving Pad\"".into()),
            _ => {}
        }
    }

    fn press(&mut self, down: bool) {
        if let Some(r) = self.ui.hits.get("zoom:fit").copied() {
            self.inject.push(egui::Event::PointerButton {
                pos: r.center(),
                button: egui::PointerButton::Primary,
                pressed: down,
                modifiers: egui::Modifiers::NONE,
            });
        }
    }
}

impl eframe::App for App {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        raw.focused = true;
        raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1440.0, 900.0)));
        raw.time = Some(f64::from(self.f) / 60.0);
        raw.predicted_dt = 1.0 / 60.0;
        if self.inject.is_empty() && self.scenario != "zoom" {
            // No pointer over the rack: no hover tooltips in the clip.
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
            // The frame the script acts on is captured one frame later: `k` counts captures.
            self.script(self.f - SETTLE);
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        self.f += 1;
        ctx.request_repaint();
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let scenario = args.next().expect("scenario: drawer|view|zoom|toast");
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "target/transitions".into()));
    let frames = match scenario.as_str() {
        "drawer" | "view" => 270,
        "zoom" => 270,
        "toast" => 330,
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
        "transitions clip",
        opts,
        Box::new(move |cc| {
            cc.egui_ctx.set_pixels_per_point(1.0);
            let mut ui = UiState::default();
            ui.theme = "kabl.look_b".into();
            ui.dark = true;
            ui.status_meter = true;
            ui.drawer_open = false;
            Ok(Box::new(App {
                scenario,
                frames,
                dir,
                editor: mockup::mockup_patch(false),
                ui,
                f: 0,
                saved: 0,
                inject: Vec::new(),
            }))
        }),
    )
    .unwrap();
}
