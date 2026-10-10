//! Application chrome: toolbar, Sounds browser, inspector rail, status bar and diagnostics.
use crate::prim::*;
use crate::tokens::{Dir, Sect};
use crate::widgets::*;
use egui::{pos2, vec2, Align2, Color32, Rect};

pub struct Areas {
    pub toolbar: Rect,
    pub status: Rect,
    pub left: Option<Rect>,
    pub right: Rect,
    pub center: Rect,
}

pub fn areas(dir: Dir, size: egui::Vec2, browser: bool, inspector_w: f32) -> Areas {
    let (tool, status) = match dir {
        Dir::A => (52.0, 30.0),
        Dir::B => (58.0, 32.0),
        Dir::C => (44.0, 26.0),
    };
    let narrow = size.x < 1360.0;
    let lw = match (dir, narrow) {
        (Dir::A, false) => 304.0,
        (Dir::A, true) => 280.0,
        (Dir::B, false) => 320.0,
        (Dir::B, true) => 288.0,
        (Dir::C, false) => 296.0,
        (Dir::C, true) => 268.0,
    };
    let toolbar = Rect::from_min_size(pos2(0.0, 0.0), vec2(size.x, tool));
    let status_r = Rect::from_min_max(pos2(0.0, size.y - status), pos2(size.x, size.y));
    let body_top = tool;
    let body_bot = size.y - status;
    let left = browser.then(|| Rect::from_min_max(pos2(0.0, body_top), pos2(lw, body_bot)));
    let right = Rect::from_min_max(pos2(size.x - inspector_w, body_top), pos2(size.x, body_bot));
    let center = Rect::from_min_max(
        pos2(left.map_or(0.0, |l| l.right()), body_top),
        pos2(right.left(), body_bot),
    );
    Areas {
        toolbar,
        status: status_r,
        left,
        right,
        center,
    }
}

/// Panel surface for browser / inspector.
pub fn panel_bg(cx: &Cx, r: Rect, edge_right: bool) {
    let k = cx.k;
    cx.p.rect_filled(r, 0.0, k.surface);
    let x = if edge_right {
        r.right() - 0.5
    } else {
        r.left() + 0.5
    };
    cx.line(
        pos2(x, r.top()),
        pos2(x, r.bottom()),
        if k.dir == Dir::C { 1.5 } else { 1.0 },
        if k.dir == Dir::C { k.line2 } else { k.line },
    );
}

pub fn heading(cx: &Cx, pos: egui::Pos2, s: &str) {
    let k = cx.k;
    match k.dir {
        Dir::A => {
            cx.text(pos, Align2::LEFT_CENTER, s, 20.0, "sans-semi", k.text);
        }
        Dir::B => {
            cx.caps(pos, Align2::LEFT_CENTER, s, 12.0, "sans-semi", k.text2, 1.4);
        }
        Dir::C => {
            cx.caps(pos, Align2::LEFT_CENTER, s, 18.0, "cond-semi", k.text, 0.8);
        }
    }
}

pub fn section_label(cx: &Cx, pos: egui::Pos2, s: &str) {
    let k = cx.k;
    let (f, sz) = if k.dir == Dir::C {
        ("cond-semi", 12.0)
    } else {
        ("sans-semi", 11.0)
    };
    cx.caps(
        pos,
        Align2::LEFT_CENTER,
        s,
        sz,
        f,
        k.text2,
        if k.dir == Dir::C { 0.9 } else { 1.0 },
    );
}

