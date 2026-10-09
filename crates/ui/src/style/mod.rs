//! The theme model: tokens, section palettes and recipe slots (docs/design/app-target/THEMING.md).
//!
//! A [`Style`] is plain data. Built-in themes are constructed in `builtin.rs`; a later loader
//! will fill the same structs from files. The widget kit (`crate::kit`) reads everything it
//! draws from here: colours, type, spacing, radii, shadows, motion and recipe choices. Hit-area
//! sizes live in [`Metrics`] and are not themable.

mod builtin;

pub use builtin::{builtin_a, BUILTIN_A_ID};

use egui::{Color32, FontFamily, FontId};
use serde::{Deserialize, Serialize};

/// Smallest text size anywhere in the chrome. Not themable.
pub const TEXT_FLOOR: f32 = 11.0;

mod hex {
    use egui::Color32;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(c: &Color32, s: S) -> Result<S::Ok, S::Error> {
        if c.a() == 255 {
            s.serialize_str(&format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b()))
        } else {
            let [r, g, b, a] = c.to_srgba_unmultiplied();
            s.serialize_str(&format!("#{r:02x}{g:02x}{b:02x}{a:02x}"))
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Color32, D::Error> {
        let s = String::deserialize(d)?;
        Color32::from_hex(&s).map_err(|_| serde::de::Error::custom(format!("bad colour {s}")))
    }
}

macro_rules! roles {
    ($($f:ident),* $(,)?) => {
        /// Colour roles. Every role is required in a complete theme.
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        pub struct Roles {
            $(#[serde(with = "hex")] pub $f: Color32,)*
        }
        impl Roles {
            pub const NAMES: &'static [&'static str] = &[$(stringify!($f)),*];
        }
    };
}

roles!(
    bg, surface, raised, inset, line, line2, text, text2, text3, accent, on_accent, accent_soft,
    focus, audio, cv, gate, good, warn, bad, rack, rail, hole, disp_bg, disp_grid, disp_trace,
    disp_text, knob, knob_hi, knob_ink,
);

/// One module-section colour family.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Section {
    #[serde(with = "hex")]
    pub base: Color32,
    #[serde(with = "hex")]
    pub ink: Color32,
    #[serde(with = "hex")]
    pub ink2: Color32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sections {
    pub osc: Section,
    pub filter: Section,
    pub modulation: Section,
    pub amp: Section,
    pub timing: Section,
    pub fx: Section,
}

/// Fonts a theme may name. v1 themes cannot embed fonts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Font {
    Sans,
    SansMedium,
    SansSemibold,
    Mono,
    MonoMedium,
}

