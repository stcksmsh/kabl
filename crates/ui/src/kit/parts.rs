//! Painter-level parts of the redesign kit (PLAN §7): selector, toggle, chip, drum cell, port
//! tray and the cable paint recipe. Pure drawing from a `Style`; the face and the parts sheet
//! (`examples/kit_parts.rs`) call the same functions. `z` is the zoom, rects are in screen px.

use crate::style::{alpha, mix, Role, Style};
use egui::{
    epaint::CubicBezierShape, pos2, vec2, Align2, Color32, CornerRadius, FontId, Painter, Pos2,
    Rect, Shape, Stroke, StrokeKind,
};

fn cr(r: f32, z: f32) -> CornerRadius {
    CornerRadius::same((r * z).round().clamp(0.0, 255.0) as u8)
}

fn font(st: &Style, role: Role, px: f32) -> FontId {
    FontId::new(px, st.type_role(role).font.family(st.fonts_ready))
}

/// The selector track colour: the inset well, a little darker than the face on paper.
pub fn track(st: &Style) -> Color32 {
    let r = &st.roles;
    mix(r.inset, r.line2, if st.dark { 0.0 } else { 0.5 })
}

/// A segmented selector: an outlined track, the selected segment as a raised accent cap.
/// `accent` overrides the cap colour (for example the gate colour for PLAY).
pub fn selector(
    p: &Painter,
    st: &Style,
    rect: Rect,
    opts: &[&str],
    sel: usize,
    z: f32,
    accent: Option<Color32>,
) {
    let r = &st.roles;
    p.rect_filled(rect, cr(4.0, z), track(st));
    p.rect_stroke(
        rect,
        cr(4.0, z),
        Stroke::new(1.0, r.line2),
        StrokeKind::Inside,
    );
    let w = rect.width() / opts.len() as f32;
    let cap = accent.unwrap_or(r.accent);
    let ink = if accent.is_some() {
        if crate::style::contrast(Color32::WHITE, cap) >= crate::style::contrast(r.on_accent, cap) {
            Color32::WHITE
        } else {
            r.on_accent
        }
    } else {
        r.on_accent
    };
    for (i, o) in opts.iter().enumerate() {
        let seg = Rect::from_min_size(rect.min + vec2(w * i as f32, 0.0), vec2(w, rect.height()));
        let on = i == sel;
        if on {
            let cap_r = seg.shrink2(vec2(1.5 * z, 1.5 * z));
            p.rect_filled(
                cap_r.translate(vec2(0.0, z)),
                cr(3.0, z),
                alpha(Color32::BLACK, 60),
            );
            p.rect_filled(cap_r, cr(3.0, z), cap);
        }
        p.text(
            seg.center(),
            Align2::CENTER_CENTER,
            o,
            font(st, Role::Value, 11.0 * z),
            if on { ink } else { r.text },
        );
    }
}

/// A two-state toggle: a pill track and a round knob, accent when on.
pub fn toggle(p: &Painter, st: &Style, rect: Rect, on: bool) {
    let r = &st.roles;
    p.rect_filled(rect, CornerRadius::same(255), track(st));
    p.rect_stroke(
        rect,
        CornerRadius::same(255),
        Stroke::new(1.0, r.line2),
        StrokeKind::Inside,
    );
    let d = rect.height() * 0.35;
    let cx = if on {
        rect.right() - rect.height() / 2.0
    } else {
        rect.left() + rect.height() / 2.0
    };
    p.circle_filled(
        pos2(cx, rect.center().y),
        d,
        if on { r.accent } else { r.text },
    );
}

/// The `+N` chip: an outlined rounded rect with mono text.
pub fn chip(p: &Painter, st: &Style, rect: Rect, text: &str, z: f32) {
    let r = &st.roles;
    p.rect_stroke(
        rect,
        cr(4.0, z),
        Stroke::new(1.0, r.line2),
        StrokeKind::Inside,
    );
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        font(st, Role::Value, 11.0 * z),
        r.text2,
    );
}

