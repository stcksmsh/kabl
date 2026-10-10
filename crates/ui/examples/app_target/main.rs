//! Design-target renderer: draws kabl's whole-application directions with egui's painter and
//! saves real framebuffer captures. No production UI code is touched.
//!
//!   xvfb-run -a cargo run --release -p kabl-ui --example app_target -- stills --out DIR
//!   xvfb-run -a cargo run --release -p kabl-ui --example app_target -- video  --out DIR
//!   cargo run --release -p kabl-ui --example app_target -- tokens | check
mod chrome;
mod closeup;
mod composition;
mod displays;
mod faces;
mod perform;
mod prim;
mod rack;
mod report;
mod scenes;
mod tokens;
mod widgets;

use eframe::egui;
use egui::{Event, LayerId, Order};
use scenes::Scene;
use std::path::PathBuf;
use tokens::Dir;

struct Job {
    dir: Dir,
    dark: bool,
    scene: Scene,
    t: f32,
    file: PathBuf,
}

struct App {
    jobs: Vec<Job>,
    i: usize,
    f: u32,
    size: egui::Vec2,
    data: scenes::Ctxs,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        for e in ctx.input(|i| i.raw.events.clone()) {
            if let Event::Screenshot { image, .. } = e {
                let j = &self.jobs[self.i];
                if let Some(d) = j.file.parent() {
                    std::fs::create_dir_all(d).unwrap();
                }
                let [w, h] = image.size;
                let buf: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                image::save_buffer(
                    &j.file,
                    &buf,
                    w as u32,
                    h as u32,
                    image::ExtendedColorType::Rgba8,
                )
                .unwrap();
                eprintln!("{} ({w}x{h})", j.file.display());
                self.i += 1;
                self.f = 0;
            }
        }
        if self.i >= self.jobs.len() {
            std::process::exit(0);
        }
        let j = &self.jobs[self.i];
        let k = tokens::tok(j.dir, j.dark);
        let p = ctx.layer_painter(LayerId::new(Order::Background, egui::Id::new("scene")));
        let cx = prim::Cx {
            p: &p,
            k: &k,
            t: j.t,
        };
        scenes::draw(&cx, &self.data, self.size, j.scene);
        self.f += 1;
        if self.f == 3 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        ctx.request_repaint();
    }
}

fn arg(a: &[String], name: &str) -> Option<String> {
    a.iter()
        .position(|x| x == name)
        .and_then(|i| a.get(i + 1))
        .cloned()
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.first().map(String::as_str) {
        Some("tokens") => return report::tokens_md(),
        Some("check") => return report::check(),
        _ => {}
    }
    let mode = a.first().cloned().unwrap_or_else(|| "stills".into());
    let out = PathBuf::from(arg(&a, "--out").unwrap_or_else(|| "out".into()));
    let sizes: Vec<(u32, u32)> = arg(&a, "--size")
        .unwrap_or_else(|| "1440x900".into())
        .split(',')
        .filter_map(|s| {
            s.split_once('x')
                .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
        })
        .collect();
    let (w, h) = sizes[0];
    let dirs: Vec<Dir> = arg(&a, "--dir").map_or(Dir::ALL.to_vec(), |s| {
        s.split(',').filter_map(Dir::parse).collect()
    });
    let themes: Vec<bool> = arg(&a, "--theme").map_or(vec![false, true], |s| {
        s.split(',').map(|t| t == "dark").collect()
    });
    let scenes_sel: Vec<Scene> = arg(&a, "--scene").map_or(Scene::ALL.to_vec(), |s| {
        if s == "all" {
            Scene::ALL.to_vec()
        } else {
            s.split(',').filter_map(Scene::parse).collect()
        }
    });
    let t0: f32 = arg(&a, "--t").and_then(|s| s.parse().ok()).unwrap_or(2.3);
    let mut jobs = vec![];
    if mode == "video" {
        let fps = 30.0;
        for &dir in &dirs {
            let dark = themes[0];
            for (n, (scene, t)) in scenes::timeline(fps).into_iter().enumerate() {
                jobs.push(Job {
                    dir,
                    dark,
                    scene,
                    t,
                    file: out.join(dir.slug()).join(format!("f{n:04}.png")),
                });
            }
        }
    } else {
        for &dir in &dirs {
            for &dark in &themes {
                for &scene in &scenes_sel {
                    let name = format!(
                        "{w}x{h}-{}-{}.png",
                        scene.slug(),
                        if dark { "dark" } else { "light" }
                    );
                    jobs.push(Job {
                        dir,
                        dark,
                        scene,
                        t: t0,
                        file: out.join(dir.slug()).join(name),
                    });
                }
            }
        }
    }
    let size = egui::vec2(w as f32, h as f32);
    let o = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(size)
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "kabl design target",
        o,
        Box::new(move |cc| {
            prim::install_fonts(&cc.egui_ctx);
            cc.egui_ctx.set_pixels_per_point(1.0);
            Ok(Box::new(App {
                jobs,
                i: 0,
                f: 0,
                size,
                data: scenes::Ctxs::new(),
            }))
        }),
    )
    .unwrap();
}
