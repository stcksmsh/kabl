//! Canvas painting. Reads the same `geom::Layout` the hit-test uses.

use crate::geom::{self, Area, Cables, CtlGeo, DepthGesture, Hit, Placed, JACK_R, PANEL_H, ROW_Y};
use crate::model::{fmt_value, to_value, CtlRef, JackRef, Kind, Patch, Route, Sig, Spec};
use crate::{App, Gesture};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};

pub struct Theme {
    pub dark: bool,
    pub chrome: Color32,
    pub ctext: Color32,
    pub rack: Color32,
    pub rail: Color32,
    pub rail_hi: Color32,
    pub hole: Color32,
    pub panel: Color32,
    pub panel_edge: Color32,
    pub ink: Color32,
    pub ink2: Color32,
    pub plate: Color32,
    pub plate_ink: Color32,
    pub knob: Color32,
    pub knob_hi: Color32,
    pub pointer: Color32,
    pub skirt: Color32,
    pub tick: Color32,
    pub nut: Color32,
    pub nut_edge: Color32,
    pub hole_c: Color32,
    pub display: Color32,
    pub display_ink: Color32,
    pub seg_bg: Color32,
    pub seg_on: Color32,
    pub seg_on_text: Color32,
    pub sel: Color32,
    pub concept: Color32,
    pub audio: Color32,
    pub cv: Color32,
    pub gate: Color32,
    pub btn: Color32,
}

pub fn hex(s: &str) -> Color32 {
    Color32::from_hex(s).expect("valid colour")
}

/// Palettes copied from revision-2 `render2.py` (A_LIGHT = render.py "a-warm", A_DARK).
pub fn theme(dark: bool) -> Theme {
    if dark {
        Theme {
            dark,
            chrome: hex("#1d1c1a"),
            ctext: hex("#efe8da"),
            rack: hex("#0e0d0c"),
            rail: hex("#5f5b55"),
            rail_hi: hex("#8c877e"),
            hole: hex("#161513"),
            panel: hex("#2f2c28"),
            panel_edge: hex("#4d4943"),
            ink: hex("#efe8da"),
            ink2: hex("#b2a893"),
            plate: hex("#191816"),
            plate_ink: hex("#efe8da"),
            knob: hex("#8e8981"),
            knob_hi: hex("#ece7de"),
            pointer: hex("#1b1917"),
            skirt: hex("#1b1a18"),
            tick: hex("#8d8475"),
            nut: hex("#a29d94"),
            nut_edge: hex("#67635c"),
            hole_c: hex("#080807"),
            display: hex("#141311"),
            display_ink: hex("#f1cf94"),
            seg_bg: hex("#23211e"),
            seg_on: hex("#e9dcc0"),
            seg_on_text: hex("#1b1916"),
            sel: hex("#5a9bff"),
            concept: hex("#ff7a5c"),
            audio: hex("#f0a640"),
            cv: hex("#35c2b1"),
            gate: hex("#a687ee"),
            btn: hex("#2c2a27"),
        }
    } else {
        Theme {
            dark,
            chrome: hex("#2a2826"),
            ctext: hex("#f1ece2"),
            rack: hex("#1b1a19"),
            rail: hex("#8f8b84"),
            rail_hi: hex("#bdb8ae"),
            hole: hex("#2a2826"),
            panel: hex("#e8e2d4"),
            panel_edge: hex("#b9b2a2"),
            ink: hex("#2a2520"),
            ink2: hex("#5d564c"),
            plate: hex("#2f2b27"),
            plate_ink: hex("#f1ece2"),
            knob: hex("#2a2826"),
            knob_hi: hex("#57524b"),
            pointer: hex("#f4efe6"),
            skirt: hex("#cfc7b6"),
            tick: hex("#6b6357"),
            nut: hex("#c3bfb6"),
            nut_edge: hex("#8b877f"),
            hole_c: hex("#121110"),
            display: hex("#23211e"),
            display_ink: hex("#efe6d2"),
            seg_bg: hex("#d8d1c1"),
            seg_on: hex("#2a2520"),
            seg_on_text: hex("#f4efe6"),
            sel: hex("#2a6ae0"),
            concept: hex("#c2412d"),
            audio: hex("#e0962b"),
            cv: hex("#23a597"),
            gate: hex("#8d67d6"),
            btn: hex("#3a3733"),
        }
    }
}

impl Theme {
    pub fn sig(&self, s: Sig) -> Color32 {
        match s {
            Sig::Audio => self.audio,
            Sig::Cv | Sig::Pitch => self.cv,
            Sig::Gate => self.gate,
        }
    }
}

/// World → screen transform.
#[derive(Clone, Copy)]
pub struct Xf {
    pub origin: Pos2,
    pub zoom: f32,
}

impl Xf {
    pub fn p(&self, w: Pos2) -> Pos2 {
        self.origin + w.to_vec2() * self.zoom
    }
    pub fn s(&self, l: f32) -> f32 {
        l * self.zoom
    }
    pub fn r(&self, r: Rect) -> Rect {
        Rect::from_min_max(self.p(r.min), self.p(r.max))
    }
    pub fn inv(&self, s: Pos2) -> Pos2 {
        ((s - self.origin) / self.zoom).to_pos2()
    }
}

fn darken(c: Color32, f: f32) -> Color32 {
    c.lerp_to_gamma(Color32::BLACK, f)
}

fn arc_points(c: Pos2, r: f32, t0: f32, t1: f32) -> Vec<Pos2> {
    let n = (((t1 - t0).abs() * 48.0).ceil() as usize).max(2);
    (0..=n).map(|i| geom::polar(c, r, t0 + (t1 - t0) * i as f32 / n as f32)).collect()
}

fn two_tone_arc(p: &Painter, c: Pos2, r: f32, t0: f32, t1: f32, col: Color32, w: f32, alpha: f32) {
    let pts = arc_points(c, r, t0, t1.max(t0 + 0.004));
    p.add(Shape::line(pts.clone(), Stroke::new(w + 2.5, darken(col, 0.45).gamma_multiply(alpha))));
    p.add(Shape::line(pts, Stroke::new(w, col.gamma_multiply(alpha))));
}

fn dashed(p: &Painter, pts: &[Pos2], stroke: Stroke, dash: f32, gap: f32) {
    p.extend(Shape::dashed_line(pts, stroke, dash, gap));
}

fn text(p: &Painter, pos: Pos2, align: Align2, s: &str, size: f32, col: Color32, mono: bool) -> Rect {
    let f = if mono { FontId::monospace(size) } else { FontId::proportional(size) };
    p.text(pos, align, s, f, col)
}

