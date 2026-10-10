//! Module faces, drawn over the real `kabl_ui::rack::layout` geometry. Modules that gain a live
//! display get a local layout override; jack identities never change, only positions.
use crate::displays::*;
use crate::prim::*;
use crate::tokens::{Dir, Face};
use crate::widgets::*;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Shape, Stroke};
use kabl_modules::{registry, ModuleInfo, ParamInfo, PortDirection, Taper};
use kabl_ui::rack::{Decor, Geo, Placed};
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
pub struct Xf {
    pub s: f32,
    pub o: egui::Vec2,
}

impl Xf {
    pub fn p(&self, w: Pos2) -> Pos2 {
        pos2(w.x * self.s + self.o.x, w.y * self.s + self.o.y)
    }
    pub fn r(&self, w: Rect) -> Rect {
        Rect::from_min_max(self.p(w.min), self.p(w.max))
    }
    /// Text size in screen px: scaled with the face but never below the 11 px floor.
    pub fn fs(&self, base: f32) -> f32 {
        (base * self.s).max(11.0)
    }
}

pub struct KnobG {
    pub name: &'static str,
    pub c: Pos2,
    pub r: f32,
}
pub struct JackG {
    pub name: &'static str,
    pub out: bool,
    pub c: Pos2,
    pub right: bool,
}
pub struct Geom {
    pub w: f32,
    pub knobs: Vec<KnobG>,
    pub sels: Vec<(&'static str, Rect)>,
    pub jacks: Vec<JackG>,
    pub display: Option<Rect>,
    pub plate: Option<Rect>,
    pub decor: Option<(Decor, Rect)>,
}

fn base_geom(pl: &Placed) -> Geom {
    let o = pl.face.min.to_vec2();
    let mut g = Geom {
        w: pl.face.width(),
        knobs: vec![],
        sels: vec![],
        jacks: vec![],
        display: None,
        plate: None,
        decor: None,
    };
    for c in &pl.ctls {
        match c.geo {
            Geo::Knob { c: p, r } => g.knobs.push(KnobG {
                name: c.param.name,
                c: pos2(p.x - o.x, p.y - o.y),
                r,
            }),
            Geo::Select { rect } => g.sels.push((c.param.name, rect.translate(-o))),
        }
    }
    for j in &pl.jacks {
        g.jacks.push(JackG {
            name: j.port.name,
            out: j.port.direction == PortDirection::Output,
            c: pos2(j.c.x - o.x, j.c.y - o.y),
            right: j.label_right,
        });
    }
    g.plate = pl.plate.map(|r| r.translate(-o));
    g
}

fn kn(name: &'static str, x: f32, y: f32, r: f32) -> KnobG {
    KnobG {
        name,
        c: pos2(x, y),
        r,
    }
}
fn jk(name: &'static str, out: bool, x: f32, y: f32) -> JackG {
    JackG {
        name,
        out,
        c: pos2(x, y),
        right: false,
    }
}
fn rc(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
    Rect::from_min_max(pos2(x0, y0), pos2(x1, y1))
}

/// Local layout for a module, from the real geometry where the module has no live display.
pub fn geom(kind: &str, pl: &Placed) -> Geom {
    let mut g = base_geom(pl);
    match kind {
        "osc.va" => {
            g.display = Some(rc(14.0, 56.0, 196.0, 122.0));
            g.knobs = vec![
                kn("base_hz", 58.0, 178.0, 22.0),
                kn("pw", 152.0, 178.0, 17.0),
            ];
            g.sels = vec![("waveform", rc(14.0, 244.0, 196.0, 270.0))];
            g.jacks = vec![
                jk("pitch", false, 40.0, 304.0),
                jk("sync", false, 105.0, 304.0),
                jk("out", true, 170.0, 304.0),
            ];
            g.plate = Some(rc(138.0, 274.0, 202.0, 330.0));
        }
        "filter.svf" => {
            g.display = Some(rc(14.0, 56.0, 196.0, 126.0));
            g.knobs = vec![
                kn("cutoff_hz", 58.0, 188.0, 22.0),
                kn("resonance", 152.0, 184.0, 17.0),
            ];
            g.jacks = vec![
                jk("in", false, 40.0, 262.0),
                jk("cutoff_cv", false, 105.0, 262.0),
                jk("resonance_cv", false, 170.0, 262.0),
                jk("lp", true, 40.0, 308.0),
                jk("bp", true, 105.0, 308.0),
                jk("hp", true, 170.0, 308.0),
            ];
            g.plate = Some(rc(10.0, 280.0, 200.0, 332.0));
        }
        "env.adsr" => {
            g.display = Some(rc(14.0, 56.0, 226.0, 134.0));
            g.knobs = vec![
                kn("attack_ms", 36.0, 190.0, 17.0),
                kn("decay_ms", 92.0, 190.0, 17.0),
                kn("sustain", 148.0, 190.0, 17.0),
                kn("release_ms", 204.0, 190.0, 17.0),
            ];
            g.sels = vec![("timing", rc(14.0, 254.0, 226.0, 280.0))];
            g.jacks = vec![
                jk("gate", false, 60.0, 308.0),
                jk("out", true, 180.0, 308.0),
            ];
            g.plate = Some(rc(148.0, 286.0, 212.0, 334.0));
        }
        "lfo" => {
            g.display = Some(rc(14.0, 56.0, 196.0, 116.0));
            g.knobs = vec![
                kn("rate_hz", 58.0, 172.0, 22.0),
                kn("phase", 152.0, 172.0, 17.0),
            ];
            g.sels = vec![("waveform", rc(14.0, 240.0, 196.0, 266.0))];
            g.jacks = vec![
                jk("clock", false, 40.0, 304.0),
                jk("reset", false, 105.0, 304.0),
                jk("out", true, 170.0, 304.0),
            ];
            g.plate = Some(rc(138.0, 274.0, 202.0, 330.0));
        }
        "vca" => {
            g.display = Some(rc(14.0, 56.0, 166.0, 122.0));
            g.knobs = vec![kn("gain", 90.0, 178.0, 22.0)];
            g.sels = vec![("exponential", rc(14.0, 244.0, 166.0, 270.0))];
            g.jacks = vec![
                jk("in", false, 34.0, 304.0),
                jk("cv", false, 90.0, 304.0),
                jk("out", true, 146.0, 304.0),
            ];
            g.plate = Some(rc(114.0, 274.0, 176.0, 330.0));
        }
        "midi.in" => {
            g.display = Some(rc(12.0, 58.0, 168.0, 104.0));
            g.sels = vec![
                ("mode", rc(14.0, 142.0, 166.0, 168.0)),
                ("glide", rc(14.0, 196.0, 166.0, 222.0)),
            ];
            g.jacks = vec![
                jk("gate", true, 34.0, 304.0),
                jk("pitch", true, 71.0, 304.0),
                jk("velocity", true, 109.0, 304.0),
                jk("wheel", true, 146.0, 304.0),
            ];
            g.plate = Some(rc(6.0, 276.0, 174.0, 332.0));
        }
        "out" => {
            g.jacks = vec![
                JackG {
                    name: "left",
                    out: false,
                    c: pos2(36.0, 232.0),
                    right: true,
                },
                JackG {
                    name: "right",
                    out: false,
                    c: pos2(36.0, 284.0),
                    right: true,
                },
            ];
            g.plate = None;
        }
        _ => {}
    }
    g
}

pub struct FaceIn<'a> {
    pub id: u32,
    pub kind: &'a str,
    pub info: &'static ModuleInfo,
    pub params: &'a BTreeMap<String, f32>,
    pub geom: Geom,
    pub origin: Pos2,
    pub conn: &'a dyn Fn(&str, bool) -> bool,
    pub selected: bool,
    pub sweep: f32,
    pub hot: Option<&'a str>,
    pub name: Option<&'a str>,
}