impl Font {
    /// `ready` is false until egui has applied the installed font set; until then the plain
    /// families stand in.
    pub fn family(self, ready: bool) -> FontFamily {
        match self {
            Font::Sans => FontFamily::Proportional,
            Font::Mono => FontFamily::Monospace,
            Font::SansMedium | Font::SansSemibold if !ready => FontFamily::Proportional,
            Font::MonoMedium if !ready => FontFamily::Monospace,
            Font::SansMedium => FontFamily::Name("kabl-sans-medium".into()),
            Font::SansSemibold => FontFamily::Name("kabl-sans-semibold".into()),
            Font::MonoMedium => FontFamily::Name("kabl-mono-medium".into()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TypeRole {
    pub font: Font,
    pub size: f32,
    /// Upper-case the text.
    #[serde(default)]
    pub caps: bool,
    /// Extra advance per character, in px (only used with `caps`).
    #[serde(default)]
    pub tracking: f32,
}

/// Seven type roles; the order is the hierarchy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Display,
    Title,
    H3,
    Body,
    Label,
    Value,
    Caption,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TypeScale {
    pub display: TypeRole,
    pub title: TypeRole,
    pub h3: TypeRole,
    pub body: TypeRole,
    pub label: TypeRole,
    pub value: TypeRole,
    pub caption: TypeRole,
    /// Section headings ("FACTORY", "AUDITION"): a label role set in caps.
    pub section: TypeRole,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Radii {
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shadow {
    pub dy: f32,
    pub blur: f32,
    /// Alpha of the black shadow, 0..=255.
    pub alpha: u8,
}

/// Durations in milliseconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Motion {
    pub hover: f32,
    pub press: f32,
    pub drawer: f32,
    pub view: f32,
    pub glide: f32,
    pub pulse: f32,
}

macro_rules! recipes {
    ($($(#[$m:meta])* $name:ident { $($v:ident),* $(,)? })*) => {$(
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "kebab-case")]
        pub enum $name { $($v),* }
    )*};
}

recipes! {
    FaceMaterial { Satin, Lacquer, Flat }
    FaceHardware { Nuts, None, Screws }
    FaceTitle { Centered, LeftDot, CapsRule }
    OutPlate { Tint, Dark, Outline }
    RackSurface { Plain, Grid }
    KnobBody { Cap, Glass, Disc }
    KnobScale { Ticks, TrackArc, None }
    KnobValue { Line, Dot, Notch, Arc }
    KnobMod { ArcDot, ArcGlow, Segment }
    SelectorRecipe { InsetPill, AccentPill, Outlined }
    JackRecipe { Nut, GlowRing, Donut }
    CableRecipe { Rope, Glow, Outlined }
    PlugRecipe { Cap, Ring, Dot }
    DisplayFrame { Glass, Neon, Paper }
    DisplayTrace { SoftGlow, StrongGlow, Crisp }
    /// Corner treatment of buttons, chips, fields and rows.
    Shape { Rounded, Pill, Square }
    ViewSwitch { Segmented, Tabs }
    Navigator { Left, PresetBar }
    BrowserRows { Cards, Table }
    Icons { Stroke }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FaceSlots {
    pub material: FaceMaterial,
    /// Gradient depth toward white at the top and black at the bottom, 0..=1.
    pub gradient: f32,
    pub gloss: f32,
    pub grain: f32,
    pub hardware: FaceHardware,
    pub title: FaceTitle,
    pub out_plate: OutPlate,
    pub shadow: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KnobSlots {
    pub body: KnobBody,
    pub scale: KnobScale,
    pub value: KnobValue,
    pub modulation: KnobMod,
    pub glow: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CableSlots {
    pub recipe: CableRecipe,
    pub width: f32,
    pub sag: f32,
    pub glow: f32,
    pub dimmed: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DisplaySlots {
    pub frame: DisplayFrame,
    pub trace: DisplayTrace,
    pub glow: f32,
    pub fill: f32,
}

/// How the chrome's widgets are shaped and finished.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ControlSlots {
    pub button: Shape,
    pub chip: Shape,
    pub field: Shape,
    /// Fill of the primary button: 0 flat, 1 full-depth gradient.
    pub gradient: f32,
    /// Outline width of cards and popovers.
    pub outline: f32,
    pub focus_ring: f32,
    pub card_shadow: usize,
    pub popover_shadow: usize,
    pub row: BrowserRows,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChromeSlots {
    pub view_switch: ViewSwitch,
    pub navigator: Navigator,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Slots {
    pub face: FaceSlots,
    pub rack: RackSurface,
    pub knob: KnobSlots,
    pub selector: SelectorRecipe,
    pub jack: JackRecipe,
    pub cable: CableSlots,
    pub plug: PlugRecipe,
    pub display: DisplaySlots,
    pub controls: ControlSlots,
    pub chrome: ChromeSlots,
    pub icons: Icons,
}

/// Sizes the app fixes: hit areas, panel widths. Not part of a theme file.
#[derive(Clone, Debug, PartialEq)]
pub struct Metrics {
    pub control_h: f32,
    pub small_h: f32,
    pub field_h: f32,
    pub row_h: f32,
    pub icon: f32,
    pub toolbar_h: f32,
    pub status_h: f32,
    pub rail_w: f32,
    pub slider_h: f32,
    pub toggle: egui::Vec2,
    pub scroll_w: f32,
}

impl Default for Metrics {
    fn default() -> Self {
        Metrics {
            control_h: 28.0,
            small_h: 24.0,
            field_h: 30.0,
            row_h: 44.0,
            icon: 14.0,
            toolbar_h: 64.0,
            status_h: 30.0,
            rail_w: 44.0,
            slider_h: 20.0,
            toggle: egui::vec2(32.0, 18.0),
            scroll_w: 6.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Style {
    pub id: String,
    pub name: String,
    pub dark: bool,
    pub roles: Roles,
    pub sections: Sections,
    pub types: TypeScale,
    /// Eight steps, smallest first.
    pub space: [f32; 8],
    pub radii: Radii,
    pub shadows: [Shadow; 3],
    pub motion: Motion,
    pub slots: Slots,
    #[serde(skip)]
    pub metrics: Metrics,
    /// The custom font families are live in egui (set by `show`, not themable).
    #[serde(skip)]
    pub fonts_ready: bool,
}

impl Style {
    pub fn type_role(&self, r: Role) -> &TypeRole {
        match r {
            Role::Display => &self.types.display,
            Role::Title => &self.types.title,
            Role::H3 => &self.types.h3,
            Role::Body => &self.types.body,
            Role::Label => &self.types.label,
            Role::Value => &self.types.value,
            Role::Caption => &self.types.caption,
        }
    }

    /// The font for a role, never below [`TEXT_FLOOR`].
    pub fn font(&self, r: Role) -> FontId {
        let t = self.type_role(r);
        FontId::new(t.size.max(TEXT_FLOOR), t.font.family(self.fonts_ready))
    }

    pub fn section_font(&self) -> FontId {
        let t = &self.types.section;
        FontId::new(t.size.max(TEXT_FLOOR), t.font.family(self.fonts_ready))
    }

    /// Spacing step by index into the scale (clamped).
    pub fn sp(&self, i: usize) -> f32 {
        self.space[i.min(self.space.len() - 1)]
    }

    pub fn radius(&self, r: Rad) -> f32 {
        match r {
            Rad::Sm => self.radii.sm,
            Rad::Md => self.radii.md,
            Rad::Lg => self.radii.lg,
        }
    }

    pub fn shape_radius(&self, s: Shape, height: f32) -> f32 {
        match s {
            Shape::Rounded => self.radii.sm + 1.0,
            Shape::Pill => height / 2.0,
            Shape::Square => self.radii.sm.min(2.0),
        }
    }

    /// Duration in seconds for egui's animation helpers.
    pub fn secs(ms: f32) -> f32 {
        ms / 1000.0
    }

    pub fn section_of(&self, kind: &str) -> &Section {
        match kind {
            "osc.va" | "osc.wt" | "noise" => &self.sections.osc,
            "filter.svf" | "filter.ladder" => &self.sections.filter,
            "env.adsr" | "lfo" | "macro" => &self.sections.modulation,
            "vca" | "mixer" | "gain" | "out" => &self.sections.amp,
            "midi.in" | "clock" | "seq" | "cues" | "clock.div" => &self.sections.timing,
            _ => &self.sections.fx,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rad {
    Sm,
    Md,
    Lg,
}

/// Linear blend in gamma space.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    a.lerp_to_gamma(b, t.clamp(0.0, 1.0))
}

pub fn alpha(c: Color32, a: u8) -> Color32 {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

fn rel_lum(c: Color32) -> f32 {
    let f = |v: u8| {
        let v = f32::from(v) / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
}

/// WCAG contrast ratio.
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (x, y) = (rel_lum(a), rel_lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

impl Style {
    /// Text/background pairs a theme must keep legible, with the minimum ratio for each.
    pub fn contrast_pairs(&self) -> Vec<(&'static str, Color32, Color32, f32)> {
        let r = &self.roles;
        let mut v = vec![
            ("text on surface", r.text, r.surface, 4.5),
            ("text on bg", r.text, r.bg, 4.5),
            ("text on raised", r.text, r.raised, 4.5),
            ("text2 on surface", r.text2, r.surface, 4.5),
            ("text2 on inset", r.text2, r.inset, 4.5),
            ("on_accent on accent", r.on_accent, r.accent, 4.5),
            ("text on accent_soft", r.text, r.accent_soft, 4.5),
            ("disp_text on disp_bg", r.disp_text, r.disp_bg, 4.5),
        ];
        for (n, s) in [
            ("osc", &self.sections.osc),
            ("filter", &self.sections.filter),
            ("modulation", &self.sections.modulation),
            ("amp", &self.sections.amp),
            ("timing", &self.sections.timing),
            ("fx", &self.sections.fx),
        ] {
            v.push((leak(format!("{n} ink on base")), s.ink, s.base, 4.5));
            v.push((leak(format!("{n} ink2 on base")), s.ink2, s.base, 4.5));
        }
        v
    }
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_a_round_trips_through_toml() {
        for dark in [false, true] {
            let a = builtin_a(dark);
            let text = toml::to_string(&a).unwrap();
            let back: Style = toml::from_str(&text).unwrap();
            assert_eq!(Style { metrics: Metrics::default(), fonts_ready: false, ..back }, a);
        }
    }

    #[test]
    fn builtin_a_is_legible_and_above_the_floor() {
        for dark in [false, true] {
            let a = builtin_a(dark);
            for (n, fg, bg, min) in a.contrast_pairs() {
                assert!(contrast(fg, bg) >= min, "{} {n}: {:.2}", a.id, contrast(fg, bg));
            }
            for r in [Role::Display, Role::Title, Role::H3, Role::Body, Role::Label, Role::Value, Role::Caption] {
                assert!(a.font(r).size >= TEXT_FLOOR);
            }
            assert!(a.section_font().size >= TEXT_FLOOR);
        }
    }

    #[test]
    fn every_role_has_a_name() {
        assert_eq!(Roles::NAMES.len(), 29);
    }
}
