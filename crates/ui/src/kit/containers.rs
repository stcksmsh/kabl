//! Frames, popups, dropdowns, menus, tooltips, dialogs and the egui style bridge.

use super::controls::{fill_rr, focus_ring};
use super::{dim, ease_to, icon, label, paragraph, rich, Ic, Tone};
use crate::style::{alpha, mix, Rad, Role, Style};
use egui::{
    pos2, vec2, Color32, Context, CornerRadius, Frame, Margin, Popup, PopupCloseBehavior, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui,
    WidgetInfo, WidgetType,
};

pub(crate) fn round_points(r: Rect, rad: f32) -> Vec<Pos2> {
    let rad = rad.min(r.width() / 2.0).min(r.height() / 2.0).max(0.0);
    let n = if rad < 1.0 { 1 } else { 6 };
    let corners = [
        (pos2(r.right() - rad, r.top() + rad), -std::f32::consts::FRAC_PI_2),
        (pos2(r.right() - rad, r.bottom() - rad), 0.0),
        (pos2(r.left() + rad, r.bottom() - rad), std::f32::consts::FRAC_PI_2),
        (pos2(r.left() + rad, r.top() + rad), std::f32::consts::PI),
    ];
    let mut v = Vec::new();
    for (c, a0) in corners {
        for i in 0..=n {
            let a = a0 + std::f32::consts::FRAC_PI_2 * i as f32 / n as f32;
            v.push(c + vec2(a.cos(), a.sin()) * rad);
        }
    }
    v
}

fn eshadow(st: &Style, level: usize) -> egui::epaint::Shadow {
    let s = st.shadows[level.min(2)];
    egui::epaint::Shadow { offset: [0, s.dy as i8], blur: s.blur as u8, spread: 0, color: alpha(Color32::BLACK, s.alpha) }
}

/// Paint the theme's shadow `level` under a rect.
pub fn shadow(ui: &Ui, st: &Style, rect: Rect, radius: f32, level: usize) {
    if st.shadows[level.min(2)].alpha > 0 {
        ui.painter().add(eshadow(st, level).as_shape(rect, CornerRadius::same(radius as u8)));
    }
}

/// Side panels (Sounds, Routing, Perform).
pub fn panel_frame(st: &Style) -> Frame {
    Frame::new().fill(st.roles.surface).inner_margin(Margin::symmetric(st.sp(3) as i8, st.sp(2) as i8))
}

/// Toolbar and status bar.
pub fn bar_frame(st: &Style) -> Frame {
    Frame::new().fill(st.roles.surface).inner_margin(Margin::symmetric(st.sp(3) as i8, st.sp(2) as i8))
}

pub fn popover_frame(st: &Style) -> Frame {
    Frame::new()
        .fill(st.roles.raised)
        .stroke(Stroke::new(st.slots.controls.outline, st.roles.line))
        .corner_radius(CornerRadius::same(st.radius(Rad::Md) as u8))
        .shadow(eshadow(st, st.slots.controls.popover_shadow))
        .inner_margin(Margin::same(st.sp(2) as i8))
}

pub fn card_frame(st: &Style) -> Frame {
    Frame::new()
        .fill(st.roles.raised)
        .stroke(Stroke::new(st.slots.controls.outline, st.roles.line))
        .corner_radius(CornerRadius::same(st.radius(Rad::Md) as u8))
        .shadow(eshadow(st, st.slots.controls.card_shadow))
        .inner_margin(Margin::same(st.sp(3) as i8))
}

pub fn card<R>(ui: &mut Ui, st: &Style, add: impl FnOnce(&mut Ui) -> R) -> R {
    card_frame(st).show(ui, add).inner
}

/// Row background for list items: selected, hovered or plain. Returns the content rect.
pub fn row_bg(ui: &Ui, st: &Style, rect: Rect, selected: bool, hover: f32) {
    let r = &st.roles;
    let radius = st.radius(Rad::Md);
    if selected {
        fill_rr(ui, rect, radius, r.accent_soft, 0.0);
        let bar = Rect::from_min_size(rect.min + vec2(0.0, st.sp(2)), vec2(3.0, rect.height() - st.sp(2) * 2.0));
        fill_rr(ui, bar, 1.5, r.accent, 0.0);
    } else if hover > 0.0 {
        fill_rr(ui, rect, radius, alpha(r.inset, (255.0 * hover) as u8), 0.0);
    }
}

