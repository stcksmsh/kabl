//! Layout and hit regions. Drawing and hit-testing both read this module, so what is drawn is
//! what is hit. World units are panel pixels at 100 % zoom (1u = 30 px, panel height 340).

use crate::model::{def, CtlRef, JackRef, Kind, Patch, Spec};
use egui::{pos2, vec2, Pos2, Rect};

pub const UNIT: f32 = 30.0;
pub const PANEL_H: f32 = 340.0;
pub const ROW_Y: [f32; 2] = [10.0, 380.0];
pub const RACK_X: f32 = 24.0;
pub const COMPACT_H: f32 = 240.0;
pub const R_LARGE: f32 = 22.0;
pub const R_SMALL: f32 = 17.0;
pub const JACK_R: f32 = 13.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cables {
    All,
    Focus,
    Hidden,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HiddenLayout {
    StableRack,
    CompactSynth,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Expansion {
    Push,
    Float,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DepthGesture {
    /// Spec: outer ring band drags depth, body drags base.
    RingBand,
    /// Alternative: only a handle at the peak dot drags depth.
    PeakHandle,
    /// Alternative: depth only in the inspector / route card.
    InspectorOnly,
}

/// View state that decides geometry. None of it is undoable patch data.
#[derive(Clone)]
pub struct ViewGeo {
    pub cables: Cables,
    pub hidden_layout: HiddenLayout,
    pub expansion: Expansion,
    pub depth: DepthGesture,
    pub expanded: Vec<bool>,
    pub choose: Option<usize>,
    /// Canvas width in world units, for the compact layout's wrapping.
    pub wrap_w: f32,
    /// The route whose depth a ring/handle drag adjusts, when it targets that knob.
    pub selected_route: Option<usize>,
}

impl ViewGeo {
    pub fn compact(&self) -> bool {
        self.cables == Cables::Hidden && self.hidden_layout == HiddenLayout::CompactSynth
    }
    pub fn is_expanded(&self, m: usize) -> bool {
        self.expanded.get(m).copied().unwrap_or(false) || self.choose == Some(m)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum CtlGeo {
    Knob { c: Pos2, r: f32 },
    Select { rect: Rect },
}

impl CtlGeo {
    pub fn label_pos(&self) -> Pos2 {
        match *self {
            CtlGeo::Knob { c, .. } => c + vec2(0.0, -47.0),
            CtlGeo::Select { rect } => pos2(rect.center().x, rect.top() - 13.0),
        }
    }
    pub fn pill_rect(&self, text: &str) -> Rect {
        let w = (text.chars().count() as f32 * 7.0 + 10.0).max(44.0);
        match *self {
            CtlGeo::Knob { c, .. } => Rect::from_center_size(c + vec2(0.0, 38.0), vec2(w, 20.0)),
            CtlGeo::Select { rect } => rect,
        }
    }
    /// Pin sits right of the label text (and of a ◆ mark, if any).
    pub fn pin_rect(&self, label: &str) -> Rect {
        let dx = label.chars().count() as f32 * 3.7 + 22.0;
        Rect::from_center_size(self.label_pos() + vec2(dx, 0.0), vec2(18.0, 18.0))
    }
}

#[derive(Clone, Debug)]
pub struct Placed {
    /// Panel rect: face plus, under Push, the advanced area.
    pub rect: Rect,
    pub face_w: f32,
    /// Advanced area in world coordinates, when expanded.
    pub adv: Option<Rect>,
    /// Advanced area floats over neighbours (Float variant).
    pub overlay: bool,
    pub compact: bool,
    pub ctls: Vec<Option<CtlGeo>>,
    /// Jack centres (rack) or chip rects (compact).
    pub jacks: Vec<Rect>,
    /// `+N` / `Less` toggle; absent for modules with nothing to hide.
    pub toggle: Option<Rect>,
    /// `Done` in choose mode.
    pub done: Option<Rect>,
    pub preview: Option<Rect>,
}

impl Placed {
    pub fn jack_center(&self, j: usize) -> Pos2 {
        self.jacks[j].center()
    }
}

pub struct Layout {
    pub mods: Vec<Placed>,
    pub bounds: Rect,
}

fn knob_size(declared_large: bool, n: usize) -> f32 {
    if n == 1 || (n == 2 && declared_large) {
        R_LARGE
    } else {
        R_SMALL
    }
}

/// Face width in units. LFO follows the revision-2 table; other kinds keep their base width
/// until their primary knobs need more room (56 px spacing, 36 px margins).
pub fn face_wu(kind: Kind, n_knobs: usize) -> u32 {
    let base = def(kind).base_wu;
    if kind == Kind::Lfo {
        return match n_knobs {
            0..=2 => 7,
            3 => 8,
            4 => 10,
            n => 10 + 2 * (n as u32 - 4),
        };
    }
    if n_knobs < 3 {
        return base;
    }
    base.max(((72.0 + 56.0 * (n_knobs as f32 - 1.0)) / UNIT).ceil() as u32)
}

/// Wide enough for the longest option in 11.5 px mono on every segment.
fn sel_width(options: &[&str]) -> f32 {
    let longest = options.iter().map(|o| o.chars().count()).max().unwrap_or(1) as f32;
    (7.0 * longest + 14.0).max(30.0) * options.len() as f32
}

pub fn layout(p: &Patch, v: &ViewGeo) -> Layout {
    let compact = v.compact();
    let mut mods: Vec<Placed> = p
        .modules
        .iter()
        .enumerate()
        .map(|(i, _)| place_local(p, i, v.is_expanded(i), compact, v.choose == Some(i)))
        .collect();
    if v.expansion == Expansion::Float {
        for pl in &mut mods {
            if pl.adv.is_some() {
                pl.overlay = true;
                pl.rect.max.x = pl.rect.min.x + pl.face_w;
            }
        }
    }

    // Position: stable rack rows, or the compact synth flow.
    let order: Vec<usize> = if compact {
        let rank = |k: Kind| match k {
            Kind::Midi => 0,
            Kind::Osc => 1,
            Kind::Filter => 2,
            Kind::Vca => 3,
            Kind::Out => 4,
            Kind::Lfo => 5,
            Kind::Adsr => 6,
            Kind::Ensemble => 7,
        };
        let mut o: Vec<usize> = (0..p.modules.len()).collect();
        o.sort_by_key(|&i| (rank(p.modules[i].kind), i));
        o
    } else {
        (0..p.modules.len()).collect()
    };
    let mut row_x = [RACK_X; 2];
    let (mut fx, mut fy) = (RACK_X, ROW_Y[0]);
    for &i in &order {
        let pl = &mods[i];
        let slot_w = pl.rect.width();
        let origin = if compact {
            if fx > RACK_X && fx + slot_w > v.wrap_w - 12.0 {
                fx = RACK_X;
                fy += COMPACT_H + 20.0;
            }
            let o = pos2(fx, fy);
            fx += slot_w + 8.0;
            o
        } else {
            let row = p.modules[i].row.min(1);
            let o = pos2(row_x[row], ROW_Y[row]);
            row_x[row] += slot_w;
            o
        };
        translate(&mut mods[i], origin.to_vec2());
    }
    let mut bounds = Rect::NOTHING;
    for m in &mods {
        bounds = bounds.union(m.rect);
        if let Some(a) = m.adv {
            bounds = bounds.union(a);
        }
    }
    Layout { mods, bounds }
}

fn translate(pl: &mut Placed, d: egui::Vec2) {
    pl.rect = pl.rect.translate(d);
    pl.adv = pl.adv.map(|r| r.translate(d));
    for c in pl.ctls.iter_mut().flatten() {
        match c {
            CtlGeo::Knob { c, .. } => *c += d,
            CtlGeo::Select { rect } => *rect = rect.translate(d),
        }
    }
    for j in &mut pl.jacks {
        *j = j.translate(d);
    }
    pl.toggle = pl.toggle.map(|r| r.translate(d));
    pl.done = pl.done.map(|r| r.translate(d));
    pl.preview = pl.preview.map(|r| r.translate(d));
}

/// Geometry of one module at the origin.
fn place_local(p: &Patch, i: usize, expanded: bool, compact: bool, choosing: bool) -> Placed {
    let m = &p.modules[i];
    let d = m.def();
    let n = d.controls.len();
    let prim_knobs: Vec<usize> =
        (0..n).filter(|&c| m.primary[c] && matches!(d.controls[c].spec, Spec::Knob { .. })).collect();
    let prim_sels: Vec<usize> =
        (0..n).filter(|&c| m.primary[c] && matches!(d.controls[c].spec, Spec::Select { .. })).collect();
    let mut fw = face_wu(m.kind, prim_knobs.len()) as f32 * UNIT;
    if compact && n == 0 {
        fw = 5.0 * UNIT;
    }
    let knob_y = if compact { 82.0 } else { d.knob_y };
    let sel_y = knob_y + if compact { 74.0 } else { 76.0 };
    let mut ctls: Vec<Option<CtlGeo>> = vec![None; n];

    let k = prim_knobs.len();
    for (idx, &c) in prim_knobs.iter().enumerate() {
        let large = matches!(d.controls[c].spec, Spec::Knob { large: true, .. });
        let r = knob_size(large, k);
        let x = match k {
            1 => fw / 2.0,
            2 => fw * (idx as f32 + 0.5) / 2.0,
            _ => 36.0 + idx as f32 * (fw - 72.0) / (k as f32 - 1.0),
        };
        ctls[c] = Some(CtlGeo::Knob { c: pos2(x, knob_y), r });
    }
    let total_opts: usize = prim_sels.iter().map(|&c| opts(d.controls[c].spec)).sum();
    let mut x = 14.0;
    for &c in &prim_sels {
        let avail = fw - 28.0 - 10.0 * (prim_sels.len() as f32 - 1.0);
        let w = avail * opts(d.controls[c].spec) as f32 / total_opts as f32;
        ctls[c] = Some(CtlGeo::Select { rect: Rect::from_min_size(pos2(x, sel_y), vec2(w, 28.0)) });
        x += w + 10.0;
    }

    // Advanced area: non-primary controls, appended right of the face.
    let hidden: Vec<usize> = (0..n).filter(|&c| !m.primary[c]).collect();
    let mut adv = None;
    let mut preview = None;
    if expanded && !hidden.is_empty() {
        let hk: Vec<usize> =
            hidden.iter().copied().filter(|&c| matches!(d.controls[c].spec, Spec::Knob { .. })).collect();
        let hs: Vec<usize> =
            hidden.iter().copied().filter(|&c| matches!(d.controls[c].spec, Spec::Select { .. })).collect();
        let cols = hk.len().min(5);
        let sel_row_w: f32 = hs.iter().map(|&c| sel_width(options_of(d.controls[c].spec))).sum::<f32>()
            + 12.0 * hs.len().saturating_sub(1) as f32
            + 28.0;
        let knob_w = if cols > 0 { 60.0 + 76.0 * (cols as f32 - 1.0) } else { 0.0 };
        let aw = (knob_w.max(sel_row_w.min(13.0 * UNIT)).max(5.0 * UNIT) / UNIT).ceil() * UNIT;
        let mut y = knob_y;
        for (idx, &c) in hk.iter().enumerate() {
            if idx > 0 && idx % 5 == 0 {
                y += 90.0;
            }
            let col = idx % 5;
            let x0 = (aw - 76.0 * (cols as f32 - 1.0)) / 2.0;
            ctls[c] = Some(CtlGeo::Knob { c: pos2(fw + x0 + 76.0 * col as f32, y), r: R_SMALL });
        }
        let mut sy = if hk.is_empty() { knob_y - 20.0 } else { y + 76.0 };
        let mut sx = 14.0;
        for &c in &hs {
            let w = sel_width(options_of(d.controls[c].spec));
            if sx > 14.0 && sx + w > aw - 14.0 {
                sx = 14.0;
                sy += 60.0;
            }
            ctls[c] =
                Some(CtlGeo::Select { rect: Rect::from_min_size(pos2(fw + sx, sy), vec2(w, 28.0)) });
            sx += w + 12.0;
        }
        let h = if compact { COMPACT_H } else { PANEL_H };
        if m.kind == Kind::Lfo {
            // Mock output preview: beside the last selector row when there is room, else below.
            if aw - 14.0 - sx >= 150.0 {
                preview = Some(Rect::from_min_size(pos2(fw + sx, sy - 16.0), vec2(aw - 14.0 - sx, 60.0)));
            } else if sy + 44.0 + 64.0 < h - 8.0 {
                preview = Some(Rect::from_min_size(pos2(fw + 14.0, sy + 44.0), vec2(aw - 28.0, 64.0)));
            }
        }
        adv = Some(Rect::from_min_size(pos2(fw, 0.0), vec2(aw, h)));
    }

    // Jacks: sockets on the rack face, or chips in the compact layout.
    let mut h = if compact { COMPACT_H } else { PANEL_H };
    let jacks: Vec<Rect> = if compact {
        let cols = if fw >= 200.0 { 2 } else { 1 };
        let cw = (fw - 20.0 - 6.0 * (cols as f32 - 1.0)) / cols as f32;
        let y0 = if n == 0 { 52.0 } else { 194.0 };
        let rects: Vec<Rect> = (0..d.jacks.len())
            .map(|j| {
                let (col, row) = (j % cols, j / cols);
                Rect::from_min_size(
                    pos2(10.0 + col as f32 * (cw + 6.0), y0 + row as f32 * 26.0),
                    vec2(cw, 22.0),
                )
            })
            .collect();
        if let Some(last) = rects.last() {
            h = h.max(last.bottom() + 10.0);
        }
        rects
    } else {
        d.jacks
            .iter()
            .map(|j| {
                let x = if j.x < 0.0 {
                    fw + j.x
                } else if j.x == 0.0 {
                    fw / 2.0
                } else {
                    j.x
                };
                Rect::from_center_size(pos2(x, j.y), vec2(2.0 * JACK_R, 2.0 * JACK_R))
            })
            .collect()
    };
    if let Some(a) = adv.as_mut() {
        a.max.y = h;
    }

    let toggle = (!hidden.is_empty() || m.primary.iter().any(|p| !p))
        .then(|| Rect::from_min_size(pos2(fw - 46.0, 12.0), vec2(36.0, 26.0)));
    let done = choosing.then(|| Rect::from_min_size(pos2(10.0, 12.0), vec2(52.0, 26.0)));
    let push_w = adv.map(|a| a.width()).unwrap_or(0.0);
    Placed {
        rect: Rect::from_min_size(Pos2::ZERO, vec2(fw + push_w, h)),
        face_w: fw,
        adv,
        overlay: false,
        compact,
        ctls,
        jacks,
        toggle,
        done,
        preview,
    }
}

fn options_of(s: Spec) -> &'static [&'static str] {
    match s {
        Spec::Select { options, .. } => options,
        _ => &[],
    }
}

fn opts(s: Spec) -> usize {
    match s {
        Spec::Select { options, .. } => options.len(),
        _ => 1,
    }
}

// ------------------------------------------------------------------------------ angles

pub const SWEEP_START: f32 = 135.0;
pub const SWEEP: f32 = 270.0;

pub fn angle(t: f32) -> f32 {
    (SWEEP_START + SWEEP * t).to_radians()
}

pub fn polar(c: Pos2, r: f32, t: f32) -> Pos2 {
    let a = angle(t);
    c + vec2(a.cos(), a.sin()) * r
}

// ------------------------------------------------------------------------------ routes on knobs

/// Plug positions at the 6 o'clock gap: up to 3 fan out 15 px apart; the rest share a `+N` plug.
pub fn plug_pos(c: Pos2, r: f32, idx: usize, n: usize) -> Pos2 {
    let shown = n.min(3);
    let i = idx.min(2) as f32;
    c + vec2((i - (shown as f32 - 1.0) / 2.0) * 15.0, r + 8.0)
}

// ------------------------------------------------------------------------------ hit regions

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Hit {
    Toggle(usize),
    Done(usize),
    Pin(CtlRef),
    Plug(usize),
    Handle(usize),
    Pill(CtlRef),
    Knob(CtlRef),
    Ring(CtlRef),
    Seg(CtlRef, usize),
    Jack(JackRef),
    Header(usize),
    Panel(usize),
}

#[derive(Clone, Copy, Debug)]
pub enum Area {
    Circle(Pos2, f32),
    Annulus(Pos2, f32, f32),
    Box(Rect),
}

impl Area {
    pub fn contains(&self, p: Pos2) -> bool {
        match *self {
            Area::Circle(c, r) => c.distance(p) <= r,
            Area::Annulus(c, r0, r1) => (r0..=r1).contains(&c.distance(p)),
            Area::Box(r) => r.contains(p),
        }
    }
    pub fn center(&self) -> Pos2 {
        match *self {
            Area::Circle(c, _) => c,
            // A point on the band at 12 o'clock: what a pointer aiming at the ring would press.
            Area::Annulus(c, r0, r1) => c - vec2(0.0, (r0 + r1) / 2.0),
            Area::Box(r) => r.center(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Region {
    pub hit: Hit,
    pub area: Area,
    /// 1 = floating advanced area, drawn over neighbours.
    pub layer: u8,
    /// Lower wins inside a layer.
    pub prio: u8,
}

pub const BODY_R_ROUTED: f32 = 4.0;
pub const BODY_R_FREE: f32 = 8.0;
pub const RING_R0: f32 = 5.0;
pub const RING_R1: f32 = 15.0;
pub const HANDLE_R: f32 = 9.0;

/// The route whose depth a gesture on this knob adjusts: the selected route when it targets
/// this knob, else the most recent one.
pub fn depth_route(p: &Patch, dst: CtlRef, selected: Option<usize>) -> Option<usize> {
    if let Some(s) = selected {
        if p.routes.get(s).is_some_and(|r| r.dst == dst) {
            return Some(s);
        }
    }
    p.routes_to(dst).map(|(i, _)| i).last()
}

pub fn regions(p: &Patch, v: &ViewGeo, l: &Layout) -> Vec<Region> {
    let mut out = Vec::new();
    let hidden = v.cables == Cables::Hidden;
    for (mi, (m, pl)) in p.modules.iter().zip(&l.mods).enumerate() {
        let d = m.def();
        for (c, g) in pl.ctls.iter().enumerate() {
            let Some(g) = g else { continue };
            let cref = CtlRef { m: mi, c };
            let layer = u8::from(pl.overlay && !m.primary[c]);
            let reg = |hit, area, prio| Region { hit, area, layer, prio };
            if v.choose == Some(mi) {
                out.push(reg(Hit::Pin(cref), Area::Box(g.pin_rect(d.controls[c].label)), 0));
            }
            match *g {
                CtlGeo::Knob { c: cen, r } => {
                    let routes: Vec<usize> = p.routes_to(cref).map(|(i, _)| i).collect();
                    let text = crate::model::fmt_value(&d.controls[c].spec, m.values[c]);
                    out.push(reg(Hit::Pill(cref), Area::Box(g.pill_rect(&text)), 3));
                    for (k, &ri) in routes.iter().enumerate().take(3) {
                        let _ = hidden;
                        out.push(reg(Hit::Plug(ri), Area::Circle(plug_pos(cen, r, k, routes.len()), 7.0), 1));
                    }
                    let routed = !routes.is_empty();
                    match v.depth {
                        DepthGesture::RingBand if routed => {
                            out.push(reg(Hit::Knob(cref), Area::Circle(cen, r + BODY_R_ROUTED), 4));
                            out.push(reg(Hit::Ring(cref), Area::Annulus(cen, r + RING_R0, r + RING_R1), 7));
                        }
                        DepthGesture::PeakHandle if routed => {
                            if let Some(ri) = depth_route(p, cref, v.selected_route) {
                                let base = m.values[c];
                                let tip = polar(cen, r + 11.0, (base + p.routes[ri].amount).clamp(0.0, 1.0));
                                out.push(reg(Hit::Handle(ri), Area::Circle(tip, HANDLE_R), 2));
                            }
                            out.push(reg(Hit::Knob(cref), Area::Circle(cen, r + BODY_R_FREE), 4));
                        }
                        _ => out.push(reg(Hit::Knob(cref), Area::Circle(cen, r + BODY_R_FREE), 4)),
                    }
                }
                CtlGeo::Select { rect } => {
                    let n = opts(d.controls[c].spec);
                    for s in 0..n {
                        let w = rect.width() / n as f32;
                        let r = Rect::from_min_size(pos2(rect.left() + w * s as f32, rect.top()), vec2(w, rect.height()));
                        out.push(reg(Hit::Seg(cref, s), Area::Box(r), 4));
                    }
                }
            }
        }
        for (j, r) in pl.jacks.iter().enumerate() {
            let area = if pl.compact {
                Area::Box(*r)
            } else {
                // 32×32 target around a 26 px socket.
                Area::Box(r.expand(3.0))
            };
            out.push(Region { hit: Hit::Jack(JackRef { m: mi, j }), area, layer: 0, prio: 5 });
        }
        if let Some(t) = pl.toggle {
            out.push(Region { hit: Hit::Toggle(mi), area: Area::Box(t), layer: 0, prio: 0 });
        }
        if let Some(t) = pl.done {
            out.push(Region { hit: Hit::Done(mi), area: Area::Box(t), layer: 0, prio: 0 });
        }
        let face = Rect::from_min_size(pl.rect.min, vec2(pl.face_w, pl.rect.height()));
        out.push(Region {
            hit: Hit::Header(mi),
            area: Area::Box(Rect::from_min_size(face.min, vec2(face.width(), 44.0))),
            layer: 0,
            prio: 8,
        });
        out.push(Region { hit: Hit::Panel(mi), area: Area::Box(face), layer: 0, prio: 9 });
        if let Some(a) = pl.adv {
            let layer = u8::from(pl.overlay);
            out.push(Region { hit: Hit::Panel(mi), area: Area::Box(a), layer, prio: 9 });
        }
    }
    out
}

/// Top hit at a world point: highest layer, then lowest priority, then nearest centre (so two
/// neighbouring ring bands split their overlap down the middle).
pub fn hit_test(regs: &[Region], p: Pos2) -> Option<Hit> {
    regs.iter()
        .filter(|r| r.area.contains(p))
        .min_by(|a, b| {
            (std::cmp::Reverse(a.layer), a.prio)
                .cmp(&(std::cmp::Reverse(b.layer), b.prio))
                .then(dist(a, p).total_cmp(&dist(b, p)))
        })
        .map(|r| r.hit)
}

fn dist(r: &Region, p: Pos2) -> f32 {
    match r.area {
        Area::Circle(c, _) | Area::Annulus(c, _, _) => c.distance(p),
        Area::Box(b) => b.center().distance(p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{reference_patch, Route};

    fn view(p: &Patch) -> ViewGeo {
        ViewGeo {
            cables: Cables::All,
            hidden_layout: HiddenLayout::StableRack,
            expansion: Expansion::Push,
            depth: DepthGesture::RingBand,
            expanded: vec![false; p.modules.len()],
            choose: None,
            wrap_w: 1400.0,
            selected_route: None,
        }
    }

    /// Every region's own centre must hit that region (nothing drawn is unreachable), for all
    /// three depth gestures, with the walkthrough's LFO → Attack route present.
    #[test]
    fn every_region_reachable_at_its_centre() {
        let mut p = reference_patch();
        let lfo = p.find(Kind::Lfo, 0);
        let env = p.find(Kind::Adsr, 0);
        p.routes.push(Route { src: p.jack(lfo, "out"), dst: p.ctl(env, "attack_ms"), amount: 0.25, bypass: false });
        p.routes.push(Route { src: p.jack(lfo, "out"), dst: p.ctl(env, "decay_ms"), amount: 0.25, bypass: false });
        for depth in [DepthGesture::RingBand, DepthGesture::PeakHandle, DepthGesture::InspectorOnly] {
            let mut v = view(&p);
            v.depth = depth;
            let l = layout(&p, &v);
            let regs = regions(&p, &v, &l);
            for r in &regs {
                if matches!(r.hit, Hit::Panel(_) | Hit::Header(_)) {
                    continue;
                }
                assert_eq!(hit_test(&regs, r.area.center()), Some(r.hit), "{depth:?} {:?}", r.hit);
            }
        }
    }

    /// Controls never overlap each other on a face or in the advanced area.
    #[test]
    fn no_control_overlap_expanded_lfo() {
        let p = reference_patch();
        let mut v = view(&p);
        let lfo = p.find(Kind::Lfo, 0);
        v.expanded[lfo] = true;
        let l = layout(&p, &v);
        let rects: Vec<Rect> = l.mods[lfo]
            .ctls
            .iter()
            .flatten()
            .map(|g| match *g {
                CtlGeo::Knob { c, r } => Rect::from_center_size(c, vec2(2.0 * r, 2.0 * r)),
                CtlGeo::Select { rect } => rect,
            })
            .collect();
        for (i, a) in rects.iter().enumerate() {
            assert!(l.mods[lfo].rect.contains_rect(*a), "control {i} outside panel");
            for b in &rects[i + 1..] {
                assert!(!a.intersects(*b), "{a:?} overlaps {b:?}");
            }
        }
    }
}
