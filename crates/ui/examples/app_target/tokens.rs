//! Design tokens for the three whole-application directions. The token sheets under
//! `docs/design/app-target/` are generated from these tables (`app_target tokens`).
use egui::Color32;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dir {
    A,
    B,
    C,
}

impl Dir {
    pub const ALL: [Dir; 3] = [Dir::A, Dir::B, Dir::C];
    pub fn slug(self) -> &'static str {
        match self {
            Dir::A => "a-satin",
            Dir::B => "b-lacquer",
            Dir::C => "c-silkscreen",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Dir::A => "A · Satin Studio",
            Dir::B => "B · Lacquer & Light",
            Dir::C => "C · Silkscreen",
        }
    }
    pub fn parse(s: &str) -> Option<Dir> {
        Some(match s {
            "a" | "A" => Dir::A,
            "b" | "B" => Dir::B,
            "c" | "C" => Dir::C,
            _ => return None,
        })
    }
}

fn h(s: &str) -> Color32 {
    Color32::from_hex(s).unwrap_or_else(|_| panic!("bad colour {s}"))
}

/// Module section colour families.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sect {
    Osc,
    Filter,
    Mod,
    Amp,
    Timing,
    Fx,
}

impl Sect {
    pub const ALL: [Sect; 6] = [Sect::Osc, Sect::Filter, Sect::Mod, Sect::Amp, Sect::Timing, Sect::Fx];
    pub fn name(self) -> &'static str {
        match self {
            Sect::Osc => "oscillator / noise",
            Sect::Filter => "filter",
            Sect::Mod => "envelope / LFO",
            Sect::Amp => "VCA / mix / output",
            Sect::Timing => "clock / sequencer / MIDI",
            Sect::Fx => "effects / other",
        }
    }
    pub fn of(kind: &str) -> Sect {
        match kind {
            "osc.va" | "osc.wt" | "noise" => Sect::Osc,
            "filter.svf" | "filter.ladder" => Sect::Filter,
            "env.adsr" | "lfo" | "macro" => Sect::Mod,
            "vca" | "mixer" | "gain" | "out" => Sect::Amp,
            "midi.in" | "clock" | "seq" | "cues" | "clock.div" => Sect::Timing,
            _ => Sect::Fx,
        }
    }
    fn ix(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy)]
pub struct Face {
    pub base: Color32,
    pub ink: Color32,
    pub ink2: Color32,
}

#[derive(Clone, Copy)]
pub struct Shadow {
    pub dy: f32,
    pub blur: f32,
    pub alpha: u8,
}

#[derive(Clone, Copy)]
pub struct Motion {
    pub hover_ms: u32,
    pub press_ms: u32,
    pub drawer_ms: u32,
    pub view_ms: u32,
    pub glide_ms: u32,
    pub pulse_ms: u32,
}

/// Type scale in logical px: (name, size, family, line height).
pub struct TypeStep {
    pub name: &'static str,
    pub size: f32,
    pub fam: &'static str,
    pub role: &'static str,
}

pub struct Tok {
    pub dir: Dir,
    pub dark: bool,
    pub bg: Color32,
    pub surface: Color32,
    pub raised: Color32,
    pub inset: Color32,
    pub line: Color32,
    pub line2: Color32,
    pub text: Color32,
    pub text2: Color32,
    pub text3: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    pub accent_soft: Color32,
    pub focus: Color32,
    pub audio: Color32,
    pub cv: Color32,
    pub gate: Color32,
    pub good: Color32,
    pub warn: Color32,
    pub bad: Color32,
    pub rack: Color32,
    pub rail: Color32,
    pub hole: Color32,
    pub disp_bg: Color32,
    pub disp_grid: Color32,
    pub disp_trace: Color32,
    pub disp_text: Color32,
    pub knob: Color32,
    pub knob_hi: Color32,
    pub knob_ink: Color32,
    pub faces: [Face; 6],
    pub r_sm: f32,
    pub r_md: f32,
    pub r_lg: f32,
    pub sh: [Shadow; 3],
    pub motion: Motion,
}

pub const SPACING: [f32; 8] = [2.0, 4.0, 8.0, 12.0, 16.0, 24.0, 32.0, 48.0];

