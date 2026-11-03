//! G4 kit parts (docs/design/redesign/g4/G4-SPEC.md §1-3): checkbox, framed modal, tool window,
//! dock, empty state, toast, banner, step grid and range bar, plus the few tokens the G4
//! surfaces need that no theme role carries (scrim, state tints, shadow base, field fill).
//! Tokens are derived from the existing roles, so theme files keep loading unchanged.

use super::controls::{fill_rr, focus_ring};
use super::{dim, ease_to, icon, icon_button, label, paragraph, rich, rule, Ic, Tone};
use crate::style::{alpha, mix, Role, Style};
use egui::{
    pos2, vec2, Color32, CornerRadius, Frame, Margin, Painter, Rect, Response, Sense, Stroke,
    StrokeKind, Ui, WidgetInfo, WidgetType,
};

/// Tokens specific to G4 surfaces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tokens {
    /// Modal backdrop: black 55 % on dark, warm near-black 28 % on light (never white).
    pub scrim: Color32,
    /// Base alpha (0..=1) of the stacked shadow: 1 on dark, 0.18 on light.
    pub shadow_base: f32,
    /// Text field fill: `inset` on dark, `raised` on light.
    pub field_bg: Color32,
}

pub fn tokens(st: &Style) -> Tokens {
    let (scrim, shadow_base, field_bg) = if st.dark {
        (Color32::from_black_alpha(140), 1.0, st.roles.inset)
    } else {
        (
            Color32::from_rgba_unmultiplied(26, 25, 23, 71),
            0.18,
            st.roles.raised,
        )
    };
    Tokens {
        scrim,
        shadow_base,
        field_bg,
    }
}

/// State colour of a banner, toast or status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sev {
    Bad,
    Warn,
    Good,
}

impl Sev {
    pub fn color(self, st: &Style) -> Color32 {
        match self {
            Sev::Bad => st.roles.bad,
            Sev::Warn => st.roles.warn,
            Sev::Good => st.roles.good,
        }
    }
    pub fn icon(self) -> Ic {
        match self {
            Sev::Bad => Ic::Err,
            Sev::Warn => Ic::Warn,
            Sev::Good => Ic::Check,
        }
    }
    /// Seconds before a toast of this severity goes away; `None` = until dismissed.
    pub fn ttl(self) -> Option<f64> {
        match self {
            Sev::Bad => None,
            Sev::Warn => Some(8.0),
            Sev::Good => Some(3.0),
        }
    }
}

/// Banner background: the state colour at 12 % (dark) or 8 % (light).
pub fn tint(st: &Style, c: Color32) -> Color32 {
    alpha(c, if st.dark { 31 } else { 20 })
}

/// Three stacked rects (offset 2/6/12, alpha .5/.3/.18 of the base), no blur.
pub fn stacked_shadow(p: &Painter, st: &Style, rect: Rect, radius: f32) {
    let base = tokens(st).shadow_base;
    for (dy, a) in [(2.0, 0.5), (6.0, 0.3), (12.0, 0.18)] {
        let r = rect.translate(vec2(0.0, dy)).expand(dy / 3.0);
        let c = Color32::from_black_alpha((255.0 * a * base) as u8);
        p.rect_filled(r, CornerRadius::same((radius + dy / 3.0) as u8), c);
    }
}

fn surface_frame(st: &Style, radius: f32) -> Frame {
    Frame::new()
        .fill(st.roles.raised)
        .stroke(Stroke::new(1.0, st.roles.line))
        .corner_radius(CornerRadius::same(radius as u8))
}

// ------------------------------------------------------------------ checkbox

