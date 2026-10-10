//! Live displays on module faces: oscillator cycle, filter response, envelope curve, LFO shape.
//! Each is drawn from the module's own parameters (so it is right with audio off) and, when the
//! engine's face taps report the module (`kabl_engine::facetap`), from what the module is doing:
//! the real cycle, the modulated cutoff, the stage the envelope is in, the LFO's phase.
//!
//! Every colour and shape comes from the theme through `kit::face`; what the picture shows
//! (axes, curves) is display behaviour and not themable.

use std::collections::HashMap;

use egui::{pos2, vec2, Pos2, Rect, Stroke};
use kabl_core::{ModuleId, PatchState, PortRef};
use kabl_modules::{registry, ModuleView, PortDirection};

use crate::kit::face;
use crate::rack::Placed;
use crate::routing;
use crate::style::{alpha, Style};
use crate::Xf;

/// What the engine's face taps last reported for one output of one module.
#[derive(Clone, Copy, Debug)]
pub struct LiveTap {
    /// The newest sample, and the extremes over the report window.
    pub last: f32,
    pub min: f32,
    pub max: f32,
    pub view: ModuleView,
    /// UI time the report arrived.
    pub at: f64,
}

/// Face-tap reports by (module, output port index).
#[derive(Default)]
pub struct Live {
    pub taps: HashMap<(ModuleId, u8), LiveTap>,
}

/// Reports older than this are not drawn as live.
const STALE_SECS: f64 = 0.6;

impl Live {
    pub fn tap(&self, id: ModuleId, port: u8, now: f64) -> Option<&LiveTap> {
        self.taps
            .get(&(id, port))
            .filter(|t| now - t.at < STALE_SECS)
    }

    /// The live value of output `port` of module `id` (by name) in units of its port type's full
    /// scale, or `None` while it is not reported.
    pub fn source_unit(
        &self,
        state: &PatchState,
        id: ModuleId,
        port: &str,
        now: f64,
    ) -> Option<f32> {
        let info = registry::info_for(&state.modules.get(&id)?.kind)?;
        let (index, p) = info
            .ports
            .iter()
            .filter(|p| p.direction == PortDirection::Output)
            .enumerate()
            .find(|(_, p)| p.name == port)?;
        let (lo, hi) = p.port_type.nominal_range();
        let full = lo.abs().max(hi.abs());
        Some(self.tap(id, index as u8, now)?.last / full)
    }
}

/// The modulated knob travel of `(id, param)` now: the base plus every active route's amount
/// times its source's live value. `None` when no route has a live source.
pub fn modulated_norm(
    live: &Live,
    state: &PatchState,
    routes: &[routing::RouteView],
    base_n: f32,
    now: f64,
) -> Option<f32> {
    let mut n = base_n;
    let mut any = false;
    for rt in routes.iter().filter(|r| !r.bypass) {
        if let Some(u) = live.source_unit(state, rt.from_id, &rt.from_port, now) {
            n += rt.amount * u;
            any = true;
        }
    }
    any.then_some(n.clamp(0.0, 1.0))
}

/// How much of a display survives at this zoom.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Detail {
    /// Frame only: the face is too small for a picture to read.
    Frame,
    /// A plain trace, no glow, fill or playhead.
    Trace,
    Full,
}

fn detail(z: f32) -> Detail {
    if z < 0.45 {
        Detail::Frame
    } else if z < 0.7 {
        Detail::Trace
    } else {
        Detail::Full
    }
}

/// The module's parameter `name` as the compiler reads it: stored value or default.
fn param(state: &PatchState, m: &Placed, name: &str) -> f32 {
    m.info
        .params
        .iter()
        .find(|q| q.name == name)
        .map_or(0.0, |q| routing::base_value(state, m.id, q))
}

