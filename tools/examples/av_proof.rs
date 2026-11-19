//! AV proof of concept (docs/av-audit/KABL_AV_ARCHITECTURE_AND_COMPETITION_PLAN.md §5, Track A):
//! renders a self-playing patch offline, *performs* it (the patch's own cues, or sequencer bank
//! launches, plus an optional lead phrase), and draws a video that follows the patch's structure:
//! sequencer and drum-grid steps, envelopes, clock ticks, macros, cue changes and lead notes,
//! read through the engine's face taps. The spectrum ring is the only element computed from the
//! mix, on the visual side. Nothing is played aloud.
//!
//!     cargo run --release -p kabl-tools --example av_proof -- PATCH OUT.mp4 [--secs 72] [--fps 60]
//!         [--size 1920x1080] [--no-lead] [--title "kabl"]
//!
//! Pass 1 renders audio through the production `Timeline` and records one tap report per video
//! frame. Pass 2 draws each frame on the CPU (threaded) and pipes raw RGB plus the WAV into ffmpeg,
//! which also sets the titles. Throwaway proof: no product code depends on it.

// Pixel and channel loops index several buffers at once; a throwaway drawing tool.
#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

use basedrop::Collector;
use kabl_core::{ModuleId, PatchState};
use kabl_engine::facetap::{FaceTarget, FaceTargets, MAX_FACE_TAPS};
use kabl_engine::keyboard::{MidiEvent, Source};
use kabl_engine::patch_engine::{Command, Launch, PatchEngine, Timing};
use kabl_engine::runtime::{Feedback, ParamSet, RuntimeTarget, ToAudio};
use kabl_engine::timeline::{Timeline, LATENCY};
use kabl_modules::builtins::Transport;
use kabl_modules::PortDirection;
use std::f32::consts::TAU;
use std::io::Write;
use std::process::{Command as Proc, Stdio};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Role {
    Step,
    Pitch,
    Env,
    Clock,
    Lfo,
    Macro,
    LeadGate,
    LeadPitch,
}

#[derive(Clone, Copy)]
struct Tap {
    role: Role,
    lane: usize,
}

#[derive(Clone, Copy, Default)]
struct Sample {
    last: f32,
    min: f32,
    max: f32,
}

struct Frame {
    taps: Vec<Sample>,
    /// A new tap report arrived during this frame (edges are only counted on fresh reports).
    fresh: bool,
    rms: f32,
    scope: Vec<f32>,
    /// Index of the section (cue) requested at or before this frame.
    section: usize,
    /// The lead note held at this frame (from the played schedule) and whether it began now.
    lead: Option<u8>,
    lead_new: bool,
}

const SCOPE: usize = 2048;
const RATE: f32 = 48000.0;

