//! Live displays: scope, envelope curve, filter response, LFO, wavetable, step lane.
//! Everything is a pure function of time so a recording is reproducible.
use crate::prim::*;
use crate::tokens::Dir;
use egui::epaint::Mesh;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Shape, Stroke};
use std::f32::consts::TAU;

/// Display bezel and graticule. Returns the inner plot rect.
pub fn frame(cx: &Cx, r: Rect, grid: (usize, usize)) -> Rect {
    let k = cx.k;
    let rad = match k.dir {
        Dir::A => 4.0,
        Dir::B => 8.0,
        Dir::C => 2.0,
    };
    if k.dir == Dir::B {
        cx.p.rect_filled(r.expand(1.5), egui::CornerRadius::same(10), a(k.cv, 16));
    }
    cx.rr(r, rad, k.disp_bg);
    if k.dir == Dir::A {
        cx.grad(Rect::from_min_size(r.min, vec2(r.width(), 9.0)), rad, Color32::from_black_alpha(90), Color32::from_black_alpha(0));
    }
    let inner = r.shrink2(vec2(6.0, 6.0));
    let gc = a(k.disp_grid, if k.dir == Dir::C { 255 } else { 190 });
    for i in 1..grid.0 {
        let x = inner.left() + inner.width() * i as f32 / grid.0 as f32;
        cx.line(pos2(x, inner.top()), pos2(x, inner.bottom()), 1.0, gc);
    }
    for i in 1..grid.1 {
        let y = inner.top() + inner.height() * i as f32 / grid.1 as f32;
        cx.line(pos2(inner.left(), y), pos2(inner.right(), y), 1.0, gc);
    }
    match k.dir {
        Dir::A => cx.rr_stroke(r, rad, 1.0, a(Color32::WHITE, if k.dark { 28 } else { 60 })),
        Dir::B => cx.rr_stroke(r, rad, 1.0, a(k.cv, 60)),
        Dir::C => cx.rr_stroke(r, rad, 1.6, k.line2),
    }
    inner
}

pub fn trace(cx: &Cx, pts: Vec<Pos2>, col: Color32, w: f32) {
    match cx.k.dir {
        Dir::C => {
            cx.p.add(Shape::line(pts, Stroke::new(w * 1.1, col)));
        }
        Dir::A => cx.glow_line(pts, w, col, 0.45),
        Dir::B => cx.glow_line(pts, w, col, 1.0),
    }
}

pub fn fill_under(cx: &Cx, pts: &[Pos2], bottom: f32, col: Color32, alpha: u8) {
    if cx.k.dir == Dir::C {
        let mut m = Mesh::default();
        for q in pts {
            m.colored_vertex(*q, a(col, alpha / 2));
            m.colored_vertex(pos2(q.x, bottom), a(col, alpha / 2));
        }
        for i in 0..(pts.len() as u32 - 1) {
            let b = i * 2;
            m.add_triangle(b, b + 1, b + 2);
            m.add_triangle(b + 1, b + 3, b + 2);
        }
        cx.p.add(Shape::mesh(m));
        return;
    }
    let mut m = Mesh::default();
    for q in pts {
        m.colored_vertex(*q, a(col, alpha));
        m.colored_vertex(pos2(q.x, bottom), a(col, 0));
    }
    for i in 0..(pts.len() as u32 - 1) {
        let b = i * 2;
        m.add_triangle(b, b + 1, b + 2);
        m.add_triangle(b + 1, b + 3, b + 2);
    }
    cx.p.add(Shape::mesh(m));
}

pub fn wave(kind: usize, ph: f32) -> f32 {
    let p = ph - ph.floor();
    match kind {
        0 => (p * TAU).sin(),
        1 => tri(p + 0.25),
        2 => 1.0 - 2.0 * p,
        _ => {
            if p < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
    }
}

/// Gate cycle shared by every envelope-driven display: 4 s, held for 2.4 s.
pub fn gate(t: f32) -> (bool, f32) {
    let ph = t.rem_euclid(4.0);
    (ph < 2.4, ph)
}

/// Oscilloscope. `kind` 0..3 as in the osc waveform selector.
pub fn scope(cx: &Cx, r: Rect, kind: usize, cycles: f32) {
    let k = cx.k;
    let p = frame(cx, r, (8, 4));
    let mid = p.center().y;
    cx.line(pos2(p.left(), mid), pos2(p.right(), mid), 1.0, a(k.disp_grid, 255));
    let amp = p.height() * 0.42 * (0.78 + 0.22 * (cx.t * 0.9).sin());
    let cyc = cycles + 0.35 * (cx.t * 0.5).sin();
    let n = p.width() as usize;
    for (lag, alpha) in [(0.05, 0.22), (0.025, 0.45), (0.0, 1.0)] {
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| {
                let x = i as f32 / n as f32;
                let mut y = wave(kind, x * cyc - lag * 2.0);
                if kind == 2 || kind == 3 {
                    y *= 0.92;
                }
                pos2(p.left() + x * p.width(), mid - y * amp)
            })
            .collect();
        trace(cx, pts, a(k.disp_trace, (255.0 * alpha) as u8), 1.7);
    }
}

