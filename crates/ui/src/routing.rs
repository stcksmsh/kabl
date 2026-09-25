//! Knob modulation UI over the real patch: route views, the reachable-range math the ring
//! draws, the modulated knob (body edits the base, ring edits the selected route's amount),
//! stepped selectors, and the routing drawer. Everything edits through `PatchEditor`, so undo,
//! save and the engine see exactly what the screen shows.
//!
//! Ranges drawn here are computed from the base, the route amounts and each source's nominal
//! range (`PortType::nominal_range`). They are what the routes *can* reach, not measured
//! telemetry, and the drawer says so.

use egui::{Color32, Id, Pos2, Rect, Sense, Stroke, Vec2 as EguiVec2};
use kabl_core::{CableId, ModuleId, ParamTarget, PatchState, PortRef};
use kabl_engine::compile::DEFAULT_ROUTE_AMOUNT;
use kabl_modules::builtins::{seq, SEQ_INFO};
use kabl_modules::{registry, ParamInfo, PortDirection, Taper};

use crate::theme::theme;
use crate::{cable_color, PatchEditor, UiState};

/// Ring band (route amount handle) spans `r + RING_INNER ..= r + RING_OUTER` around a knob of
/// radius `r`; the range arc is drawn at `r + RING_DRAW`. World units (scaled by zoom).
const RING_INNER: f32 = 4.0;
const RING_OUTER: f32 = 15.0;
const RING_DRAW: f32 = 11.0;
const DRAG_PIXELS_FOR_FULL_SWEEP: f32 = 150.0;
/// Plugs (route cable ends) sit in the 6 o'clock gap the 270° scale leaves free.
const PLUG_DROP: f32 = 8.0;
const PLUG_SPREAD: f32 = 15.0;
const MAX_PLUGS: usize = 3;
/// Spacing between per-source lanes around an inspected knob.
const LANE_GAP: f32 = 7.0;
/// Collapsed per-source rings: at most this many, this far apart, inside the ring band.
const MINI_LANES: usize = 4;
const MINI_GAP: f32 = 2.5;

const BYPASS_GREY: Color32 = Color32::from_gray(120);

/// How a control is drawn: zoom, and the label/value ink (theme ink, or a skin's ink on art).
#[derive(Clone, Copy)]
pub(crate) struct Look {
    pub z: f32,
    pub ink: Color32,
    pub ink2: Color32,
}

/// One modulation route into a param, as the UI needs it.
#[derive(Debug, Clone, PartialEq)]
pub struct RouteView {
    pub cable: CableId,
    pub from_id: ModuleId,
    pub from_port: String,
    /// Signed fraction of knob travel per unit of normalized source.
    pub amount: f32,
    pub bypass: bool,
    /// Source nominal range divided by its full scale: (-1, 1) bipolar, (0, 1) unipolar.
    pub src: (f32, f32),
}

/// Every route into `(id, param)`, in cable-id order.
pub fn routes_into(state: &PatchState, id: ModuleId, param: &str) -> Vec<RouteView> {
    let mut out = Vec::new();
    for (&cable, c) in &state.cables {
        let PortRef::Param { id: to, param: p } = &c.to else {
            continue;
        };
        if *to != id || p != param {
            continue;
        }
        let PortRef::Module {
            id: from_id,
            port: from_port,
        } = &c.from
        else {
            continue;
        };
        let src = state
            .modules
            .get(from_id)
            .and_then(|m| registry::info_for(&m.kind))
            .and_then(|info| {
                info.ports
                    .iter()
                    .find(|p| p.direction == PortDirection::Output && p.name == from_port)
            })
            .map(|p| {
                let (lo, hi) = p.port_type.nominal_range();
                let full = lo.abs().max(hi.abs());
                (lo / full, hi / full)
            })
            .unwrap_or((-1.0, 1.0));
        out.push(RouteView {
            cable,
            from_id: *from_id,
            from_port: from_port.clone(),
            amount: c
                .params
                .get("amount")
                .copied()
                .unwrap_or(DEFAULT_ROUTE_AMOUNT),
            bypass: c.params.get("bypass").is_some_and(|&b| b >= 0.5),
            src,
        });
    }
    out
}

/// Unclamped knob-travel span one route can add to `base_norm`: `(low, high)`.
pub fn route_span(r: &RouteView, base_norm: f32) -> (f32, f32) {
    let (a, b) = (
        base_norm + r.amount * r.src.0,
        base_norm + r.amount * r.src.1,
    );
    (a.min(b), a.max(b))
}

/// Unclamped span of every active (non-bypassed) route summed, as the engine sums them.
pub fn combined_span(routes: &[RouteView], base_norm: f32) -> (f32, f32) {
    let (mut lo, mut hi) = (base_norm, base_norm);
    for r in routes.iter().filter(|r| !r.bypass) {
        let (a, b) = (r.amount * r.src.0, r.amount * r.src.1);
        lo += a.min(b);
        hi += a.max(b);
    }
    (lo, hi)
}

/// "LFO #9", or "MIDI In #1 velocity" for a non-`out` port; a named macro by its name.
pub fn source_label(state: &PatchState, from_id: ModuleId, port: &str) -> String {
    if let Some(n) = crate::macro_name(state, from_id, port) {
        return format!("{n} (Macros #{from_id})");
    }
    let name = state
        .modules
        .get(&from_id)
        .and_then(|m| registry::info_for(&m.kind))
        .map_or("?", |i| i.name);
    if port == "out" {
        format!("{name} #{from_id}")
    } else {
        format!("{name} #{from_id} {port}")
    }
}

