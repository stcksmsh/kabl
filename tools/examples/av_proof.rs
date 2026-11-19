//! AV proof of concept (docs/av-audit/KABL_AV_ARCHITECTURE_AND_COMPETITION_PLAN.md §5, Track A):
//! renders a self-playing patch offline and draws a video that follows the patch's *structure*
//! (sequencer steps, envelope levels, oscillator cycles, clock ticks, macro moves), read through
//! the engine's face taps, not an FFT of the mix. Nothing is played aloud.
//!
//!     cargo run --release -p kabl-tools --example av_proof -- PATCH OUT.mp4 [--secs 45] [--fps 30]
//!         [--size 1280x720] [--no-macros]
//!
//! Pass 1 renders audio through the production `Timeline` and records one tap report per video
//! frame; macro knobs (`macro` module m1..m4) sweep slowly through the runtime value path the UI
//! uses. Pass 2 draws each frame on the CPU and pipes raw RGB plus the WAV into ffmpeg.
//! Throwaway proof: no product code depends on it.

use basedrop::Collector;
use kabl_core::ModuleId;
use kabl_engine::facetap::{FaceTarget, FaceTargets, MAX_FACE_TAPS};
use kabl_engine::patch_engine::PatchEngine;
use kabl_engine::runtime::{Feedback, ParamSet, RuntimeTarget, ToAudio};
use kabl_engine::timeline::{Timeline, LATENCY};
use kabl_modules::PortDirection;
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Clone, Copy, PartialEq)]
enum Role {
    Step,  // seq gate
    Pitch, // seq pitch, paired with the previous Step
    Env,   // env.adsr out
    Osc,   // osc.* first output, with its cycle
    Clock, // clock gate
    Lfo,   // lfo out
    Macro, // macro output
}

#[derive(Clone, Copy)]
struct Tap {
    role: Role,
    lane: usize,
}

#[derive(Clone)]
struct Sample {
    last: f32,
    min: f32,
    max: f32,
    cycle: Option<[f32; 128]>,
}

struct Frame {
    taps: Vec<Sample>,
    rms: f32,
    /// The last `SCOPE` mono output samples at this frame.
    scope: Vec<f32>,
}