struct Section {
    at: f32,
    name: String,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (patch, out) = (&args[0], &args[1]);
    let opt = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .map(|i| args[i + 1].clone())
    };
    let secs: f32 = opt("--secs").map_or(72.0, |s| s.parse().unwrap());
    let fps: u32 = opt("--fps").map_or(60, |s| s.parse().unwrap());
    let (w, h) = opt("--size").map_or((1920usize, 1080usize), |s| {
        let (a, b) = s.split_once('x').unwrap();
        (a.parse().unwrap(), b.parse().unwrap())
    });
    let lead = !args.iter().any(|a| a == "--no-lead");
    let title = opt("--title").unwrap_or_else(|| "kabl".into());

    let log = kabl_core::load(std::path::Path::new(patch)).expect("patch");
    let state: PatchState = log.state().clone();

    // ---- taps ---------------------------------------------------------------------------
    let mut targets: Vec<FaceTarget> = Vec::new();
    let mut taps: Vec<Tap> = Vec::new();
    let mut lanes = [0usize; 8];
    let (mut clocks, mut seqs, mut cues_id, mut macro_id) = (vec![], vec![], None, None);
    for pass in 0..3 {
        for (&id, m) in &state.modules {
            let Some(info) = kabl_modules::registry::info_for(&m.kind) else {
                continue;
            };
            let outs: Vec<&str> = info
                .ports
                .iter()
                .filter(|p| p.direction == PortDirection::Output)
                .map(|p| p.name)
                .collect();
            let port = |n: &str| outs.iter().position(|o| *o == n).map(|i| i as u8);
            let mut add = |port: u8, role: Role| {
                if targets.len() < MAX_FACE_TAPS {
                    taps.push(Tap {
                        role,
                        lane: lanes[role as usize],
                    });
                    lanes[role as usize] += 1;
                    targets.push(FaceTarget {
                        module: id,
                        port,
                        kind: info.kind,
                        view: false,
                    });
                }
            };
            match (pass, info.kind) {
                (0, "clock") => {
                    clocks.push(id);
                    if let Some(g) = port("gate") {
                        add(g, Role::Clock);
                    }
                }
                (0, "seq") => {
                    seqs.push(id);
                    if let (Some(g), Some(p)) = (port("gate"), port("pitch")) {
                        add(g, Role::Step);
                        add(p, Role::Pitch);
                    }
                }
                (0, "drum.grid") => {
                    for r in 0..outs.len().min(6) {
                        add(r as u8, Role::Step);
                    }
                }
                (0, "cues") => cues_id = Some(id),
                (1, "midi.in") => {
                    if let (Some(g), Some(p)) = (port("gate"), port("pitch")) {
                        add(g, Role::LeadGate);
                        add(p, Role::LeadPitch);
                    }
                }
                (1, "env.adsr") => add(0, Role::Env),
                (2, "lfo") => add(0, Role::Lfo),
                (2, "macro") => {
                    macro_id = Some((id, info.kind));
                    for k in 0..4 {
                        add(k, Role::Macro);
                    }
                }
                _ => {}
            }
        }
    }

    // ---- the performance ----------------------------------------------------------------
    // Sections: the patch's own cues if it has them, else sequencer bank launches A-D.
    let mut sections: Vec<Section> = Vec::new();
    let mut launches: Vec<(f32, Vec<Command>)> = Vec::new();
    let cue_list = cues_id
        .map(|id| kabl_ui::cues::cues(&state, id))
        .unwrap_or_default();
    let cue_list: Vec<_> = cue_list.into_iter().filter(|c| c.clock.is_some()).collect();
    if let (Some(id), false) = (cues_id, cue_list.is_empty()) {
        let span = secs / cue_list.len() as f32;
        for (k, c) in cue_list.iter().enumerate() {
            let at = if k == 0 { 0.05 } else { k as f32 * span - 1.0 };
            let mut cmds = Vec::new();
            if kabl_ui::cues::launch(&mut cmds, &state, id, c.n).is_ok() {
                sections.push(Section {
                    at,
                    name: c.name.clone(),
                });
                launches.push((at, cmds));
            }
        }
    } else if let (Some(&clock), false) = (clocks.first(), seqs.is_empty()) {
        for k in 0..4u8 {
            let at = if k == 0 {
                0.05
            } else {
                k as f32 * secs / 4.0 - 1.0
            };
            let tg: Vec<(ModuleId, u8)> = seqs.iter().map(|&s| (s, k)).collect();
            sections.push(Section {
                at,
                name: format!("Bank {}", (b'A' + k) as char),
            });
            launches.push((
                at,
                vec![Command::Launch(Launch::new(clock, Timing::NextBar, &tg))],
            ));
        }
    }
    if sections.is_empty() {
        sections.push(Section {
            at: 0.0,
            name: String::new(),
        });
    }
    // A slow lead phrase (E minor, E4..B4) in the second and last thirds, if the patch has a keyboard.
    let mut midi: Vec<(u64, MidiEvent, Option<u8>)> = Vec::new();
    if lead && lanes[Role::LeadGate as usize] > 0 {
        let phrase = [64u8, 67, 69, 71, 69, 67, 66, 64];
        for start in [secs * 0.36, secs * 0.70] {
            for (k, &n) in phrase.iter().enumerate() {
                let t0 = start + k as f32 * 1.1;
                let t1 = t0 + if k == phrase.len() - 1 { 2.4 } else { 0.95 };
                let on = MidiEvent::parse(Source::Controller, &[0x90, n, 92]).unwrap();
                let off = MidiEvent::parse(Source::Controller, &[0x80, n, 0]).unwrap();
                midi.push(((t0 * RATE) as u64, on, Some(n)));
                midi.push(((t1 * RATE) as u64, off, None));
            }
        }
        midi.sort_by_key(|e| e.0);
    }

    eprintln!(
        "taps {} | steps {} envs {} clocks {} macros {} lead {} | sections: {}",
        taps.len(),
        lanes[Role::Step as usize],
        lanes[Role::Env as usize],
        lanes[Role::Clock as usize],
        lanes[Role::Macro as usize],
        lanes[Role::LeadGate as usize] > 0 && lead,
        sections
            .iter()
            .map(|s| format!("{:.0}s {}", s.at, s.name))
            .collect::<Vec<_>>()
            .join(", ")
    );

    // ---- pass 1: audio + telemetry --------------------------------------------------------
    let collector = Collector::new();
    let mut engine = PatchEngine::new(&collector.handle(), &state, RATE, 8).expect("compiles");
    engine.face_taps(Some(FaceTargets::new(1, &targets)));
    let (mut tx, mut rx) = rtrb::RingBuffer::<ToAudio>::new(64);
    let fb = Feedback::default();
    let sweep = cues_id.is_none();
    let macro_base: Vec<f32> = (0..4)
        .map(|k| {
            macro_id
                .and_then(|(id, _)| {
                    state.modules[&id]
                        .params
                        .get(&format!("m{}", k + 1))
                        .copied()
                })
                .unwrap_or(0.5)
        })
        .collect();
    let mut timeline = Box::new(Timeline::new());
    let per_frame = (RATE as u32 / fps) as usize;
    let n_frames = (secs * fps as f32) as usize;
    let total = n_frames * per_frame + LATENCY;
    let (mut l, mut r) = (vec![0f32; per_frame], vec![0f32; per_frame]);
    let mut audio: Vec<f32> = Vec::with_capacity(total * 2);
    let mut frames: Vec<Frame> = Vec::with_capacity(n_frames + 2);
    let mut last = vec![Sample::default(); taps.len()];
    let (mut done, mut next_launch, mut next_midi, mut section) = (0usize, 0usize, 0usize, 0usize);
    let mut started = false;
    let mut held: Option<u8> = None;
    while done < total {
        let n = per_frame.min(total - done);
        let t = done as f32 / RATE;
        if !started {
            for &c in &clocks {
                engine.transport(c, Transport::Run);
            }
            started = true;
        }
        while next_launch < launches.len() && launches[next_launch].0 <= t {
            for c in &launches[next_launch].1 {
                engine.command(c);
            }
            section = next_launch;
            next_launch += 1;
        }
        if let (true, Some((id, kind))) = (sweep, macro_id) {
            let ramp = ((t - 6.0) / 10.0).clamp(0.0, 1.0);
            for k in 0..4 {
                let period = [23.0, 31.0, 17.0, 41.0][k];
                let v = macro_base[k] + ramp * 0.3 * (TAU * t / period + k as f32).sin();
                let _ = tx.push(ToAudio::Set(ParamSet {
                    rev: 1 << 40,
                    target: RuntimeTarget::Param {
                        id,
                        kind,
                        index: k as u16,
                    },
                    value: v.clamp(0.0, 1.0),
                }));
            }
            engine.drain(&mut rx, 64, &fb);
        }
        let mut batch = Vec::new();
        let prev_held = held;
        while next_midi < midi.len() && midi[next_midi].0 < (done + n) as u64 {
            batch.push((
                (midi[next_midi].0.saturating_sub(done as u64)) as usize,
                midi[next_midi].1,
            ));
            held = midi[next_midi].2;
            next_midi += 1;
        }
        let lead_new = held.is_some() && held != prev_held;
        timeline.process(&mut engine, &mut l[..n], &mut r[..n], &batch, |_, _| {});
        let mut sq = 0f32;
        for i in 0..n {
            audio.push(l[i]);
            audio.push(r[i]);
            sq += l[i] * l[i] + r[i] * r[i];
        }
        let mut fresh = false;
        if let Some(rep) = engine.take_face_report() {
            fresh = true;
            for (s, e) in rep.entries.iter().take(rep.n as usize).enumerate() {
                last[s] = Sample {
                    last: e.last,
                    min: e.min,
                    max: e.max,
                };
            }
        }
        let tail = audio.len().saturating_sub(SCOPE * 2);
        let scope = audio[tail..]
            .chunks(2)
            .map(|c| 0.5 * (c[0] + c[1]))
            .collect();
        frames.push(Frame {
            taps: last.clone(),
            fresh,
            rms: (sq / (2 * n) as f32).sqrt(),
            scope,
            section,
            lead: held,
            lead_new,
        });
        done += n;
    }
    // Lanes that never fire are left out of the lattice.
    let mut rises = vec![0u32; taps.len()];
    for (i, f) in frames.iter().enumerate().skip(1) {
        for s in 0..taps.len() {
            rises[s] += rose(f, &frames[i - 1], s) as u32;
        }
    }
    for (s, tp) in taps.iter().enumerate() {
        eprintln!("  tap {s:2} {:?}#{}: rises {}", tp.role, tp.lane, rises[s]);
    }

    let audio = &audio[LATENCY * 2..];
    let wav = format!("{out}.wav");
    {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: RATE as u32,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut wr = hound::WavWriter::create(&wav, spec).unwrap();
        let peak = audio.iter().fold(0f32, |a, s| a.max(s.abs())).max(1e-6);
        let gain = 0.89 / peak; // normalise to -1 dBFS
        for s in audio {
            wr.write_sample(s * gain).unwrap();
        }
        wr.finalize().unwrap();
        eprintln!(
            "audio {:.1} s, peak {:.3} -> -1 dBFS",
            audio.len() as f32 / 2.0 / RATE,
            peak
        );
    }

    // ---- pass 2: frames -> ffmpeg ---------------------------------------------------------
    let font = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../crates/ui/assets/IBMPlexSans-Medium.ttf"
    );
    let mono = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../crates/ui/assets/IBMPlexMono-Regular.ttf"
    );
    let fs = |x: f32| (x * h as f32 / 1080.0) as u32;
    let window = |a: f32, b: f32| {
        format!("if(lt(t,{a}),0,if(lt(t,{a}+0.6),(t-{a})/0.6,if(lt(t,{b}-0.8),1,if(lt(t,{b}),({b}-t)/0.8,0))))")
    };
    let text = |s: &str, font: &str, size: u32, x: &str, y: &str, alpha: String, col: &str| {
        format!("drawtext=fontfile={font}:text='{s}':fontsize={size}:fontcolor={col}:x={x}:y={y}:alpha='{alpha}'")
    };
    let mut vf = vec![
        "fade=t=in:st=0:d=1.2".to_string(),
        format!("fade=t=out:st={}:d=2.5", secs - 2.5),
    ];
    vf.push(text(
        &title,
        font,
        fs(132.0),
        "(w-text_w)/2",
        "(h-text_h)/2-40",
        window(0.8, 4.6),
        "white@0.92",
    ));
    vf.push(text(
        "every light is a signal inside the patch",
        font,
        fs(30.0),
        "(w-text_w)/2",
        &format!("(h/2)+{}", fs(70.0)),
        window(1.4, 4.6),
        "white@0.7",
    ));
    for (k, s) in sections.iter().enumerate() {
        if s.name.is_empty() {
            continue;
        }
        let a = s.at.max(5.0);
        let b = sections
            .get(k + 1)
            .map_or(secs - 3.0, |n| n.at.max(a + 2.0))
            .min(a + 5.0);
        let label = format!("cue {}  ·  {}", k + 1, s.name.replace(['\'', ':'], ""));
        vf.push(text(
            &label,
            mono,
            fs(30.0),
            &fs(56.0).to_string(),
            &format!("h-{}", fs(72.0)),
            window(a, b),
            "white@0.85",
        ));
    }
    let pname = std::path::Path::new(patch)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    vf.push(text(
        &format!("patch  {pname}   ·   offline render   ·   visuals from steps, envelopes, clock, macros, cues"),
        mono,
        fs(18.0),
        &format!("w-text_w-{}", fs(48.0)),
        &fs(40.0).to_string(),
        window(5.0, secs - 3.5),
        "white@0.45",
    ));
    vf.push(text(
        &title,
        font,
        fs(96.0),
        "(w-text_w)/2",
        "(h-text_h)/2-30",
        window(secs - 5.5, secs - 0.3),
        "white@0.92",
    ));
    vf.push(text(
        "structure-reactive audiovisual  ·  proof of concept",
        font,
        fs(26.0),
        "(w-text_w)/2",
        &format!("(h/2)+{}", fs(56.0)),
        window(secs - 5.0, secs - 0.3),
        "white@0.7",
    ));
    let af = format!("afade=t=in:st=0:d=0.8,afade=t=out:st={}:d=2.5", secs - 2.5);
    let mut ff = Proc::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "error",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
        ])
        .args([
            "-s",
            &format!("{w}x{h}"),
            "-r",
            &fps.to_string(),
            "-i",
            "-",
            "-i",
            &wav,
        ])
        .args(["-vf", &vf.join(","), "-af", &af])
        .args([
            "-c:v", "libx264", "-preset", "medium", "-crf", "16", "-pix_fmt", "yuv420p",
        ])
        .args(["-c:a", "aac", "-b:a", "320k", "-shortest", out])
        .stdin(Stdio::piped())
        .spawn()
        .expect("ffmpeg");
    let stdin = ff.stdin.as_mut().unwrap();
    let active: Vec<usize> = (0..taps.len())
        .filter(|&s| taps[s].role == Role::Step && rises[s] > 0)
        .collect();
    let mut vis = Visual::new(w, h, fps as f32, active);
    let mut rgb = vec![0u8; w * h * 3];
    let t0 = std::time::Instant::now();
    for i in 0..n_frames {
        let prev = if i > 0 { &frames[i - 1] } else { &frames[0] };
        vis.step(i as f32 / fps as f32, &frames[i], prev, &taps);
        vis.compose(&mut rgb, i as u32);
        stdin.write_all(&rgb).unwrap();
        if i % (fps as usize * 10) == 0 {
            eprintln!("frame {i}/{n_frames} ({:.1} s)", t0.elapsed().as_secs_f32());
        }
    }
    drop(ff.stdin.take());
    assert!(ff.wait().unwrap().success(), "ffmpeg failed");
    eprintln!("wrote {out} in {:.0} s", t0.elapsed().as_secs_f32());
}

