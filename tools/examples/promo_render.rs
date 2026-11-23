//! Offline segment renders and the video for the promo (docs/promo/STORYBOARD-V2.md,
//! docs/promo/build_animatic.py).
//!
//! ```sh
//! cargo run --release -p kabl-tools --example promo_render -- render SPEC.json OUT_DIR
//! cargo run --release -p kabl-tools --example promo_render -- list PATCH [PATCH ...]
//! xvfb-run -a -s "-screen 0 1920x1080x24" \
//!   cargo run --release -p kabl-tools --example promo_render -- video SPEC.json MIX.wav OUT.mp4
//! ```
//!
//! `SPEC.json` is `{"segments": [{name, patch, secs, preroll, bpm?, run?, macros?, ramps?, cues?,
//! hits?}]}`. `patch` is a preset folder or a `.kabl` file. Times are seconds from the start of the
//! written audio; `preroll` seconds are rendered first and thrown away (times inside it are
//! negative), so a self-playing piece can already be in full swing on the first sample. `bpm` sets
//! every clock's tempo; `run` (default `-preroll`) is when the clocks start. `macros` sets
//! `{"k": value}` before the engine starts, `ramps` moves macro k at runtime (`[k, t0, t1, v0, v1]`,
//! like a hand on the knob), `cues` launches cue n at once (`[t, n]`), `hits` plays the keyboard
//! (`[t, note, velocity|0]`). Each segment is written as `OUT_DIR/<name>.wav` (32-bit float, 48 kHz).
//!
//! `list` prints each patch's clock tempo, macros (number, name, stored value) and cues, to write
//! the spec from.
//!
//! `video` draws the real UI (look B, dark, 1920x1080, 60 frames a second of virtual time) for
//! every cut of the spec, which then also needs `bar_secs`, `end` (bars) and each segment's `bar`,
//! `bars`, `layer`, `offset` and `visual`. Each cut runs its segment through the engine exactly as
//! `render` does, so the faces, cables, knobs and pads move with the sound in the mix. The visual
//! text picks the view: the Sounds page, Perform, or the Rack. The frames and `MIX.wav` are muxed
//! into `OUT.mp4` (H.264 and AAC, needs `ffmpeg`), and a still of every cut is written next to it
//! in `OUT-stills/`. The spec's `captions` (`[{bar, bars, text}]`) are drawn as one line each in
//! the lower third. Nothing is played: the engine's audio is thrown away.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};

use basedrop::Collector;
use eframe::egui;
use egui::{Align2, Color32, FontId, Pos2, Rect};
use kabl_core::{ModuleId, PatchState};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::{Command, PatchEngine, Timing};
use kabl_engine::runtime::RuntimeTarget;
use kabl_modules::builtins::Transport;
use kabl_modules::registry;
use kabl_ui::cable_life::{CableLife, Intensity};
use kabl_ui::{cues, show, PatchEditor, UiState};
use serde_json::Value;

const SR: f32 = 48000.0;
/// Blocks between runtime writes of a ramped macro (about 5 ms).
const WRITE_EVERY: usize = 4;
const FPS: usize = 60;
const SIZE: (u32, u32) = (1920, 1080);
/// Frames drawn after a cut's view is built and before it is recorded, so the rack's fit (which
/// needs the cables of a drawn frame) and its glide have settled.
const WARM: usize = 24;

fn load(path: &str) -> PatchState {
    let p = Path::new(path);
    let log = if p.extension().is_some_and(|e| e == "kabl") {
        kabl_core::format::load_kabl(p)
    } else {
        kabl_core::load(p)
    };
    log.unwrap_or_else(|e| panic!("load {path}: {e:?}"))
        .state()
        .clone()
}

fn of(state: &PatchState, kind: &str) -> Option<ModuleId> {
    state
        .modules
        .iter()
        .find(|(_, m)| m.kind == kind)
        .map(|(&id, _)| id)
}

fn f(v: &Value) -> f32 {
    v.as_f64().expect("number") as f32
}

fn write_wav(path: &Path, left: &[f32], right: &[f32]) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR as u32,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for (l, r) in left.iter().zip(right) {
        w.write_sample(*l).unwrap();
        w.write_sample(*r).unwrap();
    }
    w.finalize().unwrap();
}