pub fn toolbar(
    cx: &Cx,
    r: Rect,
    view: &str,
    patch: &str,
    dirty: bool,
    zoom: &str,
    panes: (bool, bool),
    cable_mode: usize,
) {
    let k = cx.k;
    cx.p.rect_filled(r, 0.0, if k.dir == Dir::B { k.bg } else { k.surface });
    cx.line(
        pos2(0.0, r.bottom() - 0.5),
        pos2(r.right(), r.bottom() - 0.5),
        if k.dir == Dir::C { 1.5 } else { 1.0 },
        if k.dir == Dir::C { k.line2 } else { k.line },
    );
    let cy = r.center().y;
    let mut x = 16.0;
    // Wordmark: two plugs joined by a cable.
    let mark = pos2(x + 9.0, cy);
    cx.p.circle_filled(mark - vec2(5.0, 0.0), 4.0, k.audio);
    cx.p.circle_filled(mark + vec2(5.0, 0.0), 4.0, k.cv);
    cx.p.add(egui::Shape::line(
        cable_pts(mark - vec2(5.0, 0.0), mark + vec2(5.0, 0.0), 8.0),
        egui::Stroke::new(1.6, k.text),
    ));
    let wm = match k.dir {
        Dir::C => ("cond-semi", 22.0),
        _ => ("sans-semi", 20.0),
    };
    cx.text(
        pos2(x + 26.0, cy),
        Align2::LEFT_CENTER,
        "kabl",
        wm.1,
        wm.0,
        k.text,
    );
    x += 82.0;
    // View switch.
    let views = ["Rack", "Perform"];
    match k.dir {
        Dir::B => {
            for v in views {
                let w = cx.width(v, 14.0, "sans-semi") + 24.0;
                let on = v == view;
                let tr = Rect::from_min_size(pos2(x, cy - 15.0), vec2(w, 30.0));
                cx.text(
                    tr.center(),
                    Align2::CENTER_CENTER,
                    v,
                    14.0,
                    "sans-semi",
                    if on { k.text } else { k.text2 },
                );
                if on {
                    cx.glow_line(
                        vec![
                            pos2(tr.left() + 8.0, r.bottom() - 3.0),
                            pos2(tr.right() - 8.0, r.bottom() - 3.0),
                        ],
                        2.5,
                        k.accent,
                        1.0,
                    );
                }
                x += w + 2.0;
            }
        }
        _ => {
            let w = 176.0;
            let seg = Rect::from_min_size(pos2(x, cy - 15.0), vec2(w, 30.0));
            let (track, on_fill, on_ink, off_ink) = match k.dir {
                Dir::C => (k.surface, k.text, k.surface, k.text),
                _ => (k.inset, k.raised, k.text, k.text2),
            };
            cx.rr(seg, k.r_sm + 1.0, track);
            cx.rr_stroke(
                seg,
                k.r_sm + 1.0,
                if k.dir == Dir::C { 1.5 } else { 1.0 },
                if k.dir == Dir::C { k.line2 } else { k.line },
            );
            for (i, v) in views.iter().enumerate() {
                let s = Rect::from_min_size(
                    seg.min + vec2(w / 2.0 * i as f32, 0.0),
                    vec2(w / 2.0, 30.0),
                );
                let on = *v == view;
                if on {
                    let sr = s.shrink(2.0);
                    if k.dir != Dir::C {
                        cx.shadow(sr, k.r_sm, 0);
                    }
                    cx.rr(sr, k.r_sm, on_fill);
                }
                let (fnt, sz) = if k.dir == Dir::C {
                    ("cond-semi", 14.0)
                } else {
                    ("sans-semi", 13.0)
                };
                let lab = if k.dir == Dir::C {
                    v.to_uppercase()
                } else {
                    v.to_string()
                };
                cx.text(
                    s.center(),
                    Align2::CENTER_CENTER,
                    &lab,
                    sz,
                    fnt,
                    if on { on_ink } else { off_ink },
                );
            }
            x += w + 14.0;
        }
    }
    // Patch navigator.
    let nav_w = if k.dir == Dir::B { 320.0 } else { 280.0 };
    let nx = x + if k.dir == Dir::B { 14.0 } else { 6.0 };
    let nav = Rect::from_min_size(pos2(nx, cy - 17.0), vec2(nav_w, 34.0));
    let rad = if k.dir == Dir::B { 17.0 } else { k.r_md };
    cx.rr(nav, rad, if k.dir == Dir::B { k.inset } else { k.inset });
    cx.rr_stroke(nav, rad, 1.0, k.line);
    let chev = |x: f32, dir: f32| {
        cx.p.add(egui::Shape::line(
            vec![
                pos2(x - 3.0 * dir, cy - 5.0),
                pos2(x + 3.0 * dir, cy),
                pos2(x - 3.0 * dir, cy + 5.0),
            ],
            egui::Stroke::new(1.6, k.text2),
        ));
    };
    chev(nav.left() + 18.0, -1.0);
    chev(nav.right() - 58.0, 1.0);
    let nf = if k.dir == Dir::C {
        "cond-semi"
    } else {
        "sans-semi"
    };
    let nm = if k.dir == Dir::C {
        patch.to_uppercase()
    } else {
        patch.to_string()
    };
    cx.text(
        pos2(nav.center().x - 14.0, cy),
        Align2::CENTER_CENTER,
        &nm,
        if k.dir == Dir::C { 15.0 } else { 14.0 },
        nf,
        k.text,
    );
    if dirty {
        cx.p.circle_filled(
            pos2(
                nav.center().x - 14.0 + cx.width(&nm, 14.0, nf) / 2.0 + 9.0,
                cy,
            ),
            3.0,
            k.accent,
        );
    }
    icon(cx.p, Ic::Save, pos2(nav.right() - 24.0, cy), 15.0, k.text2);
    // Right cluster.
    let mut rx = r.right() - 16.0;
    icon_btn(cx, pos2(rx - 15.0, cy), Ic::Gear, 0, true);
    rx -= 40.0;
    let routing = Rect::from_min_size(pos2(rx - 92.0, cy - 15.0), vec2(92.0, 30.0));
    chip(
        cx,
        routing,
        "Routing",
        if panes.1 { 2 } else { 0 },
        Some(Ic::Cable),
    );
    rx -= 100.0;
    let sounds = Rect::from_min_size(pos2(rx - 88.0, cy - 15.0), vec2(88.0, 30.0));
    chip(
        cx,
        sounds,
        "Sounds",
        if panes.0 { 2 } else { 0 },
        Some(Ic::Note),
    );
    rx -= 104.0;
    let rack_tools = cable_mode != usize::MAX;
    if rack_tools && r.width() > 1360.0 {
        // Zoom cluster.
        icon_btn(cx, pos2(rx - 15.0, cy), Ic::Fit, 0, true);
        rx -= 32.0;
        icon_btn(cx, pos2(rx - 15.0, cy), Ic::Plus, 0, true);
        rx -= 32.0;
        cx.text(
            pos2(rx - 24.0, cy),
            Align2::CENTER_CENTER,
            zoom,
            12.0,
            "mono-med",
            k.text2,
        );
        rx -= 52.0;
        icon_btn(cx, pos2(rx - 15.0, cy), Ic::Minus, 0, true);
        rx -= 40.0;
    }
    // Cable visibility.
    if rack_tools {
        let cab = Rect::from_min_size(pos2(rx - 132.0, cy - 15.0), vec2(132.0, 30.0));
        cx.rr(cab, k.r_sm + 1.0, k.inset);
        cx.rr_stroke(cab, k.r_sm + 1.0, 1.0, k.line);
        for (i, s) in ["All", "Focus", "Hide"].iter().enumerate() {
            let seg = Rect::from_min_size(cab.min + vec2(44.0 * i as f32, 0.0), vec2(44.0, 30.0));
            let on = i == cable_mode;
            if on {
                cx.rr(
                    seg.shrink(2.0),
                    k.r_sm,
                    if k.dir == Dir::C { k.text } else { k.raised },
                );
            }
            let (f, sz) = if k.dir == Dir::C {
                ("cond-semi", 12.0)
            } else {
                ("sans-med", 12.0)
            };
            cx.text(
                seg.center(),
                Align2::CENTER_CENTER,
                s,
                sz,
                f,
                if on {
                    if k.dir == Dir::C {
                        k.surface
                    } else {
                        k.text
                    }
                } else {
                    k.text2
                },
            );
        }
        rx -= 150.0;
    }
    icon_btn(cx, pos2(rx - 15.0, cy), Ic::Redo, 0, false);
    rx -= 32.0;
    icon_btn(cx, pos2(rx - 15.0, cy), Ic::Undo, 0, true);
}

