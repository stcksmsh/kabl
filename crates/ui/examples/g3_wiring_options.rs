//! Split / Join (90 px wide) label options for the owner: A short labels at the current jack
//! pitch, B no labels and a wider jack pitch. One sheet per app theme, rows = zoom 100 / 75 / 58.
//!
//! `xvfb-run -a -s "-screen 0 1100x760x24" cargo run --release -p kabl-ui --example g3_wiring_options -- graphite out.png`

use egui::{pos2, vec2, Align2, Color32, FontId, Painter, Rect};
use kabl_ui::kit::face;
use kabl_ui::style::{compose, Accent, Font, Style};
use std::sync::Arc;

const FACE_W: f32 = 90.0;
const FACE_H: f32 = 340.0;
const JACK_Y: f32 = 316.0;
const LABEL_FLOOR: f32 = 9.5;

struct Option_ {
    title: &'static str,
    caption: &'static str,
    /// Jack labels left to right, empty = labels dropped.
    labels: [&'static str; 3],
    pitch: f32,
    /// Which of the three jacks are stereo.
    stereo: [bool; 3],
}

struct App {
    st: Style,
    out: String,
    frames: u32,
}

fn draw_face(p: &Painter, st: &Style, o: egui::Pos2, z: f32, f: &Option_) {
    let rect = Rect::from_min_size(o, vec2(FACE_W, FACE_H) * z);
    face::surface(p, st, rect, st.roles.surface, st.roles.line, z);
    let ink = face::family_ink(st, "split", st.roles.text);
    let tf = FontId::new(14.0 * z, Font::SansCondSemibold.family(true));
    face::cond_draw(p, o + vec2(12.0, 12.0) * z, f.title, &tf, ink);
    let lf = FontId::new((11.0 * z).max(LABEL_FLOOR), Font::SansMedium.family(true));
    let cx = FACE_W / 2.0;
    for k in 0..3 {
        let x = cx + (k as f32 - 1.0) * f.pitch;
        let c = o + vec2(x, JACK_Y) * z;
        face::jack_ch(
            p,
            st,
            c,
            13.0 * z,
            z,
            st.roles.audio,
            false,
            false,
            f.stereo[k],
        );
        if !f.labels[k].is_empty() {
            p.text(
                c - vec2(0.0, 13.0 * z + 3.0 + 6.0),
                Align2::CENTER_CENTER,
                f.labels[k],
                lf.clone(),
                st.roles.text2,
            );
        }
    }
}

impl App {
    fn sheet(&self, p: &Painter) {
        let st = &self.st;
        let split = |title, caption, labels, pitch| Option_ {
            title,
            caption,
            labels,
            pitch,
            stereo: [true, false, false],
        };
        let join = |title, caption, labels, pitch| Option_ {
            title,
            caption,
            labels,
            pitch,
            stereo: [false, false, true],
        };
        let opts = [
            split("SPLIT", "A labels", ["In", "L", "R"], 28.0),
            split("SPLIT", "B no labels", ["", "", ""], 30.0),
            join("JOIN", "A labels", ["L", "R", "Out"], 28.0),
            join("JOIN", "B no labels", ["", "", ""], 30.0),
        ];
        let cap = FontId::new(11.0, Font::Mono.family(true));
        // 100 % on the left, 75 % and 58 % stacked on the right.
        for (z, x0, y0) in [
            (1.0_f32, 30.0, 20.0),
            (0.75, 580.0, 20.0),
            (0.58, 580.0, 340.0),
        ] {
            let mut x = x0;
            for f in &opts {
                draw_face(p, st, pos2(x, y0), z, f);
                p.text(
                    pos2(x + FACE_W * z / 2.0, y0 + FACE_H * z + 12.0),
                    Align2::CENTER_CENTER,
                    format!("{} {:.0}%", f.caption, z * 100.0),
                    cap.clone(),
                    st.roles.text2,
                );
                x += FACE_W * z + 40.0;
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let p = ui.painter().clone();
        p.rect_filled(Rect::EVERYTHING, 0.0, self.st.roles.rack);
        self.sheet(&p);
        self.frames += 1;
        if self.frames == 4 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(Arc::clone(image)),
                _ => None,
            })
        });
        if let Some(img) = shot {
            let [w, h] = img.size;
            let raw: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            image::save_buffer(&self.out, &raw, w as u32, h as u32, image::ColorType::Rgba8)
                .unwrap();
            println!("wrote {} ({w}x{h})", self.out);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint();
    }
}

fn main() {
    let mut a = std::env::args().skip(1);
    let theme = a.next().unwrap_or_else(|| "graphite".into());
    let out = a
        .next()
        .unwrap_or_else(|| format!("target/mock/wiring-{theme}.png"));
    if let Some(d) = std::path::Path::new(&out).parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let mut st = compose("line_solid", theme != "paper", Accent::Amber).expect("line_solid");
    st.fonts_ready = true;
    let _: Color32 = st.roles.rack;
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 760.0])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "g3-wiring",
        opts,
        Box::new(move |cc| {
            kabl_ui::theme::install_fonts(&cc.egui_ctx);
            cc.egui_ctx.set_pixels_per_point(1.0);
            Ok(Box::new(App { st, out, frames: 0 }))
        }),
    )
    .unwrap();
}
