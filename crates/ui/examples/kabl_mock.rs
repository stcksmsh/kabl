//! G1b mock: the 7 R1 faces and a parts sheet in face style `line_solid`, drawn with the real
//! egui painter, in app themes `graphite` (dark) and `paper` (light), accent amber, at zoom
//! 100 / 75 / 58 %. Tokens (PLAN §6) and layout (PLAN §4, geometry of mockups/r1-faces.html)
//! live here only: nothing in `style/*`, `kit/*`, `theme.rs`, `rack.rs` or `lib.rs` is used for
//! the look. After drawing, the G0 checker rules run on the rects the mock drew.
//!
//! `xvfb-run -a -s "-screen 0 1100x4500x24" cargo run --release -p kabl-ui --example kabl_mock -- graphite out.png`
//! (theme: `graphite` | `paper`). Silent: no audio device is opened.

use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Shape, Stroke,
    StrokeKind,
};
use kabl_ui::style::{contrast, Font};
use std::sync::Arc;

const FACE_H: f32 = 340.0;
const M: f32 = 12.0;
const HIDE: f32 = 9.5;
const WELL_Y: f32 = 52.0;
const WELL_H: f32 = 64.0;
const CTRL_TOP: f32 = 124.0;
const CTRL_BOT: f32 = 236.0;
const TRAY_Y: f32 = 244.0;
const TRAY_H: f32 = 86.0;
const IN_JACK: f32 = 273.0;
const OUT_JACK: f32 = 316.0;
const PLATE_Y: f32 = 288.0;
const SHEET_W: f32 = 1000.0;

// ---------------------------------------------------------------- tokens

fn hex(s: &str) -> Color32 {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap();
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}
fn mixc(a: Color32, b: Color32, t: f32) -> Color32 {
    let f = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}
fn al(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

#[derive(Clone, Copy)]
struct Th {
    dark: bool,
    bg: Color32,
    surf: Color32,
    raised: Color32,
    inset: Color32,
    line: Color32,
    line2: Color32,
    text: Color32,
    text2: Color32,
    edge: Color32,
    audio: Color32,
    cv: Color32,
    gate: Color32,
    pitch: Color32,
    accent: Color32,
    on_accent: Color32,
    skirt: Color32,
    cap0: Color32,
    cap1: Color32,
    pointer: Color32,
    nut: Color32,
    nut_edge: Color32,
    hole: Color32,
    track: Color32,
    track_ink: Color32,
    shadow: u8,
}

fn graphite() -> Th {
    Th {
        dark: true,
        bg: hex("#111214"),
        surf: hex("#1A1B1E"),
        raised: hex("#232529"),
        inset: hex("#0F1012"),
        line: hex("#2A2C31"),
        line2: hex("#393C42"),
        text: hex("#ECEAE6"),
        text2: hex("#A29F99"),
        edge: Color32::from_rgba_unmultiplied(236, 234, 230, 66),
        audio: hex("#F59E3B"),
        cv: hex("#38C6B2"),
        gate: hex("#A68BFA"),
        pitch: hex("#5BA8F7"),
        accent: hex("#F2A33A"),
        on_accent: hex("#1C1206"),
        skirt: hex("#141518"),
        cap0: hex("#3C3F45"),
        cap1: hex("#1D1F23"),
        pointer: hex("#F2F0EB"),
        nut: hex("#2B2D32"),
        nut_edge: hex("#45484E"),
        hole: hex("#050506"),
        track: hex("#0F1012"),
        track_ink: hex("#D8D5CF"),
        shadow: 115,
    }
}

fn paper() -> Th {
    Th {
        dark: false,
        bg: hex("#ECEAE5"),
        surf: hex("#FBFAF7"),
        raised: hex("#FFFFFF"),
        inset: hex("#E4E1DA"),
        line: hex("#DCD8D0"),
        line2: hex("#C8C4BB"),
        text: hex("#1A1917"),
        text2: hex("#5F5C56"),
        edge: Color32::from_rgba_unmultiplied(26, 25, 23, 77),
        audio: hex("#B66518"),
        cv: hex("#11897F"),
        gate: hex("#7651D6"),
        pitch: hex("#2E7CD2"),
        accent: hex("#A97229"),
        on_accent: hex("#1C1206"),
        skirt: hex("#141518"),
        cap0: hex("#3C3F45"),
        cap1: hex("#1D1F23"),
        pointer: hex("#F2F0EB"),
        nut: hex("#FFFFFF"),
        nut_edge: hex("#C8C4BB"),
        hole: hex("#24232A"),
        track: hex("#D6D2C8"),
        track_ink: hex("#2A2824"),
        shadow: 36,
    }
}

impl Th {
    fn sig(&self, k: &str) -> Color32 {
        match k {
            "pitch" => self.pitch,
            "gate" => self.gate,
            "audio" => self.audio,
            _ => self.cv,
        }
    }
    /// Face title colour: family hue mixed to white (dark) or black (light), pushed further
    /// until it reaches 4.5:1 on the face (PLAN §6 marks this pair "contrast unverified").
    fn title_col(&self, fam: &str) -> Color32 {
        let h = hex(match fam {
            "osc" => "#E07A5F",
            "filter" => "#7FB59A",
            "env" => "#9D8DF1",
            "timing" => "#5FA8D3",
            "amp" => "#E3BE5C",
            _ => "#B9B2A6",
        });
        let to = if self.dark {
            Color32::WHITE
        } else {
            Color32::BLACK
        };
        let mut t = if self.dark { 0.15 } else { 0.35 };
        loop {
            let c = mixc(h, to, t);
            if contrast(c, self.surf) >= 4.5 || t >= 0.95 {
                return c;
            }
            t += 0.05;
        }
    }
    fn well_ink(&self) -> Color32 {
        if self.dark {
            hex("#C9C6C0")
        } else {
            hex("#3A3833")
        }
    }
}

// ---------------------------------------------------------------- face context + checker data

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LK {
    Text,
    Knob,
    Jack,
    Seg,
    Chip,
    Pad,
}

struct Leaf {
    id: u32,
    #[allow(dead_code)]
    k: LK,
    r: Rect,
    own: u32,
    inside: u32,
    fg: Option<Color32>,
    bg: Color32,
    what: String,
}

struct Face<'a> {
    p: &'a Painter,
    th: Th,
    o: Pos2,
    z: f32,
    w: f32,
    name: &'static str,
    #[allow(dead_code)]
    fam: &'static str,
    leaves: Vec<Leaf>,
    conts: Vec<Rect>,
    /// (error?, text) for text not drawn.
    drops: Vec<(bool, String)>,
    next: u32,
}

fn fnt(f: Font, size: f32) -> FontId {
    FontId::new(size, f.family(true))
}