/// A full-width row in a menu or dropdown. A check mark shows the current choice.
pub fn menu_item(ui: &mut Ui, st: &Style, text: &str, selected: bool) -> Response {
    menu_item_tone(ui, st, text, selected, Tone::Text, true)
}

pub fn menu_item_tone(ui: &mut Ui, st: &Style, text: &str, selected: bool, t: Tone, enabled: bool) -> Response {
    let enabled = enabled && ui.is_enabled();
    let h = st.metrics.control_h + 2.0;
    let w = ui.available_width().max(120.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), if enabled { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, enabled, selected, text));
    let hover = ease_to(ui, resp.id.with("hover"), enabled && resp.hovered(), st.motion.hover);
    row_bg(ui, st, rect, false, hover);
    if selected {
        icon(ui.painter(), Ic::Check, pos2(rect.left() + st.sp(3), rect.center().y), st.metrics.icon, st.roles.accent);
    }
    let ink = dim(super::tone(st, t), enabled);
    let g = ui.painter().layout_no_wrap(text.to_string(), st.font(Role::Body), ink);
    ui.painter().galley(pos2(rect.left() + st.sp(3) + st.metrics.icon + st.sp(1), rect.center().y - g.size().y / 2.0), g, ink);
    if enabled && resp.has_focus() {
        focus_ring(ui, st, rect, st.radius(Rad::Md));
    }
    resp
}

/// Closed field with a chevron that opens a popover list.
pub fn dropdown<R>(ui: &mut Ui, st: &Style, id: impl std::hash::Hash + std::fmt::Debug, current: &str, width: f32, items: impl FnOnce(&mut Ui) -> R) -> Response {
    let enabled = ui.is_enabled();
    let h = st.metrics.control_h;
    let (rect, _) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
    let resp = ui.interact(rect, ui.id().with(id), if enabled { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::ComboBox, enabled, current));
    let hover = ease_to(ui, resp.id.with("hover"), enabled && resp.hovered(), st.motion.hover);
    let r = &st.roles;
    let radius = st.shape_radius(st.slots.controls.field, h);
    fill_rr(ui, rect, radius, dim(r.inset, enabled), 0.0);
    ui.painter().rect_stroke(rect, CornerRadius::same(radius as u8), Stroke::new(1.0, mix(r.line, r.line2, hover)), StrokeKind::Inside);
    let ink = dim(r.text, enabled);
    let text = {
        let avail = rect.width() - st.sp(3) - st.metrics.icon - st.sp(3);
        fit_text(ui, st, current, avail)
    };
    let g = ui.painter().layout_no_wrap(text, st.font(Role::Body), ink);
    ui.painter().galley(pos2(rect.left() + st.sp(2) + 2.0, rect.center().y - g.size().y / 2.0), g, ink);
    icon(ui.painter(), Ic::Down, pos2(rect.right() - st.sp(2) - st.metrics.icon / 2.0, rect.center().y), st.metrics.icon, dim(r.text2, enabled));
    if enabled && resp.has_focus() {
        focus_ring(ui, st, rect, radius);
    }
    Popup::from_toggle_button_response(&resp)
        .close_behavior(PopupCloseBehavior::CloseOnClick)
        .frame(popover_frame(st))
        .width(rect.width().max(160.0))
        .show(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            items(ui)
        });
    resp
}

/// Text shortened with an ellipsis to fit `max_w` in the body role.
pub fn fit_text(ui: &Ui, st: &Style, s: &str, max_w: f32) -> String {
    let width = |t: &str| ui.painter().layout_no_wrap(t.to_string(), st.font(Role::Body), Color32::WHITE).size().x;
    if width(s) <= max_w {
        return s.to_string();
    }
    let mut chars: Vec<char> = s.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let t = chars.iter().collect::<String>() + "…";
        if width(&t) <= max_w {
            return t;
        }
    }
    String::new()
}

/// A button that opens a menu of [`menu_item`]s and other widgets.
pub fn menu_button<R>(ui: &mut Ui, st: &Style, text: &str, content: impl FnOnce(&mut Ui) -> R) -> Response {
    let resp = ui.add(super::Button::new(st, text).icon(Ic::Down));
    Popup::menu(&resp).frame(popover_frame(st)).close_behavior(PopupCloseBehavior::CloseOnClickOutside).width(260.0).show(|ui| content(ui));
    resp
}

