//! Module-face recipes: material, hardware, title, knob, jack, readouts and the display frame
//! and trace. Colours come from the theme's roles and the face's section palette, shapes from
//! the recipe slots; callers pass the zoom `z` and a screen-space position.

use super::containers::eshadow;
use crate::style::{
    alpha, mix, CompactText, DisplayFrame, DisplayTrace, FaceHardware, FaceMaterial, FaceTitle,
    JackRecipe, KnobBody, KnobMod, KnobScale, KnobValue, Role, Style, TEXT_FLOOR,
};
use egui::{
    epaint::Mesh, pos2, vec2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Shape, Stroke,
    StrokeKind,
};
use std::sync::Arc;

/// Face text smaller than this on screen is left out: it would be noise, and the 11 px floor
/// would make it collide with its neighbours.
pub const HIDE_BELOW: f32 = 9.5;

/// The font of a type role at zoom `z`, never below the 11 px floor; `None` when the scaled text
/// is too small to read.
pub fn face_font(st: &Style, role: Role, z: f32) -> Option<FontId> {
    sized(st, role, z, HIDE_BELOW)
}

/// A module's name stays readable lower than the rest, so a zoomed-out rack still says what is
/// what.
fn title_font(st: &Style, z: f32) -> Option<FontId> {
    if st.slots.face.compact == CompactText::Hold {
        let t = st.type_role(Role::H3);
        return Some(FontId::new(
            (t.size * z).max(HOLD_TITLE),
            t.font.family(st.fonts_ready),
        ));
    }
    sized(st, Role::H3, z, 6.0)
}

/// Compact tier (`CompactText::Hold`): the smallest face text and title, in screen px.
pub const HOLD_FLOOR: f32 = 9.5;
const HOLD_TITLE: f32 = 10.5;
/// Zoom below which no face text is drawn under `Hold` (the overview tier) and above which
/// values and captions are drawn again (the full tier).
const OVERVIEW_BELOW: f32 = 0.55;
const FULL_FROM: f32 = 0.864;

/// Whether `z` is in the overview tier of a `Hold` style: no face text, not even the `+N` chip.
pub fn overview(st: &Style, z: f32) -> bool {
    st.slots.face.compact == CompactText::Hold && z < OVERVIEW_BELOW
}

/// Whether `z` is in the compact tier of a `Hold` style: held text, which is dropped where it
/// does not fit its slot (PLAN section 5), instead of overlapping its neighbours.
pub fn compact_tier(st: &Style, z: f32) -> bool {
    st.slots.face.compact == CompactText::Hold && (OVERVIEW_BELOW..FULL_FROM).contains(&z)
}