/// 18 px box, radius 5; replaces the stock egui checkbox.
pub fn checkbox(ui: &mut Ui, st: &Style, on: &mut bool, text: &str) -> Response {
    let enabled = ui.is_enabled();
    let g = ui
        .painter()
        .layout_no_wrap(text.to_string(), st.font(Role::Body), st.roles.text);
    let w = 18.0
        + if text.is_empty() {
            0.0
        } else {
            st.sp(2) + g.size().x
        };
    let (rect, mut resp) = ui.allocate_exact_size(
        vec2(w, 18.0_f32.max(st.metrics.small_h)),
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
    let t = ease_to(ui, resp.id.with("on"), *on, st.motion.hover);
    let r = &st.roles;
    let bx = Rect::from_min_size(pos2(rect.left(), rect.center().y - 9.0), vec2(18.0, 18.0));
    fill_rr(
        ui,
        bx,
        5.0,
        dim(mix(tokens(st).field_bg, r.accent, t), enabled),
        0.0,
    );
    ui.painter().rect_stroke(
        bx,
        CornerRadius::same(5),
        Stroke::new(1.0, dim(mix(r.line2, r.accent, t), enabled)),
        StrokeKind::Inside,
    );
    if *on {
        icon(
            ui.painter(),
            Ic::Check,
            bx.center(),
            13.0,
            dim(r.on_accent, enabled),
        );
    }
    if !text.is_empty() {
        let g = ui.painter().layout_no_wrap(
            text.to_string(),
            st.font(Role::Body),
            dim(r.text, enabled),
        );
        let at = pos2(bx.right() + st.sp(2), rect.center().y - g.size().y / 2.0);
        ui.painter().galley(at, g, dim(r.text, enabled));
    }
    if enabled && resp.has_focus() {
        focus_ring(ui, st, bx, 5.0);
    }
    resp
}

// ------------------------------------------------------------------ frames

/// Modal widths (px; clamped to the window minus 80).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    S,
    M,
    L,
}

impl Size {
    pub fn width(self) -> f32 {
        match self {
            Size::S => 400.0,
            Size::M => 520.0,
            Size::L => 760.0,
        }
    }
}

/// Modal: scrim, title 17/600 with a 28 px close, body, footer (64 px, top rule, actions
/// right-aligned, primary last). Returns true when the user closed it (close, Esc, backdrop).
pub fn modal_framed<T>(
    ctx: &egui::Context,
    st: &Style,
    id: &str,
    size: Size,
    title: &str,
    body: impl FnOnce(&mut Ui) -> T,
    footer: impl FnOnce(&mut Ui),
) -> bool {
    let width = size.width().min(ctx.content_rect().width() - 80.0);
    let frame = surface_frame(st, 12.0)
        .shadow(super::eshadow(st, 2))
        .inner_margin(Margin::ZERO);
    let resp = egui::Modal::new(egui::Id::new(id))
        .frame(frame)
        .backdrop_color(tokens(st).scrim)
        .show(ctx, |ui| {
            ui.set_width(width);
            let mut closed = false;
            ui.add_space(20.0);
            ui.horizontal(|ui| {
                ui.add_space(24.0);
                ui.label(rich(st, Role::Title, Tone::Text, title));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(14.0);
                    closed = icon_button(ui, st, Ic::Close, false, true).clicked();
                });
            });
            ui.add_space(8.0);
            Frame::new()
                .inner_margin(Margin::symmetric(24, 4))
                .show(ui, |ui| body(ui));
            ui.add_space(12.0);
            rule(ui, st);
            ui.horizontal(|ui| {
                ui.set_height(63.0);
                ui.add_space(24.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(24.0);
                    footer(ui);
                });
            });
            closed
        });
    resp.inner || resp.should_close()
}