fn pill(p: &Painter, rect: Rect, s: &str, edge: Color32, fill: Color32, ink: Color32, size: f32) {
    p.rect_filled(rect, CornerRadius::same((rect.height() / 2.0) as u8), fill);
    p.rect_stroke(rect, CornerRadius::same((rect.height() / 2.0) as u8), Stroke::new(1.6, edge), StrokeKind::Inside);
    text(p, rect.center(), Align2::CENTER_CENTER, s, size, ink, true);
}

pub fn badge(p: &Painter, center: Pos2, s: &str, col: Color32, th: &Theme, z: f32) -> Rect {
    let g = p.layout_no_wrap(s.to_string(), FontId::proportional(10.5 * z), col);
    let rect = Rect::from_center_size(center, vec2(g.size().x + 12.0 * z, 15.0 * z));
    p.rect_filled(rect, CornerRadius::same((7.0 * z) as u8), th.panel);
    p.rect_stroke(rect, CornerRadius::same((7.0 * z) as u8), Stroke::new(1.3 * z, col), StrokeKind::Inside);
    p.galley(rect.center() - g.size() / 2.0, g, col);
    rect
}

// ------------------------------------------------------------------------------ canvas

pub fn canvas(app: &App, p: &Painter) {
    let th = theme(app.dark);
    let xf = app.xf();
    let z = xf.zoom;
    p.rect_filled(app.canvas, CornerRadius::ZERO, th.rack);
    let compact = app.geo.compact();
    if !compact {
        for &y in &ROW_Y {
            for ry in [y - 8.0, y + PANEL_H] {
                let r = Rect::from_min_max(pos2(app.canvas.left(), xf.p(pos2(0.0, ry)).y), pos2(app.canvas.right(), xf.p(pos2(0.0, ry + 8.0)).y));
                p.rect_filled(r, CornerRadius::ZERO, th.rail);
                p.line_segment([r.left_top(), r.right_top()], Stroke::new(1.0, th.rail_hi));
                let mut x = xf.p(pos2(0.0, 0.0)).x.rem_euclid(xf.s(30.0)) + app.canvas.left();
                while x < app.canvas.right() {
                    p.circle_filled(pos2(x, r.center().y), 2.0 * z, th.hole);
                    x += xf.s(30.0);
                }
            }
        }
    }
    let (cables, focus) = app.effective_cables();

    // Panels (faces, and advanced areas that push). Floating advanced areas come later.
    for (mi, pl) in app.layout.mods.iter().enumerate() {
        draw_module(app, p, &th, xf, mi, pl, false);
    }

    if cables != Cables::Hidden {
        draw_cables(app, p, &th, xf, focus, cables, false);
    }
    // Pills and hidden-mode badges sit above cables.
    for (mi, pl) in app.layout.mods.iter().enumerate() {
        draw_pills(app, p, &th, xf, mi, pl, cables == Cables::Hidden, Some(false));
    }
    // Floating advanced areas over neighbours, then the leads and pills inside them.
    for (mi, pl) in app.layout.mods.iter().enumerate() {
        if pl.overlay {
            draw_module(app, p, &th, xf, mi, pl, true);
        }
    }
    if cables != Cables::Hidden {
        draw_cables(app, p, &th, xf, focus, cables, true);
    }
    for (mi, pl) in app.layout.mods.iter().enumerate() {
        if pl.overlay {
            draw_pills(app, p, &th, xf, mi, pl, cables == Cables::Hidden, Some(true));
        }
    }

    // Gesture feedback.
    if let Gesture::Wire { from, .. } = &app.gesture {
        let a = xf.p(app.layout.mods[from.m].jack_center(from.j));
        let b = app.pointer.unwrap_or(a);
        let col = th.sig(app.patch.jack_def(*from).sig);
        cable_path(p, a, b, col, 5.5 * z, 1.0, false);
        let from_out = app.patch.jack_def(*from).out;
        // Targets: every knob (from an output) and every compatible jack.
        for (mi, pl) in app.layout.mods.iter().enumerate() {
            if mi == from.m {
                continue;
            }
            for (j, jd) in app.patch.modules[mi].def().jacks.iter().enumerate() {
                if jd.out != from_out {
                    p.circle_stroke(xf.p(pl.jack_center(j)), xf.s(JACK_R + 5.0), Stroke::new(1.5, th.sel.gamma_multiply(0.7)));
                }
            }
            if from_out {
                for g in pl.ctls.iter().flatten() {
                    if let CtlGeo::Knob { c, r } = *g {
                        let pts = arc_points(xf.p(c), xf.s(r + 11.0), 0.0, 1.0);
                        dashed(p, &pts, Stroke::new(1.6, th.sel.gamma_multiply(0.8)), 4.0, 3.0);
                    }
                }
            }
        }
    }

    if app.show_hits {
        for r in &app.regions {
            let col = match r.hit {
                Hit::Ring(_) => Color32::from_rgb(255, 60, 200),
                Hit::Handle(_) => Color32::from_rgb(255, 230, 0),
                Hit::Knob(_) => Color32::from_rgb(0, 200, 255),
                Hit::Plug(_) => Color32::from_rgb(255, 120, 0),
                Hit::Pill(_) => Color32::from_rgb(120, 255, 120),
                Hit::Jack(_) => Color32::from_rgb(255, 255, 255),
                Hit::Panel(_) | Hit::Header(_) => continue,
                _ => Color32::from_rgb(200, 200, 60),
            };
            let s = Stroke::new(1.2, col);
            match r.area {
                Area::Circle(c, rad) => {
                    p.circle_stroke(xf.p(c), xf.s(rad), s);
                }
                Area::Annulus(c, r0, r1) => {
                    p.circle_stroke(xf.p(c), xf.s(r0), s);
                    p.circle_stroke(xf.p(c), xf.s(r1), s);
                }
                Area::Box(b) => {
                    p.rect_stroke(xf.r(b), CornerRadius::ZERO, s, StrokeKind::Inside);
                }
            }
        }
    }
}

fn panel_rect(pl: &Placed, overlay: bool) -> Rect {
    if overlay {
        pl.adv.expect("overlay has advanced area")
    } else {
        pl.rect
    }
}