fn sized(st: &Style, role: Role, z: f32, hide_below: f32) -> Option<FontId> {
    let t = st.type_role(role);
    if st.slots.face.compact == CompactText::Hold {
        let value_like = matches!(role, Role::Value | Role::Caption);
        if z < OVERVIEW_BELOW || (value_like && z < FULL_FROM) {
            return None;
        }
        return Some(FontId::new(
            (t.size * z).max(HOLD_FLOOR),
            t.font.family(st.fonts_ready),
        ));
    }
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
    if f.material == FaceMaterial::Line {
        // Line art: the surface colour, one hairline edge, screws; no gradient, gloss or grain.
        p.rect_filled(rect, cr, base);
        p.rect_stroke(rect, cr, Stroke::new(1.0, edge), StrokeKind::Inside);
        hardware(p, st, rect, z);
        return;
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
        // Continuous top light: fades out over the whole face, so no edge cuts the content.
        let mut mesh = Mesh::default();
        let lit = Color32::from_white_alpha((f.gloss * 255.0) as u8);
        mesh.colored_vertex(rect.left_top(), lit);
        mesh.colored_vertex(rect.right_top(), lit);
        mesh.colored_vertex(rect.right_bottom(), Color32::TRANSPARENT);
        mesh.colored_vertex(rect.left_bottom(), Color32::TRANSPARENT);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        p.add(Shape::mesh(mesh));
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

/// The module's name and its tag line. Returns the name's rect. `avoid` is the screen rects of
/// face content (jacks and their labels, knobs, displays): a left-aligned title that would
/// touch any of them is set centred instead, as the centred recipes are.
#[allow(clippy::too_many_arguments)]
/// The name a face shows when the full one does not fit beside the `+N` chip (PLAN section 4:
/// short name, never an ellipsis). Unlisted names are already short.
pub fn short_title(name: &str) -> &str {
    match name {
        "VA Oscillator" => "VA Osc",
        "SVF Filter" => "SVF",
        "ADSR Envelope" => "ADSR",
        "Ladder Filter" => "Ladder",
        "Comb Filter" => "Comb",
        "Wavetable Oscillator" => "Wavetable",
        "FM Operator" => "FM Op",
        "FM Voice (6 operators)" => "FM Voice",
        "Ring Modulator" => "Ring Mod",
        "Sample & Hold" => "S&H",
        "Comparator" => "Comp",
        "Crossfade" => "X-Fade",
        "Quantizer" => "Quant",
        "Attenuverter" => "Atten",
        "Compressor" => "Comp",
        "Arpeggiator" => "Arp",
        "Bitcrusher" => "Crush",
        _ => name,
    }
}

/// The text a title measures as: condensed-caps titles are drawn in capitals.
pub fn title_text(st: &Style, name: &str) -> String {
    if st.slots.face.title == FaceTitle::CondCaps {
        name.to_uppercase()
    } else {
        name.to_string()
    }
}

/// Room for a title on the row at screen `y`: from the left margin to the nearest thing in the
/// header band (`avoid`), or the `+N` chip's place, less a gap.
pub fn title_room_beside(face: Rect, z: f32, y: f32, avoid: &[Rect]) -> f32 {
    let right = avoid
        .iter()
        .filter(|a| a.top() < y + 8.0 * z && a.bottom() > y - 8.0 * z && a.left() > face.center().x)
        .map(|a| a.left())
        .fold(face.right() - 46.0 * z, f32::min);
    right - 10.0 * z - (face.left() + 12.0 * z)
}

/// Room for a title: the face less the `+N` chip, its gap and the side margin.
pub fn title_room(face_w: f32, z: f32) -> f32 {
    face_w - 66.0 * z
}

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
    avoid: &[Rect],
) -> Rect {
    let cx = face.center().x;
    let cond = st.slots.face.title == FaceTitle::CondCaps;
    let y = face.top() + if cond { 19.7 } else { 26.0 } * z;
    let y_tag = face.top() + if cond { 38.0 } else { 43.0 } * z;
    let mut rect = Rect::NOTHING;
    let name = match title_font(st, z) {
        Some(f)
            if p.layout_no_wrap(title_text(st, name), f.clone(), ink)
                .size()
                .x
                * 1.06
                > title_room_beside(face, z, y, avoid) =>
        {
            short_title(name)
        }
        _ => name,
    };
    let dot = st.slots.face.title == FaceTitle::LeftDot;
    let x0 = face.left() + if dot { 32.0 } else { 12.0 } * z;
    let left = st.slots.face.title != FaceTitle::CapsRule && {
        let size = |text: &str, font: Option<FontId>| {
            font.map_or(egui::Vec2::ZERO, |f| {
                p.layout_no_wrap(text.to_string(), f, ink).size()
            })
        };
        let n = size(name, title_font(st, z));
        let t = size(tag, face_font(st, Role::Caption, z));
        let name_r = Rect::from_min_size(pos2(x0, y - n.y / 2.0), n);
        let tag_r = Rect::from_min_size(pos2(x0, y_tag - t.y / 2.0), t);
        // Too wide for the room right of the dot: centre it, as the centred recipes do.
        name_r.max.x <= face.right() - 6.0 * z
            && tag_r.max.x <= face.right() - 6.0 * z
            && !avoid
                .iter()
                .any(|a| a.intersects(name_r) || a.intersects(tag_r))
    };
    let kind = match st.slots.face.title {
        FaceTitle::CapsRule => FaceTitle::CapsRule,
        FaceTitle::CondCaps => FaceTitle::CondCaps,
        _ if left => FaceTitle::LeftDot,
        _ => FaceTitle::Centered,
    };
    if let Some(mut font) = title_font(st, z) {
        // Keep clear of the "+N" chip on the right: a long name steps down a size.
        let w = p
            .layout_no_wrap(name.to_string(), font.clone(), ink)
            .size()
            .x;
        if w > face.width() - 100.0 * z {
            font.size = (font.size * 0.8).max(TEXT_FLOOR);
        }
        rect = match kind {
            FaceTitle::CondCaps => cond_draw(p, pos2(x0, y), name, &font, ink),
            FaceTitle::Centered => {
                p.text(pos2(cx, y), egui::Align2::CENTER_CENTER, name, font, ink)
            }
            FaceTitle::LeftDot => {
                if dot {
                    p.circle_filled(pos2(face.left() + 22.0 * z, y), 4.0 * z, st.roles.accent);
                }
                p.text(pos2(x0, y), egui::Align2::LEFT_CENTER, name, font, ink)
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
        let at = pos2(cx, y_tag);
        match kind {
            FaceTitle::LeftDot | FaceTitle::CondCaps => {
                p.text(pos2(x0, at.y), egui::Align2::LEFT_CENTER, tag, font, ink2);
            }
            _ => {
                p.text(at, egui::Align2::CENTER_CENTER, tag, font, ink2);
            }
        }
    }
    rect
}

/// Condensed caps title: uppercase with +0.07 em tracking, drawn glyph by glyph because egui
/// has no letter spacing. `at` is the left end, vertically centred. Returns the text rect.
pub fn cond_draw(p: &Painter, at: Pos2, text: &str, font: &FontId, col: Color32) -> Rect {
    let up = text.to_uppercase();
    let track = 0.07 * font.size;
    let mut prefix = String::new();
    for ch in up.chars() {
        let x = if prefix.is_empty() {
            0.0
        } else {
            p.layout_no_wrap(prefix.clone(), font.clone(), col).size().x
                + track * prefix.chars().count() as f32
        };
        p.text(
            at + vec2(x, 0.0),
            egui::Align2::LEFT_CENTER,
            ch,
            font.clone(),
            col,
        );
        prefix.push(ch);
    }
    let w = p.layout_no_wrap(up.clone(), font.clone(), col).size().x
        + track * up.chars().count().saturating_sub(1) as f32;
    Rect::from_min_size(at - vec2(0.0, font.size * 0.5), vec2(w, font.size))
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
    let solid = ks.body == KnobBody::Solid;
    let track = rs + if solid { 3.5 } else { 5.5 } * z;
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
        if solid && st.dark && ks.glow > 0.0 {
            // Glow underlay: the same arc, wide and faint.
            p.add(Shape::line(
                arc_pts(c, track, 0.0, k.value),
                Stroke::new(
                    6.0 * z,
                    alpha(r.accent, (40.0 * ks.glow.min(1.0) / 0.4) as u8),
                ),
            ));
        }
        p.add(Shape::line(
            arc_pts(c, track, 0.0, k.value),
            Stroke::new(2.5 * z, r.accent),
        ));
    }
    let skirt = mix(r.knob, r.knob_hi, 0.25);
    let rim = alpha(mix(r.knob_hi, r.knob, 0.5), 255);
    match ks.body {
        KnobBody::Solid => {
            // Dark filled cap: drop shadow, skirt with ticks, four-step cap gradient, pointer.
            let sh = if st.dark { 115 } else { 36 };
            p.circle_filled(
                c + vec2(0.0, 3.0 * z),
                rs + z,
                alpha(Color32::BLACK, sh / 4),
            );
            p.circle_filled(
                c + vec2(0.0, 1.5 * z),
                rs + 0.5 * z,
                alpha(Color32::BLACK, sh / 2),
            );
            p.circle_filled(c, rs, mix(r.knob, Color32::BLACK, 0.3));
            for i in 0..=10 {
                let a = travel_angle(i as f32 / 10.0);
                p.line_segment(
                    [on_circle(c, rs - 1.2 * z, a), on_circle(c, rs - 3.6 * z, a)],
                    Stroke::new((1.0 * z).max(0.75), alpha(r.knob_ink, 90)),
                );
            }
            for i in 0..4 {
                let t = i as f32 / 3.0;
                p.circle_filled(
                    c - vec2(rs * 0.06, rs * 0.08) * (1.0 - t),
                    rs * (0.74 - 0.14 * t),
                    mix(r.knob_hi, r.knob, t),
                );
            }
            let a = travel_angle(k.value);
            p.line_segment(
                [on_circle(c, rs * 0.18, a), on_circle(c, rs * 0.68, a)],
                Stroke::new((if rs > 18.0 * z { 2.4 } else { 2.0 }) * z, r.knob_ink),
            );
        }
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
    if solid {
        return;
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
#[allow(clippy::too_many_arguments)]
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
    jack_ch(p, st, c, r, z, signal, patched, hot, false);
}

/// [`jack`] with the channel count: a stereo jack of the `Hex` recipe shows a double ring.
#[allow(clippy::too_many_arguments)]
pub fn jack_ch(
    p: &Painter,
    st: &Style,
    c: Pos2,
    r: f32,
    z: f32,
    signal: Color32,
    patched: bool,
    hot: bool,
    stereo: bool,
) {
    let ro = &st.roles;
    let nut = mix(ro.knob, ro.knob_hi, 0.35);
    let edge = mix(ro.knob, ro.knob_hi, 0.8);
    match st.slots.jack {
        JackRecipe::Hex => {
            // Hex nut (r 11 in rack units, scaled with the jack), signal ring, hole.
            let k = r / 13.0;
            let hex = |rad: f32, dy: f32| -> Vec<Pos2> {
                (0..6)
                    .map(|i| {
                        let a =
                            std::f32::consts::FRAC_PI_6 + i as f32 * std::f32::consts::FRAC_PI_3;
                        c + vec2(a.cos() * rad, a.sin() * rad + dy) * k
                    })
                    .collect()
            };
            let sh = if st.dark { 115 } else { 36 };
            p.add(Shape::convex_polygon(
                hex(11.5, 1.2),
                alpha(Color32::BLACK, sh / 2),
                Stroke::NONE,
            ));
            p.add(Shape::convex_polygon(
                hex(11.0, 0.0),
                st.roles.raised,
                Stroke::new((0.8 * z).max(0.75), st.roles.line2),
            ));
            let ring = (1.7 * z).max(1.0);
            if stereo {
                p.circle_filled(c, 8.1 * k, st.roles.hole);
                p.circle_stroke(c, 8.1 * k, Stroke::new(ring, signal));
                p.circle_stroke(c, 5.0 * k, Stroke::new(ring, signal));
                p.circle_filled(c, 2.4 * k, st.roles.hole);
            } else {
                p.circle_filled(c, 7.6 * k, signal);
                p.circle_filled(c, 5.2 * k, st.roles.hole);
            }
            if patched {
                p.circle_filled(c + vec2(0.0, 1.5 * k), 8.0 * k, alpha(Color32::BLACK, 90));
                p.circle_filled(c, 7.5 * k, signal);
                p.circle_filled(c - vec2(2.0, 2.0) * k, 3.0 * k, alpha(Color32::WHITE, 90));
            }
        }
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

/// A shadow strip along one edge of `rect` (0 top, 1 left, 2 right), `depth` deep, fading from
/// alpha `a` at the edge to nothing.
fn fade(p: &Painter, rect: Rect, side: usize, depth: f32, a: u8) {
    let (c0, c1) = (alpha(Color32::BLACK, a), alpha(Color32::BLACK, 0));
    let (l, r, t, b) = (rect.left(), rect.right(), rect.top(), rect.bottom());
    let q = match side {
        0 => [
            (pos2(l, t), c0),
            (pos2(r, t), c0),
            (pos2(r, t + depth), c1),
            (pos2(l, t + depth), c1),
        ],
        1 => [
            (pos2(l, t), c0),
            (pos2(l, b), c0),
            (pos2(l + depth, b), c1),
            (pos2(l + depth, t), c1),
        ],
        _ => [
            (pos2(r, t), c0),
            (pos2(r, b), c0),
            (pos2(r - depth, b), c1),
            (pos2(r - depth, t), c1),
        ],
    };
    let mut mesh = Mesh::default();
    for (v, c) in q {
        mesh.colored_vertex(v, c);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(Shape::mesh(mesh));
}

/// The frame of a live display, seated into the face colour `face`: a tinted glass with a
/// bezel, a lit lower lip, an inner shadow at the top and sides and a faint sheen. Paper
/// themes keep the flat sheet.
pub fn display_frame(p: &Painter, st: &Style, rect: Rect, face: Color32, z: f32) {
    let r = &st.roles;
    let cr = CornerRadius::same((3.0 * z).clamp(2.0, 6.0) as u8);
    if st.slots.display.frame == DisplayFrame::Paper {
        // Paper wells are near white on light themes; on dark themes they stay the dark inset.
        let fill = if st.dark {
            r.disp_bg
        } else {
            mix(r.disp_bg, Color32::WHITE, 0.82)
        };
        p.rect_filled(rect, cr, fill);
        p.rect_stroke(rect, cr, Stroke::new(1.0, r.line2), StrokeKind::Inside);
    } else {
        // Lower lip: the panel's edge catching the light under the cut.
        p.rect_stroke(
            rect.translate(vec2(0.0, 1.0)).expand(1.0),
            cr,
            Stroke::new(1.0, alpha(Color32::WHITE, 60)),
            StrokeKind::Outside,
        );
        p.rect_filled(rect, cr, mix(r.disp_bg, face, 0.12));
        fade(p, rect, 0, (7.0 * z).max(3.0), 120);
        fade(p, rect, 1, (4.0 * z).max(2.0), 70);
        fade(p, rect, 2, (4.0 * z).max(2.0), 70);
        let (w, h) = (rect.width(), rect.height());
        let mut sheen = Mesh::default();
        sheen.colored_vertex(rect.left_top(), alpha(Color32::WHITE, 14));
        sheen.colored_vertex(
            rect.left_top() + vec2(w * 0.5, 0.0),
            alpha(Color32::WHITE, 0),
        );
        sheen.colored_vertex(
            rect.left_top() + vec2(0.0, h * 0.6),
            alpha(Color32::WHITE, 0),
        );
        sheen.add_triangle(0, 1, 2);
        p.add(Shape::mesh(sheen));
        // Bezel.
        p.rect_stroke(
            rect,
            cr,
            Stroke::new(2.0, alpha(mix(face, Color32::BLACK, 0.7), 230)),
            StrokeKind::Inside,
        );
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
    if st.slots.display.frame == DisplayFrame::Paper && !st.dark {
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

/// The ink of a face title: the module family's colour for line styles with a coloured-title
/// family marker, else the plain face `ink`.
pub fn family_ink(st: &Style, kind: &str, ink: Color32) -> Color32 {
    let f = &st.slots.face;
    if f.material == FaceMaterial::Line && f.family == crate::style::FamilyMarker::Title {
        st.section_of(kind).ink
    } else {
        ink
    }
}

/// The 3 px family stripe along the top edge of a face (`FamilyMarker::Stripe`).
pub fn family_stripe(p: &Painter, st: &Style, face: Rect, kind: &str, z: f32) {
    if st.slots.face.family == crate::style::FamilyMarker::Stripe {
        let r = Rect::from_min_size(face.min + vec2(z, z), vec2(face.width() - 2.0 * z, 3.0 * z));
        p.rect_filled(r, CornerRadius::same(1), st.section_of(kind).ink);
    }
}