fn rose(f: &Frame, prev: &Frame, s: usize) -> bool {
    let x = f.taps[s];
    f.fresh && x.max >= 0.5 && (prev.taps[s].last < 0.5 || x.min < 0.5)
}

// ---- the look ----------------------------------------------------------------------------

/// Scene palettes; a cue change moves to the next one with a smooth blend.
const SCENES: [[[f32; 3]; 4]; 5] = [
    [
        [1.00, 0.52, 0.14],
        [0.18, 0.72, 1.00],
        [1.00, 0.86, 0.50],
        [0.85, 0.30, 0.75],
    ], // amber/cyan
    [
        [1.00, 0.24, 0.48],
        [0.38, 0.40, 1.00],
        [1.00, 0.72, 0.40],
        [0.30, 1.00, 0.85],
    ], // rose/indigo
    [
        [0.40, 1.00, 0.55],
        [0.15, 0.55, 1.00],
        [0.95, 1.00, 0.60],
        [1.00, 0.45, 0.25],
    ], // lime/blue
    [
        [0.70, 0.45, 1.00],
        [1.00, 0.40, 0.30],
        [0.60, 0.95, 1.00],
        [1.00, 0.85, 0.35],
    ], // violet/coral
    [
        [1.00, 0.80, 0.35],
        [1.00, 0.35, 0.20],
        [0.55, 0.85, 1.00],
        [0.90, 0.90, 0.95],
    ], // gold/ember
];