/// Non-blocking tool window with a 44 px title bar: grip, title 14/600, optional tag chip
/// (for example ADVANCED), close. Callers pass `at` with x = 40 so it never clips at 1180 px.
pub fn tool_window(
    ctx: &egui::Context,
    st: &Style,
    id: &str,
    (title, tag): (&str, Option<&str>),
    at: egui::Pos2,
    open: &mut bool,
    content: impl FnOnce(&mut Ui),
) {
    if !*open {
        return;
    }
    let wid = egui::Id::new(id);
    if let Some(r) = ctx.memory(|m| m.area_rect(wid)) {
        let p = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Background,
            wid.with("shadow"),
        ));
        stacked_shadow(&p, st, r, 12.0);
    }
    egui::Window::new(title)
        .id(wid)
        .title_bar(false)
        .resizable(false)
        .default_pos(at)
        .frame(surface_frame(st, 12.0).inner_margin(Margin::ZERO))
        .show(ctx, |ui| {
            ui.set_min_width(380.0);
            ui.horizontal(|ui| {
                ui.set_height(44.0);
                ui.add_space(14.0);
                let (r, _) = ui.allocate_exact_size(vec2(16.0, 44.0), Sense::hover());
                icon(ui.painter(), Ic::Grip, r.center(), 14.0, st.roles.text3);
                ui.label(rich(st, Role::Label, Tone::Text, title));
                if let Some(t) = tag {
                    super::tag(ui, st, t, st.roles.accent);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    if icon_button(ui, st, Ic::Close, false, true).clicked() {
                        *open = false;
                    }
                });
            });
            rule(ui, st);
            Frame::new()
                .inner_margin(Margin::same(16))
                .show(ui, content);
        });
}

/// Bottom dock: full width, drag handle on the top edge, resizable between `min` and `max`.
pub fn dock(
    ui: &mut Ui,
    st: &Style,
    id: &str,
    height: f32,
    min: f32,
    max: f32,
    content: impl FnOnce(&mut Ui),
) {
    egui::Panel::bottom(egui::Id::new(id))
        .resizable(true)
        .default_size(height)
        .size_range(min..=max)
        .frame(
            Frame::new()
                .fill(st.roles.surface)
                .stroke(Stroke::new(1.0, st.roles.line))
                .inner_margin(Margin::symmetric(16, 10)),
        )
        .show(ui, |ui| {
            let top = ui.max_rect().top();
            let c = pos2(ui.max_rect().center().x, top - 5.0);
            ui.painter().rect_filled(
                Rect::from_center_size(c, vec2(36.0, 4.0)),
                CornerRadius::same(2),
                st.roles.line2,
            );
            content(ui);
        });
}

// ------------------------------------------------------------------ feedback

/// A calm placeholder: 56 px circle with an icon, title, one line, up to two actions.
pub fn empty_state(
    ui: &mut Ui,
    st: &Style,
    ic: Ic,
    title: &str,
    line: &str,
    actions: impl FnOnce(&mut Ui),
) {
    ui.vertical_centered(|ui| {
        ui.add_space(24.0);
        let (r, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 28.0, st.roles.inset);
        ui.painter()
            .circle_stroke(r.center(), 28.0, Stroke::new(1.0, st.roles.line));
        icon(ui.painter(), ic, r.center(), 24.0, st.roles.text2);
        ui.add_space(12.0);
        label(ui, st, Role::H3, Tone::Text, title);
        ui.add_space(4.0);
        paragraph(ui, st, Role::Body, Tone::Text2, line);
        ui.add_space(14.0);
        ui.horizontal(actions);
    });
}

/// Inline banner (form errors, warnings): tinted, icon, title, optional mono detail.
pub fn banner(ui: &mut Ui, st: &Style, sev: Sev, title: &str, detail: Option<&str>) {
    let c = sev.color(st);
    Frame::new()
        .fill(tint(st, c))
        .stroke(Stroke::new(1.0, alpha(c, 90)))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                icon(ui.painter(), sev.icon(), r.center(), 16.0, c);
                ui.vertical(|ui| {
                    ui.label(rich(st, Role::Label, Tone::Text, title));
                    if let Some(d) = detail {
                        ui.label(rich(st, Role::Value, Tone::Text2, d));
                    }
                });
            });
        });
}

/// One queued message.
#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    pub sev: Sev,
    pub title: String,
    pub detail: Option<String>,
    pub born: f64,
}

