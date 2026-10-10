//! Dense patch: the Composition piece (41 modules, 76 cables) at fit zoom. Below 0.55 zoom
//! faces switch to overview cards (semantic zoom) and cables are dimmed unless they touch
//! the focused module.
use crate::chrome::*;
use crate::displays::*;
use crate::faces::*;
use crate::perform::card;
use crate::prim::*;
use crate::rack::*;
use crate::scenes::Ctxs;
use crate::tokens::{Dir, Face};
use crate::widgets::*;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Shape, Stroke};

fn short(kind: &str, id: u32) -> String {
    let base = match kind {
        "osc.va" => "VA Osc",
        "filter.svf" => "SVF",
        "env.adsr" => "ADSR",
        "lfo" => "LFO",
        "vca" => "VCA",
        "seq" => return if id == 6 { "Bass seq".into() } else { "Arp seq".into() },
        "clock" => "Clock",
        "cues" => "Scenes",
        "macro" => "Macros",
        "mixer" => "Mixer",
        "delay" => "Delay",
        "reverb" => "Reverb",
        "ringmod" => "Ring mod",
        "out" => "Out",
        "midi.in" => "MIDI In",
        "gain" => "Gain",
        o => o,
    };
    base.to_string()
}

fn row_names() -> [&'static str; 6] {
    ["Transport · Scenes · Macros · LFOs", "Sequencers · Ring mods", "Bass voice", "Pad & lead voices", "Mix · Delay · Reverb · Out", "Swell voice"]
}

