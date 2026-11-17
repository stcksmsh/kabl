//! How a patch cable looks: its curve, widths and opacity come from the theme's `cable` and
//! `plug` slots. A resting cable keeps at least [`MIN_CONTRAST`]:1 against the rack and every
//! face colour, so it can be found and its colour read without hovering: on a light theme
//! through a dark casing around the true colour, on a dark theme through a lightened colour.

use crate::style::{CableRecipe, CableStyle, PlugRecipe, Style};
use egui::{vec2, Color32, Id, Painter, Pos2, Shape, Stroke};

/// WCAG contrast a cable keeps against the surfaces it crosses.
pub const MIN_CONTRAST: f32 = 3.0;

fn lin(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(c: Color32) -> f32 {
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

fn contrast(a: Color32, b: Color32) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// Faces dark enough to need a lightened cable fill while the app theme (and so the rack) is
/// light: look B in the light theme keeps its dark panels.
pub fn dark_panels_on_light_rack(st: &Style) -> bool {
    !st.dark && luminance(st.sections.osc.base) < 0.2
}

/// The rack and every section's face colour: what a cable lies on.
pub fn surfaces(st: &Style) -> Vec<Color32> {
    let s = &st.sections;
    let f = &st.slots.face;
    let mut v = Vec::new();
    // Dark panels on a light rack (look B in the light app theme): the fill is kept legible on
    // the panels, and the dark casing carries the cable over the light rack.
    if !dark_panels_on_light_rack(st) {
        v.push(st.roles.rack);
    }
    v.extend([
        s.osc.base,
        s.filter.base,
        s.modulation.base,
        s.amp.base,
        s.timing.base,
        s.fx.base,
    ]);
    // Looks A and B shade the face: both ends of the gradient and the output plate too.
    v.extend([f.top, f.bottom, f.plate].into_iter().filter(|c| c.a() > 0));
    v
}

/// The lowest contrast of `col` drawn at `alpha` against any of `surfaces`.
pub fn worst_contrast(col: Color32, alpha: f32, surfaces: &[Color32]) -> f32 {
    surfaces
        .iter()
        .map(|&s| contrast(s.lerp_to_gamma(col, alpha), s))
        .fold(f32::MAX, f32::min)
}

/// `col` moved toward black or white, as little as it takes, until it keeps [`MIN_CONTRAST`] on
/// every surface at opacity `alpha`. The hue stays, so the colour still identifies the source.
pub fn legible(col: Color32, alpha: f32, surfaces: &[Color32]) -> Color32 {
    let mut best = (worst_contrast(col, alpha, surfaces), col);
    if best.0 >= MIN_CONTRAST {
        return col;
    }
    let mut reached: Option<(f32, Color32)> = None;
    for end in [Color32::BLACK, Color32::WHITE] {
        for i in 1..=19 {
            let t = i as f32 * 0.05;
            let c = col.lerp_to_gamma(end, t);
            let w = worst_contrast(c, alpha, surfaces);
            if w >= MIN_CONTRAST {
                if reached.is_none_or(|(rt, _)| t < rt) {
                    reached = Some((t, c));
                }
                break;
            }
            if w > best.0 {
                best = (w, c);
            }
        }
    }
    reached.map_or(best.1, |(_, c)| c)
}

/// Opacity of the black casing round a resting cable on a light theme.
pub const LIGHT_CASING: f32 = 0.55;

/// The fill colour and casing opacity (0..=1) a resting cable gets in this theme.
pub fn resting(ctx: &egui::Context, st: &Style, col: Color32, alpha: f32) -> (Color32, f32) {
    if st.dark {
        (legible_cached(ctx, st, col, alpha), 0.35)
    } else if dark_panels_on_light_rack(st) {
        (legible_cached(ctx, st, col, alpha), LIGHT_CASING)
    } else {
        (col, LIGHT_CASING)
    }
}

/// [`legible`], remembered per colour, opacity and theme.
pub fn legible_cached(ctx: &egui::Context, st: &Style, col: Color32, alpha: f32) -> Color32 {
    use std::hash::{Hash, Hasher};
    let surf = surfaces(st);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    surf.iter()
        .map(|c| c.to_array())
        .collect::<Vec<_>>()
        .hash(&mut h);
    let id = Id::new((
        "kabl-cable-legible",
        col.to_array(),
        (alpha * 100.0) as u8,
        h.finish(),
    ));
    if let Some(c) = ctx.data(|d| d.get_temp::<Color32>(id)) {
        return c;
    }
    let c = legible(col, alpha, &surf);
    ctx.data_mut(|d| d.insert_temp(id, c));
    c
}

/// Points to sample a cable whose rope is about `len` screen points long: one per 12, 12 to 41.
pub fn point_count(len: f32) -> usize {
    ((len / 12.0) as usize).clamp(12, 41)
}

/// A hanging patch cable, drawn as the G4 mockup does (`cables()` in `g4-surfaces.html`): a
/// cubic whose handles sit 15 % of the span inwards and hang by the theme's sag plus 0.28 of
/// the length, at most 160. `zoom` scales the base sag and the limit.
pub fn points(a: Pos2, b: Pos2, zoom: f32, sag: f32) -> Vec<Pos2> {
    points_with(a, b, zoom, sag, 0.0)
}

/// The resting sag of a cable in px: what [`points`] hangs it by.
pub fn rest_sag(a: Pos2, b: Pos2, zoom: f32, sag: f32) -> f32 {
    (sag * zoom + 0.28 * (b - a).length()).min(160.0 * zoom)
}

/// `points` with the sag amount moved by `extra` px (the settle spring), never above the ends.
pub fn points_with(a: Pos2, b: Pos2, zoom: f32, sag: f32, extra: f32) -> Vec<Pos2> {
    let d = b - a;
    let sag = (rest_sag(a, b, zoom, sag) + extra).max(0.0);
    let (c1, c2) = (a + vec2(d.x * 0.15, sag), b + vec2(-d.x * 0.15, sag));
    let n = point_count(d.length() + 2.0 * sag);
    (0..n)
        .map(|i| {
            let t = i as f32 / (n - 1) as f32;
            let u = 1.0 - t;
            (a.to_vec2() * u * u * u
                + c1.to_vec2() * 3.0 * u * u * t
                + c2.to_vec2() * 3.0 * u * t * t
                + b.to_vec2() * t * t * t)
                .to_pos2()
        })
        .collect()
}

/// `points`, remembered per `key` (the cable) and reused while its ends, zoom and sag are the
/// same as last frame, so a resting rack tessellates no curves.
pub fn points_cached(
    ctx: &egui::Context,
    key: Id,
    a: Pos2,
    b: Pos2,
    zoom: f32,
    sag: f32,
    extra: f32,
) -> std::sync::Arc<Vec<Pos2>> {
    type Hit = ([Pos2; 2], [f32; 3], std::sync::Arc<Vec<Pos2>>);
    if let Some((e, p, pts)) = ctx.data(|d| d.get_temp::<Hit>(key)) {
        if e == [a, b] && p == [zoom, sag, extra] {
            return pts;
        }
    }
    let pts = std::sync::Arc::new(points_with(a, b, zoom, sag, extra));
    ctx.data_mut(|d| d.insert_temp::<Hit>(key, ([a, b], [zoom, sag, extra], pts.clone())));
    pts
}

/// Distance from `p` to the polyline `pts`.
pub fn distance_to(pts: &[Pos2], p: Pos2) -> f32 {
    pts.windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            let ab = b - a;
            let t = if ab.length_sq() > 0.0 {
                ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0)
            } else {
                0.0
            };
            (a + ab * t - p).length()
        })
        .fold(f32::MAX, f32::min)
}

