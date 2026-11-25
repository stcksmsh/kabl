//! The factory composite `kabl.space` v1 (catalogue C2, roadmap CB1): an echo into a room.
//!
//! The inner graph is the Space tail of the factory sound Soft Sine
//! (`patches/presets/soft-sine`): `split.left → delay → reverb`, with Soft Sine's delay and
//! reverb settings as the defaults. See `docs/composites/SPACE.md`.
//!
//! `cargo run -p kabl-tools --example space_composite -- write composites` writes
//! `composites/space/v1.json`. Running it again writes the same bytes: the package holds the
//! patch state only, never the log's wall-clock timestamps.
//!
//! `cargo run --release -p kabl-tools --example space_composite -- wav DIR` renders a short
//! phrase on the default voice dry and through Space at its defaults, offline, to
//! `DIR/space-dry.wav` and `DIR/space-wet.wav`, and prints peak and RMS.
//!
//! `xvfb-run -a -s "-screen 0 1440x900x24" cargo run --release -p kabl-tools --example
//! space_composite -- capture DIR` captures the real UI: the Insert list, a voice feeding
//! Space, the Space face, and Space opened.
//!
//! The test `composites_space` includes this file to compare `bytes()` with the committed
//! file, so only the generator is compiled there.
use kabl_core::panel::{Kind, Panel, Placement};
use kabl_core::{Definition, Exposure, ModuleId, PortRef, Vec2};
use kabl_ui::{composites, PatchEditor};
use std::collections::BTreeSet;

fn jack(id: ModuleId, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}
fn param(id: ModuleId, name: &str) -> PortRef {
    PortRef::Param {
        id,
        param: name.into(),
    }
}
fn expose(label: &str, target: PortRef) -> Exposure {
    Exposure {
        label: label.into(),
        target,
        also: Vec::new(),
    }
}

/// Soft Sine's echo and room (`patches/presets/soft-sine/checkpoint.json`, modules 14 and 15).
const DELAY: [(&str, f32); 5] = [
    ("time_ms", 310.0),
    // Soft Sine leaves `sync` at the module default (1/8), which without a clock cable plays the
    // free time. Free says the same thing and is what the catalogue proposes.
    ("sync", 0.0),
    ("feedback", 40.0),
    ("mix", 12.0),
    ("tone_hz", 3500.0),
];
const REVERB: [(&str, f32); 5] = [
    ("decay_s", 1.4),
    ("damp_hz", 3500.0),
    ("mix", 10.0),
    ("predelay_ms", 0.0),
    ("width", 60.0),
];

/// Knob 70 x 110, selector 110 x 110 (at least 18 points for each of five choices), jack 128 x
/// 58 so the port names fit, all inside the 340-point face below its header.
fn placement(kind: Kind, x: f32, y: f32) -> Placement {
    let (width, height) = match kind {
        Kind::Knob => (70.0, 110.0),
        Kind::Selector => (110.0, 110.0),
        // The Space face places only knobs, selectors and jacks.
        _ => (128.0, 58.0),
    };
    Placement {
        kind,
        x,
        y,
        width,
        height,
        source: None,
        label: None,
        icons: Vec::new(),
    }
}

