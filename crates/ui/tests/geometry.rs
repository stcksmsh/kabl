//! G0 geometry checker (docs/design/redesign/PLAN.md §4, §9.3, §10 A).
//!
//! For every kind in `KNOWN_KINDS` x zoom {0.5, 0.58, 0.75, 1, 1.5, 2} it takes the leaf rects
//! from the `rack.rs` layout, measures text with the embedded Plex fonts (headless egui, no
//! GPU) and checks the PLAN rules. It reports and never fails, except under the `strict`
//! feature. Output: `target/ui-check/report.json` and `report.html`.
//!
//! Tiers: Full z >= 0.864 draws text as the app does today; Compact 0.55..0.864 is the approved
//! target (text held at 9.5 screen px, values and captions dropped, title >= 10.5); Overview
//! z < 0.55 draws no face text (G6). `required-text` is the one rule that models today's
//! renderer: it hides all label text below 9.5 screen px, so the compact tier loses its labels.
//! Zoom-independent rules (`grid4`, `empty-zone`, `zone`, `contrast`) run once, at zoom 1.

use egui::{pos2, vec2, Color32, FontId, Painter, Pos2, Rect};
use kabl_core::{ModuleState, PatchState, Vec2 as WorldVec};
use kabl_modules::{registry, PortDirection};
use kabl_ui::rack::{self, Decor, Geo, Placed};
use kabl_ui::routing::{fmt_value, param_label};
use kabl_ui::style::{builtin_a, builtin_b, contrast, KnobScale, Role, Style};
use std::fmt::Write as _;

const ZOOMS: [f32; 6] = [0.5, 0.58, 0.75, 1.0, 1.5, 2.0];
const FULL_FROM: f32 = 0.864;
const COMPACT_FROM: f32 = 0.55;
/// Text smaller than this on screen is dropped by today's renderer (`kit::face::HIDE_BELOW`).
const HIDE_BELOW: f32 = 9.5;
const EMPTY_MAX: f32 = 0.55;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Sev {
    Error,
    Warning,
    Info,
}

impl Sev {
    fn name(self) -> &'static str {
        match self {
            Sev::Error => "error",
            Sev::Warning => "warning",
            Sev::Info => "info",
        }
    }
}

struct Rec {
    face: String,
    look: &'static str,
    zoom: f32,
    rule: &'static str,
    sev: Sev,
    note: String,
    boxes: Vec<Rect>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum K {
    Knob,
    Label,
    Value,
    Select,
    Jack,
    JackLabel,
    Chip,
    Title,
    Caption,
}

/// A thing that is drawn. `group` ties a control to its own label and value.
struct Leaf {
    k: K,
    r: Rect,
    group: usize,
    /// Zoom-free x anchor, relative to the face (grid rule).
    ax: f32,
    name: String,
}

/// Measures text with the embedded fonts.
struct Ruler(Painter);

impl Ruler {
    /// Advance width from the font; height is the glyph extent (0.85 em: cap height to descender),
    /// not the line box, which would flag every label/value pair as touching.
    fn size(&self, s: &str, font: FontId) -> egui::Vec2 {
        let w = self
            .0
            .layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE)
            .size()
            .x;
        vec2(w, font.size * 0.85)
    }
}

