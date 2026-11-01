//! The three-layer theme model (PLAN §3, §6): an app theme (`graphite` dark, `paper` light), a
//! face style (`line_solid`, `outline`) and an accent, composed into one [`Style`]. Layout never
//! reads any of it. Satin and Lacquer stay whole styles (`builtin_a`, `builtin_b`).

use super::{
    alpha, builtin_a, contrast, mix, CompactText, DisplayFrame, DisplayTrace, FaceHardware,
    FaceMaterial, FaceTitle, FamilyMarker, Font, JackRecipe, KnobBody, KnobScale, KnobValue,
    OutPlate, Roles, Section, Sections, SelectorRecipe, Style, TrayRecipe,
};
use egui::Color32;

pub const LINE_SOLID_ID: &str = "kabl.line_solid";
pub const OUTLINE_ID: &str = "kabl.outline";

fn h(s: &str) -> Color32 {
    Color32::from_hex(s).expect("valid colour")
}

/// Face styles a theme file may name with `face_style` (ui_api 2).
pub const FACE_STYLES: &[&str] = &["line_solid", "outline"];

/// An accent: a named preset or a custom colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Accent {
    Amber,
    Coral,
    Cyan,
    Custom(Color32),
}

impl Accent {
    /// `amber`, `coral`, `cyan` or `#rrggbb`.
    pub fn parse(s: &str) -> Option<Accent> {
        match s {
            "amber" => Some(Accent::Amber),
            "coral" => Some(Accent::Coral),
            "cyan" => Some(Accent::Cyan),
            _ => Color32::from_hex(s).ok().map(Accent::Custom),
        }
    }

    fn colour(self) -> Color32 {
        match self {
            Accent::Amber => h("#F2A33A"),
            Accent::Coral => h("#FF6B3D"),
            Accent::Cyan => h("#33C1D8"),
            Accent::Custom(c) => c,
        }
    }

    /// The accent on `well`: mixed toward black until non-text marks (value arcs, traces)
    /// reach 3:1 there. Dark app themes keep the preset as is.
    fn on(self, dark: bool, well: Color32) -> Color32 {
        let c = self.colour();
        if dark {
            return c;
        }
        let mut t = 0.0;
        while contrast(mix(c, Color32::BLACK, t), well) < 3.0 && t < 0.9 {
            t += 0.02;
        }
        mix(c, Color32::BLACK, t)
    }
}

/// Family title colour: the hue mixed toward white (dark) or black (light) until it reaches
/// 4.5:1 on the face. PLAN §6 starts at 15 % / 35 %; paper amp and fx end at 45 % and 40 %.
fn title_ink(hue: &str, dark: bool, face: Color32) -> Color32 {
    let to = if dark { Color32::WHITE } else { Color32::BLACK };
    let mut t = if dark { 0.15 } else { 0.35 };
    loop {
        let c = mix(h(hue), to, t);
        if contrast(c, face) >= 4.5 || t >= 0.95 {
            return c;
        }
        t += 0.05;
    }
}

fn roles(dark: bool, accent: Accent) -> Roles {
    let (bg, surface, raised, inset, line, line2) = if dark {
        (
            "#111214", "#1A1B1E", "#232529", "#0F1012", "#2A2C31", "#393C42",
        )
    } else {
        (
            "#ECEAE5", "#FBFAF7", "#FFFFFF", "#E4E1DA", "#DCD8D0", "#C8C4BB",
        )
    };
    let (text, text2, text3) = if dark {
        ("#ECEAE6", "#A29F99", "#6F6D68")
    } else {
        ("#1A1917", "#5F5C56", "#8E8A83")
    };
    let (audio, cv, gate, pitch) = if dark {
        ("#F59E3B", "#38C6B2", "#A68BFA", "#5BA8F7")
    } else {
        ("#D9781C", "#12958A", "#7651D6", "#2F7FD6")
    };
    let base = builtin_a(dark).roles;
    let rack = if dark { "#0C0D0E" } else { "#DCD9D2" };
    // Cables are drawn on the rack: on paper the signal colours are darkened until they reach
    // 3:1 there (PLAN §6 hues #D9781C #12958A #7651D6 #2F7FD6 start below it).
    let sig = |c: &str| {
        let c = h(c);
        let mut t = 0.0;
        while !dark && contrast(mix(c, Color32::BLACK, t), h(rack)) < 3.0 && t < 0.9 {
            t += 0.02;
        }
        mix(c, Color32::BLACK, t)
    };
    let acc = accent.on(dark, h(inset));
    let on_accent = if contrast(h("#1C1206"), acc) >= contrast(Color32::WHITE, acc) {
        h("#1C1206")
    } else {
        Color32::WHITE
    };
    Roles {
        bg: h(bg),
        surface: h(surface),
        raised: h(raised),
        inset: h(inset),
        line: h(line),
        line2: h(line2),
        text: h(text),
        text2: h(text2),
        text3: h(text3),
        accent: acc,
        on_accent,
        accent_soft: mix(acc, h(bg), 0.8),
        focus: acc,
        audio: sig(audio),
        cv: sig(cv),
        gate: sig(gate),
        pitch: sig(pitch),
        rack: h(rack),
        rail: h(if dark { "#1B1C1F" } else { "#C9C5BC" }),
        hole: h(if dark { "#050506" } else { "#24232A" }),
        disp_bg: h(inset),
        disp_grid: alpha(h(text), 20),
        disp_trace: acc,
        disp_text: h(text2),
        // Dark filled caps in both app themes.
        knob: h("#1D1F23"),
        knob_hi: h("#3C3F45"),
        knob_ink: h("#F2F0EB"),
        ..base
    }
}

