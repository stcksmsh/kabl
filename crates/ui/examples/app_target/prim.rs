//! Painter primitives shared by every direction: fonts, text, gradients, arcs, glow, icons.
use crate::tokens::Tok;
use egui::epaint::{Mesh, Shadow as EShadow};
use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId,
    Painter, Pos2, Rect, Shape, Stroke, StrokeKind,
};
use std::f32::consts::{PI, TAU};

/// Everything a scene needs to draw one frame.
pub struct Cx<'a> {
    pub p: &'a Painter,
    pub k: &'a Tok,
    /// Seconds. Fixed per still, advancing per frame in a recording.
    pub t: f32,
}

pub fn fam(name: &str) -> FontFamily {
    match name {
        "sans" => FontFamily::Proportional,
        "mono" => FontFamily::Monospace,
        n => FontFamily::Name(n.into()),
    }
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fd = FontDefinitions::default();
    let faces: [(&str, &[u8]); 7] = [
        ("sans", include_bytes!("fonts/IBMPlexSans-Regular.ttf")),
        ("sans-med", include_bytes!("fonts/IBMPlexSans-Medium.ttf")),
        (
            "sans-semi",
            include_bytes!("fonts/IBMPlexSans-SemiBold.ttf"),
        ),
        (
            "cond-semi",
            include_bytes!("fonts/IBMPlexSansCondensed-SemiBold.ttf"),
        ),
        (
            "cond-med",
            include_bytes!("fonts/IBMPlexSansCondensed-Medium.ttf"),
        ),
        ("mono", include_bytes!("fonts/IBMPlexMono-Regular.ttf")),
        ("mono-med", include_bytes!("fonts/IBMPlexMono-Medium.ttf")),
    ];
    for (name, bytes) in faces {
        fd.font_data.insert(
            name.into(),
            std::sync::Arc::new(FontData::from_static(bytes)),
        );
        let family = match name {
            "sans" => FontFamily::Proportional,
            "mono" => FontFamily::Monospace,
            n => FontFamily::Name(n.into()),
        };
        fd.families
            .entry(family)
            .or_default()
            .insert(0, name.into());
    }
    ctx.set_fonts(fd);
}