pub fn lfo_view(cx: &Cx, r: Rect, kind: usize, hz: f32) {
    let k = cx.k;
    let p = frame(cx, r, (4, 2));
    let mid = p.center().y;
    let amp = p.height() * 0.38;
    let cyc = 1.5;
    let n = p.width() as usize;
    let sh = |ph: f32| -> f32 {
        match kind {
            4 => {
                let s = (ph * 6.0).floor();
                ((s * 12.9898).sin() * 43758.545).fract() * 2.0 - 1.0
            }
            kk => wave(kk, ph),
        }
    };
    let pts: Vec<Pos2> = (0..=n).map(|i| {
        let x = i as f32 / n as f32;
        pos2(p.left() + x * p.width(), mid - sh(x * cyc) * amp)
    }).collect();
    fill_under(cx, &pts, mid, k.cv, 60);
    trace(cx, pts, k.cv, 1.8);
    let ph = (cx.t * hz).fract() * cyc;
    let xx = ph / cyc;
    let dot = pos2(p.left() + xx * p.width(), mid - sh(ph) * amp);
    cx.line(pos2(dot.x, p.top()), pos2(dot.x, p.bottom()), 1.0, a(k.cv, 90));
    cx.glow_dot(dot, 3.0, Color32::WHITE, 0.7);
}

/// ADSR in display time (stage widths are log-scaled so a 8 ms attack stays visible).
pub struct Adsr {
    pub a_ms: f32,
    pub d_ms: f32,
    pub s: f32,
    pub r_ms: f32,
}

fn stage_w(ms: f32) -> f32 {
    0.35 + 0.65 * ((1.0 + ms / 5.0).ln() / (1.0 + 2000.0f32 / 5.0).ln())
}

pub fn env_value(e: &Adsr, x: f32) -> f32 {
    // x in 0..1 across the plot.
    let (wa, wd, wr) = (stage_w(e.a_ms), stage_w(e.d_ms), stage_w(e.r_ms));
    let wh = 0.55;
    let tot = wa + wd + wh + wr;
    let u = x * tot;
    if u < wa {
        1.0 - (1.0 - u / wa).powi(2)
    } else if u < wa + wd {
        let q = (u - wa) / wd;
        e.s + (1.0 - e.s) * (1.0 - q).powi(2)
    } else if u < wa + wd + wh {
        e.s
    } else {
        let q = (u - wa - wd - wh) / wr;
        e.s * (1.0 - q).powi(2)
    }
}

pub fn env_view(cx: &Cx, r: Rect, e: &Adsr) {
    let k = cx.k;
    let p = frame(cx, r, (6, 2));
    let plot = Rect::from_min_max(pos2(p.left() + 2.0, p.top() + 4.0), pos2(p.right() - 2.0, p.bottom() - 14.0));
    let n = plot.width() as usize;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let x = i as f32 / n as f32;
            pos2(plot.left() + x * plot.width(), plot.bottom() - env_value(e, x) * plot.height())
        })
        .collect();
    fill_under(cx, &pts, plot.bottom(), k.disp_trace, 80);
    trace(cx, pts.clone(), k.disp_trace, 1.9);
    let (wa, wd, wr) = (stage_w(e.a_ms), stage_w(e.d_ms), stage_w(e.r_ms));
    let wh = 0.55;
    let tot = wa + wd + wh + wr;
    let edges = [0.0, wa / tot, (wa + wd) / tot, (wa + wd + wh) / tot, 1.0];
    let names = ["A", "D", "S", "R"];
    let (_, ph) = gate(cx.t);
    let pos = (ph / 3.6).min(1.0);
    let stage = (0..4).rev().find(|&i| pos >= edges[i]).unwrap_or(0);
    for i in 0..4 {
        let x0 = plot.left() + edges[i] * plot.width();
        if i > 0 {
            cx.line(pos2(x0, plot.top()), pos2(x0, plot.bottom() + 2.0), 1.0, a(k.disp_grid, 255));
        }
        let xm = plot.left() + (edges[i] + edges[i + 1]) / 2.0 * plot.width();
        let on = i == stage;
        cx.text(pos2(xm, p.bottom() - 2.0), Align2::CENTER_BOTTOM, names[i], 10.5, "sans-semi", if on { k.disp_trace } else { a(k.disp_text, 150) });
    }
    let px = plot.left() + pos * plot.width();
    let v = env_value(e, pos);
    cx.line(pos2(px, plot.top()), pos2(px, plot.bottom()), 1.0, a(k.disp_trace, 110));
    cx.glow_dot(pos2(px, plot.bottom() - v * plot.height()), 3.2, if k.dir == Dir::C { k.accent } else { Color32::WHITE }, 0.8);
}

