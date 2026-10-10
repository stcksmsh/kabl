//! Module-face recipes: material, hardware, title, knob, jack, readouts and the display frame
//! and trace. Colours come from the theme's roles and the face's section palette, shapes from
//! the recipe slots; callers pass the zoom `z` and a screen-space position.

use super::containers::eshadow;
use crate::style::{
    alpha, mix, DisplayFrame, DisplayTrace, FaceHardware, FaceMaterial, FaceTitle, JackRecipe,
    KnobBody, KnobMod, KnobScale, KnobValue, Role, Style, TEXT_FLOOR,
};
use egui::{
    epaint::Mesh, pos2, vec2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Shape, Stroke,
    StrokeKind,
};
use std::sync::Arc;

/// Face text smaller than this on screen is left out: it would be noise, and the 11 px floor
/// would make it collide with its neighbours.
pub const HIDE_BELOW: f32 = 9.0;

/// The font of a type role at zoom `z`, never below the 11 px floor; `None` when the scaled text
/// is too small to read.
pub fn face_font(st: &Style, role: Role, z: f32) -> Option<FontId> {
    sized(st, role, z, HIDE_BELOW)
}

/// A module's name stays readable lower than the rest, so a zoomed-out rack still says what is
/// what.
fn title_font(st: &Style, z: f32) -> Option<FontId> {
    sized(st, Role::H3, z, 6.0)
}

fn sized(st: &Style, role: Role, z: f32, hide_below: f32) -> Option<FontId> {
    let t = st.type_role(role);
    (t.size * z >= hide_below)
        .then(|| FontId::new((t.size * z).max(TEXT_FLOOR), t.font.family(st.fonts_ready)))
}

/// Angle (radians from 12 o'clock, clockwise) of knob travel `n` on the 270° scale.
pub fn travel_angle(n: f32) -> f32 {
    (-135.0 + n.clamp(0.0, 1.0) * 270.0).to_radians()
}

pub fn on_circle(c: Pos2, radius: f32, angle: f32) -> Pos2 {
    c + vec2(angle.sin(), -angle.cos()) * radius
}

/// Points of the knob-travel arc `n0..n1` at `radius`.
pub fn arc_pts(c: Pos2, radius: f32, n0: f32, n1: f32) -> Vec<Pos2> {
    let (a0, a1) = (travel_angle(n0.min(n1)), travel_angle(n0.max(n1)));
    let steps = (((a1 - a0).abs() / 0.08).ceil() as usize).max(1);
    (0..=steps)
        .map(|i| on_circle(c, radius, a0 + (a1 - a0) * i as f32 / steps as f32))
        .collect()
}

/// The face: shadow, material, edge and the corner hardware.
pub fn surface(p: &Painter, st: &Style, rect: Rect, base: Color32, edge: Color32, z: f32) {
    let f = &st.slots.face;
    let cr = CornerRadius::same(3);
    if f.material != FaceMaterial::Flat && st.shadows[f.shadow.min(2)].alpha > 0 {
        p.add(eshadow(st, f.shadow).as_shape(rect, cr));
    }
    let top = mix(base, Color32::WHITE, f.gradient * 0.8);
    let bottom = mix(base, Color32::BLACK, f.gradient * 0.65);
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(Shape::mesh(mesh));
    if f.material == FaceMaterial::Lacquer && f.gloss > 0.0 {
        let band = Rect::from_min_size(rect.min, vec2(rect.width(), rect.height() * 0.36));
        p.rect_filled(band, cr, Color32::from_white_alpha((f.gloss * 255.0) as u8));
    }
    if f.grain > 0.0 && z >= 0.5 {
        let a = (f.grain * 100.0) as u8;
        let mut y = rect.top() + 3.0;
        let mut lines = Vec::new();
        while y < rect.bottom() {
            lines.push(Shape::line_segment(
                [pos2(rect.left() + 1.0, y), pos2(rect.right() - 1.0, y)],
                Stroke::new(0.5, Color32::from_white_alpha(a)),
            ));
            y += 5.0;
        }
        p.extend(lines);
    }
    p.rect_stroke(rect, cr, Stroke::new(1.0, edge), StrokeKind::Inside);
    if f.material == FaceMaterial::Satin {
        p.line_segment(
            [
                rect.left_top() + vec2(1.0, 1.0),
                rect.right_top() + vec2(-1.0, 1.0),
            ],
            Stroke::new(1.0, Color32::from_white_alpha(38)),
        );
    }
    if f.material != FaceMaterial::Flat || f.hardware != FaceHardware::None {
        hardware(p, st, rect, z);
    }
}

