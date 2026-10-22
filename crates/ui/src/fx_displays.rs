//! Live displays of the effect and filter modules added with the effects batch: EQ curve,
//! compressor and limiter gain reduction, phaser and flanger sweep, comb and formant response,
//! bitcrusher steps. Same rules as `displays`: drawn from the module's own parameters (so right
//! with audio off) and moved by what the face taps report (modulated parameters, the gain
//! reduction, the LFO phase). The curves are small-signal pictures of the module's own maths:
//! the EQ and formant read the module's coefficients, the compressor its gain computer; the
//! phaser, flanger and comb are their linear transfer functions (the `tanh` in their feedback
//! paths, and the comb's damping lag, are left out).

use egui::{pos2, vec2, Align2, Pos2, Rect, Stroke};
use kabl_core::{ModuleId, PatchState, PortRef};
use kabl_modules::builtins::{band_response_db, compressor_reduction, formant_bands};

use crate::displays::{modulated_norm, paint, param, plot, Detail, Live};
use crate::kit::face;
use crate::rack::Placed;
use crate::routing;
use crate::style::{alpha, Role, Style};

const SR: f32 = 48000.0;

type C = (f32, f32);

fn cmul(a: C, b: C) -> C {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}

fn cdiv(a: C, b: C) -> C {
    let d = (b.0 * b.0 + b.1 * b.1).max(1e-12);
    ((a.0 * b.0 + a.1 * b.1) / d, (a.1 * b.0 - a.0 * b.1) / d)
}

fn power_db(c: C) -> f32 {
    10.0 * (c.0 * c.0 + c.1 * c.1).max(1e-12).log10()
}

/// e^{-j w}
fn delay(w: f32) -> C {
    (w.cos(), -w.sin())
}

/// The parameter as it is now: modulated by every route with a live source, else the stored value.
fn live_param(state: &PatchState, live: &Live, now: f64, m: &Placed, name: &str) -> f32 {
    let Some(q) = m.info.params.iter().find(|q| q.name == name) else {
        return 0.0;
    };
    let base = routing::base_value(state, m.id, q);
    let routes = routing::routes_into(state, m.id, name);
    modulated_norm(live, state, &routes, q.to_norm(base), now).map_or(base, |n| q.from_norm(n))
}

/// The newest value the face taps report on the cable into input `port` of `id`.
pub(crate) fn input_value(
    state: &PatchState,
    live: &Live,
    now: f64,
    id: ModuleId,
    port: &str,
) -> Option<f32> {
    state.cables.values().find_map(|c| match (&c.from, &c.to) {
        (PortRef::Module { id: f, port: fp }, PortRef::Module { id: t, port: tp })
            if *t == id && tp == port =>
        {
            live.source_value(state, *f, fp, now).map(|(v, _)| v)
        }
        _ => None,
    })
}

/// Axis position of `hz` on a log axis from `lo` to `hi`.
fn log_x(hz: f32, lo: f32, hi: f32) -> f32 {
    (hz / lo).ln() / (hi / lo).ln()
}

/// A response curve over a log frequency axis; `g` gives dB at a frequency, shown between
/// `lo_db` and `hi_db`. `sub` evaluations per point, the loudest kept (a dense comb stays an
/// envelope instead of aliasing).
#[allow(clippy::too_many_arguments)]
fn response(
    r: Rect,
    (lo, hi): (f32, f32),
    (lo_db, hi_db): (f32, f32),
    n: usize,
    sub: usize,
    g: impl Fn(f32) -> f32,
) -> Vec<Pos2> {
    plot(r, n, |x| {
        let at = |x: f32| lo * (hi / lo).powf(x);
        let db = (0..sub)
            .map(|k| g(at(x + (k as f32 / sub as f32 - 0.5) / n as f32)))
            .fold(f32::NEG_INFINITY, f32::max);
        (db - lo_db) / (hi_db - lo_db) * 2.0 - 1.0
    })
}