fn package() -> composites::Package {
    let mut e = PatchEditor::new();
    let split = e.add_module("split", Vec2 { x: 24.0, y: 20.0 });
    let delay = e.add_module("delay", Vec2 { x: 214.0, y: 20.0 });
    let verb = e.add_module("reverb", Vec2 { x: 404.0, y: 20.0 });
    for (name, v) in DELAY {
        e.set_param(delay, name, v);
    }
    for (name, v) in REVERB {
        e.set_param(verb, name, v);
    }
    e.connect(jack(split, "left"), jack(delay, "in"));
    e.connect(jack(delay, "out"), jack(verb, "in"));
    // Interface keys: ports 1..=7, then controls 8..=17, in the order listed.
    let ports = [
        expose("In", jack(split, "in")),
        expose("Clock", jack(delay, "clock")),
        expose("Out", jack(verb, "out")),
        expose("Echo mod", param(delay, "mix")),
        expose("Room mod", param(verb, "mix")),
        expose("Decay mod", param(verb, "decay_s")),
        expose("Feedback mod", param(delay, "feedback")),
    ];
    let controls = [
        expose("Echo", param(delay, "mix")),
        expose("Time", param(delay, "time_ms")),
        expose("Sync", param(delay, "sync")),
        expose("Feedback", param(delay, "feedback")),
        expose("Tone", param(delay, "tone_hz")),
        expose("Room", param(verb, "mix")),
        expose("Decay", param(verb, "decay_s")),
        expose("Damp", param(verb, "damp_hz")),
        expose("Width", param(verb, "width")),
        expose("Predelay", param(verb, "predelay_ms")),
    ];
    let mut panel = Panel {
        width: 720.0,
        ..Panel::default()
    };
    // Echo row, then room row; the jacks in two columns on the right.
    let rows = [
        [
            (8, Kind::Knob),
            (9, Kind::Knob),
            (10, Kind::Selector),
            (11, Kind::Knob),
            (12, Kind::Knob),
        ],
        [
            (13, Kind::Knob),
            (14, Kind::Knob),
            (15, Kind::Knob),
            (16, Kind::Knob),
            (17, Kind::Knob),
        ],
    ];
    for (row, y) in rows.iter().zip([64.0, 180.0]) {
        let mut x = 8.0;
        for &(key, kind) in row {
            let p = placement(kind, x, y);
            x += p.width + 8.0;
            panel.placements.insert(key, p);
        }
    }
    for (key, x, y) in [
        (1, 448.0, 64.0),
        (3, 584.0, 64.0),
        (2, 448.0, 124.0),
        (7, 584.0, 124.0),
        (4, 448.0, 184.0),
        (6, 584.0, 184.0),
        (5, 448.0, 244.0),
    ] {
        panel.placements.insert(key, placement(Kind::Jack, x, y));
    }
    let mut log = e.log().clone();
    let id = composites::encapsulate_with(
        &mut log,
        "Space",
        Definition {
            id: "kabl.space".into(),
            version: 1,
        },
        BTreeSet::from([split, delay, verb]),
        &ports,
        &controls,
        Some(panel),
    )
    .expect("Space wraps");
    composites::package(log.state(), id).expect("Space packages")
}

/// `composites/space/v1.json` exactly as committed.
pub(crate) fn bytes() -> Vec<u8> {
    serde_json::to_vec_pretty(&package()).expect("serialise")
}

#[cfg(not(test))]
mod tool {
    use super::*;
    use eframe::egui;
    use egui::{Pos2, Rect};
    use kabl_core::{CompositeId, PatchState};
    use kabl_ui::{show, UiState};
    use std::path::{Path, PathBuf};

    /// The default voice with its VCA feeding `kabl.space` v1 from the bank, and Space feeding
    /// the output. Returns the editor and the inserted composite.
    fn voice_into_space() -> (PatchEditor, CompositeId) {
        let mut e = PatchEditor::seed_from(&kabl_standalone::default_patch());
        let find = |p: &PatchState, kind: &str| {
            *p.modules
                .iter()
                .find(|(_, m)| m.kind == kind)
                .expect("default voice module")
                .0
        };
        let (vca, out) = (find(e.state(), "vca"), find(e.state(), "out"));
        let to_out = *e
            .state()
            .cables
            .iter()
            .find(|(_, c)| c.to.module_id() == Some(out))
            .expect("a cable into out")
            .0;
        e.disconnect(to_out);
        let bank = Path::new(env!("CARGO_MANIFEST_DIR")).join("../composites");
        let entry = composites::factory_entries(&bank)
            .into_iter()
            .find(|f| f.definition.id == "kabl.space" && f.definition.version == 1)
            .expect("kabl.space v1 in the bank; run `space_composite write composites` first");
        let pkg = composites::read_factory(&entry).expect("read kabl.space v1");
        let id = composites::insert(&mut e, &pkg).expect("insert Space");
        // A row below the voice, clear of the envelope.
        let mut c = e.state().composites[&id].clone();
        c.pos = Vec2 { x: 24.0, y: 640.0 };
        composites::set(&mut e, id, c).expect("move Space");
        let port = |e: &PatchEditor, label: &str| {
            e.state().composites[&id]
                .ports
                .values()
                .find(|x| x.label == label)
                .expect("Space port")
                .target
                .clone()
        };
        let (input, output) = (port(&e, "In"), port(&e, "Out"));
        e.connect(jack(vca, "out"), input);
        e.connect(output, jack(out, "in"));
        (e, id)
    }