/// The corner nuts or screws.
pub fn hardware(p: &Painter, st: &Style, rect: Rect, z: f32) {
    let f = &st.slots.face;
    let nut = mix(st.roles.knob, st.roles.knob_hi, 0.35);
    let nut_edge = mix(st.roles.knob, st.roles.knob_hi, 0.8);
    if f.hardware != FaceHardware::None {
        for (sx, sy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
            let c = pos2(
                rect.left() + 9.0 * z + sx * (rect.width() - 18.0 * z),
                rect.top() + 9.0 * z + sy * (rect.height() - 18.0 * z),
            );
            p.circle_filled(c, 3.5 * z, nut);
            p.circle_stroke(c, 3.5 * z, Stroke::new(1.0, nut_edge));
            if f.hardware == FaceHardware::Screws {
                p.line_segment(
                    [c - vec2(2.4, 0.0) * z, c + vec2(2.4, 0.0) * z],
                    Stroke::new(1.0, nut_edge),
                );
            }
        }
    }
}

/// The module's name and its tag line. Returns the name's rect.
#[allow(clippy::too_many_arguments)]
pub fn title(
    p: &Painter,
    st: &Style,
    face: Rect,
    z: f32,
    name: &str,
    tag: &str,
    ink: Color32,
    ink2: Color32,
) -> Rect {
    let cx = face.center().x;
    let y = face.top() + 26.0 * z;
    let mut rect = Rect::NOTHING;
    if let Some(font) = title_font(st, z) {
        rect = match st.slots.face.title {
            FaceTitle::Centered => {
                p.text(pos2(cx, y), egui::Align2::CENTER_CENTER, name, font, ink)
            }
            FaceTitle::LeftDot => {
                p.circle_filled(pos2(face.left() + 22.0 * z, y), 4.0 * z, st.roles.accent);
                p.text(
                    pos2(face.left() + 32.0 * z, y),
                    egui::Align2::LEFT_CENTER,
                    name,
                    font,
                    ink,
                )
            }
            FaceTitle::CapsRule => {
                let r = p.text(
                    pos2(cx, y),
                    egui::Align2::CENTER_CENTER,
                    name.to_uppercase(),
                    font,
                    ink,
                );
                p.line_segment(
                    [
                        pos2(face.left() + 18.0 * z, y + 13.0 * z),
                        pos2(face.right() - 18.0 * z, y + 13.0 * z),
                    ],
                    Stroke::new(1.0, alpha(ink2, 120)),
                );
                r
            }
        };
    }
    if let Some(font) = face_font(st, Role::Caption, z) {
        let at = pos2(cx, face.top() + 43.0 * z);
        match st.slots.face.title {
            FaceTitle::LeftDot => {
                p.text(
                    pos2(face.left() + 32.0 * z, at.y),
                    egui::Align2::LEFT_CENTER,
                    tag,
                    font,
                    ink2,
                );
            }
            _ => {
                p.text(at, egui::Align2::CENTER_CENTER, tag, font, ink2);
            }
        }
    }
    rect
}

/// A knob in the theme's body, scale and value-marker recipes. `rs` is the screen radius.
pub struct Knob {
    pub c: Pos2,
    pub rs: f32,
    pub z: f32,
    /// Knob travel, 0..=1.
    pub value: f32,
    pub hot: bool,
}