struct Ring {
    x: f32,
    y: f32,
    born: f32,
    col: [f32; 3],
    a: f32,
    max_r: f32,
    life: f32,
}

struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    col: [f32; 3],
}

struct Visual {
    w: usize,
    h: usize,
    fps: f32,
    acc: Vec<f32>,
    tmp: Vec<f32>,
    crisp: Vec<f32>,
    small: Vec<f32>,
    small2: Vec<f32>,
    rings: Vec<Ring>,
    parts: Vec<Particle>,
    /// Step lanes shown in the lattice (tap indices).
    lanes: Vec<usize>,
    heat: Vec<[f32; 16]>,
    count: Vec<u32>,
    spec: [f32; 40],
    pal: [[f32; 3]; 4],
    scene: usize,
    spin: f32,
    flash: f32,
    punch: f32,
    rng: u64,
    lead_ang: f32,
    lead_lvl: f32,
    lead_note: f32,
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

impl Visual {
    fn new(w: usize, h: usize, fps: f32, lanes: Vec<usize>) -> Self {
        let n = lanes.len();
        Visual {
            w,
            h,
            fps,
            acc: vec![0.0; w * h * 3],
            tmp: vec![0.0; w * h * 3],
            crisp: vec![0.0; w * h * 3],
            small: vec![0.0; (w / 4) * (h / 4) * 3],
            small2: vec![0.0; (w / 4) * (h / 4) * 3],
            rings: Vec::new(),
            parts: Vec::new(),
            lanes,
            heat: vec![[0.0; 16]; n],
            count: vec![0; n],
            spec: [0.0; 40],
            pal: SCENES[0],
            scene: 0,
            spin: 0.0,
            flash: 0.0,
            punch: 0.0,
            rng: 0x9E37_79B9_7F4A_7C15,
            lead_ang: 0.0,
            lead_lvl: 0.0,
            lead_note: 64.0,
        }
    }

    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Additive soft dot; `crisp` picks the non-feedback layer.
    fn dot(&mut self, crisp: bool, x: f32, y: f32, c: [f32; 3], a: f32, r: isize) {
        let (w, h) = (self.w as isize, self.h as isize);
        let buf = if crisp {
            &mut self.crisp
        } else {
            &mut self.acc
        };
        let (xi, yi) = (x.round() as isize, y.round() as isize);
        if xi < -r || yi < -r || xi >= w + r || yi >= h + r {
            return;
        }
        let k2 = if r <= 1 { 1.6 } else { 0.7 };
        for dy in -r..=r {
            for dx in -r..=r {
                let (px, py) = (xi + dx, yi + dy);
                if px < 0 || py < 0 || px >= w || py >= h {
                    continue;
                }
                let (fx, fy) = (px as f32 - x, py as f32 - y);
                let k = a * (-(fx * fx + fy * fy) * k2).exp();
                let o = (py as usize * self.w + px as usize) * 3;
                buf[o] += c[0] * k;
                buf[o + 1] += c[1] * k;
                buf[o + 2] += c[2] * k;
            }
        }
    }

