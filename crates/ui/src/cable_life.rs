//! Living cables: what a cable shows of the signal it carries (handover 5.1, 5.3).
//!
//! The face taps report each tapped output's newest sample and its extremes. This turns one
//! report into a `lit` fraction per cable by signal class, smooths it with the class's time
//! constants, and draws the glow as one textured quad strip sampling a small pre-blurred
//! texture. A cable that stays under [`QUIET`] for [`QUIET_SECS`] is static again.

use std::collections::HashMap;

use egui::epaint::{Mesh, Vertex};
use egui::{pos2, Color32, Context, Painter, Pos2, Shape, TextureId};
use kabl_core::CableId;
use kabl_modules::PortType;
use serde::{Deserialize, Serialize};

use crate::displays::LiveTap;

/// Below this a cable counts as dark.
pub const QUIET: f32 = 0.02;
/// A cable dark for this long is completely static.
pub const QUIET_SECS: f64 = 0.5;
/// Repaint interval while any cable is live.
pub const LIVE_REPAINT_MS: u64 = 33;
/// While nothing is live and audio runs, the taps are looked at this often, to see a sound begin.
pub const WATCH_REPAINT_MS: u64 = 100;
/// Extra width of the glow over the core, in points at zoom 1.
pub const GLOW_EXTRA: f32 = 20.0;
/// How fast the halo profile drops across its width (bigger = tighter).
pub const GLOW_PROFILE: f32 = 2.6;

/// Setting "Cable life".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CableLife {
    /// Glow from the live signal, and the travelling light of gates and clocks.
    #[default]
    GlowPulses,
    Glow,
    /// The static look; no taps are asked for.
    Off,
}

impl CableLife {
    pub fn glows(self) -> bool {
        self != CableLife::Off
    }
    pub fn pulses(self) -> bool {
        self == CableLife::GlowPulses
    }
}

/// Setting "Intensity".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intensity {
    Low,
    #[default]
    High,
}

impl Intensity {
    /// Opacity of the glow at `lit` = 1. Gate (b) found the spec's .35 / .60 too strong at the
    /// 20 px halo width: low is about 35 % and high about 60 % of what that was.
    pub fn glow_alpha(self) -> f32 {
        match self {
            Intensity::Low => 0.175,
            Intensity::High => 0.51,
        }
    }
    /// Opacity of the light line on the core at `lit` = 1, scaled the same way.
    pub fn light_alpha(self) -> f32 {
        match self {
            Intensity::Low => 0.095,
            Intensity::High => 0.27,
        }
    }
}

/// How a cable's signal is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Steady glow, slow to fall.
    Audio,
    /// Glow follows the value.
    Control,
    /// Brightness steps with the value.
    Stepped,
}

impl Class {
    /// Gates and clocks keep the flash and travelling light of the cable code: no glow here.
    pub fn of(t: PortType) -> Option<Class> {
        match t {
            PortType::Audio => Some(Class::Audio),
            PortType::Cv | PortType::UnipolarCv => Some(Class::Control),
            PortType::Pitch => Some(Class::Stepped),
            PortType::Gate => None,
        }
    }

    /// Time constants in ms: (attack, release).
    pub fn tau_ms(self) -> (f32, f32) {
        match self {
            Class::Audio => (15.0, 150.0),
            Class::Control | Class::Stepped => (30.0, 30.0),
        }
    }
}

/// The unsmoothed `lit` (0..=1) of one report. `range` is the port type's full scale.
pub fn target(class: Class, tap: &LiveTap, range: f32) -> f32 {
    match class {
        Class::Audio => {
            let peak = tap.min.abs().max(tap.max.abs());
            if peak <= 0.0 {
                return 0.0;
            }
            (((20.0 * peak.log10() + 48.0) / 48.0).clamp(0.0, 1.0)).powf(0.8)
        }
        Class::Control | Class::Stepped => (tap.last.abs() / range.max(1e-6)).clamp(0.0, 1.0),
    }
}

/// `y` moved toward `target` over `dt` seconds with time constant `tau_ms`.
pub fn smooth(y: f32, target: f32, dt: f32, tau_ms: f32) -> f32 {
    y + (target - y) * (1.0 - (-dt / (tau_ms / 1000.0).max(1e-4)).exp())
}