pub fn knob(p: &Painter, st: &Style, k: &Knob, tick: Color32) {
    let (c, rs, z) = (k.c, k.rs, k.z);
    let r = &st.roles;
    let ks = &st.slots.knob;
    let track = rs + 5.5 * z;
    match ks.scale {
        KnobScale::Ticks => {
            for i in 0..=10 {
                let a = travel_angle(i as f32 / 10.0);
                p.line_segment(
                    [on_circle(c, rs + 4.0 * z, a), on_circle(c, rs + 7.5 * z, a)],
                    Stroke::new((1.2 * z).max(1.0), tick),
                );
            }
        }
        KnobScale::TrackArc => {
            p.add(Shape::line(
                arc_pts(c, track, 0.0, 1.0),
                Stroke::new(2.4 * z, alpha(tick, 90)),
            ));
        }
        KnobScale::None => {}
    }
    if ks.value == KnobValue::Arc && k.value > 0.004 {
        p.add(Shape::line(
            arc_pts(c, track, 0.0, k.value),
            Stroke::new(2.4 * z, r.accent),
        ));
    }
    let skirt = mix(r.knob, r.knob_hi, 0.25);
    let rim = alpha(mix(r.knob_hi, r.knob, 0.5), 255);
    match ks.body {
        KnobBody::Cap => {
            p.circle_filled(
                c + vec2(0.0, 1.6 * z),
                rs + 1.5 * z,
                alpha(Color32::BLACK, 50),
            );
            p.circle_filled(c, rs + 1.5 * z, skirt);
            p.circle_filled(c, rs * 0.84, r.knob);
            p.circle_stroke(c, rs * 0.84, Stroke::new((0.8 * z).max(0.8), rim));
            for i in 0..24 {
                let a = i as f32 / 24.0 * std::f32::consts::TAU;
                let d = vec2(a.cos(), a.sin());
                p.line_segment(
                    [c + d * rs * 0.73, c + d * rs * 0.82],
                    Stroke::new((0.6 * z).max(0.6), mix(r.knob_hi, r.knob, 0.65)),
                );
            }
        }
        KnobBody::Glass => {
            p.circle_filled(
                c + vec2(0.0, 1.6 * z),
                rs + 1.0 * z,
                alpha(Color32::BLACK, 50),
            );
            p.circle_filled(c, rs, r.knob);
            p.circle_filled(
                c + vec2(-rs * 0.15, -rs * 0.2),
                rs * 0.66,
                alpha(r.knob_hi, 90),
            );
            p.circle_stroke(c, rs, Stroke::new((1.0 * z).max(1.0), rim));
        }
        KnobBody::Disc => {
            p.circle_filled(c, rs, r.knob);
            p.circle_stroke(c, rs, Stroke::new((1.0 * z).max(1.0), rim));
        }
    }
    if k.hot {
        p.circle_stroke(c, rs + 1.5 * z, Stroke::new((1.2 * z).max(1.2), r.focus));
    }
    let a = travel_angle(k.value);
    let w = (2.6 * z).max(1.6);
    match ks.value {
        KnobValue::Line => {
            p.line_segment(
                [on_circle(c, rs * 0.2, a), on_circle(c, rs * 0.78, a)],
                Stroke::new(w, r.knob_ink),
            );
        }
        KnobValue::Dot => {
            p.circle_filled(on_circle(c, rs * 0.62, a), (2.8 * z).max(1.8), r.knob_ink);
        }
        KnobValue::Notch => {
            p.line_segment(
                [on_circle(c, rs * 0.52, a), on_circle(c, rs * 0.84, a)],
                Stroke::new(w * 1.3, r.knob_ink),
            );
        }
        KnobValue::Arc => {
            p.line_segment(
                [on_circle(c, rs * 0.55, a), on_circle(c, rs * 0.8, a)],
                Stroke::new(w, r.knob_ink),
            );
        }
    }
}