fn draw_module(app: &App, p: &Painter, th: &Theme, xf: Xf, mi: usize, pl: &Placed, overlay: bool) {
    let m = &app.patch.modules[mi];
    let d = m.def();
    let z = xf.zoom;
    let rect = xf.r(panel_rect(pl, overlay));
    let skin = m.kind == Kind::Ensemble;
    if overlay {
        p.rect_filled(rect.translate(vec2(6.0, 8.0) * z), CornerRadius::same(6), Color32::from_black_alpha(110));
    }
    p.rect_filled(rect, CornerRadius::same(3), th.panel);
    if skin {
        draw_skin_art(p, rect, th.dark);
    } else {
        // Brushed texture: faint horizontal strokes.
        let step = (4.0 * z).max(3.0);
        let mut y = rect.top() + step;
        let tex = if th.dark { Color32::from_white_alpha(6) } else { Color32::from_black_alpha(7) };
        while y < rect.bottom() {
            p.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], Stroke::new(1.0, tex));
            y += step;
        }
    }
    p.rect_stroke(rect, CornerRadius::same(3), Stroke::new(1.0, th.panel_edge), StrokeKind::Inside);
    if overlay || pl.adv.is_some() {
        // Engraved divider between face and advanced area.
        let x = xf.p(pos2(pl.rect.left() + pl.face_w, 0.0)).x;
        let y0 = rect.top() + 12.0 * z;
        p.line_segment([pos2(x, y0), pos2(x, rect.bottom() - 12.0 * z)], Stroke::new(1.5, darken(th.panel_edge, 0.2)));
    }
    if overlay {
        text(p, rect.left_top() + vec2(10.0, 8.0) * z, Align2::LEFT_TOP, "advanced · floating over neighbours", 10.0 * z, th.ink2, false);
        draw_controls(app, p, th, xf, mi, pl, Some(true));
        return;
    }
    if !pl.compact {
        for (sx, sy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
            let c = pos2(rect.left() + 9.0 * z + sx * (rect.width() - 18.0 * z), rect.top() + 9.0 * z + sy * (rect.height() - 18.0 * z));
            p.circle_filled(c, 3.5 * z, th.nut);
            p.circle_stroke(c, 3.5 * z, Stroke::new(1.0, th.nut_edge));
        }
    }
    let face = xf.r(Rect::from_min_size(pl.rect.min, vec2(pl.face_w, pl.rect.height())));
    let on_art = skin && app.labels_on_art;
    let ink = if on_art { skin_ink(th.dark) } else { th.ink };
    let ink2 = if on_art { skin_ink(th.dark) } else { th.ink2 };
    if skin && !on_art {
        let hdr = Rect::from_center_size(pos2(face.center().x, face.top() + 33.0 * z), vec2(150.0 * z, 38.0 * z));
        p.rect_filled(hdr, CornerRadius::same(4), th.panel.gamma_multiply(0.96));
    }
    let choosing = app.geo.choose == Some(mi);
    if pl.compact {
        text(p, pos2(face.center().x, face.top() + 18.0 * z), Align2::CENTER_CENTER, d.name, 14.0 * z, ink, false);
    } else {
        text(p, pos2(face.center().x, face.top() + 26.0 * z), Align2::CENTER_CENTER, d.name, 15.0 * z, ink, false);
        let tag = if choosing {
            let nk = (0..d.controls.len()).filter(|&c| m.primary[c] && matches!(d.controls[c].spec, Spec::Knob { .. })).count();
            format!("choose primary · face {}u", geom::face_wu(m.kind, nk))
        } else {
            d.tag.to_string()
        };
        text(p, pos2(face.center().x, face.top() + 43.0 * z), Align2::CENTER_CENTER, &tag, 11.0 * z, ink2, true);
    }
    if app.selected == Some(mi) {
        p.rect_stroke(rect.expand(1.0), CornerRadius::same(4), Stroke::new(2.5, th.sel), StrokeKind::Outside);
    }
    if !pl.compact {
        draw_decor(app, p, th, xf, mi, pl);
    }
    draw_controls(app, p, th, xf, mi, pl, if pl.overlay { Some(false) } else { None });
    draw_jacks(app, p, th, xf, mi, pl);
    if let Some(t) = pl.toggle {
        let r = xf.r(t);
        let expanded = app.geo.is_expanded(mi);
        let label = if expanded { "Less".to_string() } else { format!("+{}", m.hidden_count()) };
        p.rect_filled(r, CornerRadius::same(4), if th.dark { th.btn } else { th.plate });
        text(p, r.center(), Align2::CENTER_CENTER, &label, 12.0 * z, th.plate_ink, false);
        // A route to a control hidden by collapse docks here; never silently hidden.
        let hidden_routed: Vec<&Route> = app.patch.routes.iter().filter(|r| r.dst.m == mi && pl.ctls[r.dst.c].is_none()).collect();
        if !hidden_routed.is_empty() {
            p.rect_stroke(r.expand(3.0 * z), CornerRadius::same(6), Stroke::new(2.5 * z, th.cv), StrokeKind::Outside);
            if app.geo.cables == Cables::Hidden {
                badge(p, pos2(r.center().x - 6.0 * z, r.bottom() + 12.0 * z), &format!("◆ {} hidden", hidden_routed.len()), th.cv, th, z);
            }
        }
    }
    if let Some(dn) = pl.done {
        let r = xf.r(dn);
        p.rect_filled(r, CornerRadius::same(4), th.sel);
        text(p, r.center(), Align2::CENTER_CENTER, "Done", 12.0 * z, Color32::WHITE, false);
    }
    if skin {
        draw_skin_extras(app, p, th, xf, pl);
    }
}

pub fn skin_ink(dark: bool) -> Color32 {
    if dark {
        hex("#fff1d6")
    } else {
        hex("#2b1d33")
    }
}

/// Placeholder skin colour at a vertical fraction of the panel (0 = top).
pub fn skin_color(dark: bool, f: f32) -> Color32 {
    let (a, b, c) = if dark {
        (hex("#0d1030"), hex("#2a1f58"), hex("#51307a"))
    } else {
        (hex("#f6c9a6"), hex("#d99bb8"), hex("#8c7fc4"))
    };
    if f < 0.5 {
        a.lerp_to_gamma(b, f * 2.0)
    } else {
        b.lerp_to_gamma(c, (f - 0.5) * 2.0)
    }
}

fn moon(rect: Rect) -> (Pos2, f32) {
    (pos2(rect.right() - rect.width() * 0.26, rect.top() + rect.height() * 0.2), rect.width() * 0.09)
}

fn moon_color(dark: bool) -> Color32 {
    if dark {
        hex("#f3e7c1")
    } else {
        hex("#fff6e2")
    }
}

fn hill_y(rect: Rect, x: f32) -> f32 {
    let f = (x - rect.left()) / rect.width();
    rect.top() + rect.height() * (0.78 + 0.05 * (f * 9.0).sin())
}

fn hill_color(dark: bool) -> Color32 {
    if dark {
        hex("#1b1540")
    } else {
        hex("#6d5c9e")
    }
}

/// Placeholder art colour under a point (panel rect and point in the same space).
pub fn art_at(rect: Rect, pt: Pos2, dark: bool) -> Color32 {
    let (mc, mr) = moon(rect);
    if pt.distance(mc) <= mr {
        moon_color(dark)
    } else if pt.y >= hill_y(rect, pt.x) {
        hill_color(dark)
    } else {
        skin_color(dark, (pt.y - rect.top()) / rect.height())
    }
}