/// Option names for stepped params, where the module defines them.
pub fn step_labels(kind: &str, param: &str) -> Option<&'static [&'static str]> {
    let param = if kind == "seq" {
        seq::slot_name(param)
    } else {
        param
    };
    Some(match (kind, param) {
        ("env.adsr", "timing") => &["CONT", "KEY"],
        ("lfo", "waveform") => &["SIN", "TRI", "SAW", "SQR", "S&H"],
        ("osc.va", "waveform") => &["SIN", "TRI", "SAW", "SQR"],
        ("noise", "color") => &["WHITE", "PINK"],
        ("vca", "exponential") => &["LIN", "EXP"],
        ("seq", g) if g.starts_with('g') && g.len() == 2 => &["OFF", "ON"],
        ("delay", "sync") => &["FREE", "1/16", "1/8", "1/8D", "1/4"],
        ("delay", "mode") => &["MONO", "PING"],
        ("seq", "gate_mode") => &["CLOCK", "LENGTH"],
        ("seq", "direction") => &["FWD", "REV", "PEND"],
        ("lfo", "sync") => &kabl_modules::builtins::SYNC_LABELS,
        ("seq", "bank") => &seq::BANK_NAMES,
        ("midi.in", "mode") => &["POLY", "MONO", "LEGATO"],
        ("midi.in", "priority") => &["LAST", "LOW", "HIGH"],
        ("midi.in", "glide") => &["OFF", "ALWAYS", "LEGATO"],
        _ => return None,
    })
}

/// Label naming the target outside its module's face: a sequencer bank param says its bank
/// (`B · P3`), everything else as `param_label`.
pub fn target_label(p: &ParamInfo) -> String {
    match seq::bank_of(p.name) {
        Some((b, _)) if SEQ_INFO.params.iter().any(|q| std::ptr::eq(q, p)) => {
            format!("{} · {}", seq::BANK_NAMES[b], param_label(p))
        }
        _ => param_label(p),
    }
}

/// Human label for a param name: `attack_ms` → `Attack`. A sequencer bank param is labelled
/// by its slot (`c.v2` → `V2`), as on the face.
pub fn param_label(p: &ParamInfo) -> String {
    let name = if SEQ_INFO.params.iter().any(|q| std::ptr::eq(q, p)) {
        let slot = seq::slot_name(p.name);
        if let Some(k) = slot.strip_prefix('r').filter(|k| k.len() == 1) {
            return format!("Prob {k}");
        }
        match slot {
            "bank" => return "Startup".into(),
            "direction" => return "Direction".into(),
            _ => slot,
        }
    } else {
        p.name
    };
    match name {
        "base_hz" => return "Frequency".into(),
        "exponential" => return "Response".into(),
        "div" => return "Divide by".into(),
        "gate_len" => return "Gate length".into(),
        "gate_mode" => return "Gate".into(),
        "damp_hz" => return "Damping".into(),
        "pw" => return "Pulse width".into(),
        "predelay_ms" => return "Pre-delay".into(),
        "glide_ms" => return "Glide time".into(),
        n if n.starts_with("level") && n.len() > 5 => return format!("Level {}", &n[5..]),
        _ => {}
    }
    let base = name
        .strip_suffix(&format!("_{}", p.unit.to_lowercase()))
        .unwrap_or(name);
    let mut s = base.replace('_', " ");
    if let Some(first) = s.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    s
}

pub fn fmt_value(p: &ParamInfo, v: f32) -> String {
    match p.unit {
        "ms" if v >= 1000.0 => format!("{:.2} s", v / 1000.0),
        "ms" if v < 10.0 => format!("{v:.2} ms"),
        "ms" => format!("{v:.0} ms"),
        "Hz" if v >= 1000.0 => format!("{:.2} kHz", v / 1000.0),
        "Hz" if v < 10.0 => format!("{v:.2} Hz"),
        "Hz" => format!("{v:.0} Hz"),
        // `+ 0.0` turns the -0 that `round` gives for -0.4 into 0, as the module plays it.
        "st" => format!("{:+} st", v.round() + 0.0),
        "bpm" => format!("{v:.0} bpm"),
        "%" => format!("{v:.0} %"),
        "dB" => format!("{:+.1} dB", v + 0.0),
        "°" => format!("{v:.0}°"),
        "ct" => format!("{:+.0} ct", v.round() + 0.0),
        _ => format!("{v:.2}"),
    }
}

/// Parses `"8"`, `"8 ms"`, `"1.5 s"`, `"2k"`, `"2 kHz"` into the param's own unit.
pub fn parse_value(p: &ParamInfo, text: &str) -> Option<f32> {
    let t = text.trim().to_lowercase();
    let num_end = t
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+'))
        .unwrap_or(t.len());
    let v: f32 = t[..num_end].parse().ok()?;
    let suffix = t[num_end..].trim();
    let scale = match (p.unit, suffix) {
        ("ms", "s") => 1000.0,
        ("Hz", "k" | "khz") => 1000.0,
        _ => 1.0,
    };
    let v = v * scale;
    v.is_finite().then(|| v.clamp(p.min, p.max))
}

/// Angle (radians from 12 o'clock, clockwise) of knob travel `n` on a 270° scale.
fn travel_angle(n: f32) -> f32 {
    (-135.0 + n.clamp(0.0, 1.0) * 270.0).to_radians()
}

fn on_circle(center: Pos2, radius: f32, angle: f32) -> Pos2 {
    center + EguiVec2::new(angle.sin(), -angle.cos()) * radius
}

fn arc(painter: &egui::Painter, center: Pos2, radius: f32, n0: f32, n1: f32, stroke: Stroke) {
    let (a0, a1) = (travel_angle(n0.min(n1)), travel_angle(n0.max(n1)));
    let steps = (((a1 - a0).abs() / 0.08).ceil() as usize).max(1);
    let points: Vec<Pos2> = (0..=steps)
        .map(|i| on_circle(center, radius, a0 + (a1 - a0) * i as f32 / steps as f32))
        .collect();
    painter.add(egui::Shape::line(points, stroke));
}

/// What a knob-area drag edits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Grab {
    /// The base value (knob body), in knob travel 0..1.
    Body,
    /// The selected route's amount through the ring band.
    Ring(CableId),
    /// Ring band pressed with no source selected: edits nothing.
    RingNone,
    /// A route's amount through its lane dot.
    Lane(CableId),
}

/// Shift-drag moves values this much slower.
const FINE: f32 = 0.1;

