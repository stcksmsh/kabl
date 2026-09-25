//! "What does this control change?" An explanation of one Perform card or rack control built
//! from the current patch graph every frame (never from a cached list), with navigation to
//! each target and back.
//!
//! - A direct pin explains its own parameter: help, stored base, the routes into it, a MIDI CC
//!   mapping, and whether its module reaches an Output.
//! - A macro lists the actual cables leaving that macro output: parameter routes with signed
//!   depth and bypass, signal jacks, and (one hop, bounded) what such a jack's module then
//!   moves.
//! - Show on a target inspects it with the rack's own mechanisms (`UiState::inspected`, the
//!   transient expansion of an off-face control, the flash, pan into view). Back restores the
//!   view it came from and scrolls to the originating card.
//!
//! Navigation is view state only: it never edits the patch, sends a command to the audio
//! thread or adds an undo step. An explanation is tied to the editor it was opened on
//! (`PatchEditor::instance`): opening another sound closes it.

use std::collections::{BTreeSet, VecDeque};

use egui::{Color32, RichText, Vec2 as EguiVec2};
use kabl_core::{CableId, ModuleId, PatchState, PortRef};
use kabl_modules::{registry, ParamInfo, PortDirection};

use crate::perform::{self, BANKS, CUE_PADS, PIN_PREFIX, TRANSPORT};
use crate::routing::{self, RouteView};
use crate::{help, PatchEditor, UiState};

/// Most rows one explanation lists (direct and one-hop destinations together).
pub const MAX_ROWS: usize = 24;
/// Most modules a signal-path search visits.
const MAX_VISIT: usize = 512;

/// What is being explained.
#[derive(Debug, Clone, PartialEq)]
pub enum Subject {
    /// A parameter, or a Perform card's special key (`transport`, `banks`, `cues`).
    Control {
        id: ModuleId,
        key: String,
    },
    Module(ModuleId),
}

/// A place Show navigates to.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// A parameter; `cable` is the route that reaches it, when there is one.
    Control {
        id: ModuleId,
        param: String,
        cable: Option<CableId>,
    },
    /// A signal input jack, reached by `cable`.
    Jack {
        id: ModuleId,
        port: String,
        cable: CableId,
    },
    Module(ModuleId),
}

impl Target {
    pub fn module(&self) -> ModuleId {
        match self {
            Target::Control { id, .. } | Target::Jack { id, .. } | Target::Module(id) => *id,
        }
    }

    fn exists(&self, state: &PatchState) -> bool {
        match self {
            Target::Control { id, param, cable } => {
                param_of(state, *id, param).is_some()
                    && cable.is_none_or(|c| state.cables.contains_key(&c))
            }
            Target::Jack { cable, .. } => state.cables.contains_key(cable),
            Target::Module(id) => state.modules.contains_key(id),
        }
    }
}

/// The rack view before the first Show, restored by Back.
#[derive(Debug, Clone)]
struct SavedView {
    pan: EguiVec2,
    zoom: f32,
    inspected: Option<(ModuleId, String)>,
    selected_module: Option<ModuleId>,
    selected_route: Option<CableId>,
    expanded: BTreeSet<ModuleId>,
    edit_bank: std::collections::HashMap<ModuleId, usize>,
    perform_open: bool,
}

/// Explanation panel state. View only: never saved, never undone.
#[derive(Debug, Clone, Default)]
pub struct Explain {
    pub subject: Option<Subject>,
    /// The Perform card the explanation was opened from, for Back.
    pub card: Option<(ModuleId, String)>,
    pub target: Option<Target>,
    /// The selected target disappeared (deleted, undone); said once in the panel.
    pub target_lost: bool,
    /// Inline help text for the drawer's module and control (opt-in).
    pub help: bool,
    /// Card to outline briefly after Back: (module, key, egui time).
    pub(crate) card_flash: Option<(ModuleId, String, f64)>,
    saved: Option<SavedView>,
    editor: u64,
    /// A text field had keyboard focus, or a popup or menu was open, at the end of the last
    /// frame: an Escape now is theirs.
    pub(crate) typing: bool,
    /// Show just inspected a control: the drawer scrolls to its routes once.
    pub(crate) scroll_to_routes: bool,
}

impl Explain {
    pub fn is_open(&self) -> bool {
        self.subject.is_some()
    }

    /// Closes the explanation. The view stays where it is.
    pub fn close(&mut self) {
        self.subject = None;
        self.card = None;
        self.target = None;
        self.target_lost = false;
        self.saved = None;
    }