pub struct Snd {
    pub name: &'static str,
    pub cat: &'static str,
    pub tags: &'static [&'static str],
    pub fav: bool,
}

pub const SOUNDS: [Snd; 6] = [
    Snd {
        name: "Breath",
        cat: "Wind",
        tags: &["Keys"],
        fav: false,
    },
    Snd {
        name: "Ensemble Strings",
        cat: "Strings",
        tags: &["Keys", "Pad"],
        fav: true,
    },
    Snd {
        name: "Evolving Pad",
        cat: "Pad",
        tags: &["Keys", "Slow"],
        fav: false,
    },
    Snd {
        name: "Glass Keys",
        cat: "Keys",
        tags: &["Keys", "Bright"],
        fav: false,
    },
    Snd {
        name: "Round Keyboard Bass",
        cat: "Bass",
        tags: &["Keys"],
        fav: false,
    },
    Snd {
        name: "Singing Lead",
        cat: "Lead",
        tags: &["Keys", "Mono"],
        fav: true,
    },
];

fn cat_sect(c: &str) -> Sect {
    match c {
        "Wind" => Sect::Timing,
        "Strings" | "Pad" => Sect::Mod,
        "Keys" => Sect::Filter,
        "Bass" => Sect::Amp,
        _ => Sect::Osc,
    }
}

