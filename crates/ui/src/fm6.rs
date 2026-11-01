//! The FM voice's picture parts: the algorithm well (operator graph), the six operator cards
//! (title, carrier dot) and the editable envelope curves. Layout lives in `rack::osc`; knobs
//! and selectors are ordinary controls. A curve writes the operator's attack, decay, sustain
//! and release params, so save and undo work as for knobs (one drag undoes as one step).

use egui::{pos2, vec2, Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke};
use kabl_core::ParamTarget;
use kabl_modules::builtins::ALGORITHMS;
use kabl_modules::ParamInfo;

use crate::editor::PatchEditor;
use crate::kit::face;
use crate::rack::{self, Placed};
use crate::style::{alpha, Role};
use crate::{routing, UiState, Xf};

/// Fraction of the curve's width held at the sustain level.
const HOLD: f32 = 0.14;
/// Pixels (world units) around the curve that its handles can sit on.
const PAD: f32 = 3.0;
/// Handle grab radius, world units.
const GRAB: f32 = 9.0;

fn find(m: &Placed, name: &str) -> Option<&'static ParamInfo> {
    m.info.params.iter().find(|p| p.name == name)
}

/// The curve area inside operator card `b`.
pub fn curve_rect(b: Rect) -> Rect {
    Rect::from_min_max(
        pos2(b.left() + 6.0, b.top() + 124.0),
        b.max - vec2(6.0, 6.0),
    )
}

/// Algorithm well: the box 16 px left of the cards, 196 wide.
pub fn well_rect(cards: Rect) -> Rect {
    Rect::from_min_size(cards.left_top() - vec2(212.0, 0.0), vec2(196.0, 68.0))
}

/// Handle positions (world units): attack peak, decay end, sustain end, release end.
fn handles(r: Rect, n: [f32; 4]) -> [Pos2; 4] {
    let seg = seg_w(r);
    let (top, bottom) = (r.top() + PAD, r.bottom() - PAD);
    let xa = r.left() + PAD + seg * n[0];
    let xd = xa + seg * n[1];
    let xs = xd + HOLD * r.width();
    let ys = bottom - (bottom - top) * n[2];
    [
        pos2(xa, top),
        pos2(xd, ys),
        pos2(xs, ys),
        pos2(xs + seg * n[3], bottom),
    ]
}

/// Width of a full-scale time segment.
fn seg_w(r: Rect) -> f32 {
    (r.width() - 2.0 * PAD - HOLD * r.width()) / 3.0
}

/// Draws the well, the cards and the curves of `m`, and handles the curve drags.
pub(crate) fn draw(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    xf: Xf,
    m: &Placed,
    cards: Rect,
) {
    let st = ui_state.style.clone();
    let z = xf.zoom;
    let ro = &st.roles;
    let alg = routing::base_value(editor.state(), m.id, find(m, "algorithm").unwrap())
        .round()
        .clamp(0.0, 7.0) as usize;
    let carriers = ALGORITHMS[alg].carriers;
    // Algorithm well.
    let well = well_rect(cards);
    let cr = CornerRadius::same((3.0 * z).clamp(2.0, 6.0) as u8);
    painter.rect_filled(xf.r(well), cr, ro.disp_bg);
    painter.rect_stroke(
        xf.r(well),
        cr,
        Stroke::new(1.0, ro.line2),
        egui::StrokeKind::Inside,
    );
    graph(painter, &st, xf, well, alg);
    // Oversample caption beside its selector.
    if let Some(font) = face::face_font(&st, Role::Label, z) {
        painter.text(
            xf.p(pos2(well.left(), cards.bottom() - 18.0)),
            egui::Align2::LEFT_CENTER,
            "OVERSAMPLE",
            font,
            ro.text2,
        );
    }
    // Operator cards.
    for (i, b) in rack::fm6_blocks(cards).into_iter().enumerate() {
        let op = i + 1;
        painter.rect_stroke(
            xf.r(b),
            CornerRadius::same(5),
            Stroke::new(1.0, ro.line2),
            egui::StrokeKind::Inside,
        );
        if let Some(font) = face::face_font(&st, Role::Label, z) {
            painter.text(
                xf.p(b.left_top() + vec2(8.0, 11.0)),
                egui::Align2::LEFT_CENTER,
                format!("OP {op}"),
                font,
                ro.text,
            );
        }
        let dot = xf.p(b.right_top() + vec2(-11.0, 11.0));
        let carrier = carriers >> i & 1 == 1;
        if carrier {
            painter.circle_filled(dot, 3.5 * z, ro.accent);
        } else {
            painter.circle_stroke(dot, 3.0 * z, Stroke::new(1.0, ro.text3));
        }
        curve(editor, ui_state, ui, painter, xf, m, op, curve_rect(b));
    }
}