fn draw_skin_art(p: &Painter, rect: Rect, dark: bool) {
    let n = 40;
    for i in 0..n {
        let f0 = i as f32 / n as f32;
        let r = Rect::from_min_max(pos2(rect.left(), rect.top() + rect.height() * f0), pos2(rect.right(), rect.top() + rect.height() * (f0 + 1.0 / n as f32) + 1.0));
        p.rect_filled(r, CornerRadius::ZERO, skin_color(dark, f0));
    }
    let (mc, mr) = moon(rect);
    p.circle_filled(mc, mr, moon_color(dark));
    let hills: Vec<Pos2> = (0..=24)
        .map(|i| {
            let x = rect.left() + rect.width() * i as f32 / 24.0;
            pos2(x, hill_y(rect, x))
        })
        .chain([rect.right_bottom(), rect.left_bottom()])
        .collect();
    let hill = hill_color(dark);
    for w in hills.windows(2) {
        let (a, b) = (w[0], w[1]);
        if a.y < rect.bottom() && b.y < rect.bottom() {
            p.add(Shape::convex_polygon(vec![a, b, pos2(b.x, rect.bottom()), pos2(a.x, rect.bottom())], hill, Stroke::NONE));
        }
    }
}

/// Plates vs labels_on_art, the contrast warning, and the placeholder marking.
fn draw_skin_extras(app: &App, p: &Painter, th: &Theme, xf: Xf, pl: &Placed) {
    let z = xf.zoom;
    let r = xf.r(pl.rect);
    text(p, pos2(r.center().x, r.bottom() - 10.0 * z), Align2::CENTER_CENTER, "PLACEHOLDER ART", 9.0 * z, Color32::from_white_alpha(170), true);
    if app.labels_on_art {
        let worst = skin_label_contrast(th.dark, pl);
        if worst < 4.5 {
            let b = Rect::from_min_size(r.left_top() + vec2(8.0, 50.0) * z, vec2(r.width() - 16.0 * z, 17.0 * z));
            p.rect_filled(b, CornerRadius::same(3), hex("#7a1d12"));
            text(p, b.center(), Align2::CENTER_CENTER, &format!("⚠ label contrast {worst:.1}:1 < 4.5 (loader warns · rec)"), 9.5 * z, Color32::WHITE, false);
        }
    }
}