/// What a block did that the UI has to show: a cue launched, a macro moved by a ramp.
enum Event {
    Cue(usize),
    Macro(u64, f32),
}

/// One segment playing through the engine, block by block.
struct Take {
    state: PatchState,
    engine: PatchEngine,
    _collector: Collector,
    clocks: Vec<ModuleId>,
    knob: Option<ModuleId>,
    cue_id: Option<ModuleId>,
    /// (macro k, its param index, t0, t1, v0, v1).
    ramps: Vec<(u64, u16, f32, f32, f32, f32)>,
    cues: Vec<(f32, usize)>,
    hits: Vec<(f32, u8, u8)>,
    preroll: f32,
    run: f32,
    /// Blocks rendered.
    b: usize,
}

impl Take {
    fn new(seg: &Value) -> Take {
        let mut state = load(seg["patch"].as_str().expect("patch"));
        let knob = of(&state, "macro");
        let clocks: Vec<ModuleId> = state
            .modules
            .iter()
            .filter(|(_, m)| m.kind == "clock")
            .map(|(&id, _)| id)
            .collect();
        if let Some(bpm) = seg.get("bpm").and_then(Value::as_f64) {
            for c in &clocks {
                state
                    .modules
                    .get_mut(c)
                    .unwrap()
                    .params
                    .insert("bpm".into(), bpm as f32);
            }
        }
        if let (Some(m), Some(k)) = (seg.get("macros").and_then(Value::as_object), knob) {
            for (n, v) in m {
                state
                    .modules
                    .get_mut(&k)
                    .unwrap()
                    .params
                    .insert(format!("m{n}"), f(v));
            }
        }
        let list = |key: &str| {
            seg.get(key)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        };
        let macro_params = registry::create("macro").unwrap().info().params;
        let ramps = list("ramps")
            .iter()
            .map(|rp| {
                let k = rp[0].as_u64().expect("k");
                let index = macro_params
                    .iter()
                    .position(|p| p.name == format!("m{k}"))
                    .expect("macro k") as u16;
                (k, index, f(&rp[1]), f(&rp[2]), f(&rp[3]), f(&rp[4]))
            })
            .collect();
        let cues = list("cues")
            .iter()
            .map(|c| (f(&c[0]), c[1].as_u64().expect("cue n") as usize))
            .collect();
        let hits = list("hits")
            .iter()
            .map(|h| {
                let note = h[1].as_u64().expect("note") as u8;
                (f(&h[0]), note, h[2].as_u64().expect("velocity") as u8)
            })
            .collect();
        let preroll = seg.get("preroll").map_or(0.0, f);
        let collector = Collector::new();
        let engine = PatchEngine::new(&collector.handle(), &state, SR, 8).expect("compiles");
        Take {
            cue_id: of(&state, "cues"),
            state,
            engine,
            _collector: collector,
            clocks,
            knob,
            ramps,
            cues,
            hits,
            preroll,
            run: seg.get("run").map_or(-preroll, f),
            b: 0,
        }
    }

    /// Start of the next block, in seconds from the start of the written audio.
    fn t(&self) -> f32 {
        (self.b * BLOCK) as f32 / SR - self.preroll
    }