/// Operator graph: solid boxes 6..1 left to right. A modulation edge between neighbours is a
/// short arrow from box edge to box edge; any other edge is a curve under the row that leaves a
/// box's bottom and arrives at the target's bottom. Carriers are filled with the accent;
/// operator 6's feedback is a small arc over it. Strokes are 1.5 px in the display ink.
pub(crate) fn graph(p: &egui::Painter, st: &crate::style::Style, xf: Xf, well: Rect, alg: usize) {
    let ro = &st.roles;
    let z = xf.zoom;
    let a = &ALGORITHMS[alg];
    let (bw, pitch) = (14.0, 30.0);
    let x0 = well.center().x - 2.5 * pitch;
    let cy = well.top() + 44.0;
    let at = |op: usize| pos2(x0 + (6 - op) as f32 * pitch, cy);
    let line = Stroke::new(1.5 * z.max(0.7), ro.disp_text);
    // Filled arrowhead with its tip at `tip` (world), pointing along `dir`.
    let head = |tip: Pos2, dir: egui::Vec2| {
        let n = vec2(-dir.y, dir.x);
        p.add(egui::Shape::convex_polygon(
            vec![
                xf.p(tip),
                xf.p(tip - dir * 4.5 + n * 2.6),
                xf.p(tip - dir * 4.5 - n * 2.6),
            ],
            ro.disp_text,
            Stroke::NONE,
        ));
    };
    let curve = |p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2| {
        let pts: Vec<Pos2> = (0..=16)
            .map(|i| {
                let t = i as f32 / 16.0;
                let u = 1.0 - t;
                let q = p0.to_vec2() * (u * u * u)
                    + p1.to_vec2() * (3.0 * u * u * t)
                    + p2.to_vec2() * (3.0 * u * t * t)
                    + p3.to_vec2() * (t * t * t);
                xf.p(q.to_pos2())
            })
            .collect();
        p.add(egui::Shape::line(pts, line));
    };
    // Edges as (source op, target op), numbers 1..6.
    let edges: Vec<(usize, usize)> = (0..6usize)
        .flat_map(|k| (k + 1..6).map(move |j| (j, k)))
        .filter(|&(j, k)| a.mods[k] >> j & 1 == 1)
        .map(|(j, k)| (j + 1, k + 1))
        .collect();
    let far: Vec<(usize, usize)> = edges.iter().copied().filter(|&(j, k)| j != k + 1).collect();
    // Arcs sharing a box end are spread along its bottom so they do not land on one point.
    let spread = |n: usize, i: usize| (i as f32 - (n as f32 - 1.0) / 2.0) * 3.5;
    let rank = |same: &dyn Fn(&(usize, usize)) -> bool, e: &(usize, usize)| {
        let all: Vec<&(usize, usize)> = far.iter().filter(|x| same(x)).collect();
        (all.len(), all.iter().position(|x| *x == e).unwrap_or(0))
    };
    for e @ &(j, k) in &edges {
        let (from, to) = (at(j), at(k));
        if j == k + 1 {
            let (s, t) = (from.x + bw / 2.0, to.x - bw / 2.0);
            p.line_segment([xf.p(pos2(s, cy)), xf.p(pos2(t - 3.0, cy))], line);
            head(pos2(t, cy), vec2(1.0, 0.0));
        } else {
            let (ns, is) = rank(&|x| x.0 == j, e);
            let (nt, it) = rank(&|x| x.1 == k, e);
            let bottom = cy + bw / 2.0;
            let s = pos2(from.x + spread(ns, is), bottom);
            let t = pos2(to.x + spread(nt, it), bottom + 3.0);
            // Controls straight below the ends: the curve leaves and arrives vertically.
            let d = 2.9 * (j - k) as f32;
            curve(s, s + vec2(0.0, d), t + vec2(0.0, d), t);
            head(pos2(t.x, bottom), vec2(0.0, -1.0));
        }
    }
    for op in 1..=6usize {
        let r = Rect::from_center_size(xf.p(at(op)), vec2(bw, bw) * z);
        let carrier = a.carriers >> (op - 1) & 1 == 1;
        let fill = if carrier {
            ro.accent
        } else {
            crate::style::mix(ro.disp_bg, ro.disp_text, 0.16)
        };
        p.rect_filled(r, 3.0 * z, fill);
        p.rect_stroke(
            r,
            3.0 * z,
            Stroke::new(
                1.5 * z.max(0.7),
                if carrier { ro.accent } else { ro.disp_text },
            ),
            egui::StrokeKind::Inside,
        );
        if let Some(font) = face::face_font(st, Role::Value, z) {
            p.text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                op.to_string(),
                font,
                if carrier { ro.on_accent } else { ro.disp_text },
            );
        }
    }
    // Feedback of operator 6: a small arc over its box, arrowhead back into the box.
    let f = at(6) + vec2(0.0, -bw / 2.0);
    curve(
        f + vec2(-3.5, 0.0),
        f + vec2(-6.5, -9.0),
        f + vec2(6.5, -9.0),
        f + vec2(3.5, -3.0),
    );
    head(f + vec2(3.5, 0.0), vec2(0.0, 1.0));
}