    /// Drops references the patch no longer backs: everything when the editor was replaced
    /// (another sound opened), else a selected target that was removed.
    pub(crate) fn validate(&mut self, editor: &PatchEditor) {
        if self.editor != editor.instance() {
            // 0: no frame drawn yet, so nothing can be stale.
            if self.editor != 0 {
                self.close();
                self.card_flash = None;
            }
            self.editor = editor.instance();
            return;
        }
        if let Some(t) = &self.target {
            if !t.exists(editor.state()) {
                self.target = None;
                self.target_lost = true;
            }
        }
    }
}

/// Opens an explanation of `subject`, from Perform card `card` when given. Replaces any open
/// one; the view to go back to is kept.
pub fn open(ui_state: &mut UiState, subject: Subject, card: Option<(ModuleId, String)>) {
    let e = &mut ui_state.explain;
    e.subject = Some(subject);
    e.card = card;
    e.target = None;
    e.target_lost = false;
    ui_state.drawer_open = true;
}

/// Inspects `target` in the rack: selects it, reveals an off-face control (transiently, as
/// selecting a route does), flashes it and pans it into view. The first Show remembers the view
/// for Back.
pub fn go(editor: &PatchEditor, ui_state: &mut UiState, target: Target, now: f64) {
    if !target.exists(editor.state()) {
        return;
    }
    if ui_state.explain.saved.is_none() {
        ui_state.explain.saved = Some(SavedView {
            pan: ui_state.pan,
            zoom: ui_state.zoom,
            inspected: ui_state.inspected.clone(),
            selected_module: ui_state.selected_module,
            selected_route: ui_state.selected_route,
            expanded: ui_state.expanded.clone(),
            edit_bank: ui_state.edit_bank.clone(),
            perform_open: ui_state.perform_open,
        });
    }
    let id = target.module();
    ui_state.selected_module = Some(id);
    ui_state.reveal = Some(id);
    match &target {
        Target::Control { param, cable, .. } => {
            ui_state.inspected = Some((id, param.clone()));
            ui_state.selected_route = *cable;
            if let Some(p) = param_of(editor.state(), id, param) {
                ui_state.flash = Some((id, p.name, now));
            }
            ui_state.explain.scroll_to_routes = true;
        }
        Target::Jack { .. } | Target::Module(_) => {
            ui_state.inspected = None;
            ui_state.selected_route = None;
        }
    }
    ui_state.drawer_open = true;
    ui_state.explain.target = Some(target);
    ui_state.explain.target_lost = false;
}

/// Returns to where the explanation started: the saved view, and the originating card
/// scrolled into view and outlined.
pub fn back(ui_state: &mut UiState, now: f64) {
    if let Some(v) = ui_state.explain.saved.take() {
        ui_state.pan = v.pan;
        ui_state.zoom = v.zoom;
        ui_state.inspected = v.inspected;
        ui_state.selected_module = v.selected_module;
        ui_state.selected_route = v.selected_route;
        ui_state.expanded = v.expanded;
        ui_state.edit_bank = v.edit_bank;
        ui_state.perform_open = v.perform_open;
    }
    ui_state.explain.target = None;
    ui_state.explain.target_lost = false;
    if let Some(card) = ui_state.explain.card.clone() {
        ui_state.perform_open = true;
        ui_state.pin_reveal = Some(card.clone());
        ui_state.explain.card_flash = Some((card.0, card.1, now));
    }
}

/// `ParamInfo` of `param` on module `id`; an old patch's shared mixer `level` resolves to
/// channel 1, as the compiler does.
pub fn param_of(state: &PatchState, id: ModuleId, param: &str) -> Option<&'static ParamInfo> {
    let m = state.modules.get(&id)?;
    let info = registry::info_for(&m.kind)?;
    info.params.iter().find(|p| p.name == param).or_else(|| {
        info.params
            .iter()
            .find(|p| registry::legacy_param(info.kind, p.name) == Some(param))
    })
}

/// What one cable reaches.
#[derive(Debug, Clone)]
pub enum Reach {
    /// A modulation route into a parameter.
    Param {
        param: &'static ParamInfo,
        amount: f32,
        bypass: bool,
    },
    /// A signal input jack.
    Jack(String),
    /// A name this binary does not know (a newer patch).
    Unknown(String),
}

/// One destination: a graph fact, read from a cable.
#[derive(Debug, Clone)]
pub struct Dest {
    pub cable: CableId,
    pub to: ModuleId,
    pub reach: Reach,
    /// For a one-hop destination: the jack destination's module it passes through.
    pub via: Option<ModuleId>,
}

impl Dest {
    pub fn target(&self) -> Target {
        match &self.reach {
            Reach::Param { param, .. } => Target::Control {
                id: self.to,
                param: param.name.to_string(),
                cable: Some(self.cable),
            },
            Reach::Jack(port) => Target::Jack {
                id: self.to,
                port: port.clone(),
                cable: self.cable,
            },
            Reach::Unknown(_) => Target::Module(self.to),
        }
    }
}