/// Draws the display of `m` in `r` (world units).
pub(crate) fn draw(
    state: &PatchState,
    st: &Style,
    live: &Live,
    now: f64,
    painter: &egui::Painter,
    xf: Xf,
    m: &Placed,
    r: Rect,
) {
    let z = xf.zoom;
    let r = xf.r(r);
    face::display_frame(painter, st, r, z);
    let d = detail(z);
    if d == Detail::Frame {
        return;
    }
    let inner = r.shrink((5.0 * z).max(3.0));
    match m.info.kind {
        "osc.va" => osc_va(state, st, live, now, painter, m, inner, z, d),
        "osc.wt" | "osc.fm" | "osc.fm6" => scope(state, st, live, now, painter, m, inner, z, d),
        "filter.svf" | "filter.ladder" => filter(state, st, live, now, painter, m, inner, z, d),
        "env.adsr" => envelope(state, st, live, now, painter, m, inner, z, d),
        "lfo" => lfo(state, st, live, now, painter, m, inner, z, d),
        _ => {}
    }
}

/// A plotted function of `x` in 0..1 giving `y` in -1..1, across `r`.
fn plot(r: Rect, n: usize, f: impl Fn(f32) -> f32) -> Vec<Pos2> {
    (0..=n)
        .map(|i| {
            let x = i as f32 / n as f32;
            pos2(
                r.left() + r.width() * x,
                r.center().y - r.height() * 0.5 * f(x).clamp(-1.0, 1.0),
            )
        })
        .collect()
}

fn paint(p: &egui::Painter, st: &Style, pts: Vec<Pos2>, z: f32, d: Detail, base: Option<f32>) {
    if d == Detail::Full {
        face::trace(p, st, pts, z, base);
    } else {
        p.add(egui::Shape::line(
            pts,
            Stroke::new(1.4, face::trace_color(st)),
        ));
    }
}

