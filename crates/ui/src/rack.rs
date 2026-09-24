//! Rack geometry: where every module, control and jack sits, in world units (panel pixels at
//! 100 % zoom; 1u = 30 px, panel height 340). Drawing and hit-testing both read this, so what is
//! drawn is what is hit. Pure: no painting and no egui input, so it is tested headlessly.
//!
//! Stored module positions (`ModuleState.pos`) say which row a module is in and where it wants
//! to be along it. The layout packs each row left to right from those wishes, never overlapping:
//! a module starts at its stored x or at the previous module's right edge, whichever is further
//! right. Expansion (push) only widens a module here, so collapsing it gives back exactly the
//! stored arrangement; nothing about expansion is ever written to the patch.

use std::collections::BTreeSet;

use egui::{pos2, vec2, Pos2, Rect};
use kabl_core::{ModuleId, ModuleState, PatchState};
use kabl_modules::builtins::seq;
use kabl_modules::skin::{ControlKind, ModuleSkin};
use kabl_modules::{registry, ModuleInfo, ParamInfo, PortDirection, PortInfo, Taper};

pub const UNIT: f32 = 30.0;
pub const PANEL_H: f32 = 340.0;
pub const ROW_Y0: f32 = 10.0;
pub const ROW_PITCH: f32 = 370.0;
pub const RACK_X: f32 = 24.0;
pub const R_LARGE: f32 = 22.0;
pub const R_SMALL: f32 = 17.0;
pub const JACK_R: f32 = 13.0;

/// Stored-param prefix for the per-instance face choice: `face.<param>` = 1 on the face, 0 off
/// it, absent = the module's default. Presentation metadata in the patch; the compiler only reads
/// declared param names, so it never sees these.
pub const FACE_PREFIX: &str = "face.";

/// Stored params that are presentation or control metadata, never audio: faces, performance
/// pins (`pin.*`), MIDI CC mappings (`cc.*`) and a sequencer's launch settings (`launch.*`).
/// Editing them never rebuilds the graph.
pub fn is_presentation(param: &str) -> bool {
    param.starts_with(FACE_PREFIX)
        || param.starts_with(crate::perform::PIN_PREFIX)
        || param.starts_with(crate::perform::CC_PREFIX)
        || param.starts_with("launch.")
        || param.starts_with("cue")
        || param.starts_with(crate::perform::BTN_PREFIX)
}

pub fn face_key(param: &str) -> String {
    format!("{FACE_PREFIX}{param}")
}

/// The name a param's face choice is stored under: a sequencer bank param shares its bank-A
/// slot's choice (`c.v2` → `v2`), so every bank shows the same face.
pub fn face_name<'a>(info: &ModuleInfo, name: &'a str) -> &'a str {
    if info.kind == "seq" {
        seq::slot_name(name)
    } else {
        name
    }
}

/// Which of `info.params` are on the face: the user's choice where stored, else the default.
pub fn primary_set(m: &ModuleState, info: &ModuleInfo) -> Vec<bool> {
    info.params
        .iter()
        .map(|p| {
            let name = face_name(info, p.name);
            match m.params.get(&face_key(name)) {
                Some(&v) => v >= 0.5,
                None => !info.advanced.contains(&name),
            }
        })
        .collect()
}