/// Every cable leaving output `port` of module `from`, in cable-id order.
pub fn outgoing(state: &PatchState, from: ModuleId, port: &str) -> Vec<Dest> {
    let mut out = Vec::new();
    for (&cable, c) in &state.cables {
        let PortRef::Module { id, port: p } = &c.from else {
            continue;
        };
        if *id != from || p != port {
            continue;
        }
        let reach = match &c.to {
            PortRef::Param { id, param } => match param_of(state, *id, param) {
                Some(info) => Reach::Param {
                    param: info,
                    amount: c
                        .params
                        .get("amount")
                        .copied()
                        .unwrap_or(kabl_engine::compile::DEFAULT_ROUTE_AMOUNT),
                    bypass: c.params.get("bypass").is_some_and(|&b| b >= 0.5),
                },
                None => Reach::Unknown(param.clone()),
            },
            PortRef::Module { port, .. } => Reach::Jack(port.clone()),
        };
        out.push(Dest {
            cable,
            to: c.to.module_id(),
            reach,
            via: None,
        });
    }
    out
}

/// Destinations of output `port` of `from`, then, for each signal jack it reaches, what that
/// jack's module moves in turn (one hop). At most `MAX_ROWS`; the flag says rows were left out.
pub fn destinations(state: &PatchState, from: ModuleId, port: &str) -> (Vec<Dest>, bool) {
    let mut rows = Vec::new();
    let mut more = false;
    let mut seen = BTreeSet::new();
    for d in outgoing(state, from, port) {
        let via = matches!(d.reach, Reach::Jack(_)).then_some(d.to);
        push(&mut rows, &mut more, d);
        // One hop: a module's outputs are listed once, however many cables reach it.
        if let Some(v) = via.filter(|&v| v != from && seen.insert(v)) {
            let outs = output_ports(state, v);
            for p in outs {
                for mut n in outgoing(state, v, p) {
                    n.via = Some(v);
                    push(&mut rows, &mut more, n);
                }
            }
        }
    }
    (rows, more)
}

fn push(rows: &mut Vec<Dest>, more: &mut bool, d: Dest) {
    if rows.len() < MAX_ROWS {
        rows.push(d);
    } else {
        *more = true;
    }
}

fn output_ports(state: &PatchState, id: ModuleId) -> Vec<&'static str> {
    state
        .modules
        .get(&id)
        .and_then(|m| registry::info_for(&m.kind))
        .map(|i| {
            i.ports
                .iter()
                .filter(|p| p.direction == PortDirection::Output)
                .map(|p| p.name)
                .collect()
        })
        .unwrap_or_default()
}

/// The shortest chain of signal cables from module `id` to an `out` module, both ends
/// included, or `None` when no cable path leads to one. Breadth-first over at most
/// `MAX_VISIT` modules, so cycles and large patches stay bounded. Modulation routes (into
/// knobs) are not signal paths and are not followed.
pub fn path_to_output(
    state: &PatchState,
    id: ModuleId,
) -> Result<Option<Vec<ModuleId>>, PathLimit> {
    let mut prev = std::collections::BTreeMap::new();
    let mut queue = VecDeque::from([id]);
    prev.insert(id, id);
    while let Some(m) = queue.pop_front() {
        if state.modules.get(&m).is_some_and(|s| s.kind == "out") {
            let mut path = vec![m];
            let mut at = m;
            while at != id {
                at = prev[&at];
                path.push(at);
            }
            path.reverse();
            return Ok(Some(path));
        }
        if prev.len() >= MAX_VISIT {
            return Err(PathLimit);
        }
        for c in state.cables.values() {
            if let (PortRef::Module { id: f, .. }, PortRef::Module { id: t, .. }) = (&c.from, &c.to)
            {
                if *f == m && !prev.contains_key(t) {
                    prev.insert(*t, m);
                    queue.push_back(*t);
                }
            }
        }
    }
    Ok(None)
}

/// The signal-path search stopped at `MAX_VISIT` modules: whether a path exists is unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathLimit;

/// `Osc #2 → Filter #3 → VCA #4 → Output #5`.
pub fn path_text(state: &PatchState, path: &[ModuleId]) -> String {
    path.iter()
        .map(|id| {
            let kind = state.modules.get(id).map_or("", |m| m.kind.as_str());
            format!("{} #{id}", short(kind))
        })
        .collect::<Vec<_>>()
        .join(" → ")
}