/// One drag on a knob body, ring or lane dot. The value follows each frame's vertical pointer
/// motion, from the press point on (so egui's drag threshold loses nothing), ×0.1 while Shift
/// is held. Escape cancels it (see `show`): the edit is reverted and leaves no undo entry.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DragGrab {
    pub id: ModuleId,
    pub param: &'static str,
    pub kind: Grab,
    pub value: f32,
    last_y: f32,
    /// The gesture has written its first (undoable) edit.
    pub committed: bool,
    pub cancelled: bool,
}

impl DragGrab {
    fn new(ui: &egui::Ui, id: ModuleId, param: &'static str, kind: Grab, value: f32) -> Self {
        DragGrab {
            id,
            param,
            kind,
            value,
            last_y: ui.input(|i| i.pointer.press_origin()).map_or(0.0, |p| p.y),
            committed: false,
            cancelled: false,
        }
    }

    /// Advances by this frame's pointer motion. Returns the value to write and whether it is
    /// the gesture's first write, or `None` once cancelled.
    fn step(
        &mut self,
        ui: &egui::Ui,
        resp: &egui::Response,
        lo: f32,
        hi: f32,
    ) -> Option<(f32, bool)> {
        if self.cancelled {
            return None;
        }
        let y = resp.interact_pointer_pos()?.y;
        let scale = if ui.input(|i| i.modifiers.shift) {
            FINE
        } else {
            1.0
        };
        self.value =
            (self.value - (y - self.last_y) / DRAG_PIXELS_FOR_FULL_SWEEP * scale).clamp(lo, hi);
        self.last_y = y;
        let first = !self.committed;
        self.committed = true;
        Some((self.value, first))
    }

    fn is(&self, id: ModuleId, param: &str) -> bool {
        self.id == id && self.param == param
    }
}

/// Makes `(id, param)` the inspected knob. Newly inspecting a knob with exactly one route
/// selects that route; with several, the selection stays only if it belongs to this knob.
/// Re-pressing the knob already inspected never changes the selection, so after the selected
/// route is removed the ring edits nothing until a source is picked explicitly.
pub(crate) fn inspect(ui_state: &mut UiState, routes: &[RouteView], id: ModuleId, param: &str) {
    ui_state.selected_module = Some(id);
    if ui_state.inspected.as_ref() == Some(&(id, param.to_string())) {
        return;
    }
    ui_state.inspected = Some((id, param.to_string()));
    if routes.len() == 1 {
        ui_state.selected_route = Some(routes[0].cable);
    } else if !routes
        .iter()
        .any(|r| Some(r.cable) == ui_state.selected_route)
    {
        ui_state.selected_route = None;
    }
}

/// The stored base value of `param` on module `id`, or its default. Reads a mixer level from the
/// pre-rename shared `level` when that is what an old patch stored (as the compiler does).
pub fn base_value(state: &PatchState, id: ModuleId, param: &ParamInfo) -> f32 {
    let Some(m) = state.modules.get(&id) else {
        return param.default;
    };
    m.params
        .get(param.name)
        .or_else(|| registry::legacy_param(&m.kind, param.name).and_then(|old| m.params.get(old)))
        .copied()
        .unwrap_or(param.default)
}

/// Short module name for badges: `LFO`, `ADSR`, `Osc`.
pub fn short_name(kind: &str) -> &'static str {
    match kind {
        "midi.in" => "MIDI",
        "osc.va" => "Osc",
        "filter.svf" => "Filter",
        "env.adsr" => "ADSR",
        "lfo" => "LFO",
        "vca" => "VCA",
        "out" => "Out",
        "mixer" => "Mixer",
        "ringmod" => "Ring",
        "noise" => "Noise",
        "filter.ladder" => "Ladder",
        "chorus" => "Chorus",
        "drive" => "Drive",
        _ => "?",
    }
}

fn two_tone_arc(p: &egui::Painter, c: Pos2, r: f32, n0: f32, n1: f32, col: Color32, w: f32) {
    let outline = col.lerp_to_gamma(Color32::BLACK, 0.45);
    let outline = Color32::from_rgba_unmultiplied(outline.r(), outline.g(), outline.b(), col.a());
    arc(p, c, r, n0, n1, Stroke::new(w + 2.5, outline));
    arc(p, c, r, n0, n1, Stroke::new(w, col));
}

/// Text drawn after the cables (values, pills, badges), so a cable never hides a value.
fn defer_text(
    ui_state: &mut UiState,
    painter: &egui::Painter,
    pos: Pos2,
    align: egui::Align2,
    text: &str,
    font: egui::FontId,
    color: Color32,
) -> Rect {
    let galley = painter.layout_no_wrap(text.to_string(), font, color);
    let rect = align.anchor_size(pos, galley.size());
    ui_state
        .deferred
        .push(egui::Shape::galley(rect.min, galley, color));
    rect
}

/// Rounded value pill with a coloured edge (a modulated knob's value), drawn after the cables.
fn defer_pill(
    ui_state: &mut UiState,
    painter: &egui::Painter,
    center: Pos2,
    text: &str,
    edge: Color32,
    z: f32,
) {
    let th = theme(ui_state.dark);
    let galley =
        painter.layout_no_wrap(text.to_string(), egui::FontId::monospace(11.5 * z), th.ink);
    let size = EguiVec2::new((galley.size().x + 12.0 * z).max(44.0 * z), 20.0 * z);
    let rect = Rect::from_center_size(center, size);
    let cr = egui::CornerRadius::same((size.y / 2.0) as u8);
    ui_state
        .deferred
        .push(egui::Shape::rect_filled(rect, cr, th.panel));
    ui_state.deferred.push(egui::Shape::rect_stroke(
        rect,
        cr,
        Stroke::new(1.6 * z, edge),
        egui::StrokeKind::Inside,
    ));
    let at = rect.center() - galley.size() / 2.0;
    ui_state
        .deferred
        .push(egui::Shape::galley(at, galley, th.ink));
}