fn pinfo(info: &ModuleInfo, n: &str) -> Option<&'static ParamInfo> {
    info.params.iter().find(|p| p.name == n)
}

pub fn pval(f: &FaceIn, n: &str) -> f32 {
    f.params
        .get(n)
        .copied()
        .or_else(|| pinfo(f.info, n).map(|p| p.default))
        .unwrap_or(0.0)
}

pub fn draw_body(cx: &Cx, xf: Xf, r: Rect, face: &Face, selected: bool) {
    let k = cx.k;
    let sr = xf.r(r);
    match k.dir {
        Dir::A => {
            cx.shadow(sr, 3.0, 1);
            let top = mix(face.base, Color32::WHITE, if k.dark { 0.06 } else { 0.10 });
            let bot = mix(face.base, Color32::BLACK, 0.08);
            cx.grad(sr, 3.0, top, bot);
            let mut y = sr.top() + 2.0;
            while y < sr.bottom() {
                cx.line(
                    pos2(sr.left() + 1.0, y),
                    pos2(sr.right() - 1.0, y),
                    0.5,
                    Color32::from_white_alpha(if k.dark { 3 } else { 5 }),
                );
                y += 3.7;
            }
            cx.line(
                sr.left_top() + vec2(2.0, 1.0),
                sr.right_top() + vec2(-2.0, 1.0),
                1.0,
                Color32::from_white_alpha(70),
            );
            cx.rr_stroke(sr, 3.0, 1.0, mix(face.base, Color32::BLACK, 0.30));
            for (sx, sy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                let c = pos2(
                    sr.left() + 9.0 * xf.s + sx * (sr.width() - 18.0 * xf.s),
                    sr.top() + 9.0 * xf.s + sy * (sr.height() - 18.0 * xf.s),
                );
                cx.p.circle_filled(c, 3.4 * xf.s, mix(face.base, Color32::BLACK, 0.45));
                cx.p.circle_stroke(c, 3.4 * xf.s, Stroke::new(1.0, a(Color32::WHITE, 90)));
            }
        }
        Dir::B => {
            cx.shadow(sr, 6.0, 2);
            let top = mix(face.base, Color32::WHITE, if k.dark { 0.10 } else { 0.28 });
            let bot = mix(face.base, Color32::BLACK, if k.dark { 0.34 } else { 0.16 });
            cx.grad(sr, 6.0, top, bot);
            let gl = Rect::from_min_max(
                sr.min + vec2(1.0, 1.0),
                pos2(sr.right() - 1.0, sr.top() + sr.height() * 0.30),
            );
            cx.grad(
                gl,
                5.0,
                Color32::from_white_alpha(if k.dark { 26 } else { 60 }),
                Color32::from_white_alpha(0),
            );
            cx.rr_stroke(
                sr,
                6.0,
                1.0,
                a(Color32::WHITE, if k.dark { 50 } else { 120 }),
            );
        }
        Dir::C => {
            cx.rr(sr, 2.0, face.base);
            cx.rr_stroke(sr, 2.0, 2.0, face.ink);
        }
    }
    if selected {
        let o = sr.expand(3.0);
        if k.dir == Dir::B {
            for (m, al) in [(5.0, 24), (3.0, 48)] {
                cx.rr_stroke(sr.expand(m), 9.0, 2.0, a(k.focus, al));
            }
        }
        cx.rr_stroke(o, 5.0, 2.0, k.focus);
    }
}