pub fn browser(cx: &Cx, r: Rect, sel: usize, hover: usize, query: &str) {
    let k = cx.k;
    panel_bg(cx, r, true);
    let x0 = r.left() + 16.0;
    let w = r.width() - 32.0;
    let mut y = r.top() + 26.0;
    heading(cx, pos2(x0, y), "Sounds");
    let nb = Rect::from_min_size(
        pos2(r.right() - 16.0 - 56.0 - 36.0, y - 14.0),
        vec2(56.0, 28.0),
    );
    chip(cx, nb, "New", 0, None);
    icon_btn(cx, pos2(r.right() - 31.0, y), Ic::Close, 0, true);
    y += 34.0;
    field(
        cx,
        Rect::from_min_size(pos2(x0, y), vec2(w, 34.0)),
        "Search sounds",
        query,
        !query.is_empty(),
    );
    y += 46.0;
    // Scope tabs.
    let mut tx = x0;
    for (i, s) in ["All", "★", "Recent", "Factory", "Yours"]
        .iter()
        .enumerate()
    {
        let s = if i == 1 { "" } else { *s };
        let tw = if i == 1 {
            40.0
        } else {
            cx.width(s, 12.0, "sans-med") + 22.0
        };
        if tx + tw > r.right() - 8.0 {
            break;
        }
        chip(
            cx,
            Rect::from_min_size(pos2(tx, y), vec2(tw, 26.0)),
            s,
            if i == 3 { 2 } else { 0 },
            if i == 1 { Some(Ic::Star) } else { None },
        );
        tx += tw + 5.0;
    }
    y += 40.0;
    section_label(cx, pos2(x0, y), "Factory · 6 sounds");
    y += 18.0;
    let row_h = match k.dir {
        Dir::A => 48.0,
        Dir::B => 56.0,
        Dir::C => 32.0,
    };
    for (i, s) in SOUNDS.iter().enumerate() {
        let rr = Rect::from_min_size(pos2(r.left() + 8.0, y), vec2(r.width() - 16.0, row_h));
        let on = i == sel;
        let hv = i == hover && !on;
        match k.dir {
            Dir::A => {
                if on {
                    cx.rr(rr, k.r_md, k.accent_soft);
                    cx.rr(
                        Rect::from_min_size(rr.min + vec2(0.0, 8.0), vec2(3.0, rr.height() - 16.0)),
                        1.5,
                        k.accent,
                    );
                } else if hv {
                    cx.rr(rr, k.r_md, k.inset);
                }
            }
            Dir::B => {
                if on {
                    cx.rr(rr, k.r_md, k.raised);
                    cx.rr_stroke(rr, k.r_md, 1.5, a(k.accent, 200));
                    cx.p.rect_filled(
                        rr.expand(2.0),
                        egui::CornerRadius::same(12),
                        a(k.accent, 18),
                    );
                    cx.rr(rr, k.r_md, k.raised);
                    cx.rr_stroke(rr, k.r_md, 1.5, a(k.accent, 220));
                } else if hv {
                    cx.rr(rr, k.r_md, k.raised);
                }
            }
            Dir::C => {
                if on {
                    cx.p.rect_filled(rr, 0.0, k.text);
                } else if hv {
                    cx.p.rect_filled(rr, 0.0, k.inset);
                }
                cx.line(rr.left_bottom(), rr.right_bottom(), 1.0, k.line);
            }
        }
        let (tc, sc) = if k.dir == Dir::C && on {
            (k.surface, a(k.surface, 200))
        } else {
            (k.text, k.text2)
        };
        let cy = rr.center().y;
        let fav = if s.fav { Ic::StarFill } else { Ic::Star };
        icon(
            cx.p,
            fav,
            pos2(rr.left() + 18.0, cy),
            14.0,
            if s.fav { k.audio } else { a(sc, 150) },
        );
        let nf = if k.dir == Dir::C {
            "cond-semi"
        } else {
            "sans-semi"
        };
        let ns = if k.dir == Dir::C { 14.0 } else { 14.0 };
        let nm = if k.dir == Dir::C {
            s.name.to_uppercase()
        } else {
            s.name.to_string()
        };
        if k.dir == Dir::C {
            let nm = cx.fit(&nm, ns, nf, rr.width() - 36.0 - 84.0);
            cx.text(
                pos2(rr.left() + 36.0, cy),
                Align2::LEFT_CENTER,
                &nm,
                ns,
                nf,
                tc,
            );
            cx.text(
                pos2(rr.right() - 74.0, cy),
                Align2::LEFT_CENTER,
                s.cat,
                12.0,
                "cond-med",
                sc,
            );
        } else {
            cx.text(
                pos2(rr.left() + 36.0, cy - 9.0),
                Align2::LEFT_CENTER,
                &nm,
                ns,
                nf,
                tc,
            );
            let mut tx = rr.left() + 36.0;
            for t in s.tags {
                tx += tag(cx, pos2(tx, cy + 2.0), t, k.text2) + 4.0;
            }
            let c = k.sect(cat_sect(s.cat)).base;
            let cw = cx.width(s.cat, 12.0, "sans-med") + 22.0;
            let pr = Rect::from_min_size(pos2(rr.right() - cw - 10.0, cy - 10.0), vec2(cw, 20.0));
            cx.rr(
                pr,
                if k.dir == Dir::B { 10.0 } else { 4.0 },
                a(c, if k.dark { 90 } else { 170 }),
            );
            cx.p.circle_filled(
                pos2(pr.left() + 9.0, cy),
                3.0,
                if k.dark {
                    mix(c, Color32::WHITE, 0.5)
                } else {
                    mix(c, Color32::BLACK, 0.5)
                },
            );
            cx.text(
                pos2(pr.left() + 16.0, cy),
                Align2::LEFT_CENTER,
                s.cat,
                12.0,
                "sans-med",
                if k.dark { k.text } else { k.text },
            );
        }
        y += row_h + if k.dir == Dir::C { 0.0 } else { 3.0 };
    }
    // Audition card at the bottom.
    let ch = 196.0;
    let card = Rect::from_min_max(
        pos2(r.left() + 12.0, r.bottom() - ch - 44.0),
        pos2(r.right() - 12.0, r.bottom() - 44.0),
    );
    if card.top() > y + 6.0 {
        cx.shadow(card, k.r_md, 0);
        cx.rr(card, k.r_md, k.raised);
        cx.rr_stroke(
            card,
            k.r_md,
            if k.dir == Dir::C { 1.5 } else { 1.0 },
            if k.dir == Dir::C { k.line2 } else { k.line },
        );
        let cx0 = card.left() + 14.0;
        let mut cy = card.top() + 20.0;
        section_label(cx, pos2(cx0, cy), "Audition");
        cx.text(
            pos2(card.right() - 14.0, cy),
            Align2::RIGHT_CENTER,
            SOUNDS[sel].name,
            12.0,
            "sans-med",
            k.text2,
        );
        cy += 28.0;
        for (i, s) in ["Note", "Chord"].iter().enumerate() {
            chip(
                cx,
                Rect::from_min_size(pos2(cx0 + i as f32 * 66.0, cy - 13.0), vec2(62.0, 26.0)),
                s,
                if i == 0 { 2 } else { 0 },
                None,
            );
        }
        chip(
            cx,
            Rect::from_min_size(
                pos2(card.right() - 14.0 - 80.0, cy - 13.0),
                vec2(80.0, 26.0),
            ),
            "C4",
            1,
            Some(Ic::Down),
        );
        cy += 32.0;
        cx.text(
            pos2(cx0, cy),
            Align2::LEFT_CENTER,
            "Velocity",
            12.0,
            "sans-med",
            k.text2,
        );
        slider(
            cx,
            Rect::from_min_size(
                pos2(cx0 + 66.0, cy - 9.0),
                vec2(card.width() - 28.0 - 66.0 - 40.0, 18.0),
            ),
            0.7,
        );
        cx.text(
            pos2(card.right() - 14.0, cy),
            Align2::RIGHT_CENTER,
            "90",
            12.0,
            "mono-med",
            k.text,
        );
        cy += 30.0;
        cx.text(
            pos2(cx0, cy),
            Align2::LEFT_CENTER,
            "Length",
            12.0,
            "sans-med",
            k.text2,
        );
        slider(
            cx,
            Rect::from_min_size(
                pos2(cx0 + 66.0, cy - 9.0),
                vec2(card.width() - 28.0 - 66.0 - 40.0, 18.0),
            ),
            0.3,
        );
        cx.text(
            pos2(card.right() - 14.0, cy),
            Align2::RIGHT_CENTER,
            "1.5 s",
            12.0,
            "mono-med",
            k.text,
        );
        cy += 34.0;
        button(
            cx,
            Rect::from_min_size(pos2(cx0, cy - 15.0), vec2(112.0, 32.0)),
            "Play C4",
            Some(Ic::Play),
            false,
        );
        chip(
            cx,
            Rect::from_min_size(pos2(cx0 + 122.0, cy - 14.0), vec2(80.0, 30.0)),
            "Stop",
            0,
            Some(Ic::Stop),
        );
    }
    let fy = r.bottom() - 22.0;
    cx.line(
        pos2(r.left(), fy - 14.0),
        pos2(r.right(), fy - 14.0),
        1.0,
        k.line,
    );
    icon(cx.p, Ic::Right, pos2(x0 + 4.0, fy), 12.0, k.text2);
    cx.text(
        pos2(x0 + 16.0, fy),
        Align2::LEFT_CENTER,
        "Patch folder (advanced)",
        12.0,
        "sans",
        k.text2,
    );
}