/// Badge (Hidden cables): text in a rounded outline, drawn after the cables.
pub(crate) fn defer_badge(
    ui_state: &mut UiState,
    painter: &egui::Painter,
    center: Pos2,
    text: &str,
    col: Color32,
    z: f32,
) {
    let th = theme(ui_state.dark);
    let galley =
        painter.layout_no_wrap(text.to_string(), egui::FontId::proportional(10.5 * z), col);
    let rect = Rect::from_center_size(center, EguiVec2::new(galley.size().x + 12.0 * z, 15.0 * z));
    let cr = egui::CornerRadius::same((7.0 * z) as u8);
    ui_state
        .deferred
        .push(egui::Shape::rect_filled(rect, cr, th.panel));
    ui_state.deferred.push(egui::Shape::rect_stroke(
        rect,
        cr,
        Stroke::new(1.3 * z, col),
        egui::StrokeKind::Inside,
    ));
    let at = rect.center() - galley.size() / 2.0;
    ui_state.deferred.push(egui::Shape::galley(at, galley, col));
}

/// Draws and handles one continuous param knob of radius `r` (world units) at screen `center`.
/// Returns the plug points (route cable ends) in route order; routes past `MAX_PLUGS` share the
/// last point.
#[allow(clippy::too_many_arguments)]
pub(crate) fn param_knob(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    id: ModuleId,
    param: &'static ParamInfo,
    center: Pos2,
    r: f32,
    look: Look,
) -> Vec<(CableId, Pos2)> {
    let th = theme(ui_state.dark);
    let z = look.z;
    let base = base_value(editor.state(), id, param);
    let routes = routes_into(editor.state(), id, param.name);
    let base_n = param.to_norm(base);
    let key = format!("{id}.{}", param.name);
    let inspected = ui_state.inspected.as_ref() == Some(&(id, param.name.to_string()));

    let rs = r * z;
    let rect = Rect::from_center_size(center, EguiVec2::splat((r + RING_OUTER) * z * 2.0));
    ui_state.record(
        format!("knob:{key}"),
        Rect::from_center_size(center, EguiVec2::splat(((r + 4.0) * z * 2.0).max(20.0))),
    );
    if !routes.is_empty() {
        ui_state.record(
            format!("ring:{key}"),
            Rect::from_center_size(
                on_circle(center, (r + RING_DRAW) * z, travel_angle(0.5)),
                EguiVec2::splat(6.0),
            ),
        );
    }
    let resp = ui.interact(
        rect,
        Id::new(("kabl-knob", id, param.name)),
        Sense::click_and_drag(),
    );

    if resp.clicked() || resp.drag_started() {
        inspect(ui_state, &routes, id, param.name);
    }
    if resp.drag_started() {
        // Where the button went down, not where the pointer is once egui calls it a drag: a
        // quick flick from the body must not grab the ring.
        let d = ui
            .input(|i| i.pointer.press_origin())
            .map_or(0.0, |p| p.distance(center));
        let ring = d > (r + RING_INNER) * z;
        let selected = routes
            .iter()
            .find(|rt| Some(rt.cable) == ui_state.selected_route);
        let grab = match selected {
            // Collapsed multi-source rings are display only: pressing them opens the lanes
            // (the `inspect` above) and edits nothing.
            _ if routes.len() >= 2 && !inspected && ring => None,
            Some(sel) if ring => Some((Grab::Ring(sel.cable), sel.amount)),
            None if !routes.is_empty() && ring => Some((Grab::RingNone, 0.0)),
            _ => Some((Grab::Body, base_n)),
        };
        ui_state.drag = grab.map(|(k, v)| DragGrab::new(ui, id, param.name, k, v));
    }
    let mut grab = ui_state.drag.filter(|g| g.is(id, param.name));
    if resp.dragged() {
        if let Some(g) = grab.as_mut() {
            match g.kind {
                Grab::Ring(cable) => {
                    if let Some((amount, first)) = g.step(ui, &resp, -1.0, 1.0) {
                        editor.set_route_amount(cable, amount, first);
                    }
                }
                Grab::Body => {
                    if let Some((n, first)) = g.step(ui, &resp, 0.0, 1.0) {
                        editor.set_param_gesture(
                            ParamTarget::Module {
                                id,
                                param: param.name.into(),
                            },
                            param.from_norm(n),
                            first,
                        );
                    }
                }
                Grab::RingNone | Grab::Lane(_) => {}
            }
            ui_state.drag = Some(*g);
        }
    }
    if resp.drag_stopped() && grab.is_some() {
        ui_state.drag = None;
    }
    // Re-read after this frame's edit so the drawing below shows the new value.
    let base = base_value(editor.state(), id, param);
    let base_n = param.to_norm(base);
    let routes = routes_into(editor.state(), id, param.name);

    // Scale, skirt, cap, pointer at the base value (the A / A-dark knob).
    for i in 0..=10 {
        let a = travel_angle(i as f32 / 10.0);
        painter.line_segment(
            [
                on_circle(center, rs + 4.0 * z, a),
                on_circle(center, rs + 7.5 * z, a),
            ],
            Stroke::new(1.2 * z, th.tick),
        );
    }
    let hot = resp.hovered() || resp.dragged() || inspected;
    painter.circle_filled(center, rs + 1.5 * z, th.skirt);
    painter.circle_filled(center, rs * 0.84, th.knob);
    painter.circle_filled(
        center + EguiVec2::new(-0.25, -0.3) * rs,
        rs * 0.35,
        th.knob_hi.gamma_multiply(0.35),
    );
    if th.dark {
        // Knurled light-metal cap.
        for i in 0..24 {
            let a = i as f32 / 24.0 * std::f32::consts::TAU;
            let dir = EguiVec2::new(a.cos(), a.sin());
            painter.line_segment(
                [center + dir * rs * 0.74, center + dir * rs * 0.84],
                Stroke::new(1.0, th.knob.lerp_to_gamma(Color32::BLACK, 0.3)),
            );
        }
    }
    if hot {
        painter.circle_stroke(center, rs + 1.5 * z, Stroke::new(1.2 * z, th.sel));
    }
    let a = travel_angle(base_n);
    painter.line_segment(
        [
            on_circle(center, rs * 0.2, a),
            on_circle(center, rs * 0.78, a),
        ],
        Stroke::new(2.6 * z, th.pointer),
    );

    // Collapsed knob with several sources: one thin ring per source (display only; pressing
    // opens the editable lanes). Otherwise: combined reachable range, the selected route's own
    // span on top + its peak dot.
    let rr = (r + RING_DRAW) * z;
    if routes.len() >= 2 && !inspected {
        for (k, rt) in routes.iter().take(MINI_LANES).enumerate() {
            let radius = (r + RING_INNER + 1.0 + MINI_GAP * k as f32) * z;
            let (lo, hi) = route_span(rt, base_n);
            let color = if rt.bypass {
                BYPASS_GREY
            } else {
                cable_color(rt.cable)
            };
            arc(
                painter,
                center,
                radius,
                0.0,
                1.0,
                Stroke::new(0.5, th.tick.gamma_multiply(0.6)),
            );
            arc(
                painter,
                center,
                radius,
                lo.clamp(0.0, 1.0),
                hi.clamp(0.0, 1.0),
                Stroke::new(1.8 * z, color),
            );
        }
    } else if !routes.is_empty() {
        if routes.iter().any(|rt| !rt.bypass) {
            let (lo, hi) = combined_span(&routes, base_n);
            let strong = inspected
                || routes
                    .iter()
                    .any(|rt| Some(rt.cable) == ui_state.selected_route);
            for (edge, clamped) in [(0.0, lo < 0.0), (1.0, hi > 1.0)] {
                if clamped {
                    let a = travel_angle(edge);
                    let (p0, p1) = (
                        on_circle(center, rs + 5.0 * z, a),
                        on_circle(center, rs + 17.0 * z, a),
                    );
                    painter.line_segment(
                        [p0, p1],
                        Stroke::new(4.5 * z, th.cv.lerp_to_gamma(Color32::BLACK, 0.45)),
                    );
                    painter.line_segment([p0, p1], Stroke::new(2.5 * z, th.cv));
                }
            }
            two_tone_arc(
                painter,
                center,
                rr,
                lo.clamp(0.0, 1.0),
                hi.clamp(0.0, 1.0).max(lo.clamp(0.0, 1.0) + 0.004),
                if strong {
                    th.cv
                } else {
                    th.cv.gamma_multiply(0.75)
                },
                if strong { 4.0 } else { 3.0 } * z,
            );
        } else {
            let (lo, hi) = combined_span(
                &routes
                    .iter()
                    .map(|rt| RouteView {
                        bypass: false,
                        ..rt.clone()
                    })
                    .collect::<Vec<_>>(),
                base_n,
            );
            painter.extend(egui::Shape::dashed_line(
                &arc_points(center, rr, lo.clamp(0.0, 1.0), hi.clamp(0.0, 1.0)),
                Stroke::new(2.0 * z, look.ink2),
                4.0 * z,
                3.0 * z,
            ));
        }
        if let Some(sel) = routes
            .iter()
            .find(|rt| Some(rt.cable) == ui_state.selected_route)
        {
            let (lo, hi) = route_span(sel, base_n);
            let color = if sel.bypass {
                BYPASS_GREY
            } else {
                cable_color(sel.cable)
            };
            arc(
                painter,
                center,
                rr,
                lo.clamp(0.0, 1.0),
                hi.clamp(0.0, 1.0),
                Stroke::new(1.6 * z, color),
            );
            let tip = on_circle(
                center,
                rr,
                travel_angle((base_n + sel.amount * sel.src.1).clamp(0.0, 1.0)),
            );
            painter.circle_filled(tip, 4.2 * z, th.panel);
            painter.circle_stroke(
                tip,
                4.2 * z,
                Stroke::new(2.4 * z, color.lerp_to_gamma(Color32::BLACK, 0.45)),
            );
            painter.circle_filled(tip, 2.2 * z, color);
        }
    }

    // Plugs in the 6 o'clock gap.
    let shown = routes.len().min(MAX_PLUGS);
    let mut plugs = Vec::with_capacity(routes.len());
    for (i, rt) in routes.iter().enumerate() {
        let slot = i.min(MAX_PLUGS - 1);
        let x = (slot as f32 - (shown as f32 - 1.0) / 2.0) * PLUG_SPREAD * z;
        let p = center + EguiVec2::new(x, rs + PLUG_DROP * z);
        plugs.push((rt.cable, p));
        if i < MAX_PLUGS && ui_state.cable_view == crate::CableView::Hidden {
            let c = if rt.bypass {
                BYPASS_GREY
            } else {
                cable_color(rt.cable)
            };
            painter.circle_filled(p, 5.0 * z, c.lerp_to_gamma(Color32::BLACK, 0.45));
            painter.circle_filled(p, 3.4 * z, c);
        }
    }

    // Label above, value below (a pill with a coloured edge when modulated).
    painter.text(
        center - EguiVec2::new(0.0, 47.0 * z),
        egui::Align2::CENTER_CENTER,
        crate::macro_name(editor.state(), id, param.name).unwrap_or_else(|| param_label(param)),
        egui::FontId::proportional(12.5 * z),
        look.ink,
    );
    let value = fmt_value(param, base);
    let vpos = center + EguiVec2::new(0.0, 38.0 * z);
    if routes.is_empty() {
        defer_text(
            ui_state,
            painter,
            vpos,
            egui::Align2::CENTER_CENTER,
            &value,
            egui::FontId::monospace(11.5 * z),
            look.ink,
        );
    } else {
        let edge = if routes.iter().any(|rt| !rt.bypass) {
            th.cv
        } else {
            look.ink2
        };
        defer_pill(ui_state, painter, vpos, &value, edge, z);
        if routes.len() > MAX_PLUGS {
            defer_text(
                ui_state,
                painter,
                center + EguiVec2::new(PLUG_SPREAD * 1.6 * z, rs + PLUG_DROP * z),
                egui::Align2::LEFT_CENTER,
                &format!("+{}", routes.len() - (MAX_PLUGS - 1)),
                egui::FontId::proportional(10.0 * z),
                edge,
            );
        }
        // Hidden cables: say where modulation comes from.
        if ui_state.cable_view == crate::CableView::Hidden {
            // ASCII only: the bundled fallback fonts have no arrow glyphs.
            let badge = if routes.len() == 1 {
                let kind = editor
                    .state()
                    .modules
                    .get(&routes[0].from_id)
                    .map_or("", |m| m.kind.as_str());
                format!("< {}", short_name(kind))
            } else {
                format!("< {} mods", routes.len())
            };
            defer_badge(
                ui_state,
                painter,
                vpos + EguiVec2::new(0.0, 19.0 * z),
                &badge,
                edge,
                z,
            );
        }
    }

    // While editing: the value being changed, and whose.
    let fine = if ui.input(|i| i.modifiers.shift) {
        " · fine"
    } else {
        ""
    };
    let editing = match grab.map(|g| (g.kind, g.cancelled)) {
        Some((_, true)) if resp.dragged() => Some("Cancelled".to_string()),
        Some((Grab::Ring(_), _)) if resp.dragged() => routes
            .iter()
            .find(|rt| Some(rt.cable) == ui_state.selected_route)
            .map(|rt| {
                format!(
                    "{}: {:+.1} %{fine}",
                    source_label(editor.state(), rt.from_id, &rt.from_port),
                    rt.amount * 100.0
                )
            }),
        Some((Grab::RingNone, _)) if resp.dragged() => {
            Some("No source selected: pick one in the drawer".to_string())
        }
        Some((Grab::Body, _)) if resp.dragged() => Some(format!(
            "{} {}{fine}",
            param_label(param),
            fmt_value(param, base)
        )),
        _ if resp.hovered() => Some(format!("{} {}", param_label(param), fmt_value(param, base))),
        _ => None,
    };
    if let Some(text) = editing {
        // Tooltip layer: above cables and not clipped by the canvas.
        let top = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            Id::new("kabl-knob-pill"),
        ));
        pill(
            &top,
            center - EguiVec2::new(0.0, (r + RING_OUTER) * z + 4.0),
            &text,
        );
    }

    if inspected && !routes.is_empty() {
        source_lanes(
            editor, ui_state, ui, id, param, center, r, z, base_n, &routes,
        );
    }
    plugs
}

