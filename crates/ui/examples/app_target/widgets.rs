//! Controls the three directions draw differently: knobs, jacks, cables, selectors, buttons.
use crate::prim::*;
use crate::tokens::{Dir, Face};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Shape, Stroke};
use std::f32::consts::PI;

/// Modulation as the knob shows it: the swept range and the live position (0..1 of travel).
#[derive(Clone, Copy)]
pub struct ModVis {
    pub lo: f32,
    pub hi: f32,
    pub cur: f32,
}

pub fn knob(cx: &Cx, c: Pos2, r: f32, n: f32, m: Option<ModVis>, f: &Face, hot: bool) {
    let k = cx.k;
    let a0 = knob_angle(0.0);
    let a1 = knob_angle(1.0);
    let dir = pol(Pos2::ZERO, 1.0, knob_angle(n)).to_vec2();
    match k.dir {
        Dir::A => {
            for i in 0..=10 {
                let ang = knob_angle(i as f32 / 10.0);
                cx.line(pol(c, r + 4.5, ang), pol(c, r + 7.5, ang), 1.2, a(f.ink2, 170));
            }
            if let Some(m) = m {
                cx.arc(c, r + 11.5, knob_angle(m.lo), knob_angle(m.hi), 3.0, a(k.cv, 210));
                let d = pol(c, r + 11.5, knob_angle(m.cur));
                cx.p.circle_filled(d, 3.6, Color32::WHITE);
                cx.p.circle_stroke(d, 3.6, Stroke::new(1.5, k.cv));
            }
            cx.p.circle_filled(c + vec2(1.0, 3.0), r * 1.02, Color32::from_black_alpha(if k.dark { 110 } else { 70 }));
            cx.p.circle_filled(c, r, mix(k.knob, Color32::BLACK, 0.25));
            for i in 0..28 {
                let ang = i as f32 / 28.0 * 2.0 * PI;
                cx.line(pol(c, r * 0.9, ang), pol(c, r, ang), 1.0, a(k.knob_hi, 90));
            }
            cx.p.circle_filled(c, r * 0.8, k.knob);
            cx.p.circle_filled(c + vec2(-r * 0.18, -r * 0.24), r * 0.5, Color32::from_white_alpha(14));
            cx.p.circle_stroke(c, r * 0.8, Stroke::new(1.0, a(k.knob_hi, 140)));
            cx.line(c + dir * r * 0.25, c + dir * r * 0.74, 2.6, k.knob_ink);
            if hot {
                cx.p.circle_stroke(c, r + 14.5, Stroke::new(1.5, k.focus));
            }
        }
        Dir::B => {
            let tr = r + 5.5;
            cx.arc(c, tr, a0, a1, 3.6, a(f.ink, 45));
            cx.glow_arc(c, tr, a0, knob_angle(n), 3.6, k.accent, 0.8);
            if let Some(m) = m {
                cx.arc(c, tr + 6.0, a0, a1, 2.0, a(f.ink, 28));
                cx.glow_arc(c, tr + 6.0, knob_angle(m.lo), knob_angle(m.hi), 2.6, k.cv, 1.0);
                cx.glow_dot(pol(c, tr + 6.0, knob_angle(m.cur)), 2.8, Color32::WHITE, 0.8);
            }
            cx.p.circle_filled(c + vec2(0.0, 2.5), r * 0.98, Color32::from_black_alpha(90));
            cx.grad_circle(c, r, mix(k.knob, Color32::WHITE, 0.10), mix(k.knob, Color32::BLACK, 0.35));
            cx.arc(c, r - 1.0, PI * 1.1, PI * 1.9, 1.4, Color32::from_white_alpha(70));
            cx.p.circle_stroke(c, r, Stroke::new(1.0, a(k.knob_hi, 160)));
            cx.p.circle_filled(c + dir * r * 0.66, 2.4, k.knob_ink);
            if hot {
                cx.p.circle_stroke(c, r + 17.0, Stroke::new(1.5, k.focus));
            }
        }
        Dir::C => {
            for i in 0..=10 {
                let ang = knob_angle(i as f32 / 10.0);
                let long = i % 5 == 0;
                cx.line(pol(c, r + 4.0, ang), pol(c, r + if long { 9.0 } else { 7.0 }, ang), 1.4, f.ink);
            }
            if let Some(m) = m {
                cx.arc(c, r + 13.5, knob_angle(m.lo), knob_angle(m.hi), 4.0, k.accent);
                let d = pol(c, r + 13.5, knob_angle(m.cur));
                cx.p.circle_filled(d, 4.2, f.ink);
                cx.p.circle_filled(d, 2.0, Color32::WHITE);
            }
            cx.p.circle_filled(c, r, k.knob);
            cx.p.circle_stroke(c, r, Stroke::new(if k.dark { 2.0 } else { 0.0 }, k.knob_hi));
            cx.line(c + dir * r * 0.18, c + dir * r * 0.96, 3.4, k.knob_ink);
            if hot {
                cx.p.circle_stroke(c, r + 19.0, Stroke::new(2.0, k.focus));
            }
        }
    }
}