/// One drum-grid cell: `level` 0..=4 sets the fill height (0 = off), `chance` 0..=3 draws pips.
pub fn drum_cell(p: &Painter, st: &Style, rect: Rect, level: usize, chance: usize, z: f32) {
    let r = &st.roles;
    let well = mix(r.inset, r.text, 0.1);
    p.rect_filled(rect, cr(2.0, z), well);
    if level == 0 {
        return;
    }
    let f = [0.0, 0.3, 0.55, 0.78, 1.0][level.min(4)];
    let fh = (rect.height() - 2.0 * z) * f;
    let bar = Rect::from_min_max(
        pos2(rect.left() + z, rect.bottom() - z - fh),
        pos2(rect.right() - z, rect.bottom() - z),
    );
    p.rect_filled(bar, cr(1.5, z), alpha(r.accent, (140.0 + 115.0 * f) as u8));
    for k in 0..chance {
        p.circle_filled(
            pos2(
                rect.right() - 4.0 * z - k as f32 * 4.0 * z,
                rect.top() + 4.0 * z,
            ),
            1.4 * z,
            r.inset,
        );
    }
}

/// The port tray: an outlined rounded rect; `plate` is the raised output plate inside it.
pub fn tray(p: &Painter, st: &Style, rect: Rect, plate: Option<Rect>, z: f32) {
    let r = &st.roles;
    p.rect_stroke(
        rect,
        cr(5.0, z),
        Stroke::new(1.0, r.line2),
        StrokeKind::Inside,
    );
    if let Some(pl) = plate {
        p.rect_filled(pl, cr(5.0, z), r.raised);
        p.rect_stroke(
            pl,
            cr(5.0, z),
            Stroke::new(1.0, r.line2),
            StrokeKind::Inside,
        );
    }
}

/// A cable from `a` to `b` hanging by `sag`. Dark themes put a faint wide glow under the core;
/// a lit cable gets a lighter core and a highlight line; a stereo cable is a twin strand with a
/// dark centre line. The core stays at least 2.5 screen px wide, the stereo centre 1.5.
#[allow(clippy::too_many_arguments)]
pub fn cable(
    p: &Painter,
    st: &Style,
    a: Pos2,
    b: Pos2,
    sag: f32,
    signal: Color32,
    lit: bool,
    stereo: bool,
) {
    let c1 = a + vec2(20.0, sag);
    let c2 = b + vec2(-20.0, sag);
    let mk = |w: f32, c: Color32| {
        Shape::CubicBezier(CubicBezierShape::from_points_stroke(
            [a, c1, c2, b],
            false,
            Color32::TRANSPARENT,
            Stroke::new(w, c),
        ))
    };
    let edge = alpha(Color32::from_rgb(11, 12, 13), 255);
    if stereo {
        p.add(mk(8.0, edge));
        p.add(mk(6.4, signal));
        p.add(mk(1.6, edge));
    } else {
        let t = &st.slots.cable;
        let core = 4.0f32.max(2.5);
        if t.shadow_alpha > 0.0 {
            let dy = vec2(0.0, t.shadow_dy);
            p.add(Shape::CubicBezier(CubicBezierShape::from_points_stroke(
                [a + dy, c1 + dy, c2 + dy, b + dy],
                false,
                Color32::TRANSPARENT,
                Stroke::new(
                    core + 2.0,
                    alpha(Color32::BLACK, (t.shadow_alpha * 255.0) as u8),
                ),
            )));
        }
        if lit && t.glow_alpha > 0.0 {
            p.add(mk(
                t.glow_width,
                alpha(signal, (t.glow_alpha * 255.0) as u8),
            ));
        }
        if t.edge > 0.0 {
            p.add(mk(core + 2.0, mix(signal, Color32::BLACK, t.edge)));
        }
        p.add(mk(
            core,
            if lit {
                mix(signal, Color32::WHITE, 0.25)
            } else {
                signal
            },
        ));
        if lit {
            p.add(mk(1.0, alpha(Color32::WHITE, (t.highlight * 255.0) as u8)));
        }
    }
}

/// A functional cable's step beads along the same curve: filled = on, hollow = off.
pub fn cable_beads(
    p: &Painter,
    st: &Style,
    a: Pos2,
    b: Pos2,
    sag: f32,
    signal: Color32,
    steps: &[bool],
) {
    let c1 = a + vec2(20.0, sag);
    let c2 = b + vec2(-20.0, sag);
    let n = steps.len() as f32;
    for (i, on) in steps.iter().enumerate() {
        let t = (i as f32 + 0.5) / n;
        let u = 1.0 - t;
        let pt = a.to_vec2() * u.powi(3)
            + c1.to_vec2() * 3.0 * u * u * t
            + c2.to_vec2() * 3.0 * u * t * t
            + b.to_vec2() * t.powi(3);
        let pt = pos2(pt.x, pt.y);
        p.circle_filled(pt, 4.0, if *on { signal } else { st.roles.surface });
        p.circle_stroke(pt, 4.0, Stroke::new(1.5, signal));
    }
}