// Copy of `port_label` in lib.rs (private there; rack.rs and lib.rs are read-only for G0).
fn port_label(name: &str) -> String {
    name.split('_')
        .map(|w| {
            let w = match w {
                "resonance" => "res",
                "velocity" => "vel",
                w => w,
            };
            let (word, digits) = w.split_at(w.trim_end_matches(|c: char| c.is_ascii_digit()).len());
            let mut s = if word.len() <= 2 && word != "in" {
                word.to_uppercase()
            } else {
                let mut s = word.to_string();
                s[..1].make_ascii_uppercase();
                s
            };
            if !digits.is_empty() {
                s = format!("{s} {digits}");
            }
            s
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn font(st: &Style, role: Role, px: f32) -> FontId {
    FontId::new(px, st.type_role(role).font.family(true))
}

fn place(kind: &str) -> Placed {
    let mut p = PatchState::new();
    p.modules.insert(
        1,
        ModuleState {
            kind: kind.into(),
            pos: WorldVec {
                x: rack::RACK_X,
                y: rack::row_y(0),
            },
            params: Default::default(),
        },
    );
    let mut l = rack::layout(&p, &rack::View::default());
    l.mods.remove(0)
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

/// Face-relative design rect to screen px.
fn sc(m: &Placed, z: f32, r: Rect) -> Rect {
    let o = m.face.min.to_vec2();
    Rect::from_min_max((r.min - o) * z, (r.max - o) * z)
}

/// Containers: wells and plates. Leaves may sit inside or apart, never straddle.
fn containers(m: &Placed) -> Vec<Rect> {
    let mut v: Vec<Rect> = m.plate.into_iter().collect();
    match m.decor {
        Decor::Keys(r)
        | Decor::Display(r)
        | Decor::Transport(r)
        | Decor::Banks(r)
        | Decor::Operators(r)
        | Decor::Cues(r)
        | Decor::Grid(r) => v.push(r),
        _ => {}
    }
    v
}

struct Out {
    recs: Vec<Rec>,
    leaves: Vec<(String, f32, Vec<Rect>)>,
}

#[allow(clippy::too_many_lines)]
fn check_face(kind: &str, z: f32, ruler: &Ruler, st: &Style, out: &mut Out) {
    let m = place(kind);
    let name = kind.to_string();
    let face = sc(&m, z, m.face);
    let tier_full = z >= FULL_FROM;
    let overview = z < COMPACT_FROM;
    let mut recs: Vec<Rec> = Vec::new();
    let mut push = |rule: &'static str, sev: Sev, note: String, boxes: Vec<Rect>| {
        recs.push(Rec {
            face: name.clone(),
            look: "*",
            zoom: z,
            rule,
            sev,
            note,
            boxes,
        });
    };

    // Text size per tier: full as today's renderer, compact held at 9.5 (target).
    let label_px = if tier_full {
        (12.0 * z).max(11.0)
    } else {
        HIDE_BELOW
    };
    let caption_px = (11.0 * z).max(11.0);
    let title_px = if tier_full {
        (15.0 * z).max(11.0)
    } else {
        (15.0 * z).max(10.5)
    };
    // Today's renderer: labels need 12 * z >= 9.5.
    let renders_today = 12.0 * z >= HIDE_BELOW;

    let mut leaves: Vec<Leaf> = Vec::new();
    let mut required: Vec<(String, Rect)> = Vec::new();
    let mut dropped: Vec<Rect> = Vec::new();
    let mut group = 0usize;
    let knob_ring = match st.slots.knob.scale {
        KnobScale::Ticks => 7.5,
        KnobScale::TrackArc => 6.7,
        KnobScale::None => 1.5,
    };

    for c in &m.ctls {
        group += 1;
        let label = param_label(c.param);
        let lw = ruler.size(&label, font(st, Role::Label, label_px));
        let lp = sc(
            &m,
            z,
            Rect::from_center_size(c.geo.label_pos(), egui::Vec2::ZERO),
        )
        .center();
        let lr = Rect::from_center_size(lp, lw);
        let (slot, ax) = match c.geo {
            Geo::Knob { c: at, .. } => (m.slot_width(c), at.x - m.face.min.x),
            Geo::Select { rect } => (rect.width().max(48.0), rect.left() - m.face.min.x),
        };
        match c.geo {
            Geo::Knob { c: at, r } => {
                let rr = Rect::from_center_size(
                    sc(&m, z, Rect::from_center_size(at, egui::Vec2::ZERO)).center(),
                    egui::Vec2::splat(2.0 * (r + knob_ring) * z),
                );
                leaves.push(Leaf {
                    k: K::Knob,
                    r: rr,
                    group,
                    ax,
                    name: c.param.name.into(),
                });
            }
            Geo::Select { rect } => {
                leaves.push(Leaf {
                    k: K::Select,
                    r: sc(&m, z, rect),
                    group,
                    ax,
                    name: c.param.name.into(),
                });
            }
        }
        // The label is required text. Text wider than its slot is dropped by rule (info) in the
        // compact tier and an error in the full tier.
        if !renders_today || overview {
            required.push((format!("label {}", c.param.name), lr));
        }
        if overview {
            dropped.push(lr);
        } else if lw.x > slot * z - 4.0 {
            if tier_full {
                push(
                    "fit",
                    Sev::Error,
                    format!("label '{label}' {:.0}px > slot {:.0}px", lw.x, slot * z),
                    vec![lr],
                );
            } else {
                push(
                    "fit",
                    Sev::Info,
                    format!(
                        "label '{label}' dropped: {:.0}px > slot {:.0}px",
                        lw.x,
                        slot * z
                    ),
                    vec![lr],
                );
                dropped.push(lr);
            }
        }
        if !overview && !dropped.contains(&lr) {
            leaves.push(Leaf {
                k: K::Label,
                r: lr,
                group,
                ax,
                name: format!("label {}", c.param.name),
            });
        }
        // Value readout: full tier only (compact: shown on hover).
        if tier_full {
            if let Geo::Knob { .. } = c.geo {
                let vt = fmt_value(c.param, c.param.default);
                let vf = font(st, Role::Value, (12.0 * z).max(11.0));
                let mut vs = ruler.size(&vt, vf.clone());
                let max_w = slot * z - 4.0;
                if vs.x > max_w && vt.contains(' ') {
                    vs = ruler.size(&vt.replace(' ', ""), vf);
                }
                let vp = sc(
                    &m,
                    z,
                    Rect::from_center_size(c.geo.value_pos(), egui::Vec2::ZERO),
                )
                .center();
                let vr = Rect::from_center_size(vp, vs);
                if vs.x > max_w {
                    push(
                        "fit",
                        Sev::Error,
                        format!("value '{vt}' {:.0}px > slot {:.0}px", vs.x, max_w),
                        vec![vr],
                    );
                }
                leaves.push(Leaf {
                    k: K::Value,
                    r: vr,
                    group,
                    ax,
                    name: format!("value {}", c.param.name),
                });
            }
        }
    }

    for j in &m.jacks {
        group += 1;
        let c = sc(&m, z, Rect::from_center_size(j.c, egui::Vec2::ZERO)).center();
        let ax = j.c.x - m.face.min.x;
        leaves.push(Leaf {
            k: K::Jack,
            r: Rect::from_center_size(c, egui::Vec2::splat(2.0 * rack::JACK_R * z)),
            group,
            ax,
            name: format!("jack {}", j.port.name),
        });
        let label = port_label(j.port.name);
        let lw = ruler.size(&label, font(st, Role::Label, label_px));
        let lc = if j.label_right {
            c + vec2(20.0 * z + lw.x / 2.0, 0.0)
        } else {
            c - vec2(0.0, 24.0 * z)
        };
        let lr = Rect::from_center_size(lc, lw);
        if !renders_today || overview {
            required.push((format!("jack label {}", j.port.name), lr));
        }
        if overview {
            dropped.push(lr);
            continue;
        }
        let pitch = m
            .jacks
            .iter()
            .filter(|o| {
                !std::ptr::eq(*o, j)
                    && (o.c.y - j.c.y).abs() < 4.0
                    && o.port.direction == j.port.direction
            })
            .map(|o| (o.c.x - j.c.x).abs())
            .fold(f32::MAX, f32::min);
        let slot = if j.label_right { f32::MAX } else { pitch * z };
        if lw.x > slot - 2.0 {
            if tier_full {
                push(
                    "fit",
                    Sev::Error,
                    format!("jack label '{label}' {:.0}px > pitch {:.0}px", lw.x, slot),
                    vec![lr],
                );
            } else {
                push(
                    "fit",
                    Sev::Info,
                    format!(
                        "jack label '{label}' dropped: {:.0}px > pitch {:.0}px",
                        lw.x, slot
                    ),
                    vec![lr],
                );
                dropped.push(lr);
                continue;
            }
        }
        leaves.push(Leaf {
            k: K::JackLabel,
            r: lr,
            group,
            ax,
            name: format!("label {}", j.port.name),
        });
    }

    // Header: title (as `kit::face::title`), `+N` chip, caption.
    group += 1;
    let tf = font(st, Role::H3, title_px);
    let mut tsz = ruler.size(m.info.name, tf.clone());
    let mut tfont = tf;
    if tsz.x > face.width() - 100.0 * z {
        tfont = FontId::new((title_px * 0.8).max(11.0), tfont.family.clone());
        tsz = ruler.size(m.info.name, tfont.clone());
    }
    let tag = format!("{} · #{}", m.info.kind, 1);
    let capsz = ruler.size(&tag, font(st, Role::Caption, caption_px));
    let y = 26.0 * z;
    let name_r = Rect::from_min_size(pos2(12.0 * z, y - tsz.y / 2.0), tsz);
    let tag_r = Rect::from_min_size(pos2(12.0 * z, 43.0 * z - capsz.y / 2.0), capsz);
    let avoid: Vec<Rect> = m
        .ctls
        .iter()
        .map(|c| sc(&m, z, c.geo.bounds().expand2(vec2(4.0, 0.0))))
        .chain(m.jacks.iter().map(|j| {
            sc(
                &m,
                z,
                Rect::from_center_size(j.c, vec2(48.0, 20.0)).translate(vec2(0.0, -14.0)),
            )
        }))
        .collect();
    let decor = match m.decor {
        Decor::Keys(r)
        | Decor::Display(r)
        | Decor::Transport(r)
        | Decor::Banks(r)
        | Decor::Operators(r)
        | Decor::Grid(r)
        | Decor::Cues(r) => Some(sc(&m, z, r)),
        _ => None,
    };
    let left = name_r.max.x <= face.width() - 6.0 * z
        && tag_r.max.x <= face.width() - 6.0 * z
        && !avoid
            .iter()
            .chain(decor.iter())
            .any(|a| a.intersects(name_r) || a.intersects(tag_r));
    let name_r = if left {
        name_r
    } else {
        Rect::from_center_size(pos2(face.center().x, y), tsz)
    };
    leaves.push(Leaf {
        k: K::Title,
        r: name_r,
        group,
        ax: 12.0,
        name: "title".into(),
    });
    if tier_full {
        let cap_r = if left {
            tag_r
        } else {
            Rect::from_center_size(pos2(face.center().x, 43.0 * z), capsz)
        };
        leaves.push(Leaf {
            k: K::Caption,
            r: cap_r,
            group,
            ax: 12.0,
            name: "caption".into(),
        });
    }
    if let Some(t) = m.toggle {
        group += 1;
        leaves.push(Leaf {
            k: K::Chip,
            r: sc(&m, z, t),
            group,
            ax: t.min.x - m.face.min.x,
            name: "+N chip".into(),
        });
    }

    // overlap + gap
    let (g_any, g_own) = if tier_full { (3.0, 2.0) } else { (2.0, 1.5) };
    let boxes = containers(&m)
        .into_iter()
        .map(|r| sc(&m, z, r))
        .collect::<Vec<_>>();
    for (i, a) in leaves.iter().enumerate() {
        for b in &leaves[i + 1..] {
            let own = a.group == b.group && a.k != K::Title;
            if overlaps(a.r, b.r) {
                push(
                    "overlap",
                    Sev::Error,
                    format!("{} / {}", a.name, b.name),
                    vec![a.r, b.r],
                );
            } else {
                let min = if own { g_own } else { g_any };
                let d = dist(a.r, b.r);
                if d < min {
                    push(
                        "gap",
                        Sev::Error,
                        format!("{} / {}: {d:.1}px < {min}px", a.name, b.name),
                        vec![a.r, b.r],
                    );
                }
            }
        }
        // containers: inside or apart, never straddling
        for c in &boxes {
            if overlaps(a.r, *c) && !c.expand(0.5).contains_rect(a.r) {
                push(
                    "overlap",
                    Sev::Error,
                    format!("{} straddles a well/plate", a.name),
                    vec![a.r, *c],
                );
            }
        }
        // edges
        if !face.contains_rect(a.r) {
            push(
                "outside",
                Sev::Error,
                format!("{} outside face", a.name),
                vec![a.r, face],
            );
        } else {
            let m_ = (a.r.min.x - face.min.x)
                .min(face.max.x - a.r.max.x)
                .min(a.r.min.y - face.min.y)
                .min(face.max.y - a.r.max.y);
            if m_ < 4.0 {
                push(
                    "edge",
                    Sev::Error,
                    format!("{} {m_:.1}px from face edge", a.name),
                    vec![a.r, face],
                );
            }
        }
    }
    if !required.is_empty() {
        let sev = if overview { Sev::Info } else { Sev::Error };
        let n = required.len();
        let note = format!(
            "{n} label(s) not drawn ({}); today's renderer hides text under {HIDE_BELOW} px",
            required
                .iter()
                .take(4)
                .map(|r| r.0.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        push(
            "required-text",
            sev,
            note,
            required.iter().map(|r| r.1).collect(),
        );
    }
    if !dropped.is_empty() && overview {
        push(
            "fit",
            Sev::Info,
            format!("{} labels dropped (overview tier)", dropped.len()),
            dropped.clone(),
        );
    }

    // Zoom-independent rules: once, at zoom 1.
    if (z - 1.0).abs() < f32::EPSILON {
        let mut grid = Vec::new();
        for l in &leaves {
            if matches!(l.k, K::Knob | K::Jack | K::Select | K::Chip)
                && (l.ax / 4.0 - (l.ax / 4.0).round()).abs() > 0.01
            {
                grid.push(l.r);
            }
        }
        if !grid.is_empty() {
            push(
                "grid4",
                Sev::Warning,
                format!("{} leaves with x not on the 4 px grid", grid.len()),
                grid,
            );
        }
        // Zone conformance (approved §11.1): jack row centres at y 273 / 316 (narrow), 316 (wide).
        let want_in = if m.face.width() >= 480.0 {
            316.0
        } else {
            273.0
        };
        let bad: Vec<Rect> = m
            .jacks
            .iter()
            .filter(|j| {
                let y = j.c.y - m.face.min.y;
                let want = if j.port.direction == PortDirection::Input {
                    want_in
                } else {
                    316.0
                };
                (y - want).abs() > 0.5
            })
            .map(|j| sc(&m, z, Rect::from_center_size(j.c, egui::Vec2::splat(26.0))))
            .collect();
        if !bad.is_empty() {
            push(
                "zone",
                Sev::Warning,
                format!("{} jack(s) off the approved rows (273/316)", bad.len()),
                bad,
            );
        }
        // Empty-zone ratio over the control zone, 1 px raster.
        let w = m.face.width();
        let (x0, x1, y0, y1) = (12.0, w - 12.0, 52.0, 236.0);
        let mut cov = vec![false; ((x1 - x0) as usize + 1) * ((y1 - y0) as usize + 1)];
        let cw = (x1 - x0) as usize + 1;
        let mut mark = |r: Rect| {
            for yy in (r.min.y.max(y0) as i32)..(r.max.y.min(y1) as i32) {
                for xx in (r.min.x.max(x0) as i32)..(r.max.x.min(x1) as i32) {
                    cov[(yy - y0 as i32) as usize * cw + (xx - x0 as i32) as usize] = true;
                }
            }
        };
        for l in &leaves {
            mark(l.r);
        }
        for c in containers(&m) {
            mark(sc(&m, 1.0, c));
        }
        let ratio = 1.0 - cov.iter().filter(|c| **c).count() as f32 / cov.len() as f32;
        if ratio > EMPTY_MAX {
            push(
                "empty-zone",
                Sev::Warning,
                format!("control zone {:.0}% empty", ratio * 100.0),
                vec![Rect::from_min_max(pos2(x0, y0), pos2(x1, y1))],
            );
        }
    }

    out.leaves
        .push((name.clone(), z, leaves.iter().map(|l| l.r).collect()));
    out.recs.extend(recs);
}

fn contrast_recs(kind: &str, out: &mut Out) {
    for (id, st) in [
        ("satin-dark", builtin_a(true)),
        ("satin-light", builtin_a(false)),
        ("lacquer-dark", builtin_b(true)),
        ("lacquer-light", builtin_b(false)),
    ] {
        let s = st.section_of(kind);
        for (what, fg) in [("ink", s.ink), ("ink2", s.ink2)] {
            let c = contrast(fg, s.base);
            if c < 4.5 {
                out.recs.push(Rec {
                    face: kind.into(),
                    look: id,
                    zoom: 1.0,
                    rule: "contrast",
                    sev: Sev::Error,
                    note: format!("{what} on face base {c:.2}:1 < 4.5:1"),
                    boxes: vec![],
                });
            }
        }
    }
}

fn run_all() -> Out {
    let ctx = egui::Context::default();
    kabl_ui::theme::install_fonts(&ctx);
    let input = || egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(2000.0, 1000.0))),
        ..Default::default()
    };
    for _ in 0..2 {
        ctx.begin_pass(input());
        let _ = ctx.end_pass();
    }
    let mut st = builtin_a(true);
    st.fonts_ready = true;
    let mut out = Out {
        recs: vec![],
        leaves: vec![],
    };
    ctx.begin_pass(input());
    let ruler = Ruler(Painter::new(
        ctx.clone(),
        egui::LayerId::background(),
        Rect::EVERYTHING,
    ));
    for kind in registry::KNOWN_KINDS {
        for z in ZOOMS {
            check_face(kind, z, &ruler, &st, &mut out);
        }
        contrast_recs(kind, &mut out);
    }
    let _ = ctx.end_pass();
    out
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn write_reports(out: &Out) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-check");
    std::fs::create_dir_all(&dir).unwrap();
    let mut j = String::from("[\n");
    for (i, r) in out.recs.iter().enumerate() {
        let boxes = r
            .boxes
            .iter()
            .map(|b| {
                format!(
                    "[{:.1},{:.1},{:.1},{:.1}]",
                    b.min.x,
                    b.min.y,
                    b.width(),
                    b.height()
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let _ = write!(
            j,
            "{}{{\"face\":\"{}\",\"look\":\"{}\",\"zoom\":{},\"rule\":\"{}\",\"boxes\":[{}],\"sev\":\"{}\",\"note\":\"{}\"}}",
            if i == 0 { "" } else { ",\n" },
            r.face,
            r.look,
            r.zoom,
            r.rule,
            boxes,
            r.sev.name(),
            json_escape(&r.note)
        );
    }
    j.push_str("\n]\n");
    std::fs::write(dir.join("report.json"), j).unwrap();

    // HTML: summary + per face a row of SVGs (one per zoom) with leaves grey, violations coloured.
    let mut h = String::from(
        "<!doctype html><meta charset=utf-8><title>kabl geometry report</title><style>\
body{font:13px sans-serif;background:#111214;color:#ecebe6;margin:16px}h1{font-size:18px}\
.f{margin:14px 0}.z{display:inline-block;vertical-align:top;margin:0 10px 8px 0}\
svg{background:#1a1b1e;border:1px solid #393c42}table{border-collapse:collapse}td,th{padding:2px 10px;text-align:right}\
td:first-child,th:first-child{text-align:left}.e{color:#ff6b5d}.w{color:#f2a33a}.i{color:#6f9bd6}small{color:#a29f99}</style>",
    );
    let count = |rule: &str, s: Sev| {
        out.recs
            .iter()
            .filter(|r| r.rule == rule && r.sev == s)
            .count()
    };
    let (te, tw) = (
        out.recs.iter().filter(|r| r.sev == Sev::Error).count(),
        out.recs.iter().filter(|r| r.sev == Sev::Warning).count(),
    );
    let _ = write!(h, "<h1>kabl geometry report · errors {te} · warnings {tw}</h1><table><tr><th>rule<th>errors<th>warnings<th>info");
    let mut rules: Vec<&str> = out.recs.iter().map(|r| r.rule).collect();
    rules.sort_unstable();
    rules.dedup();
    for r in &rules {
        let _ = write!(
            h,
            "<tr><td>{r}<td class=e>{}<td class=w>{}<td class=i>{}",
            count(r, Sev::Error),
            count(r, Sev::Warning),
            count(r, Sev::Info)
        );
    }
    h.push_str("</table><p><small>Boxes are screen px at each zoom. Grey = leaf, red = error, amber = warning, blue = info. Zoom-independent rules show at 1.</small></p>");
    let mut faces: Vec<&str> = registry::KNOWN_KINDS.to_vec();
    faces.sort_by_key(|f| {
        std::cmp::Reverse(
            out.recs
                .iter()
                .filter(|r| r.face == *f && r.sev == Sev::Error)
                .count(),
        )
    });
    for f in faces {
        let ne = out
            .recs
            .iter()
            .filter(|r| r.face == f && r.sev == Sev::Error)
            .count();
        let _ = write!(h, "<div class=f><b>{f}</b> <small>{ne} errors</small><br>");
        for z in ZOOMS {
            let Some((_, _, ls)) = out.leaves.iter().find(|l| l.0 == f && l.1 == z) else {
                continue;
            };
            let m = place(f);
            let (w, hh) = (m.face.width() * z, m.face.height() * z);
            let _ = write!(h, "<div class=z><small>{z}</small><br><svg width={w:.0} height={hh:.0} viewBox='0 0 {w:.1} {hh:.1}'>");
            for r in ls {
                let _ = write!(h, "<rect x={:.1} y={:.1} width={:.1} height={:.1} fill='#8886' stroke='#999' stroke-width=.5 />", r.min.x, r.min.y, r.width(), r.height());
            }
            for r in out
                .recs
                .iter()
                .filter(|r| r.face == f && r.sev != Sev::Info && (r.zoom - z).abs() < 1e-4)
            {
                let col = if r.sev == Sev::Error {
                    "#ff6b5d"
                } else {
                    "#f2a33a"
                };
                for b in &r.boxes {
                    let _ = write!(h, "<rect x={:.1} y={:.1} width={:.1} height={:.1} fill=none stroke='{col}' stroke-width=1.2 />", b.min.x, b.min.y, b.width(), b.height());
                }
            }
            h.push_str("</svg></div>");
        }
        h.push_str("</div>");
    }
    std::fs::write(dir.join("report.html"), h).unwrap();
    dir
}

#[test]
fn geometry_report() {
    let out = run_all();
    let dir = write_reports(&out);
    let n = |s: Sev| out.recs.iter().filter(|r| r.sev == s).count();
    println!(
        "ui-check: errors {} · warnings {} · info {} ({})",
        n(Sev::Error),
        n(Sev::Warning),
        n(Sev::Info),
        dir.display()
    );
    #[cfg(feature = "strict")]
    assert_eq!(
        n(Sev::Error),
        0,
        "geometry errors; see target/ui-check/report.html"
    );
}