    fn line(&mut self, crisp: bool, x0: f32, y0: f32, x1: f32, y1: f32, c: [f32; 3], a: f32) {
        let len = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt();
        let n = (len / 0.9).ceil().max(1.0) as usize;
        for i in 0..=n {
            let t = i as f32 / n as f32;
            self.dot(crisp, x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, c, a, 1);
        }
    }

    fn rect(&mut self, cx: f32, cy: f32, hw: f32, hh: f32, c: [f32; 3], a: f32) {
        let (x0, x1) = (
            (cx - hw - 2.0).max(0.0) as usize,
            ((cx + hw + 2.0) as usize).min(self.w - 1),
        );
        let (y0, y1) = (
            (cy - hh - 2.0).max(0.0) as usize,
            ((cy + hh + 2.0) as usize).min(self.h - 1),
        );
        for y in y0..=y1 {
            for x in x0..=x1 {
                let ex = ((x as f32 - cx).abs() - hw).max(0.0);
                let ey = ((y as f32 - cy).abs() - hh).max(0.0);
                let k = a * (1.0 - (ex * ex + ey * ey).sqrt() / 2.0).clamp(0.0, 1.0);
                let o = (y * self.w + x) * 3;
                for ch in 0..3 {
                    self.crisp[o + ch] += c[ch] * k;
                }
            }
        }
    }

    /// Feedback warp, threaded: zoom, turn, a radial twist, a slow hue drift, decay.
    fn feedback(&mut self, zoom: f32, turn: f32, twist: f32, decay: f32, hue: f32) {
        std::mem::swap(&mut self.acc, &mut self.tmp);
        let (w, h) = (self.w, self.h);
        let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
        let rmax = (cx * cx + cy * cy).sqrt();
        // Hue rotation about the grey axis.
        let (s, c) = hue.sin_cos();
        let k = (1.0 - c) / 3.0;
        let q = (1.0f32 / 3.0).sqrt() * s;
        let m = [
            [c + k, k - q, k + q],
            [k + q, c + k, k - q],
            [k - q, k + q, c + k],
        ];
        let src = &self.tmp;
        let threads = std::thread::available_parallelism().map_or(8, |n| n.get());
        let rows = h.div_ceil(threads);
        std::thread::scope(|sc| {
            for (ci, chunk) in self.acc.chunks_mut(rows * w * 3).enumerate() {
                sc.spawn(move || {
                    for (ry, row) in chunk.chunks_mut(w * 3).enumerate() {
                        let y = ci * rows + ry;
                        for x in 0..w {
                            let dx = (x as f32 - cx) / zoom;
                            let dy = (y as f32 - cy) / zoom;
                            let rr = (dx * dx + dy * dy).sqrt() / rmax;
                            let a = turn + twist * rr;
                            let (sa, ca) = a.sin_cos();
                            let sx = cx + dx * ca - dy * sa;
                            let sy = cy + dx * sa + dy * ca;
                            let o = x * 3;
                            if sx < 0.0 || sy < 0.0 || sx >= (w - 1) as f32 || sy >= (h - 1) as f32
                            {
                                row[o..o + 3].fill(0.0);
                                continue;
                            }
                            let (x0, y0) = (sx as usize, sy as usize);
                            let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
                            let i00 = (y0 * w + x0) * 3;
                            let (i10, i01) = (i00 + 3, i00 + w * 3);
                            let i11 = i01 + 3;
                            let mut v = [0f32; 3];
                            for (ch, vv) in v.iter_mut().enumerate() {
                                *vv = src[i00 + ch] * (1.0 - fx) * (1.0 - fy)
                                    + src[i10 + ch] * fx * (1.0 - fy)
                                    + src[i01 + ch] * (1.0 - fx) * fy
                                    + src[i11 + ch] * fx * fy;
                            }
                            for ch in 0..3 {
                                row[o + ch] =
                                    (m[ch][0] * v[0] + m[ch][1] * v[1] + m[ch][2] * v[2]) * decay;
                            }
                        }
                    }
                });
            }
        });
    }

    fn spectrum(&mut self, scope: &[f32]) {
        let n = scope.len();
        for b in 0..40 {
            let f = 55.0 * (12000.0f32 / 55.0).powf(b as f32 / 39.0);
            let coef = 2.0 * (TAU * f / RATE).cos();
            let (mut s1, mut s2) = (0f32, 0f32);
            for (i, &x) in scope.iter().enumerate() {
                let win = 0.5 - 0.5 * (TAU * i as f32 / n as f32).cos();
                let s0 = x * win + coef * s1 - s2;
                s2 = s1;
                s1 = s0;
            }
            let p = (s1 * s1 + s2 * s2 - coef * s1 * s2).max(1e-12);
            let db = 10.0 * (p / (n as f32 * n as f32 * 0.25)).log10() + (b as f32 * 0.25); // gentle tilt
            let v = ((db + 62.0) / 52.0).clamp(0.0, 1.0);
            let s = &mut self.spec[b];
            *s = if v > *s {
                *s + (v - *s) * 0.6
            } else {
                *s * 0.88 + v * 0.12
            };
        }
    }