fn arc_points(center: Pos2, radius: f32, n0: f32, n1: f32) -> Vec<Pos2> {
    let (a0, a1) = (travel_angle(n0.min(n1)), travel_angle(n0.max(n1)));
    let steps = (((a1 - a0).abs() / 0.08).ceil() as usize).max(1);
    (0..=steps)
        .map(|i| on_circle(center, radius, a0 + (a1 - a0) * i as f32 / steps as f32))
        .collect()
}

/// Unzoomed radius of the backdrop behind `n` source lanes around a knob of radius `r`.
pub(crate) fn lanes_outer(r: f32, n: usize) -> f32 {
    r + RING_OUTER + LANE_GAP * n as f32 + LANE_GAP / 2.0
}

/// Concentric lanes, one per route, shown around the inspected knob (a single lane for a
/// single source): each lane draws that route's own span, and its handle (at the route's positive
/// peak) selects the route and drags its amount, without the drawer. Drawn and hit-tested on
/// the foreground layer so neighbouring knobs never steal the handles.
#[allow(clippy::too_many_arguments)]
fn source_lanes(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    id: ModuleId,
    param: &'static ParamInfo,
    center: Pos2,
    r: f32,
    z: f32,
    base_n: f32,
    routes: &[RouteView],
) {
    let lane_radius = |k: usize| (r + RING_OUTER + LANE_GAP * (k as f32 + 1.0)) * z;
    let canvas = ui.clip_rect();
    let top = ui
        .ctx()
        .layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            Id::new(("kabl-lanes", id, param.name)),
        ))
        .with_clip_rect(canvas);
    let outer = lanes_outer(r, routes.len()) * z;
    top.circle_filled(
        center,
        outer,
        Color32::from_rgba_unmultiplied(14, 14, 18, 215),
    );
    for (k, rt) in routes.iter().enumerate() {
        let radius = lane_radius(k);
        let selected = Some(rt.cable) == ui_state.selected_route;
        let color = if rt.bypass {
            BYPASS_GREY
        } else {
            cable_color(rt.cable)
        };
        arc(
            &top,
            center,
            radius,
            0.0,
            1.0,
            Stroke::new(1.0, Color32::from_gray(60)),
        );
        let (lo, hi) = route_span(rt, base_n);
        arc(
            &top,
            center,
            radius,
            lo.clamp(0.0, 1.0),
            hi.clamp(0.0, 1.0),
            Stroke::new(if selected { 4.0 } else { 2.5 } * z.max(0.7), color),
        );
        let peak = (base_n + rt.amount * rt.src.1).clamp(0.0, 1.0);
        let at = on_circle(center, radius, travel_angle(peak));
        // At least 9 px to grab, whatever the zoom.
        let hit = Rect::from_center_size(at, EguiVec2::splat(((LANE_GAP + 2.0) * z).max(9.0)));
        // A dot outside the canvas is not drawn, so it must not catch presses meant for the
        // drawer or toolbar either (unless it is mid-drag: the drag keeps its dot).
        let dragging = ui_state
            .drag
            .is_some_and(|g| g.is(id, param.name) && g.kind == Grab::Lane(rt.cable));
        if !canvas.contains(at) && !dragging {
            continue;
        }
        ui_state.record(format!("lane:{}", rt.cable), hit);
        let resp = egui::Area::new(Id::new(("kabl-lane", rt.cable)))
            .order(egui::Order::Foreground)
            .fixed_pos(hit.min)
            .constrain(false)
            .show(ui.ctx(), |ui| {
                ui.allocate_exact_size(hit.size(), Sense::click_and_drag())
                    .1
            })
            .inner;
        let hot = selected || resp.hovered() || resp.dragged();
        top.circle_filled(at, if hot { 5.0 } else { 3.5 } * z.max(0.7), color);
        if hot {
            top.circle_stroke(at, 5.5 * z.max(0.7), Stroke::new(1.0, Color32::WHITE));
        }
        if resp.clicked() || resp.drag_started() {
            ui_state.selected_route = Some(rt.cable);
        }
        if resp.drag_started() {
            ui_state.drag = Some(DragGrab::new(
                ui,
                id,
                param.name,
                Grab::Lane(rt.cable),
                rt.amount,
            ));
        }
        let mut grab = ui_state
            .drag
            .filter(|g| g.is(id, param.name) && g.kind == Grab::Lane(rt.cable));
        if resp.dragged() {
            if let Some(g) = grab.as_mut() {
                if let Some((amount, first)) = g.step(ui, &resp, -1.0, 1.0) {
                    editor.set_route_amount(rt.cable, amount, first);
                }
                ui_state.drag = Some(*g);
            }
        }
        if resp.drag_stopped() && grab.is_some() {
            ui_state.drag = None;
        }
        if resp.hovered() || resp.dragged() {
            pill(
                &ui.ctx().layer_painter(egui::LayerId::new(
                    egui::Order::Tooltip,
                    Id::new("kabl-lane-pill"),
                )),
                center - EguiVec2::new(0.0, outer + 4.0),
                &match grab {
                    Some(g) if g.cancelled => "Cancelled".to_string(),
                    _ => format!(
                        "{}: {:+.1} %",
                        source_label(editor.state(), rt.from_id, &rt.from_port),
                        grab.map_or(rt.amount, |g| g.value) * 100.0
                    ),
                },
            );
        }
    }
}

