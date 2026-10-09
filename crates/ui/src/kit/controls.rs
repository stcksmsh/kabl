//! Buttons, chips, segmented controls, toggle, slider, text field, tags, meters.

use super::{dim, ease_to, icon, tone, Ic, Tone};
use crate::style::{alpha, mix, Role, Style};
use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, Margin, Rect, Response, Sense, Stroke, StrokeKind,
    Ui, Widget, WidgetInfo, WidgetType,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The one main action of a panel.
    Primary,
    /// Framed, quiet. Also the base of selectable chips.
    Secondary,
    /// No frame until hovered.
    Ghost,
}

/// Fill a rounded rect with the theme's vertical gradient depth (0 = flat).
pub(crate) fn fill_rr(ui: &Ui, rect: Rect, radius: f32, color: Color32, depth: f32) {
    let p = ui.painter();
    let cr = CornerRadius::same(radius.round().clamp(0.0, 255.0) as u8);
    if depth <= 0.0 {
        p.rect_filled(rect, cr, color);
        return;
    }
    let top = mix(color, Color32::WHITE, depth * 0.35);
    let bot = mix(color, Color32::BLACK, depth * 0.25);
    let mut mesh = egui::epaint::Mesh::default();
    let pts = crate::kit::round_points(rect, radius);
    mesh.colored_vertex(rect.center(), mix(top, bot, 0.5));
    for q in &pts {
        let t = ((q.y - rect.top()) / rect.height().max(1.0)).clamp(0.0, 1.0);
        mesh.colored_vertex(*q, mix(top, bot, t));
    }
    let n = pts.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    p.add(egui::Shape::mesh(mesh));
    p.add(egui::Shape::closed_line(pts, Stroke::new(1.0, color)));
}

pub(crate) fn focus_ring(ui: &Ui, st: &Style, rect: Rect, radius: f32) {
    ui.painter().rect_stroke(
        rect.expand(2.0),
        CornerRadius::same((radius + 2.0) as u8),
        Stroke::new(st.slots.controls.focus_ring, st.roles.focus),
        StrokeKind::Outside,
    );
}

pub struct Button<'a> {
    st: &'a Style,
    text: String,
    icon: Option<Ic>,
    kind: Kind,
    selected: bool,
    enabled: bool,
    small: bool,
    min_w: f32,
    tone: Option<Tone>,
}

impl<'a> Button<'a> {
    pub fn new(st: &'a Style, text: impl Into<String>) -> Self {
        Button {
            st,
            text: text.into(),
            icon: None,
            kind: Kind::Secondary,
            selected: false,
            enabled: true,
            small: false,
            min_w: 0.0,
            tone: None,
        }
    }
    pub fn icon(mut self, ic: Ic) -> Self {
        self.icon = Some(ic);
        self
    }
    pub fn primary(mut self) -> Self {
        self.kind = Kind::Primary;
        self
    }
    pub fn ghost(mut self) -> Self {
        self.kind = Kind::Ghost;
        self
    }
    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }
    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }
    pub fn min_width(mut self, w: f32) -> Self {
        self.min_w = w;
        self
    }
    pub fn tone(mut self, t: Tone) -> Self {
        self.tone = Some(t);
        self
    }
}