/// The reachable range of a modulated knob, `lo..hi` in knob travel, at ring radius `rr`.
/// `strong` is the inspected or selected knob.
pub fn mod_range(
    p: &Painter,
    st: &Style,
    c: Pos2,
    rr: f32,
    lo: f32,
    hi: f32,
    strong: bool,
    z: f32,
) {
    let col = if strong {
        st.roles.cv
    } else {
        st.roles.cv.gamma_multiply(0.75)
    };
    let w = if strong { 4.0 } else { 3.0 } * z;
    let pts = arc_pts(c, rr, lo, hi.max(lo + 0.004));
    match st.slots.knob.modulation {
        KnobMod::ArcDot => {
            let outline = alpha(mix(col, Color32::BLACK, 0.45), col.a());
            p.add(Shape::line(pts.clone(), Stroke::new(w + 2.5, outline)));
            p.add(Shape::line(pts, Stroke::new(w, col)));
        }
        KnobMod::ArcGlow => {
            let g = st.slots.knob.glow.max(0.35);
            p.add(Shape::line(
                pts.clone(),
                Stroke::new(w + 6.0 * z, alpha(col, (70.0 * g) as u8)),
            ));
            p.add(Shape::line(pts, Stroke::new(w, col)));
        }
        KnobMod::Segment => {
            p.extend(Shape::dashed_line(
                &pts,
                Stroke::new(w, col),
                4.0 * z,
                2.0 * z,
            ));
        }
    }
}

/// The value a modulated knob has right now, as a dot on its range ring.
pub fn live_dot(p: &Painter, st: &Style, c: Pos2, rr: f32, n: f32, z: f32) {
    let at = on_circle(c, rr, travel_angle(n));
    p.circle_filled(at, 4.4 * z, st.roles.cv.gamma_multiply(0.45));
    p.circle_filled(at, 3.4 * z, st.roles.knob_ink);
    p.circle_stroke(at, 3.4 * z, Stroke::new(1.4 * z, st.roles.cv));
}

/// A jack of the theme's recipe. `signal` is the port type's colour, shown as a rim when the
/// jack carries a cable.
#[allow(clippy::too_many_arguments)]
pub fn jack(
    p: &Painter,
    st: &Style,
    c: Pos2,
    r: f32,
    z: f32,
    signal: Color32,
    patched: bool,
    hot: bool,
) {
    let ro = &st.roles;
    let nut = mix(ro.knob, ro.knob_hi, 0.35);
    let edge = mix(ro.knob, ro.knob_hi, 0.8);
    match st.slots.jack {
        JackRecipe::Nut => {
            p.circle_filled(c, r, nut);
            p.circle_stroke(c, r, Stroke::new(1.0, edge));
            if patched {
                p.circle_stroke(c, r * 0.72, Stroke::new((2.2 * z).max(1.5), signal));
            }
            p.circle_filled(c, r * 0.54, ro.hole);
        }
        JackRecipe::GlowRing => {
            p.circle_filled(c, r, mix(ro.knob, ro.hole, 0.4));
            if patched {
                let g = st.slots.knob.glow.max(0.4);
                p.circle_stroke(
                    c,
                    r * 0.86,
                    Stroke::new(5.0 * z, alpha(signal, (60.0 * g) as u8)),
                );
            }
            p.circle_stroke(
                c,
                r * 0.86,
                Stroke::new((2.0 * z).max(1.5), if patched { signal } else { edge }),
            );
            p.circle_filled(c, r * 0.5, ro.hole);
        }
        JackRecipe::Donut => {
            p.circle_stroke(
                c,
                r * 0.78,
                Stroke::new(r * 0.44, if patched { signal } else { nut }),
            );
            p.circle_stroke(c, r, Stroke::new(1.0, edge));
            p.circle_filled(c, r * 0.5, ro.hole);
        }
    }
    if hot {
        p.circle_stroke(c, r + 3.0 * z, Stroke::new(1.5, ro.focus));
    }
}

/// A value readout that fits `max_w`: the unit's space goes first. `None` when too small.
pub fn readout(
    p: &Painter,
    st: &Style,
    s: &str,
    z: f32,
    max_w: f32,
    col: Color32,
) -> Option<Arc<egui::Galley>> {
    let font = face_font(st, Role::Value, z)?;
    let g = p.layout_no_wrap(s.to_string(), font.clone(), col);
    if g.size().x <= max_w || !s.contains(' ') {
        return Some(g);
    }
    Some(p.layout_no_wrap(s.replace(' ', ""), font, col))
}

