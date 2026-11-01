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

/// Operator graph: solid boxes 6..1 left to right, strokes 1.5 px in the display ink.
/// - A modulation edge between neighbours is a short arrow, box edge to box edge.
/// - A target with several modulators (fan-in) and a modulator with several targets (fan-out)
///   use one bus line under the row: short drops, ONE arrowhead per target.
/// - Any other edge is a curve over (even source) or under (odd source) the row.
/// - Carriers are filled with the accent; operator 6's feedback is a small arc over its box.
/// Nothing is drawn across a box.
pub(crate) fn graph(p: &egui::Painter, st: &crate::style::Style, xf: Xf, well: Rect, alg: usize) {
    let ro = &st.roles;
    let z = xf.zoom;
    let a = &ALGORITHMS[alg];
    let (bw, pitch) = (14.0, 30.0);
    let x0 = well.center().x - 2.5 * pitch;
    let cy = well.top() + 44.0;
    let (top, bottom) = (cy - bw / 2.0, cy + bw / 2.0);
    let bus_y = bottom + 9.0;
    let x_of = |op: usize| x0 + (6 - op) as f32 * pitch;
    let line = Stroke::new(1.5 * z.max(0.7), ro.disp_text);
    let seg = |a: Pos2, b: Pos2| {
        p.line_segment([xf.p(a), xf.p(b)], line);
    };
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
    // Edges (source op, target op), numbers 1..6; each is drawn once.
    let mut edges: Vec<(usize, usize)> = (0..6usize)
        .flat_map(|k| (k + 1..6).map(move |j| (j, k)))
        .filter(|&(j, k)| a.mods[k] >> j & 1 == 1)
        .map(|(j, k)| (j + 1, k + 1))
        .collect();
    // Fan-in: a target with two or more modulators. One bus, one arrowhead.
    for t in 1..=6usize {
        let src: Vec<usize> = edges.iter().filter(|e| e.1 == t).map(|e| e.0).collect();
        if src.len() < 2 {
            continue;
        }
        let left = src.iter().map(|&s| x_of(s)).fold(f32::MAX, f32::min);
        for &s in &src {
            seg(pos2(x_of(s), bottom), pos2(x_of(s), bus_y));
        }
        seg(pos2(left, bus_y), pos2(x_of(t), bus_y));
        seg(pos2(x_of(t), bus_y), pos2(x_of(t), bottom + 4.0));
        head(pos2(x_of(t), bottom), vec2(0.0, -1.0));
        edges.retain(|e| e.1 != t);
    }
    // Fan-out: a modulator with two or more targets. One line into the bus, a drop per target.
    for s in 1..=6usize {
        let dst: Vec<usize> = edges.iter().filter(|e| e.0 == s).map(|e| e.1).collect();
        if dst.len() < 2 {
            continue;
        }
        let right = dst.iter().map(|&t| x_of(t)).fold(f32::MIN, f32::max);
        seg(pos2(x_of(s), bottom), pos2(x_of(s), bus_y));
        seg(pos2(x_of(s), bus_y), pos2(right, bus_y));
        for &t in &dst {
            seg(pos2(x_of(t), bus_y), pos2(x_of(t), bottom + 4.0));
            head(pos2(x_of(t), bottom), vec2(0.0, -1.0));
        }
        edges.retain(|e| e.0 != s);
    }
    // Single edges: neighbours get an arrow, others a curve over or under the row.
    for &(j, k) in &edges {
        let (xs, xt) = (x_of(j), x_of(k));
        if j == k + 1 {
            seg(pos2(xs + bw / 2.0, cy), pos2(xt - bw / 2.0 - 3.0, cy));
            head(pos2(xt - bw / 2.0, cy), vec2(1.0, 0.0));
            continue;
        }
        // Even sources go over the row, odd ones under it; ends sit 4 px off the box centre.
        let up = if j % 2 == 0 { -1.0 } else { 1.0 };
        let edge_y = if up < 0.0 { top } else { bottom };
        let d = 2.9 * (j - k) as f32 * up;
        let s = pos2(xs + 4.0, edge_y);
        let t = pos2(xt - 4.0, edge_y + 3.0 * up);
        curve(s, s + vec2(0.0, d), t + vec2(0.0, d), t);
        head(pos2(t.x, edge_y), vec2(0.0, -up));
    }
    for op in 1..=6usize {
        let r = Rect::from_center_size(xf.p(pos2(x_of(op), cy)), vec2(bw, bw) * z);
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
    // Feedback of operator 6: a small arc over the left half of its box, arrowhead into the box.
    let f = pos2(x_of(6), top);
    curve(
        f + vec2(-5.0, 0.0),
        f + vec2(-7.0, -9.0),
        f + vec2(1.0, -9.0),
        f + vec2(-1.0, -3.0),
    );
    head(f + vec2(-1.0, 0.0), vec2(0.0, 1.0));
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