    fn block(&mut self, l: &mut [f32; BLOCK], r: &mut [f32; BLOCK], events: &mut Vec<Event>) {
        let t0 = self.t();
        let t1 = t0 + BLOCK as f32 / SR;
        let now = |t: f32| t >= t0 && t < t1;
        if self.b == 0 {
            for &c in &self.clocks {
                self.engine.transport(c, Transport::Stop);
            }
        }
        if now(self.run) || (self.b == 0 && self.run < t0) {
            for &c in &self.clocks {
                self.engine.transport(c, Transport::Run);
            }
        }
        for &(_, n) in self.cues.iter().filter(|c| now(c.0)) {
            let mut cmds = Vec::new();
            let id = self.cue_id.expect("a cues module");
            cues::launch(&mut cmds, &self.state, id, n).expect("launch");
            for mut cmd in cmds {
                if let Command::Launch(la) = &mut cmd {
                    la.timing = Timing::Now;
                }
                self.engine.command(&cmd);
            }
            events.push(Event::Cue(n));
        }
        if let (Some(id), true) = (self.knob, self.b.is_multiple_of(WRITE_EVERY)) {
            for &(k, index, t_a, t_b, v0, v1) in &self.ramps {
                if t0 < t_a || t0 > t_b + 0.05 {
                    continue;
                }
                let x = ((t0 - t_a) / (t_b - t_a).max(1e-3)).clamp(0.0, 1.0);
                let v = v0 + (v1 - v0) * x;
                self.engine.active_mut().set_runtime(
                    RuntimeTarget::Param {
                        id,
                        kind: "macro",
                        index,
                    },
                    v,
                    true,
                );
                events.push(Event::Macro(k, v));
            }
        }
        for &(_, note, v) in self.hits.iter().filter(|h| now(h.0)) {
            self.engine.key(if v > 0 {
                KeyEvent::On { note, velocity: v }
            } else {
                KeyEvent::Off { note }
            });
        }
        self.engine.process_block(l, r);
        self.b += 1;
    }
}

fn render(seg: &Value) -> (Vec<f32>, Vec<f32>) {
    let mut take = Take::new(seg);
    let secs = f(&seg["secs"]);
    let (mut left, mut right) = (Vec::new(), Vec::new());
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    let blocks = ((secs + take.preroll) * SR) as usize / BLOCK + 1;
    let mut events = Vec::new();
    for _ in 0..blocks {
        take.block(&mut l, &mut r, &mut events);
        events.clear();
        left.extend_from_slice(&l);
        right.extend_from_slice(&r);
    }
    let skip = (take.preroll * SR).round() as usize;
    let n = (secs * SR).round() as usize;
    let cut = |v: Vec<f32>| v[skip..skip + n].to_vec();
    (cut(left), cut(right))
}

#[derive(Clone, Copy, PartialEq)]
enum Scene {
    Sounds,
    Perform,
    Rack,
}

/// One cut of the video: frames `start..end`, showing segment `seg` from `offset` seconds in.
struct Slot {
    seg: Value,
    start: usize,
    end: usize,
    scene: Scene,
}

/// The cut on screen: its take, the patch as the UI sees it, the view.
struct Cur {
    take: Take,
    editor: PatchEditor,
    ui: UiState,
    warm: usize,
}

/// The screenshot the app waits for: the bare UI, or the finished frame drawn over it.
#[derive(Clone, Copy, PartialEq)]
enum Shot {
    Ui,
    Frame,
}

struct Video {
    slots: Vec<Slot>,
    si: usize,
    cur: Option<Cur>,
    /// The frame being drawn; frames before it are written.
    frame: usize,
    total: usize,
    /// Captions as frames `start..end` and their text.
    captions: Vec<(usize, usize, String)>,
    /// The bare UI of this frame, which the finished frame scales and draws over.
    shot: Option<egui::TextureHandle>,
    want: Option<Shot>,
    /// The UI shot of this frame is in: draw the finished frame next.
    compose: bool,
    beat: f32,
    ffmpeg: Child,
    stills: PathBuf,
    /// Frames to keep as stills, with their file names.
    keep: Vec<(usize, String)>,
    user: tempfile::TempDir,
    idle: usize,
}

