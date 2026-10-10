//! Perform view: transport, scene cues, macros, bank launchers and live step lanes.
use crate::chrome::*;
use crate::displays::*;
use crate::prim::*;
use crate::tokens::{Dir, Face};
use crate::widgets::*;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};

pub fn card(cx: &Cx, r: Rect, title: &str, sub: &str) -> Rect {
    let k = cx.k;
    match k.dir {
        Dir::A => {
            cx.shadow(r, k.r_lg, 1);
            cx.rr(r, k.r_lg, k.surface);
            cx.rr_stroke(r, k.r_lg, 1.0, k.line);
        }
        Dir::B => {
            cx.shadow(r, k.r_lg, 1);
            cx.grad(
                r,
                k.r_lg,
                mix(k.surface, Color32::WHITE, if k.dark { 0.03 } else { 0.0 }),
                mix(k.surface, Color32::BLACK, if k.dark { 0.2 } else { 0.03 }),
            );
            cx.rr_stroke(r, k.r_lg, 1.0, k.line);
        }
        Dir::C => {
            cx.rr(r, k.r_lg, k.surface);
            cx.rr_stroke(r, k.r_lg, 1.6, k.line2);
        }
    }
    section_label(cx, pos2(r.left() + 16.0, r.top() + 20.0), title);
    if !sub.is_empty() {
        cx.text(
            pos2(r.right() - 16.0, r.top() + 20.0),
            Align2::RIGHT_CENTER,
            sub,
            12.0,
            "sans",
            k.text2,
        );
    }
    Rect::from_min_max(
        pos2(r.left() + 16.0, r.top() + 38.0),
        pos2(r.right() - 16.0, r.bottom() - 14.0),
    )
}

fn pad(cx: &Cx, r: Rect, name: &str, state: u8, prog: f32, hint: &str) {
    let k = cx.k;
    let pulse =
        0.5 + 0.5 * (cx.t * std::f32::consts::TAU * 1000.0 / k.motion.pulse_ms as f32).sin();
    let rad = if k.dir == Dir::B { 14.0 } else { k.r_md };
    match (k.dir, state) {
        (Dir::C, 1) => cx.rr(r, 2.0, k.text),
        (Dir::C, _) => {
            cx.rr(r, 2.0, k.inset);
            cx.rr_stroke(r, 2.0, 1.5, k.line2);
        }
        (Dir::B, 1) => {
            cx.p.rect_filled(r.expand(4.0), egui::CornerRadius::same(18), a(k.accent, 40));
            cx.grad(
                r,
                rad,
                mix(k.accent, Color32::WHITE, 0.15),
                mix(k.accent, Color32::BLACK, 0.2),
            );
        }
        (Dir::B, _) => {
            cx.grad(
                r,
                rad,
                mix(k.raised, Color32::WHITE, 0.03),
                mix(k.raised, Color32::BLACK, 0.15),
            );
            cx.rr_stroke(r, rad, 1.0, k.line);
        }
        (_, 1) => {
            cx.shadow(r, rad, 1);
            cx.rr(r, rad, k.accent);
        }
        _ => {
            cx.rr(r, rad, k.raised);
            cx.rr_stroke(r, rad, 1.0, k.line);
        }
    }
    if state == 2 {
        let col = if k.dir == Dir::C { k.text } else { k.accent };
        if k.dir == Dir::B {
            cx.p.rect_filled(
                r.expand(3.0 * pulse),
                egui::CornerRadius::same(17),
                a(k.accent, (50.0 * pulse) as u8),
            );
        }
        cx.rr_stroke(r, rad, 1.5 + 1.5 * pulse, col);
    }
    let on_ink = match (k.dir, state) {
        (Dir::C, 1) => k.surface,
        (_, 1) => k.on_accent,
        _ => k.text,
    };
    let (f, sz) = if k.dir == Dir::C {
        ("cond-semi", 20.0)
    } else {
        ("sans-semi", 17.0)
    };
    let nm = if k.dir == Dir::C {
        name.to_uppercase()
    } else {
        name.to_string()
    };
    cx.text(
        r.center() - vec2(0.0, 6.0),
        Align2::CENTER_CENTER,
        &nm,
        sz,
        f,
        on_ink,
    );
    let st = match state {
        1 => "PLAYING",
        2 => "NEXT BAR",
        _ => "",
    };
    if !st.is_empty() {
        cx.caps(
            r.center() + vec2(0.0, 16.0),
            Align2::CENTER_CENTER,
            st,
            11.0,
            if k.dir == Dir::C {
                "cond-semi"
            } else {
                "sans-semi"
            },
            if state == 1 { a(on_ink, 220) } else { k.text2 },
            1.1,
        );
    }
    cx.text(
        r.left_top() + vec2(10.0, 11.0),
        Align2::LEFT_CENTER,
        hint,
        11.0,
        "mono",
        if state == 1 { a(on_ink, 200) } else { k.text2 },
    );
    if state == 1 {
        let bar = Rect::from_min_size(
            pos2(r.left() + 10.0, r.bottom() - 10.0),
            vec2(r.width() - 20.0, 3.0),
        );
        cx.rr(bar, 1.5, a(on_ink, 60));
        cx.rr(
            Rect::from_min_size(bar.min, vec2(bar.width() * prog, 3.0)),
            1.5,
            on_ink,
        );
    }
}

