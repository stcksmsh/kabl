//! Rack drawing shared by the rack, faces and wavetable scenes: rails, faces, cables.
use crate::faces::*;
use crate::prim::*;
use crate::tokens::Dir;
use crate::widgets::*;
use egui::{pos2, vec2, Color32, Pos2, Rect, Vec2};
use kabl_core::{PatchState, PortRef};
use kabl_ui::rack::{layout, Layout, View, PANEL_H};
use std::collections::BTreeMap;

pub fn load(name: &str) -> PatchState {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../patches")
        .join(name);
    kabl_core::load(&dir).unwrap().state().clone()
}

pub struct Cab {
    pub from: (u32, String),
    pub to: (u32, String),
}

pub fn cables(st: &PatchState) -> Vec<Cab> {
    st.cables
        .values()
        .filter_map(|c| match (&c.from, &c.to) {
            (PortRef::Module { id: a, port: ap }, PortRef::Module { id: b, port: bp }) => {
                Some(Cab {
                    from: (*a as u32, ap.to_string()),
                    to: (*b as u32, bp.to_string()),
                })
            }
            _ => None,
        })
        .collect()
}

pub struct Rack {
    pub st: PatchState,
    pub lay: Layout,
    pub cabs: Vec<Cab>,
}

impl Rack {
    pub fn new(mut st: PatchState) -> Self {
        // Demo values from docs/design/BRIEF.md's reference patch, so every display has something to show.
        for m in st.modules.values_mut() {
            let set: &[(&str, f32)] = match m.kind.as_str() {
                "osc.va" => &[("base_hz", 261.6), ("waveform", 2.0)],
                "filter.svf" => &[("cutoff_hz", 1200.0), ("resonance", 0.35)],
                "env.adsr" => &[
                    ("attack_ms", 8.0),
                    ("decay_ms", 240.0),
                    ("sustain", 0.6),
                    ("release_ms", 420.0),
                ],
                "vca" => &[("gain", 0.85)],
                _ => &[],
            };
            for (n, v) in set {
                m.params.insert(n.to_string(), *v);
            }
        }
        let lay = layout(&st, &View::default());
        let cabs = cables(&st);
        Rack { st, lay, cabs }
    }

    pub fn world(&self) -> Rect {
        let mut r = Rect::NOTHING;
        for m in &self.lay.mods {
            r = r.union(m.face);
        }
        r
    }

    fn geoms(&self) -> Vec<(u32, Geom, Pos2)> {
        self.lay
            .mods
            .iter()
            .map(|m| (m.id as u32, geom(m.info.kind, m), m.face.min))
            .collect()
    }

    /// World position of a jack, using the override layouts.
    pub fn jack_pos(&self, id: u32, port: &str, out: bool) -> Option<Pos2> {
        let m = self.lay.mods.iter().find(|m| m.id as u32 == id)?;
        let g = geom(m.info.kind, m);
        let j = g.jacks.iter().find(|j| j.name == port && j.out == out)?;
        Some(m.face.min + j.c.to_vec2())
    }

    pub fn conn(&self, id: u32, port: &str, out: bool) -> bool {
        self.cabs.iter().any(|c| {
            if out {
                c.from.0 == id && c.from.1 == port
            } else {
                c.to.0 == id && c.to.1 == port
            }
        })
    }

    pub fn rails(&self, cx: &Cx, xf: Xf, x0: f32, x1: f32) {
        let k = cx.k;
        let rows = self.lay.rows;
        for row in 0..rows {
            let y = 10.0 + row as f32 * 370.0;
            for ry in [y - 16.0, y + PANEL_H + 3.0] {
                let r = xf.r(Rect::from_min_max(pos2(x0, ry), pos2(x1, ry + 13.0)));
                match k.dir {
                    Dir::A => {
                        cx.grad(
                            r,
                            1.0,
                            mix(k.rail, Color32::WHITE, 0.2),
                            mix(k.rail, Color32::BLACK, 0.2),
                        );
                    }
                    Dir::B => cx.rr(r, 3.0, k.rail),
                    Dir::C => {
                        cx.rr(r, 0.0, k.rail);
                        cx.rr_stroke(r, 0.0, 1.0, k.line2);
                    }
                }
                let mut x = x0 + 15.0;
                while x < x1 - 6.0 {
                    let c = xf.p(pos2(x, ry + 6.5));
                    cx.p.circle_filled(c, 2.4 * xf.s.max(0.8), k.hole);
                    x += 30.0;
                }
            }
        }
    }

    /// Faces, then cables. `sel` is the highlighted module id.
    pub fn draw(
        &self,
        cx: &Cx,
        xf: Xf,
        sel: Option<u32>,
        lfo_to_filter: bool,
        cable_alpha: f32,
        focus_mode: bool,
    ) {
        let k = cx.k;
        for (id, g, org) in self.geoms() {
            let m = self.lay.mods.iter().find(|m| m.id as u32 == id).unwrap();
            let params = &self.st.modules[&(id as _)].params;
            let conn = |p: &str, o: bool| self.conn(id, p, o);
            let mut f = FaceIn {
                id,
                kind: m.info.kind,
                info: m.info,
                params,
                geom: g,
                origin: org,
                conn: &conn,
                selected: sel == Some(id),
                sweep: 0.0,
                hot: None,
                name: None,
            };
            if m.info.kind == "filter.svf" && lfo_to_filter {
                f.sweep = 0.75;
            }
            draw_face(cx, xf, &f);
        }
        if cable_alpha <= 0.0 {
            return;
        }
        let mut order: Vec<usize> = (0..self.cabs.len()).collect();
        order.sort_by_key(|&i| (self.cabs[i].from.1 == "out") as u8);
        for (n, &i) in order.iter().enumerate() {
            let c = &self.cabs[i];
            let (Some(p0), Some(p1)) = (
                self.jack_pos(c.from.0, &c.from.1, true),
                self.jack_pos(c.to.0, &c.to.1, false),
            ) else {
                continue;
            };
            let ty = self
                .lay
                .mods
                .iter()
                .find(|m| m.id as u32 == c.from.0)
                .and_then(|m| m.info.ports.iter().find(|p| p.name == c.from.1))
                .map(|p| p.port_type);
            let col = ty.map_or(k.audio, |t| k.signal(t));
            let (s0, s1) = (xf.p(p0), xf.p(p1));
            let pts = cable_pts(s0, s1, 46.0 * xf.s);
            let touch = sel.is_some_and(|s| s == c.from.0 || s == c.to.0);
            let al = if focus_mode && !touch {
                cable_alpha * 0.22
            } else {
                cable_alpha
            };
            cable(cx, pts.clone(), col, al, touch);
            plug(cx, s0, col, al);
            plug(cx, s1, col, al);
            // Signal beads: speed independent of length.
            let len = poly_len(&pts).max(1.0);
            let ph = (cx.t * 90.0 / len + n as f32 * 0.37).fract();
            let b = along(&pts, ph);
            cx.p.circle_filled(b, 2.6, a(Color32::WHITE, (220.0 * al) as u8));
        }
    }
}

pub fn fit(world: Rect, avail: Rect, margin: Vec2, max_s: f32) -> Xf {
    let s = ((avail.width() - margin.x * 2.0) / world.width())
        .min((avail.height() - margin.y * 2.0) / world.height())
        .min(max_s);
    let used = world.size() * s;
    let o = avail.min.to_vec2() + (avail.size() - used) / 2.0 - world.min.to_vec2() * s;
    Xf { s, o }
}

pub fn _unused(_: BTreeMap<u32, u32>, _: Vec2) -> Vec2 {
    vec2(0.0, 0.0)
}