fn short(kind: &str) -> &'static str {
    match routing::short_name(kind) {
        "?" => registry::info_for(kind).map_or("?", |i| i.name),
        s => s,
    }
}

fn module_title(state: &PatchState, id: ModuleId) -> String {
    help::module_title(state.modules.get(&id).map_or("", |m| m.kind.as_str()), id)
}

/// `+20 % of travel` for a unipolar source, `±12 %` for a bipolar one; `(inverted)` when a
/// bipolar route's amount is negative.
pub fn depth_text(r: &RouteView) -> String {
    let pct = r.amount * 100.0;
    if r.src.0 < 0.0 {
        let inv = if r.amount < 0.0 { " (inverted)" } else { "" };
        format!("±{:.0} % of travel{inv}", pct.abs())
    } else {
        format!("{:+.0} % of travel at full", pct)
    }
}

/// One route into a parameter in words, with the configured values it spans from the base.
pub fn route_line(state: &PatchState, id: ModuleId, p: &ParamInfo, r: &RouteView) -> String {
    let src = routing::source_label(state, r.from_id, &r.from_port);
    if r.bypass {
        return format!("{src}: {} · bypassed, adds nothing now", depth_text(r));
    }
    let base_n = p.to_norm(routing::base_value(state, id, p));
    let (lo, hi) = routing::route_span(r, base_n);
    let (a, b) = if r.src.0 >= 0.0 && r.amount < 0.0 {
        (base_n, lo)
    } else if r.src.0 >= 0.0 {
        (base_n, hi)
    } else {
        (lo, hi)
    };
    format!(
        "{src}: {} → {} – {}",
        depth_text(r),
        routing::fmt_value(p, p.from_norm(a)),
        routing::fmt_value(p, p.from_norm(b))
    )
}

/// For a macro source: the value its route gives at the macro's stored position, when the
/// macro knob itself has no routes (else its position is not a stored fact).
fn macro_now(state: &PatchState, id: ModuleId, p: &ParamInfo, r: &RouteView) -> Option<String> {
    let m = state.modules.get(&r.from_id)?;
    if m.kind != "macro" || r.bypass {
        return None;
    }
    let knob = param_of(state, r.from_id, &r.from_port)?;
    if !routing::routes_into(state, r.from_id, knob.name).is_empty() {
        return None;
    }
    let pos = routing::base_value(state, r.from_id, knob).clamp(0.0, 1.0);
    let base_n = p.to_norm(routing::base_value(state, id, p));
    let value = routing::fmt_value(p, p.from_norm(base_n + r.amount * pos));
    let others = routing::routes_into(state, id, p.name)
        .iter()
        .any(|o| o.cable != r.cable && !o.bypass);
    Some(if others {
        format!(
            "base + this route alone at its stored {:.0} %: {value} (the other routes move it \
             further)",
            pos * 100.0
        )
    } else {
        format!("at its stored {:.0} %: {value}", pos * 100.0)
    })
}

/// Base value versus modulation for `(id, p)`, as lines of text: what sets the base, each
/// route, the combined configured reach. Built from the patch; nothing here is measured.
pub fn base_and_modulation(state: &PatchState, id: ModuleId, p: &ParamInfo) -> Vec<String> {
    let base = routing::base_value(state, id, p);
    let mut setters = "the rack knob, the drawer's Base field".to_string();
    if perform::is_pinned(state, id, p.name) {
        setters.push_str(", its Perform card");
    }
    if let Some(m) = perform::mapping(state, id, p.name) {
        setters.push_str(&format!(" and MIDI {}", perform::cc_text(m)));
    }
    let mut lines = vec![format!(
        "Base (stored): {} — set by {setters}. Save keeps it.",
        routing::fmt_value(p, base)
    )];
    if perform::mapping(state, id, p.name).is_some() {
        lines.push(
            "The MIDI CC writes this same base value (with pickup); it is not a modulation \
             source."
                .into(),
        );
    }
    let routes = routing::routes_into(state, id, p.name);
    if routes.is_empty() {
        lines.push("Modulation: none. The control stays at its base.".into());
        return lines;
    }
    lines.push(format!(
        "Modulation: {} route{} add on top in knob travel (0–100 %); the sum is clamped to \
         the range once:",
        routes.len(),
        if routes.len() == 1 { "" } else { "s" }
    ));
    for r in &routes {
        let mut l = format!("• {}", route_line(state, id, p, r));
        if let Some(now) = macro_now(state, id, p, r) {
            l.push_str(&format!(" ({now})"));
        }
        lines.push(l);
    }
    if routes.iter().any(|r| !r.bypass) {
        let (lo, hi) = routing::combined_span(&routes, p.to_norm(base));
        lines.push(format!(
            "All active routes together can reach {} – {}. Configured from the routes, not \
             measured: the sound's actual value moves within this.",
            routing::fmt_value(p, p.from_norm(lo)),
            routing::fmt_value(p, p.from_norm(hi))
        ));
    } else {
        lines.push("Every route is bypassed: the control stays at its base.".into());
    }
    lines
}