/// Number of persistent errors: the status bar's red pill shows this.
pub fn error_count(toasts: &[Toast]) -> usize {
    toasts.iter().filter(|t| t.sev == Sev::Bad).count()
}

/// One 380 px toast: 4 px severity bar, icon, title, mono detail, dismiss. True when dismissed.
pub fn toast(ui: &mut Ui, st: &Style, t: &Toast) -> bool {
    let c = t.sev.color(st);
    let mut dismissed = false;
    let out = surface_frame(st, 10.0)
        .inner_margin(Margin {
            left: 16,
            right: 8,
            top: 10,
            bottom: 10,
        })
        .show(ui, |ui| {
            ui.set_width(380.0 - 24.0);
            ui.horizontal_top(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                icon(ui.painter(), t.sev.icon(), r.center(), 16.0, c);
                ui.vertical(|ui| {
                    ui.set_width(380.0 - 24.0 - 18.0 - 34.0);
                    ui.label(rich(st, Role::Label, Tone::Text, &t.title));
                    if let Some(d) = &t.detail {
                        ui.add(
                            egui::Label::new(rich(st, Role::Value, Tone::Text2, d))
                                .selectable(false)
                                .wrap(),
                        );
                    }
                });
                dismissed = icon_button(ui, st, Ic::Close, false, true).clicked();
            });
        });
    let r = out.response.rect;
    ui.painter().rect_filled(
        Rect::from_min_size(r.min, vec2(4.0, r.height())).shrink2(vec2(0.0, 1.0)),
        CornerRadius::same(2),
        c,
    );
    dismissed
}

/// Top-right toast stack inside `area`: newest first, at most 3, expiry per severity.
pub fn toasts(ctx: &egui::Context, st: &Style, area: Rect, list: &mut Vec<Toast>, now: f64) {
    list.retain(|t| t.sev.ttl().is_none_or(|ttl| now - t.born < ttl));
    let mut gone = None;
    egui::Area::new(egui::Id::new("g4-toasts"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos2(area.right() - 16.0 - 380.0, area.top() + 16.0))
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            for (i, t) in list.iter().enumerate().rev().take(3) {
                if toast(ui, st, t) {
                    gone = Some(i);
                }
            }
        });
    if let Some(i) = gone {
        list.remove(i);
    }
}

// ------------------------------------------------------------------ editors

/// What a step-grid lane edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaneKind {
    /// Value bars 0..=1, or -1..=1 from a centre line when `bipolar`.
    Bars { bipolar: bool },
    /// On/off pills (value > 0.5 is on).
    Gate,
    /// A count 1..=`max` shown as "×n"; a tap steps it, wrapping to 1.
    Count { max: u8 },
}

pub struct Lane<'a> {
    pub label: &'a str,
    pub kind: LaneKind,
    pub values: &'a mut [f32],
    /// Small mono line under the label ("st").
    pub unit: &'a str,
    /// Bar and on-pill colour; `None` is the accent.
    pub color: Option<Color32>,
    /// Lane height; `None` is the kind's default.
    pub height: Option<f32>,
    /// Mono readout at the foot of each bar ("+7").
    pub readout: Option<fn(f32) -> String>,
}

impl<'a> Lane<'a> {
    pub fn new(label: &'a str, kind: LaneKind, values: &'a mut [f32]) -> Self {
        Lane {
            label,
            kind,
            values,
            unit: "",
            color: None,
            height: None,
            readout: None,
        }
    }
}

const LABEL_W: f32 = 56.0;
const BARS_H: f32 = 36.0;
const GATE_H: f32 = 20.0;
const LANE_GAP: f32 = 8.0;
/// The step-number row above the lanes.
const NUMBERS_H: f32 = 22.0;

