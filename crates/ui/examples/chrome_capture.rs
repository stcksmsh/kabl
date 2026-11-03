//! Scene captures for the G5 chrome PRs: the real app (`kabl_ui::show`) with the factory sound
//! Acid Line, in the default Signal solid (dark) and Drafting solid (light) looks, at the three
//! window sizes of PLAN §8, for the Rack, Perform and Sounds scenes.
//!
//! `xvfb-run -a -s "-screen 0 1440x900x24" cargo run --release -p kabl-ui --example chrome_capture -- OUT_DIR [scene,scene|all] [theme]`
//!
//! Files: `OUT/<scene>__<dark|light>__<W>x<H>.png`. `theme` is a theme id (default
//! `kabl.line_solid`; `kabl.satin` for a before/after). Silent: no audio device is opened.
use eframe::egui;
use egui::{Pos2, Rect};
use image::RgbaImage;
use kabl_ui::library::Library;
use kabl_ui::{show, PatchEditor, UiState};
use std::path::PathBuf;

const SIZES: [(u32, u32); 3] = [(1440, 900), (1280, 800), (1180, 680)];
const SCENES: [&str; 8] = [
    "rack",
    "perform",
    "sounds",
    "drawer",
    "wavetable-editor",
    "seq-pattern-editor",
    "cues-editor",
    "cable-pattern-editor",
];

struct Job {
    scene: &'static str,
    dark: bool,
    size: (u32, u32),
}

struct App {
    jobs: Vec<Job>,
    i: usize,
    f: u32,
    dir: PathBuf,
    theme: String,
    editor: PatchEditor,
    ui: UiState,
    _user: tempfile::TempDir,
}

fn patches() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../patches")
}

/// The mockup's patch: sequencer, VA (or, with `wt`, wavetable) oscillator, SVF, ADSR, mixer,
/// delay, reverb, output.
fn mockup_patch(wt: bool) -> PatchEditor {
    use kabl_core::PortRef;
    let mut e = PatchEditor::seed_from(&Default::default());
    let at = |x: f32, y: f32| kabl_core::Vec2 { x, y };
    let seq = e.add_module("seq", at(24.0, 20.0));
    let osc = e.add_module(if wt { "osc.wt" } else { "osc.va" }, at(714.0, 20.0));
    let svf = e.add_module("filter.svf", at(964.0, 20.0));
    let adsr = e.add_module("env.adsr", at(24.0, 320.0));
    let mix = e.add_module("mixer", at(244.0, 320.0));
    let delay = e.add_module("delay", at(504.0, 320.0));
    let verb = e.add_module("reverb", at(744.0, 320.0));
    let out = e.add_module("out", at(984.0, 320.0));
    let m = |id, port: &str| PortRef::Module {
        id,
        port: port.to_string(),
    };
    e.connect(m(seq, "pitch"), m(osc, "pitch"));
    e.connect(m(osc, "out"), m(svf, "in"));
    e.connect(m(seq, "gate"), m(adsr, "gate"));
    e.connect_route(m(adsr, "out"), svf, "cutoff_hz");
    e.connect(m(svf, "lp"), m(mix, "in1"));
    e.connect(m(mix, "out"), m(delay, "in"));
    e.connect(m(delay, "out"), m(verb, "in"));
    e.connect(m(verb, "out"), m(out, "in"));
    assert_eq!(e.state().cables.len(), 8, "mockup patch cables");
    e
}