impl<'a> Face<'a> {
    fn compact(&self) -> bool {
        self.z < 0.864
    }
    fn wide(&self) -> bool {
        self.w >= 480.0
    }
    fn pt(&self, x: f32, y: f32) -> Pos2 {
        self.o + vec2(x, y) * self.z
    }
    fn rc(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.pt(x, y), vec2(w, h) * self.z)
    }
    fn lw(&self, w: f32) -> f32 {
        (w * self.z).max(0.75)
    }
    /// Compact tier holds text at 9.5 screen px.
    fn fsz(&self, base: f32) -> f32 {
        if self.compact() {
            base.max(HIDE / self.z)
        } else {
            base
        }
    }
    fn tw(&self, s: &str, size: f32, f: Font) -> f32 {
        self.p
            .layout_no_wrap(s.to_string(), fnt(f, size * self.z), Color32::WHITE)
            .size()
            .x
            / self.z
    }
    fn fit(
        &self,
        full: &str,
        short: Option<&str>,
        size: f32,
        maxw: f32,
        f: Font,
    ) -> Option<String> {
        if self.tw(full, size, f) <= maxw {
            return Some(full.into());
        }
        short
            .filter(|s| self.tw(s, size, f) <= maxw)
            .map(str::to_string)
    }
    fn id(&mut self) -> u32 {
        self.next += 1;
        self.next
    }
    #[allow(clippy::too_many_arguments)]
    fn leaf(
        &mut self,
        k: LK,
        r: Rect,
        own: u32,
        inside: u32,
        fg: Option<Color32>,
        bg: Color32,
        what: &str,
    ) -> u32 {
        let id = self.id();
        // face-local screen px
        let r = r.translate(-self.o.to_vec2());
        self.leaves.push(Leaf {
            id,
            k,
            r,
            own,
            inside,
            fg,
            bg,
            what: what.into(),
        });
        id
    }
    fn drop_text(&mut self, what: &str, s: &str, required: bool) {
        self.drops.push((
            required && !self.compact(),
            format!("{what} '{s}' not drawn"),
        ));
    }
    /// Draws `s` with its top at design `top`; `x` per anchor (0 start, 1 middle, 2 end).
    #[allow(clippy::too_many_arguments)]
    fn text(
        &mut self,
        s: &str,
        x: f32,
        top: f32,
        size: f32,
        f: Font,
        col: Color32,
        anchor: u8,
        own: u32,
        inside: u32,
        bg: Color32,
    ) {
        let w = self.tw(s, size, f);
        let (x0, al_) = match anchor {
            1 => (x - w / 2.0, Align2::CENTER_CENTER),
            2 => (x - w, Align2::CENTER_CENTER),
            _ => (x, Align2::CENTER_CENTER),
        };
        let cy = top + size * 0.55;
        self.p.text(
            self.pt(x0 + w / 2.0, cy),
            al_,
            s,
            fnt(f, size * self.z),
            col,
        );
        let r = Rect::from_center_size(self.pt(x0 + w / 2.0, cy), vec2(w, size * 0.85) * self.z);
        self.leaf(LK::Text, r, own, inside, Some(col), bg, s);
    }
    fn fill(&self, r: Rect, rad: f32, c: Color32) {
        self.p.rect_filled(
            r,
            CornerRadius::same((rad * self.z).round().clamp(0.0, 255.0) as u8),
            c,
        );
    }
    fn stroke_r(&self, r: Rect, rad: f32, s: Stroke) {
        self.p.rect_stroke(
            r,
            CornerRadius::same((rad * self.z).round().clamp(0.0, 255.0) as u8),
            s,
            StrokeKind::Inside,
        );
    }
    fn line(&self, a: (f32, f32), b: (f32, f32), w: f32, c: Color32) {
        self.p.line_segment(
            [self.pt(a.0, a.1), self.pt(b.0, b.1)],
            Stroke::new(self.lw(w), c),
        );
    }
    fn arc(&self, c: (f32, f32), r: f32, t0: f32, t1: f32, w: f32, col: Color32) {
        let a0 = (-225.0f32).to_radians();
        let sw = 270.0f32.to_radians();
        let n = (((t1 - t0).abs() * 40.0).ceil() as usize).max(2);
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| {
                let a = a0 + sw * (t0 + (t1 - t0) * i as f32 / n as f32);
                self.pt(c.0 + a.cos() * r, c.1 + a.sin() * r)
            })
            .collect();
        self.p.add(Shape::line(pts, Stroke::new(self.lw(w), col)));
    }
    fn well(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let r = self.rc(x, y, w, h);
        self.fill(r, 4.0, self.th.inset);
        self.stroke_r(r, 4.0, Stroke::new(1.0, self.th.line2));
        self.conts.push(r.translate(-self.o.to_vec2()));
    }
    fn gridlines(&self, x: f32, y: f32, w: f32, h: f32, nx: u32, ny: u32) {
        let c = al(self.th.well_ink(), 20);
        for i in 1..nx {
            let xx = x + w * i as f32 / nx as f32;
            self.line((xx, y + 3.0), (xx, y + h - 3.0), 1.0, c);
        }
        for j in 1..ny {
            let yy = y + h * j as f32 / ny as f32;
            self.line((x + 3.0, yy), (x + w - 3.0, yy), 1.0, c);
        }
    }
    fn poly_line(&self, pts: &[(f32, f32)], w: f32, c: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.pt(x, y)).collect();
        self.p.add(Shape::line(v, Stroke::new(self.lw(w), c)));
    }
    fn poly_fill(&self, pts: &[(f32, f32)], c: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.pt(x, y)).collect();
        self.p.add(Shape::convex_polygon(v, c, Stroke::NONE));
    }
    fn circle(&self, c: (f32, f32), r: f32, fill: Color32) {
        self.p.circle_filled(self.pt(c.0, c.1), r * self.z, fill);
    }
    fn ring(&self, c: (f32, f32), r: f32, w: f32, col: Color32) {
        self.p
            .circle_stroke(self.pt(c.0, c.1), r * self.z, Stroke::new(self.lw(w), col));
    }

    // ---- hardware

    /// Knob: value arc in accent (glow underlay on dark), solid cap with drop shadow.
    fn knob(
        &mut self,
        cx: f32,
        cy: f32,
        r: f32,
        v: f32,
        mod_: Option<(f32, f32)>,
        state: &str,
    ) -> u32 {
        let th = self.th;
        let ar = r + 3.5;
        let dis = state == "disabled";
        let fade = |c: Color32| if dis { al(c, 100) } else { c };
        self.arc(
            (cx, cy),
            ar,
            0.0,
            1.0,
            2.5,
            al(th.text, if state == "hover" { 82 } else { 41 }),
        );
        if v > 0.001 {
            if th.dark {
                self.arc((cx, cy), ar, 0.0, v, 6.0, al(th.accent, 40));
            }
            self.arc((cx, cy), ar, 0.0, v, 2.5, fade(th.accent));
        }
        if let Some((m, live)) = mod_ {
            self.arc((cx, cy), ar, v.min(m), v.max(m), 2.5, th.cv);
            let a = (-225.0f32).to_radians() + 270.0f32.to_radians() * live;
            self.circle((cx + a.cos() * ar, cy + a.sin() * ar), 3.2, th.cv);
        }
        self.circle((cx, cy + 3.0), r + 1.0, al(Color32::BLACK, th.shadow / 4));
        self.circle((cx, cy + 1.5), r + 0.5, al(Color32::BLACK, th.shadow / 2));
        self.circle((cx, cy), r, fade(th.skirt));
        for i in 0..=10 {
            let a = (-225.0f32).to_radians() + 270.0f32.to_radians() * i as f32 / 10.0;
            self.line(
                (cx + a.cos() * (r - 1.2), cy + a.sin() * (r - 1.2)),
                (cx + a.cos() * (r - 3.6), cy + a.sin() * (r - 3.6)),
                1.0,
                al(th.pointer, 90),
            );
        }
        for i in 0..4 {
            let t = i as f32 / 3.0;
            self.circle(
                (cx - r * 0.06 * (1.0 - t), cy - r * 0.08 * (1.0 - t)),
                r * (0.74 - 0.14 * t),
                fade(mixc(th.cap0, th.cap1, t)),
            );
        }
        let a = (-225.0f32).to_radians() + 270.0f32.to_radians() * v;
        self.line(
            (cx + a.cos() * r * 0.18, cy + a.sin() * r * 0.18),
            (cx + a.cos() * r * 0.68, cy + a.sin() * r * 0.68),
            if r > 18.0 { 2.4 } else { 2.0 },
            fade(th.pointer),
        );
        if state == "focus" {
            self.ring((cx, cy), r + 7.5, 2.0, th.accent);
        }
        let e = ar + 1.25;
        let rr = self.rc(cx - e, cy - e, 2.0 * e, 2.0 * e);
        self.leaf(LK::Knob, rr, 0, 0, None, th.surf, "knob")
    }

    #[allow(clippy::too_many_arguments)]
    fn knob_cell(
        &mut self,
        l: &str,
        s: Option<&str>,
        val: Option<&str>,
        v: f32,
        cx: f32,
        cy: f32,
        r: f32,
        pitch: f32,
    ) {
        let th = self.th;
        let kid = self.knob(cx, cy, r, v, None, "");
        let ls = self.fsz(11.0);
        let lmax = pitch - 4.0;
        let lab = if self.compact() {
            self.fit(s.unwrap_or(l), None, ls, lmax, Font::SansMedium)
        } else {
            self.fit(l, s, ls, lmax, Font::SansMedium)
        };
        let l_top = cy - r - 3.5 - 1.25 - 3.0 - ls * 1.1;
        match lab {
            Some(t) => self.text(
                &t,
                cx,
                l_top,
                ls,
                Font::SansMedium,
                th.text,
                1,
                kid,
                0,
                th.surf,
            ),
            None => self.drop_text("label", l, true),
        }
        if !self.compact() {
            if let Some(val) = val {
                match self.fit(val, None, 11.0, lmax, Font::Mono) {
                    Some(t) => self.text(
                        &t,
                        cx,
                        cy + r + 3.5 + 1.25 + 3.0,
                        11.0,
                        Font::Mono,
                        th.text2,
                        1,
                        kid,
                        0,
                        th.surf,
                    ),
                    None => self.drop_text("value", val, false),
                }
            }
        }
    }

    /// Hex nut jack; `ch` 2 = stereo (double ring).
    fn jack(&mut self, cx: f32, cy: f32, sig: Color32, ch: u8, plug: bool, hover: bool) -> u32 {
        let th = self.th;
        let hexp = |r: f32, dy: f32| -> Vec<Pos2> {
            (0..6)
                .map(|i| {
                    let a = std::f32::consts::FRAC_PI_6 + i as f32 * std::f32::consts::FRAC_PI_3;
                    self.pt(cx + a.cos() * r, cy + dy + a.sin() * r)
                })
                .collect()
        };
        self.p.add(Shape::convex_polygon(
            hexp(11.5, 1.2),
            al(Color32::BLACK, th.shadow / 2),
            Stroke::NONE,
        ));
        self.p.add(Shape::convex_polygon(
            hexp(11.0, 0.0),
            th.nut,
            Stroke::new(self.lw(0.8), th.nut_edge),
        ));
        if hover {
            self.ring((cx, cy), 14.0, 2.0, al(sig, 128));
        }
        if ch == 2 {
            self.circle((cx, cy), 8.1, th.hole);
            self.ring((cx, cy), 8.1, 1.7, sig);
            self.ring((cx, cy), 5.0, 1.7, sig);
            self.circle((cx, cy), 2.4, th.hole);
        } else {
            self.circle((cx, cy), 7.6, sig);
            self.circle((cx, cy), 5.2, th.hole);
        }
        if plug {
            self.circle((cx, cy + 1.5), 8.0, al(Color32::BLACK, 90));
            self.circle((cx, cy), 7.5, sig);
            self.circle((cx - 2.0, cy - 2.0), 3.0, al(Color32::WHITE, 90));
        }
        let rr = self.rc(cx - 11.0, cy - 11.0, 22.0, 22.0);
        self.leaf(LK::Jack, rr, 0, 0, None, th.surf, "jack")
    }

    /// Segmented selector. Returns total width.
    #[allow(clippy::too_many_arguments)]
    fn seg(
        &mut self,
        x: f32,
        y: f32,
        opts: &[&str],
        sel: usize,
        h: f32,
        seg_w: Option<f32>,
        cap: Option<&str>,
        accent: Option<Color32>,
        playing: Option<usize>,
    ) -> f32 {
        let th = self.th;
        let n = opts.len();
        let sw = seg_w.unwrap_or_else(|| {
            opts.iter()
                .map(|o| self.tw(o, 11.0, Font::MonoMedium))
                .fold(0.0, f32::max)
                + 14.0
        });
        let cap_w = cap.map_or(0.0, |c| self.tw(c, 11.0, Font::MonoMedium) + 8.0);
        let ctx = self.th.surf;
        if let Some(c) = cap {
            let cs = self.fsz(11.0);
            if !self.compact() || self.tw(c, cs, Font::MonoMedium) <= cap_w - 4.0 {
                self.text(
                    c,
                    x,
                    y + (h - cs * 1.1) / 2.0,
                    cs,
                    Font::MonoMedium,
                    th.text2,
                    0,
                    0,
                    0,
                    ctx,
                );
            }
        }
        let tx = x + cap_w;
        let tr = self.rc(tx, y, sw * n as f32, h);
        self.fill(tr, 4.0, th.track);
        self.stroke_r(tr, 4.0, Stroke::new(1.0, th.line2));
        let sid = self.leaf(LK::Seg, tr, 0, 0, None, th.surf, "selector");
        let ts = self.fsz(11.0);
        let all_fit = opts
            .iter()
            .all(|o| self.tw(o, ts, Font::MonoMedium) <= sw - 4.0);
        for (i, o) in opts.iter().enumerate() {
            let sx = tx + i as f32 * sw;
            let on = i == sel;
            let capc = accent.unwrap_or(th.accent);
            if on {
                let r = self.rc(sx + 1.5, y + 1.5, sw - 3.0, h - 3.0);
                self.p.rect_filled(
                    r.translate(vec2(0.0, self.z)),
                    CornerRadius::same(3),
                    al(Color32::BLACK, th.shadow / 2),
                );
                self.fill(r, 3.0, capc);
            }
            if all_fit {
                let (fg, bg) = if on {
                    (th.on_accent, capc)
                } else {
                    (th.track_ink, th.track)
                };
                let fg = if on && accent.is_some() {
                    if contrast(Color32::WHITE, capc) >= contrast(th.on_accent, capc) {
                        Color32::WHITE
                    } else {
                        th.on_accent
                    }
                } else {
                    fg
                };
                self.text(
                    o,
                    sx + sw / 2.0,
                    y + (h - ts * 1.1) / 2.0,
                    ts,
                    Font::MonoMedium,
                    fg,
                    1,
                    0,
                    sid,
                    bg,
                );
            }
            if playing == Some(i) {
                self.circle((sx + sw - 4.0, y + 4.0), 2.0, th.gate);
            }
        }
        cap_w + sw * n as f32
    }
}