impl Tok {
    pub fn face(&self, kind: &str) -> Face {
        self.faces[Sect::of(kind).ix()]
    }
    pub fn sect(&self, s: Sect) -> Face {
        self.faces[s.ix()]
    }
    pub fn signal(&self, ty: kabl_modules::PortType) -> Color32 {
        use kabl_modules::PortType::*;
        match ty {
            Audio => self.audio,
            Cv | UnipolarCv | Pitch => self.cv,
            Gate => self.gate,
        }
    }
    pub fn type_scale(&self) -> Vec<TypeStep> {
        let s = |name, size, fam, role| TypeStep { name, size, fam, role };
        match self.dir {
            Dir::A => vec![
                s("display", 28.0, "sans-semi", "Perform: tempo, bar counter"),
                s("title", 20.0, "sans-semi", "Panel heading, patch name"),
                s("h3", 15.0, "sans-semi", "Card heading, module name"),
                s("body", 13.0, "sans", "Lists, inputs, help"),
                s("label", 12.0, "sans-med", "Control labels, tabs"),
                s("value", 12.0, "mono-med", "Numeric readouts"),
                s("caption", 11.0, "sans", "Secondary text; floor for any text"),
            ],
            Dir::B => vec![
                s("display", 30.0, "mono-med", "Perform: tempo, bar counter"),
                s("title", 20.0, "sans-semi", "Preset name, panel heading"),
                s("h3", 15.0, "sans-semi", "Card heading, module name"),
                s("body", 13.0, "sans", "Lists, inputs, help"),
                s("label", 11.5, "sans-semi", "Control labels (caps, +0.6 px tracking)"),
                s("value", 12.0, "mono-med", "Numeric readouts"),
                s("caption", 11.0, "sans", "Secondary text; floor for any text"),
            ],
            Dir::C => vec![
                s("display", 32.0, "cond-semi", "Perform: tempo, bar counter"),
                s("title", 20.0, "cond-semi", "Panel heading, patch name (caps)"),
                s("h3", 15.0, "cond-semi", "Module name (caps)"),
                s("body", 13.0, "sans", "Lists, inputs, help"),
                s("label", 12.0, "cond-semi", "Control labels (caps, +0.5 px tracking)"),
                s("value", 12.0, "mono-med", "Numeric readouts"),
                s("caption", 11.0, "cond-med", "Secondary text; floor for any text"),
            ],
        }
    }
}

