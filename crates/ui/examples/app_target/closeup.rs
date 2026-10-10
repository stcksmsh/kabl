//! Face close-up, wavetable concept, step-pattern cable concept and the component sheet.
use crate::chrome::*;
use crate::displays::*;
use crate::faces::*;
use crate::perform::card;
use crate::prim::*;
use crate::rack::*;
use crate::scenes::Ctxs;
use crate::tokens::{Dir, Face};
use crate::widgets::*;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Shape, Stroke};
use std::collections::BTreeMap;

/// A placed module's rect and its jack anchors.
type Anchored = (Rect, BTreeMap<(String, bool), Pos2>);

fn find(rk: &Rack, id: u32) -> &kabl_ui::rack::Placed {
    rk.lay.mods.iter().find(|m| m.id as u32 == id).unwrap()
}

/// Draw a real module at an arbitrary origin; returns its jack positions in world space.
#[allow(clippy::too_many_arguments)]
fn put(cx: &Cx, xf: Xf, rk: &Rack, id: u32, org: Pos2, tweak: &dyn Fn(&mut BTreeMap<String, f32>), sel: bool, sweep: f32, conn: &dyn Fn(&str, bool) -> bool) -> (Rect, BTreeMap<(String, bool), Pos2>) {
    let pl = find(rk, id);
    let mut params = rk.st.modules[&(id as _)].params.clone();
    tweak(&mut params);
    let g = geom(pl.info.kind, pl);
    let jacks: BTreeMap<(String, bool), Pos2> = g.jacks.iter().map(|j| ((j.name.to_string(), j.out), org + j.c.to_vec2())).collect();
    let w = g.w;
    let f = FaceIn { id, kind: pl.info.kind, info: pl.info, params: &params, geom: g, origin: org, conn, selected: sel, sweep, hot: None, name: None };
    draw_face(cx, xf, &f);
    (Rect::from_min_size(org, vec2(w, 340.0)), jacks)
}

fn wire(cx: &Cx, xf: Xf, a_: Pos2, b_: Pos2, col: Color32, alpha: f32, sag: f32) -> Vec<Pos2> {
    let pts = cable_pts(xf.p(a_), xf.p(b_), sag * xf.s);
    cable(cx, pts.clone(), col, alpha, true);
    plug(cx, xf.p(a_), col, alpha);
    plug(cx, xf.p(b_), col, alpha);
    pts
}

fn badge(cx: &Cx, c: Pos2, n: usize) {
    let k = cx.k;
    cx.p.circle_filled(c, 9.0, k.accent);
    cx.p.circle_stroke(c, 9.0, Stroke::new(1.5, if k.dark { Color32::BLACK } else { Color32::WHITE }));
    cx.text(c, Align2::CENTER_CENTER, &n.to_string(), 11.0, "sans-semi", k.on_accent);
}