/// One operator's envelope curve with four draggable handles.
#[allow(clippy::too_many_arguments)]
fn curve(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    xf: Xf,
    m: &Placed,
    op: usize,
    r: Rect,
) {
    let st = ui_state.style.clone();
    let z = xf.zoom;
    let names = ["attack", "decay", "sustain", "release"].map(|n| format!("{n}{op}"));
    let ps: Vec<&'static ParamInfo> = names.iter().filter_map(|n| find(m, n)).collect();
    if ps.len() != 4 {
        return;
    }
    let norm = |editor: &PatchEditor| {
        std::array::from_fn(|k| ps[k].to_norm(routing::base_value(editor.state(), m.id, ps[k])))
    };
    let id = Id::new(("kabl-fm6-env", m.id, op));
    let hit = xf.r(r);
    let resp = ui.interact(hit, id, Sense::click_and_drag());
    let held = Id::new(("kabl-fm6-handle", m.id, op));
    let mut n: [f32; 4] = norm(editor);
    if resp.drag_started() {
        let origin = ui.input(|i| i.pointer.press_origin()).map(|p| xf.inv(p));
        let pick = origin.and_then(|o| {
            handles(r, n)
                .iter()
                .enumerate()
                .map(|(k, h)| (k, h.distance(o)))
                .filter(|(_, d)| *d <= GRAB)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(k, _)| k)
        });
        ui.memory_mut(|mem| mem.data.insert_temp(held, pick));
        if pick.is_some() {
            ui_state.drag = None;
        }
    }
    let grabbed: Option<usize> =
        ui.memory(|mem| mem.data.get_temp::<Option<usize>>(held).flatten());
    if resp.dragged() {
        if let (Some(k), Some(at)) = (grabbed, ui.input(|i| i.pointer.latest_pos())) {
            let at = xf.inv(at);
            let (seg, top, bottom) = (seg_w(r), r.top() + PAD, r.bottom() - PAD);
            let h = handles(r, n);
            let nx = |from: f32| ((at.x - from) / seg).clamp(0.0, 1.0);
            let ny = ((bottom - at.y) / (bottom - top)).clamp(0.0, 1.0);
            let mut set: Vec<(usize, f32)> = Vec::new();
            match k {
                0 => set.push((0, nx(r.left() + PAD))),
                1 => {
                    set.push((1, nx(h[0].x)));
                    set.push((2, ny));
                }
                2 => set.push((2, ny)),
                _ => set.push((3, nx(h[2].x))),
            }
            let first = resp.drag_started()
                || !ui.memory(|mem| mem.data.get_temp::<bool>(held.with("on")).unwrap_or(false));
            ui.memory_mut(|mem| mem.data.insert_temp(held.with("on"), true));
            for (j, (idx, v)) in set.into_iter().enumerate() {
                editor.set_param_group(
                    ParamTarget::Module {
                        id: m.id,
                        param: ps[idx].name.into(),
                    },
                    ps[idx].from_norm(v),
                    !(first && j == 0),
                );
                n[idx] = v;
            }
        }
    }
    if resp.drag_stopped() {
        ui.memory_mut(|mem| {
            mem.data.remove::<bool>(held.with("on"));
            mem.data.insert_temp(held, None::<usize>);
        });
    }
    // Drawing.
    let n: [f32; 4] = norm(editor);
    let ro = &st.roles;
    painter.rect_filled(hit, CornerRadius::same(3), ro.disp_bg);
    painter.rect_stroke(
        hit,
        CornerRadius::same(3),
        Stroke::new(1.0, ro.line2),
        egui::StrokeKind::Inside,
    );
    let h = handles(r, n);
    let (bottom, left) = (r.bottom() - PAD, r.left() + PAD);
    let pts: Vec<Pos2> = [pos2(left, bottom), h[0], h[1], h[2], h[3]]
        .into_iter()
        .map(|q| xf.p(q))
        .collect();
    face::trace(painter, &st, pts, z, Some(xf.p(pos2(0.0, bottom)).y));
    let hot = resp.hovered() || resp.dragged();
    for (k, q) in h.iter().enumerate() {
        let on = grabbed == Some(k) && resp.dragged();
        painter.circle_filled(
            xf.p(*q),
            (if on { 3.6 } else { 2.6 }) * z.max(0.7),
            if on {
                ro.accent
            } else if hot {
                ro.disp_text
            } else {
                alpha(ro.disp_text, 150)
            },
        );
    }
    if hot {
        let v = |k: usize| routing::fmt_value(ps[k], ps[k].from_norm(n[k]));
        resp.on_hover_text(format!("A {}  D {}  S {}  R {}", v(0), v(1), v(2), v(3)));
    }
    let _: Color32 = ro.accent;
}