/// Themed styles only: the glow under a lit cable, drawn for all cables before any core so one
/// cable's halo never covers another's core. `z` is the zoom; `lit` is 0..=1.
pub fn glow(p: &Painter, st: &Style, pts: &[Pos2], col: Color32, alpha: f32, lit: f32, z: f32) {
    let t = &st.slots.cable;
    if t.style == CableStyle::Themed && t.glow_alpha > 0.0 && lit > 0.0 {
        p.add(Shape::line(
            pts.to_vec(),
            Stroke::new(
                t.glow_width * z,
                col.gamma_multiply(t.glow_alpha * lit * alpha),
            ),
        ));
    }
}

/// Themed functional cable: one bead per pattern step along the curve, filled = on, hollow =
/// off, in the signal colour; `current` (the step playing) is drawn in `accent`.
#[allow(clippy::too_many_arguments)]
pub fn beads(
    caps: &mut Vec<Shape>,
    st: &Style,
    pts: &[Pos2],
    col: Color32,
    accent: Color32,
    steps: &[bool],
    current: Option<usize>,
    z: f32,
) {
    let n = steps.len();
    if n == 0 || pts.len() < 2 {
        return;
    }
    let r = 4.0 * z.clamp(0.6, 1.5);
    for (i, &on) in steps.iter().enumerate() {
        let f = (i as f32 + 0.5) / n as f32 * (pts.len() - 1) as f32;
        let k = (f as usize).min(pts.len() - 2);
        let c = pts[k] + (pts[k + 1] - pts[k]) * (f - k as f32);
        let ink = if current == Some(i) { accent } else { col };
        caps.push(Shape::circle_filled(
            c,
            r,
            if on { ink } else { st.roles.surface },
        ));
        caps.push(Shape::circle_stroke(c, r, Stroke::new(1.5, ink)));
    }
}

