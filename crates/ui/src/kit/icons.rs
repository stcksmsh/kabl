//! Stroke icon set (`Icons::Stroke`), drawn on a 16-unit grid at any size.

use egui::epaint::PathStroke;
use egui::{vec2, Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};
use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ic {
    Play,
    Stop,
    Undo,
    Redo,
    Search,
    Star,
    StarFill,
    Plus,
    Minus,
    Fit,
    Focus,
    Gear,
    Down,
    Right,
    Close,
    Folder,
    Cable,
    Bolt,
    Save,
    Note,
    Link,
    Panel,
    Learn,
    Warn,
    Check,
    Rec,
    More,
}

fn arc(c: Pos2, r: f32, a0: f32, a1: f32) -> Vec<Pos2> {
    (0..=12)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / 12.0;
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect()
}

pub fn icon(p: &Painter, ic: Ic, c: Pos2, size: f32, col: Color32) {
    let u = size / 16.0;
    let st = Stroke::new((1.4 * u).max(1.2), col);
    let q = |x: f32, y: f32| c + vec2(x, y) * u;
    let line = |pts: &[(f32, f32)]| {
        p.add(Shape::line(pts.iter().map(|&(x, y)| q(x, y)).collect(), st));
    };
    match ic {
        Ic::Play => {
            p.add(Shape::convex_polygon(
                vec![q(-4.0, -6.0), q(6.0, 0.0), q(-4.0, 6.0)],
                col,
                PathStroke::NONE,
            ));
        }
        Ic::Stop => {
            p.rect_filled(Rect::from_center_size(c, vec2(10.0, 10.0) * u), 1.5, col);
        }
        Ic::Rec => {
            p.circle_filled(c, 5.0 * u, col);
        }
        Ic::Undo | Ic::Redo => {
            let f = if ic == Ic::Undo { 1.0 } else { -1.0 };
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
            if ic == Ic::StarFill {
                p.add(Shape::convex_polygon(pts, col, st));
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
        Ic::Focus => {
            p.circle_stroke(c, 5.0 * u, st);
            p.circle_filled(c, 1.6 * u, col);
            for (dx, dy) in [(0.0, -1.0), (0.0, 1.0), (-1.0, 0.0), (1.0, 0.0)] {
                line(&[(dx * 5.0, dy * 5.0), (dx * 8.0, dy * 8.0)]);
            }
        }
        Ic::Gear => {
            p.circle_stroke(c, 2.8 * u, st);
            for i in 0..8 {
                let a = i as f32 * PI / 4.0;
                p.line_segment(
                    [
                        c + vec2(a.cos(), a.sin()) * 5.2 * u,
                        c + vec2(a.cos(), a.sin()) * 7.2 * u,
                    ],
                    Stroke::new(st.width * 1.5, col),
                );
            }
            p.circle_stroke(c, 5.4 * u, st);
        }
        Ic::More => {
            for dx in [-4.5, 0.0, 4.5] {
                p.circle_filled(q(dx, 0.0), 1.2 * u, col);
            }
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
                arc(q(0.0, -1.0), 7.0 * u, PI * 0.8, PI * 0.2),
                st,
            ));
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
                PathStroke::NONE,
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
    }
}