/// `pinned as “Lead”` when the parameter has a Perform card.
fn pinned_as(state: &PatchState, id: ModuleId, param: &str) -> Option<String> {
    let key = format!("{PIN_PREFIX}{param}");
    state.modules.get(&id)?.params.get(&key)?;
    let pin = perform::Pin {
        id,
        key: param.to_string(),
        order: 0.0,
    };
    Some(format!(
        "Perform card “{}”",
        perform::pin_label(state, &pin)
    ))
}

/// A destination in words: `Ladder Filter #7 · Cutoff`, then details.
pub fn dest_title(state: &PatchState, d: &Dest) -> String {
    let what = match &d.reach {
        Reach::Param { param, .. } => routing::target_label(param),
        Reach::Jack(port) => format!("input “{port}” (signal)"),
        Reach::Unknown(name) => format!("“{name}” (unknown to this version)"),
    };
    format!("{} · {what}", module_title(state, d.to))
}

fn dest_detail(state: &PatchState, d: &Dest) -> String {
    match &d.reach {
        Reach::Param { param, .. } => {
            let Some(r) = routing::routes_into(state, d.to, param.name)
                .into_iter()
                .find(|r| r.cable == d.cable)
            else {
                return String::new();
            };
            let mut s = route_line(state, d.to, param, &r);
            // Drop the source label: the row is under its source already.
            if let Some((_, rest)) = s.split_once(": ") {
                s = rest.to_string();
            }
            if let Some(now) = macro_now(state, d.to, param, &r) {
                s.push_str(&format!(" · {now}"));
            }
            let others = routing::routes_into(state, d.to, param.name).len() - 1;
            if others > 0 {
                s.push_str(&format!(
                    " · {others} other route{} also move{} it",
                    if others == 1 { "" } else { "s" },
                    if others == 1 { "s" } else { "" }
                ));
            }
            if let Some(p) = pinned_as(state, d.to, param.name) {
                s.push_str(&format!(" · {p}"));
            }
            s
        }
        Reach::Jack(port) => {
            let kind = state.modules.get(&d.to).map_or("", |m| m.kind.as_str());
            help::port_help(kind, port).unwrap_or("").to_string()
        }
        Reach::Unknown(_) => String::new(),
    }
}

/// The drawer section. Drawn above the module and routing sections while open.
pub(crate) fn panel(editor: &PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui) {
    let Some(subject) = ui_state.explain.subject.clone() else {
        return;
    };
    let th = crate::theme::theme(ui_state.dark);
    let now = ui.input(|i| i.time);
    let state = editor.state();
    let frame = egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .stroke(egui::Stroke::new(1.5, th.cv))
        .inner_margin(egui::Margin::same(8));
    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Explain").strong().color(th.cv));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let r = ui
                    .small_button("✕")
                    .on_hover_text("Close the explanation (Esc)");
                ui_state.record("explain-close".into(), r.rect);
                if r.clicked() {
                    ui_state.explain.close();
                }
                let can_back = ui_state.explain.saved.is_some() || ui_state.explain.card.is_some();
                let back_text = match &ui_state.explain.card {
                    Some(c) => format!("← Back to “{}”", card_label(state, c)),
                    None => "← Back".into(),
                };
                let r = ui
                    .add_enabled(can_back, egui::Button::new(back_text).small())
                    .on_hover_text("Return to the control you started from");
                ui_state.record("explain-back".into(), r.rect);
                if r.clicked() {
                    back(ui_state, now);
                }
            });
        });
        if !ui_state.explain.is_open() {
            return;
        }
        if ui_state.explain.target_lost {
            ui.colored_label(
                Color32::from_rgb(230, 160, 60),
                "The target you were looking at was removed. Undo brings it back.",
            );
        }
        match &subject {
            Subject::Control { id, key } => control(editor, ui_state, ui, *id, key, now),
            Subject::Module(id) => module(editor, ui_state, ui, *id, now),
        }
    });
    ui.add_space(6.0);
}

fn card_label(state: &PatchState, (id, key): &(ModuleId, String)) -> String {
    perform::pin_label(
        state,
        &perform::Pin {
            id: *id,
            key: key.clone(),
            order: 0.0,
        },
    )
}

fn weak(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.add(egui::Label::new(RichText::new(text.into()).small().weak()).wrap());
}