#[derive(Clone, Copy, Default)]
struct Glow {
    lit: f32,
    /// Last time this cable was at or above [`QUIET`] (or its target was).
    live_at: f64,
}

/// Smoothed `lit` per cable, and whether anything needs frames.
#[derive(Default)]
pub struct Life {
    /// Reduced motion: the glow lets go at half speed.
    pub slow_release: bool,
    cables: HashMap<CableId, Glow>,
    live_until: f64,
    seen: Vec<CableId>,
}

impl Life {
    /// Moves cable `id` toward `target` (`None`: not tapped) and returns the `lit` to draw:
    /// 0 once the cable has been dark for [`QUIET_SECS`].
    pub fn update(
        &mut self,
        id: CableId,
        class: Class,
        target: Option<f32>,
        dt: f32,
        now: f64,
    ) -> f32 {
        self.seen.push(id);
        let g = self.cables.entry(id).or_default();
        let t = target.unwrap_or(0.0);
        let (attack, release) = class.tau_ms();
        let release = release * if self.slow_release { 2.0 } else { 1.0 };
        g.lit = smooth(g.lit, t, dt, if t > g.lit { attack } else { release });
        if g.lit >= QUIET || t >= QUIET {
            g.live_at = now;
            self.live_until = self.live_until.max(now + QUIET_SECS);
        }
        if now - g.live_at > QUIET_SECS || g.lit < QUIET {
            if now - g.live_at > QUIET_SECS {
                g.lit = 0.0;
            }
            return 0.0;
        }
        g.lit
    }

    /// Forgets the cables not updated since the last call.
    pub fn end_frame(&mut self) {
        let seen = std::mem::take(&mut self.seen);
        if self.cables.len() > seen.len() {
            self.cables.retain(|id, _| seen.contains(id));
        }
    }

    /// Some cable is lit, or was within the last [`QUIET_SECS`].
    pub fn live(&self, now: f64) -> bool {
        now < self.live_until
    }
}

/// Peak of the flash on a rising edge, as a `lit`: the cable flashes to the full glow of the
/// chosen intensity (gate (b): a third of it could not be seen on a clock cable).
pub const FLASH_PEAK: f32 = 1.0;
/// Decay of the flash, in ms.
pub const FLASH_TAU_MS: f32 = 70.0;

#[derive(Default)]
struct Edge {
    prev_high: bool,
    seen_at: Option<f64>,
    flash: f32,
}

/// Gates and clocks: a flash of the cable on each rising edge of the source, decaying with
/// tau 70 ms. Nothing travels along the cable: a steady stream of travelling dots looks like a
/// wagon wheel under a 60 Hz capture, and is too fast to follow by eye.
#[derive(Default)]
pub struct Pulses {
    /// Reduced motion: the flash decays at half speed.
    pub slow_release: bool,
    cables: HashMap<CableId, Edge>,
    seen: Vec<CableId>,
}

impl Pulses {
    /// Takes the newest report of the cable's source and returns the flash (a `lit`).
    pub fn update(&mut self, id: CableId, tap: Option<&LiveTap>, dt: f32) -> f32 {
        self.seen.push(id);
        let e = self.cables.entry(id).or_default();
        if let Some(t) = tap.filter(|t| e.seen_at != Some(t.at)) {
            e.seen_at = Some(t.at);
            let high = t.max >= 0.5;
            if high && !e.prev_high {
                e.flash = FLASH_PEAK;
            }
            e.prev_high = high;
        }
        let tau = FLASH_TAU_MS * if self.slow_release { 2.0 } else { 1.0 };
        e.flash *= (-dt / (tau / 1000.0)).exp();
        e.flash
    }

    /// Something still flashes: frames are needed.
    pub fn live(&self) -> bool {
        self.cables.values().any(|e| e.flash >= QUIET)
    }

    pub fn end_frame(&mut self) {
        let seen = std::mem::take(&mut self.seen);
        if self.cables.len() > seen.len() {
            self.cables.retain(|id, _| seen.contains(id));
        }
    }
}