impl Cx<'_> {
    pub fn glow_arc(&self, c: Pos2, r: f32, a0: f32, a1: f32, w: f32, col: Color32, strength: f32) {
        if (a1 - a0).abs() < 1e-3 {
            return;
        }
        self.glow_line(arc_pts(c, r, a0, a1), w, col, strength);
    }

    pub fn grad_circle(&self, c: Pos2, r: f32, top: Color32, bot: Color32) {
        let mut m = egui::epaint::Mesh::default();
        m.colored_vertex(c, mix(top, bot, 0.5));
        let n = 36;
        for i in 0..n {
            let ang = i as f32 / n as f32 * 2.0 * PI;
            let q = c + vec2(ang.cos(), ang.sin()) * r;
            m.colored_vertex(q, mix(top, bot, ((q.y - (c.y - r)) / (2.0 * r)).clamp(0.0, 1.0)));
        }
        for i in 0..n as u32 {
            m.add_triangle(0, 1 + i, 1 + (i + 1) % n as u32);
        }
        self.p.add(Shape::mesh(m));
    }
}

pub fn jack(cx: &Cx, c: Pos2, r: f32, col: Color32, connected: bool, f: &Face) {
    let k = cx.k;
    match k.dir {
        Dir::A => {
            cx.p.circle_filled(c + vec2(0.0, 1.5), r + 1.5, Color32::from_black_alpha(50));
            cx.p.circle_filled(c, r + 1.0, mix(f.base, Color32::BLACK, 0.45));
            cx.p.circle_stroke(c, r + 1.0, Stroke::new(1.5, a(Color32::WHITE, if k.dark { 60 } else { 120 })));
            cx.p.circle_filled(c, r * 0.72, k.hole);
            cx.arc(c, r * 0.72, PI * 1.05, PI * 1.95, 2.0, Color32::from_black_alpha(160));
            if connected {
                cx.p.circle_filled(c, r * 0.36, col);
            }
        }
        Dir::B => {
            cx.p.circle_filled(c, r + 2.5, mix(f.base, Color32::BLACK, 0.5));
            if connected {
                cx.glow_arc(c, r + 0.5, 0.0, 2.0 * PI - 0.01, 2.0, col, 0.9);
            } else {
                cx.p.circle_stroke(c, r + 0.5, Stroke::new(1.5, a(f.ink, 90)));
            }
            cx.p.circle_filled(c, r * 0.68, k.hole);
            if connected {
                cx.p.circle_filled(c, r * 0.30, col);
            }
        }
        Dir::C => {
            cx.p.circle_filled(c, r + 1.5, f.ink);
            cx.p.circle_filled(c, r * 0.62, if connected { col } else { f.base });
            cx.p.circle_stroke(c, r * 0.62, Stroke::new(1.5, f.ink));
        }
    }
}