impl Widget for Button<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let st = self.st;
        let enabled = self.enabled && ui.is_enabled();
        let font = st.font(Role::Label);
        let h = if self.small {
            st.metrics.small_h
        } else {
            st.metrics.control_h
        };
        let pad = if self.small { st.sp(2) } else { st.sp(3) };
        let icon_w = if self.icon.is_some() {
            st.metrics.icon + st.sp(1) + 2.0
        } else {
            0.0
        };
        let galley = (!self.text.is_empty()).then(|| {
            ui.painter()
                .layout_no_wrap(self.text.clone(), font, Color32::WHITE)
        });
        let tw = galley.as_ref().map_or(0.0, |g| g.size().x);
        let w = (pad * 2.0 + icon_w + tw)
            .max(self.min_w)
            .max(if self.text.is_empty() { h } else { 0.0 });
        let (rect, resp) = ui.allocate_exact_size(
            vec2(w, h),
            if enabled {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, &self.text));
        if !ui.is_rect_visible(rect) {
            return resp;
        }
        let hover = ease_to(
            ui,
            resp.id.with("hover"),
            enabled && resp.hovered(),
            st.motion.hover,
        );
        let down = enabled && resp.is_pointer_button_down_on();
        let r = &st.roles;
        let radius = st.shape_radius(st.slots.controls.button, h);
        let (fill, border, ink): (Color32, Option<Color32>, Color32) = match self.kind {
            Kind::Primary => {
                let base = if down {
                    mix(r.accent, Color32::BLACK, 0.18)
                } else {
                    mix(r.accent, Color32::BLACK, 0.10 * hover)
                };
                (base, None, r.on_accent)
            }
            Kind::Secondary => {
                if self.selected {
                    (
                        mix(r.accent_soft, r.raised, 0.25 * hover),
                        Some(alpha(r.accent, 170)),
                        r.text,
                    )
                } else if down {
                    (r.inset, Some(r.line2), r.text)
                } else {
                    (
                        mix(alpha(r.raised, 0), r.raised, hover),
                        Some(mix(r.line, r.line2, hover)),
                        mix(r.text2, r.text, hover),
                    )
                }
            }
            Kind::Ghost => (
                mix(
                    alpha(r.inset, 0),
                    r.inset,
                    hover.max(if down { 1.0 } else { 0.0 }),
                ),
                None,
                mix(r.text2, r.text, hover),
            ),
        };
        let ink = self.tone.map_or(ink, |t| tone(st, t));
        let depth = if self.kind == Kind::Primary {
            st.slots.controls.gradient
        } else {
            0.0
        };
        fill_rr(ui, rect, radius, dim(fill, enabled), depth);
        if let Some(b) = border {
            ui.painter().rect_stroke(
                rect,
                CornerRadius::same(radius as u8),
                Stroke::new(1.0, dim(b, enabled)),
                StrokeKind::Inside,
            );
        }
        let mut x = rect.center().x - (tw + icon_w) / 2.0;
        if let Some(ic) = self.icon {
            icon(
                ui.painter(),
                ic,
                pos2(x + st.metrics.icon / 2.0, rect.center().y),
                st.metrics.icon,
                dim(ink, enabled),
            );
            x += icon_w;
        }
        if galley.is_some() {
            let g = ui.painter().layout_no_wrap(
                self.text.clone(),
                st.font(Role::Label),
                dim(ink, enabled),
            );
            ui.painter().galley(
                pos2(x, rect.center().y - g.size().y / 2.0),
                g,
                dim(ink, enabled),
            );
        }
        if enabled && resp.has_focus() {
            focus_ring(ui, st, rect, radius);
        }
        resp
    }
}

/// A square icon-only button.
pub fn icon_button(ui: &mut Ui, st: &Style, ic: Ic, selected: bool, enabled: bool) -> Response {
    let enabled = enabled && ui.is_enabled();
    let s = st.metrics.control_h;
    let (rect, resp) = ui.allocate_exact_size(
        vec2(s, s),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, format!("{ic:?}")));
    let hover = ease_to(
        ui,
        resp.id.with("hover"),
        enabled && resp.hovered(),
        st.motion.hover,
    );
    let down = enabled && resp.is_pointer_button_down_on();
    let r = &st.roles;
    let radius = st.shape_radius(st.slots.controls.button, s);
    let fill = if selected {
        r.accent_soft
    } else {
        mix(
            alpha(r.inset, 0),
            r.inset,
            hover.max(if down { 1.0 } else { 0.0 }),
        )
    };
    fill_rr(ui, rect, radius, dim(fill, enabled), 0.0);
    let ink = if selected {
        r.text
    } else {
        mix(r.text2, r.text, hover)
    };
    icon(
        ui.painter(),
        ic,
        rect.center(),
        st.metrics.icon + 2.0,
        dim(ink, enabled),
    );
    if enabled && resp.has_focus() {
        focus_ring(ui, st, rect, radius);
    }
    resp
}