fn mini(cx: &Cx, kind: &str, r: Rect, p: &std::collections::BTreeMap<String, f32>, id: u32) {
    let k = cx.k;
    let g = |n: &str, d: f32| p.get(n).copied().unwrap_or(d);
    match kind {
        "osc.va" => scope(cx, r, g("waveform", 2.0) as usize, 2.0),
        "filter.svf" => filter_view(cx, r, g("cutoff_hz", 1200.0), g("resonance", 0.3), 0.6, 0),
        "env.adsr" => env_view(cx, r, &Adsr { a_ms: g("attack_ms", 10.0), d_ms: g("decay_ms", 200.0), s: g("sustain", 0.6), r_ms: g("release_ms", 400.0) }),
        "lfo" => lfo_view(cx, r, g("waveform", 0.0) as usize, g("rate_hz", 0.4) * 2.0),
        "vca" | "gain" => {
            let fr = frame(cx, r, (3, 3));
            let lvl = 0.5 + 0.4 * (cx.t * 2.0 + id as f32).sin();
            meter(cx, Rect::from_min_size(fr.left_top() + vec2(2.0, 2.0), vec2(fr.width() - 4.0, fr.height() - 4.0).min(vec2(40.0, fr.height() - 4.0))), lvl, true);
        }
        "seq" => {
            let vals: Vec<f32> = (0..8).map(|i| ((i * 5 + id as usize * 3) % 7) as f32 / 7.0 * 0.8 + 0.1).collect();
            let probs = [1.0, 1.0, 0.8, 1.0, 0.6, 1.0, 0.9, 0.7];
            let step = (cx.t * 3.7) as usize % 8;
            step_lane(cx, r.shrink2(vec2(2.0, 2.0)), &vals, &probs, step, if id == 6 { k.audio } else { k.cv });
        }
        "clock" => {
            let beat = ((cx.t * 112.0 / 60.0) as usize) % 4;
            cx.text(r.center() - vec2(0.0, 7.0), Align2::CENTER_CENTER, "112", 20.0, if k.dir == Dir::C { "cond-semi" } else { "sans-semi" }, k.face("clock").ink);
            for i in 0..4 {
                let c = pos2(r.center().x - 15.0 + i as f32 * 10.0, r.bottom() - 6.0);
                cx.p.circle_filled(c, 3.0, if i == beat { k.accent } else { a(k.face("clock").ink, 70) });
            }
        }
        "cues" => {
            let pw = (r.width() - 4.0 * 3.0) / 5.0;
            for i in 0..5 {
                let pr = Rect::from_min_size(pos2(r.left() + i as f32 * (pw + 3.0), r.top()), vec2(pw, r.height()));
                let on = i == 1;
                cx.rr(pr, 3.0, if on { k.accent } else { a(k.face("cues").ink, 40) });
                if i == 2 {
                    cx.rr_stroke(pr, 3.0, 1.5, k.accent);
                }
            }
        }
        "macro" => {
            let f = k.face("macro");
            for i in 0..4 {
                let c = pos2(r.left() + r.width() * (i as f32 + 0.5) / 4.0, r.center().y);
                knob(cx, c, (r.width() / 9.0).min(r.height() / 2.6).max(6.0), [0.4, 0.5, 0.3, 0.3][i], None, &f, false);
            }
        }
        "mixer" => {
            for i in 0..4 {
                let w = r.width() / 4.0;
                let lv = 0.35 + 0.3 * (cx.t * (1.3 + i as f32 * 0.4)).sin().abs();
                let b = Rect::from_min_size(pos2(r.left() + i as f32 * w + 3.0, r.top()), vec2(w - 6.0, r.height()));
                cx.rr(b, 2.0, a(k.face("mixer").ink, 40));
                cx.rr(Rect::from_min_max(pos2(b.left(), b.bottom() - b.height() * lv), b.max), 2.0, a(k.face("mixer").ink, 200));
            }
        }
        "delay" | "reverb" => {
            let fr = frame(cx, r, (4, 2));
            let n = fr.width() as usize;
            let pts: Vec<Pos2> = (0..=n).map(|i| {
                let x = i as f32 / n as f32;
                let tail = if kind == "delay" { ((x * 22.0).cos().max(0.0)) * (1.0 - x).powi(2) } else { (1.0 - x).powi(2) * (0.7 + 0.3 * (x * 60.0).sin()) };
                pos2(fr.left() + x * fr.width(), fr.bottom() - tail * fr.height() * 0.9)
            }).collect();
            fill_under(cx, &pts, fr.bottom(), k.disp_trace, 70);
            trace(cx, pts, k.disp_trace, 1.6);
        }
        "ringmod" => {
            cx.text(r.center(), Align2::CENTER_CENTER, "×", 26.0, "sans-med", k.face("ringmod").ink);
        }
        "out" => {
            let c = r.center();
            for rr in [0.9, 0.6, 0.3] {
                cx.p.circle_stroke(c, r.height().min(r.width()) * 0.5 * rr, Stroke::new(1.5, a(k.face("out").ink, 150)));
            }
        }
        _ => {}
    }
}