fn slug(seg: &Value) -> String {
    Path::new(seg["patch"].as_str().expect("patch"))
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

fn title(slug: &str) -> String {
    slug.split('-')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map_or(String::new(), |h| h.to_uppercase().chain(c).collect())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

impl Video {
    fn open(&self, slot: &Slot) -> Cur {
        let seg = &slot.seg;
        let mut take = Take::new(seg);
        let editor = PatchEditor::seed_from(&take.state);
        // Without scene pads, Perform is a row of four knobs in an empty page: the rack
        // shows the same move with the patch around it.
        let scene = match slot.scene {
            Scene::Perform if take.cue_id.is_none() => Scene::Rack,
            s => s,
        };
        let mut ui = UiState::default();
        ui.library = Some(kabl_ui::library::Library::open(
            Some(kabl_ui::library::built_factory_dir()),
            self.user.path().to_path_buf(),
        ));
        ui.theme = "kabl.look_b".into();
        ui.dark = true;
        ui.drawer_open = false;
        ui.browser_open = scene == Scene::Sounds;
        ui.perform_open = scene == Scene::Perform;
        ui.perform_tall = scene == Scene::Perform;
        ui.inspect.audio = true;
        ui.cable_life = CableLife::GlowPulses;
        ui.cable_intensity = Intensity::High;
        ui.status_bpm = seg.get("bpm").and_then(Value::as_f64).map(|b| b as f32);
        let slug = slug(seg);
        let origin = if seg["patch"].as_str().unwrap().ends_with(".kabl") {
            kabl_ui::browser::DocOrigin::New
        } else {
            kabl_ui::browser::DocOrigin::Library(format!("factory:{slug}"))
        };
        ui.doc = Some(kabl_ui::browser::Doc::new(
            &title(&slug),
            origin,
            editor.state(),
        ));
        if scene == Scene::Sounds {
            ui.browser.filter = kabl_ui::browser::Filter::Factory;
            ui.browser.query = title(&slug);
            ui.browser.selected = Some(format!("factory:presets/{slug}"));
        }
        // The take plays up to where the cut picks it up; its faces start from there.
        let offset = seg.get("offset").map_or(0.0, f);
        let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
        let mut events = Vec::new();
        while take.t() < offset {
            take.block(&mut l, &mut r, &mut events);
        }
        let mut cur = Cur {
            take,
            editor,
            ui,
            warm: 0,
        };
        cur.apply(events);
        cur
    }
}

impl Cur {
    fn apply(&mut self, events: Vec<Event>) {
        if events.is_empty() {
            return;
        }
        for e in events {
            match e {
                Event::Macro(k, v) => {
                    if let Some(id) = self.take.knob {
                        self.editor.set_param(id, &format!("m{k}"), v);
                    }
                }
                Event::Cue(n) => {
                    let Some(id) = self.take.cue_id else { continue };
                    let Some(c) = cues::cues(&self.take.state, id)
                        .into_iter()
                        .find(|c| c.n == n)
                    else {
                        continue;
                    };
                    for &(s, b) in &c.targets {
                        self.ui.seq_banks.insert(s, (b, None));
                    }
                    for &(m, k, v) in &c.macros {
                        self.editor.set_param(m, &format!("m{}", k + 1), v);
                    }
                    self.ui.last_cue.insert(id, n);
                }
            }
        }
        // The moves are the performance, not edits: keep the title free of the unsaved mark.
        if let Some(doc) = self.ui.doc.as_mut() {
            doc.saved = self.editor.state().clone();
        }
    }

    /// The engine up to `until` seconds into the segment, its faces and clocks shown at `now`.
    fn play(&mut self, until: f32, now: f64) {
        let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
        let mut events = Vec::new();
        while self.take.t() < until {
            self.take.block(&mut l, &mut r, &mut events);
            if let Some(rep) = self.take.engine.take_face_report() {
                self.ui.live.accept(&rep, now);
            }
        }
        self.apply(events);
        for c in std::mem::take(&mut self.ui.launches) {
            self.take.engine.command(&c);
        }
        let t = self.take.t();
        for &id in &self.take.clocks {
            let bpm = self.take.state.modules[&id]
                .params
                .get("bpm")
                .copied()
                .unwrap_or(120.0);
            let pulses = f64::from((t - self.take.run).max(0.0) * bpm / 60.0 * 4.0);
            self.ui.clock_pos.insert(id, pulses);
        }
    }
}

/// Raised-cosine 0..1 over x in 0..1.
fn ease(x: f32) -> f32 {
    0.5 - 0.5 * (x.clamp(0.0, 1.0) * std::f32::consts::PI).cos()
}

impl eframe::App for Video {
    fn ui(&mut self, root_ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = root_ui.ctx().clone();
        for e in ctx.input(|i| i.raw.events.clone()) {
            let egui::Event::Screenshot { image, .. } = e else {
                continue;
            };
            match self.want.take() {
                Some(Shot::Ui) => {
                    let img = (*image).clone();
                    match &mut self.shot {
                        Some(t) => t.set(img, egui::TextureOptions::LINEAR),
                        None => {
                            self.shot =
                                Some(ctx.load_texture("ui", img, egui::TextureOptions::LINEAR));
                        }
                    }
                    self.compose = true;
                }
                Some(Shot::Frame) => {
                    let raw: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                    self.ffmpeg.stdin.as_mut().unwrap().write_all(&raw).unwrap();
                    for (_, name) in self.keep.iter().filter(|k| k.0 == self.frame) {
                        let p = self.stills.join(name);
                        image::save_buffer(&p, &raw, SIZE.0, SIZE.1, image::ColorType::Rgba8)
                            .unwrap();
                    }
                    self.frame += 1;
                    self.compose = false;
                    self.idle = 0;
                    if self.frame.is_multiple_of(FPS) {
                        println!("  {} s", self.frame / FPS);
                    }
                }
                None => {}
            }
        }
        if self.frame >= self.total {
            drop(self.ffmpeg.stdin.take());
            assert!(self.ffmpeg.wait().unwrap().success(), "ffmpeg failed");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        ctx.request_repaint();
        // A new frame moves the take on; while a shot is pending, and for the finished frame,
        // the UI is drawn again at the same moment so nothing in it moves or restarts.
        let fresh = self.want.is_none() && !self.compose;
        if fresh && self.frame >= self.slots[self.si].end {
            self.si += 1;
            self.cur = None;
        }
        let slot = &self.slots[self.si];
        if self.cur.is_none() {
            println!("{} at frame {}", slot.seg["name"], slot.start);
            self.cur = Some(self.open(slot));
        }
        let cur = self.cur.as_mut().unwrap();
        let now = self.frame as f64 / FPS as f64;
        let dt = if fresh { 1.0 / FPS as f32 } else { 0.0 };
        ctx.input_mut(|i| {
            i.time = now;
            i.stable_dt = dt;
            i.unstable_dt = dt;
        });
        let (w, h) = (SIZE.0 as f32, SIZE.1 as f32);
        let local = (self.frame - slot.start) as f32 / FPS as f32;
        let span = (slot.end - slot.start) as f32 / FPS as f32;
        let visual = slot.seg["visual"].as_str().unwrap_or("");
        let recording = cur.warm >= WARM;
        if fresh && recording {
            let offset = slot.seg.get("offset").map_or(0.0, f);
            cur.play(offset + local + 1.0 / FPS as f32, now);
            if visual.contains("palette") {
                let all = kabl_ui::style::CablePalette::ALL;
                cur.ui.cable_palette = Some(all[(local / self.beat) as usize % all.len()]);
            }
        }
        let mut ui = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::ZERO, egui::vec2(w, h))),
        );
        show(&mut cur.editor, &mut cur.ui, &mut ui);
        if !recording {
            cur.warm += 1;
            return;
        }
        // A shot may come from a frame after the one that asked for it, so every frame until
        // it arrives draws the same picture.
        let ask = |want: &mut Option<Shot>, idle: &mut usize, shot: Shot| {
            if want.is_none() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                *want = Some(shot);
            } else {
                *idle += 1;
                assert!(*idle < 120, "screenshots stopped");
            }
        };
        if !self.compose {
            ask(&mut self.want, &mut self.idle, Shot::Ui);
            return;
        }
        // The finished frame covers the UI: its shot, scaled for the camera push (the UI
        // itself stays at one zoom, so its labels never switch detail mid-move), and the
        // overlays on top.
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Debug,
            egui::Id::new("promo frame"),
        ));
        let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(w, h));
        let z = if visual.contains("push") {
            1.0 + 0.18 * ease(local / span)
        } else {
            1.0
        };
        let uv = Rect::from_center_size(Pos2::new(0.5, 0.5), egui::vec2(1.0 / z, 1.0 / z));
        painter.image(self.shot.as_ref().unwrap().id(), screen, uv, Color32::WHITE);
        let shade = |a: f32, c: Color32| {
            if a > 0.0 {
                painter.rect_filled(screen, 0.0, c.gamma_multiply(a.min(1.0)));
            }
        };
        let last = self.si + 1 == self.slots.len();
        if last {
            // One beat of white flash on the rack, then the end card.
            shade(
                0.9 * (1.0 - local / self.beat).max(0.0).powi(2),
                Color32::WHITE,
            );
            if local >= self.beat {
                shade(
                    ease((local - self.beat) / 0.2),
                    Color32::from_rgb(12, 13, 16),
                );
                let a = ease((local - self.beat - 0.1) / 0.4);
                let c = Pos2::new(w / 2.0, h / 2.0);
                painter.text(
                    c - egui::vec2(0.0, 40.0),
                    Align2::CENTER_CENTER,
                    "kabl 1.0",
                    FontId::proportional(150.0),
                    Color32::WHITE.gamma_multiply(a),
                );
                painter.text(
                    c + egui::vec2(0.0, 80.0),
                    Align2::CENTER_CENTER,
                    "a whole song in one patch",
                    FontId::proportional(48.0),
                    Color32::from_gray(170).gamma_multiply(a),
                );
            }
        }
        if visual.contains("dims") {
            shade(0.45 * ease(local / span), Color32::BLACK);
        }
        // Captions: one line on a dark plate in the lower third, in and out over half a beat
        // on their bar lines.
        let fade = self.beat / 2.0 * FPS as f32;
        for (s, e, text) in &self.captions {
            if !(*s..*e).contains(&self.frame) {
                continue;
            }
            let a = ease((self.frame - s) as f32 / fade).min(ease((e - self.frame) as f32 / fade));
            let galley = painter.layout_no_wrap(
                text.clone(),
                FontId::proportional(44.0),
                Color32::WHITE.gamma_multiply(a),
            );
            let at = Pos2::new(w / 2.0, h - 120.0);
            let plate = Rect::from_center_size(at, galley.size() + egui::vec2(56.0, 28.0));
            painter.rect_filled(
                plate,
                12.0,
                Color32::from_black_alpha(170).gamma_multiply(a),
            );
            painter.galley(at - galley.size() / 2.0, galley, Color32::WHITE);
        }
        // In from black; out to black with the mix's last 1.5 s.
        shade(1.0 - now as f32 / 0.4, Color32::BLACK);
        let left = (self.total - self.frame) as f32 / FPS as f32;
        shade(1.0 - ease(left / 1.5), Color32::BLACK);
        ask(&mut self.want, &mut self.idle, Shot::Frame);
    }
}