/// Shared step editor (seq, cable pattern, Perform lanes): one column per step, lane labels in
/// a 56 px column, optional step-number row. Tap toggles a gate or steps a count, drag sets a
/// bar's value. The playhead is an accent outline across all lanes. The response is marked
/// changed when a value changed.
pub fn step_grid(
    ui: &mut Ui,
    st: &Style,
    lanes: &mut [Lane],
    playhead: Option<usize>,
    width: f32,
    numbers: bool,
) -> Response {
    let steps = lanes.iter().map(|l| l.values.len()).max().unwrap_or(0);
    let heights: Vec<f32> = lanes
        .iter()
        .map(|l| {
            l.height.unwrap_or(match l.kind {
                LaneKind::Bars { .. } => BARS_H,
                LaneKind::Gate | LaneKind::Count { .. } => GATE_H,
            })
        })
        .collect();
    let head = if numbers { NUMBERS_H } else { 0.0 };
    let total = head + heights.iter().sum::<f32>() + LANE_GAP * (lanes.len() as f32 - 1.0).max(0.0);
    let (rect, mut resp) = ui.allocate_exact_size(vec2(width, total), Sense::click_and_drag());
    if steps == 0 {
        return resp;
    }
    let cell_w = (width - LABEL_W) / steps as f32;
    let r = &st.roles;
    let well = if st.dark { r.inset } else { r.raised };
    let mono = st.font(Role::Value);
    let p = ui.painter();
    let mut changed = false;
    let ptr = resp.interact_pointer_pos();
    let col_x = |i: usize| rect.left() + LABEL_W + i as f32 * cell_w;
    let mut y = rect.top();
    if numbers {
        p.text(
            pos2(rect.left(), y + head / 2.0),
            egui::Align2::LEFT_CENTER,
            "STEP",
            st.section_font(),
            r.text2,
        );
        for i in 0..steps {
            p.text(
                pos2(col_x(i) + cell_w / 2.0, y + head / 2.0),
                egui::Align2::CENTER_CENTER,
                (i + 1).to_string(),
                mono.clone(),
                if playhead == Some(i) { r.text } else { r.text2 },
            );
        }
        y += head;
    }
    for (li, lane) in lanes.iter_mut().enumerate() {
        let h = heights[li];
        let col = lane.color.unwrap_or(r.accent);
        let label_y = if h > 30.0 { y + 12.0 } else { y + h / 2.0 };
        p.text(
            pos2(rect.left(), label_y),
            egui::Align2::LEFT_CENTER,
            lane.label.to_uppercase(),
            st.section_font(),
            r.text2,
        );
        if !lane.unit.is_empty() {
            p.text(
                pos2(rect.left(), label_y + 16.0),
                egui::Align2::LEFT_CENTER,
                lane.unit,
                mono.clone(),
                r.text2,
            );
        }
        let lane_r = Rect::from_min_max(pos2(col_x(0), y), pos2(rect.right(), y + h));
        if lane.kind == (LaneKind::Bars { bipolar: true }) {
            // One well for the whole lane, with the zero line across it.
            p.rect(
                lane_r,
                6.0,
                well,
                Stroke::new(1.0, r.line),
                StrokeKind::Inside,
            );
        }
        for (i, v) in lane.values.iter_mut().enumerate() {
            let cell = Rect::from_min_size(pos2(col_x(i), y), vec2(cell_w, h));
            let hit = ptr.is_some_and(|q| cell.expand2(vec2(0.0, LANE_GAP / 2.0)).contains(q));
            match lane.kind {
                LaneKind::Bars { bipolar } => {
                    let foot = if lane.readout.is_some() { 16.0 } else { 0.0 };
                    let inner = if bipolar {
                        Rect::from_min_max(
                            cell.min + vec2(5.0, 8.0),
                            cell.max - vec2(5.0, 8.0 + foot),
                        )
                    } else {
                        let c = cell.shrink2(vec2(4.0, 0.0));
                        p.rect(c, 4.0, well, Stroke::new(1.0, r.line), StrokeKind::Inside);
                        c.shrink(3.0)
                    };
                    let (lo, hi) = if bipolar { (-1.0, 1.0) } else { (0.0, 1.0) };
                    if hit && (resp.dragged() || resp.is_pointer_button_down_on()) {
                        let t =
                            1.0 - ((ptr.unwrap().y - inner.top()) / inner.height()).clamp(0.0, 1.0);
                        let nv = lo + t * (hi - lo);
                        if (nv - *v).abs() > f32::EPSILON {
                            *v = nv;
                            changed = true;
                        }
                    }
                    let t = ((*v - lo) / (hi - lo)).clamp(0.0, 1.0);
                    let base: f32 = if bipolar { 0.5 } else { 0.0 };
                    let (a, b) = (base.min(t), base.max(t));
                    let mut bar = Rect::from_min_max(
                        pos2(inner.left(), inner.bottom() - b * inner.height()),
                        pos2(inner.right(), inner.bottom() - a * inner.height()),
                    );
                    if bar.height() < 2.0 {
                        bar = Rect::from_center_size(bar.center(), vec2(bar.width(), 2.0));
                    }
                    fill_rr(ui, bar, if bipolar { 3.0 } else { 2.0 }, col, 0.0);
                    if let Some(f) = lane.readout {
                        p.text(
                            pos2(cell.center().x, cell.bottom() - 9.0),
                            egui::Align2::CENTER_CENTER,
                            f(*v),
                            mono.clone(),
                            r.text2,
                        );
                    }
                }
                LaneKind::Gate => {
                    if hit && resp.clicked() {
                        *v = if *v > 0.5 { 0.0 } else { 1.0 };
                        changed = true;
                    }
                    let pill = cell.shrink2(vec2(4.0, 0.0));
                    if *v > 0.5 {
                        fill_rr(ui, pill, h / 2.0, col, 0.0);
                    } else {
                        p.rect(
                            pill,
                            h / 2.0,
                            well,
                            Stroke::new(1.0, r.line2),
                            StrokeKind::Inside,
                        );
                    }
                }
                LaneKind::Count { max } => {
                    if hit && resp.clicked() {
                        let n = v.round() as u8;
                        *v = f32::from(if n >= max { 1 } else { n + 1 });
                        changed = true;
                    }
                    let n = v.round() as u8;
                    p.text(
                        cell.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("×{n}"),
                        mono.clone(),
                        if n > 1 { r.text } else { r.text2 },
                    );
                }
            }
        }
        if lane.kind == (LaneKind::Bars { bipolar: true }) {
            let mid = lane_r.top()
                + 8.0
                + (h - 16.0 - if lane.readout.is_some() { 16.0 } else { 0.0 }) / 2.0;
            p.hline(
                lane_r.shrink2(vec2(4.0, 0.0)).x_range(),
                mid,
                Stroke::new(1.0, r.line2),
            );
        }
        y += h + LANE_GAP;
    }
    if let Some(i) = playhead.filter(|&i| i < steps) {
        let col = Rect::from_min_max(
            pos2(col_x(i) + 1.0, rect.top() - 4.0),
            pos2(col_x(i + 1) - 1.0, rect.bottom() + 4.0),
        );
        p.rect_stroke(
            col,
            CornerRadius::same(6),
            Stroke::new(1.0, alpha(r.accent, 153)),
            StrokeKind::Inside,
        );
    }
    if changed {
        resp.mark_changed();
        ui.ctx().request_repaint();
    }
    resp
}