/// Whether param `name` of a module is drawn at all: a sequencer shows its module-wide params
/// and the edit bank's; every other module everything.
pub fn visible(info: &ModuleInfo, name: &str, edit_bank: usize) -> bool {
    info.kind != "seq" || seq::bank_of(name).is_none_or(|(b, _)| b == edit_bank)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Geo {
    Knob { c: Pos2, r: f32 },
    Select { rect: Rect },
}

impl Geo {
    pub fn label_pos(&self) -> Pos2 {
        match *self {
            Geo::Knob { c, .. } => c - vec2(0.0, 47.0),
            Geo::Select { rect } => pos2(rect.center().x, rect.top() - 13.0),
        }
    }
    pub fn value_pos(&self) -> Pos2 {
        match *self {
            Geo::Knob { c, .. } => c + vec2(0.0, 38.0),
            Geo::Select { rect } => rect.center(),
        }
    }
    /// Everything this control draws: label, knob or selector, value.
    pub fn bounds(&self) -> Rect {
        match *self {
            Geo::Knob { c, r } => {
                Rect::from_min_max(c - vec2(r + 11.0, 56.0), c + vec2(r + 11.0, 49.0))
            }
            Geo::Select { rect } => {
                rect.union(Rect::from_center_size(self.label_pos(), vec2(40.0, 16.0)))
            }
        }
    }
    /// Pin toggle (choose mode), right of the label.
    pub fn pin_rect(&self, label: &str) -> Rect {
        let dx = label.chars().count() as f32 * 3.6 + 16.0;
        Rect::from_center_size(self.label_pos() + vec2(dx, 0.0), vec2(18.0, 18.0))
    }
}

#[derive(Debug, Clone)]
pub struct Ctl {
    pub param: &'static ParamInfo,
    pub primary: bool,
    pub geo: Geo,
}

#[derive(Debug, Clone)]
pub struct Jack {
    pub port: &'static PortInfo,
    pub c: Pos2,
    pub label_right: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Decor {
    None,
    Keys(Rect),
    Speaker(Pos2),
    Envelope(Rect),
    /// The clock's Run/Stop and Restart buttons.
    Transport(Rect),
    /// Left end of the delay's sync readout line (left of the output plate).
    Status(Pos2),
    /// The sequencer's header strip: EDIT tabs on the left, PLAY buttons on the right.
    Banks(Rect),
    /// The cue buttons (two rows of four) and Cancel.
    Cues(Rect),
}

#[derive(Debug, Clone)]
pub struct Placed {
    pub id: ModuleId,
    pub info: &'static ModuleInfo,
    /// Face plus, when pushing, the advanced area: the room this module takes in its row.
    pub rect: Rect,
    pub face: Rect,
    pub adv: Option<Rect>,
    /// The advanced area floats over the neighbours instead of pushing them.
    pub overlay: bool,
    pub ctls: Vec<Ctl>,
    /// Params with no visible control right now (collapsed off the face).
    pub hidden: Vec<&'static ParamInfo>,
    pub jacks: Vec<Jack>,
    /// `+N` / `Less`; absent when the module has nothing off its face.
    pub toggle: Option<Rect>,
    /// `Done` while choosing primary controls.
    pub done: Option<Rect>,
    /// Output plate behind the output jacks.
    pub plate: Option<Rect>,
    pub decor: Decor,
    pub skin: Option<&'static ModuleSkin>,
    pub row: usize,
}

impl Placed {
    pub fn ctl(&self, param: &str) -> Option<&Ctl> {
        self.ctls.iter().find(|c| c.param.name == param)
    }
    pub fn jack(&self, port: &str, dir: PortDirection) -> Option<&Jack> {
        self.jacks
            .iter()
            .find(|j| j.port.name == port && j.port.direction == dir)
    }
    /// Face and advanced area together, whether pushing or floating.
    pub fn full(&self) -> Rect {
        self.adv.map_or(self.rect, |a| self.rect.union(a))
    }
    fn translate(&mut self, d: egui::Vec2) {
        self.rect = self.rect.translate(d);
        self.face = self.face.translate(d);
        self.adv = self.adv.map(|r| r.translate(d));
        for c in &mut self.ctls {
            c.geo = match c.geo {
                Geo::Knob { c, r } => Geo::Knob { c: c + d, r },
                Geo::Select { rect } => Geo::Select {
                    rect: rect.translate(d),
                },
            };
        }
        for j in &mut self.jacks {
            j.c += d;
        }
        self.toggle = self.toggle.map(|r| r.translate(d));
        self.done = self.done.map(|r| r.translate(d));
        self.plate = self.plate.map(|r| r.translate(d));
        self.decor = match self.decor {
            Decor::None => Decor::None,
            Decor::Keys(r) => Decor::Keys(r.translate(d)),
            Decor::Speaker(c) => Decor::Speaker(c + d),
            Decor::Envelope(r) => Decor::Envelope(r.translate(d)),
            Decor::Transport(r) => Decor::Transport(r.translate(d)),
            Decor::Status(c) => Decor::Status(c + d),
            Decor::Banks(r) => Decor::Banks(r.translate(d)),
            Decor::Cues(r) => Decor::Cues(r.translate(d)),
        };
    }
}

/// View state that decides geometry. None of it is patch data or undoable.
#[derive(Default)]
pub struct View<'a> {
    pub expanded: Option<&'a BTreeSet<ModuleId>>,
    pub float: bool,
    pub skins: bool,
    /// Module in choose mode and its pending face choice (aligned with `info.params`).
    pub choose: Option<(ModuleId, &'a [bool])>,
    /// Module being dragged and its live world position.
    pub moving: Option<(ModuleId, kabl_core::Vec2)>,
    /// Each sequencer's edit bank; absent = A.
    pub edit_banks: Option<&'a std::collections::HashMap<ModuleId, usize>>,
}

pub struct Layout {
    pub mods: Vec<Placed>,
    pub bounds: Rect,
    pub rows: usize,
}

impl Layout {
    pub fn get(&self, id: ModuleId) -> Option<&Placed> {
        self.mods.iter().find(|m| m.id == id)
    }
}

pub fn row_of(y: f32) -> usize {
    ((y - ROW_Y0) / ROW_PITCH).round().max(0.0) as usize
}

pub fn row_y(row: usize) -> f32 {
    ROW_Y0 + row as f32 * ROW_PITCH
}

/// Stored position for a module dropped with its top-left at world `p`: nearest row, whole
/// units along it.
pub fn snap(p: Pos2) -> kabl_core::Vec2 {
    kabl_core::Vec2 {
        x: ((p.x / UNIT).round() * UNIT).max(RACK_X),
        y: row_y(row_of(p.y)),
    }
}

pub fn layout(state: &PatchState, v: &View) -> Layout {
    let mut placed: Vec<(usize, f32, Placed)> = Vec::new();
    for (&id, m) in &state.modules {
        let Some(info) = registry::info_for(&m.kind) else {
            continue;
        };
        let primary = match v.choose {
            Some((cid, set)) if cid == id => set.to_vec(),
            _ => primary_set(m, info),
        };
        let choosing = v.choose.is_some_and(|(cid, _)| cid == id);
        let expanded = choosing || v.expanded.is_some_and(|e| e.contains(&id));
        let skin = info.skin.filter(|_| v.skins);
        let bank = v.edit_banks.and_then(|b| b.get(&id)).copied().unwrap_or(0);
        let shown: Vec<bool> = info
            .params
            .iter()
            .map(|p| visible(info, p.name, bank))
            .collect();
        let mut pl = place_local(id, info, &primary, &shown, expanded, choosing, skin);
        if v.float && pl.adv.is_some() && !choosing {
            pl.overlay = true;
            pl.rect = pl.face;
        }
        let pos = match v.moving {
            Some((mid, p)) if mid == id => p,
            _ => m.pos,
        };
        pl.row = row_of(pos.y);
        placed.push((pl.row, (pos.x / UNIT).round() * UNIT, pl));
    }
    placed.sort_by(|a, b| {
        (a.0, a.1, a.2.id)
            .partial_cmp(&(b.0, b.1, b.2.id))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let rows = placed.last().map_or(0, |p| p.0 + 1);
    let mut next_x = vec![RACK_X; rows];
    let mut bounds = Rect::NOTHING;
    let mut mods = Vec::with_capacity(placed.len());
    for (row, x, mut pl) in placed {
        let x = x.max(next_x[row]);
        pl.translate(vec2(x, row_y(row)));
        next_x[row] = pl.rect.right();
        bounds = bounds.union(pl.full());
        mods.push(pl);
    }
    Layout { mods, bounds, rows }
}

/// Wide enough for the longest option in 11.5 px mono on every segment.
pub fn sel_width(n: usize, longest: usize) -> f32 {
    (7.0 * longest as f32 + 14.0).max(30.0) * n as f32
}

fn options(info: &ModuleInfo, p: &ParamInfo) -> (usize, usize) {
    let n = (p.max - p.min).round() as usize + 1;
    let longest = crate::routing::step_labels(info.kind, p.name).map_or(1, |l| {
        l.iter().map(|s| s.chars().count()).max().unwrap_or(1)
    });
    (n, longest)
}

fn spread(n: usize, fw: f32) -> Vec<f32> {
    match n {
        0 => vec![],
        1 => vec![fw / 2.0],
        2 => vec![fw * 0.25, fw * 0.75],
        _ => {
            let x0 = 40.0f32.min(fw * 0.19);
            (0..n)
                .map(|i| x0 + i as f32 * (fw - 2.0 * x0) / (n as f32 - 1.0))
                .collect()
        }
    }
}

/// One module's geometry at the origin.
fn place_local(
    id: ModuleId,
    info: &'static ModuleInfo,
    primary: &[bool],
    shown: &[bool],
    expanded: bool,
    choosing: bool,
    skin: Option<&'static ModuleSkin>,
) -> Placed {
    let knob = |p: &ParamInfo| p.taper != Taper::Stepped;
    let face_knobs: Vec<usize> = (0..info.params.len())
        .filter(|&i| shown[i] && primary[i] && knob(&info.params[i]))
        .collect();
    let face_sels: Vec<usize> = (0..info.params.len())
        .filter(|&i| shown[i] && primary[i] && !knob(&info.params[i]))
        .collect();
    let base_w = info.width_units as f32 * UNIT;
    let fw = match skin {
        Some(s) => ((s.panel_size.0 / UNIT).round() * UNIT).max(base_w),
        None if face_knobs.len() >= 3 => {
            base_w.max(((72.0 + 56.0 * (face_knobs.len() as f32 - 1.0)) / UNIT).ceil() * UNIT)
        }
        None => base_w,
    };
    let mut ctls: Vec<Ctl> = Vec::new();
    let mut jacks: Vec<Jack> = Vec::new();
    let mut decor = Decor::None;
    let mut off_face: Vec<usize> = (0..info.params.len())
        .filter(|&i| shown[i] && !primary[i])
        .collect();
    let ins: Vec<&'static PortInfo> = info
        .ports
        .iter()
        .filter(|p| p.direction == PortDirection::Input)
        .collect();
    let outs: Vec<&'static PortInfo> = info
        .ports
        .iter()
        .filter(|p| p.direction == PortDirection::Output)
        .collect();

    if let Some(skin) = skin {
        let at = |id: &str, kind: ControlKind| {
            skin.controls
                .iter()
                .find(|c| c.id == id && c.kind == kind)
                .map(|c| pos2(c.pos.0 * fw, c.pos.1 * PANEL_H))
        };
        let r = if face_knobs.len() == 1 {
            R_LARGE
        } else {
            R_SMALL
        };
        for &i in face_knobs.iter().chain(&face_sels) {
            let p = &info.params[i];
            match at(p.name, ControlKind::Knob) {
                Some(c) if knob(p) => ctls.push(Ctl {
                    param: p,
                    primary: true,
                    geo: Geo::Knob { c, r },
                }),
                Some(c) => {
                    let (n, longest) = options(info, p);
                    let rect = Rect::from_center_size(c, vec2(sel_width(n, longest), 28.0));
                    ctls.push(Ctl {
                        param: p,
                        primary: true,
                        geo: Geo::Select { rect },
                    });
                }
                // A primary control the art has no place for goes to the advanced area.
                None => off_face.push(i),
            }
        }
        off_face.sort_unstable();
        for port in info.ports {
            if let Some(c) = at(port.name, ControlKind::Jack) {
                jacks.push(Jack {
                    port,
                    c,
                    label_right: false,
                });
            }
        }
    } else {
        // Jacks first: they anchor the bottom of the face and never move.
        let column = info.kind == "out";
        let ports = ins.len() + outs.len();
        // A wide panel (the sequencer) keeps one row, clear of its selector row.
        let two_rows =
            !column && ports > 3 && !(ports == 4 && fw >= 240.0) && fw < 100.0 * ports as f32;
        if column {
            for (i, &port) in ins.iter().enumerate() {
                jacks.push(Jack {
                    port,
                    c: pos2(40.0, 230.0 + 48.0 * i as f32),
                    label_right: true,
                });
            }
        } else if two_rows {
            for (x, &port) in spread(ins.len(), fw).into_iter().zip(&ins) {
                jacks.push(Jack {
                    port,
                    c: pos2(x, 212.0),
                    label_right: false,
                });
            }
            for (x, &port) in spread(outs.len(), fw).into_iter().zip(&outs) {
                jacks.push(Jack {
                    port,
                    c: pos2(x, 290.0),
                    label_right: false,
                });
            }
        } else {
            let all: Vec<&'static PortInfo> = ins.iter().chain(&outs).copied().collect();
            for (x, port) in spread(all.len(), fw).into_iter().zip(all) {
                jacks.push(Jack {
                    port,
                    c: pos2(x, 272.0),
                    label_right: false,
                });
            }
        }
        let room = jacks
            .iter()
            .map(|j| j.c.y - if j.label_right { 18.0 } else { 36.0 })
            .fold(PANEL_H, f32::min);

        // Knob row, then selector row, from the top; the module's picture (envelope display)
        // only when both still fit above the jacks.
        let block = |start: f32| {
            let mut y = start;
            if !face_knobs.is_empty() {
                y += 110.0;
            }
            if !face_sels.is_empty() {
                y += 62.0;
            }
            y
        };
        let mut y = 52.0;
        let mut sels_top = None;
        match info.kind {
            "out" => decor = Decor::Speaker(pos2(fw / 2.0, 112.0)),
            "delay" => decor = Decor::Status(pos2(14.0, 228.0)),
            "seq" => {
                decor = Decor::Banks(Rect::from_min_size(pos2(14.0, 12.0), vec2(fw - 74.0, 26.0)))
            }
            "cues" => {
                decor = Decor::Cues(Rect::from_min_size(
                    pos2(16.0, 64.0),
                    vec2(fw - 32.0, 250.0),
                ))
            }
            "clock" => {
                decor = Decor::Transport(Rect::from_min_size(
                    pos2(12.0, 174.0),
                    vec2(fw - 24.0, 32.0),
                ))
            }
            "env.adsr" if block(108.0) <= room => {
                decor =
                    Decor::Envelope(Rect::from_min_size(pos2(16.0, 54.0), vec2(fw - 32.0, 54.0)));
                y = 108.0;
            }
            // No room for the picture and a selector row under the knobs: the selectors take
            // the picture's place, so the knobs stay exactly where they are with the picture.
            "env.adsr" if !face_knobs.is_empty() && block(52.0) <= room => {
                sels_top = Some(52.0);
                y = 108.0;
            }
            _ => {}
        }
        if info.kind == "midi.in" {
            decor = midi_in_face(info, &face_knobs, &face_sels, fw, &mut ctls, &mut off_face);
        } else {
            let n = face_knobs.len();
            for (k, &i) in face_knobs.iter().enumerate() {
                let r = if n == 1 || (n == 2 && k == 0 && i == 0) {
                    R_LARGE
                } else {
                    R_SMALL
                };
                let x = match n {
                    1 => fw / 2.0,
                    2 => fw * (k as f32 + 0.5) / 2.0,
                    _ => 36.0 + k as f32 * (fw - 72.0) / (n as f32 - 1.0),
                };
                ctls.push(Ctl {
                    param: &info.params[i],
                    primary: true,
                    geo: Geo::Knob {
                        c: pos2(x, y + 60.0),
                        r,
                    },
                });
            }
            if n > 0 {
                y += 110.0;
            }
            let total: usize = face_sels
                .iter()
                .map(|&i| options(info, &info.params[i]).0)
                .sum();
            let mut x = 14.0;
            for &i in &face_sels {
                let avail = fw - 28.0 - 10.0 * (face_sels.len() as f32 - 1.0);
                let w = avail * options(info, &info.params[i]).0 as f32 / total as f32;
                let rect =
                    Rect::from_min_size(pos2(x, sels_top.unwrap_or(y) + 26.0), vec2(w, 28.0));
                ctls.push(Ctl {
                    param: &info.params[i],
                    primary: true,
                    geo: Geo::Select { rect },
                });
                x += w + 10.0;
            }
        }
    }

    // Advanced area: everything off the face, appended right of it.
    let mut adv = None;
    let mut hidden: Vec<&'static ParamInfo> = Vec::new();
    if expanded && !off_face.is_empty() {
        let mut hk: Vec<usize> = off_face
            .iter()
            .copied()
            .filter(|&i| knob(&info.params[i]))
            .collect();
        // The sequencer: a velocity row (then gate length) and a probability row (then
        // transpose), one knob per step.
        let per_row = if info.kind == "seq" { 9 } else { 5 };
        if info.kind == "seq" {
            hk.sort_by_key(|&i| {
                let slot = seq::slot_name(info.params[i].name);
                let row = match slot.as_bytes()[0] {
                    b'v' => 0,
                    b'g' => 1,
                    b'r' => 2,
                    _ => 3,
                };
                (row, i)
            });
        }
        let hs: Vec<usize> = off_face
            .iter()
            .copied()
            .filter(|&i| !knob(&info.params[i]))
            .collect();
        let cols = hk.len().min(per_row);
        let sel_w = |i: usize| {
            let (n, longest) = options(info, &info.params[i]);
            sel_width(n, longest)
        };
        let sel_row: f32 = hs.iter().map(|&i| sel_w(i)).sum::<f32>()
            + 12.0 * hs.len().saturating_sub(1) as f32
            + 28.0;
        let knob_w = if cols > 0 {
            60.0 + 76.0 * (cols as f32 - 1.0)
        } else {
            0.0
        };
        let aw = (knob_w.max(sel_row.min(14.0 * UNIT)).max(5.0 * UNIT) / UNIT).ceil() * UNIT;
        let mut y = 112.0;
        for (k, &i) in hk.iter().enumerate() {
            if k > 0 && k % per_row == 0 {
                y += 110.0;
            }
            let x0 = (aw - 76.0 * (cols as f32 - 1.0)) / 2.0;
            let c = pos2(fw + x0 + 76.0 * (k % per_row) as f32, y);
            ctls.push(Ctl {
                param: &info.params[i],
                primary: false,
                geo: Geo::Knob { c, r: R_SMALL },
            });
        }
        let mut sy = if hk.is_empty() { 92.0 } else { y + 76.0 };
        let mut sx = 14.0;
        for &i in &hs {
            let w = sel_w(i);
            if sx > 14.0 && sx + w > aw - 14.0 {
                sx = 14.0;
                sy += 62.0;
            }
            let rect = Rect::from_min_size(pos2(fw + sx, sy), vec2(w, 28.0));
            ctls.push(Ctl {
                param: &info.params[i],
                primary: false,
                geo: Geo::Select { rect },
            });
            sx += w + 12.0;
        }
        adv = Some(Rect::from_min_size(pos2(fw, 0.0), vec2(aw, PANEL_H)));
    } else {
        hidden = off_face.iter().map(|&i| &info.params[i]).collect();
    }

    let has_off_face = !off_face.is_empty();
    let toggle = (has_off_face && !choosing)
        .then(|| Rect::from_min_size(pos2(fw - 46.0, 12.0), vec2(36.0, 26.0)));
    let done = choosing.then(|| Rect::from_min_size(pos2(fw - 62.0, 12.0), vec2(52.0, 26.0)));
    let plate = skin
        .is_none()
        .then(|| {
            jacks
                .iter()
                .filter(|j| j.port.direction == PortDirection::Output)
                .map(|j| Rect::from_min_max(j.c - vec2(30.0, 53.0), j.c + vec2(30.0, 23.0)))
                .reduce(|a, b| a.union(b))
        })
        .flatten();
    let face = Rect::from_min_size(Pos2::ZERO, vec2(fw, PANEL_H));
    let push_w = adv.map_or(0.0, |a| a.width());
    Placed {
        id,
        info,
        rect: Rect::from_min_size(Pos2::ZERO, vec2(fw + push_w, PANEL_H)),
        face,
        adv,
        overlay: false,
        ctls,
        hidden,
        jacks,
        toggle,
        done,
        plate,
        decor,
        skin,
        row: 0,
    }
}

/// The MIDI In face, top to bottom: the keys picture with a line naming this module's voice
/// settings, then one full-width row per selector (three options with long names need the
/// width), then a knob row. They must end above the output plate; the picture goes first when
/// room is short, then chosen controls from the last, which move to the advanced area.
fn midi_in_face(
    info: &'static ModuleInfo,
    knobs: &[usize],
    sels: &[usize],
    fw: f32,
    ctls: &mut Vec<Ctl>,
    off_face: &mut Vec<usize>,
) -> Decor {
    const TOP: f32 = 54.0;
    const BOTTOM: f32 = 212.0;
    const SEL_ROW: f32 = 52.0;
    const KNOB_ROW: f32 = 110.0;
    const PICTURE: f32 = 46.0;
    let mut sels = sels.to_vec();
    let mut knobs = knobs.to_vec();
    let height = |s: &[usize], k: &[usize]| {
        s.len() as f32 * SEL_ROW + if k.is_empty() { 0.0 } else { KNOB_ROW }
    };
    while height(&sels, &knobs) > BOTTOM - TOP {
        let last_knob = knobs.last().copied();
        let last_sel = sels.last().copied();
        let drop = match (last_knob, last_sel) {
            (Some(k), Some(s)) if k > s => knobs.pop(),
            (Some(_), None) => knobs.pop(),
            _ => sels.pop(),
        };
        off_face.extend(drop);
    }
    off_face.sort_unstable();
    let picture = height(&sels, &knobs) + PICTURE <= BOTTOM - TOP;
    let decor = if picture {
        Decor::Keys(Rect::from_min_size(pos2(12.0, TOP), vec2(fw - 24.0, 26.0)))
    } else {
        Decor::None
    };
    let mut y = if picture { TOP + PICTURE } else { TOP };
    for &i in &sels {
        let rect = Rect::from_min_size(pos2(14.0, y + 26.0), vec2(fw - 28.0, 28.0));
        ctls.push(Ctl {
            param: &info.params[i],
            primary: true,
            geo: Geo::Select { rect },
        });
        y += SEL_ROW;
    }
    let n = knobs.len();
    for (k, &i) in knobs.iter().enumerate() {
        let x = fw * (k as f32 + 0.5) / n as f32;
        let r = if n == 1 { R_LARGE } else { R_SMALL };
        ctls.push(Ctl {
            param: &info.params[i],
            primary: true,
            geo: Geo::Knob {
                c: pos2(x, y + 60.0),
                r,
            },
        });
    }
    decor
}

#[cfg(test)]
mod tests {
    use super::*;
    use kabl_core::Vec2;

    fn module(kind: &str, x: f32, y: f32) -> ModuleState {
        ModuleState {
            kind: kind.into(),
            pos: Vec2 { x, y },
            params: Default::default(),
        }
    }

    fn rack() -> PatchState {
        let mut p = PatchState::new();
        let mut x = RACK_X;
        for (i, kind) in registry::KNOWN_KINDS.iter().enumerate() {
            p.modules
                .insert(i as u64 + 1, module(kind, x, row_y(i / 5)));
            x = if i == 4 { RACK_X } else { x + 150.0 };
        }
        p
    }

    fn rects(l: &Layout) -> Vec<(ModuleId, Rect)> {
        l.mods.iter().map(|m| (m.id, m.rect)).collect()
    }

    #[test]
    fn rows_pack_without_overlap_and_keep_stored_order() {
        let l = layout(&rack(), &View::default());
        for w in l.mods.windows(2) {
            if w[0].row == w[1].row {
                assert!(
                    w[0].rect.right() <= w[1].rect.left() + 1e-3,
                    "{} overlaps {}",
                    w[0].id,
                    w[1].id
                );
                assert!(w[0].id < w[1].id);
            }
        }
    }

    #[test]
    fn push_expansion_is_reversible_and_never_moves_the_face() {
        let p = rack();
        let env = p
            .modules
            .iter()
            .find(|(_, m)| m.kind == "vca")
            .map(|(&id, _)| id)
            .unwrap();
        let before = layout(&p, &View::default());
        let mut set = BTreeSet::new();
        set.insert(env);
        let open = layout(
            &p,
            &View {
                expanded: Some(&set),
                ..Default::default()
            },
        );
        let (a, b) = (before.get(env).unwrap(), open.get(env).unwrap());
        assert_eq!(a.face, b.face);
        assert_eq!(
            a.jacks.iter().map(|j| j.c).collect::<Vec<_>>(),
            b.jacks.iter().map(|j| j.c).collect::<Vec<_>>()
        );
        for c in &a.ctls {
            assert_eq!(
                Some(c.geo),
                b.ctl(c.param.name).map(|c| c.geo),
                "{}",
                c.param.name
            );
        }
        let adv = b.adv.expect("response is advanced");
        assert!(b.ctl("exponential").is_some() && a.ctl("exponential").is_none());
        // Right-hand neighbours in the row move by exactly the advanced width; others stay.
        for m in &open.mods {
            let old = before.get(m.id).unwrap();
            let dx = m.rect.left() - old.rect.left();
            if m.row == b.row && old.rect.left() > a.rect.left() {
                assert!((dx - adv.width()).abs() < 1e-3, "{} pushed by {dx}", m.id);
            } else if m.id != env {
                assert_eq!(dx, 0.0, "{} moved", m.id);
            }
        }
        // Collapse, many times over: exactly the original layout, no drift.
        for _ in 0..5 {
            let _ = layout(
                &p,
                &View {
                    expanded: Some(&set),
                    ..Default::default()
                },
            );
        }
        assert_eq!(rects(&layout(&p, &View::default())), rects(&before));
    }

    #[test]
    fn float_expansion_moves_nothing() {
        let p = rack();
        let vca = p
            .modules
            .iter()
            .find(|(_, m)| m.kind == "vca")
            .map(|(&id, _)| id)
            .unwrap();
        let mut set = BTreeSet::new();
        set.insert(vca);
        let before = layout(&p, &View::default());
        let float = layout(
            &p,
            &View {
                expanded: Some(&set),
                float: true,
                ..Default::default()
            },
        );
        assert_eq!(rects(&before), rects(&float));
        let m = float.get(vca).unwrap();
        assert!(m.overlay && m.adv.is_some());
    }

    #[test]
    fn controls_stay_inside_their_panel_and_apart() {
        let p = rack();
        for all_primary in [false, true] {
            for expanded in [false, true] {
                let mut q = p.clone();
                for m in q.modules.values_mut() {
                    let info = registry::info_for(&m.kind).unwrap();
                    for par in info.params {
                        if all_primary {
                            m.params.insert(face_key(par.name), 1.0);
                        }
                    }
                }
                let set: BTreeSet<ModuleId> = q.modules.keys().copied().collect();
                let v = View {
                    expanded: expanded.then_some(&set),
                    ..Default::default()
                };
                let l = layout(&q, &v);
                for m in &l.mods {
                    let area = m.full();
                    let mut boxes: Vec<Rect> = m.ctls.iter().map(|c| c.geo.bounds()).collect();
                    boxes.extend(
                        m.jacks.iter().map(|j| {
                            Rect::from_center_size(j.c - vec2(0.0, 10.0), vec2(26.0, 46.0))
                        }),
                    );
                    for (i, a) in boxes.iter().enumerate() {
                        assert!(
                            area.expand(1.0).contains_rect(*a),
                            "{} ctl {i} outside {a:?} {area:?}",
                            m.info.kind
                        );
                        for b in &boxes[i + 1..] {
                            assert!(
                                !a.shrink(1.0).intersects(*b),
                                "{}: {a:?} overlaps {b:?}",
                                m.info.kind
                            );
                        }
                    }
                    if !expanded {
                        let shown = m.ctls.len() + m.hidden.len();
                        let visible = m
                            .info
                            .params
                            .iter()
                            .filter(|p| visible(m.info, p.name, 0))
                            .count();
                        assert_eq!(shown, visible, "{}", m.info.kind);
                    }
                }
            }
        }
    }

    #[test]
    fn midi_in_face_shows_keys_mode_and_glide_with_room_for_their_names() {
        let mut p = PatchState::new();
        p.modules.insert(1, module("midi.in", RACK_X, row_y(0)));
        let l = layout(&p, &View::default());
        let m = l.get(1).unwrap();
        let Decor::Keys(keys) = m.decor else {
            panic!("no keys picture: {:?}", m.decor)
        };
        let names: Vec<&str> = m.ctls.iter().map(|c| c.param.name).collect();
        assert_eq!(names, ["mode", "glide"]);
        let plate = m.plate.expect("output plate");
        for c in &m.ctls {
            let Geo::Select { rect } = c.geo else {
                panic!("{} is not a selector", c.param.name)
            };
            // Three segments, each wide enough for LEGATO / ALWAYS in 11.5 px mono.
            assert!(rect.width() / 3.0 >= 6.0 * 7.0, "{rect:?}");
            assert!(c.geo.bounds().top() > keys.bottom() + 10.0);
            assert!(c.geo.bounds().bottom() < plate.top());
        }
        // Everything on the face: the three selectors fit, the glide time knob moves to the
        // advanced area and the picture gives way.
        let q = {
            let mut q = p.clone();
            for par in MIDI_PARAMS {
                q.modules
                    .get_mut(&1)
                    .unwrap()
                    .params
                    .insert(face_key(par), 1.0);
            }
            q
        };
        let l = layout(&q, &View::default());
        let m = l.get(1).unwrap();
        assert_eq!(m.decor, Decor::None);
        let names: Vec<&str> = m.ctls.iter().map(|c| c.param.name).collect();
        assert_eq!(names, ["mode", "priority", "glide"]);
        assert_eq!(m.hidden.len(), 1);
        assert!(m.toggle.is_some());
    }

    const MIDI_PARAMS: [&str; 4] = ["mode", "priority", "glide", "glide_ms"];

    #[test]
    fn snapping_picks_the_nearest_row_and_unit() {
        assert_eq!(snap(pos2(44.0, 20.0)), Vec2 { x: 30.0, y: ROW_Y0 });
        assert_eq!(
            snap(pos2(301.0, 300.0)),
            Vec2 {
                x: 300.0,
                y: row_y(1)
            }
        );
        assert_eq!(
            snap(pos2(-50.0, -80.0)),
            Vec2 {
                x: RACK_X,
                y: ROW_Y0
            }
        );
        assert_eq!(row_of(snap(pos2(0.0, 900.0)).y), 2);
    }

    /// No built-in has a skin (core modules stay A / A-dark), so the skin path is checked with a
    /// test skin on the oscillator: controls land where the art places them, inside the face.
    #[test]
    fn a_skin_places_face_controls_where_its_art_says() {
        use kabl_modules::skin::ControlSkin;
        static CONTROLS: &[ControlSkin] = &[
            ControlSkin {
                id: "base_hz",
                kind: ControlKind::Knob,
                pos: (0.5, 0.36),
            },
            ControlSkin {
                id: "waveform",
                kind: ControlKind::Knob,
                pos: (0.5, 0.6),
            },
            ControlSkin {
                id: "pitch",
                kind: ControlKind::Jack,
                pos: (0.19, 0.84),
            },
            ControlSkin {
                id: "sync",
                kind: ControlKind::Jack,
                pos: (0.5, 0.84),
            },
            ControlSkin {
                id: "out",
                kind: ControlKind::Jack,
                pos: (0.81, 0.84),
            },
        ];
        static SKIN: ModuleSkin = ModuleSkin {
            panel_size: (210.0, 340.0),
            background_image: None,
            background_dark: None,
            labels_on_art: false,
            art_ink: [[0; 3]; 2],
            controls: CONTROLS,
        };
        let info = registry::info_for("osc.va").unwrap();
        let p = place_local(
            1,
            info,
            &[true, true, false, false, false, false],
            &[true; 6],
            false,
            false,
            Some(&SKIN),
        );
        assert_eq!(p.face.width(), 210.0);
        assert_eq!(
            p.ctl("base_hz").map(|c| c.geo),
            Some(Geo::Knob {
                c: pos2(105.0, 0.36 * PANEL_H),
                r: R_LARGE
            })
        );
        assert!(matches!(
            p.ctl("waveform").map(|c| c.geo),
            Some(Geo::Select { .. })
        ));
        assert_eq!(p.jacks.len(), 3);
        for c in &p.ctls {
            assert!(p.face.contains_rect(c.geo.bounds()), "{}", c.param.name);
        }
        // Off the face: the waveform goes to the advanced area like any module's.
        let p = place_local(
            1,
            info,
            &[true, false, false, false, false, false],
            &[true; 6],
            true,
            false,
            Some(&SKIN),
        );
        assert!(p.adv.is_some_and(|a| p
            .ctl("waveform")
            .is_some_and(|c| a.contains_rect(c.geo.bounds()))));
    }
}