/// A cable with the direction's own weight and finish.
pub fn cable(cx: &Cx, pts: Vec<Pos2>, col: Color32, alpha: f32, emph: bool) {
    let k = cx.k;
    let al = |c: Color32| a(c, (c.a() as f32 * alpha) as u8);
    match k.dir {
        Dir::A => {
            let off: Vec<Pos2> = pts.iter().map(|q| *q + vec2(0.0, 2.5)).collect();
            cx.p.add(Shape::line(off, Stroke::new(5.0, al(Color32::from_black_alpha(if k.dark { 120 } else { 55 })))));
            cx.p.add(Shape::line(pts.clone(), Stroke::new(if emph { 5.6 } else { 4.6 }, al(mix(col, Color32::BLACK, 0.12)))));
            let hi: Vec<Pos2> = pts.iter().map(|q| *q + vec2(0.0, -1.2)).collect();
            cx.p.add(Shape::line(hi, Stroke::new(1.3, al(a(Color32::WHITE, 90)))));
        }
        Dir::B => {
            if !k.dark {
                cx.p.add(Shape::line(pts.clone(), Stroke::new(5.0, al(Color32::from_black_alpha(90)))));
            }
            if emph {
                cx.glow_line(pts.clone(), 3.2, al(col), 1.0);
            } else {
                cx.p.add(Shape::line(pts.clone(), Stroke::new(3.2, al(col))));
                cx.p.add(Shape::line(pts, Stroke::new(1.0, al(a(Color32::WHITE, 70)))));
            }
        }
        Dir::C => {
            let edge = if k.dark { Color32::BLACK } else { k.line2 };
            cx.p.add(Shape::line(pts.clone(), Stroke::new(6.4, al(edge))));
            cx.p.add(Shape::line(pts, Stroke::new(4.0, al(col))));
        }
    }
}

pub fn plug(cx: &Cx, c: Pos2, col: Color32, alpha: f32) {
    let k = cx.k;
    let al = |c: Color32| a(c, (c.a() as f32 * alpha) as u8);
    match k.dir {
        Dir::A => {
            cx.p.circle_filled(c + vec2(0.0, 1.5), 9.0, al(Color32::from_black_alpha(70)));
            cx.p.circle_filled(c, 8.5, al(mix(col, Color32::BLACK, 0.3)));
            cx.p.circle_stroke(c, 8.5, Stroke::new(1.2, al(a(Color32::WHITE, 120))));
            cx.p.circle_filled(c, 4.0, al(mix(col, Color32::WHITE, 0.25)));
        }
        Dir::B => {
            cx.p.circle_filled(c, 8.0, al(mix(col, Color32::BLACK, 0.55)));
            cx.p.circle_stroke(c, 8.0, Stroke::new(2.0, al(col)));
            cx.p.circle_filled(c, 3.0, al(Color32::WHITE));
        }
        Dir::C => {
            cx.p.circle_filled(c, 9.0, al(Color32::BLACK));
            cx.p.circle_filled(c, 6.0, al(col));
        }
    }
}

/// Segmented selector. `sel` indexes `opts`.
pub fn segmented(cx: &Cx, r: Rect, opts: &[&str], sel: usize, f: &Face, size: f32) {
    let k = cx.k;
    let (track, fill, on_ink, off_ink, rad) = match k.dir {
        Dir::A => (mix(f.base, Color32::BLACK, 0.16), k.knob, k.knob_ink, f.ink, 4.0),
        Dir::B => (mix(f.base, Color32::BLACK, 0.45), k.accent, k.on_accent, f.ink, 8.0),
        Dir::C => (f.base, f.ink, f.base, f.ink, 2.0),
    };
    cx.rr(r, rad, track);
    if k.dir == Dir::C {
        cx.rr_stroke(r, rad, 1.6, f.ink);
    }
    let wts: Vec<f32> = opts.iter().map(|o| o.chars().count() as f32 + 2.4).collect();
    let tot: f32 = wts.iter().sum();
    let mut x = r.left();
    for (i, o) in opts.iter().enumerate() {
        let w = r.width() * wts[i] / tot;
        let seg = Rect::from_min_size(pos2(x, r.top()), vec2(w, r.height()));
        x += w;
        let on = i == sel;
        if on {
            let s = seg.shrink2(vec2(1.5, 2.0));
            cx.rr(s, (rad - 1.0).max(1.0), fill);
        }
        let fam = if k.dir == Dir::C { "cond-semi" } else { "sans-semi" };
        cx.text(seg.center(), Align2::CENTER_CENTER, o, size, fam, if on { on_ink } else { off_ink });
    }
}