/// Right-click menu on `resp`.
pub fn context_menu<R>(resp: &Response, st: &Style, content: impl FnOnce(&mut Ui) -> R) {
    Popup::context_menu(resp).frame(popover_frame(st)).close_behavior(PopupCloseBehavior::CloseOnClick).width(220.0).show(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        content(ui)
    });
}

/// Hover help in the theme's type: `text` is the whole message.
pub trait Tip: Sized {
    fn tip(self, st: &Style, text: &str) -> Self;
    fn tip_titled(self, st: &Style, title: &str, body: &str) -> Self;
}

impl Tip for Response {
    fn tip(self, st: &Style, text: &str) -> Self {
        let show = |ui: &mut Ui| {
            ui.set_max_width(320.0);
            paragraph(ui, st, Role::Body, Tone::Text, text);
        };
        self.on_disabled_hover_ui(show).on_hover_ui(show)
    }

    fn tip_titled(self, st: &Style, title: &str, body: &str) -> Self {
        let show = |ui: &mut Ui| {
            ui.set_max_width(320.0);
            label(ui, st, Role::H3, Tone::Text, title);
            paragraph(ui, st, Role::Body, Tone::Text2, body);
        };
        self.on_disabled_hover_ui(show).on_hover_ui(show)
    }
}

/// A centred modal dialog. Buttons inside are ordinary kit buttons.
pub fn modal<T>(ctx: &Context, st: &Style, id: &str, width: f32, content: impl FnOnce(&mut Ui) -> T) -> egui::ModalResponse<T> {
    let frame = Frame::new()
        .fill(st.roles.raised)
        .stroke(Stroke::new(st.slots.controls.outline, st.roles.line))
        .corner_radius(CornerRadius::same(st.radius(Rad::Lg) as u8))
        .shadow(eshadow(st, 2))
        .inner_margin(Margin::same(st.sp(4) as i8));
    egui::Modal::new(egui::Id::new(id)).frame(frame).backdrop_color(alpha(st.roles.hole, 150)).show(ctx, |ui| {
        ui.set_width(width);
        content(ui)
    })
}

/// Dialog heading and body helpers keep dialogs in the type scale.
pub fn dialog_title(ui: &mut Ui, st: &Style, text: &str) {
    ui.label(rich(st, Role::Title, Tone::Text, text));
    ui.add_space(st.sp(1));
}

/// Install the theme into egui's own style so stock-drawn widgets elsewhere follow it. Runs
/// only when the theme changes.
pub fn apply(ctx: &Context, st: &Style) {
    let key = egui::Id::new("kabl-style-applied");
    let tag = (st.id.clone(), st.dark);
    if ctx.data(|d| d.get_temp::<(String, bool)>(key)).as_ref() == Some(&tag) {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(key, tag));
    let r = &st.roles;
    let mut v = if st.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = r.surface;
    v.window_fill = r.raised;
    v.extreme_bg_color = r.inset;
    v.faint_bg_color = r.inset;
    v.override_text_color = Some(r.text);
    v.window_stroke = Stroke::new(st.slots.controls.outline, r.line);
    v.window_corner_radius = CornerRadius::same(st.radius(Rad::Md) as u8);
    v.menu_corner_radius = CornerRadius::same(st.radius(Rad::Md) as u8);
    v.window_shadow = eshadow(st, 2);
    v.popup_shadow = eshadow(st, 2);
    v.selection.bg_fill = r.accent_soft;
    v.selection.stroke = Stroke::new(1.0, r.accent);
    v.text_cursor.stroke = Stroke::new(2.0, r.text);
    v.hyperlink_color = r.focus;
    v.warn_fg_color = r.warn;
    v.error_fg_color = r.bad;
    let cr = CornerRadius::same((st.radii.sm + 1.0) as u8);
    v.widgets.noninteractive.bg_fill = r.surface;
    v.widgets.noninteractive.bg_stroke.color = r.line;
    v.widgets.noninteractive.fg_stroke.color = r.text2;
    for (w, fill) in [(&mut v.widgets.inactive, r.inset), (&mut v.widgets.hovered, mix(r.inset, r.raised, 0.5)), (&mut v.widgets.active, r.accent_soft), (&mut v.widgets.open, r.inset)] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.corner_radius = cr;
        w.bg_stroke.color = r.line;
        w.fg_stroke.color = r.text;
    }
    v.widgets.noninteractive.corner_radius = cr;
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.scroll = egui::style::ScrollStyle { bar_width: st.metrics.scroll_w, ..egui::style::ScrollStyle::floating() };
    });
}