pub fn a(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

pub fn mix(x: Color32, y: Color32, t: f32) -> Color32 {
    x.lerp_to_gamma(y, t.clamp(0.0, 1.0))
}

pub fn lum(c: Color32) -> f32 {
    let f = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
}

pub fn contrast(x: Color32, y: Color32) -> f32 {
    let (l1, l2) = (lum(x), lum(y));
    (l1.max(l2) + 0.05) / (l1.min(l2) + 0.05)
}

pub fn tri(t: f32) -> f32 {
    (t - t.floor() - 0.5).abs() * 4.0 - 1.0
}

impl Cx<'_> {
    pub fn text(&self, pos: Pos2, al: Align2, s: &str, size: f32, f: &str, c: Color32) -> Rect {
        self.p.text(pos, al, s, FontId::new(size, fam(f)), c)
    }

    pub fn width(&self, s: &str, size: f32, f: &str) -> f32 {
        self.p
            .layout_no_wrap(s.to_string(), FontId::new(size, fam(f)), Color32::WHITE)
            .size()
            .x
    }

    /// Text clipped with an ellipsis to `maxw`.
    pub fn fit(&self, s: &str, size: f32, f: &str, maxw: f32) -> String {
        if self.width(s, size, f) <= maxw {
            return s.to_string();
        }
        let mut out: Vec<char> = s.chars().collect();
        while !out.is_empty() {
            out.pop();
            let t: String = out.iter().collect::<String>() + "…";
            if self.width(&t, size, f) <= maxw {
                return t;
            }
        }
        String::new()
    }

    /// Letter-spaced upper-case label.
    #[allow(clippy::too_many_arguments)]
    pub fn caps(
        &self,
        pos: Pos2,
        al: Align2,
        s: &str,
        size: f32,
        f: &str,
        c: Color32,
        track: f32,
    ) -> f32 {
        let s = s.to_uppercase();
        let ws: Vec<f32> = s
            .chars()
            .map(|ch| self.width(&ch.to_string(), size, f))
            .collect();
        let total: f32 = ws.iter().sum::<f32>() + track * (ws.len().saturating_sub(1)) as f32;
        let x0 = match al.x() {
            egui::Align::Min => pos.x,
            egui::Align::Center => pos.x - total / 2.0,
            egui::Align::Max => pos.x - total,
        };
        let mut x = x0;
        for (ch, w) in s.chars().zip(&ws) {
            self.p.text(
                pos2(x, pos.y),
                Align2([egui::Align::Min, al.y()]),
                ch,
                FontId::new(size, fam(f)),
                c,
            );
            x += w + track;
        }
        total
    }

    pub fn shadow(&self, r: Rect, rad: f32, lvl: usize) {
        let s = self.k.sh[lvl];
        if s.alpha == 0 {
            return;
        }
        let sh = EShadow {
            offset: [0, s.dy as i8],
            blur: s.blur as u8,
            spread: 0,
            color: Color32::from_black_alpha(s.alpha),
        };
        self.p.add(sh.as_shape(r, CornerRadius::same(rad as u8)));
    }

    /// Vertical gradient inside a rounded rectangle (fan triangulation keeps a linear ramp exact).
    pub fn grad(&self, r: Rect, rad: f32, top: Color32, bot: Color32) {
        let pts = round_pts(r, rad);
        let mut m = Mesh::default();
        m.colored_vertex(r.center(), mix(top, bot, 0.5));
        for q in &pts {
            let t = ((q.y - r.top()) / r.height().max(1.0)).clamp(0.0, 1.0);
            m.colored_vertex(*q, mix(top, bot, t));
        }
        let n = pts.len() as u32;
        for i in 0..n {
            m.add_triangle(0, 1 + i, 1 + (i + 1) % n);
        }
        self.p.add(Shape::mesh(m));
        // Mesh edges are not anti-aliased, so ring them.
        self.p.add(Shape::closed_line(
            pts,
            Stroke::new(1.0, mix(top, bot, 0.5)),
        ));
    }

    pub fn rr(&self, r: Rect, rad: f32, fill: Color32) {
        self.p.rect_filled(r, CornerRadius::same(rad as u8), fill);
    }

    pub fn rr_stroke(&self, r: Rect, rad: f32, w: f32, c: Color32) {
        self.p.rect_stroke(
            r,
            CornerRadius::same(rad as u8),
            Stroke::new(w, c),
            StrokeKind::Inside,
        );
    }

    pub fn arc(&self, c: Pos2, r: f32, a0: f32, a1: f32, w: f32, col: Color32) {
        if (a1 - a0).abs() < 1e-3 {
            return;
        }
        self.p
            .add(Shape::line(arc_pts(c, r, a0, a1), Stroke::new(w, col)));
    }

    /// Soft halo: wider, fainter strokes under the line.
    pub fn glow_line(&self, pts: Vec<Pos2>, w: f32, col: Color32, strength: f32) {
        for (m, al) in [(3.4, 0.10), (2.2, 0.18), (1.5, 0.30)] {
            self.p.add(Shape::line(
                pts.clone(),
                Stroke::new(w * m, a(col, (255.0 * al * strength) as u8)),
            ));
        }
        self.p.add(Shape::line(pts, Stroke::new(w, col)));
    }

    pub fn glow_dot(&self, c: Pos2, r: f32, col: Color32, strength: f32) {
        for (m, al) in [(3.0, 0.08), (2.2, 0.14), (1.6, 0.26)] {
            self.p
                .circle_filled(c, r * m, a(col, (255.0 * al * strength) as u8));
        }
        self.p.circle_filled(c, r, col);
    }

    pub fn line(&self, p0: Pos2, p1: Pos2, w: f32, c: Color32) {
        self.p.line_segment([p0, p1], Stroke::new(w, c));
    }
}

pub fn round_pts(r: Rect, rad: f32) -> Vec<Pos2> {
    let rad = rad.min(r.width() / 2.0).min(r.height() / 2.0);
    let mut v = Vec::new();
    let corners = [
        (pos2(r.right() - rad, r.top() + rad), -PI / 2.0),
        (pos2(r.right() - rad, r.bottom() - rad), 0.0),
        (pos2(r.left() + rad, r.bottom() - rad), PI / 2.0),
        (pos2(r.left() + rad, r.top() + rad), PI),
    ];
    let n = if rad < 1.0 { 1 } else { 6 };
    for (c, a0) in corners {
        for i in 0..=n {
            let a = a0 + (PI / 2.0) * i as f32 / n as f32;
            v.push(c + vec2(a.cos(), a.sin()) * rad);
        }
    }
    v
}