pub fn faces(cx: &Cx, d: &Ctxs, size: egui::Vec2) {
    let k = cx.k;
    let rk = &d.reference;
    let w = size.x;
    cx.p.rect_filled(Rect::from_min_size(pos2(0.0, 0.0), size), 0.0, k.rack);
    // Header.
    cx.p.rect_filled(Rect::from_min_size(pos2(0.0, 0.0), vec2(w, 52.0)), 0.0, k.surface);
    cx.line(pos2(0.0, 52.0), pos2(w, 52.0), if k.dir == Dir::C { 1.5 } else { 1.0 }, if k.dir == Dir::C { k.line2 } else { k.line });
    heading(cx, pos2(24.0, 27.0), "Module faces · live displays");
    cx.text(pos2(w - 24.0, 27.0), Align2::RIGHT_CENTER, k.dir.title(), 13.0, "sans-med", k.text2);
    let gap = 16.0;
    let ids = [(2u32, 0.0), (3, 0.0), (7, 0.0), (4, 0.0), (5, 0.0)];
    let total: f32 = 210.0 * 3.0 + 240.0 + 180.0 + gap * 4.0;
    let x0 = (w - total) / 2.0;
    let y0 = 76.0;
    let xf = Xf { s: 1.0, o: vec2(0.0, 0.0) };
    let mut x = x0;
    let mut pos: BTreeMap<u32, Anchored> = BTreeMap::new();
    let conns = [(2u32, "out", true), (3, "in", false), (3, "cutoff_cv", false), (7, "out", true), (4, "out", true), (5, "cv", false)];
    let conn = |id: u32| move |p: &str, o: bool| conns.iter().any(|c| c.0 == id && c.1 == p && c.2 == o);
    for (id, _) in ids {
        let c = conn(id);
        let tweak = |p: &mut BTreeMap<String, f32>| {
            if id == 7 {
                p.insert("waveform".into(), 0.0);
                p.insert("rate_hz".into(), 0.8);
            }
        };
        let r = put(cx, xf, rk, id, pos2(x, y0), &tweak, false, if id == 3 { 0.75 } else { 0.0 }, &c);
        x = r.0.right() + gap;
        pos.insert(id, r);
    }
    // Patch cables between neighbours.
    let j = |id: u32, p: &str, o: bool| pos[&id].1[&(p.to_string(), o)];
    wire(cx, xf, j(2, "out", true), j(3, "in", false), k.audio, 1.0, 36.0);
    wire(cx, xf, j(7, "out", true), j(3, "cutoff_cv", false), k.cv, 1.0, 40.0);
    wire(cx, xf, j(4, "out", true), j(5, "cv", false), k.cv, 1.0, 36.0);
    // Numbered markers.
    let marks = [(2u32, pos2(26.0, 62.0)), (3, pos2(26.0, 62.0)), (7, pos2(26.0, 62.0)), (4, pos2(26.0, 62.0)), (5, pos2(26.0, 62.0))];
    for (i, (id, off)) in marks.iter().enumerate() {
        let _ = off;
        badge(cx, pos[id].0.right_top() + vec2(-18.0, 22.0), i + 1);
    }
    // Callouts.
    let notes = [
        ("Oscilloscope", "Triggered trace with three-frame persistence; amplitude follows the gate."),
        ("Filter response", "Curve from the live cutoff and resonance. Bars: input spectrum and filtered result."),
        ("LFO", "Waveform with a phase dot at the current rate; same dot drives the filter ring."),
        ("Envelope curve", "ADSR in log-scaled time so an 8 ms attack stays visible; playhead follows the gate."),
        ("VCA transfer", "LIN/EXP gain curve; the dot is the live CV level from the envelope."),
    ];
    let cy = y0 + 340.0 + 20.0;
    for (i, (id, _)) in ids.iter().enumerate() {
        let fr = pos[id].0;
        let r = Rect::from_min_size(pos2(fr.left(), cy), vec2(fr.width(), 112.0));
        let (title, body) = notes[i];
        let inner = card(cx, r, &format!("{}  {}", i + 1, title), "");
        let lines = wrap(cx, body, 12.0, inner.width());
        for (li, l) in lines.iter().enumerate().take(4) {
            cx.text(pos2(inner.left(), inner.top() + 2.0 + li as f32 * 15.0), Align2::LEFT_TOP, l, 12.0, "sans", k.text);
        }
    }
    // Knob modulation anatomy.
    let lens = Rect::from_min_max(pos2(x0, cy + 124.0), pos2(w - x0, size.y - 14.0));
    if lens.height() > 120.0 {
        let inner = card(cx, lens, "Modulation on a knob · filter cutoff, 3×", "LFO → Cutoff CV, depth ±0.75 oct");
        let face = k.face("filter.svf");
        let fb = Rect::from_min_size(inner.min + vec2(-8.0, -2.0), vec2(190.0, inner.height() + 4.0));
        cx.rr(fb, k.r_md, face.base);
        let c = fb.center() + vec2(0.0, 2.0);
        let r = (inner.height() * 0.26).min(56.0);
        let base = 1200.0f32;
        let sw = 0.75;
        let fc = base * 2f32.powf(sw * (cx.t * 0.8 * std::f32::consts::TAU).sin());
        let n = |f: f32| ((f / 20.0).ln() / (1000.0f32).ln()).clamp(0.0, 1.0);
        knob(cx, c, r, n(base), Some(ModVis { lo: n(base * 2f32.powf(-sw)), hi: n(base * 2f32.powf(sw)), cur: n(fc) }), &face, false);
        let lx = fb.right() + 28.0;
        for (i, (col, t1, t2)) in [(k.text, "Base", "1.20 kHz · the knob pointer, set by hand"), (k.cv, "Range", "714 Hz – 2.02 kHz · the arc, set by depth"), (Color32::WHITE, "Now", &*format!("{:.2} kHz · the dot, moving live", fc / 1000.0))].iter().enumerate() {
            let y = inner.top() + 14.0 + i as f32 * 28.0;
            cx.p.circle_filled(pos2(lx, y), 5.0, *col);
            cx.p.circle_stroke(pos2(lx, y), 5.0, Stroke::new(1.0, k.line2));
            cx.text(pos2(lx + 14.0, y), Align2::LEFT_CENTER, t1, 13.0, "sans-semi", k.text);
            cx.text(pos2(lx + 70.0, y), Align2::LEFT_CENTER, t2, 12.5, "sans", k.text2);
        }
        // Scrolling history.
        let hx = lx + 440.0;
        let hr = Rect::from_min_max(pos2(hx.min(lens.right() - 320.0), inner.top() + 2.0), pos2(lens.right() - 18.0, inner.bottom() - 2.0));
        let p = frame(cx, hr, (6, 3));
        let n_ = p.width() as usize;
        let pts: Vec<Pos2> = (0..=n_)
            .map(|i| {
                let tt = cx.t - 4.0 + 4.0 * i as f32 / n_ as f32;
                let f = base * 2f32.powf(sw * (tt * 0.8 * std::f32::consts::TAU).sin());
                pos2(p.left() + i as f32, p.bottom() - n(f) * p.height() * 0.9 - 4.0)
            })
            .collect();
        trace(cx, pts.clone(), k.disp_trace, 1.8);
        cx.glow_dot(*pts.last().unwrap(), 3.0, Color32::WHITE, 0.8);
        cx.text(pos2(p.left() + 4.0, p.top() + 2.0), Align2::LEFT_TOP, "cutoff, last 4 s", 11.0, "sans", a(k.disp_text, 200));
    }
}

