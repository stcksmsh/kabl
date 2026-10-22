//! Built-in theme A, "Satin Studio" (docs/design/app-target/tokens-a-satin.md).

use super::*;

pub const BUILTIN_A_ID: &str = "kabl.satin";

fn h(s: &str) -> Color32 {
    Color32::from_hex(s).expect("built-in colour")
}

fn sect(base: &str, ink: &str, ink2: &str) -> Section {
    Section {
        base: h(base),
        ink: h(ink),
        ink2: h(ink2),
    }
}

fn role(font: Font, size: f32) -> TypeRole {
    TypeRole {
        font,
        size,
        caps: false,
        tracking: 0.0,
    }
}

pub fn builtin_a(dark: bool) -> Style {
    let (roles, sections, shadows) = if dark { dark_parts() } else { light_parts() };
    Style {
        fonts_ready: false,
        id: BUILTIN_A_ID.into(),
        name: "Satin Studio".into(),
        dark,
        roles,
        sections,
        types: TypeScale {
            display: role(Font::SansSemibold, 28.0),
            title: role(Font::SansSemibold, 20.0),
            h3: role(Font::SansSemibold, 15.0),
            body: role(Font::Sans, 13.0),
            label: role(Font::SansMedium, 12.0),
            value: role(Font::MonoMedium, 12.0),
            caption: role(Font::Sans, 11.0),
            section: TypeRole {
                font: Font::SansSemibold,
                size: 11.0,
                caps: true,
                tracking: 1.0,
            },
        },
        space: [2.0, 4.0, 8.0, 12.0, 16.0, 24.0, 32.0, 48.0],
        radii: Radii {
            sm: 4.0,
            md: 8.0,
            lg: 12.0,
        },
        shadows,
        motion: Motion {
            hover: 90.0,
            press: 60.0,
            drawer: 220.0,
            view: 180.0,
            glide: 120.0,
            pulse: 1200.0,
        },
        slots: Slots {
            face: FaceSlots {
                material: FaceMaterial::Satin,
                gradient: if dark { 0.06 } else { 0.10 },
                gloss: 0.0,
                grain: 0.02,
                hardware: FaceHardware::Nuts,
                title: FaceTitle::Centered,
                out_plate: OutPlate::Tint,
                shadow: 1,
            },
            rack: RackSurface::Plain,
            knob: KnobSlots {
                body: KnobBody::Cap,
                scale: KnobScale::Ticks,
                value: KnobValue::Line,
                modulation: KnobMod::ArcDot,
                glow: 0.0,
            },
            selector: SelectorRecipe::InsetPill,
            jack: JackRecipe::Nut,
            cable: CableSlots {
                recipe: CableRecipe::Rope,
                width: 4.6,
                rest_width: 3.0,
                rest_alpha: 0.85,
                sag: 46.0,
                glow: 0.0,
                dimmed: 0.22,
            },
            plug: PlugRecipe::Cap,
            display: DisplaySlots {
                frame: DisplayFrame::Glass,
                trace: DisplayTrace::SoftGlow,
                glow: 0.45,
                fill: 0.3,
            },
            controls: ControlSlots {
                button: Shape::Rounded,
                chip: Shape::Rounded,
                field: Shape::Rounded,
                gradient: 0.0,
                outline: 1.0,
                focus_ring: 2.0,
                card_shadow: 1,
                popover_shadow: 2,
                row: BrowserRows::Cards,
            },
            chrome: ChromeSlots {
                view_switch: ViewSwitch::Segmented,
                navigator: Navigator::Left,
            },
            icons: Icons::Stroke,
        },
        metrics: Metrics::default(),
    }
}

fn light_parts() -> (Roles, Sections, [Shadow; 3]) {
    (
        Roles {
            bg: h("#e7e2d6"),
            surface: h("#f3efe5"),
            raised: h("#fbf9f3"),
            inset: h("#dcd5c5"),
            line: h("#cbc3b0"),
            line2: h("#aaa28f"),
            text: h("#2a2621"),
            text2: h("#5a5347"),
            text3: h("#8a8272"),
            accent: h("#b8501f"),
            on_accent: h("#fff8ef"),
            accent_soft: h("#f0d9c9"),
            focus: h("#2a6ae0"),
            audio: h("#e0962b"),
            cv: h("#1f9d90"),
            gate: h("#8250d0"),
            good: h("#2f8a4a"),
            warn: h("#b0661a"),
            bad: h("#b8322a"),
            rack: h("#d6cfbf"),
            rail: h("#c3bcab"),
            hole: h("#2a2826"),
            disp_bg: h("#27241f"),
            disp_grid: h("#3d382f"),
            disp_trace: h("#f1b25a"),
            disp_text: h("#efe6d2"),
            knob: h("#2c2a27"),
            knob_hi: h("#5d5851"),
            knob_ink: h("#f4efe6"),
        },
        Sections {
            osc: sect("#d79a82", "#2a1c14", "#3e291d"),
            filter: sect("#aebf9f", "#1f2a1b", "#3d4c37"),
            modulation: sect("#bdb3d0", "#231d30", "#463d58"),
            amp: sect("#cabc92", "#2c2410", "#3f3620"),
            timing: sect("#a9bec1", "#16262a", "#35494d"),
            fx: sect("#d9cfbf", "#2a2318", "#4d4535"),
        },
        [
            Shadow {
                dy: 1.0,
                blur: 3.0,
                alpha: 28,
            },
            Shadow {
                dy: 4.0,
                blur: 12.0,
                alpha: 38,
            },
            Shadow {
                dy: 12.0,
                blur: 32.0,
                alpha: 56,
            },
        ],
    )
}

