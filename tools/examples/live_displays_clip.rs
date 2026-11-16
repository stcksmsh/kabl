//! The live displays clip (L2): the mockup rack at 100 %, look B dark, 1440x900 on a fixed 60 fps
//! clock for 6 s. The face taps are driven here (no audio engine, no sink): three notes through
//! the ADSR (which also sweeps the filter's cutoff by its route), the filter, the mixer channel
//! and the delay, then 2 s of silence in which nothing may move.
//!
//! `xvfb-run -a -s "-screen 0 1440x900x24" cargo run --release -p kabl-tools --example live_displays_clip -- OUT_DIR`
//! writes `OUT_DIR/frame_000.png ..`; `ffmpeg -framerate 60 -i OUT_DIR/frame_%03d.png -c:v libvpx-vp9 -pix_fmt yuv420p mockup__live.webm`.
use eframe::egui;
use egui::{Pos2, Rect};
use image::RgbaImage;
use kabl_modules::ModuleView;
use kabl_ui::displays::LiveTap;
use kabl_ui::{show, PatchEditor, UiState};
use std::path::PathBuf;

#[path = "support/mockup.rs"]
mod mockup;

const FRAMES: u32 = 360;
const SETTLE: u32 = 8;
/// Notes: (start, gate length) in seconds.
const NOTES: [(f64, f64); 3] = [(0.4, 0.7), (1.9, 0.7), (3.3, 0.9)];

struct App {
    dir: PathBuf,
    editor: PatchEditor,
    ui: UiState,
    f: u32,
    saved: u32,
}

/// The ADSR the clip plays (attack 20 ms, decay 250 ms to 0.55, release 500 ms): the level and
/// the stage (1 attack, 2 decay, 3 sustain, 4 release, 0 idle) at `t`.
fn adsr(t: f64) -> (f32, f32) {
    for (s, len) in NOTES {
        if t >= s && t < s + len {
            let k = t - s;
            return if k < 0.02 {
                ((k / 0.02) as f32, 1.0)
            } else if k < 0.27 {
                (1.0 - 0.45 * ((k - 0.02) / 0.25) as f32, 2.0)
            } else {
                (0.55, 3.0)
            };
        }
        let r = t - (s + len);
        if (0.0..0.5).contains(&r) {
            let from = if len > 0.27 { 0.55 } else { 1.0 };
            return (from * (1.0 - (r / 0.5) as f32), 4.0);
        }
    }
    (0.0, 0.0)
}

impl App {
    fn tap(&mut self, id: kabl_core::ModuleId, port: u8, level: f32, stage: f32, t: f64) {
        self.ui.live.taps.insert(
            (id, port),
            LiveTap {
                last: level,
                min: -level * 0.8,
                max: level,
                view: ModuleView {
                    valid: stage > 0.0,
                    position: stage,
                    cycle: [0.0; kabl_modules::VIEW_CYCLE],
                },
                at: t,
            },
        );
    }

    /// What the engine's face taps would report at `t`.
    fn drive(&mut self, t: f64) {
        let find = |kind: &str| {
            *self
                .editor
                .state()
                .modules
                .iter()
                .find(|(_, m)| m.kind == kind)
                .map(|(id, _)| id)
                .unwrap()
        };
        let (osc, svf, env, mix, delay) = (
            find("osc.va"),
            find("filter.svf"),
            find("env.adsr"),
            find("mixer"),
            find("delay"),
        );
        let (level, stage) = adsr(t);
        let out = |kind: &str, id, port: &str| {
            kabl_ui::displays::output_of(self.editor.state(), id, port)
                .map(|(i, _, _)| i)
                .unwrap_or_else(|| panic!("{kind} has no output {port}"))
        };
        let (svf_lp, osc_out, delay_out) = (out("svf", svf, "lp"), out("osc", osc, "out"), 0);
        let _ = mix;
        // The delay's echoes ring on after the notes: a decaying tail of the loudest recent level.
        let tail = NOTES
            .iter()
            .filter(|(s, _)| t >= *s)
            .map(|(s, l)| ((t - s - l).max(0.0), t - s))
            .map(|(after, _)| (0.62f32).powf((after * 3.0) as f32))
            .fold(0.0f32, f32::max);
        let silent_after = NOTES[2].0 + NOTES[2].1 + 1.6;
        let tail = if t > silent_after { 0.0 } else { tail * 0.6 };
        self.tap(env, 0, level, stage, t);
        self.tap(
            osc,
            osc_out,
            0.8 * level.min(1.0).max(f32::from(stage > 0.0) * 0.3),
            0.0,
            t,
        );
        self.tap(svf, svf_lp, 0.7 * level, 0.0, t);
        self.tap(delay, delay_out, (0.7 * level).max(tail), 0.0, t);
        let master = (0.5 * level).max(0.35 * tail);
        self.ui.meter.level = [master, master * 0.9];
        self.ui.live.master = self.ui.meter.level;
    }
}

impl eframe::App for App {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        raw.focused = true;
        raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1440.0, 900.0)));
        raw.time = Some(f64::from(self.f) / 60.0);
        raw.predicted_dt = 1.0 / 60.0;
        // No pointer over the rack: no hover tooltips in the clip.
        raw.events.push(egui::Event::PointerGone);
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
                if self.saved == FRAMES {
                    println!("wrote {} frames", self.saved);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
            }
        }
        let t = f64::from(self.f.saturating_sub(SETTLE)) / 60.0;
        if self.f >= SETTLE {
            self.drive(t);
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
            self.ui.drawer_open = false;
        }
        if self.f >= SETTLE {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        self.f += 1;
        ctx.request_repaint();
    }
}

fn main() {
    let dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "target/live".into()),
    );
    std::fs::create_dir_all(&dir).unwrap();
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "live displays clip",
        opts,
        Box::new(move |cc| {
            cc.egui_ctx.set_pixels_per_point(1.0);
            let mut ui = UiState::default();
            ui.theme = "kabl.line_solid".into();
            ui.dark = true;
            ui.status_meter = true;
            ui.inspect.audio = true;
            ui.drawer_open = false;
            Ok(Box::new(App {
                dir,
                editor: mockup::mockup_patch(false),
                ui,
                f: 0,
                saved: 0,
            }))
        }),
    )
    .unwrap();
}