pub fn draw_face(cx: &Cx, xf: Xf, f: &FaceIn) {
    let k = cx.k;
    let face = k.face(f.kind);
    let g = &f.geom;
    let h = 340.0;
    let w = g.w;
    let fr = Rect::from_min_size(f.origin, vec2(w, h));
    draw_body(cx, xf, fr, &face, f.selected);
    let loc = |p: Pos2| xf.p(f.origin + p.to_vec2());
    let locr = |r: Rect| xf.r(r.translate(f.origin.to_vec2()));
    // Title block.
    let name = f.name.unwrap_or(f.info.name);
    let sub = format!("{} · #{}", f.kind, f.id);
    match k.dir {
        Dir::A => {
            cx.text(
                loc(pos2(w / 2.0, 24.0)),
                Align2::CENTER_CENTER,
                name,
                xf.fs(15.0),
                "sans-semi",
                face.ink,
            );
            cx.text(
                loc(pos2(w / 2.0, 41.0)),
                Align2::CENTER_CENTER,
                &sub,
                xf.fs(10.5),
                "mono",
                face.ink2,
            );
        }
        Dir::B => {
            cx.p.circle_filled(loc(pos2(16.0, 24.0)), 3.5 * xf.s.max(0.8), face.ink);
            cx.text(
                loc(pos2(26.0, 24.0)),
                Align2::LEFT_CENTER,
                name,
                xf.fs(15.0),
                "sans-semi",
                face.ink,
            );
            cx.text(
                loc(pos2(26.0, 41.0)),
                Align2::LEFT_CENTER,
                &sub,
                xf.fs(10.5),
                "mono",
                face.ink2,
            );
        }
        Dir::C => {
            cx.text(
                loc(pos2(14.0, 22.0)),
                Align2::LEFT_CENTER,
                &name.to_uppercase(),
                xf.fs(16.0),
                "cond-semi",
                face.ink,
            );
            cx.text(
                loc(pos2(14.0, 39.0)),
                Align2::LEFT_CENTER,
                &sub,
                xf.fs(11.0),
                "mono",
                face.ink2,
            );
            let y = loc(pos2(0.0, 48.0)).y;
            let x0 = loc(pos2(8.0, 0.0)).x;
            let x1 = loc(pos2(w - 8.0, 0.0)).x;
            cx.line(pos2(x0, y), pos2(x1, y), 2.0, face.ink);
        }
    }
    // Output plate.
    if let Some(pr) = g.plate {
        let r = locr(pr);
        match k.dir {
            Dir::A => {
                cx.rr(r, 5.0, a(face.ink, 26));
                cx.rr_stroke(r, 5.0, 1.0, a(face.ink, 40));
            }
            Dir::B => cx.rr(
                r,
                10.0,
                Color32::from_black_alpha(if k.dark { 70 } else { 36 }),
            ),
            Dir::C => {
                cx.rr_stroke(r, 2.0, 1.6, face.ink);
            }
        }
    }
    // Display.
    if let Some(d) = g.display {
        let r = locr(d);
        match f.kind {
            "osc.va" => scope(cx, r, pval(f, "waveform") as usize, 2.5),
            "filter.svf" => filter_view(
                cx,
                r,
                pval(f, "cutoff_hz"),
                pval(f, "resonance"),
                f.sweep,
                0,
            ),
            "env.adsr" => env_view(
                cx,
                r,
                &Adsr {
                    a_ms: pval(f, "attack_ms"),
                    d_ms: pval(f, "decay_ms"),
                    s: pval(f, "sustain"),
                    r_ms: pval(f, "release_ms"),
                },
            ),
            "lfo" => lfo_view(cx, r, pval(f, "waveform") as usize, pval(f, "rate_hz")),
            "vca" => vca_view(cx, r, pval(f, "gain"), pval(f, "exponential") > 0.5),
            "midi.in" => keys(cx, r, &face),
            _ => {}
        }
    }
    if f.kind == "out" {
        speaker(cx, loc(pos2(w / 2.0, 120.0)), 40.0 * xf.s, &face);
    }
    // Selectors.
    for (n, r) in &g.sels {
        let Some(p) = pinfo(f.info, n) else { continue };
        let opts = kabl_ui::routing::step_labels(f.kind, n).unwrap_or(&["OFF", "ON"]);
        let sel = (pval(f, n) - p.min).round().max(0.0) as usize;
        let sr = locr(*r);
        cx.text(
            pos2(sr.left() + 1.0, sr.top() - 9.0 * xf.s.max(0.9)),
            Align2::LEFT_CENTER,
            &label(cx, &kabl_ui::routing::param_label(p)),
            xf.fs(11.0),
            lab_font(k.dir),
            face.ink2,
        );
        segmented(cx, sr, opts, sel.min(opts.len() - 1), &face, xf.fs(10.5));
    }
    // Knobs.
    for kg in &g.knobs {
        let Some(p) = pinfo(f.info, kg.name) else {
            continue;
        };
        let v = pval(f, kg.name);
        let n = if p.taper == Taper::Stepped {
            (v - p.min) / (p.max - p.min)
        } else {
            p.to_norm(v)
        };
        let c = loc(kg.c);
        let rr = kg.r * xf.s;
        let mut mv = None;
        let mut shown = kabl_ui::routing::fmt_value(p, v);
        if f.kind == "filter.svf" && kg.name == "cutoff_hz" && f.sweep > 0.0 {
            let sw = f.sweep;
            let fc = v * 2f32.powf(sw * (cx.t * 0.8 * std::f32::consts::TAU).sin());
            let lo = p.to_norm((v * 2f32.powf(-sw)).max(p.min));
            let hi = p.to_norm((v * 2f32.powf(sw)).min(p.max));
            mv = Some(ModVis {
                lo,
                hi,
                cur: p.to_norm(fc.clamp(p.min, p.max)),
            });
            shown = kabl_ui::routing::fmt_value(p, fc.clamp(p.min, p.max));
        }
        knob(cx, c, rr, n, mv, &face, f.hot == Some(kg.name));
        let lab = label(cx, &kabl_ui::routing::param_label(p));
        cx.text(
            c - vec2(
                0.0,
                rr + (if mv.is_some() { 27.0 } else { 20.0 }) * xf.s.max(0.9),
            ),
            Align2::CENTER_CENTER,
            &lab,
            xf.fs(11.0),
            lab_font(k.dir),
            face.ink2,
        );
        cx.text(
            c + vec2(0.0, rr + 15.0 * xf.s.max(0.9)),
            Align2::CENTER_CENTER,
            &shown,
            xf.fs(12.0),
            "mono-med",
            if mv.is_some() && k.dir != Dir::C {
                face.ink
            } else {
                face.ink
            },
        );
    }
    // Jacks.
    for jg in &g.jacks {
        let Some(pi) = f.info.ports.iter().find(|p| p.name == jg.name) else {
            continue;
        };
        let c = loc(jg.c);
        let connected = (f.conn)(jg.name, jg.out);
        let col = k.signal(pi.port_type);
        jack(cx, c, 11.0 * xf.s, col, connected, &face);
        if jg.right {
            cx.text(
                c + vec2(18.0 * xf.s.max(0.9), 0.0),
                Align2::LEFT_CENTER,
                &label(cx, port_label(jg.name)),
                xf.fs(11.0),
                lab_font(k.dir),
                face.ink,
            );
        } else {
            cx.text(
                c - vec2(0.0, 22.0 * xf.s.max(0.9)),
                Align2::CENTER_CENTER,
                &label(cx, port_label(jg.name)),
                xf.fs(10.5),
                lab_font(k.dir),
                face.ink,
            );
        }
    }
}

