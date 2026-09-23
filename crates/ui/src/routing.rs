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
use kabl_modules::{registry, ParamInfo, PortDirection, Taper};

use crate::{cable_color, PatchEditor, UiState};

pub const KNOB_RADIUS: f32 = 11.0;
/// Ring band (route amount handle) spans `KNOB_RADIUS + RING_INNER ..= KNOB_RADIUS + RING_OUTER`.
const RING_INNER: f32 = 3.5;
const RING_OUTER: f32 = 11.0;
const RING_DRAW: f32 = 7.0;
const DRAG_PIXELS_FOR_FULL_SWEEP: f32 = 150.0;
/// Plugs (route cable ends) sit in the 6 o'clock gap the 270° scale leaves free.
const PLUG_DROP: f32 = 7.0;
const PLUG_SPREAD: f32 = 7.0;
const MAX_PLUGS: usize = 3;
/// Spacing between per-source lanes around an inspected multi-source knob.
const LANE_GAP: f32 = 7.0;
/// Collapsed per-source rings: at most this many, this far apart, inside the ring band.
const MINI_LANES: usize = 4;
const MINI_GAP: f32 = 2.5;

const ROUTE_ACCENT: Color32 = Color32::from_rgb(90, 170, 255);
const BYPASS_GREY: Color32 = Color32::from_gray(120);

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

/// "LFO #9", or "MIDI In #1 velocity" for a non-`out` port.
pub fn source_label(state: &PatchState, from_id: ModuleId, port: &str) -> String {
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
    Some(match (kind, param) {
        ("env.adsr", "timing") => &["CONT", "KEY"],
        ("lfo", "waveform") => &["SIN", "TRI", "SAW", "SQR", "S&H"],
        ("osc.va", "waveform") => &["SIN", "TRI", "SAW", "SQR"],
        ("vca", "exponential") => &["LIN", "EXP"],
        _ => return None,
    })
}

/// Human label for a param name: `attack_ms` → `Attack`.
pub fn param_label(p: &ParamInfo) -> String {
    let base = p
        .name
        .strip_suffix(&format!("_{}", p.unit.to_lowercase()))
        .unwrap_or(p.name);
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

fn with_alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a * 255.0) as u8)
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