fn svf_db(f: f32, fc: f32, q: f32, mode: usize) -> f32 {
    let x = f / fc;
    let m = ((1.0 - x * x).powi(2) + (x / q).powi(2)).sqrt();
    let num = match mode {
        0 => 1.0,
        1 => x / q,
        _ => x * x,
    };
    20.0 * (num / m).max(1e-6).log10()
}

fn spec(i: usize, t: f32) -> f32 {
    // Saw-like input spectrum, harmonic comb with slow beating.
    let h = (i as f32 * 0.37 + t * 0.6).sin() * 0.5 + 0.5;
    0.35 + 0.65 * h
}

/// Filter response with the input spectrum behind it. `sweep` moves the cutoff in octaves.
pub fn filter_view(cx: &Cx, r: Rect, cutoff: f32, res: f32, sweep: f32, mode: usize) {
    let k = cx.k;
    let p = frame(cx, r, (4, 3));
    let plot = p.shrink2(vec2(2.0, 3.0));
    let fx = |f: f32| plot.left() + (f / 20.0).ln() / (1000.0f32).ln() * plot.width();
    let q = 0.55 + res * 8.0;
    let fc = cutoff * 2f32.powf(sweep * (cx.t * 0.8 * TAU).sin());
    let resp = |f: f32, fc: f32| -> f32 { svf_db(f, fc, q, mode) };
    let ydb = |db: f32| plot.bottom() - ((db + 30.0) / 50.0).clamp(0.0, 1.0) * plot.height();
    // Input spectrum.
    let cols = 56;
    for i in 0..cols {
        let f = 20.0 * 1000.0f32.powf((i as f32 + 0.5) / cols as f32);
        let x = fx(f);
        let h = spec(i, cx.t) * plot.height() * 0.55 * (1.0 - 0.4 * (i as f32 / cols as f32));
        let out_db = resp(f, fc);
        let oh = (h * 10f32.powf(out_db.min(6.0) / 20.0)).min(plot.height() * 0.95);
        let w = plot.width() / cols as f32 * 0.7;
        cx.p.rect_filled(Rect::from_min_max(pos2(x - w / 2.0, plot.bottom() - h), pos2(x + w / 2.0, plot.bottom())), 0.0, a(k.disp_text, 30));
        cx.p.rect_filled(Rect::from_min_max(pos2(x - w / 2.0, plot.bottom() - oh), pos2(x + w / 2.0, plot.bottom())), 0.0, a(k.disp_trace, if k.dir == Dir::C { 70 } else { 55 }));
    }
    // Swept extremes and live curve.
    let n = plot.width() as usize;
    let curve = |fc: f32| -> Vec<Pos2> {
        (0..=n)
            .map(|i| {
                let f = 20.0 * 1000.0f32.powf(i as f32 / n as f32);
                pos2(plot.left() + i as f32, ydb(resp(f, fc)))
            })
            .collect()
    };
    for s in [-1.0f32, 1.0] {
        cx.p.add(Shape::line(curve(cutoff * 2f32.powf(sweep * s)), Stroke::new(1.0, a(k.disp_text, 70))));
    }
    let pts = curve(fc);
    fill_under(cx, &pts, plot.bottom(), k.disp_trace, 60);
    trace(cx, pts, k.disp_trace, 2.0);
    let x = fx(fc).clamp(plot.left(), plot.right());
    cx.line(pos2(x, plot.top()), pos2(x, plot.bottom()), 1.0, a(k.disp_trace, 120));
    cx.glow_dot(pos2(x, ydb(resp(fc, fc))), 3.2, if k.dir == Dir::C { k.accent } else { Color32::WHITE }, 0.8);
}

