//! The cable settle (HANDOVER section 6): when a cable is plugged, dropped or its module moved,
//! its sag amount runs a damped spring, `a = -w^2 s - 2 zeta w v`, starting taut and swinging
//! to rest. The constants are `style::SETTLE`. A cable at rest has no entry and asks for no
//! frames; the point cache keeps working because only the spring's offset joins its key.

use std::collections::HashMap;

use egui::Pos2;
use kabl_core::CableId;

use crate::style::SETTLE;

#[derive(Clone, Copy)]
struct Spring {
    /// Offset of the sag amount from its resting value, px.
    s: f32,
    /// px per second.
    v: f32,
}

#[derive(Clone, Copy)]
struct Seen {
    /// Ends in model space, to tell a move from a pan or zoom.
    ends: [Pos2; 2],
    /// Moved while the pointer was down: kick once it lets go.
    pending: bool,
}

#[derive(Default)]
pub struct Settle {
    seen: HashMap<CableId, Seen>,
    springs: HashMap<CableId, Spring>,
    frame: Vec<CableId>,
}

/// Longest step of the integrator, s: well below a period (about 0.31 s).
const STEP: f32 = 1.0 / 240.0;

impl Settle {
    /// Notes where cable `id` ends this frame (model space) and whether the pointer is down.
    /// `fresh`: it was just plugged. Starts or retargets the spring when the cable has come to
    /// rest in a new place; `sag` is the cable's resting sag in px.
    pub fn note(
        &mut self,
        id: CableId,
        ends: [Pos2; 2],
        pointer_down: bool,
        fresh: bool,
        sag: f32,
    ) {
        self.frame.push(id);
        let moved = match self.seen.get(&id) {
            Some(prev) => prev
                .ends
                .iter()
                .zip(&ends)
                .any(|(a, b)| (*a - *b).length() > 0.01),
            None => fresh,
        };
        let pending = moved || self.seen.get(&id).is_some_and(|p| p.pending);
        let kick = pending && !pointer_down;
        self.seen.insert(
            id,
            Seen {
                ends,
                pending: pending && !kick,
            },
        );
        if kick {
            let w = std::f32::consts::TAU * SETTLE.hz;
            match self.springs.get_mut(&id) {
                // Already swinging: keep where it is and push it, so nothing jumps.
                Some(sp) => sp.v = sp.v.min(0.0) + w * SETTLE.start * sag,
                None => {
                    self.springs.insert(
                        id,
                        Spring {
                            s: SETTLE.start * sag,
                            v: 0.0,
                        },
                    );
                }
            }
        }
    }

    /// Cable `id` has not been noted before: it was just plugged, or a patch just loaded.
    pub fn unseen(&self, id: CableId) -> bool {
        !self.seen.contains_key(&id)
    }

    /// Offset of cable `id`'s sag, px; 0 when it rests.
    pub fn offset(&self, id: CableId) -> f32 {
        self.springs.get(&id).map_or(0.0, |s| s.s)
    }

    /// Advances every spring by `dt` and forgets the cables not noted this frame. Returns
    /// whether anything still moves (the caller repaints).
    pub fn end_frame(&mut self, dt: f32) -> bool {
        let frame = std::mem::take(&mut self.frame);
        if self.seen.len() > frame.len() {
            self.seen.retain(|id, _| frame.contains(id));
            self.springs.retain(|id, _| frame.contains(id));
        }
        let w = std::f32::consts::TAU * SETTLE.hz;
        let mut left = dt.clamp(0.0, 0.1);
        while left > 0.0 {
            let h = left.min(STEP);
            left -= h;
            for sp in self.springs.values_mut() {
                let a = -w * w * sp.s - 2.0 * SETTLE.zeta * w * sp.v;
                sp.v += a * h;
                sp.s += sp.v * h;
            }
        }
        self.springs
            .retain(|_, sp| sp.s.abs() >= SETTLE.sleep_px || sp.v.abs() >= SETTLE.sleep_px_s);
        !self.springs.is_empty()
    }

