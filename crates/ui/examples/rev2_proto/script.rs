//! Scripted interaction driver for verification runs. It injects real pointer and key events
//! into egui's raw input (so hit-testing, gestures and widgets run exactly as for a person),
//! checks the resulting prototype state, and saves real framebuffer screenshots.
//!
//! One command per line; `#` starts a comment. Targets: `@knob:adsr.attack_ms`, `@ring:…`,
//! `@pill:…`, `@handle:…`, `@plug:…`, `@pin:…`, `@seg:lfo.waveform.2`, `@jack:lfo.out`,
//! `@toggle:lfo`, `@done:lfo`, `@header:lfo`, `@ui:<button key>`, `@empty`, or `x,y`.
//! Any target may end in `+dx,dy`.

use crate::geom::{self, Hit};
use crate::model::{self, CtlRef, JackRef, Patch, Route};
use crate::App;
use egui::{pos2, vec2, Event, Key, Modifiers, PointerButton, Pos2};
use std::collections::VecDeque;
use std::path::PathBuf;

pub struct Script {
    lines: VecDeque<(usize, String)>,
    queue: VecDeque<Vec<Event>>,
    wait: u32,
    shot_dir: PathBuf,
    pending_shot: Option<String>,
    mods: Modifiers,
    pos: Pos2,
    log: Vec<String>,
    fails: usize,
    mark: Option<(Patch, Option<usize>, Option<CtlRef>)>,
    finished: bool,
}

pub fn route_key(p: &Patch, r: &Route) -> String {
    format!(
        "{}.{}>{}.{}",
        mod_key(p, r.src.m),
        p.jack_def(r.src).id,
        mod_key(p, r.dst.m),
        p.ctl_def(r.dst).id
    )
}

fn mod_key(p: &Patch, m: usize) -> String {
    p.module_label(m).to_lowercase().replace(' ', "")
}

fn find_mod(p: &Patch, name: &str) -> Option<usize> {
    (0..p.modules.len()).find(|&m| mod_key(p, m) == name)
}

fn find_ctl(p: &Patch, s: &str) -> Option<CtlRef> {
    let (m, c) = s.split_once('.')?;
    let m = find_mod(p, m)?;
    let c = p.modules[m].def().controls.iter().position(|d| d.id == c)?;
    Some(CtlRef { m, c })
}

fn find_jack(p: &Patch, s: &str) -> Option<JackRef> {
    let (m, j) = s.split_once('.')?;
    let m = find_mod(p, m)?;
    let j = p.modules[m].def().jacks.iter().position(|d| d.id == j)?;
    Some(JackRef { m, j })
}

fn find_route(p: &Patch, src: &str, dst: &str) -> Option<usize> {
    let (s, d) = (find_jack(p, src)?, find_ctl(p, dst)?);
    p.routes.iter().position(|r| r.src == s && r.dst == d)
}