/// One period of a plain waveform: 0 sine, 1 triangle, 2 saw, 3 square with pulse width `pw`.
fn wave(kind: i32, pw: f32, x: f32) -> f32 {
    match kind {
        0 => (std::f32::consts::TAU * x).sin(),
        1 => 1.0 - 4.0 * ((x + 0.25).fract() - 0.5).abs(),
        2 => 2.0 * x - 1.0,
        _ => {
            if x < pw {
                1.0
            } else {
                -1.0
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn osc_va(
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
    let tap = live.tap(m.id, 0, now).filter(|t| t.view.valid);
    let pts = match tap {
        Some(t) => cycle_points(&t.view.cycle, r),
        None => {
            let kind = param(state, m, "waveform").round() as i32;
            let pw = param(state, m, "pw") / 100.0;
            plot(r, 96, |x| wave(kind, pw, x))
        }
    };
    paint(p, st, pts, z, d, None);
}

fn cycle_points(c: &[f32], r: Rect) -> Vec<Pos2> {
    let n = c.len() - 1;
    c.iter()
        .enumerate()
        .map(|(i, &y)| {
            pos2(
                r.left() + r.width() * i as f32 / n as f32,
                r.center().y - r.height() * 0.5 * y.clamp(-1.0, 1.0),
            )
        })
        .collect()
}

/// The resulting cycle of a table or FM oscillator, with the table position under it.
#[allow(clippy::too_many_arguments)]
fn scope(
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
    let wt = m.info.kind == "osc.wt";
    let tap = live.tap(m.id, 0, now).filter(|t| t.view.valid);
    let body = if wt {
        Rect::from_min_max(r.min, pos2(r.right(), r.bottom() - (6.0 * z).max(4.0)))
    } else {
        r
    };
    let pts = match tap {
        Some(t) => cycle_points(&t.view.cycle, body),
        None => plot(body, 2, |_| 0.0),
    };
    paint(p, st, pts, z, d, None);
    if wt {
        let bar = Rect::from_min_max(pos2(r.left(), r.bottom() - (3.0 * z).max(2.0)), r.max);
        p.rect_filled(bar, 1.5, alpha(st.roles.disp_grid, 220));
        let pos = tap.map_or_else(|| param(state, m, "position"), |t| t.view.position);
        let pos = pos.clamp(0.0, 1.0);
        let x = bar.left() + bar.width() * pos;
        p.rect_filled(
            Rect::from_min_max(pos2(bar.left(), bar.top()), pos2(x, bar.bottom())),
            1.5,
            alpha(face::trace_color(st), 150),
        );
        p.circle_filled(
            pos2(x, bar.center().y),
            (3.0 * z).max(2.5),
            st.roles.disp_text,
        );
    }
}

/// |H| in dB of the filter at normalised frequency ratio `w = f / fc`.
fn svf_db(out: usize, w: f32, k: f32) -> f32 {
    // s = jw: LP 1/(1-w²+jkw), BP jw/(…), HP -w²/(…)
    let (re, im) = (1.0 - w * w, k * w);
    let den = (re * re + im * im).sqrt().max(1e-6);
    let num = match out {
        0 => 1.0,
        1 => w,
        _ => w * w,
    };
    20.0 * (num / den).log10()
}

fn ladder_db(w: f32, k: f32) -> f32 {
    // 1 / ((1+jw)^4 + k)
    let (a, b) = (1.0f32, w);
    let sq = |(x, y): (f32, f32)| (x * x - y * y, 2.0 * x * y);
    let s2 = sq((a, b));
    let s4 = sq(s2);
    let (re, im) = (s4.0 + k, s4.1);
    -10.0 * (re * re + im * im).max(1e-12).log10()
}

#[allow(clippy::too_many_arguments)]
fn filter(
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
    let cutoff_p = m.info.params.iter().find(|q| q.name == "cutoff_hz");
    let res_p = m.info.params.iter().find(|q| q.name == "resonance");
    let (Some(cp), Some(rp)) = (cutoff_p, res_p) else {
        return;
    };
    let base = routing::base_value(state, m.id, cp);
    let res_base = routing::base_value(state, m.id, rp);
    let live_n = |q: &kabl_modules::ParamInfo, base: f32| {
        let routes = routing::routes_into(state, m.id, q.name);
        modulated_norm(live, state, &routes, q.to_norm(base), now).map(|n| q.from_norm(n))
    };
    let cutoff = live_n(cp, base).unwrap_or(base);
    let res = live_n(rp, res_base).unwrap_or(res_base);
    let (f_lo, f_hi) = (20.0f32, 20000.0f32);
    let fx = |f: f32| (f / f_lo).ln() / (f_hi / f_lo).ln();
    let ladder = m.info.kind == "filter.ladder";
    let k = if ladder {
        4.4 * res.clamp(0.0, 1.0)
    } else {
        2.0 - 2.0 * res.clamp(0.0, 0.95)
    };
    let ymap = |db: f32| ((db + 40.0) / 64.0 * 2.0 - 1.0).clamp(-1.0, 1.0);
    let sr = 48000.0f32;
    let g = (std::f32::consts::PI * cutoff / sr).tan();
    let outs: &[usize] = if ladder { &[0] } else { &[0, 1, 2] };
    let patched = |port: &str| {
        state
            .cables
            .values()
            .any(|c| matches!(&c.from, PortRef::Module { id, port: q } if *id == m.id && q == port))
    };
    for &o in outs {
        let port = ["lp", "bp", "hp"][o];
        let strong =
            ladder || patched(port) || (o == 0 && !["lp", "bp", "hp"].iter().any(|q| patched(q)));
        if d != Detail::Full && !strong {
            continue;
        }
        let pts = plot(r, 80, |x| {
            let f = f_lo * (f_hi / f_lo).powf(x);
            let w = (std::f32::consts::PI * f / sr).tan() / g;
            ymap(if ladder {
                ladder_db(w, k)
            } else {
                svf_db(o, w, k)
            })
        });
        if strong {
            paint(p, st, pts, z, d, (o == 0).then_some(r.bottom()));
        } else {
            p.add(egui::Shape::line(
                pts,
                Stroke::new(1.0, alpha(face::trace_color(st), 90)),
            ));
        }
    }
    let x = r.left() + r.width() * fx(cutoff).clamp(0.0, 1.0);
    p.line_segment(
        [pos2(x, r.top()), pos2(x, r.bottom())],
        Stroke::new(1.0, alpha(st.roles.disp_text, 90)),
    );
    if d == Detail::Full {
        let xb = r.left() + r.width() * fx(base).clamp(0.0, 1.0);
        if (xb - x).abs() > 1.5 {
            p.circle_stroke(
                pos2(xb, r.bottom() - 2.0 * z),
                (2.5 * z).max(2.0),
                Stroke::new(1.0, alpha(st.roles.disp_text, 160)),
            );
        }
        face::playhead(p, st, pos2(x, r.bottom() - 2.0 * z), z);
    }
}

#[allow(clippy::too_many_arguments)]
fn envelope(
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
    let (a, dec, s, rel) = (
        param(state, m, "attack_ms"),
        param(state, m, "decay_ms"),
        param(state, m, "sustain").clamp(0.0, 1.0),
        param(state, m, "release_ms"),
    );
    let wlog = |ms: f32| (1.0 + ms.max(0.1)).ln();
    let total = wlog(a) + wlog(dec) + wlog(rel) + 2.0;
    let sx = |x: f32| r.left() + r.width() * x / total;
    let sy = |y: f32| r.bottom() - r.height() * y;
    let (x1, x2) = (wlog(a), wlog(a) + wlog(dec));
    let x3 = x2 + 2.0;
    let pts = vec![
        pos2(sx(0.0), sy(0.0)),
        pos2(sx(x1), sy(1.0)),
        pos2(sx(x2), sy(s)),
        pos2(sx(x3), sy(s)),
        pos2(sx(total), sy(0.0)),
    ];
    paint(p, st, pts, z, d, Some(r.bottom()));
    if d != Detail::Full {
        return;
    }
    let Some(t) = live.tap(m.id, 0, now).filter(|t| t.view.valid) else {
        return;
    };
    let level = t.last.clamp(0.0, 1.0);
    let at = match t.view.position.round() as i32 {
        1 => Some((x1 * level, level)),
        2 => Some((
            x1 + (x2 - x1) * ((1.0 - level) / (1.0 - s).max(0.01)).clamp(0.0, 1.0),
            level,
        )),
        3 => Some((x2 + 1.0, s)),
        4 => Some((
            x3 + (total - x3) * (1.0 - level / s.max(0.01)).clamp(0.0, 1.0),
            level,
        )),
        _ => None,
    };
    if let Some((x, y)) = at {
        let c = pos2(sx(x), sy(y));
        p.line_segment(
            [pos2(c.x, r.top()), pos2(c.x, r.bottom())],
            Stroke::new(1.0, alpha(st.roles.disp_text, 70)),
        );
        face::playhead(p, st, c, z);
    }
}

#[allow(clippy::too_many_arguments)]
fn lfo(
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
    let kind = param(state, m, "waveform").round() as i32;
    // The module's own shapes: sine, triangle, saw, square; sample-and-hold as steps.
    const STEPS: [f32; 8] = [0.3, -0.6, 0.8, -0.2, 0.55, -0.85, 0.1, 0.65];
    let f = |x: f32| match kind {
        1 => 1.0 - 4.0 * ((x + 0.25).fract() - 0.5).abs(),
        2 => 2.0 * x - 1.0,
        3 => {
            if x < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        4 => STEPS[((x * 8.0) as usize).min(7)],
        _ => (std::f32::consts::TAU * x).sin(),
    };
    paint(p, st, plot(r, 96, f), z, d, None);
    if d != Detail::Full {
        return;
    }
    if let Some(t) = live.tap(m.id, 0, now).filter(|t| t.view.valid) {
        let x = t.view.position.rem_euclid(1.0);
        // The dot rides the real output; the shape is the picture of one cycle.
        let y = if kind == 4 { t.last } else { f(x) };
        face::playhead(
            p,
            st,
            pos2(
                r.left() + r.width() * x,
                r.center().y - r.height() * 0.5 * y.clamp(-1.0, 1.0),
            ),
            z,
        );
    }
}

/// The scope in the FM voice's header, between its jacks and the feedback knob.
pub fn fm6_scope(face: Rect) -> Rect {
    Rect::from_min_size(face.min + vec2(246.0, 38.0), vec2(150.0, 52.0))
}
