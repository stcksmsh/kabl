//! The G1b parts sheet drawn by the app's kit (`kit::face`, `kit::parts`) with the composed
//! `line_solid` style, to compare against the mock's sheet (`docs/design/redesign/g1b`).
//!
//! `xvfb-run -a -s "-screen 0 1100x800x24" cargo run --release -p kabl-ui --example kit_parts -- graphite out.png`
//! (`paper` for the light app theme). The window is 1000 x 740 px; the mock sheet's parts panel
//! starts at y 3502.8, so the panel here is drawn 0.2 px up to match its anti-aliasing.

use egui::{pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind};
use kabl_ui::kit::{face, parts};
use kabl_ui::style::{compose, Accent, Font, Style};
use std::sync::Arc;

struct App {
    st: Style,
    out: String,
    frames: u32,
}

fn mono(px: f32) -> FontId {
    FontId::new(px, Font::Mono.family(true))
}

impl App {
    fn heading(&self, p: &Painter, o: Pos2, x: f32, y: f32, s: &str) {
        let f = FontId::new(14.0, Font::SansCondSemibold.family(true));
        face::cond_draw(p, o + vec2(x, y), s, &f, self.st.sections.fx.ink);
    }

    fn caption(&self, p: &Painter, o: Pos2, x: f32, y: f32, s: &str) {
        p.text(
            o + vec2(x, y),
            Align2::CENTER_CENTER,
            s,
            mono(11.0),
            self.st.roles.text2,
        );
    }

