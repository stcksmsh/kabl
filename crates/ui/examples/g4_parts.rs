//! G4 parts sheet: every `kit::g4` part and the stock kit parts it builds on, in both default
//! looks (`graphite`, `paper` x `line_solid`) at 1440x900. Writes `DIR/g4-parts__<theme>__1440x900.png`
//! and prints the contrast table of the state colours.
//!
//! `xvfb-run -a -s "-screen 0 1440x900x24" cargo run --release -p kabl-ui --example g4_parts -- DIR`
use eframe::egui;
use egui::{pos2, vec2, Rect};
use kabl_ui::kit::{self, g4, Ic, Tone};
use kabl_ui::style::{contrast, line_solid_default, Role, Style};
use std::path::PathBuf;

struct App {
    dir: PathBuf,
    dark: bool,
    f: u32,
    on: bool,
    text: String,
    val: f64,
    bars: Vec<f32>,
    pitch: Vec<f32>,
    gate: Vec<f32>,
}

fn card(ui: &mut egui::Ui, st: &Style, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    kit::card(ui, st, |ui| {
        ui.set_width(ui.available_width());
        kit::section(ui, st, title);
        kit::gap(ui, st, 1);
        add(ui);
    });
    kit::gap(ui, st, 1);
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        for e in ctx.input(|i| i.raw.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = e {
                let [w, h] = image.size;
                let raw: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                let name = format!(
                    "g4-parts__{}__1440x900.png",
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
        let area = Rect::from_min_size(pos2(0.0, 0.0), vec2(1440.0, 900.0));
        g4::dock(ui, &st, "sheet-dock", 210.0, 120.0, 400.0, |ui| {
            kit::label(ui, &st, Role::H3, Tone::Text, "Wavetable editor");
            kit::label(
                ui,
                &st,
                Role::Body,
                Tone::Text2,
                "Bottom dock, drag the handle to resize.",
            );
        });
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(st.roles.bg).inner_margin(16))
            .show(ui, |ui| {
                ui.columns(3, |c| {
                    let (a, rest) = c.split_at_mut(1);
                    let (b, d) = rest.split_at_mut(1);
                    let (a, b, d) = (&mut a[0], &mut b[0], &mut d[0]);
                    card(a, &st, "Buttons", |ui| {
                        ui.horizontal(|ui| {
                            ui.add(kit::Button::new(&st, "Primary").primary());
                            ui.add(kit::Button::new(&st, "Secondary"));
                            ui.add(kit::Button::new(&st, "Ghost").ghost());
                            ui.add(kit::Button::new(&st, "Delete").ghost().tone(Tone::Bad));
                        });
                        ui.horizontal(|ui| {
                            ui.add(kit::Button::new(&st, "Open").icon(Ic::Folder));
                            for ic in [Ic::Close, Ic::Trash, Ic::Copy, Ic::External] {
                                kit::icon_button(ui, &st, ic, false, true);
                            }
                        });
                    });
                    card(a, &st, "Fields and choices", |ui| {
                        kit::field(
                            ui,
                            &st,
                            &mut self.text,
                            "Search sounds",
                            Some(Ic::Search),
                            None,
                        );
                        kit::gap(ui, &st, 1);
                        ui.horizontal(|ui| {
                            kit::segmented(ui, &st, |ui| {
                                kit::seg(ui, &st, "Controls", true);
                                kit::seg(ui, &st, "Face layout", false);
                            });
                            kit::tag(ui, &st, "ADVANCED", st.roles.accent);
                            kit::tag(ui, &st, "audio", st.roles.audio);
                        });
                        kit::gap(ui, &st, 1);
                        g4::checkbox(ui, &st, &mut self.on, "Checkbox");
                        let mut off = false;
                        g4::checkbox(ui, &st, &mut off, "Unchecked");
                        kit::toggle(ui, &st, &mut self.on, "Toggle");
                        let mut v = self.val;
                        kit::slider(ui, &st, &mut v, 0.0..=1.0, None, "0.42");
                    });
                    card(a, &st, "Icons", |ui| {
                        ui.horizontal_wrapped(|ui| {
                            use Ic::*;
                            for ic in [
                                File, Err, Back, Up, Home, Copy, Grip, Invert, Bypass, Trash, Plug,
                                Wave, Clock, External, Folder, Warn, Check, Pin, Star, Close,
                            ] {
                                let (r, _) =
                                    ui.allocate_exact_size(vec2(28.0, 28.0), egui::Sense::hover());
                                kit::icon(ui.painter(), ic, r.center(), 18.0, st.roles.text2);
                            }
                        });
                    });
                    card(b, &st, "Step grid", |ui| {
                        let w = ui.available_width() - 8.0;
                        g4::step_grid(
                            ui,
                            &st,
                            &mut [
                                g4::Lane::new(
                                    "pitch",
                                    g4::LaneKind::Bars { bipolar: true },
                                    &mut self.pitch,
                                ),
                                g4::Lane::new(
                                    "vel",
                                    g4::LaneKind::Bars { bipolar: false },
                                    &mut self.bars,
                                ),
                                g4::Lane::new("gate", g4::LaneKind::Gate, &mut self.gate),
                            ],
                            Some(2),
                            w,
                            true,
                        );
                    });
                    card(b, &st, "Range bar", |ui| {
                        let w = ui.available_width() - 8.0;
                        g4::range_bar(
                            ui,
                            &st,
                            w,
                            g4::Range {
                                min: 20.0,
                                max: 20000.0,
                                base: 1000.0,
                                lo: 600.0,
                                hi: 7940.0,
                                now: 2400.0,
                            },
                            |v| format!("{v:.0} Hz"),
                        );
                    });
                    card(b, &st, "Banners", |ui| {
                        g4::banner(
                            ui,
                            &st,
                            g4::Sev::Bad,
                            "Could not write the sound",
                            Some("permission denied"),
                        );
                        kit::gap(ui, &st, 1);
                        g4::banner(ui, &st, g4::Sev::Warn, "Sample rate differs", None);
                        kit::gap(ui, &st, 1);
                        g4::banner(ui, &st, g4::Sev::Good, "Saved to Your Sounds", None);
                    });
                    card(d, &st, "Empty state", |ui| {
                        g4::empty_state(
                            ui,
                            &st,
                            Ic::Plug,
                            "Nothing patched",
                            "Drag a module from the library, or load a sound.",
                            &[
                                g4::EmptyAction {
                                    label: "Open library",
                                    primary: true,
                                    icon: Ic::Plus,
                                },
                                g4::EmptyAction {
                                    label: "New patch",
                                    primary: false,
                                    icon: Ic::Plus,
                                },
                            ],
                            None,
                        );
                    });
                    card(d, &st, "Menu", |ui| {
                        kit::label(ui, &st, Role::H3, Tone::Text, "Sequencer #2");
                        kit::menu_rule(ui, &st);
                        kit::menu_item(ui, &st, "Explain this module", false);
                        kit::menu_item(ui, &st, "Edit face…", true);
                        kit::menu_item_tone(ui, &st, "Remove module", false, Tone::Bad, true);
                    });
                    kit::label(
                        d,
                        &st,
                        Role::Caption,
                        Tone::Text2,
                        "Toasts, modal, tool window and dock: floating, below.",
                    );
                });
            });
        let t = self.f as f64;
        let list = [
            g4::Toast {
                sev: g4::Sev::Bad,
                title: "Could not load patch".into(),
                detail: Some("unknown module kind \"osc.xyz\"".into()),
                actions: vec!["Show details".into(), "Open another…".into()],
                expanded: false,
                born: t,
            },
            g4::Toast {
                sev: g4::Sev::Good,
                title: "Saved to Your Sounds".into(),
                detail: None,
                actions: vec![],
                expanded: false,
                born: t,
            },
        ];
        let toast_area = Rect::from_min_size(pos2(1000.0, 500.0), vec2(400.0, 300.0));
        egui::Area::new(egui::Id::new("sheet-toasts"))
            .fixed_pos(toast_area.min)
            .show(&ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                for t in &list {
                    g4::toast(ui, &st, t);
                }
            });
        let mut open = true;
        g4::tool_window(
            &ctx,
            &st,
            "sheet-tool",
            ("Panel authoring", Some("ADVANCED")),
            pos2(40.0, 500.0),
            &mut open,
            |ui| {
                kit::label(ui, &st, Role::Body, Tone::Text2, "Same kit, ADVANCED tag.");
            },
        );
        let _ = area;
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
            .unwrap_or_else(|| "target/g4-parts".into()),
    );
    std::fs::create_dir_all(&dir).unwrap();
    for dark in [true, false] {
        let st = line_solid_default(dark);
        let r = &st.roles;
        for (n, c) in [
            ("bad", r.bad),
            ("good", r.good),
            ("warn", r.warn),
            ("accent", r.accent),
        ] {
            println!(
                "{} {n:6} on surface {:.2}  on raised {:.2}  on bg {:.2}",
                if dark { "graphite" } else { "paper   " },
                contrast(c, r.surface),
                contrast(c, r.raised),
                contrast(c, r.bg)
            );
        }
    }
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "g4 parts",
        opts,
        Box::new(move |cc| {
            cc.egui_ctx.set_pixels_per_point(1.0);
            Ok(Box::new(App {
                dir,
                dark: true,
                f: 0,
                on: true,
                text: String::new(),
                val: 0.42,
                bars: vec![0.3, 0.7, 0.5, 0.9, 0.4, 0.6, 0.8, 0.2],
                pitch: vec![0.0, 0.5, -0.3, 0.8, 0.0, -0.6, 0.4, 0.2],
                gate: vec![1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 1.0],
            }))
        }),
    )
    .unwrap();
}