// ---------------------------------------------------------------- Plex Sans Condensed (locked: titles + section labels)

fn cond_family() -> egui::FontFamily {
    egui::FontFamily::Name("kabl-cond-semibold".into())
}

/// Uppercase, +0.07 em tracking. Returns the screen width of `s` at `px`.
fn cond_width(p: &Painter, s: &str, px: f32) -> f32 {
    let u = s.to_uppercase();
    p.layout_no_wrap(u.clone(), FontId::new(px, cond_family()), Color32::WHITE)
        .size()
        .x
        + 0.07 * px * u.chars().count().saturating_sub(1) as f32
}

/// Draws `s` left-aligned, vertically centred on `at`, glyph by glyph so the tracking is kept.
fn cond_draw(p: &Painter, at: Pos2, s: &str, px: f32, col: Color32) {
    let u = s.to_uppercase();
    let font = FontId::new(px, cond_family());
    let mut prefix = String::new();
    for ch in u.chars() {
        let x = if prefix.is_empty() {
            0.0
        } else {
            p.layout_no_wrap(prefix.clone(), font.clone(), col).size().x
                + 0.07 * px * prefix.chars().count() as f32
        };
        p.text(
            at + vec2(x, 0.0),
            Align2::LEFT_CENTER,
            ch.to_string(),
            font.clone(),
            col,
        );
        prefix.push(ch);
    }
}

fn cond_text(p: &Painter, at: Pos2, s: &str, px: f32, col: Color32) {
    cond_draw(p, at, s, px, col);
}

impl Face<'_> {
    fn cond_w(&self, s: &str, size: f32) -> f32 {
        cond_width(self.p, s, size * self.z) / self.z
    }
    fn cond_title(&mut self, s: &str, x: f32, top: f32, size: f32, col: Color32) {
        let cy = top + size * 0.55;
        cond_draw(self.p, self.pt(x, cy), s, size * self.z, col);
        let w = self.cond_w(s, size);
        let r = Rect::from_min_size(self.pt(x, cy - size * 0.425), vec2(w, size * 0.85) * self.z);
        let bg = self.th.surf;
        self.leaf(LK::Text, r, 0, 0, Some(col), bg, s);
    }
}

// ---------------------------------------------------------------- specs

struct Ctl {
    l: &'static str,
    s: Option<&'static str>,
    val: &'static str,
    v: f32,
    size: Option<char>,
}
fn c(l: &'static str, s: Option<&'static str>, val: &'static str, v: f32) -> Ctl {
    Ctl {
        l,
        s,
        val,
        v,
        size: None,
    }
}
type Port = (&'static str, &'static str, u8, Option<&'static str>);

struct Spec {
    id: &'static str,
    title: &'static str,
    short: &'static str,
    fam: &'static str,
    w: f32,
    num: u32,
    more: u32,
    well: &'static str,
    header: Option<(&'static [&'static str], usize)>,
    rows: Vec<Vec<Ctl>>,
    sel_row: Option<(&'static [&'static str], usize)>,
    body: &'static str,
    ins: Vec<Port>,
    outs: Vec<Port>,
}

fn specs() -> Vec<Spec> {
    let lv = ["0.80", "0.62", "1.00", "0.35"];
    let lvv = [0.8, 0.62, 1.0, 0.35];
    let lvn = ["Level 1", "Level 2", "Level 3", "Level 4"];
    let lvs = ["Lv 1", "Lv 2", "Lv 3", "Lv 4"];
    vec![
        Spec {
            id: "osc.va",
            title: "VA Oscillator",
            short: "VA Osc",
            fam: "osc",
            w: 240.0,
            num: 10,
            more: 7,
            well: "saw",
            header: None,
            rows: vec![vec![
                Ctl {
                    size: Some('L'),
                    ..c("Frequency", Some("Freq"), "262 Hz", 0.56)
                },
                Ctl {
                    size: Some('S'),
                    ..c("Fine", None, "+0 ct", 0.5)
                },
                Ctl {
                    size: Some('S'),
                    ..c("Pulse width", Some("PW"), "50 %", 0.5)
                },
            ]],
            sel_row: Some((&["SIN", "TRI", "SAW", "SQR"], 2)),
            body: "",
            ins: vec![("Pitch", "pitch", 1, None), ("Sync", "gate", 1, None)],
            outs: vec![("Out", "audio", 1, None), ("Stereo", "audio", 2, None)],
        },
        Spec {
            id: "filter.svf",
            title: "SVF Filter",
            short: "SVF",
            fam: "filter",
            w: 210.0,
            num: 11,
            more: 0,
            well: "svf",
            header: None,
            rows: vec![vec![
                c("Cutoff", None, "1.00 kHz", 0.58),
                c("Resonance", Some("Res"), "0.30", 0.32),
            ]],
            sel_row: None,
            body: "",
            ins: vec![
                ("In", "audio", 1, None),
                ("Cutoff CV", "cv", 1, Some("Cut CV")),
                ("Res CV", "cv", 1, None),
            ],
            outs: vec![
                ("LP", "audio", 1, None),
                ("BP", "audio", 1, None),
                ("HP", "audio", 1, None),
                ("Notch", "audio", 1, Some("N")),
            ],
        },
        Spec {
            id: "env.adsr",
            title: "ADSR Envelope",
            short: "ADSR",
            fam: "env",
            w: 210.0,
            num: 12,
            more: 0,
            well: "adsr",
            header: Some((&["CONT", "KEY"], 0)),
            rows: vec![vec![
                c("Attack", Some("Atk"), "10 ms", 0.2),
                c("Decay", Some("Dec"), "180 ms", 0.42),
                c("Sustain", Some("Sus"), "0.62", 0.62),
                c("Release", Some("Rel"), "420 ms", 0.5),
            ]],
            sel_row: None,
            body: "",
            ins: vec![("Gate", "gate", 1, None)],
            outs: vec![("Out", "cv", 1, None)],
        },
        Spec {
            id: "mixer",
            title: "Mixer",
            short: "Mixer",
            fam: "amp",
            w: 240.0,
            num: 14,
            more: 0,
            well: "meters",
            header: None,
            rows: vec![(0..4)
                .map(|i| c(lvn[i], Some(lvs[i]), lv[i], lvv[i]))
                .collect()],
            sel_row: None,
            body: "",
            ins: vec![
                ("In 1", "audio", 1, None),
                ("In 2", "audio", 1, None),
                ("In 3", "audio", 1, None),
                ("In 4", "audio", 1, None),
            ],
            outs: vec![("Out", "audio", 1, None)],
        },
        Spec {
            id: "seq",
            title: "Sequencer",
            short: "Seq",
            fam: "timing",
            w: 660.0,
            num: 8,
            more: 9,
            well: "",
            header: None,
            rows: vec![],
            sel_row: None,
            body: "seq",
            ins: vec![("Clock", "gate", 1, None), ("Reset", "gate", 1, None)],
            outs: vec![
                ("Gate", "gate", 1, None),
                ("Pitch", "pitch", 1, None),
                ("Vel", "cv", 1, None),
            ],
        },
        Spec {
            id: "drum.grid",
            title: "Drum Grid",
            short: "Drums",
            fam: "timing",
            w: 660.0,
            num: 21,
            more: 0,
            well: "",
            header: None,
            rows: vec![],
            sel_row: None,
            body: "drum",
            ins: vec![
                ("Clock", "gate", 1, None),
                ("Reset", "gate", 1, None),
                ("Pattern", "cv", 1, Some("Pat")),
            ],
            outs: vec![
                ("Kick", "gate", 1, Some("BD")),
                ("Snare", "gate", 1, Some("SD")),
                ("Clap", "gate", 1, Some("CP")),
                ("Chat", "gate", 1, Some("CH")),
                ("Ohat", "gate", 1, Some("OH")),
                ("Tom", "gate", 1, Some("TM")),
                ("Perc 1", "gate", 1, Some("P1")),
                ("Perc 2", "gate", 1, Some("P2")),
            ],
        },
        Spec {
            id: "osc.fm6",
            title: "FM Voice (6 operators)",
            short: "FM Voice",
            fam: "osc",
            w: 660.0,
            num: 17,
            more: 12,
            well: "",
            header: None,
            rows: vec![],
            sel_row: None,
            body: "fm6",
            ins: vec![
                ("Pitch", "pitch", 1, None),
                ("Gate", "gate", 1, None),
                ("Velocity", "cv", 1, Some("Vel")),
            ],
            outs: vec![("Out", "audio", 1, None)],
        },
    ]
}