pub fn wrap(cx: &Cx, s: &str, size: f32, maxw: f32) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    for wd in s.split_whitespace() {
        let t = if cur.is_empty() { wd.to_string() } else { format!("{cur} {wd}") };
        if cx.width(&t, size, "sans") > maxw && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
            cur = wd.to_string();
        } else {
            cur = t;
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

// ---------------------------------------------------------------- wavetable

fn wt_face(cx: &Cx, xf: Xf, org: Pos2, sel: bool) -> BTreeMap<&'static str, Pos2> {
    let k = cx.k;
    let face = k.face("osc.va");
    let fr = Rect::from_min_size(org, vec2(360.0, 340.0));
    draw_body(cx, xf, fr, &face, sel);
    let loc = |p: Pos2| xf.p(org + p.to_vec2());
    let locr = |x0: f32, y0: f32, x1: f32, y1: f32| xf.r(Rect::from_min_max(pos2(x0, y0), pos2(x1, y1)).translate(org.to_vec2()));
    match k.dir {
        Dir::A => {
            cx.text(loc(pos2(180.0, 24.0)), Align2::CENTER_CENTER, "Wavetable Oscillator", 15.0, "sans-semi", face.ink);
            cx.text(loc(pos2(180.0, 41.0)), Align2::CENTER_CENTER, "osc.wt · #9 · concept", 10.5, "mono", face.ink2);
        }
        Dir::B => {
            cx.p.circle_filled(loc(pos2(16.0, 24.0)), 3.5, face.ink);
            cx.text(loc(pos2(26.0, 24.0)), Align2::LEFT_CENTER, "Wavetable Oscillator", 15.0, "sans-semi", face.ink);
            cx.text(loc(pos2(26.0, 41.0)), Align2::LEFT_CENTER, "osc.wt · #9 · concept", 10.5, "mono", face.ink2);
        }
        Dir::C => {
            cx.text(loc(pos2(14.0, 22.0)), Align2::LEFT_CENTER, "WAVETABLE OSCILLATOR", 16.0, "cond-semi", face.ink);
            cx.text(loc(pos2(14.0, 39.0)), Align2::LEFT_CENTER, "osc.wt · #9 · concept", 11.0, "mono", face.ink2);
            cx.line(pos2(loc(pos2(8.0, 0.0)).x, loc(pos2(0.0, 48.0)).y), pos2(loc(pos2(352.0, 0.0)).x, loc(pos2(0.0, 48.0)).y), 2.0, face.ink);
        }
    }
    let pos = 0.5 + 0.4 * (cx.t * 0.8 * std::f32::consts::TAU).sin();
    let warp = 0.25 + 0.2 * (cx.t * 0.35).sin();
    let dr = locr(14.0, 56.0, 346.0, 176.0);
    wavetable_view(cx, dr, pos, warp, 24);
    // Table chip and single-cycle inset.
    let chip_r = Rect::from_min_size(dr.left_top() + vec2(8.0, 7.0), vec2(112.0, 22.0));
    cx.rr(chip_r, if k.dir == Dir::B { 11.0 } else { 3.0 }, a(Color32::BLACK, 120));
    cx.text(chip_r.left_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, "Vowels", 12.0, "sans-semi", k.disp_text);
    cx.p.add(Shape::line(vec![chip_r.right_center() + vec2(-16.0, -2.0), chip_r.right_center() + vec2(-11.0, 3.0), chip_r.right_center() + vec2(-6.0, -2.0)], Stroke::new(1.5, k.disp_text)));
    let ins = Rect::from_min_size(pos2(dr.right() - 86.0, dr.bottom() - 46.0), vec2(80.0, 40.0));
    wt_cycle(cx, ins, pos, warp);
    // Knobs.
    let kn_: [(&str, f32, String, Option<ModVis>); 5] = [
        ("Position", pos, format!("{:02}", (pos * 23.0).round() as i32 + 1), Some(ModVis { lo: 0.1, hi: 0.9, cur: pos })),
        ("Warp", 0.25, "25 %".into(), None),
        ("Unison", 0.33, "3 voices".into(), None),
        ("Detune", 0.4, "12 ct".into(), None),
        ("Tune", 0.5, "+0 st".into(), None),
    ];
    for (i, (nm, v, txt, mv)) in kn_.iter().enumerate() {
        let c = loc(pos2(40.0 + i as f32 * 70.0, 228.0));
        knob(cx, c, 17.0 * xf.s, *v, *mv, &face, false);
        cx.text(c - vec2(0.0, 36.0 * xf.s.max(0.9) + if mv.is_some() { 6.0 } else { 0.0 }), Align2::CENTER_CENTER, &if k.dir == Dir::A { nm.to_string() } else { nm.to_uppercase() }, 11.0, lab_font(k.dir), face.ink2);
        cx.text(c + vec2(0.0, 32.0 * xf.s.max(0.9)), Align2::CENTER_CENTER, txt, 12.0, "mono-med", face.ink);
    }
    let mut out = BTreeMap::new();
    let jk: [(&'static str, &str, f32, bool, Color32); 5] = [("pitch", "Pitch", 40.0, false, k.cv), ("sync", "Sync", 110.0, false, k.gate), ("pos_cv", "Pos CV", 180.0, false, k.cv), ("warp_cv", "Warp CV", 250.0, false, k.cv), ("out", "Out", 320.0, true, k.audio)];
    let plate = locr(284.0, 276.0, 350.0, 332.0);
    match k.dir {
        Dir::A => cx.rr(plate, 5.0, a(face.ink, 26)),
        Dir::B => cx.rr(plate, 10.0, Color32::from_black_alpha(if k.dark { 70 } else { 36 })),
        Dir::C => cx.rr_stroke(plate, 2.0, 1.6, face.ink),
    }
    for (id, label, x, _o, col) in jk {
        let c = loc(pos2(x, 308.0));
        let connected = id == "out" || id == "pos_cv" || id == "pitch";
        jack(cx, c, 11.0 * xf.s, col, connected, &face);
        cx.text(c - vec2(0.0, 22.0 * xf.s.max(0.9)), Align2::CENTER_CENTER, &if k.dir == Dir::A { label.to_string() } else { label.to_uppercase() }, 10.5, lab_font(k.dir), face.ink);
        out.insert(id, org + vec2(x, 308.0));
    }
    out
}

pub fn wavetable(cx: &Cx, d: &Ctxs, size: egui::Vec2) {
    let k = cx.k;
    let rk = &d.reference;
    let ar = areas(k.dir, size, false, 44.0);
    toolbar(cx, ar.toolbar, "Rack", "Vowel Pad", true, "100%", (false, false), 1);
    rail(cx, ar.right, 0);
    status(cx, ar.status, (0.55 + 0.1 * (cx.t * 3.0).sin(), 0.5 + 0.1 * (cx.t * 2.7).sin()));
    cx.p.rect_filled(ar.center, 0.0, k.rack);
    let strip_h = (ar.center.height() - 400.0).clamp(188.0, 300.0);
    let rack_area = Rect::from_min_size(ar.center.min, vec2(ar.center.width(), 400.0));
    let world = Rect::from_min_size(pos2(0.0, 10.0), vec2(210.0 + 360.0 + 210.0 + 180.0 + 150.0 + 16.0 * 4.0, 340.0)).expand2(vec2(14.0, 20.0));
    let xf = fit(world, rack_area, vec2(8.0, 2.0), 1.0);
    let rails = |x0: f32, x1: f32| {
        for ry in [-6.0f32, 353.0] {
            let r = xf.r(Rect::from_min_max(pos2(x0, ry), pos2(x1, ry + 13.0)));
            cx.rr(r, 2.0, k.rail);
            let mut x = x0 + 15.0;
            while x < x1 - 6.0 {
                cx.p.circle_filled(xf.p(pos2(x, ry + 6.5)), 2.4 * xf.s.max(0.8), k.hole);
                x += 30.0;
            }
        }
    };
    rails(world.left(), world.right());
    let mut x = 6.0;
    let none = |_: &mut BTreeMap<String, f32>| {};
    let conn = |_: &str, _: bool| true;
    let (_, lfo) = put(cx, xf, rk, 7, pos2(x, 10.0), &|p| { p.insert("waveform".into(), 0.0); p.insert("rate_hz".into(), 0.8); }, false, 0.0, &conn);
    x += 210.0 + 16.0;
    let wt_org = pos2(x, 10.0);
    let wt = wt_face(cx, xf, wt_org, true);
    x += 360.0 + 16.0;
    let (_, flt) = put(cx, xf, rk, 3, pos2(x, 10.0), &none, false, 0.0, &conn);
    x += 210.0 + 16.0;
    let (_, vca) = put(cx, xf, rk, 5, pos2(x, 10.0), &none, false, 0.0, &conn);
    x += 180.0 + 16.0;
    let (_, out) = put(cx, xf, rk, 6, pos2(x, 10.0), &none, false, 0.0, &conn);
    let kj = |m: &BTreeMap<(String, bool), Pos2>, p: &str, o: bool| m[&(p.to_string(), o)];
    wire(cx, xf, kj(&lfo, "out", true), wt["pos_cv"], k.cv, 1.0, 54.0);
    wire(cx, xf, wt["out"], kj(&flt, "in", false), k.audio, 1.0, 40.0);
    wire(cx, xf, kj(&flt, "lp", true), kj(&vca, "in", false), k.audio, 1.0, 36.0);
    wire(cx, xf, kj(&vca, "out", true), kj(&out, "left", false), k.audio, 1.0, 36.0);
    // Table strip.
    let strip = Rect::from_min_max(pos2(ar.center.min.x + 12.0, ar.center.max.y - strip_h), pos2(ar.center.max.x - 12.0, ar.center.max.y - 10.0));
    let inner = card(cx, strip, "Wavetable · Vowels", "24 frames · drag to scrub · import .wav");
    let n = 24usize;
    let pos = 0.5 + 0.4 * (cx.t * 0.8 * std::f32::consts::TAU).sin();
    let cw = (inner.width() - 6.0 * (n as f32 - 1.0)).max(0.0) / n as f32;
    let cur = (pos * (n as f32 - 1.0)).round() as usize;
    for i in 0..n {
        let th = (inner.height() - 110.0).clamp(64.0, 110.0);
        let r = Rect::from_min_size(pos2(inner.left() + i as f32 * (cw + 6.0), inner.top()), vec2(cw, th));
        cx.rr(r, 3.0, k.disp_bg);
        let m = i as f32 / (n as f32 - 1.0);
        let pts: Vec<Pos2> = (0..=24).map(|j| {
            let xx = j as f32 / 24.0;
            pos2(r.left() + 3.0 + xx * (r.width() - 6.0), r.center().y - crate::displays::wt_sample(m, xx) * r.height() * 0.34)
        }).collect();
        let on = i == cur;
        cx.p.add(Shape::line(pts, Stroke::new(if on { 2.0 } else { 1.2 }, if on { k.disp_trace } else { a(k.disp_text, 130) })));
        if on {
            cx.rr_stroke(r, 3.0, 2.0, if k.dir == Dir::C { k.accent } else { k.disp_trace });
        }
    }
    let y = inner.bottom() - 52.0;
    let mut x = inner.left();
    for (i, s) in ["Vowels", "Basic Shapes", "Digital Bells", "Formant Sweep", "Analog Drift"].iter().enumerate() {
        let w = cx.width(s, 12.0, "sans-med") + 26.0;
        chip(cx, Rect::from_min_size(pos2(x, y), vec2(w, 28.0)), s, if i == 0 { 2 } else { 0 }, None);
        x += w + 6.0;
    }
    chip(cx, Rect::from_min_size(pos2(x + 10.0, y), vec2(116.0, 28.0)), "Import…", 0, Some(Ic::Folder));
    cx.text(pos2(inner.left(), y + 44.0), Align2::LEFT_CENTER, "Position is modulated by LFO #7 → Pos CV. The ring on the knob and the lit frame above move together.", 12.0, "sans", k.text2);
}

// ---------------------------------------------------------------- step cable

fn rnd(n: u32) -> f32 {
    let x = (n as f32 * 12.9898).sin() * 43758.547;
    x - x.floor()
}

pub fn step_cable(cx: &Cx, d: &Ctxs, size: egui::Vec2) {
    let k = cx.k;
    let rk = &d.comp;
    let narrow = size.x < 1360.0;
    let insp_w = if narrow { 288.0 } else { 312.0 };
    let ar = areas(k.dir, size, false, insp_w);
    toolbar(cx, ar.toolbar, "Rack", "Composition", true, "", (false, true), 0);
    status(cx, ar.status, (0.6 + 0.15 * (cx.t * 3.0).sin(), 0.58 + 0.12 * (cx.t * 2.5).sin()));
    cx.p.rect_filled(ar.center, 0.0, k.rack);
    let world = Rect::from_min_max(pos2(0.0, 10.0), pos2(1130.0, 730.0)).expand2(vec2(12.0, 20.0));
    let xf = fit(world, ar.center, vec2(8.0, 4.0), 1.0);
    for row in 0..2 {
        let y = 10.0 + row as f32 * 370.0;
        for ry in [y - 16.0, y + 343.0] {
            let r = xf.r(Rect::from_min_max(pos2(world.left(), ry), pos2(world.right(), ry + 13.0)));
            cx.rr(r, 2.0, k.rail);
            let mut x = world.left() + 15.0;
            while x < world.right() - 6.0 {
                cx.p.circle_filled(xf.p(pos2(x, ry + 6.5)), 2.4 * xf.s.max(0.8), k.hole);
                x += 30.0;
            }
        }
    }
    let conn = |_: &str, _: bool| true;
    let none = |_: &mut BTreeMap<String, f32>| {};
    let (_, clock) = put(cx, xf, rk, 1, pos2(8.0, 10.0), &none, false, 0.0, &conn);
    let (seq_r, seq) = put(cx, xf, rk, 6, pos2(176.0, 10.0), &none, false, 0.0, &conn);
    let (_, osc) = put(cx, xf, rk, 10, pos2(850.0, 10.0), &none, false, 0.0, &conn);
    let (_, env) = put(cx, xf, rk, 12, pos2(520.0, 380.0), &|p| { p.insert("attack_ms".into(), 8.0); p.insert("decay_ms".into(), 240.0); }, false, 0.0, &conn);
    let (_, vca) = put(cx, xf, rk, 14, pos2(774.0, 380.0), &|p| { p.insert("gain".into(), 0.85); }, false, 0.0, &conn);
    let (_, out) = put(cx, xf, rk, 39, pos2(968.0, 380.0), &none, false, 0.0, &conn);
    let kj = |m: &BTreeMap<(String, bool), Pos2>, p: &str, o: bool| m[&(p.to_string(), o)];
    let _ = seq_r;
    wire(cx, xf, kj(&clock, "gate", true), kj(&seq, "clock", false), k.gate, 0.9, 40.0);
    wire(cx, xf, kj(&seq, "pitch", true), kj(&osc, "pitch", false), k.cv, 0.9, 50.0);
    wire(cx, xf, kj(&env, "out", true), kj(&vca, "cv", false), k.cv, 0.9, 36.0);
    wire(cx, xf, kj(&vca, "out", true), kj(&out, "left", false), k.audio, 0.9, 36.0);
    // The hero cable: Bass seq gate -> ADSR gate, carrying a pattern and a probability.
    let (p0, p1) = (kj(&seq, "gate", true), kj(&env, "gate", false));
    let pts = cable_pts(xf.p(p0), xf.p(p1), 70.0 * xf.s);
    let steps: [f32; 16] = [1.0, 0.0, 0.6, 1.0, 0.0, 0.8, 0.0, 1.0, 1.0, 0.0, 0.6, 0.0, 1.0, 0.7, 0.0, 0.9];
    let prob = 0.75f32;
    cable(cx, pts.clone(), k.gate, 1.0, true);
    plug(cx, xf.p(p0), k.gate, 1.0);
    plug(cx, xf.p(p1), k.gate, 1.0);
    let mid_t = 0.5;
    let tether_t = 0.28;
    let tp = along(&pts, tether_t);
    // Pulses: one per 16th at 112 bpm; each is gated by the pattern, then thinned by the probability.
    let step_len = 60.0 / 112.0 / 4.0;
    let total = poly_len(&pts);
    let speed = 260.0;
    let ahead = (total / speed) / step_len;
    let now = cx.t / step_len;
    for j in 0..(ahead.ceil() as i32 + 2) {
        let n = (now.floor() as i32 - j).max(0) as u32;
        let age = (now - n as f32) * step_len;
        let dpx = age * speed;
        if dpx > total || age < 0.0 {
            continue;
        }
        let t = dpx / total;
        let s = steps[(n % 16) as usize];
        if s <= 0.0 {
            continue;
        }
        let pass = rnd(n.wrapping_mul(7919)) < prob;
        let q = along(&pts, t);
        let r = 2.6 + 3.0 * s;
        if t < mid_t {
            cx.glow_dot(q, r, Color32::WHITE, 0.7);
        } else if pass {
            cx.glow_dot(q, r, k.gate, 0.9);
            cx.p.circle_filled(q, r * 0.6, Color32::WHITE);
        } else if t < mid_t + 0.07 {
            let f = 1.0 - (t - mid_t) / 0.07;
            cx.p.circle_stroke(q, r + 4.0 * (1.0 - f), Stroke::new(1.2, a(k.text2, (200.0 * f) as u8)));
        }
    }
    // Cartridge hanging on the cable.
    let cw = 132.0;
    let ch = 54.0;
    let cr = Rect::from_center_size(tp + vec2(-cw / 2.0 - 26.0, 8.0), vec2(cw, ch));
    cx.p.line_segment([cr.right_center(), tp], Stroke::new(2.0, k.gate));
    cx.p.circle_filled(tp, 4.0, k.gate);
    cx.shadow(cr, 8.0, 1);
    cx.rr(cr, if k.dir == Dir::C { 3.0 } else { 9.0 }, if k.dir == Dir::C { k.surface } else { k.raised });
    cx.rr_stroke(cr, if k.dir == Dir::C { 3.0 } else { 9.0 }, if k.dir == Dir::C { 2.0 } else { 1.5 }, k.gate);
    let lane = Rect::from_min_size(cr.min + vec2(8.0, 7.0), vec2(cw - 16.0, 22.0));
    let cur = ((cx.t / step_len) as usize) % 16;
    for (i, &step) in steps.iter().enumerate().take(16) {
        let w = lane.width() / 16.0;
        let bh = (step * lane.height()).max(2.0);
        let r = Rect::from_min_max(pos2(lane.left() + i as f32 * w + 0.6, lane.bottom() - bh), pos2(lane.left() + (i as f32 + 1.0) * w - 0.6, lane.bottom()));
        cx.rr(r, 1.0, if steps[i] > 0.0 { a(k.gate, if i == cur { 255 } else { 190 }) } else { a(k.text3, 90) });
    }
    icon(cx.p, Ic::Dice, pos2(cr.left() + 18.0, cr.bottom() - 12.0), 13.0, k.text2);
    cx.text(pos2(cr.left() + 32.0, cr.bottom() - 12.0), Align2::LEFT_CENTER, "75 %", 12.0, "mono-med", k.text);
    cx.text(pos2(cr.right() - 10.0, cr.bottom() - 12.0), Align2::RIGHT_CENTER, "16 steps", 11.0, "sans", k.text2);
    // Callout.
    let note = Rect::from_min_size(xf.p(pos2(24.0, 410.0)), vec2(300.0 * xf.s.max(0.85), 150.0));
    let inner = card(cx, note, "Functional cable", "");
    for (i, l) in ["Pattern: which steps let a pulse through.", "Probability: the chance that it does.", "Both are stored on the cable, so the same", "modules can be re-patched with a new feel."].iter().enumerate() {
        cx.text(pos2(inner.left(), inner.top() + 4.0 + i as f32 * 17.0), Align2::LEFT_TOP, l, 12.0, "sans", k.text);
    }
    // Inspector: cable editor.
    let ins = ar.right;
    panel_bg(cx, ins, false);
    let x0 = ins.left() + 16.0;
    let mut y = ins.top() + 28.0;
    heading(cx, pos2(x0, y), "Cable");
    y += 24.0;
    cx.p.circle_filled(pos2(x0 + 4.0, y), 4.0, k.gate);
    cx.text(pos2(x0 + 16.0, y), Align2::LEFT_CENTER, "Bass seq · gate → ADSR · gate", 13.0, "sans-semi", k.text);
    y += 30.0;
    section_label(cx, pos2(x0, y), "Pattern");
    cx.text(pos2(ins.right() - 16.0, y), Align2::RIGHT_CENTER, "16 steps · ◂ rotate ▸", 11.5, "sans", k.text2);
    y += 18.0;
    let bw = (ins.width() - 32.0 - 7.0 * 5.0) / 8.0;
    for (i, &step) in steps.iter().enumerate().take(16) {
        let (rx, ry) = ((i % 8) as f32, (i / 8) as f32);
        let r = Rect::from_min_size(pos2(x0 + rx * (bw + 5.0), y + ry * 52.0), vec2(bw, 46.0));
        let on = step > 0.0;
        cx.rr(r, k.r_sm + 1.0, k.inset);
        cx.rr_stroke(r, k.r_sm + 1.0, if i == cur { 2.0 } else { 1.0 }, if i == cur { k.accent } else { k.line });
        if on {
            let h = steps[i] * (r.height() - 22.0);
            cx.rr(Rect::from_min_max(pos2(r.left() + 4.0, r.bottom() - 4.0 - h), pos2(r.right() - 4.0, r.bottom() - 4.0)), 2.0, k.gate);
        }
        cx.text(r.left_top() + vec2(5.0, 8.0), Align2::LEFT_CENTER, &format!("{}", i + 1), 10.5, "mono", if on { k.text } else { k.text3 });
    }
    y += 112.0;
    section_label(cx, pos2(x0, y), "Probability");
    cx.text(pos2(ins.right() - 16.0, y), Align2::RIGHT_CENTER, "75 %", 20.0, "mono-med", k.text);
    y += 22.0;
    slider(cx, Rect::from_min_size(pos2(x0, y), vec2(ins.width() - 32.0, 18.0)), 0.75);
    y += 30.0;
    cx.text(pos2(x0, y), Align2::LEFT_CENTER, "Per-step probability", 12.5, "sans", k.text);
    toggle(cx, pos2(ins.right() - 32.0, y), false);
    y += 28.0;
    let tail = [1.0, 0.0, 0.6, 1.0, 0.0, 0.8, 0.0, 1.0f32];
    let _ = tail;
    section_label(cx, pos2(x0, y), "Shape");
    y += 20.0;
    let mut x = x0;
    for (s, ic) in [("Rotate", None), ("Euclid 9/16", None), ("Randomise", Some(Ic::Dice)), ("Clear", None)] {
        let w = cx.width(s, 12.0, "sans-med") + 24.0 + if ic.is_some() { 20.0 } else { 0.0 };
        if x + w > ins.right() - 12.0 {
            x = x0;
            y += 34.0;
        }
        chip(cx, Rect::from_min_size(pos2(x, y - 13.0), vec2(w, 28.0)), s, 0, ic);
        x += w + 6.0;
    }
    y += 40.0;
    section_label(cx, pos2(x0, y), "Result");
    y += 20.0;
    cx.text(pos2(x0, y), Align2::LEFT_CENTER, "9 of 16 steps are on. About 7 trigger the", 12.5, "sans", k.text);
    cx.text(pos2(x0, y + 17.0), Align2::LEFT_CENTER, "envelope per bar at 75 %.", 12.5, "sans", k.text);
    y += 44.0;
    button(cx, Rect::from_min_size(pos2(x0, y - 14.0), vec2(130.0, 34.0)), "Preview", Some(Ic::Play), false);
    chip(cx, Rect::from_min_size(pos2(x0 + 140.0, y - 13.0), vec2(120.0, 32.0)), "Remove pattern", 0, None);
}

// ---------------------------------------------------------------- components

pub fn components(cx: &Cx, size: egui::Vec2) {
    let k = cx.k;
    cx.p.rect_filled(Rect::from_min_size(pos2(0.0, 0.0), size), 0.0, k.bg);
    cx.p.rect_filled(Rect::from_min_size(pos2(0.0, 0.0), vec2(size.x, 52.0)), 0.0, k.surface);
    cx.line(pos2(0.0, 52.0), pos2(size.x, 52.0), if k.dir == Dir::C { 1.5 } else { 1.0 }, if k.dir == Dir::C { k.line2 } else { k.line });
    heading(cx, pos2(24.0, 27.0), "Components replacing stock egui widgets");
    cx.text(pos2(size.x - 24.0, 27.0), Align2::RIGHT_CENTER, k.dir.title(), 13.0, "sans-med", k.text2);
    let m = 18.0;
    let gap = 14.0;
    let cw = (size.x - m * 2.0 - gap * 2.0) / 3.0;
    let top = 70.0;
    let col = |i: usize| m + i as f32 * (cw + gap);
    // Column 1: buttons.
    let r1 = Rect::from_min_size(pos2(col(0), top), vec2(cw, 310.0));
    let inner = card(cx, r1, "Buttons · toggles · selection", "");
    let mut y = inner.top() + 10.0;
    button(cx, Rect::from_min_size(pos2(inner.left(), y), vec2(112.0, 34.0)), "Play C4", Some(Ic::Play), false);
    button(cx, Rect::from_min_size(pos2(inner.left() + 122.0, y), vec2(112.0, 34.0)), "Pressed", Some(Ic::Play), true);
    y += 48.0;
    for (i, (l, s)) in [("Idle", 0u8), ("Hover", 1), ("Selected", 2)].iter().enumerate() {
        chip(cx, Rect::from_min_size(pos2(inner.left() + i as f32 * 96.0, y), vec2(88.0, 30.0)), l, *s, None);
    }
    y += 46.0;
    for (i, (ic, s, en)) in [(Ic::Undo, 0u8, true), (Ic::Redo, 0, false), (Ic::Cable, 1, true), (Ic::Panel, 2, true), (Ic::Gear, 0, true)].iter().enumerate() {
        icon_btn(cx, pos2(inner.left() + 16.0 + i as f32 * 40.0, y + 14.0), *ic, *s, *en);
    }
    y += 48.0;
    segmented(cx, Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), 32.0)), &["POLY", "MONO", "LEGATO"], 0, &Face { base: k.inset, ink: k.text, ink2: k.text2 }, 12.0);
    y += 46.0;
    toggle(cx, pos2(inner.left() + 20.0, y + 10.0), true);
    toggle(cx, pos2(inner.left() + 66.0, y + 10.0), false);
    cx.text(pos2(inner.left() + 96.0, y + 10.0), Align2::LEFT_CENTER, "Cue launches on next bar", 12.5, "sans", k.text);
    // Column 2: inputs and lists.
    let r2 = Rect::from_min_size(pos2(col(1), top), vec2(cw, 310.0));
    let inner = card(cx, r2, "Inputs · lists · tags", "");
    let mut y = inner.top() + 6.0;
    field(cx, Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), 34.0)), "Search sounds", "", false);
    y += 42.0;
    field(cx, Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), 34.0)), "", "pad", true);
    y += 46.0;
    for (i, (n, sel)) in [("Breath", 0), ("Evolving Pad", 2), ("Glass Keys", 1)].iter().enumerate() {
        let rr = Rect::from_min_size(pos2(inner.left(), y + i as f32 * 34.0), vec2(inner.width(), 32.0));
        match (k.dir, sel) {
            (Dir::C, 2) => {
                cx.p.rect_filled(rr, 0.0, k.text);
            }
            (_, 2) => cx.rr(rr, k.r_md, k.accent_soft),
            (_, 1) => cx.rr(rr, k.r_md, k.inset),
            _ => {}
        }
        cx.text(rr.left_center() + vec2(12.0, 0.0), Align2::LEFT_CENTER, n, 13.0, if k.dir == Dir::C { "cond-semi" } else { "sans-semi" }, if k.dir == Dir::C && *sel == 2 { k.surface } else { k.text });
    }
    y += 112.0;
    let mut x = inner.left();
    for (s, c) in [("Keys", k.text2), ("Pad", k.text2), ("Slow", k.text2), ("Factory", k.cv)] {
        x += tag(cx, pos2(x, y), s, c) + 5.0;
    }
    slider(cx, Rect::from_min_size(pos2(inner.left(), y + 38.0), vec2(inner.width() - 50.0, 18.0)), 0.7);
    cx.text(pos2(inner.right(), y + 47.0), Align2::RIGHT_CENTER, "90", 13.0, "mono-med", k.text);
    // Column 3: chrome.
    let r3 = Rect::from_min_size(pos2(col(2), top), vec2(cw, 310.0));
    let inner = card(cx, r3, "Knob and jack states", "");
    let face = k.face("filter.svf");
    let fb = Rect::from_min_size(inner.min + vec2(-6.0, 4.0), vec2(inner.width() + 12.0, 128.0));
    cx.rr(fb, k.r_md, face.base);
    for (i, (n, hot)) in [("Idle", false), ("Focus", true), ("Modulated", false)].iter().enumerate() {
        let c = pos2(fb.left() + fb.width() * (i as f32 + 0.5) / 3.0, fb.center().y - 6.0);
        let mv = (i == 2).then(|| ModVis { lo: 0.25, hi: 0.8, cur: 0.25 + 0.55 * (0.5 + 0.5 * (cx.t * 2.0).sin()) });
        knob(cx, c, 19.0, 0.55, mv, &face, *hot);
        cx.text(c + vec2(0.0, 44.0), Align2::CENTER_CENTER, n, 11.0, lab_font(k.dir), face.ink);
    }
    let jy = fb.bottom() + 36.0;
    for (i, (n, conn)) in [("Free", false), ("Patched", true), ("Selected", true)].iter().enumerate() {
        let c = pos2(inner.left() + 24.0 + i as f32 * 92.0, jy);
        jack(cx, c, 11.0, [k.audio, k.cv, k.gate][i], *conn, &face);
        cx.text(c + vec2(0.0, 24.0), Align2::CENTER_CENTER, n, 11.0, "sans", k.text2);
    }
    // Bottom row: status bar and popover, toast, context menu.
    let by = top + 310.0 + gap;
    let sb = Rect::from_min_size(pos2(m, size.y - 30.0 - 14.0), vec2(size.x - m * 2.0, 30.0));
    cx.rr(sb, 0.0, k.surface);
    status(cx, sb, (0.6, 0.5));
    let _ = by;
    let pop = diagnostics(cx, pos2(sb.right() - 20.0, sb.top() - 8.0));
    // Toast.
    let toast = Rect::from_min_size(pos2(m, by + 8.0), vec2(330.0, 48.0));
    cx.shadow(toast, k.r_md, 2);
    cx.rr(toast, k.r_md, if k.dark { k.raised } else { k.text });
    icon(cx.p, Ic::Check, toast.left_center() + vec2(22.0, 0.0), 16.0, k.good);
    cx.text(toast.left_center() + vec2(42.0, 0.0), Align2::LEFT_CENTER, "Saved “Evolving Pad” to Your Sounds", 13.0, "sans-med", if k.dark { k.text } else { k.surface });
    cx.text(toast.right_center() - vec2(14.0, 0.0), Align2::RIGHT_CENTER, "Undo", 13.0, "sans-semi", if k.dark { k.accent } else { k.accent_soft });
    // Context menu.
    let menu = Rect::from_min_size(pos2(m + 360.0, by + 8.0), vec2(220.0, 150.0));
    cx.shadow(menu, k.r_md, 2);
    cx.rr(menu, k.r_md, k.raised);
    cx.rr_stroke(menu, k.r_md, if k.dir == Dir::C { 1.5 } else { 1.0 }, if k.dir == Dir::C { k.line2 } else { k.line });
    for (i, (t, sc)) in [("Edit face…", "E"), ("Duplicate", "⌘D"), ("Replace with…", ""), ("Disconnect all", ""), ("Delete module", "⌫")].iter().enumerate() {
        let r = Rect::from_min_size(menu.min + vec2(6.0, 6.0 + i as f32 * 27.0), vec2(menu.width() - 12.0, 26.0));
        if i == 1 {
            cx.rr(r, k.r_sm, k.accent_soft);
        }
        cx.text(r.left_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, t, 13.0, "sans", if i == 4 { k.bad } else { k.text });
        cx.text(r.right_center() - vec2(10.0, 0.0), Align2::RIGHT_CENTER, sc, 12.0, "mono", k.text2);
    }
    // Tooltip.
    let tip = Rect::from_min_size(pos2(m + 610.0, by + 8.0), vec2(260.0, 72.0));
    cx.shadow(tip, k.r_sm, 1);
    cx.rr(tip, k.r_sm + 2.0, if k.dark { k.raised } else { k.text });
    let ti = if k.dark { k.text } else { k.surface };
    cx.text(tip.min + vec2(12.0, 18.0), Align2::LEFT_CENTER, "Cutoff · 1.20 kHz", 13.0, "sans-semi", ti);
    cx.text(tip.min + vec2(12.0, 38.0), Align2::LEFT_CENTER, "Moved by LFO #7 (±0.75 oct) and", 12.0, "sans", a(ti, 220));
    cx.text(tip.min + vec2(12.0, 55.0), Align2::LEFT_CENTER, "Macro Energy. Double-click to reset.", 12.0, "sans", a(ti, 220));
    let _ = pop;
    // Dialog (replaces egui::Window), dropdown (replaces ComboBox), empty state.
    let dy = by + 176.0;
    let dlg = Rect::from_min_size(pos2(m, dy), vec2(400.0, 176.0));
    cx.shadow(dlg, k.r_lg, 2);
    cx.rr(dlg, k.r_lg, k.raised);
    cx.rr_stroke(dlg, k.r_lg, if k.dir == Dir::C { 1.6 } else { 1.0 }, if k.dir == Dir::C { k.line2 } else { k.line });
    cx.text(dlg.min + vec2(22.0, 30.0), Align2::LEFT_CENTER, "Save changes to “Evolving Pad”?", 17.0, if k.dir == Dir::C { "cond-semi" } else { "sans-semi" }, k.text);
    cx.text(dlg.min + vec2(22.0, 58.0), Align2::LEFT_TOP, "Your edits are kept until you quit. The factory", 13.0, "sans", k.text2);
    cx.text(dlg.min + vec2(22.0, 76.0), Align2::LEFT_TOP, "original is never changed; this saves a copy.", 13.0, "sans", k.text2);
    button(cx, Rect::from_min_size(dlg.min + vec2(22.0, 122.0), vec2(120.0, 36.0)), "Save copy", None, false);
    chip(cx, Rect::from_min_size(dlg.min + vec2(152.0, 123.0), vec2(104.0, 34.0)), "Discard", 0, None);
    chip(cx, Rect::from_min_size(dlg.min + vec2(266.0, 123.0), vec2(104.0, 34.0)), "Cancel", 0, None);
    let dd = Rect::from_min_size(pos2(m + 430.0, dy), vec2(250.0, 34.0));
    cx.rr(dd, k.r_sm + 1.0, k.inset);
    cx.rr_stroke(dd, k.r_sm + 1.0, 1.0, k.line);
    cx.text(dd.left_center() + vec2(12.0, 0.0), Align2::LEFT_CENTER, "Output · System default", 13.0, "sans", k.text);
    icon(cx.p, Ic::Down, dd.right_center() - vec2(18.0, 0.0), 14.0, k.text2);
    let ml = Rect::from_min_size(dd.left_bottom() + vec2(0.0, 6.0), vec2(250.0, 118.0));
    cx.shadow(ml, k.r_md, 2);
    cx.rr(ml, k.r_md, k.raised);
    cx.rr_stroke(ml, k.r_md, if k.dir == Dir::C { 1.5 } else { 1.0 }, if k.dir == Dir::C { k.line2 } else { k.line });
    for (i, (t, on)) in [("System default", true), ("USB Audio 2.0 · 2 out", false), ("HDMI · Display 1", false)].iter().enumerate() {
        let r = Rect::from_min_size(ml.min + vec2(6.0, 6.0 + i as f32 * 34.0), vec2(ml.width() - 12.0, 32.0));
        if i == 1 {
            cx.rr(r, k.r_sm, k.inset);
        }
        if *on {
            icon(cx.p, Ic::Check, r.left_center() + vec2(14.0, 0.0), 14.0, k.accent);
        }
        cx.text(r.left_center() + vec2(32.0, 0.0), Align2::LEFT_CENTER, t, 13.0, "sans", k.text);
    }
    let es = Rect::from_min_size(pos2(m + 700.0, dy), vec2(240.0, 176.0));
    let ei = card(cx, es, "Empty state", "");
    icon(cx.p, Ic::Search, pos2(es.center().x, ei.top() + 22.0), 26.0, k.text3);
    cx.text(pos2(es.center().x, ei.top() + 58.0), Align2::CENTER_CENTER, "No sounds match “glass pad”", 13.0, "sans-semi", k.text);
    cx.text(pos2(es.center().x, ei.top() + 78.0), Align2::CENTER_CENTER, "Try a category, or clear the search.", 12.0, "sans", k.text2);
    chip(cx, Rect::from_center_size(pos2(es.center().x, ei.top() + 108.0), vec2(120.0, 28.0)), "Clear search", 0, Some(Ic::Close));
}