pub fn lab_font(d: Dir) -> &'static str {
    match d {
        Dir::A => "sans-med",
        Dir::B => "sans-semi",
        Dir::C => "cond-semi",
    }
}

fn label(cx: &Cx, s: &str) -> String {
    match cx.k.dir {
        Dir::A => s.to_string(),
        _ => s.to_uppercase(),
    }
}

pub fn port_label(n: &str) -> &str {
    match n {
        "cutoff_cv" => "Cutoff CV",
        "resonance_cv" => "Res CV",
        "lp" => "LP",
        "bp" => "BP",
        "hp" => "HP",
        "in" => "In",
        "out" => "Out",
        "cv" => "CV",
        "gate" => "Gate",
        "pitch" => "Pitch",
        "sync" => "Sync",
        "clock" => "Clock",
        "reset" => "Reset",
        "velocity" => "Vel",
        "wheel" => "Wheel",
        "left" => "Left",
        "right" => "Right",
        o => o,
    }
}

fn vca_view(cx: &Cx, r: Rect, gain: f32, exp: bool) {
    let k = cx.k;
    let p = frame(cx, r, (4, 4));
    let pl = p.shrink2(vec2(3.0, 3.0));
    let f = |x: f32| (if exp { x * x * x } else { x }) * gain.clamp(0.0, 1.0);
    let n = pl.width() as usize;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let x = i as f32 / n as f32;
            pos2(pl.left() + x * pl.width(), pl.bottom() - f(x) * pl.height())
        })
        .collect();
    fill_under(cx, &pts, pl.bottom(), k.disp_trace, 60);
    trace(cx, pts, k.disp_trace, 1.8);
    let (_, ph) = gate(cx.t);
    let x = (ph / 3.6).min(1.0);
    let ev = crate::displays::env_value(
        &Adsr {
            a_ms: 8.0,
            d_ms: 240.0,
            s: 0.6,
            r_ms: 420.0,
        },
        x,
    );
    let d = pos2(
        pl.left() + ev * pl.width(),
        pl.bottom() - f(ev) * pl.height(),
    );
    cx.line(pos2(d.x, pl.bottom()), d, 1.0, a(k.disp_trace, 110));
    cx.line(pos2(pl.left(), d.y), d, 1.0, a(k.disp_trace, 110));
    cx.glow_dot(
        d,
        3.0,
        if k.dir == Dir::C {
            k.accent
        } else {
            Color32::WHITE
        },
        0.8,
    );
}