    /// Drops every spring (reduced motion): cables rest at once.
    pub fn clear(&mut self) {
        self.springs.clear();
    }

    #[cfg(test)]
    fn moving(&self) -> bool {
        !self.springs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;

    const DT: f32 = 1.0 / 60.0;
    const SAG: f32 = 100.0;

    fn ends(x: f32) -> [Pos2; 2] {
        [pos2(x, 0.0), pos2(x + 300.0, 40.0)]
    }

    /// One frame: note cable 1 at `x`, step.
    fn frame(s: &mut Settle, x: f32, down: bool, fresh: bool) -> bool {
        s.note(1, ends(x), down, fresh, SAG);
        s.end_frame(DT)
    }

    #[test]
    fn a_plug_swings_then_sleeps_and_stops_asking_for_frames() {
        let mut s = Settle::default();
        assert!(frame(&mut s, 0.0, false, true), "starts on the plug");
        assert!((s.offset(1) - SETTLE.start * SAG).abs() < 2.0);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        let mut frames = 0;
        while frame(&mut s, 0.0, false, false) {
            lo = lo.min(s.offset(1));
            hi = hi.max(s.offset(1));
            frames += 1;
            assert!(frames < 600, "never settles");
        }
        assert!(hi > 0.0, "swings past rest once: {hi}");
        // zeta .45: first overshoot is about 20 % of the start offset.
        assert!(hi < 0.3 * SETTLE.start.abs() * SAG, "damped: {hi}");
        assert!(lo >= SETTLE.start * SAG - 1.0);
        assert_eq!(s.offset(1), 0.0, "rests exactly");
        assert!(!s.moving());
        for _ in 0..60 {
            assert!(!frame(&mut s, 0.0, false, false), "idle after sleeping");
        }
        assert!(frames < 120, "settles within about two seconds: {frames}");
    }

    #[test]
    fn a_second_drop_while_swinging_does_not_jump() {
        let mut s = Settle::default();
        frame(&mut s, 0.0, false, true);
        for _ in 0..12 {
            frame(&mut s, 0.0, false, false);
        }
        let before = s.offset(1);
        // Dragged (pointer down): no kick yet. Dropped: kick on the first frame up.
        frame(&mut s, 5.0, true, false);
        let during = s.offset(1);
        frame(&mut s, 5.0, false, false);
        let after = s.offset(1);
        assert!((during - before).abs() < 25.0, "{before} {during}");
        assert!(
            (after - during).abs() < 25.0,
            "no jump at the drop: {during} {after}"
        );
        let mut n = 0;
        while frame(&mut s, 5.0, false, false) {
            n += 1;
            assert!(n < 600);
        }
        assert_eq!(s.offset(1), 0.0);
    }

    #[test]
    fn a_pan_or_a_resting_cable_never_starts_a_spring() {
        let mut s = Settle::default();
        // Seen for the first time with no plug: nothing.
        assert!(!frame(&mut s, 0.0, false, false));
        for _ in 0..30 {
            assert!(!frame(&mut s, 0.0, false, false));
        }
        assert_eq!(s.offset(1), 0.0);
    }

    #[test]
    fn a_dragged_module_kicks_once_on_the_drop() {
        let mut s = Settle::default();
        frame(&mut s, 0.0, false, false);
        for k in 1..20 {
            assert!(
                !frame(&mut s, k as f32 * 3.0, true, false),
                "no spring while dragged"
            );
        }
        assert!(frame(&mut s, 57.0, false, false), "kicks on the drop");
        assert!(s.offset(1) < 0.0);
    }

    #[test]
    fn a_removed_cable_is_forgotten() {
        let mut s = Settle::default();
        frame(&mut s, 0.0, false, true);
        assert!(!s.end_frame(DT), "nothing noted this frame: cable gone");
        assert!(!s.moving());
    }

    #[test]
    fn clearing_rests_every_cable() {
        let mut s = Settle::default();
        frame(&mut s, 0.0, false, true);
        s.clear();
        assert_eq!(s.offset(1), 0.0);
        assert!(!s.moving());
    }
}
