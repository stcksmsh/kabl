//! A-light / A-dark: the approved revision-2 palettes (`docs/design/revision-2/render2.py`,
//! the same values the `rev2_proto` example draws with), and the chrome/fonts that go with them.

use egui::Color32;
use kabl_modules::PortType;

pub struct Theme {
    pub dark: bool,
    pub chrome: Color32,
    pub ctext: Color32,
    pub rack: Color32,
    pub rail: Color32,
    pub rail_hi: Color32,
    pub hole: Color32,
    pub panel: Color32,
    pub panel_edge: Color32,
    pub ink: Color32,
    pub ink2: Color32,
    pub plate: Color32,
    pub plate_ink: Color32,
    pub knob: Color32,
    pub knob_hi: Color32,
    pub pointer: Color32,
    pub skirt: Color32,
    pub tick: Color32,
    pub nut: Color32,
    pub nut_edge: Color32,
    pub hole_c: Color32,
    pub display: Color32,
    pub display_ink: Color32,
    pub seg_bg: Color32,
    pub seg_on: Color32,
    pub seg_on_text: Color32,
    pub sel: Color32,
    pub warn: Color32,
    pub audio: Color32,
    pub cv: Color32,
    pub gate: Color32,
    pub btn: Color32,
}

fn hex(s: &str) -> Color32 {
    Color32::from_hex(s).expect("valid colour")
}

pub fn theme(dark: bool) -> Theme {
    if dark {
        Theme {
            dark,
            chrome: hex("#1d1c1a"),
            ctext: hex("#efe8da"),
            rack: hex("#0e0d0c"),
            rail: hex("#5f5b55"),
            rail_hi: hex("#8c877e"),
            hole: hex("#161513"),
            panel: hex("#2f2c28"),
            panel_edge: hex("#4d4943"),
            ink: hex("#efe8da"),
            ink2: hex("#b2a893"),
            plate: hex("#191816"),
            plate_ink: hex("#efe8da"),
            knob: hex("#8e8981"),
            knob_hi: hex("#ece7de"),
            pointer: hex("#1b1917"),
            skirt: hex("#1b1a18"),
            tick: hex("#8d8475"),
            nut: hex("#a29d94"),
            nut_edge: hex("#67635c"),
            hole_c: hex("#080807"),
            display: hex("#141311"),
            display_ink: hex("#f1cf94"),
            seg_bg: hex("#23211e"),
            seg_on: hex("#e9dcc0"),
            seg_on_text: hex("#1b1916"),
            sel: hex("#5a9bff"),
            warn: hex("#f1b75a"),
            audio: hex("#f0a640"),
            cv: hex("#35c2b1"),
            gate: hex("#a687ee"),
            btn: hex("#2c2a27"),
        }
    } else {
        Theme {
            dark,
            chrome: hex("#2a2826"),
            ctext: hex("#f1ece2"),
            rack: hex("#1b1a19"),
            rail: hex("#8f8b84"),
            rail_hi: hex("#bdb8ae"),
            hole: hex("#2a2826"),
            panel: hex("#e8e2d4"),
            panel_edge: hex("#b9b2a2"),
            ink: hex("#2a2520"),
            ink2: hex("#5d564c"),
            plate: hex("#2f2b27"),
            plate_ink: hex("#f1ece2"),
            knob: hex("#2a2826"),
            knob_hi: hex("#57524b"),
            pointer: hex("#f4efe6"),
            skirt: hex("#cfc7b6"),
            tick: hex("#6b6357"),
            nut: hex("#c3bfb6"),
            nut_edge: hex("#8b877f"),
            hole_c: hex("#121110"),
            display: hex("#23211e"),
            display_ink: hex("#efe6d2"),
            seg_bg: hex("#d8d1c1"),
            seg_on: hex("#2a2520"),
            seg_on_text: hex("#f4efe6"),
            sel: hex("#2a6ae0"),
            warn: hex("#b0661a"),
            audio: hex("#e0962b"),
            cv: hex("#23a597"),
            gate: hex("#8d67d6"),
            btn: hex("#3a3733"),
        }
    }
}

impl Theme {
    /// Jack and cable colour by signal kind.
    pub fn signal(&self, t: PortType) -> Color32 {
        match t {
            PortType::Audio => self.audio,
            PortType::Cv | PortType::UnipolarCv | PortType::Pitch => self.cv,
            PortType::Gate => self.gate,
        }
    }

    /// Chrome (toolbar, drawer, menus) for this theme. Both themes keep dark chrome, as designed.
    pub fn visuals(&self) -> egui::Visuals {
        let mut v = egui::Visuals::dark();
        v.panel_fill = self.chrome;
        v.window_fill = self.chrome;
        v.extreme_bg_color = if self.dark { self.rack } else { hex("#1f1d1b") };
        v.override_text_color = Some(self.ctext);
        v.selection.bg_fill = self.sel;
        v
    }
}

/// IBM Plex (the mockups' face) and DejaVu (arrows, symbols) from `/usr/share/fonts` when
/// installed, else egui's bundled fonts.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let dir = "/usr/share/fonts/truetype";
    let load = [
        (
            "plex-sans",
            format!("{dir}/ibm-plex/IBMPlexSans-Regular.ttf"),
            egui::FontFamily::Proportional,
            0,
        ),
        (
            "plex-mono",
            format!("{dir}/ibm-plex/IBMPlexMono-Regular.ttf"),
            egui::FontFamily::Monospace,
            0,
        ),
        (
            "dejavu",
            format!("{dir}/dejavu/DejaVuSans.ttf"),
            egui::FontFamily::Proportional,
            1,
        ),
        (
            "dejavu",
            format!("{dir}/dejavu/DejaVuSans.ttf"),
            egui::FontFamily::Monospace,
            1,
        ),
    ];
    for (name, path, family, at) in load {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        fonts
            .font_data
            .entry(name.to_string())
            .or_insert_with(|| std::sync::Arc::new(egui::FontData::from_owned(bytes)));
        let list = fonts.families.entry(family).or_default();
        list.insert(at.min(list.len()), name.to_string());
    }
    ctx.set_fonts(fonts);
}