    fn write(root: &Path) {
        let dir = root.join("space");
        std::fs::create_dir_all(&dir).expect("bank folder");
        let bytes = bytes();
        composites::decode(&bytes).expect("the package decodes");
        let path = dir.join("v1.json");
        std::fs::write(&path, bytes).expect("write");
        println!("wrote {}", path.display());
    }

    /// Two bars of eighths at 124 BPM on the default voice, then a bar and a half of tail.
    fn render(p: &PatchState) -> Vec<f32> {
        const SR: f32 = 48000.0;
        let notes: [(f32, f32); 8] = [
            (0.0, 0.0),
            (0.5, 3.0),
            (1.0, 7.0),
            (1.5, 10.0),
            (2.0, 12.0),
            (3.0, 7.0),
            (3.5, 5.0),
            (4.0, 0.0),
        ];
        let beat = 60.0 / 124.0;
        let total = (7.0 * beat * SR) as usize;
        let mut engine = kabl_engine::compile::compile(p, SR, 8).expect("compile");
        let mut audio = Vec::with_capacity(total * 2);
        let mut t = 0usize;
        let mut held: Option<usize> = None;
        while t < total {
            for (i, &(b, semis)) in notes.iter().enumerate() {
                let on = (b * beat * SR) as usize;
                if (t..t + 64).contains(&on) {
                    if let Some(v) = held.take() {
                        engine.note_off(v);
                    }
                    engine.note_on(i % 8, semis, 0.8);
                    held = Some(i % 8);
                }
                let off = ((b + 0.4) * beat * SR) as usize;
                if (t..t + 64).contains(&off) && held == Some(i % 8) {
                    engine.note_off(i % 8);
                    held = None;
                }
            }
            engine.process_block();
            for (l, r) in engine.left().iter().zip(engine.right()) {
                audio.push(*l);
                audio.push(*r);
            }
            t += 64;
        }
        audio
    }

    fn wav(dir: &Path) {
        std::fs::create_dir_all(dir).expect("out folder");
        let dry = PatchEditor::seed_from(&kabl_standalone::default_patch());
        let (wet, _) = voice_into_space();
        for (name, p) in [("space-dry", dry.state()), ("space-wet", wet.state())] {
            let audio = render(p);
            let path = dir.join(format!("{name}.wav"));
            let mut w = hound::WavWriter::create(
                &path,
                hound::WavSpec {
                    channels: 2,
                    sample_rate: 48000,
                    bits_per_sample: 32,
                    sample_format: hound::SampleFormat::Float,
                },
            )
            .expect("wav");
            for s in &audio {
                w.write_sample(*s).expect("sample");
            }
            w.finalize().expect("finalize");
            let peak = audio.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            let rms = (audio.iter().map(|s| s * s).sum::<f32>() / audio.len() as f32).sqrt();
            println!(
                "{}: peak {:.1} dBFS, RMS {:.1} dBFS",
                path.display(),
                20.0 * peak.log10(),
                20.0 * rms.log10()
            );
        }
    }

    const WINDOW: (u32, u32) = (1440, 900);

    #[derive(Clone, Copy, PartialEq)]
    enum Shot {
        Insert,
        Rack,
        Face,
        Inside,
    }

    struct App {
        dir: PathBuf,
        shots: Vec<Shot>,
        i: usize,
        f: u32,
        editor: PatchEditor,
        space: Option<CompositeId>,
        ui: UiState,
        inject: Vec<egui::Event>,
    }

    impl App {
        fn start(&mut self, shot: Shot) {
            self.ui = UiState::default();
            self.ui.dark = true;
            self.space = None;
            if shot == Shot::Insert {
                self.editor = PatchEditor::seed_from(&kabl_standalone::default_patch());
            } else {
                let (e, id) = voice_into_space();
                self.editor = e;
                self.space = Some(id);
            }
        }
    }