pub fn composition(cx: &Cx, d: &Ctxs, size: egui::Vec2, focus: u32) {
    let k = cx.k;
    let narrow = size.x < 1360.0;
    let outline_w = if narrow { 214.0 } else { 236.0 };
    let insp_w = if narrow { 288.0 } else { 312.0 };
    let ar = areas(k.dir, size, false, insp_w);
    toolbar(cx, ar.toolbar, "Rack", "Composition", false, "", (false, true), 1);
    status(cx, ar.status, (0.64 + 0.16 * (cx.t * 3.1).sin(), 0.6 + 0.14 * (cx.t * 2.3).sin()));
    let rk = &d.comp;
    let left = Rect::from_min_max(ar.center.min, pos2(ar.center.min.x + outline_w, ar.center.max.y));
    let cen = Rect::from_min_max(pos2(left.right(), ar.center.min.y), ar.center.max);
    cx.p.rect_filled(cen, 0.0, k.rack);
    let world = rk.world().expand2(vec2(10.0, 18.0));
    let xf = fit(world, cen, vec2(8.0, 4.0), 1.0);
    // Rails behind the cards.
    rk.rails(cx, xf, world.left(), world.right());
    let neigh: Vec<u32> = rk.cabs.iter().filter_map(|c| if c.from.0 == focus { Some(c.to.0) } else if c.to.0 == focus { Some(c.from.0) } else { None }).collect();
    for m in &rk.lay.mods {
        let id = m.id as u32;
        let sm = &rk.st.modules[&m.id];
        let org = m.face.min;
        let fr = Rect::from_min_size(org, vec2(m.face.width(), 340.0));
        let face = k.face(m.info.kind);
        draw_body(cx, xf, fr, &face, id == focus);
        let sr = xf.r(fr);
        let nm = short(m.info.kind, id);
        let (tf, ts) = match k.dir {
            Dir::C => ("cond-semi", 13.5),
            _ => ("sans-semi", 12.5),
        };
        let nm_s = if k.dir == Dir::C { nm.to_uppercase() } else { nm.clone() };
        let t = cx.fit(&nm_s, ts, tf, sr.width() - 10.0);
        cx.text(pos2(sr.center().x, sr.top() + 14.0), Align2::CENTER_CENTER, &t, ts, tf, face.ink);
        if sr.width() > 80.0 {
            cx.text(pos2(sr.center().x, sr.top() + 28.0), Align2::CENTER_CENTER, &format!("#{id}"), 11.0, "mono", face.ink2);
        }
        let disp = Rect::from_min_max(pos2(sr.left() + 5.0, sr.top() + if sr.width() > 80.0 { 38.0 } else { 28.0 }), pos2(sr.right() - 5.0, sr.top() + sr.height() * 0.5));
        mini(cx, m.info.kind, disp, &sm.params, id);
        // Connected jacks only.
        let g = geom(m.info.kind, m);
        for j in &g.jacks {
            if rk.conn(id, j.name, j.out) {
                let ty = m.info.ports.iter().find(|p| p.name == j.name).map(|p| p.port_type);
                let c = xf.p(org + j.c.to_vec2());
                cx.p.circle_filled(c, 4.2, mix(face.base, Color32::BLACK, 0.55));
                cx.p.circle_filled(c, 2.6, ty.map_or(k.audio, |t| k.signal(t)));
            }
        }
        if id != focus && !neigh.contains(&id) {
            cx.p.rect_filled(sr, 3.0, a(if k.dark { k.rack } else { k.bg }, 100));
        }
    }
    // Cables: dim unless they touch the focus.
    let mut touched = vec![];
    for (i, c) in rk.cabs.iter().enumerate() {
        let (Some(p0), Some(p1)) = (rk.jack_pos(c.from.0, &c.from.1, true), rk.jack_pos(c.to.0, &c.to.1, false)) else { continue };
        let ty = rk.lay.mods.iter().find(|m| m.id as u32 == c.from.0).and_then(|m| m.info.ports.iter().find(|p| p.name == c.from.1)).map(|p| p.port_type);
        let col = ty.map_or(k.audio, |t| k.signal(t));
        let pts = cable_pts(xf.p(p0), xf.p(p1), 40.0 * xf.s + 8.0);
        let touch = c.from.0 == focus || c.to.0 == focus;
        if touch {
            touched.push((i, pts, col));
            continue;
        }
        let w = 1.3;
        match k.dir {
            Dir::C => {
                cx.p.add(Shape::line(pts.clone(), Stroke::new(w + 1.4, a(k.line2, 50))));
                cx.p.add(Shape::line(pts, Stroke::new(w, a(col, 100))));
            }
            _ => {
                cx.p.add(Shape::line(pts, Stroke::new(w, a(col, 80))));
            }
        }
    }
    let mut num = 0;
    let mut list: Vec<(usize, Color32, bool, u32, String, String)> = vec![];
    for (i, pts, col) in &touched {
        let c = &rk.cabs[*i];
        let inbound = c.to.0 == focus;
        match k.dir {
            Dir::B => cx.glow_line(pts.clone(), 2.6, *col, 1.0),
            Dir::C => {
                cx.p.add(Shape::line(pts.clone(), Stroke::new(6.0, k.line2)));
                cx.p.add(Shape::line(pts.clone(), Stroke::new(3.6, *col)));
            }
            Dir::A => {
                cx.p.add(Shape::line(pts.iter().map(|q| *q + vec2(0.0, 1.5)).collect(), Stroke::new(4.4, Color32::from_black_alpha(70))));
                cx.p.add(Shape::line(pts.clone(), Stroke::new(3.6, *col)));
            }
        }
        let ph = (cx.t * 0.5 + *i as f32 * 0.21).fract();
        let len = poly_len(pts);
        for q in 0..3 {
            let t = (ph + q as f32 / 3.0).fract();
            let t = if inbound { 1.0 - t } else { t };
            let _ = len;
            cx.p.circle_filled(along(pts, t), 2.8, Color32::WHITE);
        }
        num += 1;
        let peer = if inbound { c.from.clone() } else { c.to.clone() };
        let end = if inbound { pts[0] } else { *pts.last().unwrap() };
        let badge = end + vec2(0.0, if inbound { 15.0 } else { 15.0 });
        cx.p.circle_filled(badge, 8.0, k.accent);
        cx.text(badge, Align2::CENTER_CENTER, &num.to_string(), 11.0, "sans-semi", k.on_accent);
        let me_port = if inbound { c.to.1.clone() } else { c.from.1.clone() };
        let peer_mod = rk.lay.mods.iter().find(|m| m.id as u32 == peer.0).map(|m| short(m.info.kind, peer.0)).unwrap_or_default();
        list.push((num, *col, inbound, peer.0, format!("{peer_mod} #{}", peer.0), format!("{} {} {}", me_port, if inbound { "←" } else { "→" }, peer.1)));
    }
    // Outline (left).
    panel_bg(cx, left, true);
    heading(cx, pos2(left.left() + 16.0, left.top() + 26.0), "Outline");
    field(cx, Rect::from_min_size(pos2(left.left() + 12.0, left.top() + 48.0), vec2(left.width() - 24.0, 32.0)), "Find module", "", false);
    let mut y = left.top() + 104.0;
    section_label(cx, pos2(left.left() + 16.0, y), "Rows");
    y += 20.0;
    for (i, n) in row_names().iter().enumerate() {
        let r = Rect::from_min_size(pos2(left.left() + 8.0, y), vec2(left.width() - 16.0, 44.0));
        let on = i == 1;
        if on {
            cx.rr(r, k.r_md, k.accent_soft);
        }
        let cnt = rk.lay.mods.iter().filter(|m| m.row == i).count();
        cx.text(pos2(r.left() + 12.0, r.top() + 15.0), Align2::LEFT_CENTER, &format!("Row {}", i + 1), 12.5, if k.dir == Dir::C { "cond-semi" } else { "sans-semi" }, k.text);
        cx.text(pos2(r.right() - 10.0, r.top() + 15.0), Align2::RIGHT_CENTER, &format!("{cnt}"), 12.0, "mono-med", k.text2);
        let t = cx.fit(n, 11.5, "sans", r.width() - 24.0);
        cx.text(pos2(r.left() + 12.0, r.top() + 32.0), Align2::LEFT_CENTER, &t, 11.5, "sans", k.text2);
        y += 48.0;
    }
    y += 10.0;
    section_label(cx, pos2(left.left() + 16.0, y), "Cables");
    y += 22.0;
    for (n, col, l) in [("Audio", k.audio, "sound"), ("Control", k.cv, "modulation, pitch"), ("Gate", k.gate, "triggers, clock")] {
        cx.p.circle_filled(pos2(left.left() + 24.0, y), 5.0, col);
        cx.text(pos2(left.left() + 38.0, y), Align2::LEFT_CENTER, n, 12.5, "sans-semi", k.text);
        cx.text(pos2(left.right() - 14.0, y), Align2::RIGHT_CENTER, l, 11.0, "sans", k.text2);
        y += 24.0;
    }
    cx.text(pos2(left.left() + 16.0, y + 8.0), Align2::LEFT_CENTER, "Only cables on the selected module are lit.", 11.0, "sans", k.text2);
    // Inspector.
    let ins = ar.right;
    panel_bg(cx, ins, false);
    let fm = rk.lay.mods.iter().find(|m| m.id as u32 == focus).unwrap();
    let x0 = ins.left() + 16.0;
    let mut y = ins.top() + 28.0;
    heading(cx, pos2(x0, y), &short(fm.info.kind, focus));
    cx.text(pos2(ins.right() - 16.0, y), Align2::RIGHT_CENTER, &format!("{} · #{}", fm.info.kind, focus), 12.0, "mono", k.text2);
    y += 34.0;
    for (i, s) in ["Inspect", "Routing", "Compare", "Learn"].iter().enumerate() {
        let w = (ins.width() - 32.0 - 9.0) / 4.0;
        chip(cx, Rect::from_min_size(pos2(x0 + i as f32 * (w + 3.0), y - 13.0), vec2(w, 28.0)), s, if i == 1 { 2 } else { 0 }, None);
    }
    y += 34.0;
    let nin = list.iter().filter(|l| l.2).count();
    let rows = |cx: &Cx, y: &mut f32, title: &str, inbound: bool| {
        section_label(cx, pos2(x0, *y), title);
        *y += 20.0;
        for (n, col, inb, _, who, ports) in list.iter().filter(|l| l.2 == inbound) {
            let _ = inb;
            let r = Rect::from_min_size(pos2(ins.left() + 8.0, *y - 13.0), vec2(ins.width() - 16.0, 36.0));
            if k.dir != Dir::C {
                cx.rr(r, k.r_sm + 1.0, a(k.text3, 22));
            } else {
                cx.line(r.left_bottom(), r.right_bottom(), 1.0, k.line);
            }
            cx.p.circle_filled(pos2(r.left() + 14.0, r.center().y), 8.0, k.accent);
            cx.text(pos2(r.left() + 14.0, r.center().y), Align2::CENTER_CENTER, &n.to_string(), 11.0, "sans-semi", k.on_accent);
            cx.p.circle_filled(pos2(r.left() + 32.0, r.center().y), 4.0, *col);
            cx.text(pos2(r.left() + 44.0, r.center().y - 7.0), Align2::LEFT_CENTER, &cx.fit(who, 12.5, "sans-semi", r.width() - 54.0), 12.5, "sans-semi", k.text);
            cx.text(pos2(r.left() + 44.0, r.center().y + 8.0), Align2::LEFT_CENTER, &cx.fit(ports, 11.5, "mono", r.width() - 54.0), 11.5, "mono", k.text2);
            *y += 40.0;
        }
        *y += 6.0;
    };
    let max_rows = ((ins.bottom() - y - 220.0) / 40.0) as usize;
    let _ = (max_rows, nin);
    rows(cx, &mut y, "Receives", true);
    rows(cx, &mut y, "Sends", false);
    y += 6.0;
    cx.text(pos2(x0, y + 4.0), Align2::LEFT_CENTER, &format!("{} other cables dimmed", rk.cabs.len() - touched.len()), 12.0, "sans", k.text2);
    chip(cx, Rect::from_min_size(pos2(ins.right() - 16.0 - 96.0, y - 10.0), vec2(96.0, 28.0)), "Show all", 0, Some(Ic::Eye));
    // Pattern lens.
    let lens = Rect::from_min_max(pos2(ins.left() + 12.0, ins.bottom() - 150.0), pos2(ins.right() - 12.0, ins.bottom() - 12.0));
    if lens.top() > y {
        let inner = card(cx, lens, "Pattern · bank A", "playing");
        let vals = [0.7, 0.0, 0.5, 0.5, 0.0, 0.7, 0.4, 0.0];
        let probs = [1.0, 0.0, 0.9, 1.0, 0.0, 0.75, 0.6, 0.0];
        step_lane(cx, Rect::from_min_size(inner.min + vec2(0.0, 6.0), vec2(inner.width(), 44.0)), &vals, &probs, (cx.t * 3.7) as usize % 8, k.audio);
    }
    let _ = Face { base: k.bg, ink: k.text, ink2: k.text2 };
}
