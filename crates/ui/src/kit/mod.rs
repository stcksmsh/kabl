//! The chrome widget kit. Every colour, size, radius, shadow and duration comes from a
//! [`Style`]; the only literals here are hit-area metrics (`Style::metrics`) and geometry.
//!
//! Widgets are ordinary `egui::Widget`s or functions returning a `Response`, so call sites keep
//! their `ui_state.record(..)` hit keys and `clicked()` logic. Each widget has hover, pressed,
//! keyboard-focus and disabled states.

mod containers;
mod controls;
mod icons;

pub use containers::*;
pub use controls::*;
pub use icons::{icon, Ic};

use crate::style::{alpha, Role, Style};
use egui::{Color32, Response, RichText, Ui};

/// Which text colour a label takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Text,
    Text2,
    Text3,
    Accent,
    OnAccent,
    Good,
    Warn,
    Bad,
}

pub fn tone(st: &Style, t: Tone) -> Color32 {
    let r = &st.roles;
    match t {
        Tone::Text => r.text,
        Tone::Text2 => r.text2,
        Tone::Text3 => r.text3,
        Tone::Accent => r.accent,
        Tone::OnAccent => r.on_accent,
        Tone::Good => r.good,
        Tone::Warn => r.warn,
        Tone::Bad => r.bad,
    }
}

/// Styled rich text for a role, with caps applied when the role asks for it.
pub fn rich(st: &Style, role: Role, t: Tone, s: impl AsRef<str>) -> RichText {
    let tr = st.type_role(role);
    let text = if tr.caps {
        s.as_ref().to_uppercase()
    } else {
        s.as_ref().to_string()
    };
    RichText::new(text).font(st.font(role)).color(tone(st, t))
}

/// A non-interactive label in a type role.
pub fn label(ui: &mut Ui, st: &Style, role: Role, t: Tone, s: impl AsRef<str>) -> Response {
    ui.add(egui::Label::new(rich(st, role, t, s)).selectable(false))
}

/// A label that truncates with an ellipsis to the space left.
pub fn label_truncated(
    ui: &mut Ui,
    st: &Style,
    role: Role,
    t: Tone,
    s: impl AsRef<str>,
) -> Response {
    ui.add(
        egui::Label::new(rich(st, role, t, s))
            .selectable(false)
            .truncate(),
    )
}

/// A wrapping paragraph.
pub fn paragraph(ui: &mut Ui, st: &Style, role: Role, t: Tone, s: impl AsRef<str>) -> Response {
    ui.add(
        egui::Label::new(rich(st, role, t, s))
            .selectable(false)
            .wrap(),
    )
}

/// Section heading: the theme's `section` type role (caps, tracked).
pub fn section(ui: &mut Ui, st: &Style, s: impl AsRef<str>) -> Response {
    let tr = &st.types.section;
    let text = if tr.caps {
        s.as_ref().to_uppercase()
    } else {
        s.as_ref().to_string()
    };
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &text,
        0.0,
        egui::TextFormat {
            font_id: st.section_font(),
            color: st.roles.text2,
            extra_letter_spacing: tr.tracking,
            ..Default::default()
        },
    );
    ui.add(egui::Label::new(job).selectable(false))
}

/// Hairline rule across the available width.
pub fn rule(ui: &mut Ui, st: &Style) {
    let (r, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().rect_filled(r, 0.0, st.roles.line);
}

/// Gap of `step` on the spacing scale.
pub fn gap(ui: &mut Ui, st: &Style, step: usize) {
    ui.add_space(st.sp(step));
}

/// Eased 0..=1 value toward `on`, over `ms`.
pub(crate) fn ease_to(ui: &Ui, id: egui::Id, on: bool, ms: f32) -> f32 {
    ui.ctx().animate_bool_with_time(id, on, Style::secs(ms))
}

pub(crate) fn dim(c: Color32, enabled: bool) -> Color32 {
    if enabled {
        c
    } else {
        alpha(c, (f32::from(c.a()) * 0.45) as u8)
    }
}