fn dark_parts() -> (Roles, Sections, [Shadow; 3]) {
    (
        Roles {
            bg: h("#161412"),
            surface: h("#1f1c19"),
            raised: h("#292520"),
            inset: h("#110f0d"),
            line: h("#3a352e"),
            line2: h("#5b5448"),
            text: h("#efe8da"),
            text2: h("#b9af9c"),
            text3: h("#7f7769"),
            accent: h("#f0a04b"),
            on_accent: h("#26180a"),
            accent_soft: h("#4a3620"),
            focus: h("#6aa4ff"),
            audio: h("#f0a640"),
            cv: h("#35c2b1"),
            gate: h("#a687ee"),
            good: h("#6cc788"),
            warn: h("#f1b75a"),
            bad: h("#ef7a6e"),
            rack: h("#0f0e0d"),
            rail: h("#3f3b35"),
            hole: h("#050504"),
            disp_bg: h("#0f0d0b"),
            disp_grid: h("#2a2620"),
            disp_trace: h("#f4b862"),
            disp_text: h("#f1e3c6"),
            knob: h("#201e1b"),
            knob_hi: h("#6b665d"),
            knob_ink: h("#f4efe6"),
        },
        Sections {
            osc: sect("#85594a", "#fff4e6", "#f8ece0"),
            filter: sect("#53664d", "#f4fbee", "#dbe7d3"),
            modulation: sect("#62537a", "#f6f1ff", "#e0d8ee"),
            amp: sect("#6e5f40", "#fff8e4", "#ebe0c4"),
            timing: sect("#3f5a60", "#effbfd", "#d1e4e8"),
            fx: sect("#5d564c", "#f8f1e6", "#e0d8ca"),
        },
        [
            Shadow {
                dy: 1.0,
                blur: 3.0,
                alpha: 90,
            },
            Shadow {
                dy: 4.0,
                blur: 12.0,
                alpha: 110,
            },
            Shadow {
                dy: 12.0,
                blur: 32.0,
                alpha: 150,
            },
        ],
    )
}

/// Built-in theme B, "Lacquer and Light" (docs/design/app-target/tokens-b-lacquer.md).
pub const BUILTIN_B_ID: &str = "kabl.lacquer";

pub fn builtin_b(dark: bool) -> Style {
    let (roles, sections, shadows) = if dark {
        b_dark_parts()
    } else {
        b_light_parts()
    };
    let caps = |font, size, tracking| TypeRole {
        font,
        size,
        caps: true,
        tracking,
    };
    Style {
        fonts_ready: false,
        id: BUILTIN_B_ID.into(),
        name: "Lacquer and Light".into(),
        dark,
        roles,
        sections,
        types: TypeScale {
            display: role(Font::MonoMedium, 30.0),
            title: role(Font::SansSemibold, 20.0),
            h3: role(Font::SansSemibold, 15.0),
            body: role(Font::Sans, 13.0),
            label: caps(Font::SansSemibold, 11.5, 0.6),
            value: role(Font::MonoMedium, 12.0),
            caption: role(Font::Sans, 11.0),
            section: caps(Font::SansSemibold, 11.0, 1.0),
        },
        space: [2.0, 4.0, 8.0, 12.0, 16.0, 24.0, 32.0, 48.0],
        radii: Radii {
            sm: 5.0,
            md: 10.0,
            lg: 16.0,
        },
        shadows,
        motion: Motion {
            hover: 120.0,
            press: 80.0,
            drawer: 260.0,
            view: 220.0,
            glide: 160.0,
            pulse: 900.0,
        },
        slots: Slots {
            face: FaceSlots {
                material: FaceMaterial::Lacquer,
                gradient: 0.28,
                gloss: 0.24,
                grain: 0.0,
                hardware: FaceHardware::None,
                title: FaceTitle::LeftDot,
                out_plate: OutPlate::Dark,
                shadow: 1,
            },
            rack: RackSurface::Plain,
            knob: KnobSlots {
                body: KnobBody::Glass,
                scale: KnobScale::TrackArc,
                value: KnobValue::Dot,
                modulation: KnobMod::ArcGlow,
                glow: 0.6,
            },
            selector: SelectorRecipe::AccentPill,
            jack: JackRecipe::GlowRing,
            cable: CableSlots {
                recipe: CableRecipe::Glow,
                width: 4.2,
                rest_width: 2.8,
                rest_alpha: 0.85,
                sag: 46.0,
                glow: 0.5,
                dimmed: 0.22,
            },
            plug: PlugRecipe::Ring,
            display: DisplaySlots {
                frame: DisplayFrame::Neon,
                trace: DisplayTrace::StrongGlow,
                glow: 0.8,
                fill: 0.35,
            },
            controls: ControlSlots {
                button: Shape::Pill,
                chip: Shape::Pill,
                field: Shape::Rounded,
                gradient: 0.6,
                outline: 1.0,
                focus_ring: 2.0,
                card_shadow: 1,
                popover_shadow: 2,
                row: BrowserRows::Cards,
            },
            chrome: ChromeSlots {
                view_switch: ViewSwitch::Tabs,
                navigator: Navigator::Left,
            },
            icons: Icons::Stroke,
        },
        metrics: Metrics::default(),
    }
}