const SCOPE: usize = 2048;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (patch, out) = (&args[0], &args[1]);
    let opt = |name: &str| args.iter().position(|a| a == name).map(|i| args[i + 1].clone());
    let secs: f32 = opt("--secs").map_or(45.0, |s| s.parse().unwrap());
    let fps: u32 = opt("--fps").map_or(30, |s| s.parse().unwrap());
    let (w, h) = opt("--size").map_or((1280usize, 720usize), |s| {
        let (a, b) = s.split_once('x').unwrap();
        (a.parse().unwrap(), b.parse().unwrap())
    });
    let macros = !args.iter().any(|a| a == "--no-macros");
    let rate = 48000.0f32;

    // ---- choose taps from the patch -------------------------------------------------------
    let log = kabl_core::load(std::path::Path::new(patch)).expect("patch");
    let state = log.state().clone();
    let mut targets: Vec<FaceTarget> = Vec::new();
    let mut taps: Vec<Tap> = Vec::new();
    let mut lanes = [0usize; 7];
    let mut macro_id: Option<(ModuleId, &'static str)> = None;
    let mut add = |t: FaceTarget, role: Role, targets: &mut Vec<FaceTarget>, taps: &mut Vec<Tap>| {
        if targets.len() < MAX_FACE_TAPS {
            let lane = lanes[role as usize];
            lanes[role as usize] += 1;
            targets.push(t);
            taps.push(Tap { role, lane });
        }
    };
    // Order: the musically important roles first, so the 16-tap cap drops the least useful.
    for pass in 0..3 {
        for (&id, m) in &state.modules {
            let Some(info) = kabl_modules::registry::info_for(&m.kind) else { continue };
            let outs: Vec<&str> = info
                .ports
                .iter()
                .filter(|p| p.direction == PortDirection::Output)
                .map(|p| p.name)
                .collect();
            let port = |n: &str| outs.iter().position(|o| *o == n).map(|i| i as u8);
            let tg = |port: u8, view: bool| FaceTarget { module: id, port, kind: info.kind, view };
            match (pass, info.kind) {
                (0, "seq") => {
                    if let (Some(g), Some(p)) = (port("gate"), port("pitch")) {
                        add(tg(g, false), Role::Step, &mut targets, &mut taps);
                        add(tg(p, false), Role::Pitch, &mut targets, &mut taps);
                    }
                }
                (0, "clock") => {
                    if let Some(g) = port("gate") {
                        add(tg(g, false), Role::Clock, &mut targets, &mut taps);
                    }
                }
                (1, "env.adsr") => add(tg(0, false), Role::Env, &mut targets, &mut taps),
                (1, k) if k.starts_with("osc.") => add(tg(0, true), Role::Osc, &mut targets, &mut taps),
                (2, "lfo") => add(tg(0, false), Role::Lfo, &mut targets, &mut taps),
                (2, "macro") => {
                    macro_id = Some((id, info.kind));
                    add(tg(0, false), Role::Macro, &mut targets, &mut taps);
                }
                _ => {}
            }
        }
    }
    eprintln!(
        "taps: {} (steps {}, envs {}, oscs {}, clocks {})",
        taps.len(),
        lanes[Role::Step as usize],
        lanes[Role::Env as usize],
        lanes[Role::Osc as usize],
        lanes[Role::Clock as usize]
    );

    // ---- pass 1: audio + telemetry --------------------------------------------------------
    let collector = Collector::new();
    let mut engine = PatchEngine::new(&collector.handle(), &state, rate, 8).expect("compiles");
    engine.face_taps(Some(FaceTargets::new(1, &targets)));
    let (mut tx, mut rx) = rtrb::RingBuffer::<ToAudio>::new(64);
    let fb = Feedback::default();
    let macro_base: Vec<f32> = (0..4)
        .map(|k| {
            let name = format!("m{}", k + 1);
            macro_id
                .and_then(|(id, _)| state.modules[&id].params.get(&name).copied())
                .unwrap_or(0.5)
        })
        .collect();

    let mut timeline = Box::new(Timeline::new());
    let per_frame = (rate as u32 / fps) as usize;
    let n_frames = (secs * fps as f32) as usize;
    let total = n_frames * per_frame + LATENCY;
    let (mut l, mut r) = (vec![0f32; per_frame], vec![0f32; per_frame]);
    let mut audio: Vec<f32> = Vec::with_capacity(total * 2);
    let mut frames: Vec<Frame> = Vec::with_capacity(n_frames + 2);
    let mut last_taps: Vec<Sample> =
        taps.iter().map(|_| Sample { last: 0.0, min: 0.0, max: 0.0, cycle: None }).collect();
    let mut done = 0usize;
    while done < total {
        let n = per_frame.min(total - done);
        let t = done as f32 / rate;
        if let (true, Some((id, kind))) = (macros, macro_id) {
            // Slow, out-of-phase sweeps around each knob's stored value; a 6 s intro holds still.
            let ramp = ((t - 6.0) / 10.0).clamp(0.0, 1.0);
            for k in 0..4 {
                let period = [23.0, 31.0, 17.0, 41.0][k];
                let v = macro_base[k] + ramp * 0.35 * (std::f32::consts::TAU * t / period + k as f32).sin();
                let _ = tx.push(ToAudio::Set(ParamSet {
                    rev: 1 << 40,
                    target: RuntimeTarget::Param { id, kind, index: k as u16 },
                    value: v.clamp(0.0, 1.0),
                }));
            }
        }
        engine.drain(&mut rx, 64, &fb);
        timeline.process(&mut engine, &mut l[..n], &mut r[..n], &[], |_, _| {});
        let mut sq = 0f32;
        for i in 0..n {
            audio.push(l[i]);
            audio.push(r[i]);
            sq += l[i] * l[i] + r[i] * r[i];
        }
        if let Some(rep) = engine.take_face_report() {
            for (s, e) in rep.entries.iter().take(rep.n as usize).enumerate() {
                last_taps[s] = Sample {
                    last: e.last,
                    min: e.min,
                    max: e.max,
                    cycle: e.view_valid.then(|| e.cycle_f32()),
                };
            }
        }
        let tail = audio.len().saturating_sub(SCOPE * 2);
        let scope = audio[tail..].chunks(2).map(|c| 0.5 * (c[0] + c[1])).collect();
        frames.push(Frame { taps: last_taps.clone(), rms: (sq / (2 * n) as f32).sqrt(), scope });
        done += n;
    }
    for (s, tp) in taps.iter().enumerate() {
        let (mut lo, mut hi, mut rises, mut cyc) = (f32::MAX, f32::MIN, 0, 0);
        for (i, f) in frames.iter().enumerate() {
            let x = &f.taps[s];
            lo = lo.min(x.min);
            hi = hi.max(x.max);
            let was = if i > 0 { frames[i - 1].taps[s].last } else { 0.0 };
            rises += (x.max >= 0.5 && (was < 0.5 || x.min < 0.5)) as u32;
            cyc += x.cycle.is_some() as u32;
        }
        eprintln!("tap {s} {:?}#{}: range {lo:.2}..{hi:.2} rises {rises} cycles {cyc}", tp.role as u8, tp.lane);
    }
    // Drop the adapter's fixed latency so audio sample 0 is performance time 0; video frames
    // were taken at block ends, so shift them by the same amount (one 64-sample block < 1 frame).
    let audio = &audio[LATENCY * 2..];
    let wav = format!("{out}.wav");
    {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: rate as u32,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut wr = hound::WavWriter::create(&wav, spec).unwrap();
        let peak = audio.iter().fold(0f32, |a, s| a.max(s.abs())).max(1e-6);
        let gain = if peak > 0.98 { 0.98 / peak } else { 1.0 };
        for s in audio {
            wr.write_sample(s * gain).unwrap();
        }
        wr.finalize().unwrap();
        eprintln!("audio: {:.1} s, peak {:.3}", audio.len() as f32 / 2.0 / rate, peak);
    }

    // ---- pass 2: frames -> ffmpeg ---------------------------------------------------------
    let mut ff = Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgb24"])
        .args(["-s", &format!("{w}x{h}"), "-r", &fps.to_string(), "-i", "-", "-i", &wav])
        .args(["-c:v", "libx264", "-preset", "medium", "-crf", "18", "-pix_fmt", "yuv420p"])
        .args(["-c:a", "aac", "-b:a", "256k", "-shortest", out])
        .stdin(Stdio::piped())
        .spawn()
        .expect("ffmpeg");
    let stdin = ff.stdin.as_mut().unwrap();
    let mut vis = Visual::new(w, h);
    let mut rgb = vec![0u8; w * h * 3];
    for (i, f) in frames.iter().enumerate().take(n_frames) {
        let prev = if i > 0 { Some(&frames[i - 1]) } else { None };
        vis.step(i as f32 / fps as f32, f, prev, &taps, fps as f32);
        vis.tonemap(&mut rgb);
        stdin.write_all(&rgb).unwrap();
        if i % (fps as usize * 5) == 0 {
            eprintln!("frame {i}/{n_frames}");
        }
    }
    drop(ff.stdin.take());
    let st = ff.wait().unwrap();
    assert!(st.success(), "ffmpeg failed");
    eprintln!("wrote {out}");
}