/// The frame of a live display: glass, neon or paper, with a faint grid.
pub fn display_frame(p: &Painter, st: &Style, rect: Rect, z: f32) {
    let r = &st.roles;
    let cr = CornerRadius::same((3.0 * z).clamp(2.0, 6.0) as u8);
    match st.slots.display.frame {
        DisplayFrame::Glass => {
            p.rect_filled(rect, cr, r.disp_bg);
            let sh = Rect::from_min_size(rect.min, vec2(rect.width(), (6.0 * z).max(2.0)));
            p.rect_filled(sh, cr, alpha(Color32::BLACK, 70));
            p.rect_stroke(
                rect,
                cr,
                Stroke::new(1.0, alpha(Color32::BLACK, 120)),
                StrokeKind::Inside,
            );
        }
        DisplayFrame::Neon => {
            p.rect_filled(rect, cr, r.disp_bg);
            p.rect_stroke(
                rect.expand(1.5),
                cr,
                Stroke::new(
                    3.0,
                    alpha(r.disp_trace, (70.0 * st.slots.display.glow) as u8),
                ),
                StrokeKind::Outside,
            );
            p.rect_stroke(
                rect,
                cr,
                Stroke::new(1.0, alpha(r.disp_trace, 150)),
                StrokeKind::Inside,
            );
        }
        DisplayFrame::Paper => {
            p.rect_filled(rect, cr, mix(r.disp_bg, Color32::WHITE, 0.82));
            p.rect_stroke(rect, cr, Stroke::new(1.0, r.line2), StrokeKind::Inside);
        }
    }
    if rect.height() >= 28.0 {
        let grid = if st.slots.display.frame == DisplayFrame::Paper {
            alpha(r.line2, 90)
        } else {
            alpha(r.disp_grid, 200)
        };
        p.line_segment(
            [
                pos2(rect.left() + 2.0, rect.center().y),
                pos2(rect.right() - 2.0, rect.center().y),
            ],
            Stroke::new(1.0, grid),
        );
    }
}

/// The trace colour of a display.
pub fn trace_color(st: &Style) -> Color32 {
    if st.slots.display.frame == DisplayFrame::Paper {
        mix(st.roles.disp_bg, st.roles.accent, 0.7)
    } else {
        st.roles.disp_trace
    }
}

/// A trace along `pts` in the theme's glow and width, optionally filled down to `base_y`.
pub fn trace(p: &Painter, st: &Style, pts: Vec<Pos2>, z: f32, base_y: Option<f32>) {
    let col = trace_color(st);
    let d = &st.slots.display;
    if let (Some(y), true) = (base_y, d.fill > 0.0) {
        let mut mesh = Mesh::default();
        let fill = alpha(col, (255.0 * d.fill * 0.5) as u8);
        for w in pts.windows(2) {
            let i = mesh.vertices.len() as u32;
            mesh.colored_vertex(w[0], fill);
            mesh.colored_vertex(w[1], fill);
            mesh.colored_vertex(pos2(w[1].x, y), alpha(fill, 0));
            mesh.colored_vertex(pos2(w[0].x, y), alpha(fill, 0));
            mesh.add_triangle(i, i + 1, i + 2);
            mesh.add_triangle(i, i + 2, i + 3);
        }
        p.add(Shape::mesh(mesh));
    }
    let w = (1.6 * z).clamp(1.2, 3.0);
    match st.slots.display.trace {
        DisplayTrace::Crisp => {}
        DisplayTrace::SoftGlow => {
            p.add(Shape::line(
                pts.clone(),
                Stroke::new(w + 3.0, alpha(col, (60.0 * d.glow) as u8)),
            ));
        }
        DisplayTrace::StrongGlow => {
            p.add(Shape::line(
                pts.clone(),
                Stroke::new(w + 5.0, alpha(col, (90.0 * d.glow.max(0.6)) as u8)),
            ));
            p.add(Shape::line(
                pts.clone(),
                Stroke::new(w + 2.0, alpha(col, (110.0 * d.glow.max(0.6)) as u8)),
            ));
        }
    }
    p.add(Shape::line(pts, Stroke::new(w, col)));
}

/// A playhead dot on a display.
pub fn playhead(p: &Painter, st: &Style, at: Pos2, z: f32) {
    let col = trace_color(st);
    p.circle_filled(at, 5.0 * z.max(0.7), alpha(col, 70));
    p.circle_filled(at, 3.0 * z.max(0.7), st.roles.disp_text);
}