/// Framed track for a row of [`seg`] items.
pub fn segmented<R>(ui: &mut Ui, st: &Style, add: impl FnOnce(&mut Ui) -> R) -> R {
    let radius = st.shape_radius(st.slots.controls.chip, st.metrics.control_h);
    egui::Frame::new()
        .fill(st.roles.inset)
        .stroke(Stroke::new(1.0, st.roles.line))
        .corner_radius(CornerRadius::same(radius as u8))
        .inner_margin(Margin::same(2))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.horizontal(add).inner
        })
        .inner
}

/// One item of a segmented control.
pub fn seg(ui: &mut Ui, st: &Style, text: &str, selected: bool) -> Response {
    seg_enabled(ui, st, text, selected, true)
}

pub fn seg_enabled(ui: &mut Ui, st: &Style, text: &str, selected: bool, enabled: bool) -> Response {
    let enabled = enabled && ui.is_enabled();
    let font = st.font(Role::Label);
    let g = ui
        .painter()
        .layout_no_wrap(text.to_string(), font, Color32::WHITE);
    let h = st.metrics.control_h - 6.0;
    let (rect, resp) = ui.allocate_exact_size(
        vec2(g.size().x + st.sp(3) * 2.0, h),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    resp.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, enabled, selected, text));
    let hover = ease_to(
        ui,
        resp.id.with("hover"),
        enabled && resp.hovered(),
        st.motion.hover,
    );
    let r = &st.roles;
    let radius = st.shape_radius(st.slots.controls.chip, h);
    if selected {
        crate::kit::shadow(ui, st, rect, radius, 0);
        fill_rr(ui, rect, radius, r.raised, 0.0);
    } else if hover > 0.0 {
        fill_rr(
            ui,
            rect,
            radius,
            alpha(r.raised, (140.0 * hover) as u8),
            0.0,
        );
    }
    let ink = if selected {
        r.text
    } else {
        mix(r.text2, r.text, hover)
    };
    let ink = dim(ink, enabled);
    let g = ui
        .painter()
        .layout_no_wrap(text.to_string(), st.font(Role::Label), ink);
    ui.painter().galley(rect.center() - g.size() / 2.0, g, ink);
    if enabled && resp.has_focus() {
        focus_ring(ui, st, rect, radius);
    }
    resp
}

/// On/off switch with a label to its right.
pub fn toggle(ui: &mut Ui, st: &Style, on: &mut bool, text: &str) -> Response {
    let enabled = ui.is_enabled();
    let label = ui
        .painter()
        .layout_no_wrap(text.to_string(), st.font(Role::Body), st.roles.text);
    let sz = st.metrics.toggle;
    let w = sz.x
        + if text.is_empty() {
            0.0
        } else {
            st.sp(2) + label.size().x
        };
    let (rect, mut resp) = ui.allocate_exact_size(
        vec2(w, sz.y.max(st.metrics.small_h)),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    resp.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, *on, text));
    let t = ease_to(ui, resp.id.with("on"), *on, st.motion.glide);
    let r = &st.roles;
    let track = Rect::from_min_size(pos2(rect.left(), rect.center().y - sz.y / 2.0), sz);
    let rad = st
        .shape_radius(st.slots.controls.chip, sz.y)
        .min(sz.y / 2.0);
    fill_rr(ui, track, rad, dim(mix(r.inset, r.accent, t), enabled), 0.0);
    ui.painter().rect_stroke(
        track,
        CornerRadius::same(rad as u8),
        Stroke::new(1.0, dim(mix(r.line2, r.accent, t), enabled)),
        StrokeKind::Inside,
    );
    let kx = track.left() + sz.y / 2.0 + (sz.x - sz.y) * t;
    ui.painter().circle_filled(
        pos2(kx, track.center().y),
        sz.y / 2.0 - 3.0,
        dim(mix(r.text2, r.on_accent, t), enabled),
    );
    if !text.is_empty() {
        let g = ui.painter().layout_no_wrap(
            text.to_string(),
            st.font(Role::Body),
            dim(r.text, enabled),
        );
        ui.painter().galley(
            pos2(track.right() + st.sp(2), rect.center().y - g.size().y / 2.0),
            g,
            dim(r.text, enabled),
        );
    }
    if enabled && resp.has_focus() {
        focus_ring(ui, st, track, rad);
    }
    resp
}