/// Plain toggle chip used in chrome. `state`: 0 idle, 1 hover, 2 active/selected.
pub fn chip(cx: &Cx, r: Rect, label: &str, state: u8, ic: Option<Ic>) {
    let k = cx.k;
    let (bg, fg, brd) = match (k.dir, state) {
        (Dir::C, 2) => (k.text, k.surface, k.text),
        (Dir::C, 1) => (k.inset, k.text, k.line2),
        (Dir::C, _) => (k.surface, k.text, k.line),
        (_, 2) => (k.accent_soft, if k.dark { k.text } else { k.accent }, a(k.accent, 160)),
        (_, 1) => (k.raised, k.text, k.line2),
        _ => (a(k.raised, 0), k.text2, k.line),
    };
    cx.rr(r, if k.dir == Dir::B { r.height() / 2.0 } else { k.r_sm + 1.0 }, bg);
    cx.rr_stroke(r, if k.dir == Dir::B { r.height() / 2.0 } else { k.r_sm + 1.0 }, 1.0, brd);
    let mut x = r.center().x;
    let fam = if k.dir == Dir::C { "cond-semi" } else { "sans-med" };
    let lw = cx.width(label, 12.0, fam);
    let iw = if ic.is_some() { 16.0 + if label.is_empty() { 0.0 } else { 5.0 } } else { 0.0 };
    x -= (lw + iw) / 2.0;
    if let Some(ic) = ic {
        icon(cx.p, ic, pos2(x + 8.0, r.center().y), 14.0, fg);
        x += iw;
    }
    if !label.is_empty() {
        cx.text(pos2(x, r.center().y), Align2::LEFT_CENTER, label, 12.0, fam, fg);
    }
}

/// Primary call-to-action button.
pub fn button(cx: &Cx, r: Rect, label: &str, ic: Option<Ic>, pressed: bool) {
    let k = cx.k;
    let rad = if k.dir == Dir::B { r.height() / 2.0 } else { k.r_sm + 1.0 };
    if k.dir == Dir::B && !pressed {
        cx.p.rect_filled(r.expand(3.0), egui::CornerRadius::same(255), a(k.accent, 40));
    }
    cx.rr(r, rad, if pressed { mix(k.accent, Color32::BLACK, 0.18) } else { k.accent });
    let fam = if k.dir == Dir::C { "cond-semi" } else { "sans-semi" };
    let lw = cx.width(label, 13.0, fam);
    let iw = if ic.is_some() { 20.0 } else { 0.0 };
    let mut x = r.center().x - (lw + iw) / 2.0;
    if let Some(ic) = ic {
        icon(cx.p, ic, pos2(x + 7.0, r.center().y), 14.0, k.on_accent);
        x += iw;
    }
    cx.text(pos2(x, r.center().y), Align2::LEFT_CENTER, label, 13.0, fam, k.on_accent);
}

pub fn icon_btn(cx: &Cx, c: Pos2, ic: Ic, state: u8, enabled: bool) {
    let k = cx.k;
    let r = Rect::from_center_size(c, vec2(30.0, 30.0));
    if state > 0 {
        cx.rr(r, k.r_sm + 1.0, if state == 2 { k.accent_soft } else { k.inset });
    }
    let col = if !enabled { k.text3 } else if state == 2 { if k.dark || k.dir == Dir::C { k.text } else { k.accent } } else { k.text2 };
    icon(cx.p, ic, c, 16.0, col);
}