/// Gaussian falloff from the centre (`x` = 0) to nothing at the edge (`|x|` = 1); `k` is how
/// fast it drops (bigger = tighter).
pub fn falloff(x: f32, k: f32) -> f32 {
    let edge = (-k).exp();
    (((-k * x * x).exp() - edge) / (1.0 - edge)).max(0.0)
}

fn glow_texture(ctx: &Context, k: f32) -> TextureId {
    crate::kit::blur::texture(ctx, &format!("cable-glow-{k}"), 32, 1, |u, _| falloff(u, k))
}

/// The glow of one cable: a strip of quads along `pts`, `width` across, sampling the blurred
/// profile (`k`: how tight), tinted `col` at opacity `alpha`. Two vertices per point.
pub fn paint_glow(painter: &Painter, pts: &[Pos2], width: f32, col: Color32, alpha: f32, k: f32) {
    if pts.len() < 2 || alpha <= 0.0 {
        return;
    }
    let tint = col.gamma_multiply(alpha.clamp(0.0, 1.0));
    painter.add(Shape::mesh(strip(
        glow_texture(painter.ctx(), k),
        pts,
        width,
        tint,
    )));
}

/// The quad strip of [`paint_glow`].
fn strip(tex: TextureId, pts: &[Pos2], width: f32, tint: Color32) -> Mesh {
    let half = width * 0.5;
    let mut mesh = Mesh::with_texture(tex);
    mesh.vertices.reserve(pts.len() * 2);
    mesh.indices.reserve((pts.len() - 1) * 6);
    for i in 0..pts.len() {
        let a = pts[i.saturating_sub(1)];
        let b = pts[(i + 1).min(pts.len() - 1)];
        let d = (b - a).normalized();
        let n = egui::vec2(-d.y, d.x) * half;
        let n = if n.x.is_finite() && n.y.is_finite() {
            n
        } else {
            egui::Vec2::ZERO
        };
        mesh.vertices.push(Vertex {
            pos: pts[i] - n,
            uv: pos2(0.0, 0.5),
            color: tint,
        });
        mesh.vertices.push(Vertex {
            pos: pts[i] + n,
            uv: pos2(1.0, 0.5),
            color: tint,
        });
        if i > 0 {
            let k = (i * 2) as u32;
            mesh.indices
                .extend_from_slice(&[k - 2, k - 1, k, k - 1, k + 1, k]);
        }
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use kabl_modules::ModuleView;

    fn tap(last: f32, min: f32, max: f32) -> LiveTap {
        LiveTap {
            last,
            min,
            max,
            view: ModuleView::default(),
            at: 0.0,
        }
    }

    #[test]
    fn audio_curve_matches_the_spec() {
        assert_eq!(target(Class::Audio, &tap(0.0, 0.0, 0.0), 1.0), 0.0);
        let full = target(Class::Audio, &tap(1.0, -1.0, 1.0), 1.0);
        assert!((full - 1.0).abs() < 1e-5);
        // -48 dB is the floor.
        assert!(target(Class::Audio, &tap(0.0, -0.003, 0.003), 1.0) < 0.01);
        // -6 dB: ((20 log10 .5 + 48) / 48)^.8
        let half = target(Class::Audio, &tap(0.0, -0.5, 0.4), 1.0);
        let want = ((20.0f32 * 0.5f32.log10() + 48.0) / 48.0).powf(0.8);
        assert!((half - want).abs() < 1e-5);
    }

    #[test]
    fn control_follows_last_over_range() {
        assert!((target(Class::Control, &tap(-0.5, 0.0, 0.0), 1.0) - 0.5).abs() < 1e-6);
        assert!((target(Class::Stepped, &tap(12.0, 0.0, 0.0), 60.0) - 0.2).abs() < 1e-6);
        assert_eq!(target(Class::Control, &tap(3.0, 0.0, 0.0), 1.0), 1.0);
    }

    #[test]
    fn audio_attacks_faster_than_it_releases() {
        let up = smooth(0.0, 1.0, 0.015, Class::Audio.tau_ms().0);
        let down = 1.0 - smooth(1.0, 0.0, 0.015, Class::Audio.tau_ms().1);
        assert!(up > 0.6 && down < 0.15, "{up} {down}");
    }

    #[test]
    fn a_cable_dark_for_half_a_second_is_static() {
        let mut life = Life::default();
        let dt = 1.0 / 30.0;
        let mut now = 0.0;
        for _ in 0..10 {
            assert!(life.update(1, Class::Audio, Some(1.0), dt, now) > 0.0);
            life.end_frame();
            now += f64::from(dt);
        }
        assert!(life.live(now));
        let off = now;
        let mut last = 1.0;
        while now - off < 3.0 {
            last = life.update(1, Class::Audio, Some(0.0), dt, now);
            life.end_frame();
            now += f64::from(dt);
        }
        assert_eq!(last, 0.0);
        assert!(!life.live(now), "idle must not ask for frames");
    }

    fn gate(high: bool, at: f64) -> LiveTap {
        let v = f32::from(high);
        LiveTap {
            at,
            ..tap(v, 0.0, v)
        }
    }

    #[test]
    fn a_rising_edge_flashes_once_to_full_and_decays_with_tau_70_ms() {
        let mut p = Pulses::default();
        let dt = 1.0 / 60.0;
        let mut now = 0.0;
        let mut peaks = 0;
        let mut prev = 0.0;
        // Low, then high for 10 reports (a held gate), then low.
        for k in 0..60 {
            let high = (5..15).contains(&k);
            let f = p.update(1, Some(&gate(high, now)), dt);
            p.end_frame();
            if f > prev + 0.1 {
                peaks += 1;
                assert!(f <= FLASH_PEAK + 1e-6, "no more than full: {f}");
            }
            prev = f;
            now += f64::from(dt);
        }
        assert_eq!(peaks, 1, "one flash for the held gate");
        assert!(!p.live(), "gone after about half a second");
        let mut p = Pulses::default();
        let f = p.update(1, Some(&gate(true, 0.0)), 0.0);
        assert!((f - FLASH_PEAK).abs() < 1e-6);
        let f = p.update(1, None, FLASH_TAU_MS / 1000.0);
        assert!((f - FLASH_PEAK * (-1.0f32).exp()).abs() < 1e-5, "{f}");
    }

    #[test]
    fn reduced_motion_doubles_the_glow_release() {
        let release = |slow: bool| {
            let mut life = Life {
                slow_release: slow,
                ..Life::default()
            };
            for k in 0..20 {
                life.update(1, Class::Audio, Some(1.0), 0.015, f64::from(k) * 0.015);
            }
            life.update(1, Class::Audio, Some(0.0), 0.15, 0.4)
        };
        // After one release tau (150 ms) the normal glow is at 1/e, the slowed one far above.
        assert!(
            release(true) > release(false) + 0.15,
            "{} {}",
            release(true),
            release(false)
        );
        let flash = |slow: bool| {
            let mut p = Pulses {
                slow_release: slow,
                ..Pulses::default()
            };
            p.update(1, Some(&gate(true, 0.0)), 0.0);
            p.update(1, None, FLASH_TAU_MS / 1000.0)
        };
        assert!((flash(true) - (-0.5f32).exp()).abs() < 1e-5);
        assert!((flash(false) - (-1.0f32).exp()).abs() < 1e-5);
    }

    #[test]
    fn unseen_cables_are_forgotten() {
        let mut life = Life::default();
        life.update(1, Class::Audio, Some(1.0), 0.03, 0.0);
        life.update(2, Class::Audio, Some(1.0), 0.03, 0.0);
        life.end_frame();
        life.update(2, Class::Audio, Some(1.0), 0.03, 0.1);
        life.end_frame();
        assert_eq!(life.cables.len(), 1);
    }

    #[test]
    fn glow_strip_is_two_vertices_per_point() {
        let pts: Vec<Pos2> = (0..41).map(|i| pos2(i as f32 * 5.0, 20.0)).collect();
        let m = strip(TextureId::Managed(1), &pts, 14.0, Color32::WHITE);
        assert_eq!(m.vertices.len(), 82);
        assert_eq!(m.indices.len(), 40 * 6);
        // Straight cable: the strip is `width` across.
        assert!((m.vertices[10].pos.y - m.vertices[11].pos.y).abs() - 14.0 < 1e-3);
    }
}