fn para(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.add(egui::Label::new(text.into()).wrap());
}

/// A Show button for `target`; marked when it is the selected one.
fn show_button(
    editor: &PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    target: Target,
    hit: String,
    now: f64,
) {
    let selected = ui_state.explain.target.as_ref() == Some(&target);
    let r = ui
        .add(egui::Button::selectable(
            selected,
            if selected { "● Shown" } else { "Show" },
        ))
        .on_hover_text("Bring it into view in the rack (no change to the sound)");
    ui_state.record(hit, r.rect);
    if r.clicked() {
        go(editor, ui_state, target, now);
    }
}

fn control(
    editor: &PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    id: ModuleId,
    key: &str,
    now: f64,
) {
    let state = editor.state();
    let Some(kind) = state.modules.get(&id).map(|m| m.kind.clone()) else {
        para(
            ui,
            format!("Module #{id} is no longer in the patch. Undo brings it back."),
        );
        return;
    };
    let title = module_title(state, id);
    // Perform card name (authored) and what it is (graph).
    let pin = perform::Pin {
        id,
        key: key.to_string(),
        order: 0.0,
    };
    let card = perform::is_pinned(state, id, key).then(|| perform::pin_label(state, &pin));
    if let Some(help) = help::special_pin_help(key) {
        ui.label(RichText::new(card.unwrap_or_else(|| key.to_string())).heading());
        weak(ui, format!("Perform card for {title}"));
        para(ui, help);
        special(editor, ui_state, ui, id, key, now);
        return;
    }
    let Some(p) = param_of(state, id, key) else {
        para(ui, format!("{title} has no control “{key}” any more."));
        return;
    };
    let label = routing::target_label(p);
    let is_macro = kind == "macro";
    let display = if is_macro {
        crate::macro_name(state, id, p.name).map_or(label.clone(), |n| n)
    } else {
        card.clone().unwrap_or(label.clone())
    };
    ui.label(RichText::new(&display).heading());
    // The identity, always: a duplicated label stays distinguishable.
    weak(
        ui,
        format!(
            "{}{title} · {label} ({kind} “{}”)",
            if card.is_some() {
                "Perform card → "
            } else {
                ""
            },
            p.name
        ),
    );
    if card.is_some() && !is_macro && card.as_deref() != Some(label.as_str()) {
        weak(
            ui,
            "The card's name is a label chosen by the patch author; the line above is what it \
             actually controls.",
        );
    }
    if let Some(src) = perform::mixer_source(state, id, p.name) {
        weak(ui, format!("This mixer channel's input comes from {src}."));
    }
    let show_self = |ui_state: &mut UiState, ui: &mut egui::Ui| {
        ui.horizontal(|ui| {
            show_button(
                editor,
                ui_state,
                ui,
                Target::Control {
                    id,
                    param: p.name.to_string(),
                    cable: None,
                },
                format!("explain-show:{id}.{}", p.name),
                now,
            );
            ui.label(RichText::new(format!("{title} · {label} in the rack")).small());
        });
    };
    if is_macro {
        macro_destinations(editor, ui_state, ui, id, p.name, now);
        ui.separator();
        ui.label(RichText::new("The macro knob itself").strong());
        show_self(ui_state, ui);
    } else {
        show_self(ui_state, ui);
        if let Some(h) = help::param_help(&kind, p.name) {
            para(ui, h);
        }
    }
    weak(ui, help::range_text(&kind, p));
    for l in base_and_modulation(editor.state(), id, p) {
        weak(ui, l);
    }
    if !is_macro {
        path_line(editor.state(), ui, id);
    }
    selected_detail(editor.state(), ui_state, ui);
}

fn path_line(state: &PatchState, ui: &mut egui::Ui, id: ModuleId) {
    let kind = state.modules.get(&id).map_or("", |m| m.kind.as_str());
    if matches!(kind, "macro" | "clock" | "cues" | "out") {
        return;
    }
    match path_to_output(state, id) {
        Err(PathLimit) => weak(
            ui,
            format!(
                "Signal path (cables): not worked out, the patch is too large to search \
                 (over {MAX_VISIT} modules)."
            ),
        ),
        Ok(Some(p)) => weak(
            ui,
            format!("Signal path (cables): {}", path_text(state, &p)),
        ),
        Ok(None) => weak(
            ui,
            format!(
                "Signal path (cables): no cable path from {} to an Output. It can still \
                 affect the sound through modulation routes.",
                module_title(state, id)
            ),
        ),
    }
}