// ---------------------------------------------------------------- face drawing

fn knob_r(size: Option<char>, n: usize) -> f32 {
    match size {
        Some('L') => 24.0,
        Some('M') => 20.0,
        Some('S') => 16.0,
        _ => match n {
            0..=2 => 24.0,
            3 => 20.0,
            _ => 16.0,
        },
    }
}

fn paint_face(f: &mut Face, sp: &Spec) {
    let th = f.th;
    let (w, z) = (f.w, f.z);
    let compact = f.compact();
    let wide = f.wide();
    let bot = if wide { PLATE_Y - 10.0 } else { CTRL_BOT };
    let lift = if compact {
        (HIDE / z * 1.1 - 12.0).max(0.0)
    } else {
        0.0
    };

    // surface: line_solid panel = 1 px line art on the face fill, screws
    let face_r = f.rc(0.0, 0.0, w, FACE_H);
    f.p.rect_filled(
        face_r.translate(vec2(0.0, 2.0 * z)),
        CornerRadius::same((3.0 * z) as u8),
        al(Color32::BLACK, th.shadow / 3),
    );
    f.fill(face_r, 3.0, th.surf);
    f.stroke_r(face_r, 3.0, Stroke::new(1.0, th.edge));
    for (sx, sy) in [
        (7.0, 7.0),
        (w - 7.0, 7.0),
        (7.0, FACE_H - 7.0),
        (w - 7.0, FACE_H - 7.0),
    ] {
        f.circle((sx, sy), 2.4, th.inset);
        f.ring((sx, sy), 2.4, 0.8, th.line2);
        f.line((sx - 1.4, sy), (sx + 1.4, sy), 0.8, th.line2);
    }

    // header right cluster
    let mut rx = w - M;
    if sp.more > 0 {
        let t = format!("+{}", sp.more);
        let fs = f.fsz(11.0);
        let cw = (f.tw(&t, fs, Font::MonoMedium) + 12.0).max(26.0);
        rx -= cw;
        let r = f.rc(rx, 14.0, cw, 18.0);
        f.stroke_r(r, 4.0, Stroke::new(1.0, th.line2));
        let cid = f.leaf(LK::Chip, r, 0, 0, None, th.surf, "+N chip");
        f.text(
            &t,
            rx + cw / 2.0,
            14.0 + (18.0 - fs * 1.1) / 2.0,
            fs,
            Font::MonoMedium,
            th.text2,
            1,
            0,
            cid,
            th.surf,
        );
        rx -= 8.0;
    }
    if let Some((opts, sel)) = sp.header {
        let sw = opts
            .iter()
            .map(|o| f.tw(o, f.fsz(11.0), Font::MonoMedium))
            .fold(0.0, f32::max)
            + 8.0;
        let sw = sw.max(34.0);
        rx -= sw * opts.len() as f32;
        f.seg(rx, 14.0, opts, sel, 18.0, Some(sw), None, None, None);
        rx -= 8.0;
    }
    if sp.body == "seq" || sp.body == "drum" {
        #[allow(clippy::type_complexity)]
        let segs: Vec<(&str, Vec<&str>, usize, Option<Color32>, Option<usize>)> =
            if sp.body == "seq" {
                vec![
                    ("EDIT", vec!["A", "B", "C", "D"], 0, None, None),
                    ("PLAY", vec!["A", "B", "C", "D"], 0, Some(th.gate), None),
                    ("PAGE", vec!["1", "2", "3", "4"], 0, None, Some(1)),
                ]
            } else {
                vec![
                    ("PAT", vec!["A", "B", "C", "D"], 0, None, None),
                    ("VEL", vec!["30", "55", "78", "100"], 3, None, None),
                    ("CHANCE", vec!["100", "75", "50", "25"], 0, None, None),
                ]
            };
        for (cap, opts, sel, acc, pl) in segs.into_iter().rev() {
            let sw = opts
                .iter()
                .map(|o| f.tw(o, 11.0, Font::MonoMedium))
                .fold(0.0, f32::max)
                + 12.0;
            let cw = f.tw(cap, 11.0, Font::MonoMedium) + 8.0;
            rx -= cw + sw * opts.len() as f32;
            f.seg(rx, 14.0, &opts, sel, 18.0, Some(sw), Some(cap), acc, pl);
            rx -= 14.0;
        }
    }
    // title + caption
    let tsize = if compact { 14.0f32.max(10.5 / z) } else { 14.0 };
    let max_title = rx - M
        + if (rx - (w - M)).abs() < 0.01 {
            0.0
        } else {
            6.0
        };
    let tcol = th.title_col(sp.fam);
    match [sp.title, sp.short]
        .into_iter()
        .find(|t| f.cond_w(t, tsize) <= max_title)
    {
        Some(t) => f.cond_title(t, M, 12.0, tsize, tcol),
        None => f.drop_text("title", sp.title, true),
    }
    if !compact {
        let cap = format!("{} · #{}", sp.id, sp.num);
        f.text(&cap, M, 32.0, 11.0, Font::Mono, th.text2, 0, 0, 0, th.surf);
    }

    // display well
    let has_well = !sp.well.is_empty();
    if has_well {
        let (x, y, ww, hh) = (M, WELL_Y, w - 2.0 * M, WELL_H);
        f.well(x, y, ww, hh);
        let trace = th.accent;
        let soft = al(th.accent, if th.dark { 40 } else { 56 });
        if sp.well != "meters" {
            f.gridlines(x, y, ww, hh, 8, 4);
        }
        match sp.well {
            "saw" => {
                let n = 3;
                let mut pts = vec![];
                for i in 0..n {
                    let x0 = x + 10.0 + i as f32 * (ww - 20.0) / n as f32;
                    let x1 = x0 + (ww - 20.0) / n as f32;
                    pts.push((x0, y + hh - 12.0));
                    pts.push((x1, y + 12.0));
                }
                pts.push((x + ww - 10.0, y + hh - 12.0));
                if th.dark {
                    f.poly_line(&pts, 6.0, al(trace, 36));
                }
                f.poly_line(&pts, 2.0, trace);
            }
            "svf" => {
                let mut pts = vec![];
                for i in 0..=60 {
                    let fr = i as f32 / 60.0;
                    let fc = 0.58f32;
                    let q = 2.8f32;
                    let g =
                        1.0 / ((1.0 - (fr / fc).powi(2)).powi(2) + ((fr / fc) / q).powi(2)).sqrt();
                    let yy = y + hh - 10.0 - (g * 0.55).clamp(0.0, 1.25) * (hh - 22.0);
                    let yy = if fr > fc {
                        (yy + (fr - fc) * 90.0).min(y + hh - 6.0)
                    } else {
                        yy
                    };
                    pts.push((x + 8.0 + fr * (ww - 16.0), yy));
                }
                let mut fillp = pts.clone();
                fillp.push((x + ww - 8.0, y + hh - 6.0));
                fillp.push((x + 8.0, y + hh - 6.0));
                // fill as vertical strips (convex_polygon needs convex shapes)
                for k in pts.windows(2) {
                    let (a, b) = (k[0], k[1]);
                    f.poly_fill(&[a, b, (b.0, y + hh - 6.0), (a.0, y + hh - 6.0)], soft);
                }
                if th.dark {
                    f.poly_line(&pts, 6.0, al(trace, 36));
                }
                f.poly_line(&pts, 2.0, trace);
                let cx = x + 8.0 + 0.58 * (ww - 16.0);
                f.line(
                    (cx, y + 6.0),
                    (cx, y + hh - 6.0),
                    1.0,
                    al(th.well_ink(), 90),
                );
            }
            "adsr" => {
                let (b, t) = (y + hh - 10.0, y + 10.0);
                let xs = [x + 10.0, x + 30.0, x + 62.0, x + ww - 54.0, x + ww - 10.0];
                let sy = t + (b - t) * (1.0 - 0.62);
                let pts = [
                    (xs[0], b),
                    (xs[1], t),
                    (xs[1] + 10.0, sy),
                    (xs[2], sy),
                    (xs[3], sy),
                    (xs[3] + 14.0, b - 4.0),
                    (xs[4], b),
                ];
                for k in pts.windows(2) {
                    f.poly_fill(&[k[0], k[1], (k[1].0, b), (k[0].0, b)], soft);
                }
                if th.dark {
                    f.poly_line(&pts, 6.0, al(trace, 36));
                }
                f.poly_line(&pts, 2.0, trace);
                f.line(
                    (xs[2] + 30.0, y + 5.0),
                    (xs[2] + 30.0, y + hh - 5.0),
                    1.0,
                    al(th.well_ink(), 110),
                );
                f.circle((xs[2] + 30.0, sy), 3.6, trace);
            }
            _ => {
                let n = 4;
                let pitch = (w - 2.0 * M) / n as f32;
                let lvs = [0.72, 0.55, 0.9, 0.3];
                for (i, lv) in lvs.iter().enumerate() {
                    let cx = M + pitch * (i as f32 + 0.5);
                    let segs = 12;
                    let sh = (hh - 14.0) / segs as f32;
                    for s in 0..segs {
                        let sy = y + hh - 8.0 - (s + 1) as f32 * sh;
                        let on = (s as f32 / segs as f32) < *lv;
                        let col = if s >= 10 {
                            hex("#E0533B")
                        } else if s >= 8 {
                            trace
                        } else {
                            th.cv
                        };
                        let r = f.rc(cx - 5.0, sy + 1.0, 10.0, sh - 1.5);
                        f.fill(r, 1.0, al(col, if on { 240 } else { 36 }));
                    }
                }
            }
        }
    }

    // control rows
    let top = if has_well { CTRL_TOP } else { WELL_Y };
    if !sp.rows.is_empty() || sp.sel_row.is_some() {
        let mut heights: Vec<f32> = sp
            .rows
            .iter()
            .map(|r| {
                r.iter()
                    .map(|k| 2.0 * knob_r(k.size, r.len()) + 34.0)
                    .fold(0.0, f32::max)
            })
            .collect();
        if sp.sel_row.is_some() {
            heights.push(20.0);
        }
        let total: f32 = heights.iter().sum::<f32>() + 8.0 * (heights.len() as f32 - 1.0);
        let mut y = top + ((CTRL_BOT - top - total) / 2.0).max(0.0);
        for (ri, row) in sp.rows.iter().enumerate() {
            let n = row.len();
            let pitch = (w - 2.0 * M) / n as f32;
            let rmax = row.iter().map(|k| knob_r(k.size, n)).fold(0.0, f32::max);
            let cy = y + 14.0 + 3.5 + rmax + lift;
            for (i, k) in row.iter().enumerate() {
                f.knob_cell(
                    k.l,
                    k.s,
                    Some(k.val),
                    k.v,
                    M + pitch * (i as f32 + 0.5),
                    cy,
                    knob_r(k.size, n),
                    pitch,
                );
            }
            y += heights[ri] + 8.0;
        }
        if let Some((opts, sel)) = sp.sel_row {
            let sw = (w - 2.0 * M) / opts.len() as f32;
            f.seg(M, y, opts, sel, 20.0, Some(sw), None, None, None);
        }
    }

    match sp.body {
        "seq" => paint_seq(f, bot, lift),
        "drum" => paint_drum(f, bot),
        "fm6" => paint_fm6(f, bot, lift),
        _ => {}
    }
    paint_tray(f, sp);
}