fn luminance(c: Color32) -> f32 {
    let f = |v: u8| {
        let s = v as f32 / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
}

pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// Worst label contrast over the art: each label and value box is sampled on a 5×3 grid
/// against the placeholder art under it (what a skin loader would measure on real art).
pub fn skin_label_contrast(dark: bool, pl: &Placed) -> f32 {
    let ink = skin_ink(dark);
    let boxes = pl.ctls.iter().flatten().flat_map(|g| {
        let lab = Rect::from_center_size(g.label_pos(), vec2(44.0, 14.0));
        let val = match *g {
            CtlGeo::Knob { .. } => g.pill_rect("0.00 Hz").shrink2(vec2(4.0, 3.0)),
            CtlGeo::Select { rect } => rect,
        };
        [lab, val]
    });
    let jacks = pl.jacks.iter().map(|j| Rect::from_center_size(j.center() - vec2(0.0, 24.0), vec2(20.0, 14.0)));
    boxes
        .chain(jacks)
        .flat_map(|b| (0..15).map(move |i| pos2(b.left() + b.width() * (i % 5) as f32 / 4.0, b.top() + b.height() * (i / 5) as f32 / 2.0)))
        .map(|pt| contrast(ink, art_at(pl.rect, pt, dark)))
        .fold(f32::MAX, f32::min)
}

fn draw_decor(app: &App, p: &Painter, th: &Theme, xf: Xf, mi: usize, pl: &Placed) {
    let m = &app.patch.modules[mi];
    let z = xf.zoom;
    let o = pl.rect.min;
    let fw = pl.face_w;
    match m.kind {
        Kind::Midi => {
            let r = xf.r(Rect::from_min_size(o + vec2(12.0, 70.0), vec2(fw - 24.0, 64.0)));
            let n = 7;
            let w = r.width() / n as f32;
            for i in 0..n {
                let k = Rect::from_min_size(pos2(r.left() + w * i as f32, r.top()), vec2(w - 1.0, r.height()));
                p.rect_filled(k, CornerRadius::same(2), hex("#f2eee6"));
                p.rect_stroke(k, CornerRadius::same(2), Stroke::new(1.0, th.tick), StrokeKind::Inside);
            }
            for i in [0, 1, 3, 4, 5] {
                let k = Rect::from_min_size(pos2(r.left() + w * (i as f32 + 0.68), r.top()), vec2(w * 0.62, r.height() * 0.6));
                p.rect_filled(k, CornerRadius::same(1), hex("#2a2826"));
            }
            let plate = xf.r(Rect::from_min_size(o + vec2(10.0, 182.0), vec2(fw - 20.0, 144.0)));
            p.rect_filled(plate, CornerRadius::same(6), th.plate);
        }
        Kind::Out => {
            let c = xf.p(o + vec2(fw / 2.0, 112.0));
            for r in [40.0, 30.0, 20.0, 10.0] {
                p.circle_stroke(c, r * z, Stroke::new(1.2, th.ink2));
            }
        }
        Kind::Adsr => {
            let r = xf.r(Rect::from_min_size(o + vec2(16.0, 54.0), vec2(fw - 32.0, 54.0)));
            p.rect_filled(r, CornerRadius::same(3), th.display);
            let v = |id: &str| {
                let c = app.patch.ctl(mi, id).c;
                to_value(&m.def().controls[c].spec, m.values[c])
            };
            let (a, d, s, rel) = (v("attack_ms"), v("decay_ms"), v("sustain"), v("release_ms"));
            // Log-ish time widths so short and long segments both stay visible.
            let wlog = |ms: f32| (1.0 + ms.max(0.1)).ln();
            let total = wlog(a) + wlog(d) + wlog(rel) + 2.0;
            let sx = |x: f32| r.left() + 6.0 * z + (r.width() - 12.0 * z) * x / total;
            let sy = |y: f32| r.bottom() - 6.0 * z - (r.height() - 12.0 * z) * y;
            let x1 = wlog(a);
            let x2 = x1 + wlog(d);
            let x3 = x2 + 2.0;
            let pts = vec![pos2(sx(0.0), sy(0.0)), pos2(sx(x1), sy(1.0)), pos2(sx(x2), sy(s)), pos2(sx(x3), sy(s)), pos2(sx(total), sy(0.0))];
            p.add(Shape::line(pts, Stroke::new(1.6 * z, th.display_ink)));
        }
        _ => {}
    }
    // Output plates behind output jacks (A grammar). Skinned panels use theme plates instead.
    if m.kind != Kind::Midi && m.kind != Kind::Ensemble {
        let outs: Vec<Rect> = m
            .def()
            .jacks
            .iter()
            .enumerate()
            .filter(|(_, j)| j.out)
            .map(|(j, _)| pl.jacks[j])
            .collect();
        if let Some(b) = outs.iter().copied().reduce(|a, b| a.union(b)) {
            let plate = Rect::from_min_max(b.min + vec2(-17.0, -40.0), b.max + vec2(17.0, 10.0));
            p.rect_filled(xf.r(plate), CornerRadius::same(6), th.plate);
        }
    }
}

fn draw_controls(app: &App, p: &Painter, th: &Theme, xf: Xf, mi: usize, pl: &Placed, only_adv: Option<bool>) {
    let m = &app.patch.modules[mi];
    let d = m.def();
    let z = xf.zoom;
    let skin = m.kind == Kind::Ensemble;
    let on_art = skin && app.labels_on_art;
    let (ink, ink2) = if on_art { (skin_ink(th.dark), skin_ink(th.dark)) } else { (th.ink, th.ink2) };
    for (c, g) in pl.ctls.iter().enumerate() {
        let Some(g) = g else { continue };
        if let Some(adv) = only_adv {
            if adv == m.primary[c] {
                continue;
            }
        }
        let def = &d.controls[c];
        let cref = CtlRef { m: mi, c };
        if skin && !on_art {
            // Theme plate behind each control group: contrast never depends on the art.
            let plate = match *g {
                CtlGeo::Knob { c, r } => Rect::from_min_max(c + vec2(-r - 14.0, -58.0), c + vec2(r + 14.0, 50.0)),
                CtlGeo::Select { rect } => rect.expand2(vec2(6.0, 6.0)).union(Rect::from_center_size(g.label_pos(), vec2(40.0, 16.0))),
            };
            p.rect_filled(xf.r(plate), CornerRadius::same(5), th.panel.gamma_multiply(0.96));
        }
        let lp = xf.p(g.label_pos());
        let lr = text(p, lp, Align2::CENTER_CENTER, def.label, 12.5 * z, ink, false);
        if def.conceptual {
            diamond(p, pos2(lr.right() + 6.0 * z, lr.center().y), th.concept, 3.5 * z);
        }
        if app.geo.choose == Some(mi) {
            let pr = xf.r(g.pin_rect(def.label));
            let on = m.primary[c];
            p.circle_filled(pr.center(), 7.5 * z, if on { th.sel } else { th.panel });
            p.circle_stroke(pr.center(), 7.5 * z, Stroke::new(1.6, th.sel));
            text(p, pr.center(), Align2::CENTER_CENTER, "📌", 9.0 * z, if on { Color32::WHITE } else { th.sel }, false);
        }
        let flash = app.flash_on(cref);
        match *g {
            CtlGeo::Knob { c: cen, r } => {
                draw_knob(app, p, th, xf, cref, cen, r);
                if flash {
                    p.circle_stroke(xf.p(cen), xf.s(r + 20.0), Stroke::new(3.0 * z, th.sel));
                }
            }
            CtlGeo::Select { rect } => {
                let Spec::Select { options, .. } = def.spec else { unreachable!() };
                let rr = xf.r(rect);
                p.rect_filled(rr, CornerRadius::same(4), th.seg_bg);
                let w = rr.width() / options.len() as f32;
                let cur = m.values[c] as usize;
                for (i, o) in options.iter().enumerate() {
                    let sr = Rect::from_min_size(pos2(rr.left() + w * i as f32, rr.top()), vec2(w, rr.height()));
                    if i == cur {
                        p.rect_filled(sr.shrink(2.0 * z), CornerRadius::same(3), th.seg_on);
                    }
                    text(p, sr.center(), Align2::CENTER_CENTER, o, 11.5 * z, if i == cur { th.seg_on_text } else { ink2 }, true);
                }
                // Modulated selector: bracket under the reachable options (whole steps).
                let routes: Vec<(usize, &Route)> = app.patch.routes_to(cref).collect();
                if !routes.is_empty() {
                    let (lo, hi) = app.patch.mod_span(cref, false);
                    let n = options.len() as f32 - 1.0;
                    let a = (cur as f32 + (lo * n).round()).clamp(0.0, n);
                    let b = (cur as f32 + (hi * n).round()).clamp(0.0, n);
                    let y = rr.bottom() + 4.0 * z;
                    let x0 = rr.left() + w * a + 3.0;
                    let x1 = rr.left() + w * (b + 1.0) - 3.0;
                    let col = if routes.iter().all(|(_, r)| r.bypass) { th.ink2 } else { th.cv };
                    p.line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(3.0 * z, col));
                    for (k, _) in routes.iter().enumerate().take(3) {
                        let pp = pos2(rr.center().x + (k as f32 - (routes.len().min(3) as f32 - 1.0) / 2.0) * 15.0 * z, rr.bottom() + 10.0 * z);
                        p.circle_filled(pp, 3.4 * z, col);
                    }
                }
                if flash {
                    p.rect_stroke(rr.expand(5.0 * z), CornerRadius::same(6), Stroke::new(3.0 * z, th.sel), StrokeKind::Outside);
                }
            }
        }
    }
    if only_adv != Some(false) {
        if let Some(pr) = pl.preview {
            draw_lfo_preview(app, p, th, xf, mi, pr);
        }
    }
}

fn diamond(p: &Painter, c: Pos2, col: Color32, s: f32) {
    p.add(Shape::convex_polygon(vec![c + vec2(0.0, -s), c + vec2(s, 0.0), c + vec2(0.0, s), c + vec2(-s, 0.0)], col, Stroke::NONE));
}