/// Horizontal slider with a numeric readout at the right. `value` is clamped to `range`;
/// `step` snaps it. Dragging, clicking and the arrow keys all work.
pub fn slider(
    ui: &mut Ui,
    st: &Style,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    step: Option<f64>,
    readout: &str,
) -> Response {
    let enabled = ui.is_enabled();
    let readout_w = ui
        .painter()
        .layout_no_wrap("000.00 ms".into(), st.font(Role::Value), Color32::WHITE)
        .size()
        .x;
    let w = (ui.available_width() - ui.spacing().item_spacing.x).max(60.0);
    let (rect, mut resp) = ui.allocate_exact_size(
        vec2(w, st.metrics.slider_h),
        if enabled {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        },
    );
    let track_r = Rect::from_min_max(
        rect.min,
        pos2(rect.right() - readout_w - st.sp(2), rect.max.y),
    );
    let (lo, hi) = (*range.start(), *range.end());
    let span = (hi - lo).max(f64::EPSILON);
    let before = *value;
    if enabled {
        if let Some(p) = resp
            .interact_pointer_pos()
            .filter(|_| resp.is_pointer_button_down_on() || resp.clicked())
        {
            let t =
                ((p.x - track_r.left() - 8.0) / (track_r.width() - 16.0)).clamp(0.0, 1.0) as f64;
            *value = lo + span * t;
        }
        if resp.has_focus() {
            let (l, r_) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::ArrowLeft),
                    i.key_pressed(egui::Key::ArrowRight),
                )
            });
            let d = step.unwrap_or(span / 20.0);
            if l {
                *value -= d;
            }
            if r_ {
                *value += d;
            }
        }
        if let Some(s) = step {
            *value = lo + ((*value - lo) / s).round() * s;
        }
        *value = value.clamp(lo, hi);
    }
    if (*value - before).abs() > f64::EPSILON {
        resp.mark_changed();
    }
    resp.widget_info(|| WidgetInfo::slider(enabled, *value, readout));
    let r = &st.roles;
    let t = ((*value - lo) / span) as f32;
    let th = st.sp(1) + 1.0;
    let cy = rect.center().y;
    let tr = Rect::from_min_max(
        pos2(track_r.left() + 8.0, cy - th / 2.0),
        pos2(track_r.right() - 8.0, cy + th / 2.0),
    );
    fill_rr(ui, tr, th / 2.0, dim(r.inset, enabled), 0.0);
    ui.painter().rect_stroke(
        tr,
        CornerRadius::same((th / 2.0) as u8),
        Stroke::new(1.0, r.line),
        StrokeKind::Inside,
    );
    let x = tr.left() + tr.width() * t;
    fill_rr(
        ui,
        Rect::from_min_max(tr.min, pos2(x, tr.max.y)),
        th / 2.0,
        dim(r.accent, enabled),
        0.0,
    );
    let h = pos2(x, cy);
    let hover = ease_to(
        ui,
        resp.id.with("hover"),
        enabled && (resp.hovered() || resp.dragged()),
        st.motion.hover,
    );
    let hr = 7.0 + hover;
    ui.painter().circle_filled(
        h + vec2(0.0, 1.5),
        hr,
        alpha(Color32::BLACK, st.shadows[0].alpha),
    );
    ui.painter().circle_filled(h, hr, dim(r.raised, enabled));
    ui.painter()
        .circle_stroke(h, hr, Stroke::new(1.5, dim(r.accent, enabled)));
    if resp.has_focus() && enabled {
        ui.painter().circle_stroke(
            h,
            hr + 3.0,
            Stroke::new(st.slots.controls.focus_ring, r.focus),
        );
    }
    ui.painter().text(
        pos2(rect.right(), cy),
        Align2::RIGHT_CENTER,
        readout,
        st.font(Role::Value),
        dim(r.text, enabled),
    );
    resp
}