fn video(spec: &Value, audio: &Path, out: &Path) {
    let bar = f(&spec["bar_secs"]);
    let at = |bars: f32| (bars * bar * FPS as f32).round() as usize;
    let mut cuts: Vec<&Value> = spec["segments"]
        .as_array()
        .expect("segments")
        .iter()
        .filter(|s| !s["layer"].as_bool().unwrap_or(false))
        .collect();
    cuts.sort_by(|a, b| f(&a["bar"]).total_cmp(&f(&b["bar"])));
    let total = at(f(&spec["end"]));
    let mut slots: Vec<Slot> = Vec::new();
    for (i, s) in cuts.iter().enumerate() {
        let visual = s["visual"].as_str().unwrap_or("");
        let scene = if visual.contains("Perform") {
            Scene::Perform
        } else if visual.contains("Sounds page") || visual.contains(" card") {
            Scene::Sounds
        } else if ["Rack", "rack", "cable", "module", "flash"]
            .iter()
            .any(|w| visual.contains(w))
        {
            Scene::Rack
        } else {
            // A knob move: Perform shows the macros largest.
            Scene::Perform
        };
        slots.push(Slot {
            seg: (*s).clone(),
            start: at(f(&s["bar"])),
            end: cuts.get(i + 1).map_or(total, |n| at(f(&n["bar"]))),
            scene,
        });
    }
    let stills = out.with_extension("").with_file_name(format!(
        "{}-stills",
        out.file_stem().unwrap().to_string_lossy()
    ));
    std::fs::create_dir_all(&stills).unwrap();
    // A still just after every cut, one mid-cut for the long ones, and one of the end card.
    let mut keep = Vec::new();
    for s in &slots {
        let name = s.seg["name"].as_str().unwrap();
        let k = s.start + 12;
        keep.push((k, format!("{k:05}-{name}.png")));
        if s.end - s.start > 2 * FPS {
            let m = (s.start + s.end) / 2;
            keep.push((m, format!("{m:05}-{name}-mid.png")));
        }
    }
    let captions: Vec<(usize, usize, String)> = spec["captions"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .map(|c| {
            let s = at(f(&c["bar"]));
            let text = c["text"].as_str().expect("caption text").to_owned();
            (s, at(f(&c["bar"]) + f(&c["bars"])), text)
        })
        .collect();
    // A still in the middle of every caption.
    for (i, (s, e, _)) in captions.iter().enumerate() {
        let m = (s + e) / 2;
        keep.push((m, format!("{m:05}-caption-{:02}.png", i + 1)));
    }
    keep.push((
        total - 2 * FPS,
        format!("{:05}-end-card.png", total - 2 * FPS),
    ));
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
        .args(["-s", &format!("{}x{}", SIZE.0, SIZE.1)])
        .args(["-r", &FPS.to_string(), "-i", "-", "-i"])
        .arg(audio)
        .args([
            "-map", "0:v", "-map", "1:a", "-c:v", "libx264", "-preset", "medium",
        ])
        .args([
            "-crf", "18", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "320k",
        ])
        .args(["-movflags", "+faststart", "-shortest"])
        .arg(out)
        .stdin(Stdio::piped())
        .spawn()
        .expect("ffmpeg");
    let app = Video {
        slots,
        si: 0,
        cur: None,
        frame: 0,
        total,
        captions,
        shot: None,
        want: None,
        compose: false,
        beat: bar / 4.0,
        ffmpeg,
        stills,
        keep,
        user: tempfile::tempdir().expect("a user folder"),
        idle: 0,
    };
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([SIZE.0 as f32, SIZE.1 as f32])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "promo video",
        opts,
        Box::new(move |cc| {
            cc.egui_ctx.set_pixels_per_point(1.0);
            Ok(Box::new(app))
        }),
    )
    .unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("render") if args.len() == 4 => {
            let spec: Value =
                serde_json::from_str(&std::fs::read_to_string(&args[2]).expect("spec"))
                    .expect("json");
            let out = Path::new(&args[3]);
            std::fs::create_dir_all(out).unwrap();
            for seg in spec["segments"].as_array().expect("segments") {
                let name = seg["name"].as_str().expect("name");
                let (l, r) = render(seg);
                write_wav(&out.join(format!("{name}.wav")), &l, &r);
                let peak = l.iter().chain(&r).fold(0.0f32, |m, s| m.max(s.abs()));
                println!("{name:28} peak {:6.1} dBFS", 20.0 * peak.max(1e-9).log10());
            }
        }
        Some("video") if args.len() == 5 => {
            let spec: Value =
                serde_json::from_str(&std::fs::read_to_string(&args[2]).expect("spec"))
                    .expect("json");
            video(&spec, Path::new(&args[3]), Path::new(&args[4]));
        }
        Some("list") => {
            for path in &args[2..] {
                let state = load(path);
                let bpm: Vec<f32> = state
                    .modules
                    .values()
                    .filter(|m| m.kind == "clock")
                    .map(|m| m.params.get("bpm").copied().unwrap_or(120.0))
                    .collect();
                let mut macros = Vec::new();
                if let Some(k) = of(&state, "macro") {
                    let p = &state.modules[&k].params;
                    for n in (1..=8).filter(|n| p.contains_key(&format!("pin.m{n}"))) {
                        let name =
                            kabl_ui::macro_name(&state, k, &format!("m{n}")).unwrap_or_default();
                        let v = p.get(&format!("m{n}")).copied().unwrap_or(0.0);
                        macros.push(format!("m{n} {name}={v:.2}"));
                    }
                }
                let cue_names: Vec<String> = of(&state, "cues")
                    .map(|id| {
                        cues::cues(&state, id)
                            .into_iter()
                            .map(|c| format!("{} {}", c.n, c.name))
                            .collect()
                    })
                    .unwrap_or_default();
                println!(
                    "{path} | bpm {bpm:?} | {} | cues: {}",
                    macros.join(", "),
                    cue_names.join(", ")
                );
            }
        }
        _ => eprintln!(
            "usage: promo_render render SPEC.json OUT_DIR | video SPEC.json MIX.wav OUT.mp4 \
             | list PATCH [PATCH ...]"
        ),
    }
}