fn draw_knob(app: &App, p: &Painter, th: &Theme, xf: Xf, cref: CtlRef, cen: Pos2, r: f32) {
    let z = xf.zoom;
    let c = xf.p(cen);
    let rs = xf.s(r);
    let m = &app.patch.modules[cref.m];
    let base = m.values[cref.c];
    for i in 0..=10 {
        let t = i as f32 / 10.0;
        p.line_segment([geom::polar(c, rs + 4.0 * z, t), geom::polar(c, rs + 7.5 * z, t)], Stroke::new(1.2 * z, th.tick));
    }
    p.circle_filled(c, rs + 1.5 * z, th.skirt);
    p.circle_filled(c, rs * 0.84, th.knob);
    p.circle_filled(c + vec2(-0.25, -0.3) * rs, rs * 0.35, th.knob_hi.gamma_multiply(0.35));
    if th.dark {
        // Knurled light-metal cap (A-dark takes B's material).
        for i in 0..24 {
            let a = i as f32 / 24.0 * std::f32::consts::TAU;
            let dir = vec2(a.cos(), a.sin());
            p.line_segment([c + dir * rs * 0.74, c + dir * rs * 0.84], Stroke::new(1.0, darken(th.knob, 0.3)));
        }
    }
    p.line_segment([c + (geom::polar(c, 1.0, base) - c) * rs * 0.2, geom::polar(c, rs * 0.78, base)], Stroke::new(2.6 * z, th.pointer));

    let routes: Vec<(usize, &Route)> = app.patch.routes_to(cref).collect();
    if routes.is_empty() {
        return;
    }
    let col = th.cv;
    let rr = rs + 11.0 * z;
    let inspected = app.inspect == Some(cref);
    let depth_drag = matches!(app.gesture, Gesture::Depth { route, .. } if app.patch.routes.get(route).is_some_and(|r| r.dst == cref));
    let strong = inspected || depth_drag || routes.iter().any(|(i, r)| Some(r.src.m) == app.selected || Some(*i) == app.geo.selected_route);
    let active: Vec<&(usize, &Route)> = routes.iter().filter(|(_, r)| !r.bypass).collect();
    if !active.is_empty() {
        let (lo, hi) = app.patch.mod_span(cref, false);
        let (l, h) = (base + lo, base + hi);
        for (beyond, lim) in [(l < 0.0, 0.0), (h > 1.0, 1.0)] {
            if beyond {
                let (t0, t1) = if lim == 0.0 { (l.max(-0.12), 0.0) } else { (1.0, h.min(1.12)) };
                dashed(p, &arc_points(c, rr, t0, t1), Stroke::new(2.0 * z, col.gamma_multiply(0.7)), 3.0 * z, 3.0 * z);
                let (a, b) = (geom::polar(c, rs + 5.0 * z, lim), geom::polar(c, rs + 17.0 * z, lim));
                p.line_segment([a, b], Stroke::new(4.5 * z, darken(col, 0.45)));
                p.line_segment([a, b], Stroke::new(2.5 * z, col));
            }
        }
        two_tone_arc(p, c, rr, l.max(0.0), h.min(1.0), col, if strong { 4.0 * z } else { 3.0 * z }, if strong { 1.0 } else { 0.75 });
        if active.len() == 1 {
            let tip = geom::polar(c, rr, (base + active[0].1.amount).clamp(0.0, 1.0));
            p.circle_filled(tip, 4.2 * z, th.panel);
            p.circle_stroke(tip, 4.2 * z, Stroke::new(2.4 * z, darken(col, 0.45)));
            p.circle_filled(tip, 2.2 * z, col);
        }
    } else {
        let (lo, hi) = app.patch.mod_span(cref, true);
        dashed(p, &arc_points(c, rr, (base + lo).max(0.0), (base + hi).min(1.0)), Stroke::new(2.0 * z, th.ink2), 4.0 * z, 3.0 * z);
    }
    if inspected && routes.len() > 1 {
        for (i, (_, r)) in routes.iter().enumerate() {
            let (a, b) = app.patch.route_span(r);
            let lane = col.lerp_to_gamma(Color32::WHITE, 0.25 * i as f32);
            two_tone_arc(p, c, rr + (7.0 + 6.0 * i as f32) * z, (base + a).max(0.0), (base + b).min(1.0), if r.bypass { th.ink2 } else { lane }, 2.0 * z, 1.0);
        }
    }
    // Depth handle: the explicit alternative always shows it; the ring variant shows it while
    // dragging.
    let show_handle = app.geo.depth == DepthGesture::PeakHandle || depth_drag;
    if show_handle {
        if let Some(ri) = geom::depth_route(&app.patch, cref, app.geo.selected_route) {
            let tip = geom::polar(c, rr, (base + app.patch.routes[ri].amount).clamp(0.0, 1.0));
            p.circle_filled(tip, if depth_drag { 7.0 } else { 5.5 } * z, Color32::WHITE);
            p.circle_stroke(tip, if depth_drag { 7.0 } else { 5.5 } * z, Stroke::new(2.5 * z, th.sel));
        }
    }
    if app.geo.depth == DepthGesture::RingBand && matches!(app.hover, Some(Hit::Ring(h)) if h == cref) && !depth_drag {
        p.circle_stroke(c, rs + geom::RING_R0 * z, Stroke::new(1.0, th.sel.gamma_multiply(0.5)));
        p.circle_stroke(c, rs + geom::RING_R1 * z, Stroke::new(1.0, th.sel.gamma_multiply(0.5)));
    }
}

/// `overlay_pass`: Some(true) draws only a floating area's controls, Some(false) everything else.
fn draw_pills(app: &App, p: &Painter, th: &Theme, xf: Xf, mi: usize, pl: &Placed, hidden: bool, overlay_pass: Option<bool>) {
    let m = &app.patch.modules[mi];
    let d = m.def();
    let z = xf.zoom;
    let on_art = m.kind == Kind::Ensemble && app.labels_on_art;
    for (c, g) in pl.ctls.iter().enumerate() {
        let Some(CtlGeo::Knob { c: cen, r }) = *g else { continue };
        if overlay_pass.is_some_and(|o| o != (pl.overlay && !m.primary[c])) {
            continue;
        }
        let cref = CtlRef { m: mi, c };
        let value = fmt_value(&d.controls[c].spec, m.values[c]);
        let routes: Vec<(usize, &Route)> = app.patch.routes_to(cref).collect();
        let pr = xf.r(g.as_ref().unwrap().pill_rect(&value));
        if routes.is_empty() {
            let ink = if on_art { skin_ink(th.dark) } else { th.ink };
            text(p, pr.center(), Align2::CENTER_CENTER, &value, 11.5 * z, ink, true);
            continue;
        }
        let active = routes.iter().any(|(_, r)| !r.bypass);
        let pc = if active { th.cv } else { th.ink2 };
        if routes.len() > 3 {
            // The 3rd plug position is shared by every further route.
            let third = xf.p(geom::plug_pos(cen, r, 2, routes.len()));
            text(p, third + vec2(9.0, 0.0) * z, Align2::LEFT_CENTER, &format!("+{}", routes.len() - 2), 10.0 * z, pc, false);
        }
        if hidden {
            for (k, _) in routes.iter().enumerate().take(3) {
                let sp = xf.p(geom::plug_pos(cen, r, k, routes.len()));
                p.circle_filled(sp, 5.0 * z, darken(pc, 0.45));
                p.circle_filled(sp, 3.4 * z, pc);
            }
            let s = if routes.len() == 1 { format!("← {}", app.patch.module_label(routes[0].1.src.m)) } else { format!("← {} mods", routes.len()) };
            badge(p, badge_pos(app, pl, mi, c, pr, &s, xf), &s, pc, th, z);
        }
        pill(p, pr, &value, pc, th.panel, th.ink, 11.5 * z);
    }
    if hidden && !pl.compact && overlay_pass != Some(true) {
        // Jack badges: where each cable goes, in place of the cable.
        for (j, jd) in d.jacks.iter().enumerate() {
            let jr = JackRef { m: mi, j };
            if let Some(s) = jack_badge(&app.patch, jr) {
                let c = xf.p(pl.jack_center(j));
                let col = th.sig(jd.sig);
                let at = if jd.label_right { c + vec2(0.0, 22.0) * z } else { c + vec2(0.0, 23.0) * z };
                badge(p, at, &s, col, th, z);
            }
        }
    }
}