/// Draws and handles one continuous param knob. Returns the plug points (route cable ends) in
/// route order; routes past `MAX_PLUGS` share the last point.
#[allow(clippy::too_many_arguments)]
pub(crate) fn param_knob(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    id: ModuleId,
    param: &'static ParamInfo,
    center: Pos2,
) -> Vec<(CableId, Pos2)> {
    let base = editor
        .state()
        .modules
        .get(&id)
        .and_then(|m| m.params.get(param.name).copied())
        .unwrap_or(param.default);
    let routes = routes_into(editor.state(), id, param.name);
    let base_n = param.to_norm(base);
    let key = format!("{id}.{}", param.name);
    let inspected = ui_state.inspected.as_ref() == Some(&(id, param.name.to_string()));

    let r = KNOB_RADIUS;
    let rect = Rect::from_center_size(center, EguiVec2::splat((r + RING_OUTER) * 2.0));
    ui_state.record(
        format!("knob:{key}"),
        Rect::from_center_size(center, EguiVec2::splat(r * 1.6)),
    );
    if !routes.is_empty() {
        ui_state.record(
            format!("ring:{key}"),
            Rect::from_center_size(
                on_circle(center, r + RING_DRAW, travel_angle(0.5)),
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
        let d = resp
            .interact_pointer_pos()
            .map_or(0.0, |p| p.distance(center));
        let selected = routes
            .iter()
            .find(|rt| Some(rt.cable) == ui_state.selected_route);
        let grab = match selected {
            // Collapsed multi-source rings are display only: pressing them opens the lanes
            // (the `inspect` above) and edits nothing.
            _ if routes.len() >= 2 && !inspected && d > r + RING_INNER => None,
            Some(sel) if d > r + RING_INNER => Some((Grab::Ring(sel.cable), sel.amount)),
            None if !routes.is_empty() && d > r + RING_INNER => Some((Grab::RingNone, 0.0)),
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
    let base = editor
        .state()
        .modules
        .get(&id)
        .and_then(|m| m.params.get(param.name).copied())
        .unwrap_or(param.default);
    let base_n = param.to_norm(base);
    let routes = routes_into(editor.state(), id, param.name);

    // Collapsed knob with several sources: one thin ring per source (display only; pressing
    // opens the editable lanes). Otherwise: combined reachable range (faint), selected route's
    // own span (strong) + peak dot.
    if routes.len() >= 2 && !inspected {
        for (k, rt) in routes.iter().take(MINI_LANES).enumerate() {
            let radius = r + RING_INNER + 1.0 + MINI_GAP * k as f32;
            arc(
                painter,
                center,
                radius,
                0.0,
                1.0,
                Stroke::new(0.5, Color32::from_gray(60)),
            );
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
                lo.clamp(0.0, 1.0),
                hi.clamp(0.0, 1.0),
                Stroke::new(1.8, color),
            );
        }
    } else if !routes.is_empty() {
        let track = Stroke::new(1.0, Color32::from_gray(70));
        arc(painter, center, r + RING_DRAW, 0.0, 1.0, track);
        let active = routes.iter().any(|rt| !rt.bypass);
        if active {
            let (lo, hi) = combined_span(&routes, base_n);
            let alpha = if inspected { 0.55 } else { 0.35 };
            arc(
                painter,
                center,
                r + RING_DRAW,
                lo.clamp(0.0, 1.0),
                hi.clamp(0.0, 1.0),
                Stroke::new(3.0, with_alpha(ROUTE_ACCENT, alpha)),
            );
            for (edge, clamped) in [(0.0, lo < 0.0), (1.0, hi > 1.0)] {
                if clamped {
                    let a = travel_angle(edge);
                    painter.line_segment(
                        [
                            on_circle(center, r + RING_DRAW - 3.0, a),
                            on_circle(center, r + RING_DRAW + 3.0, a),
                        ],
                        Stroke::new(2.0, Color32::from_rgb(255, 200, 90)),
                    );
                }
            }
        } else {
            arc(
                painter,
                center,
                r + RING_DRAW,
                0.0,
                1.0,
                Stroke::new(1.5, BYPASS_GREY),
            );
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
                r + RING_DRAW,
                lo.clamp(0.0, 1.0),
                hi.clamp(0.0, 1.0),
                Stroke::new(4.0, color),
            );
            let peak = (base_n + sel.amount * sel.src.1).clamp(0.0, 1.0);
            painter.circle_filled(
                on_circle(center, r + RING_DRAW, travel_angle(peak)),
                3.0,
                color,
            );
        }
    }

    // Body: cap + pointer at the base value.
    let hot = resp.hovered() || resp.dragged() || inspected;
    painter.circle_filled(center, r, Color32::from_rgb(22, 22, 25));
    painter.circle_stroke(
        center,
        r,
        Stroke::new(1.5, Color32::from_gray(if hot { 230 } else { 170 })),
    );
    painter.line_segment(
        [center, on_circle(center, r * 0.75, travel_angle(base_n))],
        Stroke::new(2.0, Color32::WHITE),
    );

    // Plugs in the 6 o'clock gap.
    let shown = routes.len().min(MAX_PLUGS);
    let mut plugs = Vec::with_capacity(routes.len());
    for (i, rt) in routes.iter().enumerate() {
        let slot = i.min(MAX_PLUGS - 1);
        let x = (slot as f32 - (shown as f32 - 1.0) / 2.0) * PLUG_SPREAD;
        let p = center + EguiVec2::new(x, r + PLUG_DROP);
        plugs.push((rt.cable, p));
        if i < MAX_PLUGS {
            let c = if rt.bypass {
                BYPASS_GREY
            } else {
                cable_color(rt.cable)
            };
            painter.circle_filled(p, 2.5, c);
        }
    }
    if routes.len() > MAX_PLUGS {
        painter.text(
            center + EguiVec2::new(PLUG_SPREAD * 1.6, r + PLUG_DROP),
            egui::Align2::LEFT_CENTER,
            format!("+{}", routes.len() - (MAX_PLUGS - 1)),
            egui::FontId::proportional(8.0),
            Color32::from_gray(200),
        );
    }

    let label_color = if routes.is_empty() {
        Color32::from_gray(170)
    } else {
        ROUTE_ACCENT
    };
    painter.text(
        center + EguiVec2::new(0.0, r + PLUG_DROP + 5.0),
        egui::Align2::CENTER_TOP,
        param_label(param),
        egui::FontId::proportional(8.0),
        label_color,
    );

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
            center - EguiVec2::new(0.0, r + RING_OUTER + 4.0),
            &text,
        );
    }

    // Hidden cables: say where modulation comes from.
    if ui_state.cable_view == crate::CableView::Hidden && !routes.is_empty() {
        // ASCII only: the bundled fonts have no arrow glyphs.
        let badge = if routes.len() == 1 {
            let name = editor
                .state()
                .modules
                .get(&routes[0].from_id)
                .and_then(|m| registry::info_for(&m.kind))
                .map_or("?", |i| i.name);
            format!("< {name}")
        } else {
            format!("< {} mods", routes.len())
        };
        painter.text(
            center + EguiVec2::new(0.0, r + PLUG_DROP + 15.0),
            egui::Align2::CENTER_TOP,
            badge,
            egui::FontId::proportional(8.0),
            ROUTE_ACCENT,
        );
    }
    if inspected && !routes.is_empty() {
        source_lanes(editor, ui_state, ui, id, param, center, base_n, &routes);
    }
    plugs
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
    base_n: f32,
    routes: &[RouteView],
) {
    let top = ui.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        Id::new(("kabl-lanes", id, param.name)),
    ));
    let outer = lane_radius(routes.len() - 1) + LANE_GAP / 2.0;
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
            Stroke::new(if selected { 4.0 } else { 2.5 }, color),
        );
        let peak = (base_n + rt.amount * rt.src.1).clamp(0.0, 1.0);
        let at = on_circle(center, radius, travel_angle(peak));
        let hit = Rect::from_center_size(at, EguiVec2::splat(LANE_GAP + 2.0));
        ui_state.record(format!("lane:{}", rt.cable), hit);
        let resp = egui::Area::new(Id::new(("kabl-lane", rt.cable)))
            .order(egui::Order::Foreground)
            .fixed_pos(hit.min)
            .show(ui.ctx(), |ui| {
                ui.allocate_exact_size(hit.size(), Sense::click_and_drag())
                    .1
            })
            .inner;
        let hot = selected || resp.hovered() || resp.dragged();
        top.circle_filled(at, if hot { 5.0 } else { 3.5 }, color);
        if hot {
            top.circle_stroke(at, 5.5, Stroke::new(1.0, Color32::WHITE));
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

/// Radius of lane `k` around an inspected knob.
fn lane_radius(k: usize) -> f32 {
    KNOB_RADIUS + RING_OUTER + LANE_GAP * (k as f32 + 1.0)
}

pub(crate) fn pill(painter: &egui::Painter, bottom_center: Pos2, text: &str) {
    let galley = painter.layout_no_wrap(
        text.to_string(),
        egui::FontId::proportional(11.0),
        Color32::WHITE,
    );
    let size = galley.size() + EguiVec2::new(10.0, 4.0);
    let rect = Rect::from_center_size(bottom_center - EguiVec2::new(0.0, size.y / 2.0), size);
    painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(10, 10, 14, 235));
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0, ROUTE_ACCENT),
        egui::StrokeKind::Outside,
    );
    painter.galley(rect.min + EguiVec2::new(5.0, 2.0), galley, Color32::WHITE);
}