    fn step(&mut self, t: f32, f: &Frame, prev: &Frame, taps: &[Tap]) {
        let (w, h) = (self.w as f32, self.h as f32);
        let (cx, cy) = (w * 0.5, h * 0.42);
        let u = h / 1080.0;
        let dt = 1.0 / self.fps;
        let by = |role: Role| -> Vec<usize> {
            (0..taps.len()).filter(|&i| taps[i].role == role).collect()
        };
        let mac: Vec<f32> = by(Role::Macro)
            .iter()
            .map(|&i| f.taps[i].last.clamp(0.0, 1.0))
            .collect();
        let m = |k: usize| mac.get(k).copied().unwrap_or(0.5);
        let (energy, motion, space, glow) = (m(0), m(1), m(2), m(3));
        let lfo = by(Role::Lfo).first().map_or(0.0, |&i| f.taps[i].last);
        let envs = by(Role::Env);
        let env: f32 =
            envs.iter().map(|&i| f.taps[i].last.max(0.0)).sum::<f32>() / envs.len().max(1) as f32;

        // Scene change on a new cue: next palette, a flash and a shockwave.
        if f.section != prev.section || (t == 0.0 && f.section > 0) {
            self.scene = (self.scene + 1) % SCENES.len();
            self.flash = 1.0;
            for k in 0..3 {
                self.rings.push(Ring {
                    x: cx,
                    y: cy,
                    born: t + k as f32 * 0.09,
                    col: [1.0, 0.95, 0.85],
                    a: 1.4,
                    max_r: w * 0.7,
                    life: 1.8,
                });
            }
        }
        let target = SCENES[self.scene];
        for k in 0..4 {
            self.pal[k] = mix(self.pal[k], target[k], 0.025);
        }

        // Clock: a beat punch and a thin shockwave.
        let mut beat = false;
        for i in by(Role::Clock) {
            if rose(f, prev, i) {
                beat = true;
            }
        }
        if beat {
            self.punch = 1.0;
            let c = self.pal[2];
            self.rings.push(Ring {
                x: cx,
                y: cy,
                born: t,
                col: c,
                a: 0.22,
                max_r: h * 0.55,
                life: 0.7,
            });
        }
        self.punch *= (-9.0 * dt).exp();
        self.flash *= (-2.5 * dt).exp();
        self.spin += dt * (0.12 + 0.5 * motion + 0.4 * lfo);

        self.feedback(
            1.0035 + 0.006 * energy + 0.012 * self.punch,
            0.0015 + 0.004 * lfo * motion,
            0.010 * (motion - 0.3) + 0.004 * (t * 0.21).sin(),
            0.88 + 0.085 * space,
            0.004 + 0.01 * motion,
        );
        self.crisp.fill(0.0);

        // Lattice: one row per active step lane, 16 cells, heat memory; the bottom band.
        let n = self.lanes.len();
        let row_h = (h * 0.22 / n.max(1) as f32).min(30.0 * u);
        let y_top = h * 0.855 - row_h * n as f32;
        let (x0, x1) = (w * 0.12, w * 0.88);
        let cw = (x1 - x0) / 16.0;
        let mut hits: Vec<(f32, f32, [f32; 3])> = Vec::new();
        for li in 0..n {
            let ti = self.lanes[li];
            let col = self.pal[li % 4];
            let y = y_top + (li as f32 + 0.5) * row_h;
            if rose(f, prev, ti) {
                let c = (self.count[li] % 16) as usize;
                self.count[li] += 1;
                self.heat[li][c] = 1.0;
                let x = x0 + (c as f32 + 0.5) * cw;
                let pitch = taps
                    .get(ti + 1)
                    .filter(|tp| tp.role == Role::Pitch)
                    .map_or(0.0, |_| f.taps[ti + 1].last);
                hits.push((x, y, col));
                self.rings.push(Ring {
                    x,
                    y,
                    born: t,
                    col,
                    a: 0.9,
                    max_r: 26.0 * u + 30.0 * u * env,
                    life: 0.6,
                });
                // A beam from the hit up into the core: the step "feeds" the sound.
                let ang = -std::f32::consts::FRAC_PI_2 + (x - cx) / w * 1.4 + pitch * 0.01;
                let (ex, ey) = (cx + ang.cos() * h * 0.2, cy + ang.sin() * h * 0.2);
                self.line(false, x, y - row_h * 0.4, ex, ey, col, 0.10 + 0.12 * energy);
            }
            let play = (self.count[li] + 15) % 16;
            for c in 0..16 {
                let hcell = &mut self.heat[li][c];
                *hcell *= (-dt / 4.0).exp();
                let hv = *hcell;
                let x = x0 + (c as f32 + 0.5) * cw;
                let base = if c % 4 == 0 { 0.022 } else { 0.010 };
                let a =
                    base + 0.30 * hv * hv + 0.12 * hv + if c == play as usize { 0.05 } else { 0.0 };
                self.rect(x, y, cw * 0.34, row_h * 0.26, col, a);
            }
        }
        // Coincidence: lanes that fire together are joined by a gold arc.
        if hits.len() >= 2 {
            for k in 0..hits.len() - 1 {
                let (ax, ay, _) = hits[k];
                let (bx, by2, _) = hits[k + 1];
                let lift = 60.0 * u + (bx - ax).abs() * 0.25;
                let gold = [1.0, 0.85, 0.5];
                let mut px = ax;
                let mut py = ay;
                for s in 1..=48 {
                    let tt = s as f32 / 48.0;
                    let mx = (ax + bx) * 0.5;
                    let my = (ay + by2) * 0.5 - lift;
                    let qx = (1.0 - tt).powi(2) * ax + 2.0 * (1.0 - tt) * tt * mx + tt * tt * bx;
                    let qy = (1.0 - tt).powi(2) * ay + 2.0 * (1.0 - tt) * tt * my + tt * tt * by2;
                    self.line(false, px, py, qx, qy, gold, 0.35);
                    px = qx;
                    py = qy;
                }
            }
        }
        // Attacks: particle bursts from the core, sized by energy.
        let attack = envs
            .iter()
            .any(|&i| f.fresh && f.taps[i].max > 0.75 && prev.taps[i].last < 0.45)
            || (!hits.is_empty() && envs.is_empty());
        if attack {
            let burst = (40.0 + 140.0 * energy) as usize;
            for _ in 0..burst {
                let a = self.rand() * TAU;
                let sp = (1.5 + 6.0 * self.rand()) * u * (0.6 + energy);
                let col = self.pal[(self.rand() * 4.0) as usize % 4];
                let life = 1.0 + self.rand() * 1.5;
                self.parts.push(Particle {
                    x: cx,
                    y: cy,
                    vx: a.cos() * sp,
                    vy: a.sin() * sp,
                    life,
                    col,
                });
            }
        }
        if self.parts.len() > 9000 {
            let extra = self.parts.len() - 9000;
            self.parts.drain(..extra);
        }
        let mut parts = std::mem::take(&mut self.parts);
        parts.retain_mut(|p| {
            p.x += p.vx;
            p.y += p.vy;
            p.vx *= 0.975;
            p.vy *= 0.975;
            // a slow curl around the core
            let (dx, dy) = (p.x - cx, p.y - cy);
            p.x += -dy * 0.004 * motion;
            p.y += dx * 0.004 * motion;
            p.life -= dt;
            p.life > 0.0
        });
        for p in &parts {
            self.dot(false, p.x, p.y, p.col, 0.16 * p.life.min(1.0), 1);
        }
        self.parts = parts;

        // Rings (hits, beats, scene changes).
        let rings = std::mem::take(&mut self.rings);
        let mut keep = Vec::with_capacity(rings.len());
        for rg in rings {
            let age = t - rg.born;
            if age > rg.life {
                continue;
            }
            if age >= 0.0 {
                let rad = 2.0 + rg.max_r * (1.0 - (-age * 4.0 / rg.life).exp());
                let a = rg.a * (1.0 - age / rg.life).powi(2) * 0.5;
                let pts = (rad * 0.9) as usize + 24;
                for p in 0..pts {
                    let th = p as f32 / pts as f32 * TAU;
                    self.dot(
                        false,
                        rg.x + rad * th.cos(),
                        rg.y + rad * th.sin(),
                        rg.col,
                        a,
                        1,
                    );
                }
            }
            keep.push(rg);
        }
        self.rings = keep;

        // Core: the output waveform as a closed polar line, twice, counter-rotating.
        let sc = &f.scope;
        let start = (1..sc.len() / 2)
            .find(|&j| sc[j - 1] < 0.0 && sc[j] >= 0.0)
            .unwrap_or(0);
        let span = 1024.min(sc.len() - start);
        let gain = 0.3 / f.rms.max(0.02);
        let base = h * (0.085 + 0.07 * env) * (1.0 + 0.08 * self.punch);
        let raw: Vec<f32> = (0..=480)
            .map(|p| sc[start + (p as f32 / 480.0 * (span - 1) as f32) as usize])
            .collect();
        let pts: Vec<f32> = (0..raw.len())
            .map(|i| {
                let a = raw[i.saturating_sub(2)..(i + 3).min(raw.len())].iter();
                let n = a.len() as f32;
                a.sum::<f32>() / n
            })
            .collect();
        for (k, sign) in [1.0f32, -1.0].iter().enumerate() {
            let col = mix(self.pal[k], [1.0, 1.0, 1.0], 0.25);
            let rot = self.spin * sign;
            let mut last = None;
            for (p, &v) in pts.iter().enumerate() {
                let th = p as f32 / 480.0 * TAU + rot;
                let rr = base * (1.0 + 0.55 * (v * gain).clamp(-1.0, 1.0));
                let (x, y) = (cx + rr * th.cos(), cy + rr * th.sin());
                if let Some((lx, ly)) = last {
                    self.line(true, lx, ly, x, y, col, 0.35 + 0.4 * env);
                    self.line(false, lx, ly, x, y, col, 0.05 + 0.08 * glow);
                }
                last = Some((x, y));
            }
        }
        // Spectrum ring around the core, mirrored left/right.
        self.spectrum(&f.scope);
        let r0 = base * 1.55 + 8.0 * u;
        for b in 0..40 {
            let v = self.spec[b];
            let col = mix(self.pal[0], self.pal[1], b as f32 / 39.0);
            for side in [-1.0f32, 1.0] {
                let th = -std::f32::consts::FRAC_PI_2
                    + side * (b as f32 + 0.5) / 40.0 * std::f32::consts::PI
                    + self.spin * 0.15;
                let (c1, s1) = (th.cos(), th.sin());
                let len = 3.0 * u + v * v * h * 0.13;
                self.line(
                    true,
                    cx + c1 * r0,
                    cy + s1 * r0,
                    cx + c1 * (r0 + len),
                    cy + s1 * (r0 + len),
                    col,
                    0.18 + 0.4 * v,
                );
                self.dot(
                    false,
                    cx + c1 * (r0 + len),
                    cy + s1 * (r0 + len),
                    col,
                    0.12 * v,
                    2,
                );
            }
        }
        // Lead: a comet orbiting the core while a played note is held; radius from the note.
        let _ = (by(Role::LeadGate), by(Role::LeadPitch));
        let on = f.lead.is_some();
        self.lead_lvl += ((on as u8 as f32) - self.lead_lvl) * if on { 0.25 } else { 0.05 };
        if let Some(n) = f.lead {
            self.lead_note = n as f32;
        }
        if self.lead_lvl > 0.02 {
            self.lead_ang += dt * (1.3 + 0.5 * energy);
            let rr = base * 2.3 + (self.lead_note - 64.0) * 9.0 * u;
            let (x, y) = (
                cx + rr * self.lead_ang.cos(),
                cy + rr * self.lead_ang.sin() * 0.62,
            );
            let col = mix(self.pal[3], [1.0, 1.0, 1.0], 0.35);
            self.dot(false, x, y, col, 0.55 * self.lead_lvl, 2);
            self.dot(true, x, y, [1.0, 1.0, 1.0], 0.8 * self.lead_lvl, 2);
            if f.lead_new {
                self.rings.push(Ring {
                    x,
                    y,
                    born: t,
                    col,
                    a: 1.1,
                    max_r: 70.0 * u,
                    life: 1.0,
                });
                self.line(false, x, y, cx, cy, col, 0.25);
            }
        }
    }

