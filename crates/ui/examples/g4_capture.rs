//! G4 brief captures: surfaces outside the rack faces. One window size per run.
//!
//! `xvfb-run -a -s "-screen 0 1440x900x24" cargo run --release -p kabl-ui --example g4_capture -- 1440x900 DIR [scene,..]`
//! Writes `DIR/<scene>__<graphite|paper>__<WxH>.png`. Silent: no audio device is opened.
use eframe::egui;
use egui::{Event, PointerButton, Pos2, Rect};
use kabl_core::{PortRef, Vec2};
use kabl_ui::{show, PatchEditor, UiState};
use std::path::PathBuf;

type Setup = fn(&mut PatchEditor, &mut UiState);
/// Right-click (or left-click) the hit rect with this key after the setup settled.
type Click = Option<(&'static str, bool)>;

struct Scene {
    name: &'static str,
    empty: bool,
    setup: Setup,
    click: Click,
    late: Option<Setup>,
}

fn jack(id: u64, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

fn find(e: &PatchEditor, kind: &str) -> u64 {
    *e.state()
        .modules
        .iter()
        .find(|(_, m)| m.kind == kind)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {kind}"))
}

fn patch() -> PatchEditor {
    let mut e = PatchEditor::seed_from(&Default::default());
    let at = |x: f32, y: f32| Vec2 { x, y };
    let midi = e.add_module("midi.in", at(20.0, 20.0));
    let seq = e.add_module("seq", at(240.0, 20.0));
    let wt = e.add_module("osc.wt", at(560.0, 20.0));
    let filter = e.add_module("filter.svf", at(20.0, 380.0));
    let env = e.add_module("env.adsr", at(240.0, 380.0));
    let vca = e.add_module("vca", at(480.0, 380.0));
    let out = e.add_module("out", at(700.0, 380.0));
    let _cues = e.add_module("cues", at(860.0, 20.0));
    let _kick = e.add_module("drum.kick", at(860.0, 380.0));
    let _lfo = e.add_module("lfo", at(1100.0, 380.0));
    e.connect(jack(seq, "pitch"), jack(wt, "pitch"));
    e.connect(jack(wt, "out"), jack(filter, "in"));
    e.connect(jack(filter, "lp"), jack(vca, "in"));
    e.connect(jack(seq, "gate"), jack(env, "gate"));
    e.connect(jack(env, "out"), jack(vca, "cv"));
    e.connect(jack(vca, "out"), jack(out, "in"));
    let c = e.connect_route(jack(env, "out"), filter, "cutoff_hz");
    e.set_route_amount(c, 0.3, true);
    let _ = midi;
    e
}

impl Scene {
    fn late(mut self, f: Setup) -> Self {
        self.late = Some(f);
        self
    }
}

fn none(_: &mut PatchEditor, _: &mut UiState) {}

fn scenes() -> Vec<Scene> {
    let s = |name, setup: Setup, click| Scene {
        name,
        empty: false,
        setup,
        click,
        late: None,
    };
    vec![
        Scene {
            name: "empty-rack",
            empty: true,
            setup: none,
            click: None,
            late: None,
        },
        s("rack-with-drawer-closed", none, None),
        s(
            "wavetable-editor",
            |e, u| {
                let id = find(e, "osc.wt");
                kabl_ui::tables::open(e, u, id);
            },
            None,
        ),
        s(
            "about-dialog",
            |_, u| u.about.open = true,
            None,
        ),
        s(
            "file-open-dialog",
            |_, u| {
                kabl_ui::filebrowser::open(
                    u,
                    kabl_ui::filebrowser::Purpose::PatchLoad,
                    "Open patch",
                    kabl_ui::filebrowser::Mode::Open,
                    kabl_ui::filebrowser::Spec {
                        exts: &["kabl"],
                        what: ".kabl patches",
                    },
                    "",
                    "/secondary/Programming/Github/kabl/docs",
                );
            },
            None,
        ),
        s(
            "file-save-dialog",
            |_, u| {
                kabl_ui::filebrowser::open(
                    u,
                    kabl_ui::filebrowser::Purpose::PatchSave,
                    "Save patch",
                    kabl_ui::filebrowser::Mode::Save,
                    kabl_ui::filebrowser::Spec {
                        exts: &["kabl"],
                        what: ".kabl patches",
                    },
                    "my-patch.kabl",
                    "/secondary/Programming/Github/kabl/docs",
                );
            },
            None,
        ),
        s(
            "file-empty-folder",
            |_, u| {
                kabl_ui::filebrowser::open(
                    u,
                    kabl_ui::filebrowser::Purpose::PatchLoad,
                    "Open patch",
                    kabl_ui::filebrowser::Mode::Open,
                    kabl_ui::filebrowser::Spec {
                        exts: &["kabl"],
                        what: ".kabl patches",
                    },
                    "",
                    "/secondary/Programming/Github/kabl/.worktrees/g4-brief/packaging",
                );
            },
            None,
        ),
        s(
            "dialog-unsaved",
            |_, u| {
                u.browser.dialog = Some(kabl_ui::browser::Dialog::Unsaved {
                    then: kabl_ui::browser::Pending::New,
                    error: None,
                })
            },
            None,
        ),
        s(
            "dialog-save-as",
            |_, u| {
                u.browser.dialog = Some(kabl_ui::browser::Dialog::SaveAs {
                    name: "Warm pad".into(),
                    category: "Pad".into(),
                    tags: "slow, wide".into(),
                    taken: None,
                    then: None,
                    error: None,
                })
            },
            None,
        ),
        s(
            "dialog-save-as-error",
            |_, u| {
                u.browser.dialog = Some(kabl_ui::browser::Dialog::SaveAs {
                    name: "Warm pad".into(),
                    category: "Pad".into(),
                    tags: "".into(),
                    taken: Some(("x".into(), "Warm pad".into())),
                    then: None,
                    error: Some("Could not write the sound: permission denied".into()),
                })
            },
            None,
        ),
        s(
            "dialog-rename",
            |_, u| {
                u.browser.dialog = Some(kabl_ui::browser::Dialog::Rename {
                    id: "x".into(),
                    name: "Warm pad".into(),
                    error: None,
                })
            },
            None,
        ),
        s(
            "dialog-restore",
            |_, u| u.browser.dialog = Some(kabl_ui::browser::Dialog::Restore),
            None,
        ),
        s(
            "context-menu-module",
            none,
            Some(("module:SEQ", true)),
        ),
        s(
            "more-controls-and-edit-face",
            |e, u| {
                let id = find(e, "filter.svf");
                u.choose = Some((id, vec![true; 16]));
            },
            None,
        ),
        s(
            "more-controls-modal",
            none,
            None,
        )
        .late(|e, u| {
            u.expanded.insert(find(e, "osc.wt"));
            u.float_expansion = true;
        }),
        s(
            "composites-instance-tools",
            |e, u| {
                let m = [find(e, "filter.svf"), find(e, "env.adsr"), find(e, "vca")]
                    .into_iter()
                    .collect();
                kabl_ui::composites::encapsulate(e, m, Default::default(), "Voice").unwrap();
                let _ = u;
            },
            None,
        )
        .late(|_, u| u.composites.panel = true),
        s(
            "panel-authoring",
            |e, u| {
                let m = [find(e, "filter.svf"), find(e, "env.adsr"), find(e, "vca")]
                    .into_iter()
                    .collect();
                kabl_ui::composites::encapsulate(e, m, Default::default(), "Voice").unwrap();
                let _ = u;
            },
            None,
        )
        .late(|e, u| {
            let (&id, c) = e.state().composites.iter().next().unwrap();
            kabl_ui::panels::open(u, id, c);
        }),
        s("cable-pattern-editor", none, None).late(|e, u| {
            let id = *e.state().cables.keys().next().unwrap();
            u.cable_fn = Some(id);
            u.drawer_open = true;
        }),
        s("context-menu-cable", none, Some(("cable:*", true))),
        s(
            "seq-pattern-editor",
            |e, u| {
                let id = find(e, "seq");
                u.selected_module = Some(id);
                u.drawer_open = true;
            },
            None,
        ),
        s(
            "cues-editor",
            |e, u| {
                let id = find(e, "cues");
                u.selected_module = Some(id);
                u.drawer_open = true;
            },
            None,
        ),
        s(
            "cable-routes-drawer",
            |e, u| {
                let id = find(e, "filter.svf");
                u.inspected = Some((id, "cutoff_hz".into()));
                u.drawer_open = true;
            },
            None,
        ),
        s(
            "explain-panel",
            |e, u| {
                let id = find(e, "filter.svf");
                kabl_ui::explain::open(
                    u,
                    kabl_ui::explain::Subject::Control {
                        id,
                        key: "cutoff_hz".into(),
                    },
                    None,
                );
            },
            None,
        ),
        s(
            "compare-panel",
            |_, u| {
                u.compare.open = true;
                u.drawer_open = true;
            },
            None,
        ),
        s(
            "recipes-panel",
            |_, u| {
                u.recipes.open = true;
                u.drawer_open = true;
            },
            None,
        ),
        s(
            "signal-inspector",
            |_, u| {
                u.inspect.open = true;
                u.drawer_open = true;
            },
            None,
        ),
        s(
            "load-stopped-error",
            |_, u| {
                u.load_stopped = true;
                u.last_message = Some("Could not load patch: unknown module kind \"osc.xyz\"".into());
            },
            None,
        ),
    ]
}

struct App {
    scenes: Vec<Scene>,
    i: usize,
    f: u32,
    dark: bool,
    dir: PathBuf,
    size: (u32, u32),
    editor: PatchEditor,
    ui: UiState,
    queued: Vec<Event>,
    fresh: bool,
}

impl eframe::App for App {
    fn raw_input_hook(&mut self, _: &egui::Context, raw: &mut egui::RawInput) {
        raw.events.append(&mut self.queued);
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let mut shot = None;
        for e in ctx.input(|i| i.raw.events.clone()) {
            if let Event::Screenshot { image, .. } = e {
                shot = Some(image);
            }
        }
        if let Some(img) = shot {
            let sc = &self.scenes[self.i];
            let [w, h] = img.size;
            let raw: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            let full = image::RgbaImage::from_raw(w as u32, h as u32, raw).unwrap();
            let name = format!(
                "{}__{}__{}x{}.png",
                sc.name,
                if self.dark { "graphite" } else { "paper" },
                self.size.0,
                self.size.1
            );
            full.save(self.dir.join(&name)).unwrap();
            println!("wrote {name}");
            self.fresh = true;
            if !self.dark {
                self.i += 1;
            }
            self.dark = !self.dark;
        }
        if self.i >= self.scenes.len() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.fresh {
            self.fresh = false;
            self.f = 0;
            let sc = &self.scenes[self.i];
            self.editor = if sc.empty {
                PatchEditor::seed_from(&Default::default())
            } else {
                patch()
            };
            let mut u = UiState::default();
            u.browser_open = false;
            u.drawer_open = false;
            u.theme = "kabl.line_solid".into();
            self.ui = u;
            (sc.setup)(&mut self.editor, &mut self.ui);
        }
        self.ui.dark = self.dark;
        self.ui.zoom = 1.0;
        self.ui.pan = egui::vec2(12.0, 12.0);
        let mut root = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(self.size.0 as f32, self.size.1 as f32),
            )),
        );
        show(&mut self.editor, &mut self.ui, &mut root);
        self.f += 1;
        let sc = &self.scenes[self.i];
        if self.f == 6 && sc.click.is_some() {
            let k = egui::Key::Escape;
            let m = egui::Modifiers::NONE;
            self.queued = [true, false]
                .map(|pressed| Event::Key {
                    key: k,
                    physical_key: Some(k),
                    pressed,
                    repeat: false,
                    modifiers: m,
                })
                .to_vec();
        }
        if self.f == 10 {
            if let Some((key, secondary)) = sc.click {
                let key = if key == "module:SEQ" {
                    format!("module:{}", find(&self.editor, "seq"))
                } else if let Some(pre) = key.strip_suffix('*') {
                    self.ui
                        .hits
                        .keys()
                        .find(|k| k.starts_with(pre))
                        .cloned()
                        .unwrap_or_default()
                } else {
                    key.to_string()
                };
                if let Some(r) = self.ui.hits.get(&key) {
                    let p = r.center();
                    let b = if secondary {
                        PointerButton::Secondary
                    } else {
                        PointerButton::Primary
                    };
                    let m = egui::Modifiers::NONE;
                    self.queued = vec![
                        Event::PointerMoved(p),
                        Event::PointerButton {
                            pos: p,
                            button: b,
                            pressed: true,
                            modifiers: m,
                        },
                        Event::PointerButton {
                            pos: p,
                            button: b,
                            pressed: false,
                            modifiers: m,
                        },
                    ];
                }
            }
        }
        if self.f == 3 {
            if let Some(l) = self.scenes[self.i].late {
                l(&mut self.editor, &mut self.ui);
            }
        }
        if self.f == 22 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        ctx.request_repaint();
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (w, h) = a[0].split_once('x').unwrap();
    let size = (w.parse::<u32>().unwrap(), h.parse::<u32>().unwrap());
    let dir = PathBuf::from(&a[1]);
    std::fs::create_dir_all(&dir).unwrap();
    let want = a.get(2).cloned().unwrap_or_else(|| "all".into());
    let scenes: Vec<Scene> = scenes()
        .into_iter()
        .filter(|s| want == "all" || want.split(',').any(|w| w == s.name))
        .collect();
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([size.0 as f32, size.1 as f32])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "g4 capture",
        opts,
        Box::new(move |cc| {
            cc.egui_ctx.set_pixels_per_point(1.0);
            Ok(Box::new(App {
                scenes,
                i: 0,
                f: 0,
                dark: true,
                dir,
                size,
                editor: PatchEditor::seed_from(&Default::default()),
                ui: UiState::default(),
                queued: Vec::new(),
                fresh: true,
            }))
        }),
    )
    .unwrap();
}