fn paint_seq(f: &mut Face, bot: f32, lift: f32) {
    let th = f.th;
    let (w, wide) = (f.w, f.wide());
    let compact = f.compact();
    let steps = [
        ("+0", 1.0, true, 100),
        ("+7", 0.55, true, 100),
        ("+12", 0.78, false, 100),
        ("+3", 0.78, true, 75),
        ("-5", 1.0, true, 100),
        ("+7", 0.3, true, 50),
        ("+10", 0.55, false, 100),
        ("+12", 0.78, true, 100),
    ];
    let playing = 4usize;
    let lx = M;
    let lw = 112.0;
    let cy_a = 52.0 + 14.0 + 3.5 + 16.0 + lift + 10.0 + if wide { 18.0 } else { 0.0 };
    f.knob_cell(
        "Length",
        Some("Len"),
        Some("32"),
        1.0,
        lx + lw / 2.0,
        cy_a,
        16.0,
        lw,
    );
    f.knob_cell(
        "Gate length",
        Some("Gate"),
        Some("50 %"),
        0.5,
        lx + lw / 2.0,
        cy_a + 82.0,
        16.0,
        lw,
    );
    f.line(
        (lx + lw + 8.0, 56.0),
        (lx + lw + 8.0, bot - 4.0),
        1.0,
        th.line2,
    );
    let rl = lx + lw + 16.0;
    let col_x0 = rl + 44.0;
    let cw = (w - M - col_x0) / 8.0;
    let dy = if wide { 18.0 } else { 0.0 };
    let (knob_cy, gate_y, vel_y, vel_h, pr_y) =
        (104.0 + dy, 145.0 + dy, 170.0 + dy, 24.0, 201.0 + dy);
    let lsz = f.fsz(11.0);
    for (n, yy) in [
        ("Pitch", knob_cy - lsz * 0.55),
        ("Gate", gate_y + 9.0 - lsz * 0.55),
        ("Vel", vel_y + vel_h / 2.0 - lsz * 0.55),
        ("Prob", pr_y),
    ] {
        if !compact || f.tw(n, lsz, Font::MonoMedium) <= 40.0 {
            f.text(n, rl, yy, lsz, Font::MonoMedium, th.text2, 0, 0, 0, th.surf);
        }
    }
    for (i, (p, v, g, pr)) in steps.iter().enumerate() {
        let cx = col_x0 + cw * (i as f32 + 0.5);
        let on = i == playing;
        f.circle(
            (cx - 10.0, 62.0 + dy),
            3.2,
            if on { th.gate } else { th.line2 },
        );
        let led = f.rc(cx - 13.2, 58.8 + dy, 6.4, 6.4);
        f.leaf(LK::Pad, led, 0, 0, None, th.surf, "led");
        f.text(
            &(i + 1).to_string(),
            cx - 3.0,
            56.0 + dy,
            f.fsz(11.0),
            Font::MonoMedium,
            th.text,
            0,
            0,
            0,
            th.surf,
        );
        let pv: i32 = p.parse().unwrap_or(0);
        let kid = f.knob(cx, knob_cy, 16.0, (pv as f32 + 24.0) / 48.0, None, "");
        if !compact {
            f.text(
                &format!("{p} st"),
                cx,
                knob_cy + 16.0 + 3.5 + 1.25 + 3.0,
                11.0,
                Font::Mono,
                th.text2,
                1,
                kid,
                0,
                th.surf,
            );
        }
        let (gw, gh) = (28.0, 18.0);
        let gr = f.rc(cx - gw / 2.0, gate_y, gw, gh);
        f.fill(gr, 3.0, if *g { th.gate } else { th.inset });
        if !*g {
            f.stroke_r(gr, 3.0, Stroke::new(1.0, th.line2));
        }
        f.leaf(LK::Pad, gr, 0, 0, None, th.surf, "gate pad");
        let vr = f.rc(cx - 13.0, vel_y, 26.0, vel_h);
        f.fill(vr, 3.0, th.inset);
        f.stroke_r(vr, 3.0, Stroke::new(1.0, th.line));
        let fh = (vel_h - 6.0) * v;
        let br = f.rc(cx - 10.0, vel_y + vel_h - 3.0 - fh, 20.0, fh);
        f.fill(br, 1.5, al(th.gate, (115.0 + 140.0 * v) as u8));
        f.leaf(LK::Pad, vr, 0, 0, None, th.surf, "vel lane");
        let pc = if *pr < 100 { th.text } else { th.text2 };
        f.text(
            &pr.to_string(),
            cx,
            pr_y,
            f.fsz(11.0),
            Font::MonoMedium,
            pc,
            1,
            0,
            0,
            th.surf,
        );
        if on {
            let r = f.rc(cx - cw / 2.0 + 3.0, 50.0 + dy, cw - 6.0, 178.0);
            f.stroke_r(r, 4.0, Stroke::new(1.0, al(th.gate, 120)));
        }
    }
}

fn paint_drum(f: &mut Face, bot: f32) {
    let th = f.th;
    let (x, y, ww, hh) = (M, 52.0, f.w - 2.0 * M, bot - 52.0);
    f.well(x, y, ww, hh);
    let names = [
        "Kick", "Snare", "Clap", "Chat", "Ohat", "Tom", "Perc 1", "Perc 2",
    ];
    let sh = ["BD", "SD", "CP", "CH", "OH", "TM", "P1", "P2"];
    let lens = [16, 16, 16, 16, 16, 16, 16, 12];
    let pat = [
        "4000400040004030",
        "0000400000004000",
        "0000000200000002",
        "2121212121212121",
        "0020002000200020",
        "0000000000003020",
        "1000010000100001",
        "0300030003000",
    ];
    let chn = [
        "0000000000000000",
        "0000000000000001",
        "0000000300000000",
        "0002000200020002",
        "0000000000000000",
        "0000000000000000",
        "0100000100000100",
        "000000000000",
    ];
    let name_w = 40.0;
    let (gx, gw, gap, grp) = (x + name_w + 6.0, ww - name_w - 12.0, 4.0, 6.0);
    let cwid = (gw - 15.0 * gap - 3.0 * (grp - gap)) / 16.0;
    let rh = (hh - 10.0) / 8.0;
    let lsz = f.fsz(11.0);
    let play = 4usize;
    for r in 0..8 {
        let ry = y + 5.0 + r as f32 * rh;
        let lab = if f.compact() {
            (f.tw(sh[r], lsz, Font::MonoMedium) <= name_w - 4.0).then(|| sh[r])
        } else if f.tw(names[r], lsz, Font::MonoMedium) <= name_w - 4.0 {
            Some(names[r])
        } else {
            Some(sh[r])
        };
        if let Some(l) = lab {
            f.text(
                l,
                x + 6.0,
                ry + (rh - lsz * 1.1) / 2.0,
                lsz,
                Font::MonoMedium,
                th.well_ink(),
                0,
                0,
                0,
                th.inset,
            );
        }
        for s in 0..16 {
            let cx = gx + s as f32 * (cwid + gap) + (s / 4) as f32 * (grp - gap);
            let (cy, chh) = (ry + 2.0, rh - 4.0);
            let cr = f.rc(cx, cy, cwid, chh);
            if s >= lens[r] {
                f.stroke_r(cr, 2.0, Stroke::new(1.0, al(th.well_ink(), 24)));
                continue;
            }
            f.fill(cr, 2.0, mixc(th.inset, th.well_ink(), 0.1));
            if s == 0 {
                f.leaf(LK::Pad, cr, 0, 0, None, th.inset, "first cell");
            }
            let lvl = (pat[r].as_bytes().get(s).copied().unwrap_or(b'0') - b'0') as usize;
            let chance = (chn[r].as_bytes().get(s).copied().unwrap_or(b'0') - b'0') as usize;
            if lvl > 0 {
                let fr = [0.0, 0.3, 0.55, 0.78, 1.0][lvl];
                let fh = (chh - 2.0) * fr;
                let br = f.rc(cx + 1.0, cy + chh - 1.0 - fh, cwid - 2.0, fh);
                f.fill(br, 1.5, al(th.accent, (140.0 + 115.0 * fr) as u8));
                for k in 0..chance {
                    f.circle((cx + cwid - 4.0 - k as f32 * 4.0, cy + 4.0), 1.4, th.inset);
                }
            }
        }
    }
    let px = gx + play as f32 * (cwid + gap) + (play / 4) as f32 * (grp - gap);
    let pr = f.rc(px - 1.5, y + 3.0, cwid + 3.0, hh - 6.0);
    f.stroke_r(pr, 3.0, Stroke::new(1.2, al(th.well_ink(), 140)));
}