fn macro_destinations(
    editor: &PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    id: ModuleId,
    port: &str,
    now: f64,
) {
    let state = editor.state();
    let (rows, more) = destinations(state, id, port);
    let direct = rows.iter().filter(|d| d.via.is_none()).count();
    ui.separator();
    ui.label(
        RichText::new(format!(
            "What it changes ({direct} from its {} output)",
            port.to_uppercase()
        ))
        .strong(),
    );
    if rows.is_empty() {
        para(
            ui,
            "Nothing: no cable leaves this macro's output, so turning it changes nothing in \
             this patch. Drag its output jack onto a knob to give it a destination.",
        );
        return;
    }
    weak(
        ui,
        "From the patch's cables, as they are now. The macro does nothing by itself: turning \
         it up adds each route's depth to that control's own base value.",
    );
    for d in &rows {
        let indent = if d.via.is_some() { 18.0 } else { 0.0 };
        ui.horizontal(|ui| {
            ui.add_space(indent);
            show_button(
                editor,
                ui_state,
                ui,
                d.target(),
                format!("explain-dest:{}", d.cable),
                now,
            );
            let bypassed = matches!(d.reach, Reach::Param { bypass: true, .. });
            let mut t = RichText::new(dest_title(state, d));
            if bypassed {
                t = t.color(Color32::from_gray(128)).strikethrough();
            }
            ui.add(egui::Label::new(t).wrap());
        });
        let detail = dest_detail(state, d);
        if !detail.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(indent + 52.0);
                weak(ui, detail);
            });
        }
        if let Reach::Jack(_) = d.reach {
            let n = rows.iter().filter(|x| x.via == Some(d.to)).count();
            ui.horizontal(|ui| {
                ui.add_space(indent + 52.0);
                weak(
                    ui,
                    if n == 0 {
                        "That module's output goes nowhere else that is listed here.".to_string()
                    } else {
                        format!("Through {}, it then moves:", module_title(state, d.to))
                    },
                );
            });
        }
    }
    if more {
        weak(ui, format!("Only the first {MAX_ROWS} are listed."));
    }
}

/// Sequencers, LFOs and delays a clock reaches through signal cables into their clock jacks
/// (through dividers too), breadth-first over at most `MAX_VISIT` modules.
pub fn clocked(state: &PatchState, clock: ModuleId) -> Vec<ModuleId> {
    let mut seen = BTreeSet::from([clock]);
    let mut queue = VecDeque::from([clock]);
    let mut out = Vec::new();
    while let Some(m) = queue.pop_front() {
        for c in state.cables.values() {
            let (PortRef::Module { id: f, .. }, PortRef::Module { id: t, port }) = (&c.from, &c.to)
            else {
                continue;
            };
            if *f != m || seen.len() >= MAX_VISIT {
                continue;
            }
            // Only a clock input counts: a reset cable alone does not make a module clocked.
            match state.modules.get(t).map_or("", |s| s.kind.as_str()) {
                "clock.div" if port == "clock" && seen.insert(*t) => queue.push_back(*t),
                "seq" | "lfo" | "delay" if port == "clock" && seen.insert(*t) => out.push(*t),
                _ => {}
            }
        }
    }
    out.sort();
    out
}

/// Transport, bank and cue cards: what they act on, from the patch.
fn special(
    editor: &PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    id: ModuleId,
    key: &str,
    now: f64,
) {
    let state = editor.state();
    ui.horizontal(|ui| {
        show_button(
            editor,
            ui_state,
            ui,
            Target::Module(id),
            format!("explain-show:{id}"),
            now,
        );
        ui.label(RichText::new(module_title(state, id)).small());
    });
    let driven: Vec<ModuleId> = match key {
        TRANSPORT => clocked(state, id),
        CUE_PADS => crate::cues::cues(state, id)
            .into_iter()
            .flat_map(|c| c.targets.into_iter().map(|t| t.0))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        BANKS => Vec::new(),
        _ => Vec::new(),
    };
    if key == BANKS {
        path_line(state, ui, id);
    } else if driven.is_empty() {
        weak(ui, "Acts on nothing in this patch yet.");
    } else {
        ui.label(RichText::new("Acts on").strong());
        for m in driven {
            ui.horizontal(|ui| {
                show_button(
                    editor,
                    ui_state,
                    ui,
                    Target::Module(m),
                    format!("explain-mod:{m}"),
                    now,
                );
                ui.label(module_title(state, m));
            });
        }
    }
}