fn wt_frame(m: f32, ph: f32, warp: f32) -> f32 {
    // Table morph: sine -> saw (adds harmonics) -> square-ish (odd) -> formant ripple.
    let ph = ph + warp * 0.12 * (ph * TAU).sin() / TAU;
    let hmax = 1 + (m * 14.0) as i32;
    let mut s = 0.0;
    for h in 1..=hmax {
        let hf = h as f32;
        let w = if m < 0.6 || h % 2 == 1 { 1.0 / hf } else { 0.15 / hf };
        let fade = if h == hmax { (m * 14.0).fract() } else { 1.0 };
        s += (ph * TAU * hf + 0.4 * hf * m).sin() * w * fade;
    }
    s * 0.62 * (1.0 - 0.15 * m)
}

/// Stacked wavetable frames in light perspective with the active frame lit.
pub fn wavetable_view(cx: &Cx, r: Rect, pos: f32, warp: f32, frames: usize) {
    let k = cx.k;
    let p = frame(cx, r, (1, 1));
    let plot = p.shrink2(vec2(10.0, 8.0));
    let depth = vec2(plot.width() * 0.12, -plot.height() * 0.58);
    let base = pos2(plot.left(), plot.bottom() - plot.height() * 0.28);
    let wdt = plot.width() * 0.8;
    let amp = plot.height() * 0.17;
    let cur = pos * (frames - 1) as f32;
    let n = 120;
    for fi in (0..frames).rev() {
        let m = fi as f32 / (frames - 1) as f32;
        let off = depth * (1.0 - m);
        let lit = (fi as f32 - cur).abs();
        let pts: Vec<Pos2> = (0..=n).map(|i| {
            let x = i as f32 / n as f32;
            pos2(base.x + off.x + x * wdt, base.y + off.y - wt_frame(m, x, warp) * amp)
        }).collect();
        if lit < 0.5 {
            continue;
        }
        let near = (1.0 - lit / frames as f32).powi(2);
        let col = if k.dir == Dir::C { k.disp_text } else { k.disp_trace };
        cx.p.add(Shape::line(pts, Stroke::new(1.1, a(col, (40.0 + 80.0 * near) as u8))));
    }
    let m = pos;
    let off = depth * (1.0 - m);
    let pts: Vec<Pos2> = (0..=n).map(|i| {
        let x = i as f32 / n as f32;
        pos2(base.x + off.x + x * wdt, base.y + off.y - wt_frame(m, x, warp) * amp)
    }).collect();
    fill_under(cx, &pts, base.y + off.y + amp * 0.2, k.disp_trace, 70);
    let col = if k.dir == Dir::C { k.accent } else { k.disp_trace };
    trace(cx, pts, col, 2.3);
    cx.text(pos2(p.right() - 2.0, p.top() + 2.0), Align2::RIGHT_TOP, &format!("frame {:02}/{}", cur.round() as usize + 1, frames), 10.5, "mono-med", a(k.disp_text, 200));
}

/// Flat single-cycle wave of the current frame with a phase dot.
pub fn wt_cycle(cx: &Cx, r: Rect, pos: f32, warp: f32) {
    let k = cx.k;
    let p = frame(cx, r, (4, 2));
    let mid = p.center().y;
    let n = p.width() as usize;
    let amp = p.height() * 0.4;
    let pts: Vec<Pos2> = (0..=n).map(|i| {
        let x = i as f32 / n as f32;
        pos2(p.left() + x * p.width(), mid - wt_frame(pos, x, warp) * amp)
    }).collect();
    trace(cx, pts, k.disp_trace, 1.8);
}

/// Eight-step lane: bar height = step value, bar opacity = probability.
pub fn step_lane(cx: &Cx, r: Rect, vals: &[f32], probs: &[f32], playing: usize, col: Color32) {
    let k = cx.k;
    let n = vals.len();
    let gap = 3.0;
    let w = (r.width() - gap * (n as f32 - 1.0)) / n as f32;
    for i in 0..n {
        let x = r.left() + i as f32 * (w + gap);
        let on = vals[i] > 0.0;
        let h = (vals[i].max(0.08)) * r.height();
        let bar = Rect::from_min_max(pos2(x, r.bottom() - h), pos2(x + w, r.bottom()));
        cx.rr(Rect::from_min_max(pos2(x, r.top()), pos2(x + w, r.bottom())), 2.0, a(k.disp_grid, 255));
        let al = (80.0 + 175.0 * probs[i]) as u8;
        cx.rr(bar, 2.0, if on { a(col, al) } else { a(k.text3, 90) });
        if i == playing {
            cx.rr_stroke(Rect::from_min_max(pos2(x - 1.0, r.top() - 1.0), pos2(x + w + 1.0, r.bottom() + 1.0)), 3.0, 1.6, if k.dir == Dir::C { k.accent } else { Color32::WHITE });
        }
    }
}

pub fn wt_sample(m: f32, x: f32) -> f32 {
    wt_frame(m, x, 0.0)
}