fn b_shadows(alpha: [u8; 3]) -> [Shadow; 3] {
    let s = |dy, blur, alpha| Shadow { dy, blur, alpha };
    [
        s(1.0, 3.0, alpha[0]),
        s(6.0, 16.0, alpha[1]),
        s(16.0, 36.0, alpha[2]),
    ]
}

fn b_light_parts() -> (Roles, Sections, [Shadow; 3]) {
    (
        Roles {
            bg: h("#e6e9f0"),
            surface: h("#f4f6fa"),
            raised: h("#ffffff"),
            inset: h("#dce1ea"),
            line: h("#c8cfdb"),
            line2: h("#9aa3b5"),
            text: h("#161922"),
            text2: h("#464e60"),
            text3: h("#7b8396"),
            accent: h("#3b4fe0"),
            on_accent: h("#ffffff"),
            accent_soft: h("#dbe0fb"),
            focus: h("#3b4fe0"),
            audio: h("#e08a14"),
            cv: h("#0f9d8a"),
            gate: h("#7b4fd8"),
            good: h("#1f8a4c"),
            warn: h("#a8620f"),
            bad: h("#c23030"),
            rack: h("#cfd4df"),
            rail: h("#b4bbcb"),
            hole: h("#2c303a"),
            disp_bg: h("#12141a"),
            disp_grid: h("#262a35"),
            disp_trace: h("#5ff0d6"),
            disp_text: h("#d6f3f3"),
            knob: h("#1c1f27"),
            knob_hi: h("#5b6274"),
            knob_ink: h("#f2f4fa"),
        },
        Sections {
            osc: sect("#f0a791", "#2a110c", "#4d2a22"),
            filter: sect("#aad3b8", "#0f2619", "#2c4a3a"),
            modulation: sect("#c6baf0", "#1a1233", "#3d3262"),
            amp: sect("#ebd28e", "#2b2108", "#524416"),
            timing: sect("#a1d0e2", "#0c2530", "#26495a"),
            fx: sect("#d9d2e4", "#1e1829", "#413a54"),
        },
        b_shadows([26, 36, 56]),
    )
}

fn b_dark_parts() -> (Roles, Sections, [Shadow; 3]) {
    (
        Roles {
            bg: h("#0d0e12"),
            surface: h("#15171d"),
            raised: h("#1e2129"),
            inset: h("#090a0d"),
            line: h("#292d38"),
            line2: h("#434958"),
            text: h("#eceef4"),
            text2: h("#a5abbb"),
            text3: h("#6c7388"),
            accent: h("#7f8bff"),
            on_accent: h("#0b0d1f"),
            accent_soft: h("#24285a"),
            focus: h("#9aa4ff"),
            audio: h("#ffb454"),
            cv: h("#3de0c8"),
            gate: h("#b690ff"),
            good: h("#5fe39a"),
            warn: h("#ffc666"),
            bad: h("#ff7a7a"),
            rack: h("#08090c"),
            rail: h("#2a2d36"),
            hole: h("#030305"),
            disp_bg: h("#07080b"),
            disp_grid: h("#1d212a"),
            disp_trace: h("#5ff0d6"),
            disp_text: h("#cfeff0"),
            knob: h("#12141a"),
            knob_hi: h("#4a5060"),
            knob_ink: h("#f2f4fa"),
        },
        Sections {
            osc: sect("#8e4338", "#fff6ef", "#f1ddd2"),
            filter: sect("#35705a", "#f2fff8", "#d3eadf"),
            modulation: sect("#5b4ca3", "#f6f3ff", "#ddd6f4"),
            amp: sect("#82672b", "#fff9e8", "#fcf3dc"),
            timing: sect("#2b6377", "#effcff", "#cde7ef"),
            fx: sect("#5d566b", "#f7f4fb", "#ded8e8"),
        },
        b_shadows([120, 150, 190]),
    )
}