pub fn arc_pts(c: Pos2, r: f32, a0: f32, a1: f32) -> Vec<Pos2> {
    let n = (((a1 - a0).abs() * r / 2.5) as usize).clamp(4, 80);
    (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect()
}

/// Knob travel: 0 → lower left, 1 → lower right, 270° sweep (screen angles, y down).
pub fn knob_angle(n: f32) -> f32 {
    PI * 0.75 + n.clamp(0.0, 1.0) * PI * 1.5
}

pub fn pol(c: Pos2, r: f32, ang: f32) -> Pos2 {
    c + vec2(ang.cos(), ang.sin()) * r
}

/// Cable path: cubic with gravity sag.
pub fn cable_pts(p0: Pos2, p1: Pos2, sag: f32) -> Vec<Pos2> {
    let d = (p1 - p0).length();
    let s = sag * (0.5 + (d / 400.0).min(1.0));
    let c0 = p0 + vec2(0.0, s);
    let c1 = p1 + vec2(0.0, s);
    (0..=40)
        .map(|i| {
            let t = i as f32 / 40.0;
            let u = 1.0 - t;
            let q = p0.to_vec2() * u * u * u
                + c0.to_vec2() * 3.0 * u * u * t
                + c1.to_vec2() * 3.0 * u * t * t
                + p1.to_vec2() * t * t * t;
            q.to_pos2()
        })
        .collect()
}

pub fn along(pts: &[Pos2], t: f32) -> Pos2 {
    let t = t.clamp(0.0, 1.0) * (pts.len() - 1) as f32;
    let i = (t as usize).min(pts.len() - 2);
    pts[i].lerp(pts[i + 1], t - i as f32)
}

pub fn poly_len(pts: &[Pos2]) -> f32 {
    pts.windows(2).map(|w| (w[1] - w[0]).length()).sum()
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum Ic {
    Play,
    Stop,
    Rec,
    Undo,
    Redo,
    Search,
    Star,
    StarFill,
    Plus,
    Minus,
    Fit,
    Gear,
    Down,
    Right,
    Close,
    Folder,
    Cable,
    Eye,
    Bolt,
    Save,
    Dots,
    Note,
    Link,
    Panel,
    Learn,
    Warn,
    Check,
    Dice,
}

/// Small stroke icons on a 16 px grid, drawn at `s` px.
pub fn icon(p: &Painter, ic: Ic, c: Pos2, s: f32, col: Color32) {
    let u = s / 16.0;
    let w = (1.4 * u).max(1.2);
    let st = Stroke::new(w, col);
    let q = |x: f32, y: f32| c + vec2(x, y) * u;
    let line = |pts: &[(f32, f32)]| {
        p.add(Shape::line(pts.iter().map(|&(x, y)| q(x, y)).collect(), st));
    };
    match ic {
        Ic::Play => {
            p.add(Shape::convex_polygon(
                vec![q(-4.0, -6.0), q(6.0, 0.0), q(-4.0, 6.0)],
                col,
                Stroke::NONE,
            ));
        }
        Ic::Stop => {
            p.rect_filled(Rect::from_center_size(c, vec2(10.0, 10.0) * u), 1.5, col);
        }
        Ic::Rec => {
            p.circle_filled(c, 5.0 * u, col);
        }
        Ic::Undo | Ic::Redo => {
            let f = if matches!(ic, Ic::Undo) { 1.0 } else { -1.0 };
            let pts: Vec<Pos2> = (0..=10)
                .map(|i| {
                    let a = -PI * 0.95 + PI * 1.15 * i as f32 / 10.0;
                    q(f * a.cos() * 5.5, a.sin() * 5.5 + 1.0)
                })
                .collect();
            let s0 = pts[0];
            p.add(Shape::line(pts, st));
            p.add(Shape::line(
                vec![
                    s0 + vec2(-3.2 * f, -0.5) * u,
                    s0 + vec2(0.0, 3.4 * u),
                    s0 + vec2(3.2 * f * u, -0.5 * u),
                ],
                st,
            ));
        }
        Ic::Search => {
            p.circle_stroke(q(-1.5, -1.5), 4.6 * u, st);
            line(&[(2.0, 2.0), (6.0, 6.0)]);
        }
        Ic::Star | Ic::StarFill => {
            let pts: Vec<Pos2> = (0..10)
                .map(|i| {
                    let r = if i % 2 == 0 { 7.0 } else { 3.0 };
                    let a = -PI / 2.0 + i as f32 * PI / 5.0;
                    c + vec2(a.cos(), a.sin()) * r * u
                })
                .collect();
            if matches!(ic, Ic::StarFill) {
                p.add(Shape::convex_polygon(pts.clone(), col, st));
            } else {
                p.add(Shape::closed_line(pts, st));
            }
        }
        Ic::Plus => {
            line(&[(-6.0, 0.0), (6.0, 0.0)]);
            line(&[(0.0, -6.0), (0.0, 6.0)]);
        }
        Ic::Minus => line(&[(-6.0, 0.0), (6.0, 0.0)]),
        Ic::Fit => {
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                line(&[
                    (sx * 7.0, sy * 3.0),
                    (sx * 7.0, sy * 7.0),
                    (sx * 3.0, sy * 7.0),
                ]);
            }
        }
        Ic::Gear => {
            p.circle_stroke(c, 2.8 * u, st);
            for i in 0..8 {
                let a = i as f32 * TAU / 8.0;
                p.line_segment(
                    [
                        c + vec2(a.cos(), a.sin()) * 5.2 * u,
                        c + vec2(a.cos(), a.sin()) * 7.2 * u,
                    ],
                    Stroke::new(w * 1.5, col),
                );
            }
            p.circle_stroke(c, 5.4 * u, st);
        }
        Ic::Down => line(&[(-4.0, -2.0), (0.0, 2.0), (4.0, -2.0)]),
        Ic::Right => line(&[(-2.0, -4.0), (2.0, 0.0), (-2.0, 4.0)]),
        Ic::Close => {
            line(&[(-4.5, -4.5), (4.5, 4.5)]);
            line(&[(4.5, -4.5), (-4.5, 4.5)]);
        }
        Ic::Folder => {
            p.add(Shape::closed_line(
                vec![
                    q(-7.0, -4.0),
                    q(-2.0, -4.0),
                    q(0.0, -2.0),
                    q(7.0, -2.0),
                    q(7.0, 5.0),
                    q(-7.0, 5.0),
                ],
                st,
            ));
        }
        Ic::Cable => {
            p.circle_stroke(q(-5.0, -4.0), 2.2 * u, st);
            p.circle_stroke(q(5.0, 4.0), 2.2 * u, st);
            p.add(Shape::line(
                cable_pts(q(-5.0, -2.0), q(5.0, 2.0), 5.0 * u),
                st,
            ));
        }
        Ic::Eye => {
            p.add(Shape::line(
                arc_pts(q(0.0, 5.0), 9.0 * u, PI * 1.2, PI * 1.8),
                st,
            ));
            p.add(Shape::line(
                arc_pts(q(0.0, -5.0), 9.0 * u, PI * 0.2, PI * 0.8),
                st,
            ));
            p.circle_filled(c, 2.2 * u, col);
        }
        Ic::Bolt => {
            p.add(Shape::convex_polygon(
                vec![
                    q(1.0, -7.0),
                    q(-5.0, 1.0),
                    q(-0.5, 1.0),
                    q(-1.0, 7.0),
                    q(5.0, -1.5),
                    q(0.5, -1.5),
                ],
                col,
                Stroke::NONE,
            ));
        }
        Ic::Save => {
            p.add(Shape::closed_line(
                vec![
                    q(-6.0, -6.0),
                    q(4.0, -6.0),
                    q(6.0, -4.0),
                    q(6.0, 6.0),
                    q(-6.0, 6.0),
                ],
                st,
            ));
            line(&[(-3.0, -6.0), (-3.0, -2.0), (2.0, -2.0), (2.0, -6.0)]);
            p.rect_stroke(
                Rect::from_center_size(q(0.0, 3.5), vec2(7.0, 4.0) * u),
                0.0,
                st,
                StrokeKind::Middle,
            );
        }
        Ic::Dots => {
            for dx in [-5.0, 0.0, 5.0] {
                p.circle_filled(q(dx, 0.0), 1.4 * u, col);
            }
        }
        Ic::Note => {
            p.circle_filled(q(-2.5, 4.5), 2.8 * u, col);
            line(&[(0.0, 4.0), (0.0, -6.0), (5.0, -4.0)]);
        }
        Ic::Link => {
            p.circle_stroke(q(-3.0, 0.0), 3.6 * u, st);
            p.circle_stroke(q(3.0, 0.0), 3.6 * u, st);
        }
        Ic::Panel => {
            p.rect_stroke(
                Rect::from_center_size(c, vec2(14.0, 12.0) * u),
                2.0,
                st,
                StrokeKind::Middle,
            );
            line(&[(3.0, -6.0), (3.0, 6.0)]);
        }
        Ic::Learn => {
            p.add(Shape::closed_line(
                vec![q(-7.0, -2.0), q(0.0, -6.0), q(7.0, -2.0), q(0.0, 2.0)],
                st,
            ));
            line(&[(-4.0, 0.5), (-4.0, 4.5), (0.0, 6.5), (4.0, 4.5), (4.0, 0.5)]);
        }
        Ic::Warn => {
            p.add(Shape::closed_line(
                vec![q(0.0, -7.0), q(7.0, 6.0), q(-7.0, 6.0)],
                st,
            ));
            line(&[(0.0, -2.0), (0.0, 2.0)]);
            p.circle_filled(q(0.0, 4.2), 0.9 * u, col);
        }
        Ic::Check => line(&[(-5.0, 0.0), (-1.5, 4.0), (5.5, -4.0)]),
        Ic::Dice => {
            p.rect_stroke(
                Rect::from_center_size(c, vec2(13.0, 13.0) * u),
                3.0,
                st,
                StrokeKind::Middle,
            );
            for (x, y) in [
                (-3.0, -3.0),
                (3.0, 3.0),
                (0.0, 0.0),
                (3.0, -3.0),
                (-3.0, 3.0),
            ] {
                p.circle_filled(q(x, y), 1.0 * u, col);
            }
        }
    }
}