    /// Bloom, tonemap, vignette, grain, flash; writes 8-bit RGB.
    fn compose(&mut self, out: &mut [u8], frame: u32) {
        let (w, h) = (self.w, self.h);
        let (sw, sh) = (w / 4, h / 4);
        // Downsample acc + crisp (bright parts only) to quarter resolution.
        for y in 0..sh {
            for x in 0..sw {
                let mut v = [0f32; 3];
                for dy in 0..4 {
                    for dx in 0..4 {
                        let o = ((y * 4 + dy) * w + x * 4 + dx) * 3;
                        for (ch, vv) in v.iter_mut().enumerate() {
                            *vv += (self.acc[o + ch] + self.crisp[o + ch] - 0.25).max(0.0);
                        }
                    }
                }
                let o = (y * sw + x) * 3;
                for ch in 0..3 {
                    self.small[o + ch] = v[ch] / 16.0;
                }
            }
        }
        // Two separable box blurs, radius 5.
        for _ in 0..2 {
            for (src_is_small, horiz) in [(true, true), (false, false)] {
                let (src, dst) = if src_is_small {
                    (&self.small, &mut self.small2)
                } else {
                    (&self.small2, &mut self.small)
                };
                for y in 0..sh {
                    for x in 0..sw {
                        let mut v = [0f32; 3];
                        let mut n = 0.0;
                        for d in -5isize..=5 {
                            let (xx, yy) = if horiz {
                                (x as isize + d, y as isize)
                            } else {
                                (x as isize, y as isize + d)
                            };
                            if xx < 0 || yy < 0 || xx >= sw as isize || yy >= sh as isize {
                                continue;
                            }
                            let o = (yy as usize * sw + xx as usize) * 3;
                            v[0] += src[o];
                            v[1] += src[o + 1];
                            v[2] += src[o + 2];
                            n += 1.0;
                        }
                        let o = (y * sw + x) * 3;
                        for ch in 0..3 {
                            dst[o + ch] = v[ch] / n;
                        }
                    }
                }
            }
        }
        let (acc, crisp, small) = (&self.acc, &self.crisp, &self.small);
        let flash = self.flash * 0.05;
        let threads = std::thread::available_parallelism().map_or(8, |n| n.get());
        let rows = h.div_ceil(threads);
        std::thread::scope(|sc| {
            for (ci, chunk) in out.chunks_mut(rows * w * 3).enumerate() {
                sc.spawn(move || {
                    for (ry, row) in chunk.chunks_mut(w * 3).enumerate() {
                        let y = ci * rows + ry;
                        let fy = (y as f32 / 4.0 - 0.5).clamp(0.0, (sh - 1) as f32);
                        let (y0, ty) = (fy as usize, fy.fract());
                        let y1 = (y0 + 1).min(sh - 1);
                        let ny = y as f32 / h as f32 - 0.5;
                        for x in 0..w {
                            let fx = (x as f32 / 4.0 - 0.5).clamp(0.0, (sw - 1) as f32);
                            let (x0, tx) = (fx as usize, fx.fract());
                            let x1 = (x0 + 1).min(sw - 1);
                            let nx = x as f32 / w as f32 - 0.5;
                            let vig = 1.0 - 0.55 * (nx * nx * 1.2 + ny * ny * 1.6);
                            let mut hsh = (x as u32).wrapping_mul(0x9E37_79B1)
                                ^ (y as u32).wrapping_mul(0x85EB_CA77)
                                ^ frame.wrapping_mul(0xC2B2_AE3D);
                            hsh ^= hsh >> 15;
                            hsh = hsh.wrapping_mul(0x2C1B_3C6D);
                            hsh ^= hsh >> 12;
                            let grain = ((hsh & 0xFFFF) as f32 / 65535.0 - 0.5) * 0.018;
                            let o = (y * w + x) * 3;
                            for ch in 0..3 {
                                let b = small[(y0 * sw + x0) * 3 + ch] * (1.0 - tx) * (1.0 - ty)
                                    + small[(y0 * sw + x1) * 3 + ch] * tx * (1.0 - ty)
                                    + small[(y1 * sw + x0) * 3 + ch] * (1.0 - tx) * ty
                                    + small[(y1 * sw + x1) * 3 + ch] * tx * ty;
                                let bg = [0.010, 0.011, 0.018][ch] * (1.0 - ny);
                                let v = (acc[o + ch] + crisp[o + ch] + 0.9 * b + flash + bg)
                                    * 1.15
                                    * vig;
                                let a = (v * (2.51 * v + 0.03)) / (v * (2.43 * v + 0.59) + 0.14);
                                let g = a.clamp(0.0, 1.0).powf(1.0 / 2.2) + grain;
                                row[x * 3 + ch] = (g * 255.0).clamp(0.0, 255.0) as u8;
                            }
                        }
                    }
                });
            }
        });
    }
}