/// Segmented selector for a `Taper::Stepped` param (envelope timing, waveforms). Accepts routes
/// like a knob (its whole rect is the drop target); returns its plug point for route cables.
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
) -> Vec<(CableId, Pos2)> {
    debug_assert_eq!(param.taper, Taper::Stepped);
    let value = editor
        .state()
        .modules
        .get(&id)
        .and_then(|m| m.params.get(param.name).copied())
        .unwrap_or(param.default)
        .round();
    let n = (param.max - param.min).round() as usize + 1;
    let labels = step_labels(kind, param.name);
    let key = format!("{id}.{}", param.name);
    ui_state.record(format!("knob:{key}"), rect);
    let routes = routes_into(editor.state(), id, param.name);
    let w = rect.width() / n as f32;
    for k in 0..n {
        let opt = param.min + k as f32;
        let r = Rect::from_min_size(
            rect.min + EguiVec2::new(k as f32 * w, 0.0),
            EguiVec2::new(w, rect.height()),
        )
        .shrink(1.0);
        ui_state.record(format!("sel:{key}.{k}"), r);
        let resp = ui.interact(r, Id::new(("kabl-sel", id, param.name, k)), Sense::click());
        let on = opt == value;
        painter.rect_filled(
            r,
            2.0,
            if on {
                Color32::from_rgb(70, 110, 170)
            } else if resp.hovered() {
                Color32::from_gray(55)
            } else {
                Color32::from_gray(38)
            },
        );
        let text = labels
            .and_then(|l| l.get(k).copied())
            .map_or(format!("{k}"), str::to_string);
        painter.text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(8.0),
            Color32::WHITE,
        );
        if resp.clicked() {
            inspect(ui_state, &routes, id, param.name);
            if !on {
                editor.set_param(id, param.name, opt);
            }
        }
    }
    let plug = Pos2::new(rect.center().x, rect.max.y + 3.0);
    if !routes.is_empty() {
        painter.circle_filled(plug, 2.5, ROUTE_ACCENT);
        if ui_state.cable_view == crate::CableView::Hidden {
            painter.text(
                plug + EguiVec2::new(0.0, 3.0),
                egui::Align2::CENTER_TOP,
                format!(
                    "< {} mod{}",
                    routes.len(),
                    if routes.len() == 1 { "" } else { "s" }
                ),
                egui::FontId::proportional(8.0),
                ROUTE_ACCENT,
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
    let base = editor.state().modules[&id]
        .params
        .get(param.name)
        .copied()
        .unwrap_or(param.default);
    let routes = routes_into(editor.state(), id, param.name);

    ui.separator();
    ui.heading(format!("{} · {kind} #{id}", param_label(param)));

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
        ui.horizontal(|ui| {
            let label = source_label(editor.state(), rt.from_id, &rt.from_port);
            let resp = ui.selectable_label(
                selected,
                egui::RichText::new(label.clone()).color(if rt.bypass {
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