/// Text field with leading icon.
pub fn field(cx: &Cx, r: Rect, ph: &str, val: &str, focused: bool) {
    let k = cx.k;
    cx.rr(r, k.r_sm + 1.0, k.inset);
    cx.rr_stroke(r, k.r_sm + 1.0, if focused { 2.0 } else { 1.0 }, if focused { k.focus } else { k.line });
    icon(cx.p, Ic::Search, pos2(r.left() + 16.0, r.center().y), 14.0, k.text2);
    let fam = if k.dir == Dir::C { "sans" } else { "sans" };
    if val.is_empty() {
        cx.text(pos2(r.left() + 30.0, r.center().y), Align2::LEFT_CENTER, ph, 13.0, fam, k.text3);
    } else {
        cx.text(pos2(r.left() + 30.0, r.center().y), Align2::LEFT_CENTER, val, 13.0, fam, k.text);
    }
}

/// Horizontal slider with value.
pub fn slider(cx: &Cx, r: Rect, n: f32) {
    let k = cx.k;
    let y = r.center().y;
    let th = if k.dir == Dir::C { 4.0 } else { 5.0 };
    let tr = Rect::from_min_max(pos2(r.left(), y - th / 2.0), pos2(r.right(), y + th / 2.0));
    cx.rr(tr, th / 2.0, k.inset);
    cx.rr_stroke(tr, th / 2.0, 1.0, k.line);
    let x = r.left() + r.width() * n;
    cx.rr(Rect::from_min_max(tr.min, pos2(x, tr.max.y)), th / 2.0, k.accent);
    let h = pos2(x, y);
    match k.dir {
        Dir::C => {
            cx.p.rect_filled(Rect::from_center_size(h, vec2(8.0, 18.0)), 1.0, k.text);
        }
        _ => {
            cx.p.circle_filled(h + vec2(0.0, 1.5), 8.0, Color32::from_black_alpha(50));
            cx.p.circle_filled(h, 8.0, k.raised);
            cx.p.circle_stroke(h, 8.0, Stroke::new(1.5, k.accent));
        }
    }
}

pub fn toggle(cx: &Cx, c: Pos2, on: bool) {
    let k = cx.k;
    let r = Rect::from_center_size(c, vec2(32.0, 18.0));
    let rad = if k.dir == Dir::C { 3.0 } else { 9.0 };
    cx.rr(r, rad, if on { k.accent } else { k.inset });
    cx.rr_stroke(r, rad, 1.0, if on { k.accent } else { k.line2 });
    let x = if on { r.right() - 9.0 } else { r.left() + 9.0 };
    cx.p.circle_filled(pos2(x, c.y), 6.0, if on { k.on_accent } else { k.text2 });
}

/// Level meter, vertical, `n` in 0..1.
pub fn meter(cx: &Cx, r: Rect, n: f32, horizontal: bool) {
    let k = cx.k;
    let segs = if horizontal { (r.width() / 4.0) as usize } else { (r.height() / 4.0) as usize };
    for i in 0..segs {
        let f = i as f32 / segs as f32;
        let lit = f < n;
        let col = if f > 0.86 { k.bad } else if f > 0.68 { k.warn } else { k.good };
        let seg = if horizontal {
            Rect::from_min_size(pos2(r.left() + i as f32 * 4.0, r.top()), vec2(3.0, r.height()))
        } else {
            Rect::from_min_size(pos2(r.left(), r.bottom() - (i as f32 + 1.0) * 4.0 + 1.0), vec2(r.width(), 3.0))
        };
        cx.p.rect_filled(seg, 0.5, if lit { col } else { a(k.text3, 70) });
    }
}

/// Small rounded tag.
pub fn tag(cx: &Cx, pos: Pos2, s: &str, col: Color32) -> f32 {
    let k = cx.k;
    let fam = if k.dir == Dir::C { "cond-med" } else { "sans-med" };
    let w = cx.width(s, 11.0, fam) + 12.0;
    let r = Rect::from_min_size(pos, vec2(w, 18.0));
    cx.rr(r, if k.dir == Dir::B { 9.0 } else { 3.0 }, a(col, if k.dark { 50 } else { 38 }));
    cx.text(r.center(), Align2::CENTER_CENTER, s, 11.0, fam, if k.dark { mix(col, Color32::WHITE, 0.45) } else { mix(col, Color32::BLACK, 0.45) });
    w
}
