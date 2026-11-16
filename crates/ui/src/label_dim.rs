//! Labels vs cables (F2b): text is painted on the face, cables lie on top as one continuous line,
//! and where a cable crosses a text rect its stretch dims so the text reads through.
//!
//! The cable code paints plain strokes and one glow mesh; this pass runs over the shapes it added
//! and rewrites them: a polyline is cut into pieces of (quantised) equal opacity, a mesh has its
//! vertex colours scaled. Only shapes whose bounds meet a text rect are touched.

use egui::epaint::{ColorMode, PathShape, Shape};
use egui::{Color32, Pos2, Rect};

/// Opacity of a cable over the middle of a text rect.
pub const MIN: f32 = 0.3;
/// Width of the soft edge around a text rect, in px.
pub const FEATHER: f32 = 4.0;
/// Opacity steps between `MIN` and 1 (a polyline piece is one step).
const STEPS: f32 = 4.0;

/// Which variant draws (`KABL_LABELS=dim|top`; anything else is `dim`; temporary, until Kosta picks).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Dim,
    Top,
}

pub fn mode() -> Mode {
    static MODE: std::sync::OnceLock<Mode> = std::sync::OnceLock::new();
    *MODE.get_or_init(|| {
        if std::env::var("KABL_LABELS").is_ok_and(|v| v == "top") {
            Mode::Top
        } else {
            Mode::Dim
        }
    })
}

/// The opacity factor at `p`: `MIN` inside a rect, rising to 1 over `FEATHER` px outside it.
pub fn factor(rects: &[Rect], p: Pos2) -> f32 {
    let d = rects
        .iter()
        .map(|r| r.distance_to_pos(p))
        .fold(f32::MAX, f32::min);
    MIN + (1.0 - MIN) * (d / FEATHER).clamp(0.0, 1.0)
}

fn quantise(f: f32) -> f32 {
    let step = (1.0 - MIN) / STEPS;
    (MIN + step * ((f - MIN) / step).round()).clamp(MIN, 1.0)
}

fn scaled(c: Color32, f: f32) -> Color32 {
    if f >= 1.0 {
        c
    } else {
        c.gamma_multiply(f)
    }
}

/// Dims every shape in `shapes` that crosses one of `rects`.
pub fn dim(shapes: &mut [Shape], rects: &[Rect]) {
    if rects.is_empty() {
        return;
    }
    for s in shapes {
        dim_shape(s, rects);
    }
}

fn dim_shape(s: &mut Shape, rects: &[Rect]) {
    let reach = |bounds: Rect, rects: &[Rect]| -> Vec<Rect> {
        let b = bounds.expand(FEATHER);
        rects.iter().copied().filter(|r| r.intersects(b)).collect()
    };
    match s {
        Shape::Vec(v) => v.iter_mut().for_each(|s| dim_shape(s, rects)),
        Shape::Path(p) if !p.closed => {
            let ColorMode::Solid(col) = p.stroke.color else {
                return;
            };
            let near = reach(Rect::from_points(&p.points).expand(p.stroke.width), rects);
            if near.is_empty() {
                return;
            }
            let pieces = split(&p.points, &near);
            *s = Shape::Vec(
                pieces
                    .into_iter()
                    .map(|(pts, f)| {
                        let mut piece = PathShape::line(pts, p.stroke.clone());
                        piece.stroke.color = ColorMode::Solid(scaled(col, f));
                        Shape::Path(piece)
                    })
                    .collect(),
            );
        }
        Shape::Mesh(m) => {
            let b = Rect::from_points(&m.vertices.iter().map(|v| v.pos).collect::<Vec<_>>());
            let near = reach(b, rects);
            if near.is_empty() {
                return;
            }
            let m = std::sync::Arc::make_mut(m);
            for v in &mut m.vertices {
                v.color = scaled(v.color, factor(&near, v.pos));
            }
        }
        _ => {}
    }
}

/// Cuts `pts` into runs of one quantised opacity. Segments that come near a rect are resampled to
/// about 1.5 px so the feather is smooth; the rest are left whole.
fn split(pts: &[Pos2], near: &[Rect]) -> Vec<(Vec<Pos2>, f32)> {
    let mut out: Vec<(Vec<Pos2>, f32)> = Vec::new();
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let seg = Rect::from_two_pos(a, b).expand(FEATHER);
        let n = if near.iter().any(|r| r.intersects(seg)) {
            ((b - a).length() / 1.5).ceil().max(1.0) as usize
        } else {
            1
        };
        for k in 0..n {
            let p = a + (b - a) * (k as f32 / n as f32);
            let q = a + (b - a) * ((k + 1) as f32 / n as f32);
            let f = quantise(factor(near, p + (q - p) * 0.5));
            match out.last_mut() {
                Some((run, g)) if (*g - f).abs() < 1e-3 => run.push(q),
                _ => out.push((vec![p, q], f)),
            }
        }
    }
    if out.is_empty() {
        out.push((pts.to_vec(), 1.0));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2, Stroke};

    fn rect() -> Rect {
        Rect::from_min_size(pos2(40.0, 0.0), vec2(20.0, 10.0))
    }

    #[test]
    fn factor_is_min_inside_one_far_away_and_monotone_between() {
        let r = [rect()];
        assert!((factor(&r, pos2(50.0, 5.0)) - MIN).abs() < 1e-6);
        assert_eq!(factor(&r, pos2(200.0, 5.0)), 1.0);
        let a = factor(&r, pos2(61.0, 5.0));
        let b = factor(&r, pos2(63.0, 5.0));
        assert!(MIN < a && a < b && b < 1.0);
    }

    #[test]
    fn a_line_through_text_keeps_its_ends_and_dims_the_middle() {
        let line = Shape::line(
            vec![pos2(0.0, 5.0), pos2(100.0, 5.0)],
            Stroke::new(3.0, Color32::from_rgb(200, 100, 50)),
        );
        let mut v = vec![line];
        dim(&mut v, &[rect()]);
        let Shape::Vec(pieces) = &v[0] else {
            panic!("not split")
        };
        let alpha_at = |x: f32| {
            pieces.iter().find_map(|s| {
                let Shape::Path(p) = s else { return None };
                let (lo, hi) = (p.points[0].x, p.points.last().unwrap().x);
                let ColorMode::Solid(c) = p.stroke.color else {
                    return None;
                };
                (lo <= x && x <= hi).then_some(c)
            })
        };
        let full = alpha_at(5.0).unwrap();
        let mid = alpha_at(50.0).unwrap();
        assert_eq!(full, Color32::from_rgb(200, 100, 50));
        assert!(mid.r() < 80, "text stretch is dim: {mid:?}");
        // One continuous line: each piece starts where the last ended.
        let ends: Vec<(Pos2, Pos2)> = pieces
            .iter()
            .filter_map(|s| match s {
                Shape::Path(p) => Some((p.points[0], *p.points.last().unwrap())),
                _ => None,
            })
            .collect();
        assert!(ends.windows(2).all(|w| w[0].1 == w[1].0));
        assert_eq!(ends[0].0, pos2(0.0, 5.0));
        assert_eq!(ends.last().unwrap().1, pos2(100.0, 5.0));
    }

    #[test]
    fn a_line_away_from_text_is_untouched() {
        let line = Shape::line(
            vec![pos2(0.0, 80.0), pos2(100.0, 80.0)],
            Stroke::new(3.0, Color32::RED),
        );
        let mut v = vec![line.clone()];
        dim(&mut v, &[rect()]);
        assert_eq!(v[0], line);
    }
}