fn chips_row(
    cx: &Cx,
    origin: Pos2,
    labels: &[&str],
    playing: usize,
    queued: Option<usize>,
    w: f32,
) -> f32 {
    let k = cx.k;
    let pulse = 0.5 + 0.5 * (cx.t * 6.0).sin();
    let mut x = origin.x;
    for (i, l) in labels.iter().enumerate() {
        let r = Rect::from_min_size(pos2(x, origin.y), vec2(w, 34.0));
        let on = i == playing;
        let q = queued == Some(i);
        let rad = if k.dir == Dir::B { 17.0 } else { k.r_sm + 1.0 };
        cx.rr(
            r,
            rad,
            if on {
                if k.dir == Dir::C {
                    k.text
                } else {
                    k.accent
                }
            } else {
                k.inset
            },
        );
        cx.rr_stroke(
            r,
            rad,
            if q { 1.5 + 1.5 * pulse } else { 1.0 },
            if q {
                if k.dir == Dir::C {
                    k.text
                } else {
                    k.accent
                }
            } else {
                k.line
            },
        );
        let ink = if on {
            if k.dir == Dir::C {
                k.surface
            } else {
                k.on_accent
            }
        } else {
            k.text
        };
        cx.text(
            r.center(),
            Align2::CENTER_CENTER,
            l,
            14.0,
            if k.dir == Dir::C {
                "cond-semi"
            } else {
                "sans-semi"
            },
            ink,
        );
        x += w + 6.0;
    }
    x
}

fn macro_knob(
    cx: &Cx,
    c: Pos2,
    r: f32,
    name: &str,
    n: f32,
    m: Option<ModVis>,
    cc: u32,
    dest: &str,
    shown: &str,
) {
    let k = cx.k;
    let f = Face {
        base: k.surface,
        ink: k.text,
        ink2: k.text2,
    };
    knob(cx, c, r, n, m, &f, false);
    cx.text(
        c - vec2(0.0, r + 28.0),
        Align2::CENTER_CENTER,
        name,
        15.0,
        if k.dir == Dir::C {
            "cond-semi"
        } else {
            "sans-semi"
        },
        k.text,
    );
    cx.text(
        c + vec2(0.0, r + 18.0),
        Align2::CENTER_CENTER,
        shown,
        14.0,
        "mono-med",
        k.text,
    );
    cx.text(
        c + vec2(0.0, r + 38.0),
        Align2::CENTER_CENTER,
        dest,
        11.0,
        "sans",
        k.text2,
    );
    let b = Rect::from_center_size(c + vec2(0.0, r + 58.0), vec2(52.0, 18.0));
    cx.rr(b, 3.0, a(k.text3, 40));
    cx.text(
        b.center(),
        Align2::CENTER_CENTER,
        &format!("CC {cc}"),
        11.0,
        "mono-med",
        k.text2,
    );
}