fn module(editor: &PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui, id: ModuleId, now: f64) {
    let state = editor.state();
    let Some(info) = state
        .modules
        .get(&id)
        .and_then(|m| registry::info_for(&m.kind))
    else {
        para(
            ui,
            format!("Module #{id} is no longer in the patch. Undo brings it back."),
        );
        return;
    };
    ui.label(RichText::new(module_title(state, id)).heading());
    weak(ui, format!("kind “{}”", info.kind));
    para(ui, info.explain);
    if let Some(n) = help::module_note(info.kind) {
        weak(ui, n);
    }
    ui.horizontal(|ui| {
        show_button(
            editor,
            ui_state,
            ui,
            Target::Module(id),
            format!("explain-show:{id}"),
            now,
        );
        ui.label(RichText::new("in the rack").small());
    });
    path_line(state, ui, id);
    // Connections, from the cables.
    let mut ins = Vec::new();
    for c in state.cables.values() {
        if let (PortRef::Module { id: f, port: fp }, PortRef::Module { id: t, port: tp }) =
            (&c.from, &c.to)
        {
            if *t == id {
                ins.push(format!("{tp} ← {}", routing::source_label(state, *f, fp)));
            }
        }
    }
    let mut outs = Vec::new();
    for p in output_ports(state, id) {
        for d in outgoing(state, id, p) {
            outs.push(format!("{p} → {}", dest_title(state, &d)));
        }
    }
    for (title, rows) in [("Inputs", ins), ("Outputs", outs)] {
        if rows.is_empty() {
            continue;
        }
        ui.label(RichText::new(title).strong());
        let n = rows.len();
        for r in rows.into_iter().take(12) {
            weak(ui, r);
        }
        if n > 12 {
            weak(ui, format!("… and {} more", n - 12));
        }
    }
    let edit = ui_state.edit_bank_of(editor.state(), id);
    let params: Vec<&ParamInfo> = info
        .params
        .iter()
        .filter(|p| crate::rack::visible(info, p.name, edit))
        .collect();
    if !params.is_empty() {
        egui::CollapsingHeader::new(format!("Controls ({})", params.len()))
            .id_salt(("explain-controls", id))
            .show(ui, |ui| {
                for p in params {
                    ui.horizontal_wrapped(|ui| {
                        let r = ui.small_button(routing::target_label(p));
                        ui_state.record(format!("explain-ctl:{id}.{}", p.name), r.rect);
                        if r.on_hover_text("Explain this control").clicked() {
                            let card = ui_state.explain.card.clone();
                            open(
                                ui_state,
                                Subject::Control {
                                    id,
                                    key: p.name.to_string(),
                                },
                                card,
                            );
                        }
                        weak(ui, help::param_help(info.kind, p.name).unwrap_or(""));
                    });
                }
            });
    }
    selected_detail(editor.state(), ui_state, ui);
}

/// What the selected target is part of: its module's signal path.
fn selected_detail(state: &PatchState, ui_state: &UiState, ui: &mut egui::Ui) {
    let Some(t) = &ui_state.explain.target else {
        return;
    };
    let id = t.module();
    let subject_module = match &ui_state.explain.subject {
        Some(Subject::Control { id, .. }) | Some(Subject::Module(id)) => *id,
        None => return,
    };
    if id == subject_module {
        return;
    }
    ui.separator();
    ui.label(RichText::new(format!("Shown: {}", module_title(state, id))).strong());
    path_line(state, ui, id);
}

/// Outlines the selected target in the rack, over the module drawing.
pub(crate) fn mark_target(
    ui_state: &UiState,
    painter: &egui::Painter,
    th: &crate::theme::Theme,
    zoom: f32,
    lay: &crate::rack::Layout,
    to_screen: impl Fn(egui::Rect) -> egui::Rect,
    port_at: impl Fn(ModuleId, &str) -> Option<egui::Pos2>,
) {
    let Some(t) = &ui_state.explain.target else {
        return;
    };
    let stroke = egui::Stroke::new(2.5 * zoom, th.cv);
    match t {
        Target::Control { id, param, .. } => {
            let Some(c) = lay.get(*id).and_then(|m| m.ctl(param)) else {
                return;
            };
            let r = to_screen(c.geo.bounds()).expand(4.0 * zoom);
            painter.rect_stroke(r, 6.0 * zoom, stroke, egui::StrokeKind::Outside);
            routing::pill(painter, r.center_top() - EguiVec2::new(0.0, 2.0), "Shown");
        }
        Target::Jack { id, port, .. } => {
            if let Some(p) = port_at(*id, port) {
                painter.circle_stroke(p, 12.0 * zoom, stroke);
                routing::pill(painter, p - EguiVec2::new(0.0, 14.0 * zoom), "Shown");
            }
        }
        Target::Module(id) => {
            if let Some(m) = lay.get(*id) {
                let r = to_screen(m.face).expand(3.0 * zoom);
                painter.rect_stroke(r, 8.0 * zoom, stroke, egui::StrokeKind::Outside);
            }
        }
    }
}
