//! Living-cable clips (handover 5.1, taste gate b): the real rack in look B, driven by the real
//! engine rendered offline, so the glow comes from the patch's own sound. Nothing is played on
//! any sink: the audio is rendered into a buffer and thrown away; only the face-tap reports
//! reach the UI. Time is virtual (30 frames a second, exactly), so a clip is the same on every
//! machine however slowly the window draws.
//!
//! `xvfb-run -a -s "-screen 0 1440x900x24" cargo run --release -p kabl-tools --example cable_glow_clip -- WxH DIR [patch,..] [secs]`
//!
//! Patches: simple (default patch, played by a little phrase), composition (plays itself, and
//! the same phrase on its keys). Writes `DIR/<patch>__look-b__<low|high>.webm` (needs
//! `ffmpeg`) and `DIR/<patch>__look-b__<low|high>__f<N>.png` stills at 1 s, 3 s and 5 s.
use eframe::egui;
use egui::{Pos2, Rect};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::PatchEngine;
use kabl_ui::cable_life::{CableLife, Intensity};
use kabl_ui::{show, PatchEditor, UiState};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};

const FPS: usize = 60;
const SR: f32 = 48000.0;
/// Samples per video frame.
const STEP: usize = SR as usize / FPS;

#[path = "support/mockup.rs"]
mod mockup;

fn patch(name: &str) -> kabl_core::PatchState {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../patches");
    match name {
        "simple" => kabl_standalone::default_patch(),
        // The design mockup's rack (sequencer, oscillator, filter, envelope, mixer, delay, reverb,
        // output), shared with chrome_capture.
        "mockup" => mockup::mockup_patch(false).state().clone(),
        // The factory sound Acid Line.
        "acid-line" => kabl_core::load(&root.join("presets/acid-line"))
            .unwrap()
            .state()
            .clone(),
        "composition" => kabl_core::load(&root.join("composition"))
            .unwrap()
            .state()
            .clone(),
        _ => panic!("unknown patch {name}"),
    }
}

/// A phrase on the keys: (start second, note, hold seconds). Repeats every 6.4 s.
const PHRASE: [(f32, u8, f32); 8] = [
    (0.1, 60, 0.6),
    (0.9, 64, 0.6),
    (1.7, 67, 0.6),
    (2.5, 72, 1.2),
    (4.1, 67, 0.4),
    (4.5, 64, 0.4),
    (4.9, 60, 0.4),
    (5.3, 55, 0.9),
];

struct Job {
    patch: String,
    intensity: Intensity,
}

struct Run {
    editor: PatchEditor,
    ui: UiState,
    engine: PatchEngine,
    _collector: basedrop::Collector,
    ffmpeg: Child,
    /// Frames simulated, frames written.
    sim: usize,
    /// Blocks rendered so far (a frame is not a whole number of blocks at 60 fps).
    blocks: usize,
    got: usize,
    peak: f32,
    name: String,
}

struct App {
    jobs: Vec<Job>,
    i: usize,
    run: Option<Run>,
    dir: PathBuf,
    size: (u32, u32),
    frames: usize,
}

impl App {
    fn start(&mut self) {
        let j = &self.jobs[self.i];
        let state = patch(&j.patch);
        let collector = basedrop::Collector::new();
        let engine = PatchEngine::new(&collector.handle(), &state, SR, 8).unwrap();
        let mut ui = UiState::default();
        ui.library = Some(kabl_ui::library::Library::open(
            Some(kabl_ui::library::built_factory_dir()),
            tempfile::tempdir().unwrap().keep(),
        ));
        ui.drawer_open = false;
        ui.browser_open = false;
        ui.theme = "kabl.look_b".into();
        // KABL_CAP_DARK=1 records the dark theme (name gets `__dark`); KABL_CLIP_TAG=x adds `__x`.
        ui.dark = std::env::var("KABL_CAP_DARK").is_ok();
        ui.inspect.audio = true;
        ui.cable_life = CableLife::GlowPulses;
        ui.cable_intensity = j.intensity;
        let name = format!(
            "{}__look-b__{}{}{}",
            j.patch,
            if j.intensity == Intensity::High {
                "high"
            } else {
                "low"
            },
            if ui.dark { "__dark" } else { "" },
            std::env::var("KABL_CLIP_TAG").map_or(String::new(), |t| format!("__{t}"))
        );
        let ffmpeg = std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-loglevel",
                "error",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgba",
            ])
            .args(["-s", &format!("{}x{}", self.size.0, self.size.1)])
            .args(["-r", &FPS.to_string(), "-i", "-"])
            .args([
                "-c:v",
                "libvpx-vp9",
                "-crf",
                "34",
                "-b:v",
                "0",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(self.dir.join(format!("{name}.webm")))
            .stdin(Stdio::piped())
            .spawn()
            .expect("ffmpeg");
        println!("{name}: {} frames", self.frames);
        self.run = Some(Run {
            editor: PatchEditor::seed_from(&state),
            ui,
            engine,
            _collector: collector,
            ffmpeg,
            sim: 0,
            blocks: 0,
            got: 0,
            peak: 0.0,
            name,
        });
    }
}