/// Single-line text field in a themed frame, with an optional leading icon. The editing itself
/// is `egui::TextEdit` (selection, IME, clipboard); the frame, colours and focus ring are ours.
pub fn field(
    ui: &mut Ui,
    st: &Style,
    text: &mut String,
    hint: &str,
    lead: Option<Ic>,
    width: Option<f32>,
) -> Response {
    let enabled = ui.is_enabled();
    let h = st.metrics.field_h;
    let w = width.unwrap_or_else(|| ui.available_width());
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let radius = st.shape_radius(st.slots.controls.field, h);
    let back = ui.painter().add(egui::Shape::Noop);
    let pad = st.sp(2) + 2.0;
    let lead_w = if lead.is_some() {
        st.metrics.icon + st.sp(2)
    } else {
        0.0
    };
    if let Some(ic) = lead {
        icon(
            ui.painter(),
            ic,
            pos2(rect.left() + pad + st.metrics.icon / 2.0, rect.center().y),
            st.metrics.icon,
            st.roles.text2,
        );
    }
    let inner = Rect::from_min_max(
        pos2(rect.left() + pad + lead_w, rect.top()),
        pos2(rect.right() - pad, rect.bottom()),
    );
    let font = st.font(Role::Body);
    let vpad = ((h - font.size * 1.25) / 2.0).max(0.0);
    let hint_text = egui::RichText::new(hint)
        .font(font.clone())
        .color(st.roles.text3);
    let edit = egui::TextEdit::singleline(text)
        .frame(egui::Frame::NONE)
        .font(font)
        .text_color(st.roles.text)
        .hint_text(hint_text)
        .margin(Margin::symmetric(0, vpad as i8))
        .vertical_align(egui::Align::Center)
        .desired_width(inner.width());
    let resp = ui.put(inner, edit);
    let focused = resp.has_focus();
    let hover = ease_to(ui, resp.id.with("hover"), resp.hovered(), st.motion.hover);
    let r = &st.roles;
    let mut shapes = vec![
        egui::Shape::rect_filled(
            rect,
            CornerRadius::same(radius as u8),
            dim(r.inset, enabled),
        ),
        egui::Shape::rect_stroke(
            rect,
            CornerRadius::same(radius as u8),
            Stroke::new(
                if focused {
                    st.slots.controls.focus_ring
                } else {
                    1.0
                },
                if focused {
                    r.focus
                } else {
                    mix(r.line, r.line2, hover)
                },
            ),
            StrokeKind::Inside,
        ),
    ];
    ui.painter()
        .set(back, egui::Shape::Vec(std::mem::take(&mut shapes)));
    resp
}

/// Small rounded label, tinted by `col`.
pub fn tag(ui: &mut Ui, st: &Style, text: &str, col: Color32) -> Response {
    let font = st.font(Role::Caption);
    let g = ui
        .painter()
        .layout_no_wrap(text.to_string(), font, Color32::WHITE);
    let h = st.metrics.small_h - 6.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + st.sp(2) * 1.5, h), Sense::hover());
    let radius = st.shape_radius(st.slots.controls.chip, h).min(h / 2.0);
    fill_rr(
        ui,
        rect,
        radius,
        alpha(col, if st.dark { 50 } else { 38 }),
        0.0,
    );
    let ink = if st.dark {
        mix(col, Color32::WHITE, 0.45)
    } else {
        mix(col, Color32::BLACK, 0.45)
    };
    let g = ui
        .painter()
        .layout_no_wrap(text.to_string(), st.font(Role::Caption), ink);
    ui.painter().galley(rect.center() - g.size() / 2.0, g, ink);
    resp
}

/// Level bar: `n` in 0..=1 of a track, tinted good / warn / bad.
pub fn bar(ui: &mut Ui, st: &Style, rect: Rect, n: f32) {
    let r = &st.roles;
    let rad = (rect.height() / 2.0).min(st.radii.sm);
    fill_rr(ui, rect, rad, r.inset, 0.0);
    let col = if n >= 1.0 {
        r.bad
    } else if n > 0.7 {
        r.warn
    } else {
        r.good
    };
    let w = rect.width() * n.clamp(0.0, 1.0);
    fill_rr(
        ui,
        Rect::from_min_size(rect.min, vec2(w, rect.height())),
        rad,
        col,
        0.0,
    );
}
