//! Displays of the drum voices: the amplitude envelope of a hit on a time axis, drawn from the
//! module's own parameters (so it is right with audio off), with the pitch curve of a kick or
//! tom, the two layers of a snare, the bursts and tail of a clap and the closed and open decays
//! of a hat. While audio runs, a playhead rides the curve at the time since the last hit
//! (`Module::view().position`, from the face taps); the hat lights the curve of the hit it
//! played. The envelope is the shape the module multiplies in; the drive, tone filter and
//! noise colour are not drawn.

use egui::{pos2, Align2, Rect, Stroke};
use kabl_core::PatchState;
use kabl_modules::builtins::{clap_curve, snare_decays, thump_curve};

use crate::displays::{paint, param, plot, Detail, Live};
use crate::fx_displays::{caption, faint, live_param};
use crate::kit::face;
use crate::rack::Placed;
use crate::style::{alpha, Style};

/// 60 dB as a neper ratio: the decays are times to fall this far.
const LN_1000: f32 = 6.907_755;

/// `exp` decay with a 60 dB time of `ms`, at `t` seconds.
fn decay(ms: f32, t: f32) -> f32 {
    (-LN_1000 * t / (ms * 0.001).max(1e-4)).exp()
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
    let q = |name: &str| live_param(state, live, now, m, name);
    // The span shown (s) and the curves: (amplitude at t, filled, drawn bright).
    let tap = live.tap(m.id, 0, now).filter(|t| t.view.valid);
    let open_hit = tap.is_some_and(|t| t.view.cycle[0] > 0.5);
    type Curve<'a> = Box<dyn Fn(f32) -> f32 + 'a>;
    let mut curves: Vec<(Curve, bool)> = Vec::new();
    let mut pitch: Option<Curve> = None;
    let span;
    match m.info.kind {
        "drum.kick" | "drum.tom" => {
            let (tune, sweep, ms, dec) =
                (q("tune_hz"), q("sweep_st"), q("sweep_ms"), q("decay_ms"));
            span = dec * 0.0005;
            curves.push((
                Box::new(move |t| thump_curve(tune, sweep, ms, dec, t).1),
                true,
            ));
            if sweep >= 0.5 {
                let top = 2f32.powf(sweep / 12.0).ln();
                pitch = Some(Box::new(move |t| {
                    (thump_curve(tune, sweep, ms, dec, t).0 / tune).ln() / top
                }));
            }
        }
        "drum.snare" => {
            let dec = q("decay_ms");
            let (body_ms, noise_ms) = snare_decays(dec);
            let a = q("snappy") * std::f32::consts::FRAC_PI_2;
            let (wb, wn) = (a.cos() * 0.8, a.sin() * 1.3);
            let top = wb.max(wn).max(0.05);
            span = dec * 0.0006;
            curves.push((Box::new(move |t| wn / top * decay(noise_ms, t)), true));
            curves.push((Box::new(move |t| wb / top * decay(body_ms, t)), false));
        }
        "drum.clap" => {
            let bursts = param(state, m, "bursts").round().clamp(2.0, 6.0) as usize;
            let (spread, dec) = (q("spread_ms"), q("decay_ms"));
            span = (bursts - 1) as f32 * spread * 0.001 + dec * 0.0005;
            curves.push((Box::new(move |t| clap_curve(bursts, spread, dec, t)), true));
        }
        _ => {
            let (closed, open) = (q("closed_ms"), q("open_ms"));
            span = open * 0.0005;
            // The curve of the hit that last played is the bright one.
            curves.push((Box::new(move |t| decay(closed, t)), !open_hit));
            curves.push((Box::new(move |t| decay(open, t)), open_hit));
        }
    }
    let span = span.max(0.01);
    let line = alpha(st.roles.disp_text, 90);
    // Time ticks every 10th of the span.
    if d == Detail::Full {
        for k in 1..4 {
            let x = r.left() + r.width() * k as f32 / 4.0;
            p.line_segment(
                [pos2(x, r.top()), pos2(x, r.bottom())],
                Stroke::new(1.0, faint(st)),
            );
        }
    }
    // The dimmer curve first, so the bright one lies over it.
    curves.sort_by_key(|c| c.1);
    for (f, bright) in &curves {
        let pts = plot(r, 96, |x| f(x * span) * 2.0 - 1.0);
        if *bright {
            paint(p, st, pts, z, d, Some(r.bottom()));
        } else {
            p.add(egui::Shape::line(pts, Stroke::new(1.2, line)));
        }
    }
    if let (Some(f), true) = (&pitch, d == Detail::Full) {
        let pts = plot(r, 96, |x| f(x * span) * 1.8 - 0.9);
        p.add(egui::Shape::dashed_line(
            &pts,
            Stroke::new(1.0, line),
            3.0,
            3.0,
        ));
    }
    if d != Detail::Full {
        return;
    }
    caption(
        p,
        st,
        z,
        pos2(r.right(), r.top()),
        Align2::RIGHT_TOP,
        &format!("{:.0} ms", span * 1000.0),
    );
    if let Some(t) = tap {
        let x = t.view.position / span;
        if x <= 1.0 {
            // On the bright curve; the hat's playhead follows the hit it played.
            let f = &curves.last().expect("a curve").0;
            let c = pos2(
                r.left() + r.width() * x,
                r.bottom() - r.height() * f(t.view.position).clamp(0.0, 1.0),
            );
            p.line_segment(
                [pos2(c.x, r.top()), pos2(c.x, r.bottom())],
                Stroke::new(1.0, alpha(st.roles.disp_text, 70)),
            );
            face::playhead(p, st, c, z);
        }
    }
}