// ---- the look ----------------------------------------------------------------------------

const PALETTE: [[f32; 3]; 6] = [
    [1.00, 0.55, 0.15], // amber
    [0.20, 0.75, 1.00], // cyan
    [1.00, 0.25, 0.60], // magenta
    [0.55, 1.00, 0.35], // lime
    [0.70, 0.45, 1.00], // violet
    [1.00, 0.90, 0.55], // pale gold
];

struct Ring {
    x: f32,
    y: f32,
    born: f32,
    col: [f32; 3],
    energy: f32,
}

struct Visual {
    w: usize,
    h: usize,
    acc: Vec<f32>,
    tmp: Vec<f32>,
    rings: Vec<Ring>,
    spin: f32,
    flash: f32,
    beats: u32,
    counts: [u32; 16],
}

impl Visual {
    fn new(w: usize, h: usize) -> Self {
        Visual { w, h, acc: vec![0.0; w * h * 3], tmp: vec![0.0; w * h * 3], rings: Vec::new(), spin: 0.0, flash: 0.0, beats: 0, counts: [0; 16] }
    }

    fn splat(&mut self, x: f32, y: f32, c: [f32; 3], a: f32) {
        let (xi, yi) = (x.round() as isize, y.round() as isize);
        for dy in -2isize..=2 {
            for dx in -2isize..=2 {
                let (px, py) = (xi + dx, yi + dy);
                if px < 0 || py < 0 || px >= self.w as isize || py >= self.h as isize {
                    continue;
                }
                let fx = px as f32 - x;
                let fy = py as f32 - y;
                let k = a * (-(fx * fx + fy * fy) * 0.9).exp();
                let o = (py as usize * self.w + px as usize) * 3;
                for ch in 0..3 {
                    self.acc[o + ch] += c[ch] * k;
                }
            }
        }
    }

