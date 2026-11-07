//! Captures the custom-code notice in both default looks (`graphite`, `paper`) at 1440x900.
//! Writes `DIR/code-notice__<theme>__1440x900.png`.
//!
//! `xvfb-run -a -s "-screen 0 1440x900x24" cargo run --release -p kabl-ui --example code_notice_capture -- DIR`
use eframe::egui;
use kabl_ui::code_notice::{self, CodeEntry};
use kabl_ui::kit;
use kabl_ui::style::line_solid_default;
use std::path::PathBuf;

struct App {
    dir: PathBuf,
    dark: bool,
    f: u32,
    code: Vec<CodeEntry>,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        for e in ctx.input(|i| i.raw.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = e {
                let [w, h] = image.size;
                let raw: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                let name = format!(
                    "code-notice__{}__1440x900.png",
                    if self.dark { "graphite" } else { "paper" }
                );
                image::RgbaImage::from_raw(w as u32, h as u32, raw)
                    .unwrap()
                    .save(self.dir.join(&name))
                    .unwrap();
                println!("wrote {name}");
                if !self.dark {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
                self.dark = false;
                self.f = 0;
            }
        }
        let st = line_solid_default(self.dark);
        kabl_ui::theme::ensure_fonts(&ctx);
        kit::apply(&ctx, &st);
        let _ = code_notice::show(&ctx, &st, &self.code);
        self.f += 1;
        if self.f == 12 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        ctx.request_repaint();
    }
}

fn main() {
    let dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "target/code-notice".into()),
    );
    std::fs::create_dir_all(&dir).unwrap();
    let code = vec![
        CodeEntry {
            module_id: "demo.bitfilter".into(),
            version: 3,
            hash: "9f2c41d07a6be3158c0d4e7a21b9f8630a5d17c2e4b86f90d3a15c7e82b40d69".into(),
        },
        CodeEntry {
            module_id: "demo.logic".into(),
            version: 1,
            hash: "04b7e19a3c58d2f6a71e0b94c3d85f2e60a9b14d7c3e58f1a2b69d04c7e83a15".into(),
        },
    ];
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "code notice",
        opts,
        Box::new(move |cc| {
            cc.egui_ctx.set_pixels_per_point(1.0);
            Ok(Box::new(App {
                dir,
                dark: true,
                f: 0,
                code,
            }))
        }),
    )
    .unwrap();
}
