//! The chrome widget kit. Every colour, size, radius, shadow and duration comes from a
//! [`Style`]; the only literals here are hit-area metrics (`Style::metrics`) and geometry.
//!
//! Widgets are ordinary `egui::Widget`s or functions returning a `Response`, so call sites keep
//! their `ui_state.record(..)` hit keys and `clicked()` logic. Each widget has hover, pressed,
//! keyboard-focus and disabled states.

pub mod blur;
mod containers;
mod controls;
pub mod face;
pub mod g4;
mod icons;
pub mod parts;
mod recipes;

pub use containers::*;
pub use controls::*;
pub use icons::{icon, Ic};
pub use recipes::*;

/// The kabl lockup (`assets/logo`), tinted from the theme's text colour.
pub fn logo(ui: &mut egui::Ui, st: &crate::style::Style, height: f32) -> egui::Response {
    let id = egui::Id::new("kabl-logo");
    let tex = ui.ctx().data(|d| d.get_temp::<egui::TextureHandle>(id));
    let tex = tex.unwrap_or_else(|| {
        let img = image::load_from_memory(include_bytes!("../../assets/logo/kabl-logo-mask.png"))
            .expect("bundled logo")
            .to_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        let tex = ui.ctx().load_texture(
            "kabl-logo",
            egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()),
            // Mipmaps: the 948 px mask is drawn at ~87 px, which aliases without them.
            egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
        );
        ui.ctx().data_mut(|d| d.insert_temp(id, tex.clone()));
        tex
    });
    let s = tex.size_vec2();
    ui.add(
        egui::Image::new(&tex)
            .tint(st.roles.text)
            .fit_to_exact_size(egui::vec2(height * s.x / s.y, height)),
    )
}

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
        Tone::Accent => r.accent_text,
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

/// A CSS-style cubic-bezier easing curve `(x1, y1, x2, y2)`: progress 0..=1 in, eased 0..=1 out.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Bezier(pub f32, pub f32, pub f32, pub f32);

impl Bezier {
    /// Drawer in, toast in (HANDOVER section 6).
    pub const DRAWER_IN: Self = Self(0.0, 0.0, 0.0, 1.0);
    /// Drawer out, toast out.
    pub const DRAWER_OUT: Self = Self(0.3, 0.0, 1.0, 1.0);
    /// View crossfade.
    pub const VIEW: Self = Self(0.2, 0.0, 0.0, 1.0);

    /// Eased value at progress `x`.
    pub fn at(self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        let Self(x1, y1, x2, y2) = self;
        let at = |a: f32, b: f32, t: f32| {
            let u = 1.0 - t;
            3.0 * u * u * t * a + 3.0 * u * t * t * b + t * t * t
        };
        // Solve x(t) = x by bisection: monotone for the control points used here.
        let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
        for _ in 0..24 {
            let mid = 0.5 * (lo + hi);
            if at(x1, x2, mid) < x {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        at(y1, y2, 0.5 * (lo + hi))
    }
}

/// Open/close value 0..=1 for a drawer-like surface: `ms` to open on `open_curve`, 0.7 x that
/// to close on `close_curve`. Reversing mid-way continues from where it is. Repaints only while
/// moving.
pub(crate) fn open_close(ctx: &egui::Context, id: egui::Id, open: bool, ms: f32) -> f32 {
    let secs = Style::secs(if open { ms } else { ms * 0.7 });
    let lin = ctx.animate_bool_with_time(id, open, secs);
    if lin <= 0.0 || lin >= 1.0 {
        return lin;
    }
    if open {
        Bezier::DRAWER_IN.at(lin)
    } else {
        1.0 - Bezier::DRAWER_OUT.at(1.0 - lin)
    }
}

/// One exponential-smoothing step of `v` toward `target` over `dt` seconds, time constant `ms`.
pub(crate) fn glide_step(v: f32, target: f32, dt: f32, ms: f32) -> f32 {
    let k = (-dt / Style::secs(ms).max(1e-3)).exp();
    target + (v - target) * k
}

/// Exponential smoothing toward `target`: time constant `ms` (a `style.motion` token), so it
/// follows a moving target without the linear ramp of `ease_to`. Keeps the frame repainting
/// until it settles; a first call starts at `target`.
pub(crate) fn smooth_to(ctx: &egui::Context, id: egui::Id, target: f32, ms: f32) -> f32 {
    let v = ctx.data(|d| d.get_temp::<f32>(id)).unwrap_or(target);
    let v = glide_step(v, target, ctx.input(|i| i.stable_dt), ms);
    ctx.data_mut(|d| d.insert_temp(id, v));
    if (v - target).abs() > 1e-3 {
        ctx.request_repaint();
    }
    v
}

pub(crate) fn dim(c: Color32, enabled: bool) -> Color32 {
    if enabled {
        c
    } else {
        alpha(c, (f32::from(c.a()) * 0.45) as u8)
    }
}

#[cfg(test)]
mod motion_tests {
    use super::Bezier;

    #[test]
    fn curves_run_from_zero_to_one_without_going_back() {
        for c in [Bezier::DRAWER_IN, Bezier::DRAWER_OUT, Bezier::VIEW] {
            assert!(c.at(0.0).abs() < 1e-4 && (c.at(1.0) - 1.0).abs() < 1e-4);
            let mut last = 0.0;
            for i in 0..=100 {
                let y = c.at(i as f32 / 100.0);
                assert!(y >= last - 1e-4, "{c:?} dips at {i}");
                last = y;
            }
        }
        // Ease-out is ahead of linear mid-way; ease-in is behind it.
        assert!(Bezier::DRAWER_IN.at(0.5) > 0.5);
        assert!(Bezier::DRAWER_OUT.at(0.5) < 0.5);
    }

    #[test]
    fn glide_step_approaches_without_overshoot() {
        let mut v = 0.0;
        for _ in 0..600 {
            let n = super::glide_step(v, 1.0, 1.0 / 60.0, 60.0);
            assert!(n >= v && n <= 1.0);
            v = n;
        }
        assert!((v - 1.0).abs() < 1e-4);
    }
}