fn sections(dark: bool, face: Color32, text2: Color32) -> Sections {
    let s = |hue: &str| Section {
        base: face,
        ink: title_ink(hue, dark, face),
        ink2: text2,
    };
    Sections {
        osc: s("#E07A5F"),
        filter: s("#7FB59A"),
        modulation: s("#9D8DF1"),
        timing: s("#5FA8D3"),
        amp: s("#E3BE5C"),
        fx: s("#B9B2A6"),
    }
}

/// `line_solid`: line-art panels and wells, solid hardware (filled caps, hex jacks, accent
/// selectors), glow on dark.
fn line_solid(dark: bool, accent: Accent) -> Style {
    let mut s = builtin_a(dark);
    s.roles = roles(dark, accent);
    s.sections = sections(dark, s.roles.surface, s.roles.text2);
    s.id = LINE_SOLID_ID.into();
    s.name = "Signal and Drafting solid".into();
    s.types.h3.font = Font::SansCondSemibold;
    s.types.h3.size = 14.0;
    s.types.label.size = 11.0;
    s.types.value.font = Font::Mono;
    s.types.value.size = 11.0;
    s.types.caption.font = Font::Mono;
    s.types.section.font = Font::SansCondSemibold;
    s.types.section.tracking = 0.8;
    let f = &mut s.slots.face;
    f.material = FaceMaterial::Line;
    f.gradient = 0.0;
    f.gloss = 0.0;
    f.grain = 0.0;
    f.hardware = FaceHardware::Screws;
    f.title = FaceTitle::CondCaps;
    f.out_plate = OutPlate::Outline;
    f.shadow = 0;
    f.family = FamilyMarker::Title;
    f.compact = CompactText::Hold;
    f.tray = TrayRecipe::Outline;
    let k = &mut s.slots.knob;
    k.body = KnobBody::Solid;
    k.scale = KnobScale::TrackArc;
    k.value = KnobValue::Arc;
    k.glow = if dark { 0.4 } else { 0.0 };
    s.slots.selector = SelectorRecipe::AccentPill;
    s.slots.jack = JackRecipe::Hex;
    s.slots.display.frame = DisplayFrame::Paper;
    s.slots.display.trace = if dark {
        DisplayTrace::SoftGlow
    } else {
        DisplayTrace::Crisp
    };
    s
}

/// `outline`: the same line art with outlined hardware (flat discs, ring jacks, outlined
/// selectors) and no glow.
fn outline(dark: bool, accent: Accent) -> Style {
    let mut s = line_solid(dark, accent);
    s.id = OUTLINE_ID.into();
    s.name = "Outline".into();
    s.slots.knob.body = KnobBody::Disc;
    s.slots.knob.value = KnobValue::Line;
    s.slots.knob.glow = 0.0;
    s.slots.jack = JackRecipe::Donut;
    s.slots.selector = SelectorRecipe::Outlined;
    s.slots.display.trace = DisplayTrace::Crisp;
    s
}

/// The face style `name` (see [`FACE_STYLES`]) in app theme `dark` with `accent`.
pub fn compose(face_style: &str, dark: bool, accent: Accent) -> Option<Style> {
    match face_style {
        "line_solid" => Some(line_solid(dark, accent)),
        "outline" => Some(outline(dark, accent)),
        _ => None,
    }
}

/// `style` with its accent replaced (an accent preset or custom colour on any built-in).
pub fn with_accent(mut style: Style, accent: Accent) -> Style {
    let r = &mut style.roles;
    let acc = accent.on(style.dark, r.inset);
    r.accent = acc;
    r.on_accent = if contrast(h("#1C1206"), acc) >= contrast(Color32::WHITE, acc) {
        h("#1C1206")
    } else {
        Color32::WHITE
    };
    r.accent_soft = mix(acc, r.bg, 0.8);
    r.focus = acc;
    r.disp_trace = acc;
    style
}

/// `line_solid` with the default amber accent: the redesign's two default looks.
pub fn line_solid_default(dark: bool) -> Style {
    line_solid(dark, Accent::Amber)
}

/// `outline` with the default amber accent.
pub fn outline_default(dark: bool) -> Style {
    outline(dark, Accent::Amber)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_amber_and_hue_mixes_are_the_approved_tokens() {
        let paper = line_solid_default(false);
        assert_eq!(paper.roles.accent, h("#A97229"));
        let dark = line_solid_default(true);
        assert_eq!(dark.roles.accent, h("#F2A33A"));
        // Approved mixes on paper: amp 45 %, fx 40 %, the rest 35 %.
        let face = paper.roles.surface;
        for (sec, hue, t) in [
            (paper.sections.amp.ink, "#E3BE5C", 0.45),
            (paper.sections.fx.ink, "#B9B2A6", 0.40),
            (paper.sections.osc.ink, "#E07A5F", 0.35),
        ] {
            let want = mix(h(hue), Color32::BLACK, t);
            assert!(
                (i32::from(sec.r()) - i32::from(want.r())).abs() <= 1,
                "{hue} {sec:?} {want:?}"
            );
        }
        assert!(contrast(paper.sections.amp.ink, face) >= 4.5);
    }

    #[test]
    fn every_combo_passes_the_text_pairs() {
        for dark in [true, false] {
            for accent in [Accent::Amber, Accent::Coral, Accent::Cyan] {
                for fs in FACE_STYLES {
                    let s = compose(fs, dark, accent).unwrap();
                    for (what, a, b, min) in s.contrast_pairs() {
                        let c = contrast(a, b);
                        assert!(c >= min, "{fs} dark={dark} {accent:?} {what}: {c:.2}");
                    }
                    for (what, a, b, min) in s.mark_pairs() {
                        let c = contrast(a, b);
                        assert!(c >= min, "{fs} dark={dark} {accent:?} {what}: {c:.2}");
                    }
                }
            }
        }
    }
}