pub(crate) fn pill(painter: &egui::Painter, bottom_center: Pos2, text: &str) {
    let galley = painter.layout_no_wrap(
        text.to_string(),
        egui::FontId::proportional(11.0),
        Color32::WHITE,
    );
    let size = galley.size() + EguiVec2::new(10.0, 4.0);
    let rect = Rect::from_center_size(bottom_center - EguiVec2::new(0.0, size.y / 2.0), size);
    painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(20, 20, 22, 235));
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0, Color32::from_rgb(90, 155, 255)),
        egui::StrokeKind::Outside,
    );
    painter.galley(rect.min + EguiVec2::new(5.0, 2.0), galley, Color32::WHITE);
}

/// Segmented selector for a `Taper::Stepped` param (envelope timing, waveforms) at screen
/// `rect`, label above. Accepts routes like a knob (its whole rect is the drop target); returns
/// its plug point for route cables.
#[allow(clippy::too_many_arguments)]
pub(crate) fn stepped_selector(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    id: ModuleId,
    kind: &str,
    param: &'static ParamInfo,
    rect: Rect,
    look: Look,
) -> Vec<(CableId, Pos2)> {
    debug_assert_eq!(param.taper, Taper::Stepped);
    let th = theme(ui_state.dark);
    let z = look.z;
    let value = base_value(editor.state(), id, param).round();
    let n = (param.max - param.min).round() as usize + 1;
    let labels = step_labels(kind, param.name);
    let key = format!("{id}.{}", param.name);
    ui_state.record(format!("knob:{key}"), rect);
    let routes = routes_into(editor.state(), id, param.name);
    painter.text(
        egui::pos2(rect.center().x, rect.top() - 13.0 * z),
        egui::Align2::CENTER_CENTER,
        param_label(param),
        egui::FontId::proportional(12.5 * z),
        look.ink,
    );
    painter.rect_filled(rect, 4.0 * z, th.seg_bg);
    let w = rect.width() / n as f32;
    for k in 0..n {
        let opt = param.min + k as f32;
        let r = Rect::from_min_size(
            rect.min + EguiVec2::new(k as f32 * w, 0.0),
            EguiVec2::new(w, rect.height()),
        );
        ui_state.record(format!("sel:{key}.{k}"), r.shrink(1.0));
        let resp = ui.interact(r, Id::new(("kabl-sel", id, param.name, k)), Sense::click());
        let on = opt == value;
        if on {
            painter.rect_filled(r.shrink(2.0 * z), 3.0 * z, th.seg_on);
        } else if resp.hovered() {
            painter.rect_stroke(
                r.shrink(2.0 * z),
                3.0 * z,
                Stroke::new(1.0, th.sel),
                egui::StrokeKind::Inside,
            );
        }
        let text = labels
            .and_then(|l| l.get(k).copied())
            .map_or(format!("{}", param.min + k as f32), str::to_string);
        painter.text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::monospace(11.5 * z),
            if on { th.seg_on_text } else { look.ink2 },
        );
        if resp.clicked() {
            inspect(ui_state, &routes, id, param.name);
            if !on {
                editor.set_param(id, param.name, opt);
            }
        }
    }
    let plug = Pos2::new(rect.center().x, rect.max.y + 6.0 * z);
    if !routes.is_empty() {
        let col = if routes.iter().all(|r| r.bypass) {
            look.ink2
        } else {
            th.cv
        };
        painter.circle_filled(plug, 3.4 * z, col);
        if ui_state.cable_view == crate::CableView::Hidden {
            defer_badge(
                ui_state,
                painter,
                plug + EguiVec2::new(0.0, 12.0 * z),
                &format!(
                    "< {} mod{}",
                    routes.len(),
                    if routes.len() == 1 { "" } else { "s" }
                ),
                col,
                z,
            );
        }
    }
    routes.iter().map(|rt| (rt.cable, plug)).collect()
}