    /// Feedback: the last frame, zoomed toward the viewer and turned a little, decays under the new one.
    fn feedback(&mut self, zoom: f32, turn: f32, decay: f32) {
        let (w, h) = (self.w as f32, self.h as f32);
        let (cx, cy) = (w * 0.5, h * 0.5);
        let (s, c) = turn.sin_cos();
        std::mem::swap(&mut self.acc, &mut self.tmp);
        for y in 0..self.h {
            for x in 0..self.w {
                let dx = (x as f32 - cx) / zoom;
                let dy = (y as f32 - cy) / zoom;
                let sx = cx + dx * c - dy * s;
                let sy = cy + dx * s + dy * c;
                let o = (y * self.w + x) * 3;
                if sx < 0.0 || sy < 0.0 || sx >= w - 1.0 || sy >= h - 1.0 {
                    self.acc[o..o + 3].fill(0.0);
                    continue;
                }
                let (x0, y0) = (sx as usize, sy as usize);
                let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
                let i00 = (y0 * self.w + x0) * 3;
                let i10 = i00 + 3;
                let i01 = i00 + self.w * 3;
                let i11 = i01 + 3;
                for ch in 0..3 {
                    let v = self.tmp[i00 + ch] * (1.0 - fx) * (1.0 - fy)
                        + self.tmp[i10 + ch] * fx * (1.0 - fy)
                        + self.tmp[i01 + ch] * (1.0 - fx) * fy
                        + self.tmp[i11 + ch] * fx * fy;
                    self.acc[o + ch] = v * decay;
                }
            }
        }
    }