pub fn tok(dir: Dir, dark: bool) -> Tok {
    let f = |b: &str, i: &str, i2: &str| Face { base: h(b), ink: h(i), ink2: h(i2) };
    let sh = |dy, blur, alpha| Shadow { dy, blur, alpha };
    match (dir, dark) {
        (Dir::A, false) => Tok {
            dir,
            dark,
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
            faces: [
                f("#d79a82", "#2a1c14", "#3e291d"),
                f("#aebf9f", "#1f2a1b", "#3d4c37"),
                f("#bdb3d0", "#231d30", "#463d58"),
                f("#cabc92", "#2c2410", "#3f3620"),
                f("#a9bec1", "#16262a", "#35494d"),
                f("#d9cfbf", "#2a2318", "#4d4535"),
            ],
            r_sm: 4.0,
            r_md: 8.0,
            r_lg: 12.0,
            sh: [sh(1.0, 3.0, 28), sh(4.0, 12.0, 38), sh(12.0, 32.0, 56)],
            motion: Motion { hover_ms: 90, press_ms: 60, drawer_ms: 220, view_ms: 180, glide_ms: 120, pulse_ms: 1200 },
        },
        (Dir::A, true) => Tok {
            dir,
            dark,
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
            faces: [
                f("#85594a", "#fff4e6", "#f8ece0"),
                f("#53664d", "#f4fbee", "#dbe7d3"),
                f("#62537a", "#f6f1ff", "#e0d8ee"),
                f("#6e5f40", "#fff8e4", "#ebe0c4"),
                f("#3f5a60", "#effbfd", "#d1e4e8"),
                f("#5d564c", "#f8f1e6", "#e0d8ca"),
            ],
            r_sm: 4.0,
            r_md: 8.0,
            r_lg: 12.0,
            sh: [sh(1.0, 3.0, 90), sh(4.0, 12.0, 110), sh(12.0, 32.0, 150)],
            motion: Motion { hover_ms: 90, press_ms: 60, drawer_ms: 220, view_ms: 180, glide_ms: 120, pulse_ms: 1200 },
        },
        (Dir::B, true) => Tok {
            dir,
            dark,
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
            faces: [
                f("#8e4338", "#fff6ef", "#f1ddd2"),
                f("#35705a", "#f2fff8", "#d3eadf"),
                f("#5b4ca3", "#f6f3ff", "#ddd6f4"),
                f("#82672b", "#fff9e8", "#fcf3dc"),
                f("#2b6377", "#effcff", "#cde7ef"),
                f("#5d566b", "#f7f4fb", "#ded8e8"),
            ],
            r_sm: 5.0,
            r_md: 10.0,
            r_lg: 16.0,
            sh: [sh(1.0, 4.0, 120), sh(6.0, 18.0, 150), sh(16.0, 40.0, 190)],
            motion: Motion { hover_ms: 120, press_ms: 80, drawer_ms: 260, view_ms: 220, glide_ms: 160, pulse_ms: 900 },
        },
        (Dir::B, false) => Tok {
            dir,
            dark,
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
            faces: [
                f("#f0a791", "#2a110c", "#4d2a22"),
                f("#aad3b8", "#0f2619", "#2c4a3a"),
                f("#c6baf0", "#1a1233", "#3d3262"),
                f("#ebd28e", "#2b2108", "#524416"),
                f("#a1d0e2", "#0c2530", "#26495a"),
                f("#d9d2e4", "#1e1829", "#413a54"),
            ],
            r_sm: 5.0,
            r_md: 10.0,
            r_lg: 16.0,
            sh: [sh(1.0, 3.0, 26), sh(6.0, 16.0, 36), sh(16.0, 36.0, 56)],
            motion: Motion { hover_ms: 120, press_ms: 80, drawer_ms: 260, view_ms: 220, glide_ms: 160, pulse_ms: 900 },
        },
        (Dir::C, false) => Tok {
            dir,
            dark,
            bg: h("#efeee9"),
            surface: h("#ffffff"),
            raised: h("#ffffff"),
            inset: h("#e6e4de"),
            line: h("#d2cfc7"),
            line2: h("#14130f"),
            text: h("#15130f"),
            text2: h("#55514a"),
            text3: h("#8a867d"),
            accent: h("#d23a12"),
            on_accent: h("#ffffff"),
            accent_soft: h("#fbdcd1"),
            focus: h("#1d4ed8"),
            audio: h("#e89a00"),
            cv: h("#00968c"),
            gate: h("#6c3fe6"),
            good: h("#1b8a3d"),
            warn: h("#a65f00"),
            bad: h("#c4271c"),
            rack: h("#d9d7d0"),
            rail: h("#bdbab1"),
            hole: h("#1a1915"),
            disp_bg: h("#fbfaf6"),
            disp_grid: h("#dedbd2"),
            disp_trace: h("#14130f"),
            disp_text: h("#14130f"),
            knob: h("#17150f"),
            knob_hi: h("#17150f"),
            knob_ink: h("#ffffff"),
            faces: [
                f("#f2a48b", "#14110d", "#3a352d"),
                f("#b5d3a8", "#14110d", "#3a352d"),
                f("#cbbbee", "#14110d", "#3a352d"),
                f("#f1d57c", "#14110d", "#3a352d"),
                f("#a9d0e1", "#14110d", "#3a352d"),
                f("#dedacd", "#14110d", "#3a352d"),
            ],
            r_sm: 2.0,
            r_md: 4.0,
            r_lg: 6.0,
            sh: [sh(0.0, 0.0, 0), sh(2.0, 6.0, 22), sh(8.0, 20.0, 40)],
            motion: Motion { hover_ms: 60, press_ms: 40, drawer_ms: 160, view_ms: 120, glide_ms: 90, pulse_ms: 800 },
        },
        (Dir::C, true) => Tok {
            dir,
            dark,
            bg: h("#111110"),
            surface: h("#1a1a18"),
            raised: h("#222220"),
            inset: h("#0b0b0a"),
            line: h("#34332f"),
            line2: h("#efece4"),
            text: h("#f3f1ea"),
            text2: h("#b0aca2"),
            text3: h("#78756c"),
            accent: h("#ff6b3d"),
            on_accent: h("#160803"),
            accent_soft: h("#4a2214"),
            focus: h("#7ea2ff"),
            audio: h("#ffb21a"),
            cv: h("#22d3c5"),
            gate: h("#a98bff"),
            good: h("#5fd388"),
            warn: h("#ffc04d"),
            bad: h("#ff7468"),
            rack: h("#080808"),
            rail: h("#2f2e2a"),
            hole: h("#000000"),
            disp_bg: h("#0a0a09"),
            disp_grid: h("#262622"),
            disp_trace: h("#f3f1ea"),
            disp_text: h("#f3f1ea"),
            knob: h("#0e0e0c"),
            knob_hi: h("#f3f1ea"),
            knob_ink: h("#f3f1ea"),
            faces: [
                f("#e0714f", "#0e0c09", "#2b2218"),
                f("#78b27b", "#0b0f0b", "#1f2c20"),
                f("#9a82e0", "#0c0a14", "#271f40"),
                f("#d5ac3a", "#100c02", "#32280b"),
                f("#4a9fc2", "#06101a", "#12303f"),
                f("#9a948a", "#0e0d0a", "#2b2822"),
            ],
            r_sm: 2.0,
            r_md: 4.0,
            r_lg: 6.0,
            sh: [sh(0.0, 0.0, 0), sh(2.0, 6.0, 100), sh(8.0, 20.0, 160)],
            motion: Motion { hover_ms: 60, press_ms: 40, drawer_ms: 160, view_ms: 120, glide_ms: 90, pulse_ms: 800 },
        },
    }
}