fn faint(st: &Style) -> egui::Color32 {
    alpha(st.roles.disp_text, 55)
}

fn hline(p: &egui::Painter, st: &Style, r: Rect, y: f32) {
    p.line_segment(
        [pos2(r.left(), y), pos2(r.right(), y)],
        Stroke::new(1.0, faint(st)),
    );
}

fn caption(p: &egui::Painter, st: &Style, z: f32, at: Pos2, align: Align2, s: &str) {
    if let Some(font) = face::face_font(st, Role::Caption, z) {
        p.text(at, align, s, font, alpha(st.roles.disp_text, 200));
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    state: &PatchState,
    st: &Style,
    live: &Live,
    now: f64,
    p: &egui::Painter,
    m: &Placed,
    r: Rect,
    z: f32,
    d: Detail,
) {
    match m.info.kind {
        "eq" => eq(state, st, live, now, p, m, r, z, d),
        "compressor" => compressor(state, st, live, now, p, m, r, z, d),
        "limiter" => limiter(state, st, live, now, p, m, r, z, d),
        "phaser" | "flanger" => sweep(state, st, live, now, p, m, r, z, d),
        "filter.comb" => comb(state, st, live, now, p, m, r, z, d),
        "filter.formant" => formant(state, st, live, now, p, m, r, z, d),
        "bitcrusher" => bitcrusher(state, st, p, m, r, z, d),
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn eq(
    state: &PatchState,
    st: &Style,
    live: &Live,
    now: f64,
    p: &egui::Painter,
    m: &Placed,
    r: Rect,
    z: f32,
    d: Detail,
) {
    const BANDS: [(&str, &str, Option<&str>); 4] = [
        ("low_hz", "low_db", None),
        ("mid1_hz", "mid1_db", Some("mid1_q")),
        ("mid2_hz", "mid2_db", Some("mid2_q")),
        ("high_hz", "high_db", None),
    ];
    let set: Vec<(f32, f32, f32)> = BANDS
        .iter()
        .map(|&(f, g, q)| {
            (
                live_param(state, live, now, m, f),
                live_param(state, live, now, m, g),
                q.map_or(1.0, |q| live_param(state, live, now, m, q)),
            )
        })
        .collect();
    let total = |hz: f32| -> f32 {
        set.iter()
            .enumerate()
            .map(|(b, &(f, g, q))| band_response_db(b, f, g, q, SR, hz))
            .sum()
    };
    // The bands reach +-18 dB; +-14 keeps ordinary settings readable, the rest clips.
    let (axis, range) = ((20.0, 20000.0), (-14.0, 14.0));
    let mid = r.center().y;
    hline(p, st, r, mid);
    paint(
        p,
        st,
        response(r, axis, range, 96, 1, total),
        z,
        d,
        Some(mid),
    );
    if d == Detail::Full {
        for &(f, g, _) in &set {
            let y = mid - r.height() * 0.5 * (g / range.1).clamp(-1.0, 1.0);
            let x = r.left() + r.width() * log_x(f, axis.0, axis.1).clamp(0.0, 1.0);
            p.circle_stroke(
                pos2(x, y),
                (2.5 * z).max(2.0),
                Stroke::new(1.0, alpha(st.roles.disp_text, 170)),
            );
        }
    }
}

/// A transfer curve on the left (input dB across, output dB up) and a gain-reduction bar beside
/// it. `gr` is the live reduction in dB.
#[allow(clippy::too_many_arguments)]
fn dynamics(
    st: &Style,
    p: &egui::Painter,
    r: Rect,
    z: f32,
    d: Detail,
    transfer: impl Fn(f32) -> f32,
    mark: f32,
    gr: Option<f32>,
) {
    const LO: f32 = -60.0;
    let side = r.height();
    let sq = Rect::from_min_size(r.min, vec2(side, side));
    let map = |x_db: f32, y_db: f32| {
        pos2(
            sq.left() + side * (x_db - LO) / -LO,
            sq.bottom() - side * (y_db - LO) / -LO,
        )
    };
    if d == Detail::Full {
        p.line_segment([map(LO, LO), map(0.0, 0.0)], Stroke::new(1.0, faint(st)));
        p.line_segment(
            [map(mark, LO), map(mark, 0.0)],
            Stroke::new(1.0, alpha(st.roles.disp_text, 90)),
        );
    }
    let pts: Vec<Pos2> = (0..=48)
        .map(|i| {
            let x = LO + (-LO) * i as f32 / 48.0;
            map(x, transfer(x).clamp(LO, 0.0))
        })
        .collect();
    paint(p, st, pts, z, d, None);
    // The bar: 0..24 dB of reduction, growing from the left.
    let gap = 10.0 * z;
    let bar = Rect::from_center_size(
        pos2((sq.right() + gap + r.right()) / 2.0, r.center().y + 4.0 * z),
        vec2((r.right() - sq.right() - gap).max(8.0), 9.0 * z.max(0.7)),
    );
    p.rect_stroke(
        bar,
        2.0,
        Stroke::new(1.0, alpha(st.roles.disp_text, 110)),
        egui::StrokeKind::Inside,
    );
    if let Some(gr) = gr {
        let w = bar.width() * (gr / 24.0).clamp(0.0, 1.0);
        p.rect_filled(
            Rect::from_min_size(bar.min, vec2(w, bar.height())),
            2.0,
            face::trace_color(st),
        );
    }
    if d == Detail::Full {
        for k in 1..4 {
            let x = bar.left() + bar.width() * k as f32 / 4.0;
            p.line_segment(
                [pos2(x, bar.top()), pos2(x, bar.bottom())],
                Stroke::new(1.0, faint(st)),
            );
        }
        let label = gr.map_or("GR".to_string(), |g| format!("GR {g:.1} dB"));
        caption(
            p,
            st,
            z,
            pos2(bar.left(), bar.top() - 3.0 * z),
            Align2::LEFT_BOTTOM,
            &label,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn compressor(
    state: &PatchState,
    st: &Style,
    live: &Live,
    now: f64,
    p: &egui::Painter,
    m: &Placed,
    r: Rect,
    z: f32,
    d: Detail,
) {
    let thr = live_param(state, live, now, m, "threshold_db");
    let ratio = live_param(state, live, now, m, "ratio");
    let knee = live_param(state, live, now, m, "knee_db");
    let makeup = live_param(state, live, now, m, "makeup_db");
    // `gr` is output 2; the largest reduction of the window.
    let gr = live.tap(m.id, 2, now).map(|t| t.max.max(0.0));
    dynamics(
        st,
        p,
        r,
        z,
        d,
        |x| x - compressor_reduction(x, thr, ratio, knee) + makeup,
        thr,
        gr,
    );
}

#[allow(clippy::too_many_arguments)]
fn limiter(
    state: &PatchState,
    st: &Style,
    live: &Live,
    now: f64,
    p: &egui::Painter,
    m: &Placed,
    r: Rect,
    z: f32,
    d: Detail,
) {
    let gain = live_param(state, live, now, m, "gain_db");
    let ceil = live_param(state, live, now, m, "ceiling_db");
    let gr = live
        .tap(m.id, 0, now)
        .filter(|t| t.view.valid)
        .map(|t| t.view.position);
    dynamics(st, p, r, z, d, |x| (x + gain).min(ceil), ceil - gain, gr);
}

/// Phaser notches or flanger comb at the LFO's present position, with the two ends of the
/// sweep behind it.
#[allow(clippy::too_many_arguments)]
fn sweep(
    state: &PatchState,
    st: &Style,
    live: &Live,
    now: f64,
    p: &egui::Painter,
    m: &Placed,
    r: Rect,
    z: f32,
    d: Detail,
) {
    let phaser = m.info.kind == "phaser";
    let depth = live_param(state, live, now, m, "depth") / 100.0;
    let fb = live_param(state, live, now, m, "feedback") / 100.0;
    let mix = live_param(state, live, now, m, "mix") / 100.0;
    let stages = if phaser {
        param(state, m, "stages").round().clamp(2.0, 12.0) as usize
    } else {
        0
    };
    let tap = live.tap(m.id, 0, now).filter(|t| t.view.valid);
    let phase = tap.map_or(0.0, |t| t.view.position);
    let h = move |lfo: f32, hz: f32| -> f32 {
        let w = std::f32::consts::TAU * hz / SR;
        let zi = delay(w);
        let wet = if phaser {
            let fc = (800.0 * (depth * 2.5 * lfo).exp2()).clamp(20.0, 0.45 * SR);
            let t = (std::f32::consts::PI * fc / SR).tan();
            let a = (t - 1.0) / (t + 1.0);
            let ap = cdiv((a + zi.0, zi.1), (1.0 + a * zi.0, a * zi.1));
            let chain = (0..stages).fold((1.0, 0.0), |acc, _| cmul(acc, ap));
            let loop_ = cmul(zi, chain);
            cdiv(chain, (1.0 - fb * loop_.0, -fb * loop_.1))
        } else {
            let ms = 0.5 + 3.0 * (1.0 + depth * lfo);
            let e = delay(w * ms * 0.001 * SR);
            cdiv(e, (1.0 - fb * e.0, -fb * e.1))
        };
        power_db(((1.0 - mix) + mix * wet.0, mix * wet.1))
    };
    let (axis, range) = ((20.0, 20000.0), (-36.0, 18.0));
    // The 0 dB line.
    hline(
        p,
        st,
        r,
        r.bottom() - r.height() * -range.0 / (range.1 - range.0),
    );
    let now_lfo = (std::f32::consts::TAU * phase).sin();
    if d == Detail::Full {
        for end in [-1.0f32, 1.0] {
            p.add(egui::Shape::line(
                response(r, axis, range, 96, 1, |hz| h(end, hz)),
                Stroke::new(1.0, alpha(face::trace_color(st), 70)),
            ));
        }
    }
    paint(
        p,
        st,
        response(r, axis, range, 96, 1, |hz| h(now_lfo, hz)),
        z,
        d,
        None,
    );
}

#[allow(clippy::too_many_arguments)]
fn comb(
    state: &PatchState,
    st: &Style,
    live: &Live,
    now: f64,
    p: &egui::Painter,
    m: &Placed,
    r: Rect,
    z: f32,
    d: Detail,
) {
    let freq = live_param(state, live, now, m, "freq_hz");
    let fb = live_param(state, live, now, m, "feedback") / 100.0;
    let a = 0.6 * live_param(state, live, now, m, "damp") / 100.0;
    let pitch = input_value(state, live, now, m.id, "pitch").unwrap_or(0.0);
    let f0 = (freq * (pitch / 12.0).exp2()).clamp(20.0, SR / 6.0);
    let level = (1.0 - fb * fb).sqrt();
    let g = |hz: f32| -> f32 {
        let w = std::f32::consts::TAU * hz / SR;
        // The loop low-pass loses highs each trip; its phase is taken off the delay by the module.
        let lp = (1.0 - a) / ((1.0 - a * w.cos()).powi(2) + (a * w.sin()).powi(2)).sqrt();
        let e = delay(w * SR / f0);
        let den = (1.0 - fb * lp * e.0, -fb * lp * e.1);
        20.0 * (level / (den.0 * den.0 + den.1 * den.1).sqrt().max(1e-6)).log10()
    };
    let (axis, range) = ((20.0, 20000.0), (-30.0, 24.0));
    hline(p, st, r, r.center().y);
    paint(p, st, response(r, axis, range, 200, 4, g), z, d, None);
    if d == Detail::Full {
        let x = r.left() + r.width() * log_x(f0, axis.0, axis.1).clamp(0.0, 1.0);
        p.line_segment(
            [pos2(x, r.top()), pos2(x, r.bottom())],
            Stroke::new(1.0, alpha(st.roles.disp_text, 90)),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn formant(
    state: &PatchState,
    st: &Style,
    live: &Live,
    now: f64,
    p: &egui::Painter,
    m: &Placed,
    r: Rect,
    z: f32,
    d: Detail,
) {
    let vowel = live_param(state, live, now, m, "vowel");
    let shift = live_param(state, live, now, m, "shift_st");
    let q = live_param(state, live, now, m, "q");
    let bands = formant_bands(vowel, shift);
    let g = |hz: f32| -> f32 {
        let sum = bands.iter().fold((0.0f32, 0.0f32), |acc, &(f, db, bw)| {
            let f = f.clamp(20.0, 0.45 * SR);
            let k = (bw / f / q).clamp(0.02, 2.0);
            let w = hz / f;
            let level = 10f32.powf(db / 20.0);
            // level * k * jw / (1 - w² + j k w)
            let h = cdiv((0.0, level * k * w), (1.0 - w * w, k * w));
            (acc.0 + h.0, acc.1 + h.1)
        });
        power_db(sum)
    };
    let (axis, range) = ((100.0, 6000.0), (-40.0, 8.0));
    paint(
        p,
        st,
        response(r, axis, range, 120, 1, g),
        z,
        d,
        Some(r.bottom()),
    );
    if d == Detail::Full {
        let letter = ["a", "e", "i", "o", "u"][(vowel.round() as usize).min(4)];
        caption(
            p,
            st,
            z,
            pos2(r.left() + 3.0 * z, r.top() + 2.0 * z),
            Align2::LEFT_TOP,
            letter,
        );
        for &(f, _, _) in &bands[..2] {
            let x = r.left()
                + r.width() * log_x(f.clamp(20.0, 0.45 * SR), axis.0, axis.1).clamp(0.0, 1.0);
            p.line_segment(
                [pos2(x, r.bottom() - 4.0 * z), pos2(x, r.bottom())],
                Stroke::new(1.0, alpha(st.roles.disp_text, 140)),
            );
        }
    }
}

/// A sine as the module would hand it on: rounded to the bit depth, held for the downsample
/// ratio. The picture is one wave of 48 samples shown twice; the module's own loop.
fn bitcrusher(
    state: &PatchState,
    st: &Style,
    p: &egui::Painter,
    m: &Placed,
    r: Rect,
    z: f32,
    d: Detail,
) {
    let bits = param(state, m, "bits").clamp(1.0, 16.0);
    let down = param(state, m, "downsample").clamp(1.0, 64.0);
    let mix = param(state, m, "mix").clamp(0.0, 100.0) / 100.0;
    const N: usize = 96;
    let step = (1.0 - bits).exp2();
    let src = |i: usize| 0.9 * (std::f32::consts::TAU * i as f32 / 48.0).sin();
    let (mut acc, mut held) = (1.0f32, 0.0f32);
    let mut pts = Vec::with_capacity(2 * N);
    let y = |v: f32| r.center().y - r.height() * 0.5 * v.clamp(-1.0, 1.0);
    let x = |i: usize| r.left() + r.width() * i as f32 / N as f32;
    for i in 0..N {
        let s = src(i);
        if acc >= 1.0 {
            acc = (acc - 1.0).min(1.0);
            held = (s / step).round() * step;
        }
        acc += 1.0 / down;
        let out = s * (1.0 - mix) + held * mix;
        // Stairs: a flat tread to the next sample.
        pts.push(pos2(x(i), y(out)));
        pts.push(pos2(x(i + 1), y(out)));
    }
    hline(p, st, r, r.center().y);
    if d == Detail::Full {
        p.add(egui::Shape::line(
            (0..=N).map(|i| pos2(x(i), y(src(i)))).collect(),
            Stroke::new(1.0, alpha(face::trace_color(st), 70)),
        ));
    }
    paint(p, st, pts, z, d, None);
}