/// Collapsed inspector rail with four tool buttons.
pub fn rail(cx: &Cx, r: Rect, on: usize) {
    let k = cx.k;
    cx.p.rect_filled(r, 0.0, k.surface);
    cx.line(
        pos2(r.left() + 0.5, r.top()),
        pos2(r.left() + 0.5, r.bottom()),
        if k.dir == Dir::C { 1.5 } else { 1.0 },
        if k.dir == Dir::C { k.line2 } else { k.line },
    );
    for (i, ic) in [Ic::Panel, Ic::Cable, Ic::Link, Ic::Learn]
        .iter()
        .enumerate()
    {
        let c = pos2(r.center().x + 0.5, r.top() + 28.0 + i as f32 * 40.0);
        icon_btn(cx, c, *ic, if i == on { 2 } else { 0 }, true);
    }
}

pub fn status(cx: &Cx, r: Rect, meter_n: (f32, f32)) {
    let k = cx.k;
    cx.p.rect_filled(r, 0.0, if k.dir == Dir::B { k.bg } else { k.surface });
    cx.line(
        pos2(r.left(), r.top() + 0.5),
        pos2(r.right(), r.top() + 0.5),
        if k.dir == Dir::C { 1.5 } else { 1.0 },
        if k.dir == Dir::C { k.line2 } else { k.line },
    );
    let cy = r.center().y;
    let f = if k.dir == Dir::C {
        "cond-semi"
    } else {
        "sans-med"
    };
    cx.text(
        pos2(14.0, cy),
        Align2::LEFT_CENTER,
        if k.dir == Dir::C { "OUT" } else { "Out" },
        12.0,
        f,
        k.text2,
    );
    meter(
        cx,
        Rect::from_min_size(pos2(46.0, cy - 6.0), vec2(96.0, 4.0)),
        meter_n.0,
        true,
    );
    meter(
        cx,
        Rect::from_min_size(pos2(46.0, cy + 1.0), vec2(96.0, 4.0)),
        meter_n.1,
        true,
    );
    let mut x = 160.0;
    for (dot, s) in [
        (k.good, "Audio · 48 kHz · 256"),
        (k.good, "MIDI · KeyLab 49"),
    ] {
        cx.p.circle_filled(pos2(x, cy), 4.0, dot);
        cx.text(pos2(x + 10.0, cy), Align2::LEFT_CENTER, s, 12.0, f, k.text);
        x += 24.0 + cx.width(s, 12.0, f);
    }
    let right = r.right() - 14.0;
    let d = Rect::from_min_size(pos2(right - 118.0, cy - 11.0), vec2(118.0, 22.0));
    cx.rr(d, k.r_sm, a(k.text3, 40));
    icon(cx.p, Ic::Bolt, pos2(d.left() + 14.0, cy), 12.0, k.text2);
    cx.text(
        pos2(d.left() + 28.0, cy),
        Align2::LEFT_CENTER,
        "Diagnostics",
        12.0,
        f,
        k.text2,
    );
    cx.text(
        pos2(d.left() - 14.0, cy),
        Align2::RIGHT_CENTER,
        "4 voices",
        12.0,
        f,
        k.text2,
    );
}