    fn step(&mut self, t: f32, f: &Frame, prev: Option<&Frame>, taps: &[Tap], fps: f32) {
        let (w, h) = (self.w as f32, self.h as f32);
        let get = |role: Role| -> Vec<(usize, usize)> {
            taps.iter().enumerate().filter(|(_, tp)| tp.role == role).map(|(i, tp)| (i, tp.lane)).collect()
        };
        let rose = |i: usize| -> bool {
            let s = &f.taps[i];
            let was = prev.map_or(0.0, |p| p.taps[i].last);
            s.max >= 0.5 && (was < 0.5 || s.min < 0.5)
        };
        let macro_v = get(Role::Macro).first().map_or(0.5, |&(i, _)| f.taps[i].last.clamp(0.0, 1.0));
        let lfo = get(Role::Lfo).first().map_or(0.0, |&(i, _)| f.taps[i].last);
        let env_sum: f32 = get(Role::Env).iter().map(|&(i, _)| f.taps[i].last.max(0.0)).sum();

        // Clock: a beat flash and a slow turn that advances one notch per tick.
        for (i, _) in get(Role::Clock) {
            if rose(i) {
                self.flash = 1.0;
                self.beats += 1;
            }
        }
        self.flash *= (-6.0 / fps).exp();
        self.spin += 0.002 + 0.004 * lfo + 0.01 * self.flash;

        let bloom = 0.3 + 0.7 * macro_v;
        self.feedback(1.006 + 0.012 * bloom + 0.01 * self.flash, 0.003 + 0.004 * lfo, 0.86 + 0.1 * bloom);

        // Sequencer steps light a 16-column lattice, one row per lane: lanes of different
        // lengths drift against each other, so the interlock is visible.
        let steps = get(Role::Step);
        let n_lanes = steps.len().max(1) as f32;
        for (k, &(i, lane)) in steps.iter().enumerate() {
            if rose(i) {
                let col_i = self.counts[k % 16] % 16;
                self.counts[k % 16] += 1;
                let pitch = taps.get(i + 1).filter(|tp| tp.role == Role::Pitch).map_or(0.0, |_| f.taps[i + 1].last);
                let x = w * (0.08 + 0.84 * col_i as f32 / 15.0);
                let y = h * ((k as f32 + 1.0) / (n_lanes + 1.0)) - pitch * 4.0;
                self.rings.push(Ring { x, y, born: t, col: PALETTE[lane % 6], energy: 0.7 + 1.2 * f.rms.min(0.5) });
            }
        }
        let rings = std::mem::take(&mut self.rings);
        let mut keep = Vec::with_capacity(rings.len());
        for rg in rings {
            let age = t - rg.born;
            if age > 1.6 {
                continue;
            }
            let rad = 3.0 + 70.0 * (1.0 - (-age * 3.0).exp());
            let a = rg.energy * (1.0 - age / 1.6).powi(2) * 0.55;
            let pts = (rad * 1.2) as usize + 24;
            for p in 0..pts {
                let th = p as f32 / pts as f32 * std::f32::consts::TAU;
                self.splat(rg.x + rad * th.cos(), rg.y + rad * th.sin() * 0.85, rg.col, a);
            }
            if age < 0.08 {
                self.splat(rg.x, rg.y, [1.0, 1.0, 1.0], 3.0);
            }
            keep.push(rg);
        }
        self.rings = keep;

        // The output waveform as a polar scope, starting at a rising zero crossing so it
        // stands still on a steady tone; radius breathes with the envelopes.
        let sc = &f.scope;
        let start = (1..sc.len() / 2).find(|&j| sc[j - 1] < 0.0 && sc[j] >= 0.0).unwrap_or(0);
        let span = 1024.min(sc.len() - start);
        let base = h * (0.12 + 0.18 * env_sum.min(1.5) / 1.5);
        let gain = 1.0 / f.rms.max(0.02);
        for (k, sign) in [1.0f32, -1.0].iter().enumerate() {
            let col = PALETTE[k * 3 % 6];
            let rot = self.spin * sign;
            let n = 900;
            for p in 0..n {
                let u = p as f32 / n as f32;
                let v = (sc[start + (u * (span - 1) as f32) as usize] * gain * 0.25).clamp(-1.0, 1.0);
                let th = u * std::f32::consts::TAU + rot;
                let rr = base * (1.0 + 0.5 * v);
                self.splat(w * 0.5 + rr * th.cos(), h * 0.5 + rr * th.sin(), col, 0.06 + 0.2 * env_sum.min(1.0));
            }
        }
        let _ = get(Role::Osc);

        // Beat spokes.
        if self.flash > 0.05 {
            let spokes = 12;
            for s in 0..spokes {
                let th = s as f32 / spokes as f32 * std::f32::consts::TAU + self.spin * 2.0;
                for p in 0..60 {
                    let rr = h * 0.08 + p as f32 * 7.0;
                    self.splat(w * 0.5 + rr * th.cos(), h * 0.5 + rr * th.sin(), PALETTE[5], 0.05 * self.flash);
                }
            }
        }
    }

    fn tonemap(&self, out: &mut [u8]) {
        for (o, a) in out.iter_mut().zip(self.acc.iter()) {
            let v = 1.0 - (-a * 1.6).exp();
            *o = (v.powf(0.85) * 255.0).clamp(0.0, 255.0) as u8;
        }
    }
}
