//! Larger recipes that module faces and the Perform view share: the value-arc knob, the launch
//! pad and the step lane. Every colour, size and shape comes from `Style`.

use super::{ease_to, tone, Tone};
use crate::style::{alpha, mix, KnobBody, KnobScale, KnobValue, Role, Style};
use egui::{pos2, vec2, Color32, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui};

/// The knob sweep: from 135° (bottom left) clockwise through the top to 405° (bottom right).
const A0: f32 = 135.0_f32 * std::f32::consts::PI / 180.0;
const SWEEP: f32 = 270.0_f32 * std::f32::consts::PI / 180.0;

fn arc_points(c: Pos2, r: f32, t0: f32, t1: f32) -> Vec<Pos2> {
    let n = (((t1 - t0).abs() * 28.0) as usize).max(2);
    (0..=n)
        .map(|i| {
            let a = A0 + SWEEP * (t0 + (t1 - t0) * i as f32 / n as f32);
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect()
}

/// Draws a knob with a value arc at `c`, no interaction. `value` and `mod_span` are 0..=1.
/// Reads the theme's knob slots: body, scale and value-marker recipes.
pub fn paint_arc_knob(
    p: &Painter,
    st: &Style,
    c: Pos2,
    size: f32,
    value: f32,
    mod_span: Option<(f32, f32)>,
    hover: f32,
) {
    let r = &st.roles;
    let track_r = size / 2.0 - 2.5;
    let w = (size / 14.0).clamp(2.5, 4.5);
    match st.slots.knob.scale {
        KnobScale::None => {}
        KnobScale::TrackArc | KnobScale::Ticks => {
            p.add(Shape::line(
                arc_points(c, track_r, 0.0, 1.0),
                Stroke::new(w, r.line2),
            ));
        }
    }
    if let Some((a, b)) = mod_span {
        p.add(Shape::line(
            arc_points(c, track_r + w, a.min(b), a.max(b)),
            Stroke::new(w * 0.6, alpha(r.cv, 200)),
        ));
    }
    if value > 0.004 {
        p.add(Shape::line(
            arc_points(c, track_r, 0.0, value),
            Stroke::new(w, mix(r.accent, r.text, hover * 0.15)),
        ));
    }
    let body_r = track_r - w - 3.0;
    p.circle_filled(c, body_r, r.knob);
    match st.slots.knob.body {
        KnobBody::Disc => {}
        KnobBody::Cap | KnobBody::Glass => {
            let k = if st.slots.knob.body == KnobBody::Glass {
                0.5
            } else {
                0.3
            };
            p.circle_filled(
                c + vec2(-body_r * 0.15, -body_r * 0.2),
                body_r * 0.72,
                alpha(r.knob_hi, (255.0 * k) as u8),
            );
        }
    }
    p.circle_stroke(c, body_r, Stroke::new(1.0, alpha(r.knob_hi, 160)));
    let a = A0 + SWEEP * value;
    let dir = vec2(a.cos(), a.sin());
    match st.slots.knob.value {
        KnobValue::Dot => {
            p.circle_filled(c + dir * body_r * 0.68, (size / 18.0).max(2.0), r.knob_ink);
        }
        KnobValue::Arc => {}
        KnobValue::Line | KnobValue::Notch => {
            p.line_segment(
                [c + dir * body_r * 0.25, c + dir * body_r * 0.85],
                Stroke::new((size / 22.0).max(2.0), r.knob_ink),
            );
        }
    }
}

/// A knob with a value arc. Dragging up or right raises it, a double-click calls `reset`
/// (returns true when the caller should apply the default). Returns the new value (0..=1)
/// when the user changed it.
pub fn arc_knob(
    ui: &mut Ui,
    st: &Style,
    value: f32,
    size: f32,
    mod_span: Option<(f32, f32)>,
) -> (Response, Option<f32>) {
    let enabled = ui.is_enabled();
    let (rect, resp) = ui.allocate_exact_size(
        vec2(size, size),
        if enabled {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        },
    );
    let mut out = None;
    if enabled && resp.dragged() {
        let d = resp.drag_delta();
        let fine = ui.input(|i| i.modifiers.shift);
        let per_px = if fine { 1.0 / 800.0 } else { 1.0 / 200.0 };
        let v = (value + (d.x - d.y) * per_px).clamp(0.0, 1.0);
        if v != value {
            out = Some(v);
        }
    }
    let hover = ease_to(
        ui,
        resp.id.with("hover"),
        enabled && (resp.hovered() || resp.dragged()),
        st.motion.hover,
    );
    paint_arc_knob(
        ui.painter(),
        st,
        rect.center(),
        size,
        out.unwrap_or(value),
        mod_span,
        hover,
    );
    if resp.has_focus() {
        ui.painter().circle_stroke(
            rect.center(),
            size / 2.0,
            Stroke::new(st.slots.controls.focus_ring, st.roles.focus),
        );
    }
    (resp, out)
}

/// What a launch pad shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PadState {
    Idle,
    /// Waiting for the launch point.
    Queued,
    Playing,
}

/// A large launch pad. `top` is the small text at the top left (the MIDI binding), `sub` a state
/// word under the title. `progress` (0..=1) draws a bar at the bottom toward the launch point.
#[allow(clippy::too_many_arguments)]
pub fn pad(
    ui: &mut Ui,
    st: &Style,
    size: egui::Vec2,
    title: &str,
    top: Option<&str>,
    sub: Option<&str>,
    state: PadState,
    progress: Option<f32>,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let r = &st.roles;
    let hover = ease_to(ui, resp.id.with("hover"), resp.hovered(), st.motion.hover);
    let radius = st.radius(crate::style::Rad::Md);
    let down = resp.is_pointer_button_down_on();
    let (fill, ink, ink2) = match state {
        PadState::Playing => (r.accent, r.on_accent, alpha(r.on_accent, 200)),
        _ => (
            mix(
                r.raised,
                r.text,
                0.03 + hover * 0.05 + if down { 0.05 } else { 0.0 },
            ),
            r.text,
            r.text2,
        ),
    };
    let p = ui.painter();
    p.rect_filled(rect, radius, fill);
    let stroke = match state {
        PadState::Queued => {
            let t = ui.input(|i| i.time);
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
            let pulse = 0.55 + 0.45 * ((t * 5.0).sin() as f32 * 0.5 + 0.5);
            Stroke::new(2.5, alpha(r.accent, (255.0 * pulse) as u8))
        }
        PadState::Playing => Stroke::new(1.0, r.accent),
        PadState::Idle => Stroke::new(st.slots.controls.outline, r.line),
    };
    p.rect_stroke(rect, radius, stroke, egui::StrokeKind::Inside);
    if let Some(t) = top {
        p.text(
            rect.left_top() + vec2(st.sp(2), st.sp(2)),
            egui::Align2::LEFT_TOP,
            t,
            st.font(Role::Caption),
            ink2,
        );
    }
    let cy = rect.center().y + if sub.is_some() { -6.0 } else { 0.0 };
    let tr = p.layout(
        title.to_string(),
        st.font(Role::H3),
        ink,
        rect.width() - st.sp(2) * 2.0,
    );
    let tpos = pos2(rect.center().x - tr.size().x / 2.0, cy - tr.size().y / 2.0);
    p.galley(tpos, tr, ink);
    if let Some(s) = sub {
        p.text(
            pos2(rect.center().x, cy + 14.0),
            egui::Align2::CENTER_CENTER,
            s.to_uppercase(),
            st.font(Role::Caption),
            if state == PadState::Queued {
                r.accent
            } else {
                ink2
            },
        );
    }
    if let Some(f) = progress {
        let bar = Rect::from_min_size(
            pos2(rect.left() + st.sp(2), rect.bottom() - st.sp(2) - 3.0),
            vec2(rect.width() - st.sp(4), 3.0),
        );
        p.rect_filled(bar, 1.5, alpha(ink, 50));
        p.rect_filled(
            Rect::from_min_size(bar.min, vec2(bar.width() * f.clamp(0.0, 1.0), 3.0)),
            1.5,
            ink,
        );
    }
    resp
}

/// One step of a lane: level and probability, 0..=1, and whether the gate is on.
#[derive(Clone, Copy, Debug)]
pub struct Step {
    pub level: f32,
    pub prob: f32,
    pub gate: bool,
}

/// A live step lane: bar height is the level, brightness the probability, a gate-off step is
/// drawn as a stub and the playing step is outlined.
pub fn step_lane(
    ui: &mut Ui,
    st: &Style,
    size: egui::Vec2,
    steps: &[Step],
    playing: Option<usize>,
    hue: Color32,
) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let r = &st.roles;
    let n = steps.len().max(1);
    let gap = st.sp(1) * 0.75;
    let w = (rect.width() - gap * (n as f32 - 1.0)) / n as f32;
    for (i, s) in steps.iter().enumerate() {
        let x = rect.left() + i as f32 * (w + gap);
        let cell = Rect::from_min_size(pos2(x, rect.top()), vec2(w, rect.height()));
        ui.painter().rect_filled(cell, 3.0, r.inset);
        let h = if s.gate {
            (cell.height() * s.level.clamp(0.0, 1.0)).max(3.0)
        } else {
            3.0
        };
        let col = if s.gate {
            mix(alpha(hue, 110), hue, s.prob.clamp(0.0, 1.0))
        } else {
            r.line2
        };
        ui.painter().rect_filled(
            Rect::from_min_max(pos2(cell.left(), cell.bottom() - h), cell.max),
            3.0,
            col,
        );
        if playing == Some(i) {
            ui.painter().rect_stroke(
                cell,
                3.0,
                Stroke::new(1.5, tone(st, Tone::Text)),
                egui::StrokeKind::Inside,
            );
        }
    }
}