/// Below the pill; if that hits another control's label or selector, beside the pill on
/// whichever side stays inside the panel (revision-2 INTERACTIONS §2.1).
fn badge_pos(app: &App, pl: &Placed, mi: usize, own: usize, pill: Rect, s: &str, xf: Xf) -> Pos2 {
    let z = xf.zoom;
    let d = app.patch.modules[mi].def();
    let bw = (s.chars().count() as f32 * 6.2 + 12.0) * z;
    let obstacles: Vec<Rect> = pl
        .ctls
        .iter()
        .enumerate()
        .filter(|(c, _)| *c != own)
        .filter_map(|(c, g)| g.map(|g| (c, g)))
        .flat_map(|(c, g)| {
            let lab = Rect::from_center_size(g.label_pos(), vec2(d.controls[c].label.chars().count() as f32 * 7.0, 16.0));
            let body = match g {
                CtlGeo::Select { rect } => rect,
                CtlGeo::Knob { c, r } => Rect::from_center_size(c, vec2(2.0 * r, 2.0 * r)),
            };
            let val = match g {
                CtlGeo::Knob { .. } => g.pill_rect("0000000"),
                CtlGeo::Select { rect } => rect,
            };
            [xf.r(lab), xf.r(body), xf.r(val)]
        })
        .collect();
    let panel = xf.r(pl.adv.map_or(pl.rect, |a| pl.rect.union(a)));
    let below = pos2(pill.center().x, pill.bottom() + 9.0 * z);
    let right = pos2(pill.right() + 4.0 * z + bw / 2.0, pill.center().y);
    let left = pos2(pill.left() - 4.0 * z - bw / 2.0, pill.center().y);
    let fits = |c: Pos2| {
        let r = Rect::from_center_size(c, vec2(bw, 15.0 * z));
        panel.contains_rect(r) && !obstacles.iter().any(|o| o.intersects(r))
    };
    [below, right, left].into_iter().find(|c| fits(*c)).unwrap_or(below)
}

/// `← MIDI`, `→ Filter`, `→ 2 routes`; None when unconnected.
pub fn jack_badge(p: &Patch, jr: JackRef) -> Option<String> {
    let jd = p.jack_def(jr);
    if jd.out {
        let mut dests: Vec<usize> = p.cables.iter().filter(|c| c.from == jr).map(|c| c.to.m).collect();
        dests.extend(p.routes.iter().filter(|r| r.src == jr).map(|r| r.dst.m));
        match dests.len() {
            0 => None,
            1 => Some(format!("→ {}", p.module_label(dests[0]))),
            n => {
                let first = dests[0];
                if dests.iter().all(|&d| d == first) {
                    Some(format!("→ {} ×{n}", p.module_label(first)))
                } else {
                    Some(format!("→ {n} routes"))
                }
            }
        }
    } else {
        p.cables.iter().find(|c| c.to == jr).map(|c| format!("← {}", p.module_label(c.from.m)))
    }
}

fn draw_jacks(app: &App, p: &Painter, th: &Theme, xf: Xf, mi: usize, pl: &Placed) {
    let m = &app.patch.modules[mi];
    let z = xf.zoom;
    let skin = m.kind == Kind::Ensemble;
    for (j, jd) in m.def().jacks.iter().enumerate() {
        let jr = JackRef { m: mi, j };
        let col = th.sig(jd.sig);
        let connected = jack_badge(&app.patch, jr).is_some();
        if pl.compact {
            let r = xf.r(pl.jacks[j]);
            p.rect_filled(r, CornerRadius::same(4), if th.dark { th.plate } else { th.seg_bg });
            p.rect_filled(Rect::from_min_size(r.min, vec2(3.0 * z, r.height())), CornerRadius::same(1), col);
            let s = match jack_badge(&app.patch, jr) {
                Some(b) => format!("{} {b}", jd.label),
                None => jd.label.to_string(),
            };
            let ink = if th.dark { th.plate_ink } else { th.ink };
            let g = p.layout_no_wrap(s, FontId::proportional(10.5 * z), if connected { ink } else { th.ink2 });
            let clip = p.with_clip_rect(r.shrink(2.0));
            clip.galley(pos2(r.left() + 7.0 * z, r.center().y - g.size().y / 2.0), g, ink);
            continue;
        }
        let c = xf.p(pl.jack_center(j));
        let on_plate = jd.out && m.kind != Kind::Ensemble;
        let lab_ink = if on_plate || m.kind == Kind::Midi { th.plate_ink } else if skin && app.labels_on_art { skin_ink(th.dark) } else { th.ink };
        if skin && !app.labels_on_art {
            p.rect_filled(Rect::from_center_size(c + vec2(0.0, -10.0) * z, vec2(40.0, 52.0) * z), CornerRadius::same(5), th.panel.gamma_multiply(0.96));
        }
        if jd.label_right {
            text(p, c + vec2(20.0 * z, 0.0), Align2::LEFT_CENTER, jd.label, 12.5 * z, lab_ink, false);
        } else {
            let lr = text(p, c + vec2(0.0, -24.0 * z), Align2::CENTER_CENTER, jd.label, 12.5 * z, lab_ink, false);
            if jd.conceptual {
                diamond(p, pos2(lr.right() + 6.0 * z, lr.center().y), th.concept, 3.5 * z);
            }
        }
        p.circle_filled(c, xf.s(JACK_R), th.nut);
        p.circle_stroke(c, xf.s(JACK_R), Stroke::new(1.0, th.nut_edge));
        if connected && app.effective_cables().0 == Cables::Hidden {
            p.circle_stroke(c, xf.s(JACK_R - 2.0), Stroke::new(3.0 * z, col));
        }
        p.circle_filled(c, xf.s(7.0), th.hole_c);
    }
}