fn paint_fm6(f: &mut Face, bot: f32, lift: f32) {
    let th = f.th;
    let (lw, x) = (180.0, M);
    f.well(x, WELL_Y, lw, WELL_H);
    let ay = WELL_Y + WELL_H / 2.0 + 6.0;
    f.text(
        "ALG 1",
        x + 8.0,
        WELL_Y + 6.0,
        f.fsz(11.0),
        Font::MonoMedium,
        th.well_ink(),
        0,
        0,
        0,
        th.inset,
    );
    for i in 0..6 {
        let nx = x + 18.0 + i as f32 * 29.0;
        let op = 6 - i;
        let carrier = op == 1;
        let r = f.rc(nx - 9.0, ay - 9.0, 18.0, 18.0);
        if carrier {
            f.fill(r, 3.0, th.accent);
        } else {
            f.stroke_r(r, 3.0, Stroke::new(f.lw(1.3), al(th.well_ink(), 180)));
        }
        if i < 5 {
            f.line(
                (nx + 10.0, ay),
                (nx + 19.0, ay),
                1.3,
                al(th.well_ink(), 150),
            );
            f.poly_line(
                &[
                    (nx + 16.0, ay - 2.5),
                    (nx + 19.5, ay),
                    (nx + 16.0, ay + 2.5),
                ],
                1.3,
                al(th.well_ink(), 150),
            );
        }
        if !f.compact() {
            let pos = f.pt(nx, ay);
            f.p.text(
                pos,
                Align2::CENTER_CENTER,
                op.to_string(),
                fnt(Font::MonoMedium, 11.0 * f.z),
                if carrier { th.on_accent } else { th.well_ink() },
            );
        }
    }
    f.poly_line(
        &[
            (x + 14.0, ay - 9.0),
            (x + 14.0, ay - 16.0),
            (x + 22.0, ay - 16.0),
            (x + 22.0, ay - 9.0),
        ],
        1.3,
        th.accent,
    );
    let gk = [
        ("Frequency", Some("Freq"), "262 Hz", 0.56),
        ("Fine", None, "+0 ct", 0.5),
        ("Feedback", Some("Fdbk"), "0.20", 0.2),
        ("Index", None, "1.10", 0.55),
    ];
    let pitch = lw / 4.0;
    let cy = CTRL_TOP + 14.0 + 3.5 + 16.0 + 20.0 + lift;
    for (i, (l, s, v, n)) in gk.iter().enumerate() {
        f.knob_cell(
            l,
            *s,
            Some(v),
            *n,
            x + pitch * (i as f32 + 0.5),
            cy,
            16.0,
            pitch,
        );
    }
    f.seg(
        x,
        240.0,
        &["2×", "4×"],
        0,
        20.0,
        Some(44.0),
        Some("OVERSAMPLE"),
        None,
        None,
    );
    f.line(
        (x + lw + 8.0, 56.0),
        (x + lw + 8.0, bot - 4.0),
        1.0,
        th.line2,
    );
    let ox = x + lw + 16.0;
    let ow = f.w - M - ox;
    let gap = 5.0;
    let cw = (ow - 5.0 * gap) / 6.0;
    let ratios = ["×1.00", "×1.00", "×2.00", "×3.00", "×0.50", "×7.00"];
    let lvs = [0.9, 0.6, 0.45, 0.7, 0.3, 0.5];
    for i in 0..6 {
        let cx0 = ox + i as f32 * (cw + gap);
        let (cy0, chh) = (52.0, bot - 52.0);
        let cr = f.rc(cx0, cy0, cw, chh);
        f.stroke_r(cr, 5.0, Stroke::new(1.0, th.line2));
        f.conts.push(cr.translate(-f.o.to_vec2()));
        let fs = f.fsz(11.0);
        f.text(
            &format!("OP {}", i + 1),
            cx0 + 7.0,
            cy0 + 6.0,
            fs,
            Font::MonoMedium,
            th.text,
            0,
            0,
            0,
            th.surf,
        );
        if i == 0 {
            f.circle((cx0 + cw - 10.0, cy0 + 12.0), 3.4, th.accent);
        }
        let fy = cy0 + 26.0f32.max(6.0 + fs * 1.1 + 4.0);
        let fw = cw - 12.0;
        let fr = f.rc(cx0 + 6.0, fy, fw, 20.0);
        f.fill(fr, 4.0, th.track);
        f.stroke_r(fr, 4.0, Stroke::new(1.0, th.line2));
        let sid = f.leaf(LK::Seg, fr, 0, 0, None, th.surf, "ratio field");
        f.text(
            ratios[i],
            cx0 + 11.0,
            fy + (20.0 - fs * 1.1) / 2.0,
            fs,
            Font::MonoMedium,
            th.track_ink,
            0,
            0,
            sid,
            th.track,
        );
        f.poly_line(
            &[
                (cx0 + 6.0 + fw - 12.0, fy + 8.0),
                (cx0 + 6.0 + fw - 8.5, fy + 12.0),
                (cx0 + 6.0 + fw - 5.0, fy + 8.0),
            ],
            1.4,
            th.track_ink,
        );
        let kcy = cy0 + 60.0 + 14.0 + 3.5 + 16.0 + lift;
        f.knob_cell(
            "Level",
            Some("Lvl"),
            Some(&format!("{}", (lvs[i] * 99.0f32).round())),
            lvs[i],
            cx0 + cw / 2.0,
            kcy,
            16.0,
            cw,
        );
        let (ex, ey, ew, eh) = (cx0 + 6.0, cy0 + chh - 62.0, cw - 12.0, 56.0);
        let er = f.rc(ex, ey, ew, eh);
        f.fill(er, 3.0, th.inset);
        f.stroke_r(er, 3.0, Stroke::new(1.0, th.line));
        let (b, t) = (ey + eh - 6.0, ey + 6.0);
        let s = t + (b - t) * (0.3 + 0.1 * (i % 3) as f32);
        let pts = [
            (ex + 4.0, b),
            (ex + 10.0 + i as f32, t),
            (ex + 22.0, s),
            (ex + ew - 16.0, s),
            (ex + ew - 4.0, b),
        ];
        f.poly_line(&pts, 1.6, th.accent);
        for (k, px) in [ex + 10.0 + i as f32, ex + 22.0, ex + ew - 16.0]
            .iter()
            .enumerate()
        {
            f.circle((*px, if k > 0 { s } else { t }), 2.2, th.accent);
        }
        f.leaf(LK::Pad, er, 0, 0, None, th.surf, "env curve");
    }
}

fn paint_tray(f: &mut Face, sp: &Spec) {
    let th = f.th;
    let (w, compact) = (f.w, f.compact());
    let ls = f.fsz(11.0);
    if f.wide() {
        let (x, ww) = (8.0, w - 16.0);
        let y = PLATE_Y - 2.0;
        let h = FACE_H - 10.0 - y;
        let pitch = 56.0;
        let out_w = sp.outs.len() as f32 * pitch + 8.0;
        let px = x + ww - out_w;
        let tr = f.rc(x, y, ww, h);
        f.stroke_r(tr, 5.0, Stroke::new(1.0, th.line2));
        f.conts.push(tr.translate(-f.o.to_vec2()));
        let pr = f.rc(px, y, out_w + 0.0, h);
        f.fill(pr, 5.0, th.raised);
        f.stroke_r(pr, 5.0, Stroke::new(1.0, th.line2));
        let put = |f: &mut Face, p: &Port, cx: f32, bg: Color32| {
            let jid = f.jack(cx, OUT_JACK, th.sig(p.1), p.2, false, false);
            let lab0 = if compact {
                f.fit(p.3.unwrap_or(p.0), None, ls, pitch - 4.0, Font::SansMedium)
            } else {
                f.fit(p.0, p.3, ls, pitch - 4.0, Font::SansMedium)
            };
            let room = (OUT_JACK - 11.0 - 3.0) - (y + 2.0);
            match lab0.filter(|_| ls * 1.1 <= room) {
                Some(l) => f.text(
                    &l,
                    cx,
                    OUT_JACK - 11.0 - 3.0 - ls * 1.1,
                    ls,
                    Font::SansMedium,
                    th.text,
                    1,
                    jid,
                    0,
                    bg,
                ),
                None => f.drop_text("port label", p.0, !compact),
            }
        };
        for (i, p) in sp.ins.iter().enumerate() {
            put(f, p, M + pitch * (i as f32 + 0.5), th.surf);
        }
        for (i, p) in sp.outs.iter().enumerate() {
            put(f, p, px + 4.0 + pitch * (i as f32 + 0.5), th.raised);
        }
        return;
    }
    let (x, ww, y, h) = (8.0, w - 16.0, TRAY_Y, TRAY_H);
    let tr = f.rc(x, y, ww, h);
    f.stroke_r(tr, 5.0, Stroke::new(1.0, th.line2));
    f.conts.push(tr.translate(-f.o.to_vec2()));
    let plate = f.rc(x, PLATE_Y, ww, y + h - PLATE_Y);
    f.fill(plate, 5.0, th.raised);
    f.stroke_r(plate, 5.0, Stroke::new(1.0, th.line2));
    let usable = w - 2.0 * M;
    let n = sp.ins.len().max(sp.outs.len()).max(3);
    let pitch = 72.0f32.min(usable / n as f32);
    for (ports, cy, bg) in [(&sp.ins, IN_JACK, th.surf), (&sp.outs, OUT_JACK, th.raised)] {
        for (i, p) in ports.iter().enumerate() {
            let cx = M + pitch * (i as f32 + 0.5);
            let jid = f.jack(cx, cy, th.sig(p.1), p.2, false, false);
            let lab0 = if compact {
                f.fit(p.3.unwrap_or(p.0), None, ls, pitch - 4.0, Font::SansMedium)
            } else {
                f.fit(p.0, p.3, ls, pitch - 4.0, Font::SansMedium)
            };
            let above = if cy == IN_JACK {
                TRAY_Y + 2.0
            } else {
                IN_JACK + 11.0 + if compact { 2.0 / f.z } else { 3.0 }
            };
            let room = (cy - 11.0 - 3.0) - above;
            match lab0.filter(|_| ls * 1.1 <= room) {
                Some(l) => f.text(
                    &l,
                    cx,
                    cy - 11.0 - 3.0 - ls * 1.1,
                    ls,
                    Font::SansMedium,
                    th.text,
                    1,
                    jid,
                    0,
                    bg,
                ),
                None => f.drop_text("port label", p.0, !compact),
            }
        }
    }
}

