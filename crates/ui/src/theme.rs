//! A-light / A-dark: the approved revision-2 palettes (`docs/design/revision-2/render2.py`,
//! the same values the `rev2_proto` example draws with), and the chrome/fonts that go with them.

use crate::style::{mix, Style};
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

/// The rack's palette, read from the theme: face colours come from the section families, the
/// rest from the colour roles. One mapping, so a second theme restyles every face.
pub fn theme(st: &Style) -> Theme {
    let r = &st.roles;
    let dark = st.dark;
    Theme {
        dark,
        chrome: r.bg,
        ctext: r.text,
        rack: r.rack,
        rail: r.rail,
        rail_hi: r.text3,
        hole: r.hole,
        panel: r.surface,
        panel_edge: r.line2,
        ink: r.text,
        ink2: r.text2,
        plate: mix(r.knob, r.hole, 0.5),
        plate_ink: r.knob_ink,
        knob: r.knob,
        knob_hi: r.knob_hi,
        pointer: r.knob_ink,
        skirt: mix(r.knob, r.knob_hi, 0.25),
        tick: r.text2,
        nut: mix(r.knob, r.knob_hi, 0.35),
        nut_edge: mix(r.knob, r.knob_hi, 0.8),
        hole_c: r.hole,
        display: r.disp_bg,
        display_ink: r.disp_trace,
        seg_bg: r.inset,
        seg_on: r.text,
        seg_on_text: r.surface,
        sel: r.focus,
        warn: r.warn,
        audio: r.audio,
        cv: r.cv,
        gate: r.gate,
        btn: mix(r.inset, r.raised, 0.4),
    }
}

fn hex(s: &str) -> Color32 {
    Color32::from_hex(s).expect("valid colour")
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
        (
            "plex-sans",
            include_bytes!("../assets/IBMPlexSans-Regular.ttf"),
            egui::FontFamily::Proportional,
        ),
        (
            "plex-mono",
            include_bytes!("../assets/IBMPlexMono-Regular.ttf"),
            egui::FontFamily::Monospace,
        ),
        (
            "plex-sans-medium",
            include_bytes!("../assets/IBMPlexSans-Medium.ttf"),
            egui::FontFamily::Name("kabl-sans-medium".into()),
        ),
        (
            "plex-sans-semibold",
            include_bytes!("../assets/IBMPlexSans-SemiBold.ttf"),
            egui::FontFamily::Name("kabl-sans-semibold".into()),
        ),
        (
            "plex-mono-medium",
            include_bytes!("../assets/IBMPlexMono-Medium.ttf"),
            egui::FontFamily::Name("kabl-mono-medium".into()),
        ),
    ];
    for (name, bytes, family) in faces {
        fonts.font_data.insert(
            name.into(),
            std::sync::Arc::new(egui::FontData::from_static(bytes)),
        );
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, name.into());
    }
    ctx.set_fonts(fonts);
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("kabl-fonts"), pass));
}

/// egui applies new fonts at the next pass: true once that has happened.
pub fn fonts_ready(ctx: &egui::Context) -> bool {
    let pass = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<u64>(egui::Id::new("kabl-fonts")))
        .is_some_and(|p| pass > p)
}

/// Fonts for hosts that never called [`install_fonts`] (the plugin editor): once per context.
pub fn ensure_fonts(ctx: &egui::Context) {
    if ctx
        .data(|d| d.get_temp::<u64>(egui::Id::new("kabl-fonts")))
        .is_none()
    {
        install_fonts(ctx);
    }
}

/// Original sectional palettes. Geometry, identities and sound remain unchanged.
/// The palette of one module's face: its section family over the rack palette.
pub fn panel_theme(st: &Style, kind: &str) -> Theme {
    let mut t = theme(st);
    let sec = st.section_of(kind);
    t.panel = sec.base;
    t.panel_edge = t.panel.lerp_to_gamma(Color32::BLACK, 0.3);
    t.ink = sec.ink;
    t.ink2 = sec.ink2;
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
