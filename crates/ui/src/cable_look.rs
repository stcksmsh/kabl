//! How a patch cable looks: its curve, widths and opacity come from the theme's `cable` and
//! `plug` slots. A resting cable keeps at least [`MIN_CONTRAST`]:1 against the rack and every
//! face colour, so it can be found and its colour read without hovering: on a light theme
//! through a dark casing around the true colour, on a dark theme through a lightened colour.

use crate::style::{CableRecipe, PlugRecipe, Style};
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

/// The rack and every section's face colour: what a cable lies on.
pub fn surfaces(st: &Style) -> [Color32; 7] {
    let s = &st.sections;
    [
        st.roles.rack,
        s.osc.base,
        s.filter.base,
        s.modulation.base,
        s.amp.base,
        s.timing.base,
        s.fx.base,
    ]
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
    } else {
        (col, LIGHT_CASING)
    }
}

/// [`legible`], remembered per colour, opacity and theme.
pub fn legible_cached(ctx: &egui::Context, st: &Style, col: Color32, alpha: f32) -> Color32 {
    use std::hash::{Hash, Hasher};
    let surf = surfaces(st);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    surf.map(|c| c.to_array()).hash(&mut h);
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

/// A hanging patch cable: a cubic with both handles pulled down by the theme's sag, more when
/// longer. `zoom` scales the base sag.
pub fn points(a: Pos2, b: Pos2, zoom: f32, sag: f32) -> Vec<Pos2> {
    let sag = sag * zoom + 0.25 * (b - a).length();
    let (c1, c2) = (a + vec2(0.0, sag), b + vec2(0.0, sag));
    (0..=40)
        .map(|i| {
            let t = i as f32 / 40.0;
            let u = 1.0 - t;
            (a.to_vec2() * u * u * u
                + c1.to_vec2() * 3.0 * u * u * t
                + c2.to_vec2() * 3.0 * u * t * t
                + b.to_vec2() * t * t * t)
                .to_pos2()
        })
        .collect()
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
) {
    let slots = &st.slots.cable;
    if bypass {
        p.extend(Shape::dashed_line(
            pts,
            Stroke::new(w, Color32::from_gray(140).gamma_multiply(alpha)),
            7.0,
            5.0,
        ));
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
            PlugRecipe::Cap => {
                caps.push(Shape::circle_filled(e, w * 1.35, dark));
                caps.push(Shape::circle_filled(e, w * 0.9, col.gamma_multiply(alpha)));
            }
            PlugRecipe::Ring => {
                caps.push(Shape::circle_stroke(
                    e,
                    w * 1.1,
                    Stroke::new(w * 0.6, col.gamma_multiply(alpha)),
                ));
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

    /// Every cable colour the rack uses keeps 3:1 against the rack and every face while at rest,
    /// in each built-in theme and mode: the edge on light themes, the lightened fill on dark.
    #[test]
    fn resting_cables_keep_contrast_on_every_builtin_surface() {
        for dark in [false, true] {
            let st = builtin_a(dark);
            let th = crate::theme::theme(&st);
            let surf = surfaces(&st);
            let mut colours = vec![th.audio, th.cv, th.gate];
            colours.extend(crate::CABLE_COLORS.iter().copied());
            let rest = st.slots.cable.rest_alpha;
            let ctx = egui::Context::default();
            let mut worst = f32::MAX;
            for c in colours {
                let (fill, casing) = resting(&ctx, &st, c, rest);
                let w = if dark {
                    worst_contrast(fill, rest, &surf)
                } else {
                    worst_contrast(Color32::BLACK, casing, &surf)
                };
                worst = worst.min(w);
                assert!(w >= MIN_CONTRAST, "dark={dark} {c:?}: {w:.2}");
            }
            println!("theme A dark={dark}: worst resting cable contrast {worst:.2}:1");
        }
    }
}