/// Diagnostics popover: the telemetry that used to live in the status bar.
pub fn diagnostics(cx: &Cx, anchor: egui::Pos2) -> Rect {
    let k = cx.k;
    let r = Rect::from_min_size(pos2(anchor.x - 300.0, anchor.y - 214.0), vec2(300.0, 208.0));
    cx.shadow(r, k.r_md, 2);
    cx.rr(r, k.r_md, k.raised);
    cx.rr_stroke(
        r,
        k.r_md,
        if k.dir == Dir::C { 1.5 } else { 1.0 },
        if k.dir == Dir::C { k.line2 } else { k.line },
    );
    let x0 = r.left() + 16.0;
    let mut y = r.top() + 22.0;
    section_label(cx, pos2(x0, y), "Audio engine");
    cx.p.circle_filled(pos2(r.right() - 20.0, y), 4.0, k.good);
    y += 26.0;
    let rows: [(&str, &str); 5] = [
        ("Session", "1 · graph installed"),
        ("Output", "System default · 48 kHz"),
        ("Late callbacks", "0"),
        ("Xruns", "0"),
        ("Real-time priority", "granted (rtkit)"),
    ];
    for (a_, b_) in rows {
        cx.text(pos2(x0, y), Align2::LEFT_CENTER, a_, 12.0, "sans", k.text2);
        cx.text(
            pos2(r.right() - 16.0, y),
            Align2::RIGHT_CENTER,
            b_,
            12.0,
            "mono-med",
            k.text,
        );
        y += 22.0;
    }
    cx.text(
        pos2(x0, y + 4.0),
        Align2::LEFT_CENTER,
        "Callback worst",
        12.0,
        "sans",
        k.text2,
    );
    cx.text(
        pos2(r.right() - 16.0, y + 4.0),
        Align2::RIGHT_CENTER,
        "2.6 ms of 10.7 ms",
        12.0,
        "mono-med",
        k.text,
    );
    let bar = Rect::from_min_size(pos2(x0, y + 18.0), vec2(r.width() - 32.0, 6.0));
    cx.rr(bar, 3.0, k.inset);
    cx.rr(
        Rect::from_min_size(bar.min, vec2(bar.width() * 0.24, 6.0)),
        3.0,
        k.good,
    );
    r
}