    fn sheet(&self, p: &Painter, o: Pos2) {
        let st = &self.st;
        let r = &st.roles;
        let panel = Rect::from_min_size(o, vec2(1000.0, 740.0));
        p.rect_filled(panel, 3.0, r.surface);
        p.rect_stroke(
            panel,
            3.0,
            Stroke::new(
                1.0,
                egui::Color32::from_rgba_unmultiplied(
                    r.text.r(),
                    r.text.g(),
                    r.text.b(),
                    if st.dark { 66 } else { 77 },
                ),
            ),
            StrokeKind::Inside,
        );
        // knobs
        self.heading(p, o, 16.0, 20.0, "KNOB  S 16 · M 20 · L 24 · states");
        let mut x = 50.0;
        for (rad, label, v) in [
            (16.0, "r16", 0.3),
            (20.0, "r20", 0.6),
            (24.0, "r24", 0.8),
            (20.0, "hover", 0.5),
            (20.0, "focus", 0.5),
            (20.0, "mod", 0.4),
            (20.0, "disabled", 0.5),
        ] {
            face::knob(
                p,
                st,
                &face::Knob {
                    c: o + vec2(x, 80.0),
                    rs: rad,
                    z: 1.0,
                    value: v,
                    hot: label == "focus",
                },
                r.text2,
            );
            self.caption(p, o, x, 125.0, label);
            x += 90.0;
        }
        // selector, toggle, chip
        self.heading(p, o, 16.0, 160.0, "SELECTOR · TOGGLE · CHIP");
        parts::selector(
            p,
            st,
            Rect::from_min_size(o + vec2(20.0, 180.0), vec2(208.0, 20.0)),
            &["SIN", "TRI", "SAW", "SQR"],
            2,
            1.0,
            None,
        );
        parts::selector(
            p,
            st,
            Rect::from_min_size(o + vec2(260.0 + 36.0, 181.0), vec2(104.0, 18.0)),
            &["A", "B", "C", "D"],
            0,
            1.0,
            Some(r.gate),
        );
        parts::toggle(
            p,
            st,
            Rect::from_min_size(o + vec2(470.0, 180.0), vec2(36.0, 20.0)),
            true,
        );
        parts::toggle(
            p,
            st,
            Rect::from_min_size(o + vec2(520.0, 180.0), vec2(36.0, 20.0)),
            false,
        );
        parts::chip(
            p,
            st,
            Rect::from_min_size(o + vec2(580.0, 181.0), vec2(36.0, 18.0)),
            "+7",
            1.0,
        );
        // jacks
        self.heading(
            p,
            o,
            16.0,
            230.0,
            "JACK  audio · cv · gate · pitch · stereo · patched · hover",
        );
        let mut x = 40.0;
        for (sig, stereo, patched, hot, name) in [
            (r.audio, false, false, false, "audio"),
            (r.cv, false, false, false, "cv"),
            (r.gate, false, false, false, "gate"),
            (r.pitch, false, false, false, "pitch"),
            (r.audio, true, false, false, "stereo"),
            (r.audio, false, true, false, "patched"),
            (r.cv, false, false, true, "hover"),
        ] {
            face::jack_ch(
                p,
                st,
                o + vec2(x, 275.0),
                13.0,
                1.0,
                sig,
                patched,
                hot,
                stereo,
            );
            self.caption(p, o, x, 305.0, name);
            x += 80.0;
        }
        // cables
        self.heading(
            p,
            o,
            16.0,
            340.0,
            "CABLE  rest · lit · stereo · functional (step beads)",
        );
        let ends = |x0: f32| (o + vec2(x0, 360.0), o + vec2(x0 + 200.0, 370.0));
        let (a, b) = ends(20.0);
        parts::cable(p, st, a, b, 60.0, r.audio, false, false);
        let (a, b) = ends(260.0);
        parts::cable(p, st, a, b, 60.0, r.cv, true, false);
        let (a, b) = ends(500.0);
        parts::cable(p, st, a, b, 60.0, r.audio, false, true);
        let (a, b) = ends(740.0);
        parts::cable(p, st, a, b, 60.0, r.gate, false, false);
        parts::cable_beads(
            p,
            st,
            a,
            b,
            60.0,
            r.gate,
            &[true, false, true, true, false, true, true, false],
        );
        // family marker
        self.heading(
            p,
            o,
            16.0,
            470.0,
            "FAMILY MARKER  title colour (line styles) · 3 px stripe (solid styles)",
        );
        let secs = [
            (&st.sections.osc, "osc"),
            (&st.sections.filter, "filter"),
            (&st.sections.modulation, "env"),
            (&st.sections.timing, "timing"),
            (&st.sections.amp, "amp"),
            (&st.sections.fx, "fx"),
        ];
        for (i, (s, name)) in secs.iter().enumerate() {
            let x = 20.0 + i as f32 * 160.0;
            let rr = Rect::from_min_size(o + vec2(x, 490.0), vec2(140.0, 60.0));
            p.rect_filled(rr, 3.0, r.surface);
            p.rect_stroke(
                rr,
                3.0,
                Stroke::new(
                    1.0,
                    egui::Color32::from_rgba_unmultiplied(
                        r.text.r(),
                        r.text.g(),
                        r.text.b(),
                        if st.dark { 66 } else { 77 },
                    ),
                ),
                StrokeKind::Inside,
            );
            p.text(
                o + vec2(x + 10.0, 506.0),
                Align2::LEFT_CENTER,
                name,
                FontId::new(14.0, Font::SansCondSemibold.family(true)),
                s.ink,
            );
            p.rect_filled(
                Rect::from_min_size(o + vec2(x + 1.0, 491.0), vec2(138.0, 3.0)),
                1.0,
                s.ink,
            );
            p.text(
                o + vec2(x + 10.0, 530.0),
                Align2::LEFT_CENTER,
                "title / stripe",
                mono(11.0),
                r.text2,
            );
        }
        // trays
        self.heading(
            p,
            o,
            16.0,
            575.0,
            "TRAY  two_row (< 480 px) · single_row_wide (>= 480 px)",
        );
        parts::tray(
            p,
            st,
            Rect::from_min_size(o + vec2(20.0, 590.0), vec2(200.0, 86.0)),
            Some(Rect::from_min_size(
                o + vec2(20.0, 634.0),
                vec2(200.0, 42.0),
            )),
            1.0,
        );
        for i in 0..3 {
            face::jack_ch(
                p,
                st,
                o + vec2(50.0 + i as f32 * 60.0, 615.0),
                13.0,
                1.0,
                r.audio,
                false,
                false,
                false,
            );
            face::jack_ch(
                p,
                st,
                o + vec2(50.0 + i as f32 * 60.0, 656.0),
                13.0,
                1.0,
                r.gate,
                false,
                false,
                false,
            );
        }
        parts::tray(
            p,
            st,
            Rect::from_min_size(o + vec2(260.0, 620.0), vec2(400.0, 50.0)),
            Some(Rect::from_min_size(
                o + vec2(500.0, 620.0),
                vec2(160.0, 50.0),
            )),
            1.0,
        );
        for i in 0..3 {
            face::jack_ch(
                p,
                st,
                o + vec2(295.0 + i as f32 * 56.0, 645.0),
                13.0,
                1.0,
                r.gate,
                false,
                false,
                false,
            );
        }
        for i in 0..2 {
            face::jack_ch(
                p,
                st,
                o + vec2(535.0 + i as f32 * 56.0, 645.0),
                13.0,
                1.0,
                r.audio,
                false,
                false,
                false,
            );
        }
        // drum cell
        self.heading(
            p,
            o,
            700.0,
            575.0,
            "DRUM CELL  off · 4 velocities · 3 chances",
        );
        let well = Rect::from_min_size(o + vec2(700.0, 590.0), vec2(280.0, 60.0));
        p.rect_filled(well, 4.0, r.inset);
        p.rect_stroke(well, 4.0, Stroke::new(1.0, r.line2), StrokeKind::Inside);
        for (i, (lvl, chance)) in [
            (0, 0),
            (1, 0),
            (2, 0),
            (3, 0),
            (4, 0),
            (4, 1),
            (4, 2),
            (4, 3),
        ]
        .into_iter()
        .enumerate()
        {
            let cell =
                Rect::from_min_size(o + vec2(708.0 + i as f32 * 33.0, 600.0), vec2(24.0, 40.0));
            parts::drum_cell(p, st, cell, lvl, chance, 1.0);
            if lvl == 0 {
                p.rect_stroke(
                    cell,
                    2.0,
                    Stroke::new(1.0, face_alpha(r.text, 40)),
                    StrokeKind::Inside,
                );
            }
        }
    }
}

fn face_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let p = ui.painter().clone();
        p.rect_filled(Rect::EVERYTHING, 0.0, self.st.roles.bg);
        self.sheet(&p, pos2(0.0, -0.2));
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
        .unwrap_or_else(|| format!("target/mock/kit-{theme}.png"));
    if let Some(d) = std::path::Path::new(&out).parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let mut st = compose("line_solid", theme != "paper", Accent::Amber).expect("line_solid");
    st.fonts_ready = true;
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 740.0])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "kit-parts",
        opts,
        Box::new(move |cc| {
            kabl_ui::theme::install_fonts(&cc.egui_ctx);
            cc.egui_ctx.set_pixels_per_point(1.0);
            Ok(Box::new(App { st, out, frames: 0 }))
        }),
    )
    .unwrap();
}