fn cable_path(p: &Painter, a: Pos2, b: Pos2, col: Color32, w: f32, alpha: f32, bypass: bool) {
    let sag = 30.0 + 0.25 * (b - a).length();
    let (c1, c2) = (a + vec2(0.0, sag), b + vec2(0.0, sag));
    let pts: Vec<Pos2> = (0..=40)
        .map(|i| {
            let t = i as f32 / 40.0;
            let u = 1.0 - t;
            (a.to_vec2() * u * u * u + c1.to_vec2() * 3.0 * u * u * t + c2.to_vec2() * 3.0 * u * t * t + b.to_vec2() * t * t * t).to_pos2()
        })
        .collect();
    if bypass {
        dashed(p, &pts, Stroke::new(w, Color32::from_gray(140).gamma_multiply(alpha)), 7.0, 5.0);
    } else {
        p.add(Shape::line(pts.clone(), Stroke::new(w + 2.0, Color32::from_black_alpha((90.0 * alpha) as u8))));
        p.add(Shape::line(pts, Stroke::new(w, col.gamma_multiply(alpha))));
    }
    for e in [a, b] {
        p.circle_filled(e, w * 1.35, darken(col, 0.35).gamma_multiply(alpha));
        p.circle_filled(e, w * 0.9, col.gamma_multiply(alpha));
    }
}

/// Where a route's lead ends: the knob's 6 o'clock plug, or the `+N` button when collapsed.
pub fn route_end(app: &App, ri: usize) -> Option<Pos2> {
    let r = app.patch.routes[ri];
    let pl = &app.layout.mods[r.dst.m];
    match pl.ctls[r.dst.c] {
        Some(CtlGeo::Knob { c, r: rad }) => {
            let idx = app.patch.routes_to(r.dst).position(|(i, _)| i == ri).unwrap_or(0);
            Some(geom::plug_pos(c, rad, idx, app.patch.routes_to(r.dst).count()))
        }
        Some(CtlGeo::Select { rect }) => Some(pos2(rect.center().x, rect.bottom() + 10.0)),
        None => pl.toggle.map(|t| t.center()),
    }
}

fn draw_cables(app: &App, p: &Painter, th: &Theme, xf: Xf, focus: Option<usize>, mode: Cables, overlay_pass: bool) {
    let z = xf.zoom;
    let in_overlay = |ri: usize| {
        let r = app.patch.routes[ri];
        let pl = &app.layout.mods[r.dst.m];
        pl.overlay && !app.patch.modules[r.dst.m].primary[r.dst.c]
    };
    let alpha_for = |a: usize, b: usize| match (mode, focus) {
        (Cables::Focus, Some(f)) if a != f && b != f => 0.12,
        _ => 1.0,
    };
    if !overlay_pass {
        for c in &app.patch.cables {
            let a = xf.p(app.layout.mods[c.from.m].jack_center(c.from.j));
            let b = xf.p(app.layout.mods[c.to.m].jack_center(c.to.j));
            let col = th.sig(app.patch.jack_def(c.from).sig);
            cable_path(p, a, b, col, 5.5 * z, alpha_for(c.from.m, c.to.m), false);
        }
    }
    for (ri, r) in app.patch.routes.iter().enumerate() {
        if in_overlay(ri) != overlay_pass {
            continue;
        }
        let Some(end) = route_end(app, ri) else { continue };
        let a = xf.p(app.layout.mods[r.src.m].jack_center(r.src.j));
        let strong = app.geo.selected_route == Some(ri) || app.inspect == Some(r.dst) || app.selected == Some(r.src.m);
        let alpha = alpha_for(r.src.m, r.dst.m) * if strong { 1.0 } else { 0.72 };
        cable_path(p, a, xf.p(end), th.sig(app.patch.jack_def(r.src).sig), 4.0 * z, alpha, r.bypass);
    }
}

fn draw_lfo_preview(app: &App, p: &Painter, th: &Theme, xf: Xf, mi: usize, pr: Rect) {
    let z = xf.zoom;
    let r = xf.r(pr);
    p.rect_filled(r, CornerRadius::same(3), th.display);
    let m = &app.patch.modules[mi];
    let v = |id: &str| {
        let c = app.patch.ctl(mi, id).c;
        to_value(&m.def().controls[c].spec, m.values[c])
    };
    let (wave, amp, off, phase, fade) = (v("waveform") as usize, v("amp"), v("offset"), v("phase") / 360.0, v("fade_ms"));
    let uni = v("polarity") >= 1.0;
    let n = 120;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let x = i as f32 / n as f32;
            let ph = (x * 2.0 + phase).fract();
            let s = match wave {
                0 => (ph * std::f32::consts::TAU).sin(),
                1 => 1.0 - 4.0 * (ph - 0.5).abs(),
                2 => 2.0 * ph - 1.0,
                3 => if ph < 0.5 { 1.0 } else { -1.0 },
                _ => [0.3, -0.7, 0.9, -0.2][((x * 2.0 + phase) as usize) % 4],
            };
            let s = if uni { (s + 1.0) / 2.0 } else { s };
            // Preview spans 2 cycles at the current rate; fade-in drawn on that time axis.
            let secs = x * 2.0 / v("rate_hz").max(0.01);
            let f = (secs * 1000.0 / fade.max(1.0)).min(1.0);
            let y = (off + amp * f * s).clamp(-1.0, 1.0);
            pos2(r.left() + r.width() * x, r.center().y - y * (r.height() / 2.0 - 6.0 * z))
        })
        .collect();
    p.line_segment([pos2(r.left(), r.center().y), pos2(r.right(), r.center().y)], Stroke::new(1.0, th.display_ink.gamma_multiply(0.2)));
    p.add(Shape::line(pts, Stroke::new(1.6 * z, th.display_ink)));
    text(p, r.left_top() + vec2(4.0, 3.0) * z, Align2::LEFT_TOP, "PREVIEW · from params · not audio", 9.0 * z, th.concept, true);
}

// ------------------------------------------------------------------------------ envelope A/B plot

pub fn env_plot(p: &Painter, rect: Rect, a: &[f32], b: &[f32], seconds: f32, th: &Theme) {
    p.rect_filled(rect, CornerRadius::same(3), th.display);
    let n = rect.width() as usize;
    for (sig, col) in [(a, th.cv), (b, th.audio)] {
        let pts: Vec<Pos2> = (0..n)
            .map(|x| {
                let i = x * sig.len() / n;
                pos2(rect.left() + x as f32, rect.bottom() - 4.0 - (rect.height() - 8.0) * sig[i.min(sig.len() - 1)])
            })
            .collect();
        p.add(Shape::line(pts, Stroke::new(1.4, col)));
    }
    for s in 0..=(seconds as usize) {
        let x = rect.left() + rect.width() * s as f32 / seconds;
        p.line_segment([pos2(x, rect.bottom() - 4.0), pos2(x, rect.bottom())], Stroke::new(1.0, th.display_ink.gamma_multiply(0.5)));
        p.text(pos2(x + 2.0, rect.bottom() - 2.0), Align2::LEFT_BOTTOM, format!("{s}s"), FontId::monospace(9.0), th.display_ink.gamma_multiply(0.6));
    }
}
