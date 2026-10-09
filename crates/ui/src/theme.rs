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
            chrome: hex("#1b2021"),
            ctext: hex("#efe8da"),
            rack: hex("#101516"),
            rail: hex("#3f4748"),
            rail_hi: hex("#8c877e"),
            hole: hex("#161513"),
            panel: hex("#2f2c28"),
            panel_edge: hex("#4d4943"),
            ink: hex("#efe8da"),
            ink2: hex("#b2a893"),
            plate: hex("#191816"),
            plate_ink: hex("#efe8da"),
            knob: hex("#292c2b"),
            knob_hi: hex("#666a66"),
            pointer: hex("#f0e8da"),
            skirt: hex("#242724"),
            tick: hex("#8d8475"),
            nut: hex("#302f2c"),
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
            chrome: hex("#eee9df"),
            ctext: hex("#302b26"),
            rack: hex("#dfd9cd"),
            rail: hex("#c5bfb3"),
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
            skirt: hex("#514f49"),
            tick: hex("#6b6357"),
            nut: hex("#55534e"),
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
            btn: hex("#ddd6c8"),
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

    /// Matching material, distinct warm light and graphite dark UI surfaces.
    pub fn visuals(&self) -> egui::Visuals {
        let mut v = if self.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        v.panel_fill = self.chrome;
        v.window_fill = self.chrome;
        v.extreme_bg_color = self.rack;
        v.faint_bg_color = self.btn;
        v.widgets.noninteractive.bg_fill = self.chrome;
        v.widgets.inactive.bg_fill = self.btn;
        v.widgets.hovered.bg_fill = self.btn.gamma_multiply(if self.dark { 1.2 } else { 0.96 });
        v.widgets.active.bg_fill = hex(if self.dark { "#355580" } else { "#c1d3e9" });
        v.override_text_color = Some(self.ctext);
        v.selection.bg_fill = hex(if self.dark { "#355580" } else { "#c1d3e9" });
        v
    }
}

/// Portable IBM Plex typography; OFL-1.1 and reserved name notice in assets/FONT-LICENSE.txt.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let faces: [(&str, &[u8], egui::FontFamily); 5] = [
        ("plex-sans", include_bytes!("../assets/IBMPlexSans-Regular.ttf"), egui::FontFamily::Proportional),
        ("plex-mono", include_bytes!("../assets/IBMPlexMono-Regular.ttf"), egui::FontFamily::Monospace),
        ("plex-sans-medium", include_bytes!("../assets/IBMPlexSans-Medium.ttf"), egui::FontFamily::Name("kabl-sans-medium".into())),
        ("plex-sans-semibold", include_bytes!("../assets/IBMPlexSans-SemiBold.ttf"), egui::FontFamily::Name("kabl-sans-semibold".into())),
        ("plex-mono-medium", include_bytes!("../assets/IBMPlexMono-Medium.ttf"), egui::FontFamily::Name("kabl-mono-medium".into())),
    ];
    for (name, bytes, family) in faces {
        fonts.font_data.insert(name.into(), std::sync::Arc::new(egui::FontData::from_static(bytes)));
        fonts.families.entry(family).or_default().insert(0, name.into());
    }
    ctx.set_fonts(fonts);
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("kabl-fonts"), true));
}

/// Fonts for hosts that never called [`install_fonts`] (the plugin editor): once per context.
pub fn ensure_fonts(ctx: &egui::Context) {
    if ctx.data(|d| d.get_temp::<bool>(egui::Id::new("kabl-fonts"))).is_none() {
        install_fonts(ctx);
    }
}

/// Original sectional palettes. Geometry, identities and sound remain unchanged.
pub fn panel_theme(dark: bool, kind: &str) -> Theme {
    let mut t = theme(dark);
    let (light, night) = match kind {
        "osc.va" | "osc.fm" | "osc.fm6" | "osc.wt" | "osc" | "noise" => ("#cf947c", "#926958"),
        "filter.svf" | "filter" => ("#a8b99d", "#62735c"),
        "env.adsr" | "lfo" => ("#b8afca", "#71617d"),
        "vca" | "mix" => ("#c4b891", "#7b6c52"),
        "midi.in" | "midi" | "clock" | "seq" | "cues" => ("#a6babc", "#4b6469"),
        _ => ("#d6ccbc", "#696157"),
    };
    t.panel = hex(if dark { night } else { light });
    t.panel_edge = t.panel.lerp_to_gamma(Color32::BLACK, 0.3);
    t.ink = hex(if dark { "#fff4e6" } else { "#292721" });
    t.ink2 = hex(if dark { "#e8dfd2" } else { "#49443b" });
    t.tick = t.ink2;
    t
}

/// Satin coating with shared light direction, subtle grain and restrained edge depth.
/// Original procedural art: no external texture or bitmap labels.
pub fn satin(p: &egui::Painter, r: egui::Rect, t: &Theme) {
    use egui::{pos2, Color32, Stroke};
    let mut mesh = egui::Mesh::default();
    let top = t
        .panel
        .lerp_to_gamma(Color32::WHITE, if t.dark { 0.045 } else { 0.08 });
    let bottom = t.panel.lerp_to_gamma(Color32::BLACK, 0.065);
    mesh.colored_vertex(r.left_top(), top);
    mesh.colored_vertex(r.right_top(), top);
    mesh.colored_vertex(r.right_bottom(), bottom);
    mesh.colored_vertex(r.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(egui::Shape::mesh(mesh));
    // Sparse low-alpha grain remains subordinate to editable text at every zoom.
    let mut y = r.top() + 2.0;
    while y < r.bottom() {
        p.line_segment(
            [pos2(r.left() + 1.0, y), pos2(r.right() - 1.0, y)],
            Stroke::new(0.5, Color32::from_white_alpha(2)),
        );
        y += 3.7;
    }
    p.rect_stroke(
        r,
        egui::CornerRadius::same(2),
        Stroke::new(1.0, t.panel_edge),
        egui::StrokeKind::Inside,
    );
    p.line_segment(
        [
            r.left_top() + egui::vec2(1.0, 1.0),
            r.right_top() + egui::vec2(-1.0, 1.0),
        ],
        Stroke::new(1.0, Color32::from_white_alpha(38)),
    );
}