fn keys(cx: &Cx, r: Rect, face: &Face) {
    let k = cx.k;
    cx.rr(r, 3.0, a(Color32::BLACK, 70));
    let n = 14;
    let w = r.width() / n as f32;
    let held = [
        (((cx.t * 0.5).floor() as usize) * 2) % 8 + 2,
        ((cx.t * 0.5).floor() as usize * 2) % 8 + 6,
    ];
    for i in 0..n {
        let kr = Rect::from_min_size(
            pos2(r.left() + i as f32 * w + 0.5, r.top() + 1.0),
            vec2(w - 1.0, r.height() - 2.0),
        );
        let on = held.contains(&i);
        cx.rr(
            kr,
            1.5,
            if on {
                k.accent
            } else if k.dark {
                Color32::from_gray(0xe8)
            } else {
                Color32::from_gray(0xfa)
            },
        );
    }
    for i in 0..n - 1 {
        if matches!(i % 7, 2 | 6) {
            continue;
        }
        let kr = Rect::from_min_size(
            pos2(r.left() + (i as f32 + 0.68) * w, r.top() + 1.0),
            vec2(w * 0.64, r.height() * 0.58),
        );
        cx.rr(kr, 1.0, face.ink);
    }
}

/// One-line helper for scenes that need a face for a kind in a given state.
pub fn face_for<'a>(
    kind: &'a str,
    id: u32,
    params: &'a BTreeMap<String, f32>,
    pl: &Placed,
    origin: Pos2,
    conn: &'a dyn Fn(&str, bool) -> bool,
) -> Option<FaceIn<'a>> {
    let info = registry::info_for(kind)?;
    Some(FaceIn {
        id,
        kind,
        info,
        params,
        geom: geom(kind, pl),
        origin,
        conn,
        selected: false,
        sweep: 0.0,
        hot: None,
        name: None,
    })
}

pub fn _shape(_: Shape) {}

fn speaker(cx: &Cx, c: Pos2, r: f32, face: &Face) {
    let k = cx.k;
    cx.p.circle_filled(c, r, a(face.ink, if k.dir == Dir::C { 255 } else { 40 }));
    for (i, rr) in [0.82, 0.62, 0.42].iter().enumerate() {
        cx.p.circle_stroke(
            c,
            r * rr,
            Stroke::new(
                1.5,
                if k.dir == Dir::C {
                    face.base
                } else {
                    a(face.ink, 90 + 30 * i as u8)
                },
            ),
        );
    }
    cx.p.circle_filled(
        c,
        r * 0.16,
        if k.dir == Dir::C {
            face.base
        } else {
            a(face.ink, 130)
        },
    );
}