/// Values of a [`range_bar`]: the parameter's limits, its base, the span modulation sweeps, and now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    pub min: f32,
    pub max: f32,
    pub base: f32,
    pub lo: f32,
    pub hi: f32,
    pub now: f32,
}

/// Modulation range: a 10 px track, the base marker, the swept span in the cv colour, and
/// mono min / now / max below. `lo`..`hi` is the span the modulation reaches.
pub fn range_bar(
    ui: &mut Ui,
    st: &Style,
    width: f32,
    v: Range,
    fmt: impl Fn(f32) -> String,
) -> Response {
    let Range {
        min,
        max,
        base,
        lo,
        hi,
        now,
    } = v;
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 18.0 + 16.0), Sense::hover());
    let r = &st.roles;
    let span = (max - min).max(f32::EPSILON);
    let x = |v: f32| rect.left() + width * ((v - min) / span).clamp(0.0, 1.0);
    let track = Rect::from_min_size(pos2(rect.left(), rect.top() + 4.0), vec2(width, 10.0));
    fill_rr(ui, track, 5.0, r.inset, 0.0);
    let swept = Rect::from_min_max(pos2(x(lo), track.top()), pos2(x(hi), track.bottom()));
    fill_rr(ui, swept, 5.0, alpha(r.cv, 115), 0.0);
    ui.painter().rect_filled(
        Rect::from_center_size(pos2(x(base), track.center().y), vec2(3.0, 18.0)),
        CornerRadius::same(1),
        r.text,
    );
    let font = st.font(Role::Value);
    let y = rect.bottom() - 7.0;
    let w = |s: &str| {
        ui.painter()
            .layout_no_wrap(s.to_string(), font.clone(), r.text2)
            .size()
            .x
    };
    let (smin, smax, snow) = (fmt(min), fmt(max), fmt(now));
    let lo_edge = rect.left() + w(&smin) + 8.0 + w(&snow) / 2.0;
    let hi_edge = rect.right() - w(&smax) - 8.0 - w(&snow) / 2.0;
    let nx = if lo_edge <= hi_edge {
        x(now).clamp(lo_edge, hi_edge)
    } else {
        rect.center().x
    };
    for (t, al, px) in [
        (smin, egui::Align2::LEFT_CENTER, rect.left()),
        (snow, egui::Align2::CENTER_CENTER, nx),
        (smax, egui::Align2::RIGHT_CENTER, rect.right()),
    ] {
        ui.painter().text(pos2(px, y), al, t, font.clone(), r.text2);
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{contrast, line_solid_default};

    #[test]
    fn scrim_never_white_and_shadow_lighter_on_paper() {
        let (d, l) = (line_solid_default(true), line_solid_default(false));
        assert_eq!(tokens(&d).scrim.to_array()[..3], [0, 0, 0]);
        let ls = tokens(&l).scrim;
        assert!(ls.r() < 40 && ls.a() < 100);
        assert!(tokens(&l).shadow_base < tokens(&d).shadow_base);
    }

    #[test]
    fn state_text_has_contrast_on_surfaces_and_tints() {
        for dark in [true, false] {
            let st = line_solid_default(dark);
            for sev in [Sev::Bad, Sev::Warn, Sev::Good] {
                let c = sev.color(&st);
                for (name, bg) in [("surface", st.roles.surface), ("raised", st.roles.raised)] {
                    let k = contrast(c, bg);
                    assert!(k >= 3.0, "{sev:?} on {name} dark={dark}: {k:.2}");
                }
                // Banner title is `text` on the composited tint.
                let bg = mix(st.roles.raised, c, f32::from(tint(&st, c).a()) / 255.0);
                let k = contrast(st.roles.text, bg);
                assert!(k >= 4.5, "text on {sev:?} tint dark={dark}: {k:.2}");
            }
        }
    }

    #[test]
    fn toast_lifetimes() {
        let mk = |sev| Toast {
            sev,
            title: "t".into(),
            detail: None,
            born: 0.0,
        };
        let ctx = egui::Context::default();
        let st = line_solid_default(true);
        let mut v = vec![mk(Sev::Bad), mk(Sev::Warn), mk(Sev::Good)];
        let area = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        let _ = ctx.run_ui(Default::default(), |ui| {
            toasts(ui.ctx(), &st, area, &mut v, 4.0);
        });
        assert_eq!(v.len(), 2);
        let _ = ctx.run_ui(Default::default(), |ui| {
            toasts(ui.ctx(), &st, area, &mut v, 9.0);
        });
        assert_eq!(v.len(), 1);
        assert_eq!(error_count(&v), 1);
    }
}