    impl App {
        /// Saves the current shot: the whole window, or for the face only Space's card.
        fn write_shot(&self, img: &egui::ColorImage) {
            let size = egui::vec2(img.size[0] as f32, img.size[1] as f32);
            let bytes = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            let mut png = image::RgbaImage::from_raw(size.x as u32, size.y as u32, bytes).unwrap();
            let (name, card) = match self.shots[self.i] {
                Shot::Insert => ("space-insert-menu.png", None),
                Shot::Rack => ("space-rack-voice.png", None),
                Shot::Face => ("space-face.png", self.space),
                Shot::Inside => ("space-inside.png", None),
            };
            if let Some(id) = card {
                let r = self.ui.hits[&format!("composite:{id}:body")]
                    .expand(8.0)
                    .intersect(Rect::from_min_size(Pos2::ZERO, size));
                let (x, y) = (r.min.x as u32, r.min.y as u32);
                png = image::imageops::crop_imm(&png, x, y, r.width() as u32, r.height() as u32)
                    .to_image();
            }
            let path = self.dir.join(name);
            png.save(&path).unwrap();
            println!("wrote {}", path.display());
        }
    }

    impl eframe::App for App {
        fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
            raw.events.append(&mut self.inject);
        }

        fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
            let ctx = ui.ctx().clone();
            let taken = ctx.input(|i| {
                i.raw.events.iter().find_map(|e| match e {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            });
            if let Some(img) = taken {
                self.write_shot(&img);
                self.i += 1;
                self.f = 0;
            }
            if self.i >= self.shots.len() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
            let which = self.shots[self.i];
            if self.f == 0 {
                self.start(which);
            }
            // The panel resets its view on the first frame with a new editor; open it after that.
            if which == Shot::Insert && self.f == 2 {
                self.ui.composites.panel = true;
            }
            // Open the inside the way a user does: click "Inspect internals" on the card.
            if which == Shot::Inside && self.f == 4 {
                let id = self.space.unwrap();
                if let Some(r) = self.ui.hits.get(&format!("composite:{id}:open")) {
                    let pos = r.center();
                    self.inject.push(egui::Event::PointerMoved(pos));
                    for pressed in [true, false] {
                        self.inject.push(egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        });
                    }
                }
            }
            let mut root = egui::Ui::new(
                ctx.clone(),
                egui::Id::new("root"),
                egui::UiBuilder::new().max_rect(Rect::from_min_size(
                    Pos2::ZERO,
                    egui::vec2(WINDOW.0 as f32, WINDOW.1 as f32),
                )),
            );
            show(&mut self.editor, &mut self.ui, &mut root);
            self.f += 1;
            if self.f == 16 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            ctx.request_repaint();
        }
    }

    fn capture(dir: PathBuf) {
        std::fs::create_dir_all(&dir).expect("out folder");
        // The Insert list reads the bank from here first; point it at this checkout's bank.
        std::env::set_var(
            "KABL_FACTORY_COMPOSITES_DIR",
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../composites"),
        );
        let opts = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([WINDOW.0 as f32, WINDOW.1 as f32])
                .with_resizable(false),
            ..Default::default()
        };
        eframe::run_native(
            "space capture",
            opts,
            Box::new(move |_| {
                Ok(Box::new(App {
                    dir,
                    shots: vec![Shot::Insert, Shot::Rack, Shot::Face, Shot::Inside],
                    i: 0,
                    f: 0,
                    editor: PatchEditor::new(),
                    space: None,
                    ui: UiState::default(),
                    inject: Vec::new(),
                }))
            }),
        )
        .unwrap();
    }

    pub(super) fn main() {
        let a: Vec<String> = std::env::args().skip(1).collect();
        let dir = PathBuf::from(a.get(1).cloned().unwrap_or_else(|| ".".into()));
        match a.first().map(String::as_str) {
            Some("write") => write(&dir),
            Some("wav") => wav(&dir),
            Some("capture") => capture(dir),
            _ => eprintln!("usage: space_composite write|wav|capture DIR"),
        }
    }
}

#[cfg(not(test))]
fn main() {
    tool::main();
}