/// Paints the rope into `p`; its two plugs go to `caps`, which the caller paints above the
/// faces. `casing` (0..=1) is the opacity of the dark edge, `lit` (0..=1) how emphasised the
/// cable is: it drives the halo.
#[allow(clippy::too_many_arguments)]
pub fn paint(
    p: &Painter,
    caps: &mut Vec<Shape>,
    st: &Style,
    pts: &[Pos2],
    col: Color32,
    w: f32,
    alpha: f32,
    casing: f32,
    bypass: bool,
    lit: f32,
    z: f32,
    stereo: bool,
) {
    let slots = &st.slots.cable;
    let themed = slots.style == CableStyle::Themed;
    if bypass {
        p.extend(Shape::dashed_line(
            pts,
            Stroke::new(w, Color32::from_gray(140).gamma_multiply(alpha)),
            7.0,
            5.0,
        ));
    } else if themed {
        // Paint tokens (PLAN §7 Cables): shadow, darkened edge, core, highlight. The glow is
        // drawn earlier by `glow`. A stereo cable is a twin strand with a dark centre line.
        let w = w.max(2.5);
        let ww = if stereo { w * 1.6 } else { w };
        let a8 = |x: f32| (x.clamp(0.0, 1.0) * 255.0) as u8;
        if slots.shadow_alpha > 0.0 {
            let dy = vec2(0.0, slots.shadow_dy * z);
            p.add(Shape::line(
                pts.iter().map(|&q| q + dy).collect(),
                Stroke::new(
                    ww + 2.0,
                    Color32::from_black_alpha(a8(slots.shadow_alpha * alpha)),
                ),
            ));
        }
        let edge = col.lerp_to_gamma(Color32::BLACK, slots.edge);
        p.add(Shape::line(
            pts.to_vec(),
            Stroke::new(ww + 2.0, edge.gamma_multiply(alpha)),
        ));
        let core = col.lerp_to_gamma(Color32::WHITE, 0.25 * lit);
        p.add(Shape::line(
            pts.to_vec(),
            Stroke::new(ww, core.gamma_multiply(alpha)),
        ));
        if stereo {
            p.add(Shape::line(
                pts.to_vec(),
                Stroke::new(1.5, edge.gamma_multiply(alpha)),
            ));
        }
        if lit > 0.0 && slots.highlight > 0.0 {
            p.add(Shape::line(
                pts.to_vec(),
                Stroke::new(
                    1.0,
                    Color32::from_white_alpha(a8(slots.highlight * lit * alpha)),
                ),
            ));
        }
    } else {
        let glow = slots.glow * lit;
        if glow > 0.0 {
            p.add(Shape::line(
                pts.to_vec(),
                Stroke::new(w * 2.6, col.gamma_multiply(0.22 * glow * alpha)),
            ));
        }
        let casing = match slots.recipe {
            CableRecipe::Rope | CableRecipe::Glow => casing,
            CableRecipe::Outlined => (casing * 1.4).min(1.0),
        };
        p.add(Shape::line(
            pts.to_vec(),
            Stroke::new(w + 2.0, Color32::from_black_alpha((casing * 255.0) as u8)),
        ));
        p.add(Shape::line(
            pts.to_vec(),
            Stroke::new(w, col.gamma_multiply(alpha)),
        ));
    }
    let (Some(&a), Some(&b)) = (pts.first(), pts.last()) else {
        return;
    };
    let dark = col
        .lerp_to_gamma(Color32::BLACK, 0.35)
        .gamma_multiply(alpha);
    for e in [a, b] {
        match st.slots.plug {
            PlugRecipe::Cap if themed && !stereo => {
                // The G4 mockup's plug: a shadow 1.2 down (r 7.6, black 35 %), the plug in the
                // signal colour (r 7.2) and its darker centre (r 3.2, 30 % toward black).
                caps.push(Shape::circle_filled(
                    e + vec2(0.0, 1.2 * z),
                    7.6 * z,
                    Color32::from_black_alpha((0.35 * alpha * 255.0) as u8),
                ));
                caps.push(Shape::circle_filled(e, 7.2 * z, col.gamma_multiply(alpha)));
                caps.push(Shape::circle_filled(
                    e,
                    3.2 * z,
                    col.lerp_to_gamma(Color32::BLACK, 0.3).gamma_multiply(alpha),
                ));
            }
            PlugRecipe::Cap => {
                caps.push(Shape::circle_filled(e, w * 1.35, dark));
                caps.push(Shape::circle_filled(e, w * 0.9, col.gamma_multiply(alpha)));
                if themed && stereo {
                    caps.push(Shape::circle_stroke(
                        e,
                        w * 2.1,
                        Stroke::new(w * 0.35, col.gamma_multiply(alpha)),
                    ));
                }
            }
            PlugRecipe::Ring => {
                caps.push(Shape::circle_stroke(
                    e,
                    w * 1.1,
                    Stroke::new(w * 0.6, col.gamma_multiply(alpha)),
                ));
                if themed && stereo {
                    caps.push(Shape::circle_stroke(
                        e,
                        w * 2.1,
                        Stroke::new(w * 0.35, col.gamma_multiply(alpha)),
                    ));
                }
            }
            PlugRecipe::Dot => {
                caps.push(Shape::circle_filled(e, w * 1.0, col.gamma_multiply(alpha)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::builtin_a;

    #[test]
    fn points_are_cached_until_the_inputs_change() {
        let ctx = egui::Context::default();
        let (k, a, b) = (Id::new("c"), Pos2::new(0.0, 0.0), Pos2::new(90.0, 10.0));
        let p = points_cached(&ctx, k, a, b, 1.0, 12.0, 0.0);
        assert!(std::sync::Arc::ptr_eq(
            &p,
            &points_cached(&ctx, k, a, b, 1.0, 12.0, 0.0)
        ));
        let q = points_cached(&ctx, k, a, b, 2.0, 12.0, 0.0);
        assert!(!std::sync::Arc::ptr_eq(&p, &q));
        assert_eq!(*q, points(a, b, 2.0, 12.0));
        // The settle spring's offset is part of the key, and never lifts the rope above its ends.
        let r = points_cached(&ctx, k, a, b, 2.0, 12.0, -15.0);
        assert!(!std::sync::Arc::ptr_eq(&q, &r));
        assert_eq!(*r, points_with(a, b, 2.0, 12.0, -15.0));
        let taut = points_with(a, b, 1.0, 12.0, -1000.0);
        assert!(taut.iter().all(|p| p.y <= a.y.max(b.y) + 1e-3));
    }

    /// Every cable colour the rack uses keeps 3:1 against the rack and every face while at rest,
    /// in each built-in theme and mode: the edge on light themes, the lightened fill on dark.
    #[test]
    fn resting_cables_keep_contrast_on_every_builtin_surface() {
        let looks = [
            ("A", false),
            ("A", true),
            ("B", false),
            ("B", true),
            ("look A", false),
            ("look A", true),
            ("look B", false),
            ("look B", true),
        ];
        for (name, dark) in looks {
            let st = match name {
                "A" => builtin_a(dark),
                "B" => crate::style::builtin_b(dark),
                "look A" => crate::style::line_solid_default(dark),
                _ => crate::style::look_b_default(dark),
            };
            let th = crate::theme::theme(&st);
            let surf = surfaces(&st);
            let mut colours = vec![th.audio, th.cv, th.gate];
            colours.extend(st.route_colors.iter().copied());
            let rest = st.slots.cable.rest_alpha;
            let ctx = egui::Context::default();
            let mut worst = f32::MAX;
            for c in colours {
                let (fill, casing) = resting(&ctx, &st, c, rest);
                let w = if dark {
                    worst_contrast(fill, rest, &surf)
                } else if dark_panels_on_light_rack(&st) {
                    // Dark panels on a light rack: the fill on the panels, the casing on the rack.
                    worst_contrast(fill, rest, &surf).min(worst_contrast(
                        Color32::BLACK,
                        casing,
                        &[st.roles.rack],
                    ))
                } else {
                    worst_contrast(Color32::BLACK, casing, &surf)
                };
                worst = worst.min(w);
                assert!(w >= MIN_CONTRAST, "{name} dark={dark} {c:?}: {w:.2}");
            }
            println!("theme {name} dark={dark}: worst resting cable contrast {worst:.2}:1");
        }
    }
}