impl Run {
    /// One video frame of audio: the keys, the commands the UI asked for, the face reports.
    fn render(&mut self, now: f64) {
        let p = (self.sim as f32 / FPS as f32) % 6.4;
        let fires = |at: f32| p <= at && at < p + 1.0 / FPS as f32;
        for &(at, note, hold) in &PHRASE {
            if fires(at) {
                self.engine.key(KeyEvent::On {
                    note,
                    velocity: 100,
                });
            }
            if fires((at + hold) % 6.4) {
                self.engine.key(KeyEvent::Off { note });
            }
        }
        // The clock's song position in pulses (four to a beat), as the audio thread reports it.
        for (&id, m) in &self.editor.state().modules {
            if m.kind == "clock" {
                let bpm = f64::from(m.params.get("bpm").copied().unwrap_or(120.0));
                self.ui
                    .clock_pos
                    .insert(id, self.sim as f64 / FPS as f64 * bpm / 60.0 * 4.0);
            }
        }
        for c in std::mem::take(&mut self.ui.launches) {
            self.engine.command(&c);
        }
        let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
        while self.blocks * BLOCK < (self.sim + 1) * STEP {
            self.engine.process_block(&mut l, &mut r);
            self.blocks += 1;
            self.peak = l
                .iter()
                .chain(r.iter())
                .fold(self.peak, |p, s| p.max(s.abs()));
            if let Some(rep) = self.engine.take_face_report() {
                self.ui.live.accept(&rep, now);
            }
        }
        self.sim += 1;
        if self.sim.is_multiple_of(FPS) {
            println!("  {} s: output peak {:.3}", self.sim / FPS, self.peak);
            self.peak = 0.0;
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.i >= self.jobs.len() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.run.is_none() {
            self.start();
        }
        let (w, h) = self.size;
        let mut done = false;
        let run = self.run.as_mut().unwrap();
        for e in ctx.input(|i| i.raw.events.clone()) {
            let egui::Event::Screenshot { image, .. } = e else {
                continue;
            };
            // Frames before the first render settled are not part of the clip.
            if run.got >= self.frames {
                continue;
            }
            let raw: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            run.ffmpeg.stdin.as_mut().unwrap().write_all(&raw).unwrap();
            run.got += 1;
            if run.got % (2 * FPS) == FPS {
                let f = self.dir.join(format!("{}__f{:03}.png", run.name, run.got));
                image::save_buffer(&f, &raw, w, h, image::ColorType::Rgba8).unwrap();
            }
            done = run.got >= self.frames;
        }
        if done {
            let mut run = self.run.take().unwrap();
            drop(run.ffmpeg.stdin.take());
            run.ffmpeg.wait().unwrap();
            println!("wrote {}.webm", run.name);
            self.i += 1;
            ctx.request_repaint();
            return;
        }
        // Virtual time: every frame is exactly 1/30 s, whatever the window took to draw.
        let now = run.sim as f64 / FPS as f64;
        ctx.input_mut(|i| {
            i.time = now;
            i.stable_dt = 1.0 / FPS as f32;
            i.unstable_dt = 1.0 / FPS as f32;
        });
        run.render(now);
        // KABL_CLIP_ZOOM=0.58 shows the whole of a large patch.
        run.ui.zoom = std::env::var("KABL_CLIP_ZOOM")
            .ok()
            .and_then(|z| z.parse().ok())
            .unwrap_or(1.0);
        run.ui.pan = egui::vec2(12.0, 12.0);
        let mut root = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(w as f32, h as f32),
            )),
        );
        show(&mut run.editor, &mut run.ui, &mut root);
        // The first frames settle the layout and the taps; they are not recorded.
        if run.sim > 12 && run.sim <= 12 + self.frames + 4 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        ctx.request_repaint();
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (w, h) = a
        .first()
        .and_then(|s| s.split_once('x'))
        .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
        .unwrap_or((1180u32, 680u32));
    let dir = PathBuf::from(a.get(1).cloned().unwrap_or_else(|| "target/glow".into()));
    std::fs::create_dir_all(&dir).unwrap();
    let patches: Vec<String> = a.get(2).map_or_else(
        || ["simple", "composition"].map(String::from).to_vec(),
        |s| s.split(',').map(String::from).collect(),
    );
    let secs: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(6);
    let mut jobs = Vec::new();
    for p in &patches {
        // KABL_CLIP_INT=low|high records one intensity only.
        for intensity in [Intensity::Low, Intensity::High] {
            let only = std::env::var("KABL_CLIP_INT").unwrap_or_default();
            if (only == "low" && intensity == Intensity::High)
                || (only == "high" && intensity == Intensity::Low)
            {
                continue;
            }
            jobs.push(Job {
                patch: p.clone(),
                intensity,
            });
        }
    }
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([w as f32, h as f32])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "cable glow clips",
        opts,
        Box::new(move |cc| {
            cc.egui_ctx.set_pixels_per_point(1.0);
            Ok(Box::new(App {
                jobs,
                i: 0,
                run: None,
                dir,
                size: (w, h),
                frames: secs * FPS,
            }))
        }),
    )
    .unwrap();
}