/// The routing drawer for the inspected param: base entry, reachable range, one row per route
/// with select / amount / invert / bypass / remove.
pub(crate) fn drawer(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui) {
    let Some((id, pname)) = ui_state.inspected.clone() else {
        return;
    };
    let Some((kind, param)) = editor.state().modules.get(&id).and_then(|m| {
        let info = registry::info_for(&m.kind)?;
        Some((
            m.kind.clone(),
            info.params.iter().find(|p| p.name == pname)?,
        ))
    }) else {
        ui_state.inspected = None;
        return;
    };
    let base = base_value(editor.state(), id, param);
    let routes = routes_into(editor.state(), id, param.name);

    ui.separator();
    let head = ui.horizontal_wrapped(|ui| {
        ui.heading(format!("{} · {kind} #{id}", target_label(param)));
        let r = ui
            .small_button("Explain")
            .on_hover_text("What this control does, its base value and what moves it");
        ui_state.record(format!("explain-control:{id}.{pname}"), r.rect);
        if r.clicked() {
            crate::explain::open(
                ui_state,
                crate::explain::Subject::Control {
                    id,
                    key: pname.clone(),
                },
                None,
            );
        }
    });
    // Just shown from an explanation: this control's routes, not the module list, in view.
    if std::mem::take(&mut ui_state.explain.scroll_to_routes) {
        ui.scroll_to_rect(head.response.rect, Some(egui::Align::TOP));
    }
    if ui_state.explain.help {
        if let Some(h) = crate::help::param_help(&kind, param.name) {
            ui.add(egui::Label::new(h).wrap());
        }
        ui.add(
            egui::Label::new(
                egui::RichText::new(crate::help::range_text(&kind, param))
                    .small()
                    .weak(),
            )
            .wrap(),
        );
    }

    // Base value entry.
    ui.horizontal(|ui| {
        ui.label("Base");
        let key = (id, pname.clone());
        if ui_state.base_text_for.as_ref() != Some(&key) {
            ui_state.base_text = fmt_value(param, base);
            ui_state.base_text_for = Some(key);
        }
        let resp = ui.add(
            egui::TextEdit::singleline(&mut ui_state.base_text)
                .desired_width(80.0)
                .id(Id::new("kabl-base-entry")),
        );
        ui_state.record(format!("base-entry:{id}.{pname}"), resp.rect);
        if resp.lost_focus() {
            if let Some(v) = parse_value(param, &ui_state.base_text) {
                if v != base {
                    editor.set_param(id, param.name, v);
                }
            }
            ui_state.base_text_for = None;
        } else if !resp.has_focus() && ui_state.base_text != fmt_value(param, base) {
            ui_state.base_text = fmt_value(param, base);
        }
        if ui.small_button("Default").clicked() {
            editor.set_param(id, param.name, param.default);
        }
    });

    if routes.is_empty() {
        ui.label("No modulation. Drag an output jack onto the knob to add a source.");
        return;
    }

    let base_n = param.to_norm(base);
    let (lo, hi) = combined_span(&routes, base_n);
    ui.label(format!(
        "All sources: {} – {}",
        fmt_value(param, param.from_norm(lo)),
        fmt_value(param, param.from_norm(hi))
    ));
    // Exactly one status line whether or not a source is selected, so selecting one never
    // shifts the rows below under the pointer.
    match routes
        .iter()
        .find(|r| Some(r.cable) == ui_state.selected_route)
    {
        Some(sel) => {
            let (a, b) = route_span(sel, base_n);
            ui.label(format!(
                "Selected ({}): {} – {}",
                source_label(editor.state(), sel.from_id, &sel.from_port),
                fmt_value(param, param.from_norm(a)),
                fmt_value(param, param.from_norm(b))
            ));
        }
        None => {
            ui.colored_label(
                Color32::from_rgb(255, 200, 90),
                "No source selected: click one below to edit its depth.",
            );
        }
    }
    ui.small("Ranges are computed from each source's nominal range, not measured. Block rate (64 samples).");

    let mut remove = None;
    for rt in &routes {
        let selected = Some(rt.cable) == ui_state.selected_route;
        ui.horizontal_wrapped(|ui| {
            let label = source_label(editor.state(), rt.from_id, &rt.from_port);
            let resp = ui.selectable_label(
                selected,
                // Route colours are unreadable on the selection fill.
                egui::RichText::new(label.clone()).color(if selected {
                    Color32::WHITE
                } else if rt.bypass {
                    BYPASS_GREY
                } else {
                    cable_color(rt.cable)
                }),
            );
            ui_state.record(format!("row:{}", rt.cable), resp.rect);
            if resp.clicked() {
                ui_state.selected_route = Some(rt.cable);
            }
            let mut pct = rt.amount * 100.0;
            let resp = ui.add(
                egui::DragValue::new(&mut pct)
                    .range(-100.0..=100.0)
                    .speed(0.5)
                    .suffix(" %")
                    .max_decimals(1),
            );
            ui_state.record(format!("amount:{}", rt.cable), resp.rect);
            if resp.changed() {
                let continuing = resp.dragged() && !resp.drag_started();
                editor.set_route_amount(rt.cable, pct / 100.0, !continuing);
            }
            let resp = ui.small_button("Invert");
            ui_state.record(format!("invert:{}", rt.cable), resp.rect);
            if resp.clicked() {
                editor.set_route_amount(rt.cable, -rt.amount, true);
            }
            let mut bypass = rt.bypass;
            let resp = ui.checkbox(&mut bypass, "Bypass");
            ui_state.record(format!("bypass:{}", rt.cable), resp.rect);
            if resp.changed() {
                editor.set_route_bypass(rt.cable, bypass);
            }
            let resp = ui.small_button("Remove");
            ui_state.record(format!("remove:{}", rt.cable), resp.rect);
            if resp.clicked() {
                remove = Some(rt.cable);
            }
        });
    }
    if let Some(cable) = remove {
        editor.disconnect(cable);
        if ui_state.selected_route == Some(cable) {
            ui_state.selected_route = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The label shows the semitone the sequencer plays (it rounds), never "-0 st".
    #[test]
    fn pitch_labels_match_the_played_semitone() {
        let p = kabl_modules::registry::info_for("seq").unwrap().params[0];
        for (v, want) in [
            (-0.4, "+0 st"),
            (0.4, "+0 st"),
            (6.6, "+7 st"),
            (-6.5, "-7 st"),
        ] {
            assert_eq!(fmt_value(&p, v), want);
            assert_eq!(fmt_value(&p, v), format!("{:+} st", v.round() + 0.0));
        }
    }
}