// ---------------------------------------------------------------- checker (G0 rules on the mock's rects)

#[derive(Default)]
struct Tally {
    leaves: usize,
    dropped: usize,
    errors: usize,
    warnings: usize,
    lines: Vec<String>,
}

fn dist(a: Rect, b: Rect) -> f32 {
    let dx = (a.min.x - b.max.x).max(b.min.x - a.max.x).max(0.0);
    let dy = (a.min.y - b.max.y).max(b.min.y - a.max.y).max(0.0);
    dx.hypot(dy)
}
fn overlaps(a: Rect, b: Rect) -> bool {
    a.min.x < b.max.x - 0.01
        && b.min.x < a.max.x - 0.01
        && a.min.y < b.max.y - 0.01
        && b.min.y < a.max.y - 0.01
}

fn check(f: &Face, t: &mut Tally) {
    let full = f.z >= 0.864;
    t.leaves += f.leaves.len();
    t.dropped += f.drops.len();
    let (g_any, g_own) = if full { (3.0, 2.0) } else { (2.0, 1.5) };
    let face = Rect::from_min_size(Pos2::ZERO, vec2(f.w, FACE_H) * f.z);
    let err = |t: &mut Tally, s: String| {
        t.errors += 1;
        t.lines.push(format!("error {} z{}: {s}", f.name, f.z));
    };
    for (i, a) in f.leaves.iter().enumerate() {
        for b in &f.leaves[i + 1..] {
            if a.inside == b.id || b.inside == a.id {
                continue;
            }
            let own = a.own != 0 && (a.own == b.id || b.own == a.id || a.own == b.own);
            let min = if own { g_own } else { g_any };
            if overlaps(a.r, b.r) {
                err(t, format!("overlap '{}' / '{}'", a.what, b.what));
            } else if dist(a.r, b.r) < min {
                err(
                    t,
                    format!("gap {:.1}px '{}' / '{}'", dist(a.r, b.r), a.what, b.what),
                );
            }
        }
        for c in &f.conts {
            if overlaps(a.r, *c) && !c.expand(0.6).contains_rect(a.r) {
                err(t, format!("'{}' straddles a well/tray/card", a.what));
            }
        }
        if !face.contains_rect(a.r) {
            err(t, format!("'{}' outside face", a.what));
        } else if (a.r.min.x - face.min.x)
            .min(face.max.x - a.r.max.x)
            .min(a.r.min.y - face.min.y)
            .min(face.max.y - a.r.max.y)
            < 4.0
        {
            err(t, format!("edge < 4px '{}'", a.what));
        }
        if let Some(fg) = a.fg {
            let c = contrast(fg, a.bg);
            if c < 4.5 {
                err(t, format!("contrast {c:.2}:1 '{}'", a.what));
            }
        }
    }
    for (is_err, s) in &f.drops {
        if *is_err {
            err(t, s.clone());
        }
    }
}

// ---------------------------------------------------------------- parts sheet

fn paint_parts(p: &Painter, th: Th, o: Pos2) {
    let mut f = Face {
        p,
        th,
        o,
        z: 1.0,
        w: 1000.0,
        name: "parts",
        fam: "fx",
        leaves: vec![],
        conts: vec![],
        drops: vec![],
        next: 0,
    };
    let cap = |f: &mut Face, s: &str, x: f32, y: f32| {
        f.p.text(
            f.pt(x, y),
            Align2::CENTER_CENTER,
            s,
            fnt(Font::Mono, 11.0),
            th.text2,
        )
    };
    let panel = f.rc(0.0, 0.0, 1000.0, 740.0);
    f.fill(panel, 3.0, th.surf);
    f.stroke_r(panel, 3.0, Stroke::new(1.0, th.edge));
    // knobs
    cond_text(
        f.p,
        f.pt(16.0, 20.0),
        "KNOB  S 16 · M 20 · L 24 · states",
        14.0,
        th.title_col("fx"),
    );
    let mut x = 50.0;
    for (r, st, v, m) in [
        (16.0, "", 0.3, None),
        (20.0, "", 0.6, None),
        (24.0, "", 0.8, None),
        (20.0, "hover", 0.5, None),
        (20.0, "focus", 0.5, None),
        (20.0, "mod", 0.4, Some((0.7, 0.62))),
        (20.0, "disabled", 0.5, None),
    ] {
        f.knob(x, 80.0, r, v, m, st);
        let nm = if st.is_empty() {
            format!("r{r}")
        } else {
            st.to_string()
        };
        cap(&mut f, &nm, x, 125.0);
        x += 90.0;
    }
    // selector + toggle + chip
    cond_text(
        f.p,
        f.pt(16.0, 160.0),
        "SELECTOR · TOGGLE · CHIP",
        14.0,
        th.title_col("fx"),
    );
    f.seg(
        20.0,
        180.0,
        &["SIN", "TRI", "SAW", "SQR"],
        2,
        20.0,
        Some(52.0),
        None,
        None,
        None,
    );
    f.seg(
        260.0,
        180.0,
        &["A", "B", "C", "D"],
        0,
        18.0,
        Some(26.0),
        Some("PLAY"),
        Some(th.gate),
        Some(1),
    );
    let tg = f.rc(470.0, 180.0, 36.0, 20.0);
    f.fill(tg, 10.0, th.track);
    f.stroke_r(tg, 10.0, Stroke::new(1.0, th.line2));
    f.circle((470.0 + 26.0, 190.0), 7.0, th.accent);
    let tg2 = f.rc(520.0, 180.0, 36.0, 20.0);
    f.fill(tg2, 10.0, th.track);
    f.stroke_r(tg2, 10.0, Stroke::new(1.0, th.line2));
    f.circle((520.0 + 10.0, 190.0), 7.0, th.track_ink);
    let ch = f.rc(580.0, 181.0, 36.0, 18.0);
    f.stroke_r(ch, 4.0, Stroke::new(1.0, th.line2));
    f.p.text(
        ch.center(),
        Align2::CENTER_CENTER,
        "+7",
        fnt(Font::MonoMedium, 11.0),
        th.text2,
    );
    // jacks
    cond_text(
        f.p,
        f.pt(16.0, 230.0),
        "JACK  audio · cv · gate · pitch · stereo · patched · hover",
        14.0,
        th.title_col("fx"),
    );
    let mut x = 40.0;
    for (sig, ch, plug, hov, name) in [
        ("audio", 1, false, false, "audio"),
        ("cv", 1, false, false, "cv"),
        ("gate", 1, false, false, "gate"),
        ("pitch", 1, false, false, "pitch"),
        ("audio", 2, false, false, "stereo"),
        ("audio", 1, true, false, "patched"),
        ("cv", 1, false, true, "hover"),
    ] {
        f.jack(x, 275.0, th.sig(sig), ch, plug, hov);
        cap(&mut f, name, x, 305.0);
        x += 80.0;
    }
    // cables
    cond_text(
        f.p,
        f.pt(16.0, 340.0),
        "CABLE  rest · lit · stereo · functional (step beads)",
        14.0,
        th.title_col("fx"),
    );
    let cable =
        |f: &Face, x0: f32, y0: f32, col: Color32, w: f32, stereo: bool, lit: bool, beads: bool| {
            let a = f.pt(x0, y0);
            let b = f.pt(x0 + 200.0, y0 + 10.0);
            let c1 = a + vec2(20.0, 60.0);
            let c2 = b + vec2(-20.0, 60.0);
            let mk = |wd: f32, c: Color32| {
                Shape::CubicBezier(egui::epaint::CubicBezierShape::from_points_stroke(
                    [a, c1, c2, b],
                    false,
                    Color32::TRANSPARENT,
                    Stroke::new(wd, c),
                ))
            };
            if stereo {
                f.p.add(mk(8.0, hex("#0B0C0D")));
                f.p.add(mk(6.4, col));
                f.p.add(mk(1.6, hex("#0B0C0D")));
            } else {
                if th.dark {
                    f.p.add(mk(w + 5.0, al(col, 40)));
                }
                f.p.add(mk(
                    w,
                    if lit {
                        mixc(col, Color32::WHITE, 0.25)
                    } else {
                        col
                    },
                ));
                if lit {
                    f.p.add(mk(1.0, al(Color32::WHITE, 120)));
                }
            }
            if beads {
                for i in 0..8 {
                    let t = (i as f32 + 0.5) / 8.0;
                    let pt = a.to_vec2() * (1.0 - t).powi(3)
                        + c1.to_vec2() * 3.0 * (1.0 - t).powi(2) * t
                        + c2.to_vec2() * 3.0 * (1.0 - t) * t * t
                        + b.to_vec2() * t.powi(3);
                    f.p.circle_filled(
                        pos2(pt.x, pt.y),
                        4.0,
                        if i % 3 == 1 { th.surf } else { col },
                    );
                    f.p.circle_stroke(pos2(pt.x, pt.y), 4.0, Stroke::new(1.5, col));
                }
            }
        };
    cable(&f, 20.0, 360.0, th.audio, 4.0, false, false, false);
    cable(&f, 260.0, 360.0, th.cv, 4.0, false, true, false);
    cable(&f, 500.0, 360.0, th.audio, 4.0, true, false, false);
    cable(&f, 740.0, 360.0, th.gate, 4.0, false, false, true);
    // family marker / stripe
    cond_text(
        f.p,
        f.pt(16.0, 470.0),
        "FAMILY MARKER  title colour (line styles) · 3 px stripe (solid styles)",
        14.0,
        th.title_col("fx"),
    );
    for (i, fam) in ["osc", "filter", "env", "timing", "amp", "fx"]
        .iter()
        .enumerate()
    {
        let x = 20.0 + i as f32 * 160.0;
        let r = f.rc(x, 490.0, 140.0, 60.0);
        f.fill(r, 3.0, th.surf);
        f.stroke_r(r, 3.0, Stroke::new(1.0, th.edge));
        cond_text(f.p, f.pt(x + 10.0, 506.0), fam, 14.0, th.title_col(fam));
        let stripe = f.rc(x + 1.0, 491.0, 138.0, 3.0);
        let hue = th.title_col(fam);
        f.fill(stripe, 1.0, hue);
        f.p.text(
            f.pt(x + 10.0, 530.0),
            Align2::LEFT_CENTER,
            "title / stripe",
            fnt(Font::Mono, 11.0),
            th.text2,
        );
    }
    // trays
    cond_text(
        f.p,
        f.pt(16.0, 575.0),
        "TRAY  two_row (< 480 px) · single_row_wide (>= 480 px)",
        14.0,
        th.title_col("fx"),
    );
    let t2 = f.rc(20.0, 590.0, 200.0, 86.0);
    f.stroke_r(t2, 5.0, Stroke::new(1.0, th.line2));
    let pl = f.rc(20.0, 634.0, 200.0, 42.0);
    f.fill(pl, 5.0, th.raised);
    f.stroke_r(pl, 5.0, Stroke::new(1.0, th.line2));
    for i in 0..3 {
        f.jack(
            50.0 + i as f32 * 60.0,
            615.0,
            th.sig("audio"),
            1,
            false,
            false,
        );
        f.jack(
            50.0 + i as f32 * 60.0,
            656.0,
            th.sig("gate"),
            1,
            false,
            false,
        );
    }
    let tw_ = f.rc(260.0, 620.0, 400.0, 50.0);
    f.stroke_r(tw_, 5.0, Stroke::new(1.0, th.line2));
    let plw = f.rc(500.0, 620.0, 160.0, 50.0);
    f.fill(plw, 5.0, th.raised);
    f.stroke_r(plw, 5.0, Stroke::new(1.0, th.line2));
    for i in 0..3 {
        f.jack(
            295.0 + i as f32 * 56.0,
            645.0,
            th.sig("gate"),
            1,
            false,
            false,
        );
    }
    for i in 0..2 {
        f.jack(
            535.0 + i as f32 * 56.0,
            645.0,
            th.sig("audio"),
            1,
            false,
            false,
        );
    }
    // drum cell
    cond_text(
        f.p,
        f.pt(700.0, 575.0),
        "DRUM CELL  off · 4 velocities · 3 chances",
        14.0,
        th.title_col("fx"),
    );
    let well = f.rc(700.0, 590.0, 280.0, 60.0);
    f.fill(well, 4.0, th.inset);
    f.stroke_r(well, 4.0, Stroke::new(1.0, th.line2));
    for (i, (fr, chance)) in [
        (0.0, 0),
        (0.3, 0),
        (0.55, 0),
        (0.78, 0),
        (1.0, 0),
        (1.0, 1),
        (1.0, 2),
        (1.0, 3),
    ]
    .iter()
    .enumerate()
    {
        let (cx, cy, cw, chh) = (708.0 + i as f32 * 33.0, 600.0, 24.0, 40.0);
        let cr = f.rc(cx, cy, cw, chh);
        f.fill(cr, 2.0, mixc(th.inset, th.well_ink(), 0.1));
        if *fr > 0.0 {
            let fh = (chh - 2.0) * fr;
            let br = f.rc(cx + 1.0, cy + chh - 1.0 - fh, cw - 2.0, fh);
            f.fill(br, 1.5, al(th.accent, (140.0 + 115.0 * fr) as u8));
            for k in 0..*chance {
                f.circle((cx + cw - 4.0 - k as f32 * 4.0, cy + 4.0), 1.4, th.inset);
            }
        } else if i == 0 {
            f.stroke_r(cr, 2.0, Stroke::new(1.0, al(th.well_ink(), 40)));
        }
    }
}

