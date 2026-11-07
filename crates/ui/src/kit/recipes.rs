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
        KnobBody::Cap | KnobBody::Glass | KnobBody::Solid => {
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

/// The Perform macro knob, drawn as in the mockup: a raised body with a soft shadow, a pointer
/// line, and a 6 px value arc around it. `size` is the whole square; the body is 11 px smaller
/// than the arc ring on each side. The caller paints the value inside it. Same drag as
/// [`arc_knob`].
pub fn macro_knob(
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
    let value = out.unwrap_or(value);
    let r = &st.roles;
    let c = rect.center();
    let ring = size / 2.0 - 3.0;
    let body = ring - 8.0;
    let p = ui.painter();
    let cap = |p: &Painter, v: f32, col: Color32| {
        let pt = A0 + SWEEP * v;
        p.circle_filled(c + vec2(pt.cos(), pt.sin()) * ring, 3.0, col);
    };
    p.add(Shape::line(
        arc_points(c, ring, 0.0, 1.0),
        Stroke::new(6.0, r.inset),
    ));
    cap(p, 0.0, r.inset);
    cap(p, 1.0, r.inset);
    if let Some((a, b)) = mod_span {
        p.add(Shape::line(
            arc_points(c, ring + 5.0, a.min(b), a.max(b)),
            Stroke::new(2.0, alpha(r.cv, 220)),
        ));
    }
    if value > 0.004 {
        p.add(Shape::line(
            arc_points(c, ring, 0.0, value),
            Stroke::new(6.0, r.accent),
        ));
        cap(p, 0.0, r.accent);
        cap(p, value, r.accent);
    }
    p.circle_filled(
        c + vec2(0.0, 3.0),
        body,
        alpha(Color32::BLACK, if st.dark { 100 } else { 30 }),
    );
    p.circle_filled(c, body, r.raised);
    p.circle_stroke(c, body, Stroke::new(1.0, r.line2));
    let a = A0 + SWEEP * value;
    let dir = vec2(a.cos(), a.sin());
    p.line_segment(
        [c + dir * body * 0.56, c + dir * body * 0.96],
        Stroke::new(3.0, r.text),
    );
    if resp.has_focus() {
        p.circle_stroke(
            c,
            size / 2.0,
            Stroke::new(st.slots.controls.focus_ring, r.focus),
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
    // Tall pads read from a distance: the title steps up a role.
    let role = if size.y >= 96.0 {
        Role::Title
    } else {
        Role::H3
    };
    let cy = rect.center().y + if sub.is_some() { -size.y * 0.07 } else { 0.0 };
    let avail = rect.width() - st.sp(2) * 2.0;
    let mut tr = p.layout(title.to_string(), st.font(role), ink, f32::INFINITY);
    if tr.size().x > avail {
        tr = p.layout(title.to_string(), st.font(Role::Label), ink, avail);
    }
    let th = tr.size().y;
    let tpos = pos2(rect.center().x - tr.size().x / 2.0, cy - th / 2.0);
    p.galley(tpos, tr, ink);
    if let Some(s) = sub {
        p.text(
            pos2(rect.center().x, cy + th / 2.0 + st.sp(2)),
            egui::Align2::CENTER_CENTER,
            s.to_uppercase(),
            st.font(Role::Caption),
            if state == PadState::Queued {
                r.accent_text
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

/// A scene pad: the title top-left, the scene number top-right, the state (Ready / Playing /
/// Queued) bottom-left and, while a cue arrives or waits for its bar, a short progress bar
/// bottom-right. A playing pad is filled with the accent, a queued one pulses its outline.
#[allow(clippy::too_many_arguments)]
pub fn scene_pad(
    ui: &mut Ui,
    st: &Style,
    size: egui::Vec2,
    title: &str,
    number: usize,
    state_text: &str,
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
    let p = ui.painter();
    p.rect_filled(rect, radius, fill);
    p.rect_stroke(rect, radius, stroke, egui::StrokeKind::Inside);
    let pad = st.sp(2);
    let avail = rect.width() - pad * 2.0 - st.sp(3);
    let mut tr = p.layout(title.to_string(), st.font(Role::H3), ink, f32::INFINITY);
    if tr.size().x > avail {
        tr = p.layout(title.to_string(), st.font(Role::Label), ink, avail);
    }
    p.galley(rect.left_top() + vec2(pad, pad), tr, ink);
    p.text(
        rect.right_top() + vec2(-pad, pad),
        egui::Align2::RIGHT_TOP,
        number.to_string(),
        st.font(Role::Caption),
        ink2,
    );
    p.text(
        rect.left_bottom() + vec2(pad, -pad),
        egui::Align2::LEFT_BOTTOM,
        state_text,
        st.font(Role::Caption),
        if state == PadState::Queued {
            r.accent
        } else {
            ink2
        },
    );
    if let Some(f) = progress {
        let h = st.sp(0) * 1.5;
        let bar = Rect::from_min_size(
            pos2(
                rect.right() - pad - rect.width() * 0.3,
                rect.bottom() - pad - h,
            ),
            vec2(rect.width() * 0.3, h),
        );
        p.rect_filled(bar, h / 2.0, alpha(ink, 50));
        p.rect_filled(
            Rect::from_min_size(bar.min, vec2(bar.width() * f.clamp(0.0, 1.0), h)),
            h / 2.0,
            if state == PadState::Playing {
                ink
            } else {
                r.accent
            },
        );
    }
    resp
}

/// A round icon button of diameter `d`: the accent fill when `primary`, else a raised disc.
pub fn round_button(
    ui: &mut Ui,
    st: &Style,
    ic: crate::kit::Ic,
    primary: bool,
    d: f32,
    enabled: bool,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(
        egui::Vec2::splat(d),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let r = &st.roles;
    let hover = ease_to(
        ui,
        resp.id.with("hover"),
        enabled && resp.hovered(),
        st.motion.hover,
    );
    let (fill, ink) = if primary {
        (mix(r.accent, r.text, hover * 0.15), r.on_accent)
    } else {
        (mix(r.raised, r.text, hover * 0.1), r.text)
    };
    let (fill, ink) = if enabled {
        (fill, ink)
    } else {
        (alpha(fill, 110), alpha(ink, 110))
    };
    let p = ui.painter();
    p.circle_filled(rect.center(), d / 2.0, fill);
    if !primary {
        p.circle_stroke(
            rect.center(),
            d / 2.0,
            Stroke::new(st.slots.controls.outline, r.line),
        );
    }
    crate::kit::icon(p, ic, rect.center(), d * 0.42, ink);
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

/// The Perform step lane, as in the mockup: each step a dim well with a bar rising from the
/// bottom to its level (brighter with its probability, a stub when its gate is off), a 4 px gap
/// between steps, cells at most 48 px wide, and the playing step outlined by a column a little
/// taller than the wells. `size` is the lane's full height; the wells are 10 px in from the
/// top and bottom.
pub fn seq_lane(
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
    let pitch = (rect.width() / n as f32).min(48.0);
    let well_h = (rect.height() - 20.0).max(8.0);
    for (i, s) in steps.iter().enumerate() {
        let x = rect.left() + i as f32 * pitch;
        let well = Rect::from_min_size(pos2(x + 2.0, rect.top() + 10.0), vec2(pitch - 4.0, well_h));
        // Every other bar of 16 steps gets a lighter well, so a long pattern reads in bars.
        let tint = if (i / 16) % 2 == 1 {
            mix(r.inset, r.line2, 0.6)
        } else {
            r.inset
        };
        ui.painter().rect_filled(well, 3.0, tint);
        let h = if s.gate {
            (well_h * s.level.clamp(0.0, 1.0)).max(3.0)
        } else {
            3.0
        };
        let col = if s.gate {
            mix(alpha(hue, 115), hue, s.prob.clamp(0.0, 1.0))
        } else {
            r.line2
        };
        ui.painter().rect_filled(
            Rect::from_min_max(pos2(well.left(), well.bottom() - h), well.max),
            3.0,
            col,
        );
        if playing == Some(i) {
            let col_rect = Rect::from_min_size(
                pos2(x + 0.5, rect.top() + 6.0),
                vec2(pitch - 1.0, rect.height() - 12.0),
            );
            ui.painter().rect_stroke(
                col_rect,
                4.0,
                Stroke::new(1.0, alpha(r.text, 130)),
                egui::StrokeKind::Inside,
            );
        }
    }
}

/// A bar as four beat segments: the beats played are filled, the current one fills as it goes.
/// `beats` is the position in the bar, 0..4.
pub fn bar_meter(ui: &mut Ui, st: &Style, size: egui::Vec2, beats: f32) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let gap = st.sp(1);
    let w = (rect.width() - gap * 3.0) / 4.0;
    for i in 0..4 {
        let seg = Rect::from_min_size(
            pos2(rect.left() + i as f32 * (w + gap), rect.top()),
            vec2(w, size.y),
        );
        ui.painter().rect_filled(seg, size.y / 2.0, st.roles.line2);
        let f = (beats - i as f32).clamp(0.0, 1.0);
        if f > 0.0 {
            let fill = Rect::from_min_size(seg.min, vec2(w * f, size.y));
            ui.painter()
                .rect_filled(fill, size.y / 2.0, st.roles.accent);
        }
    }
}