impl Script {
    pub fn load(path: &str, shot_dir: String) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let shot_dir = PathBuf::from(shot_dir);
        std::fs::create_dir_all(&shot_dir)?;
        let lines = text
            .lines()
            .enumerate()
            .map(|(i, l)| (i + 1, l.split('#').next().unwrap_or("").trim().to_string()))
            .filter(|(_, l)| !l.is_empty())
            .collect();
        Ok(Script {
            lines,
            queue: VecDeque::new(),
            wait: 20,
            shot_dir,
            pending_shot: None,
            mods: Modifiers::NONE,
            pos: pos2(5.0, 5.0),
            log: vec![],
            fails: 0,
            mark: None,
            finished: false,
        })
    }

    fn ok(&mut self, line: usize, msg: String) {
        self.log.push(format!("PASS  l{line:<3} {msg}"));
    }

    fn fail(&mut self, line: usize, msg: String) {
        self.fails += 1;
        self.log.push(format!("FAIL  l{line:<3} {msg}"));
    }

    pub fn frame(&mut self, app: &mut App, ctx: &egui::Context, raw: &mut egui::RawInput) {
        if self.finished {
            return;
        }
        for e in &raw.events {
            if let Event::Screenshot { image, .. } = e {
                if let Some(name) = self.pending_shot.take() {
                    let path = self.shot_dir.join(format!("{name}.png"));
                    let [w, h] = image.size;
                    let buf: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                    match image::save_buffer(&path, &buf, w as u32, h as u32, image::ExtendedColorType::Rgba8) {
                        Ok(()) => self.log.push(format!("SHOT  {} ({w}×{h})", path.display())),
                        Err(e) => {
                            self.fails += 1;
                            self.log.push(format!("FAIL  screenshot {name}: {e}"));
                        }
                    }
                    self.wait = 2;
                }
            }
        }
        raw.modifiers = self.mods;
        if self.pending_shot.is_some() {
            return;
        }
        if self.wait > 0 {
            self.wait -= 1;
            return;
        }
        if let Some(evs) = self.queue.pop_front() {
            for e in &evs {
                if let Event::PointerMoved(p) = e {
                    self.pos = *p;
                }
            }
            raw.events.extend(evs);
            return;
        }
        while let Some((n, line)) = self.lines.pop_front() {
            if self.exec(app, ctx, n, &line) {
                return;
            }
        }
        self.finished = true;
        let summary = format!("{} checks failed", self.fails);
        self.log.push(summary);
        let out = self.log.join("\n");
        println!("{out}");
        let _ = std::fs::write(self.shot_dir.join("results.txt"), &out);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn target(&self, app: &App, t: &str) -> Option<Pos2> {
        let (t, off) = match t.split_once('+') {
            Some((a, b)) => {
                let (x, y) = b.split_once(',')?;
                (a, vec2(x.parse().ok()?, y.parse().ok()?))
            }
            None => (t, vec2(0.0, 0.0)),
        };
        let p = &app.patch;
        let xf = app.xf();
        let world = |want: &dyn Fn(&Hit) -> bool| {
            app.regions.iter().find(|r| want(&r.hit)).map(|r| xf.p(r.area.center()))
        };
        let pos = if let Some(k) = t.strip_prefix("@ui:") {
            app.ui_rects.get(k).map(|r| r.center())
        } else if t == "@empty" {
            // A canvas point with nothing under it (bare rack), scanning from the bottom right.
            let c = app.canvas;
            (0..400).find_map(|i| {
                let s = pos2(c.right() - 20.0 - (i % 20) as f32 * 30.0, c.bottom() - 20.0 - (i / 20) as f32 * 30.0);
                geom::hit_test(&app.regions, xf.inv(s)).is_none().then_some(s)
            })
        } else if let Some((kind, name)) = t.strip_prefix('@').and_then(|s| s.split_once(':')) {
            match kind {
                "knob" | "ring" | "pill" | "pin" => {
                    let c = find_ctl(p, name)?;
                    world(&|h| match (kind, h) {
                        ("knob", Hit::Knob(x)) | ("ring", Hit::Ring(x)) | ("pill", Hit::Pill(x)) | ("pin", Hit::Pin(x)) => *x == c,
                        _ => false,
                    })
                }
                "handle" | "plug" => {
                    let c = find_ctl(p, name)?;
                    world(&|h| match h {
                        Hit::Handle(r) if kind == "handle" => p.routes[*r].dst == c,
                        Hit::Plug(r) if kind == "plug" => p.routes[*r].dst == c,
                        _ => false,
                    })
                }
                "seg" => {
                    let (c, i) = name.rsplit_once('.')?;
                    let (c, i) = (find_ctl(p, c)?, i.parse::<usize>().ok()?);
                    world(&|h| *h == Hit::Seg(c, i))
                }
                "jack" => {
                    let j = find_jack(p, name)?;
                    world(&|h| *h == Hit::Jack(j))
                }
                "toggle" | "done" | "header" => {
                    let m = find_mod(p, name)?;
                    world(&|h| match (kind, h) {
                        ("toggle", Hit::Toggle(x)) | ("done", Hit::Done(x)) | ("header", Hit::Header(x)) => *x == m,
                        _ => false,
                    })
                }
                _ => None,
            }
        } else {
            let (x, y) = t.split_once(',')?;
            Some(pos2(x.parse().ok()?, y.parse().ok()?))
        };
        pos.map(|p| p + off)
    }

    fn button(&self, pos: Pos2, button: PointerButton, pressed: bool) -> Event {
        Event::PointerButton { pos, button, pressed, modifiers: self.mods }
    }

    fn set_mods(&mut self, words: &[&str]) {
        self.mods = Modifiers {
            shift: words.contains(&"shift"),
            alt: words.contains(&"alt"),
            ctrl: words.contains(&"ctrl"),
            command: words.contains(&"ctrl"),
            mac_cmd: false,
        };
    }

    /// Returns true when the command consumed this frame.
    fn exec(&mut self, app: &mut App, ctx: &egui::Context, n: usize, line: &str) -> bool {
        let w: Vec<&str> = line.split_whitespace().collect();
        let tgt = |s: &Self, i: usize| w.get(i).and_then(|t| s.target(app, t));
        macro_rules! need {
            ($e:expr, $what:expr) => {
                match $e {
                    Some(v) => v,
                    None => {
                        self.fail(n, format!("cannot resolve {}: {line}", $what));
                        return false;
                    }
                }
            };
        }
        match w[0] {
            "wait" => {
                self.wait = w.get(1).and_then(|s| s.parse().ok()).unwrap_or(5);
                true
            }
            "log" => {
                self.log.push(format!("----  {}", &line[4..]));
                false
            }
            "shot" => {
                self.pending_shot = Some(w[1].to_string());
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                true
            }
            "move" | "hover" => {
                let p = need!(tgt(self, 1), w[1]);
                self.queue.push_back(vec![Event::PointerMoved(p)]);
                self.wait = 0;
                self.queue.push_back(vec![]);
                self.queue.push_back(vec![]);
                true
            }
            "click" | "rclick" | "dblclick" => {
                let p = need!(tgt(self, 1), w[1]);
                self.set_mods(&w[2..]);
                let b = if w[0] == "rclick" { PointerButton::Secondary } else { PointerButton::Primary };
                self.queue.push_back(vec![Event::PointerMoved(p)]);
                let times = if w[0] == "dblclick" { 2 } else { 1 };
                for _ in 0..times {
                    self.queue.push_back(vec![self.button(p, b, true)]);
                    self.queue.push_back(vec![self.button(p, b, false)]);
                }
                self.queue.push_back(vec![]);
                self.queue.push_back(vec![]);
                true
            }
            "press" => {
                let p = need!(tgt(self, 1), w[1]);
                self.set_mods(&w[2..]);
                self.queue.push_back(vec![Event::PointerMoved(p)]);
                self.queue.push_back(vec![self.button(p, PointerButton::Primary, true)]);
                true
            }
            "moveby" => {
                let d = vec2(w[1].parse().unwrap_or(0.0), w[2].parse().unwrap_or(0.0));
                self.set_mods(&w[3..]);
                let start = self.pos;
                for i in 1..=8 {
                    self.queue.push_back(vec![Event::PointerMoved(start + d * i as f32 / 8.0)]);
                }
                self.queue.push_back(vec![]);
                true
            }
            "moveto" => {
                let b = need!(tgt(self, 1), w[1]);
                let a = self.pos;
                for i in 1..=8 {
                    self.queue.push_back(vec![Event::PointerMoved(a + (b - a) * i as f32 / 8.0)]);
                }
                self.queue.push_back(vec![]);
                true
            }
            "release" => {
                self.set_mods(&w[1..]);
                let p = self.pos;
                self.queue.push_back(vec![self.button(p, PointerButton::Primary, false)]);
                self.queue.push_back(vec![]);
                self.queue.push_back(vec![]);
                self.set_mods(&[]);
                true
            }
            "drag" | "dragby" => {
                let a = need!(tgt(self, 1), w[1]);
                let (b, rest) = if w[0] == "drag" {
                    (need!(tgt(self, 2), w[2]), 3)
                } else {
                    (a + vec2(w[2].parse().unwrap_or(0.0), w[3].parse().unwrap_or(0.0)), 4)
                };
                self.set_mods(&w[rest..]);
                self.queue.push_back(vec![Event::PointerMoved(a)]);
                self.queue.push_back(vec![self.button(a, PointerButton::Primary, true)]);
                for i in 1..=10 {
                    self.queue.push_back(vec![Event::PointerMoved(a + (b - a) * i as f32 / 10.0)]);
                }
                self.queue.push_back(vec![]);
                self.queue.push_back(vec![self.button(b, PointerButton::Primary, false)]);
                self.queue.push_back(vec![]);
                self.queue.push_back(vec![]);
                true
            }
            "key" => {
                let key = need!(Key::from_name(w[1]), w[1]);
                self.set_mods(&w[2..]);
                let m = self.mods;
                self.queue.push_back(vec![Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: m }]);
                self.queue.push_back(vec![Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: m }]);
                self.queue.push_back(vec![]);
                self.queue.push_back(vec![]);
                true
            }
            "mods" => {
                self.set_mods(&w[1..]);
                false
            }
            "type" => {
                let text = line[5..].to_string();
                self.queue.push_back(vec![Event::Key { key: Key::A, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND }]);
                self.queue.push_back(vec![Event::Text(text)]);
                self.queue.push_back(vec![]);
                true
            }
            "mark" => {
                self.mark = Some((app.patch.clone(), app.selected, app.inspect));
                false
            }
            "expect" => {
                self.expect(app, n, &w[1..], line);
                false
            }
            _ => {
                self.fail(n, format!("unknown command: {line}"));
                false
            }
        }
    }

    fn expect(&mut self, app: &App, n: usize, w: &[&str], line: &str) {
        let p = &app.patch;
        let f = |i: usize| w.get(i).and_then(|s| s.parse::<f32>().ok());
        let check = |cond: bool, got: String| (cond, got);
        let (ok, got) = match w[0] {
            "route" => {
                let r = find_route(p, w[1], w[2]);
                match (w[3], r) {
                    ("absent", r) => check(r.is_none(), format!("{r:?}")),
                    ("present", r) => check(r.is_some(), format!("{r:?}")),
                    ("amount", Some(r)) => {
                        let a = p.routes[r].amount;
                        check((a - f(4).unwrap_or(0.0)).abs() <= f(5).unwrap_or(0.005), format!("{a:.3}"))
                    }
                    ("bypass", Some(r)) => check(p.routes[r].bypass.to_string() == w[4], p.routes[r].bypass.to_string()),
                    (_, None) => (false, "route missing".into()),
                    _ => (false, "bad expect".into()),
                }
            }
            "routes" => check(p.routes.len() == w[1].parse().unwrap_or(usize::MAX), p.routes.len().to_string()),
            "value" => match find_ctl(p, w[1]) {
                Some(c) => {
                    let v = model::to_value(&p.ctl_def(c).spec, p.modules[c.m].values[c.c]);
                    let want = f(2).unwrap_or(f32::NAN);
                    let tol = f(3).unwrap_or(want.abs() * 0.01 + 1e-3);
                    check((v - want).abs() <= tol, format!("{v:.4}"))
                }
                None => (false, "no control".into()),
            },
            "primary" => match find_ctl(p, w[1]) {
                Some(c) => check(p.modules[c.m].primary[c.c].to_string() == w[2], p.modules[c.m].primary[c.c].to_string()),
                None => (false, "no control".into()),
            },
            "visible" => match find_ctl(p, w[1]) {
                Some(c) => {
                    let vis = app.layout.mods[c.m].ctls[c.c].is_some();
                    check(vis.to_string() == w[2], vis.to_string())
                }
                None => (false, "no control".into()),
            },
            "expanded" => match find_mod(p, w[1]) {
                Some(m) => check(app.geo.expanded[m].to_string() == w[2], app.geo.expanded[m].to_string()),
                None => (false, "no module".into()),
            },
            "cables" => check(format!("{:?}", app.effective_cables().0) == w[1], format!("{:?}", app.effective_cables().0)),
            "selected" => {
                let want = find_mod(p, w[1]);
                check(app.selected == want, format!("{:?}", app.selected.map(|m| mod_key(p, m))))
            }
            "inspect" => {
                let want = if w[1] == "none" { None } else { find_ctl(p, w[1]) };
                check(app.inspect == want, format!("{:?}", app.inspect.map(|c| p.ctl_def(c).id)))
            }
            "hit" => match self.target(app, w[1]) {
                Some(s) => {
                    let h = geom::hit_test(&app.regions, app.xf().inv(s));
                    let got = format!("{h:?}");
                    check(got.starts_with(w[2]), got)
                }
                None => (false, "unresolved target".into()),
            },
            "onscreen" => match self.target(app, w[1]) {
                Some(s) => check(app.canvas.contains(s), format!("{s:?} in {:?}", app.canvas)),
                None => (false, "unresolved target".into()),
            },
            "toast" => {
                let want = w[1..].join(" ");
                let got = app.toast_text();
                check(got.contains(&want), got)
            }
            "same" => match &self.mark {
                Some((pm, sel, ins)) => check(*pm == app.patch && *sel == app.selected && *ins == app.inspect, "patch/selection differ".into()),
                None => (false, "no mark".into()),
            },
            "nooverlap" => {
                let l = &app.layout;
                let mut bad = vec![];
                for (i, a) in l.mods.iter().enumerate() {
                    for (j, b) in l.mods.iter().enumerate().skip(i + 1) {
                        if a.rect.intersect(b.rect).area() > 1.0 {
                            bad.push(format!("{}/{}", mod_key(p, i), mod_key(p, j)));
                        }
                    }
                    let area = a.adv.map_or(a.rect, |x| a.rect.union(x));
                    for g in a.ctls.iter().flatten() {
                        let r = match *g {
                            geom::CtlGeo::Knob { c, r } => egui::Rect::from_center_size(c, vec2(2.0 * r, 2.0 * r)),
                            geom::CtlGeo::Select { rect } => rect,
                        };
                        if !area.contains_rect(r) {
                            bad.push(format!("{} control outside panel", mod_key(p, i)));
                        }
                    }
                }
                check(bad.is_empty(), bad.join(", "))
            }
            "zoom" => check((app.zoom - f(1).unwrap_or(1.0)).abs() < 0.02, format!("{:.2}", app.zoom)),
            _ => (false, "unknown expect".into()),
        };
        let msg = format!("{} → {got}", &line[7..]);
        if ok {
            self.ok(n, msg);
        } else {
            self.fail(n, msg);
        }
    }
}

impl App {
    pub fn toast_text(&self) -> String {
        self.toast.as_ref().map(|(s, _)| s.clone()).unwrap_or_default()
    }
}