// ---------------------------------------------------------------- sheet + app

struct App {
    th: Th,
    out: String,
    frames: u32,
    reported: bool,
}

const ZOOMS: [f32; 3] = [1.0, 0.75, 0.58];

fn paint_sheet(p: &Painter, th: Th, top_left: Pos2, report: bool) -> Tally {
    let sp = specs();
    let mut tally = Tally::default();
    let mut y = 16.0;
    let label = |y: f32, s: &str| cond_text(p, top_left + vec2(24.0, y + 10.0), s, 14.0, th.text);
    for z in ZOOMS {
        label(
            y,
            &format!(
                "{} · line_solid · amber · zoom {:.0} %",
                if th.dark { "graphite" } else { "paper" },
                z * 100.0
            ),
        );
        y += 30.0;
        let layout: [&[usize]; 4] = [&[0, 1, 2, 3], &[4], &[5], &[6]];
        for row in layout {
            let mut x = 24.0;
            for &i in row {
                let s = &sp[i];
                let o = top_left + vec2(x, y);
                let mut f = Face {
                    p,
                    th,
                    o,
                    z,
                    w: s.w,
                    name: s.id,
                    fam: s.fam,
                    leaves: vec![],
                    conts: vec![],
                    drops: vec![],
                    next: 0,
                };
                paint_face(&mut f, s);
                check(&f, &mut tally);
                x += s.w * z + 16.0;
            }
            y += FACE_H * z + 14.0;
        }
        y += 10.0;
    }
    // Non-text marks (value arcs, traces, jack rings) need 3:1 on the surface they sit on (WCAG 1.4.11).
    for (n, fg, bg) in [
        ("accent on surface", th.accent, th.surf),
        ("accent on well", th.accent, th.inset),
        ("audio on surface", th.audio, th.surf),
        ("cv on surface", th.cv, th.surf),
        ("gate on surface", th.gate, th.surf),
        ("pitch on surface", th.pitch, th.surf),
        ("pointer on cap (light end)", th.pointer, th.cap0),
        ("pointer on cap (dark end)", th.pointer, th.cap1),
    ] {
        let c = contrast(fg, bg);
        if c < 3.0 {
            tally.warnings += 1;
            tally
                .lines
                .push(format!("warning marks: {n} {c:.2}:1 < 3:1"));
        }
    }
    label(
        y,
        &format!(
            "{} · parts sheet",
            if th.dark { "graphite" } else { "paper" }
        ),
    );
    paint_parts(p, th, top_left + vec2(0.0, y + 30.0));
    if report {
        let by: std::collections::BTreeMap<_, usize> =
            tally.lines.iter().fold(Default::default(), |mut m, l| {
                *m.entry(l.split(' ').nth(2).unwrap_or("").to_string())
                    .or_default() += 1;
                m
            });
        println!("checker: errors {} · warnings {} ({} leaves checked, {} labels dropped by rule at compact) {by:?}", tally.errors, tally.warnings, tally.leaves, tally.dropped);
        for l in tally.lines.iter().take(40) {
            println!("  {l}");
        }
    }
    tally
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let p = ui.painter().clone();
        p.rect_filled(Rect::EVERYTHING, 0.0, self.th.bg);
        let t = paint_sheet(&p, self.th, pos2(0.0, 0.0), !self.reported);
        self.reported = true;
        let _ = t;
        self.frames += 1;
        if self.frames == 4 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(Arc::clone(image)),
                _ => None,
            })
        });
        if let Some(img) = shot {
            let [w, h] = img.size;
            let raw: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            image::save_buffer(&self.out, &raw, w as u32, h as u32, image::ColorType::Rgba8)
                .unwrap();
            println!("wrote {} ({w}x{h})", self.out);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint();
    }
}

fn main() {
    let mut a = std::env::args().skip(1);
    let theme = a.next().unwrap_or_else(|| "graphite".into());
    let out = a
        .next()
        .unwrap_or_else(|| format!("target/mock/{theme}.png"));
    if let Some(d) = std::path::Path::new(&out).parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let th = if theme == "paper" {
        paper()
    } else {
        graphite()
    };
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([SHEET_W + 48.0, 4400.0])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "kabl-mock",
        opts,
        Box::new(move |cc| {
            kabl_ui::theme::install_fonts(&cc.egui_ctx);
            cc.egui_ctx.add_font(egui::epaint::text::FontInsert::new(
                "plex-cond-semibold",
                egui::FontData::from_static(include_bytes!(
                    "../assets/IBMPlexSansCondensed-SemiBold.ttf"
                )),
                vec![egui::epaint::text::InsertFontFamily {
                    family: cond_family(),
                    priority: egui::epaint::text::FontPriority::Highest,
                }],
            ));
            cc.egui_ctx.set_pixels_per_point(1.0);
            Ok(Box::new(App {
                th,
                out,
                frames: 0,
                reported: false,
            }))
        }),
    )
    .unwrap();
}