impl App {
    fn prepare(&mut self) {
        let j = &self.jobs[self.i];
        self.editor = mockup_patch(j.scene == "wavetable-editor");
        let mut ui = UiState::default();
        ui.theme = self.theme.clone();
        ui.dark = j.dark;
        ui.status_meter = true;
        // The mockup's status bar readouts, as a host would feed them.
        ui.status_cpu = Some(6.0);
        ui.status_buffer = Some(128);
        ui.status_voices = Some((4, 8));
        ui.status_bpm = Some(124.0);
        ui.status_pos = Some("17 . 3 . 2".into());
        ui.library = Some(Library::open(
            Some(patches()),
            self._user.path().to_path_buf(),
        ));
        if j.scene == "sounds" {
            ui.browser.selected = Some("factory:presets/acid-line".into());
        }
        let mut doc = kabl_ui::browser::Doc::new(
            "Acid Line",
            kabl_ui::browser::DocOrigin::Library("factory:acid-line".into()),
            self.editor.state(),
        );
        doc.meta.category = "Bass".into();
        ui.doc = Some(doc);
        // An unsaved edit, so the sound dropdown shows its dot.
        let edit = self
            .editor
            .state()
            .modules
            .iter()
            .find_map(|(&id, m)| m.params.iter().next().map(|(k, &v)| (id, k.clone(), v)));
        if let Some((id, k, v)) = edit {
            self.editor.set_param(id, &k, v + 1.0);
        }
        ui.browser_open = j.scene == "sounds";
        ui.drawer_open = matches!(
            j.scene,
            "rack" | "seq-pattern-editor" | "cues-editor" | "cable-pattern-editor"
        );
        ui.perform_open = j.scene == "perform";
        ui.perform_tall = j.scene == "perform";
        // Like the mockup: the oscillator-to-filter cable and the filter are selected.
        ui.selected_route = self
            .editor
            .state()
            .cables
            .iter()
            .find(|(_, c)| matches!(&c.to, kabl_core::PortRef::Module { port, .. } if port == "in"))
            .map(|(&id, _)| id);
        ui.selected_module = self
            .editor
            .state()
            .modules
            .iter()
            .find(|(_, m)| m.kind == "filter.svf")
            .map(|(&id, _)| id);
        if j.scene == "seq-pattern-editor" {
            // The mockup's state: editing A, B playing, step 5 under the playhead.
            let seq = self
                .editor
                .state()
                .modules
                .iter()
                .find(|(_, m)| m.kind == "seq")
                .map(|(&id, _)| id)
                .unwrap();
            let gate = [1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 0.0, 1.0];
            let vel = [100.0, 80.0, 80.0, 60.0, 100.0, 50.0, 70.0, 90.0];
            let prob = [100.0, 100.0, 100.0, 75.0, 100.0, 50.0, 100.0, 100.0];
            let ratch = [1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 1.0, 3.0];
            for i in 0..8 {
                self.editor.set_param(seq, &format!("g{}", i + 1), gate[i]);
                self.editor.set_param(seq, &format!("v{}", i + 1), vel[i]);
                self.editor.set_param(seq, &format!("r{}", i + 1), prob[i]);
                self.editor.set_param(seq, &format!("k{}", i + 1), ratch[i]);
            }
            ui.selected_module = Some(seq);
            ui.selected_route = None;
            ui.edit_bank.insert(seq, 0);
            ui.seq_banks.insert(seq, (1, None));
            ui.seq_steps.insert(seq, 4);
        }
        if j.scene == "wavetable-editor" {
            let wt = self
                .editor
                .state()
                .modules
                .iter()
                .find(|(_, m)| m.kind == "osc.wt")
                .map(|(&id, _)| id)
                .unwrap();
            kabl_ui::tables::open(&self.editor, &mut ui, wt);
            // Like the mockup: frame 7 of 16 selected, the oscillator's position at 0.40.
            ui.tables.select(6);
            self.editor.set_param(wt, "position", 0.4);
        }
        if j.scene == "cable-pattern-editor" {
            // The mockup's state: Seq Pitch to VA Osc Pitch with a 16-step pattern, chance
            // 100 %, morph 35 % moved by the Bite macro, glide 40 ms, and no clock.
            let cable = self
                .editor
                .state()
                .cables
                .iter()
                .find(|(_, c)| {
                    matches!(&c.to, kabl_core::PortRef::Module { port, .. } if port == "pitch")
                })
                .map(|(&id, _)| id)
                .unwrap();
            let on = [1, 1, 0, 1, 1, 0, 1, 1, 1, 0, 1, 1, 0, 1, 1, 1];
            let chance = [
                1.0, 1.0, 1.0, 0.75, 1.0, 1.0, 0.5, 1.0, 1.0, 1.0, 1.0, 0.75, 1.0, 1.0, 0.5, 1.0,
            ];
            let mut set = |slot: usize, v: f32| {
                self.editor.set_param_gesture(
                    kabl_core::ParamTarget::Cable {
                        id: cable,
                        param: kabl_cables::slot_name(slot),
                    },
                    v,
                    true,
                );
            };
            set(0, 16.0);
            set(1, 100.0);
            for k in 0..16 {
                set(kabl_cables::LEVEL0 + k, on[k] as f32);
                set(kabl_cables::CHANCE0 + k, chance[k] * 100.0);
            }
            set(kabl_cables::MORPH, 0.35);
            set(kabl_cables::GLIDE, 40.0);
            let mac = self
                .editor
                .add_module("macro", kabl_core::Vec2 { x: 24.0, y: 640.0 });
            self.editor.set_label(mac, "name.m1", Some("Bite".into()));
            self.editor.connect_cable_route(
                kabl_core::PortRef::Module {
                    id: mac,
                    port: "m1".into(),
                },
                cable,
                "morph",
            );
            ui.cable_fn = Some(cable);
            ui.selected_route = Some(cable);
            ui.selected_module = None;
        }
        if j.scene == "cues-editor" {
            // The mockup's state: Bass, Drums and Lead sequencers (two more in a third row,
            // below the view), four cues, Drop playing and Break queued.
            let at = |x: f32, y: f32| kabl_core::Vec2 { x, y };
            let bass = self
                .editor
                .state()
                .modules
                .iter()
                .find(|(_, m)| m.kind == "seq")
                .map(|(&id, _)| id)
                .unwrap();
            let drums = self.editor.add_module("seq", at(24.0, 640.0));
            let lead = self.editor.add_module("seq", at(724.0, 640.0));
            let cues = self.editor.add_module("cues", at(1424.0, 640.0));
            for (s, name) in [(bass, "Bass"), (drums, "Drums"), (lead, "Lead")] {
                kabl_ui::perform::rename_pin(&mut self.editor, s, kabl_ui::perform::BANKS, name);
            }
            // Banks of (Bass, Drums, Lead) per cue; None leaves it ("off" in the mockup).
            let scenes: [(&str, [Option<usize>; 3]); 4] = [
                ("Intro", [Some(0), Some(0), None]),
                ("Build", [Some(0), Some(1), Some(0)]),
                ("Drop", [Some(1), Some(0), Some(2)]),
                ("Break", [None, Some(3), Some(1)]),
            ];
            for (k, (name, banks)) in scenes.into_iter().enumerate() {
                kabl_ui::cues::add(&mut self.editor, cues, &|_| 0);
                kabl_ui::cues::rename(&mut self.editor, cues, k + 1, name);
                for (s, b) in [bass, drums, lead].into_iter().zip(banks) {
                    kabl_ui::cues::set_target(&mut self.editor, cues, k + 1, s, b);
                }
            }
            ui.selected_module = Some(cues);
            ui.selected_route = None;
            // Drop plays; Break is queued (Drums waits for D, Lead for B; Bass keeps B).
            ui.seq_banks.insert(bass, (1, None));
            ui.seq_banks.insert(drums, (0, Some(3)));
            ui.seq_banks.insert(lead, (2, Some(1)));
            ui.last_cue.insert(cues, 3);
        }
        self.ui = ui;
        self.f = 0;
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
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
            let j = &self.jobs[self.i];
            let [w, h] = img.size;
            let raw: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            let full = RgbaImage::from_raw(w as u32, h as u32, raw).unwrap();
            let crop = image::imageops::crop_imm(
                &full,
                0,
                0,
                j.size.0.min(w as u32),
                j.size.1.min(h as u32),
            )
            .to_image();
            let name = format!(
                "{}__{}__{}x{}.png",
                j.scene,
                if j.dark { "dark" } else { "light" },
                j.size.0,
                j.size.1
            );
            crop.save(self.dir.join(&name)).unwrap();
            println!("wrote {name}");
            self.i += 1;
            if self.i >= self.jobs.len() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
            self.prepare();
        }
        let size = self.jobs[self.i].size;
        let mut root = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(size.0 as f32, size.1 as f32),
            )),
        );
        if self.f == 6
            && matches!(
                self.jobs[self.i].scene,
                "drawer" | "seq-pattern-editor" | "cues-editor" | "cable-pattern-editor"
            )
        {
            // After the first frame: a compact window closes the drawer there, the user opens it.
            self.ui.drawer_open = true;
        }
        show(&mut self.editor, &mut self.ui, &mut root);
        // The mockup's rack zoom (95 %, 78 % at 1180); the extra modules sit below the view.
        if self.f == 6
            && matches!(
                self.jobs[self.i].scene,
                "cues-editor" | "cable-pattern-editor"
            )
        {
            self.ui.zoom = if self.jobs[self.i].size.0 < 1280 {
                0.78
            } else {
                0.95
            };
        }
        self.f += 1;
        if self.f == 12 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        ctx.request_repaint();
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let dir = PathBuf::from(a.first().cloned().unwrap_or_else(|| "target/chrome".into()));
    std::fs::create_dir_all(&dir).unwrap();
    let want = a.get(1).cloned().unwrap_or_else(|| "all".into());
    let theme = a
        .get(2)
        .cloned()
        .unwrap_or_else(|| "kabl.line_solid".into());
    let mut jobs = Vec::new();
    for scene in SCENES {
        if want != "all" && !want.split(',').any(|w| w == scene) {
            continue;
        }
        for dark in [true, false] {
            for size in SIZES {
                jobs.push(Job { scene, dark, size });
            }
        }
    }
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "chrome capture",
        opts,
        Box::new(move |cc| {
            cc.egui_ctx.set_pixels_per_point(1.0);
            let mut app = App {
                jobs,
                i: 0,
                f: 0,
                dir,
                theme,
                editor: PatchEditor::seed_from(&Default::default()),
                ui: UiState::default(),
                _user: tempfile::tempdir().unwrap(),
            };
            app.prepare();
            Ok(Box::new(app))
        }),
    )
    .unwrap();
}