pub fn perform(cx: &Cx, size: egui::Vec2) {
    let k = cx.k;
    let ar = areas(k.dir, size, false, 44.0);
    toolbar(
        cx,
        ar.toolbar,
        "Perform",
        "Composition",
        false,
        "",
        (false, false),
        usize::MAX,
    );
    rail(cx, ar.right, 0);
    status(
        cx,
        ar.status,
        (
            0.7 + 0.15 * (cx.t * 3.1).sin(),
            0.66 + 0.12 * (cx.t * 2.3).sin(),
        ),
    );
    cx.p.rect_filled(ar.center, 0.0, k.bg);
    let c = ar.center.shrink(16.0);
    let gap = 14.0;
    let big = size.x > 1360.0;
    let h1 = c.height() * 0.28;
    let h2 = c.height() * 0.48;
    let h3 = c.height() - h1 - h2 - gap * 2.0;
    let beats = cx.t * 112.0 / 60.0;
    let bar = (beats / 4.0).floor() as i32 + 12;
    let beat = beats % 4.0;
    // Transport.
    let tw = if big { 330.0 } else { 290.0 };
    let tr = Rect::from_min_size(c.min, vec2(tw, h1));
    let inner = card(cx, tr, "Transport", "Clock #1");
    let (df, ds) = match k.dir {
        Dir::A => ("sans-semi", 54.0),
        Dir::B => ("mono-med", 50.0),
        Dir::C => ("cond-semi", 60.0),
    };
    cx.text(
        pos2(inner.left(), inner.top() + 34.0),
        Align2::LEFT_CENTER,
        "112",
        ds,
        df,
        k.text,
    );
    cx.text(
        pos2(
            inner.left() + cx.width("112", ds, df) + 8.0,
            inner.top() + 46.0,
        ),
        Align2::LEFT_CENTER,
        "bpm",
        14.0,
        "sans-med",
        k.text2,
    );
    cx.text(
        pos2(inner.right(), inner.top() + 22.0),
        Align2::RIGHT_CENTER,
        &format!("BAR {bar}"),
        13.0,
        "mono-med",
        k.text2,
    );
    for i in 0..4 {
        let on = beat as usize == i;
        let p = pos2(inner.right() - 54.0 + i as f32 * 16.0, inner.top() + 44.0);
        if on {
            cx.glow_dot(
                p,
                4.0,
                if k.dir == Dir::C { k.accent } else { k.accent },
                0.9,
            );
        } else {
            cx.p.circle_filled(p, 4.0, k.line2.gamma_multiply(0.6));
        }
    }
    let prog = Rect::from_min_size(
        pos2(inner.left(), inner.bottom() - 70.0),
        vec2(inner.width(), 6.0),
    );
    for i in 0..4 {
        let seg = Rect::from_min_size(
            prog.min + vec2(i as f32 * (prog.width() / 4.0), 0.0),
            vec2(prog.width() / 4.0 - 4.0, 6.0),
        );
        cx.rr(seg, 3.0, k.inset);
        let f = (beats % 4.0 - i as f32).clamp(0.0, 1.0);
        cx.rr(
            Rect::from_min_size(seg.min, vec2(seg.width() * f, 6.0)),
            3.0,
            k.accent,
        );
    }
    let by = inner.bottom() - 18.0;
    button(
        cx,
        Rect::from_min_size(pos2(inner.left(), by - 18.0), vec2(116.0, 36.0)),
        "Running",
        Some(Ic::Play),
        false,
    );
    chip(
        cx,
        Rect::from_min_size(pos2(inner.left() + 126.0, by - 17.0), vec2(104.0, 34.0)),
        "Restart",
        0,
        None,
    );
    // Cues.
    let cr = Rect::from_min_max(
        pos2(tr.right() + gap, c.top()),
        pos2(c.right(), c.top() + h1),
    );
    let inner = card(cx, cr, "Scenes", "queued cues launch on the next bar");
    let names = ["Intro", "Main", "Variation", "Breakdown", "Return"];
    let hints = ["CC 40", "CC 41", "CC 42", "CC 43", "CC 44"];
    let n = 5usize;
    let cw = (inner.width() - 6.0 * n as f32 - 90.0) / n as f32;
    let (playing, queued) = if cx.t.rem_euclid(8.0) < 4.4 {
        (1, Some(2))
    } else {
        (2, None)
    };
    for i in 0..n {
        let r = Rect::from_min_size(
            pos2(inner.left() + i as f32 * (cw + 6.0), inner.top()),
            vec2(cw, inner.height()),
        );
        let st = if i == playing {
            1
        } else if queued == Some(i) {
            2
        } else {
            0
        };
        pad(cx, r, names[i], st, (beats % 4.0) / 4.0, hints[i]);
    }
    let cancel = Rect::from_min_size(
        pos2(inner.right() - 84.0, inner.top()),
        vec2(84.0, inner.height()),
    );
    cx.rr(cancel, k.r_md, a(k.text3, 28));
    cx.text(
        cancel.center(),
        Align2::CENTER_CENTER,
        "Cancel",
        13.0,
        "sans-med",
        if queued.is_some() { k.text } else { k.text3 },
    );
    // Macros.
    let y2 = c.top() + h1 + gap;
    let mw = c.width() * 0.52;
    let mr = Rect::from_min_size(pos2(c.left(), y2), vec2(mw, h2));
    let inner = card(
        cx,
        mr,
        "Macros",
        "named controls · turn to hear, hover to see what moves",
    );
    let r = (h2 * 0.16).min(44.0);
    let mvals = [
        ("Energy", 0.40, 20, "Filter · Seq level · Delay", None),
        ("Motion", 0.50, 21, "LFO rate · Filter · Pan", Some(())),
        ("Space", 0.30, 22, "Reverb mix · Delay fb", None),
        ("Glow", 0.30, 23, "Osc detune · Ring mod", None),
    ];
    for (i, (nm, v, cc, dest, mv)) in mvals.iter().enumerate() {
        let cxp = pos2(
            inner.left() + inner.width() * (i as f32 + 0.5) / 4.0,
            inner.top() + 46.0 + r + 6.0,
        );
        let sweep = 0.5 + 0.5 * (cx.t * 0.9).sin();
        let m = mv.map(|_| ModVis {
            lo: 0.25,
            hi: 0.85,
            cur: 0.25 + 0.6 * sweep,
        });
        let shown = if mv.is_some() {
            format!("{:.2}", 0.25 + 0.6 * sweep)
        } else {
            format!("{v:.2}")
        };
        macro_knob(cx, cxp, r, nm, *v, m, *cc, dest, &shown);
    }
    // Sequencer lanes.
    let sr = Rect::from_min_max(pos2(mr.right() + gap, y2), pos2(c.right(), y2 + h2));
    let inner = card(cx, sr, "Sequences", "step lanes · live");
    let step = (beats * 2.0) as usize;
    let lane_h = ((inner.height() - 52.0) / 2.0 - 40.0).clamp(56.0, 110.0);
    let lane = |cx: &Cx,
                y: f32,
                title: &str,
                vals: &[f32],
                probs: &[f32],
                col: Color32,
                sp: usize,
                bank: usize| {
        cx.text(
            pos2(inner.left(), y),
            Align2::LEFT_CENTER,
            title,
            14.0,
            if cx.k.dir == Dir::C {
                "cond-semi"
            } else {
                "sans-semi"
            },
            cx.k.text,
        );
        chips_row(
            cx,
            pos2(inner.right() - 4.0 * 36.0 + 6.0, y - 17.0),
            &["A", "B", "C", "D"],
            bank,
            if bank == 0 { Some(1) } else { None },
            30.0,
        );
        step_lane(
            cx,
            Rect::from_min_size(pos2(inner.left(), y + 18.0), vec2(inner.width(), lane_h)),
            vals,
            probs,
            sp,
            col,
        );
    };
    let bass = [0.7, 0.0, 0.5, 0.5, 0.0, 0.7, 0.4, 0.0];
    let arp = [0.4, 0.55, 0.7, 0.55, 0.85, 0.55, 0.7, 0.4];
    let pb = [1.0, 0.0, 0.9, 1.0, 0.0, 0.75, 0.6, 0.0];
    let pa = [1.0, 1.0, 0.9, 0.9, 1.0, 0.6, 0.8, 0.5];
    lane(
        cx,
        inner.top() + 22.0,
        "Bass",
        &bass,
        &pb,
        k.audio,
        step % 8,
        0,
    );
    lane(
        cx,
        inner.top() + 22.0 + lane_h + 52.0,
        "Arp",
        &arp,
        &pa,
        k.cv,
        (step + 3) % 8,
        1,
    );
    cx.text(
        pos2(inner.left(), inner.bottom() - 4.0),
        Align2::LEFT_CENTER,
        "bar height = level · bar brightness = probability",
        11.0,
        "sans",
        k.text2,
    );
    // Bottom row.
    let y3 = y2 + h2 + gap;
    let w3 = (c.width() - gap * 2.0) / 3.0;
    let r1 = Rect::from_min_size(pos2(c.left(), y3), vec2(w3, h3));
    let inner = card(cx, r1, "Lead level", "Mixer #32 · CC 24");
    slider(
        cx,
        Rect::from_min_size(
            pos2(inner.left(), inner.center().y - 9.0),
            vec2(inner.width() - 160.0, 18.0),
        ),
        0.25,
    );
    cx.text(
        pos2(inner.right() - 74.0, inner.center().y),
        Align2::RIGHT_CENTER,
        "0.25",
        20.0,
        "mono-med",
        k.text,
    );
    meter(
        cx,
        Rect::from_min_size(
            pos2(inner.right() - 56.0, inner.center().y - 8.0),
            vec2(56.0, 16.0),
        ),
        0.3 + 0.12 * (cx.t * 4.0).sin(),
        true,
    );
    let r2 = Rect::from_min_size(pos2(c.left() + w3 + gap, y3), vec2(w3, h3));
    let inner = card(cx, r2, "Bass transpose", "Sequencer #6");
    icon_btn(
        cx,
        pos2(inner.left() + 18.0, inner.center().y),
        Ic::Minus,
        0,
        true,
    );
    cx.text(
        pos2(inner.left() + 70.0, inner.center().y),
        Align2::CENTER_CENTER,
        "+0 st",
        22.0,
        "mono-med",
        k.text,
    );
    icon_btn(
        cx,
        pos2(inner.left() + 122.0, inner.center().y),
        Ic::Plus,
        0,
        true,
    );
    chip(
        cx,
        Rect::from_min_size(
            pos2(inner.right() - 84.0, inner.center().y - 14.0),
            vec2(84.0, 28.0),
        ),
        "Learn",
        0,
        Some(Ic::Learn),
    );
    let r3 = Rect::from_min_size(pos2(c.left() + (w3 + gap) * 2.0, y3), vec2(w3, h3));
    let inner = card(cx, r3, "Arp direction", "Sequencer #7");
    segmented(
        cx,
        Rect::from_min_size(
            pos2(inner.left(), inner.center().y - 16.0),
            vec2(inner.width(), 34.0),
        ),
        &["FWD", "REV", "PEND"],
        0,
        &Face {
            base: k.inset,
            ink: k.text,
            ink2: k.text2,
        },
        13.0,
    );
}
