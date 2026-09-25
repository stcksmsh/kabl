//! `kabl-ui`: the rack editor (brief section 12's `ui` crate). Split like `kabl-standalone` into a
//! display-independent core (`editor.rs`'s `PatchEditor`: patch editing over
//! `kabl_core::PatchLog`, testable headlessly), pure rack geometry (`rack.rs`), and the `egui`
//! rendering here and in `routing.rs`. `main.rs` wires live audio (cpal/midir) and the window.
//!
//! `show()` is testable without a display: `egui::Context` only needs `begin_pass`/`end_pass`
//! to drive a frame, so real egui input can be injected (`tests/interaction.rs`).
//!
//! Everything view-only lives in `UiState` and never touches the patch or the audio: theme,
//! zoom and pan, cable view, which modules are expanded, push vs float. The one presentation
//! choice that is saved with the patch is each module's face (primary controls), stored as
//! `face.*` params that the compiler never reads (`rack::FACE_PREFIX`).

pub mod banks;
pub mod browser;
pub mod cues;
pub mod editor;
pub mod explain;
pub mod help;
pub mod library;
pub mod perform;
pub mod rack;
pub mod record;
pub mod routing;
pub mod theme;

pub use editor::PatchEditor;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use egui::{pos2, vec2, Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke, Vec2 as EguiVec2};
use kabl_core::{CableId, ModuleId, PatchState, PortRef, Vec2};
use kabl_engine::patch_engine::Command;
use kabl_modules::builtins::{seq, DelayLock, LfoSync, Transport};
use kabl_modules::info::PortDirection;
use kabl_modules::registry;
use rack::{Decor, Geo, Layout, Placed, JACK_R, PANEL_H};
use routing::Look;
use theme::{theme, Theme};

/// Width of the routing drawer (right).
pub const DRAWER_W: f32 = 360.0;
pub const MIN_ZOOM: f32 = 0.5;
pub const MAX_ZOOM: f32 = 2.0;

/// How patch cables are drawn. Positions never change between modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CableView {
    /// Every cable.
    All,
    /// Cables touching the selected module at full strength, the rest faint.
    Focus,
    /// No cables; jacks and modulated knobs show badges instead.
    Hidden,
}

/// Colours routes cycle through by `CableId`, so each source of a multi-source knob keeps one
/// colour across its lead, lane, ring and drawer row.
const CABLE_COLORS: &[Color32] = &[
    Color32::from_rgb(220, 80, 80),
    Color32::from_rgb(80, 140, 220),
    Color32::from_rgb(90, 200, 120),
    Color32::from_rgb(230, 190, 60),
    Color32::from_rgb(180, 100, 220),
    Color32::from_rgb(230, 140, 70),
    Color32::from_rgb(80, 200, 200),
    Color32::from_rgb(230, 110, 170),
];

pub(crate) fn cable_color(id: CableId) -> Color32 {
    CABLE_COLORS[(id as usize) % CABLE_COLORS.len()]
}

/// World (rack units at 100 %) to screen.
#[derive(Clone, Copy)]
struct Xf {
    origin: Pos2,
    zoom: f32,
}

impl Xf {
    fn p(&self, w: Pos2) -> Pos2 {
        self.origin + w.to_vec2() * self.zoom
    }
    fn r(&self, r: Rect) -> Rect {
        Rect::from_min_max(self.p(r.min), self.p(r.max))
    }
    fn inv(&self, s: Pos2) -> Pos2 {
        ((s - self.origin) / self.zoom).to_pos2()
    }
}

/// Interaction and view state that is not part of the patch.
pub struct UiState {
    pub selected_kind: String,
    pub selected_module: Option<ModuleId>,
    pending_output: Option<PortRef>,
    moving: Option<Moving>,
    /// The sound library (`None` until `main.rs` opens it, or in tests that don't need it).
    pub library: Option<library::Library>,
    /// The open document; set from the editor on the first frame when `None`.
    pub doc: Option<browser::Doc>,
    pub browser: browser::Browser,
    /// The sound browser (left panel) is open.
    pub browser_open: bool,
    /// Every question answered: `main.rs` closes the window.
    pub quit_now: bool,
    /// The last Load came from the browser and is a piece: its clocks start stopped.
    pub load_stopped: bool,
    /// Output sample rate, for preview lengths (set by `main.rs`).
    pub sample_rate: f32,
    pub last_message: Option<String>,
    pub cable_view: CableView,
    /// The knob (module, param) whose routes the drawer lists and whose ring edits
    /// `selected_route`.
    pub inspected: Option<(ModuleId, String)>,
    /// The route the ring and the drawer's highlight refer to. Never re-pointed silently: it is
    /// cleared when that route disappears, and only set by an explicit pick or by inspecting a
    /// knob that has exactly one route.
    pub selected_route: Option<CableId>,
    /// Output jack being dragged toward a knob or input.
    port_drag: Option<PortRef>,
    /// Jack cable whose plug is being pulled out of its input (moved or removed on release).
    unplug: Option<CableId>,
    /// Knob body / ring / lane-dot drag in progress.
    pub(crate) drag: Option<routing::DragGrab>,
    pub(crate) base_text: String,
    pub(crate) base_text_for: Option<(ModuleId, String)>,
    /// Screen rects of interactive targets drawn last frame (`knob:4.attack_ms`,
    /// `ring:4.attack_ms`, `out:9.out`, `in:5.cv`, `sel:4.timing.1`, `row:12`, `toggle:4`,
    /// `pin:4.timing`, `module:4`, ...), for headless interaction tests and scripted real-input
    /// runs.
    pub hits: BTreeMap<String, Rect>,
    frame_hits: BTreeMap<String, Rect>,
    /// Decoded skin art, keyed by (module kind, dark variant); the handle keeps the texture alive.
    image_cache: HashMap<(&'static str, bool), egui::TextureHandle>,
    /// Each sequencer's playing step (0-based), fed by `main.rs` from the audio thread.
    pub seq_steps: HashMap<ModuleId, usize>,
    /// Each sequencer's playing bank and queued bank (a pending launch or an arm), fed by
    /// `main.rs` from the audio thread.
    pub seq_banks: HashMap<ModuleId, (usize, Option<usize>)>,
    /// Edit bank the user picked per sequencer. View only: never saved, never undone. Without
    /// a pick, a sequencer edits the bank it plays.
    pub edit_bank: HashMap<ModuleId, usize>,
    /// Every sequencer's effective edit bank, rebuilt each frame for the layout.
    edit_view: HashMap<ModuleId, usize>,
    /// Bank launches and cancels for `main.rs` to send to the audio thread (runtime only).
    pub launches: Vec<Command>,
    /// Last state of each MIDI button (channel, CC): high = pressed.
    pub(crate) button_high: HashMap<(u8, u8), bool>,
    /// Egui time before which button presses only set state (after a (re)connect).
    pub(crate) button_guard: f64,
    /// Set by `main.rs` on MIDI (re)connect: forget button states and start the guard.
    pub button_rearm: bool,
    /// Cue (or macro, index 100 + k) name being typed: (module, cue number, text).
    cue_rename: Option<(ModuleId, usize, String)>,
    /// Bank name being typed: (sequencer, bank, text).
    bank_rename: Option<(ModuleId, usize, String)>,
    /// The inspected control whose bank was last shown.
    bank_revealed: Option<(ModuleId, String)>,
    /// Each clock's run state as the audio thread last reported it; absent means running.
    pub clock_running: HashMap<ModuleId, bool>,
    /// Transport commands for `main.rs` to send to the audio thread. Runtime only: never in the
    /// op log, so undo and reload can't replay them.
    pub transport: Vec<(ModuleId, Transport)>,
    /// Each LFO's sync state as the audio thread last reported it.
    pub lfo_status: HashMap<ModuleId, LfoSync>,
    /// Each delay's lock state and target time as the audio thread last reported it.
    pub delay_status: HashMap<ModuleId, (DelayLock, f32)>,
    /// The last rebuild request came from Load: `main.rs` sends that graph fresh (no state
    /// carry), then clears this.
    pub loaded: bool,
    /// A-dark when true, A-light otherwise.
    pub dark: bool,
    pub zoom: f32,
    /// Screen offset of the rack origin from the canvas' top-left.
    pub pan: EguiVec2,
    pub drawer_open: bool,
    /// Modules showing their advanced controls. View only: never saved, never undone.
    pub expanded: BTreeSet<ModuleId>,
    /// Advanced controls float over the neighbours instead of pushing them (setting).
    pub float_expansion: bool,
    /// Show illustrated skins where a module has one (core modules stay A / A-dark otherwise).
    pub skins: bool,
    /// Viewer preview of a skin's `labels_on_art` flag; `None` = the skin's own setting.
    pub skin_labels_on_art: Option<bool>,
    /// Module in choose-primary mode and its pending face choice (committed by `Done`).
    pub choose: Option<(ModuleId, Vec<bool>)>,
    flash: Option<(ModuleId, &'static str, f64)>,
    /// Module to pan into view after the next layout (just expanded or revealed).
    reveal: Option<ModuleId>,
    canvas: Rect,
    fitted: bool,
    last_inspected: Option<(ModuleId, String)>,
    /// Inspected knob and route count whose source lanes were last panned into view.
    lanes_shown: Option<(ModuleId, String, usize)>,
    /// Values, pills and badges: drawn after the cables so no cable hides them.
    pub(crate) deferred: Vec<egui::Shape>,
    /// The performance panel (bottom) is open.
    pub perform_open: bool,
    /// Param waiting for the next MIDI CC to map to it.
    pub learn: Option<(ModuleId, String)>,
    pub takeover: perform::TakeoverMap,
    /// The performance panel shows more rows (taller), at the rack's expense.
    pub perform_tall: bool,
    /// Pin being renamed: (module, pin key, text so far).
    pub renaming: Option<(ModuleId, String, String)>,
    /// A MIDI input notice for the panel ("disconnected", "reconnected").
    pub midi_note: Option<String>,
    /// The user asked to release every MIDI voice; `main.rs` sends the note-offs.
    pub all_notes_off: bool,
    /// Pin card to scroll into view on the next frame (just pinned or moved).
    pub pin_reveal: Option<(ModuleId, String)>,
    /// The mapping a CC gesture is on and when its last message came (seconds, egui time).
    pub(crate) cc_gesture: Option<((ModuleId, String), f64)>,
    /// Incoming MIDI CC `(channel, controller, value)` for the next frame, fed by `main.rs`.
    pub midi_cc: Vec<(u8, u8, u8)>,
    /// MIDI input ports seen, the connected one, and a switch the user asked for.
    pub midi_inputs: Vec<String>,
    pub midi_input: Option<String>,
    /// `Some(None)` = disconnect.
    pub midi_select: Option<Option<String>>,
    /// Final output meter (fed by `main.rs` from the audio callback).
    pub meter: record::Meter,
    /// Stereo output recorder; `None` without an audio device.
    pub recorder: Option<record::Recorder>,
    /// The open "what does this control change" explanation and inline help (view only).
    pub explain: explain::Explain,
}

struct Moving {
    id: ModuleId,
    /// Pointer minus module origin, in world units, at the press.
    grab: EguiVec2,
    live: Vec2,
    start: Vec2,
}

impl Default for UiState {
    fn default() -> Self {
        UiState {
            selected_kind: PatchEditor::known_kinds()
                .first()
                .copied()
                .unwrap_or("osc.va")
                .to_string(),
            selected_module: None,
            pending_output: None,
            moving: None,
            library: None,
            doc: None,
            browser: Default::default(),
            browser_open: false,
            quit_now: false,
            load_stopped: false,
            sample_rate: 48000.0,
            last_message: None,
            cable_view: CableView::All,
            inspected: None,
            selected_route: None,
            port_drag: None,
            unplug: None,
            drag: None,
            base_text: String::new(),
            base_text_for: None,
            hits: Default::default(),
            frame_hits: Default::default(),
            image_cache: HashMap::new(),
            seq_steps: HashMap::new(),
            seq_banks: HashMap::new(),
            edit_bank: HashMap::new(),
            edit_view: HashMap::new(),
            launches: Vec::new(),
            bank_rename: None,
            cue_rename: None,
            button_high: HashMap::new(),
            button_guard: 0.0,
            button_rearm: true,
            bank_revealed: None,
            clock_running: HashMap::new(),
            transport: Vec::new(),
            delay_status: HashMap::new(),
            lfo_status: HashMap::new(),
            loaded: false,
            dark: false,
            zoom: 1.0,
            pan: EguiVec2::ZERO,
            drawer_open: true,
            expanded: BTreeSet::new(),
            float_expansion: false,
            skins: true,
            skin_labels_on_art: None,
            choose: None,
            flash: None,
            reveal: None,
            canvas: Rect::NOTHING,
            fitted: false,
            last_inspected: None,
            lanes_shown: None,
            deferred: Vec::new(),
            perform_open: false,
            learn: None,
            takeover: Default::default(),
            pin_reveal: None,
            renaming: None,
            perform_tall: false,
            midi_note: None,
            all_notes_off: false,
            cc_gesture: None,
            midi_cc: Vec::new(),
            midi_inputs: Vec::new(),
            midi_input: None,
            midi_select: None,
            recorder: None,
            meter: Default::default(),
            explain: Default::default(),
        }
    }
}

impl UiState {
    pub(crate) fn record(&mut self, key: String, rect: Rect) {
        self.frame_hits.insert(key, rect);
    }

    /// Drops selections that point at things undo, delete or load removed.
    fn validate(&mut self, editor: &PatchEditor) {
        let state = editor.state();
        if let Some((id, param)) = &self.inspected {
            let exists = state
                .modules
                .get(id)
                .and_then(|m| registry::info_for(&m.kind))
                .is_some_and(|i| i.params.iter().any(|p| p.name == param));
            if !exists {
                self.inspected = None;
            }
        }
        if let Some(cable) = self.selected_route {
            let belongs = state.cables.get(&cable).is_some_and(|c| {
                matches!((&c.to, &self.inspected),
                    (PortRef::Param { id, param }, Some((iid, ip))) if id == iid && param == ip)
            });
            if !belongs {
                self.selected_route = None;
            }
        }
        if self
            .selected_module
            .is_some_and(|id| !state.modules.contains_key(&id))
        {
            self.selected_module = None;
        }
        self.expanded.retain(|id| state.modules.contains_key(id));
        if self
            .learn
            .as_ref()
            .is_some_and(|(id, _)| !state.modules.contains_key(id))
        {
            self.learn = None;
        }
        if self
            .choose
            .as_ref()
            .is_some_and(|(id, _)| !state.modules.contains_key(id))
        {
            self.choose = None;
        }
    }

    fn xf(&self) -> Xf {
        Xf {
            origin: self.canvas.min + self.pan,
            zoom: self.zoom,
        }
    }

    fn zoom_about(&mut self, zoom: f32, at: Pos2) {
        let world = self.xf().inv(at);
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan = at - self.canvas.min - world.to_vec2() * self.zoom;
    }

    /// Zooms so `world` fits the canvas (at most `max_zoom`), left-aligned, centred vertically
    /// when there is room.
    fn frame_world(&mut self, world: Rect, max_zoom: f32) {
        let c = self.canvas.shrink(12.0);
        let z = (c.width() / world.width())
            .min(c.height() / world.height())
            .clamp(MIN_ZOOM, max_zoom);
        self.zoom = z;
        self.pan = c.min - self.canvas.min - world.min.to_vec2() * z;
        let spare = c.height() - world.height() * z;
        if spare > 0.0 {
            self.pan.y += spare / 2.0;
        }
    }

    /// Pans the least needed to bring the screen rect `target` inside the canvas (its
    /// top-left corner first when it cannot fit whole).
    fn keep_visible(&mut self, target: Rect) {
        let c = self.canvas.shrink(8.0);
        let mut d = EguiVec2::ZERO;
        if target.right() > c.right() {
            d.x = c.right() - target.right();
        }
        if target.left() + d.x < c.left() {
            d.x = c.left() - target.left();
        }
        if target.bottom() > c.bottom() {
            d.y = c.bottom() - target.bottom();
        }
        if target.top() + d.y < c.top() {
            d.y = c.top() - target.top();
        }
        self.pan += d;
    }

    fn view(&self) -> rack::View<'_> {
        rack::View {
            expanded: Some(&self.expanded),
            float: self.float_expansion,
            skins: self.skins,
            choose: self.choose.as_ref().map(|(id, set)| (*id, set.as_slice())),
            moving: self.moving.as_ref().map(|m| (m.id, m.live)),
            edit_banks: Some(&self.edit_view),
        }
    }

    /// The bank a sequencer's face edits: the user's pick, else the playing bank, else the
    /// startup bank.
    pub fn edit_bank_of(&self, state: &PatchState, id: ModuleId) -> usize {
        self.edit_bank
            .get(&id)
            .or(self.seq_banks.get(&id).map(|b| &b.0))
            .copied()
            .unwrap_or_else(|| banks::startup(state, id))
    }

    fn flash_on(&self, id: ModuleId, param: &str, now: f64) -> bool {
        matches!(self.flash, Some((fid, p, t)) if fid == id && p == param && now - t < 1.5)
    }
}

/// Draws the rack editor for one frame and applies user edits to `editor` directly. Call once
/// per frame from `eframe::App::ui`.
pub fn show(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui) {
    browser::frame_input(editor, ui_state, ui);
    ui_state.validate(editor);
    ui_state.explain.validate(editor);
    // Inspecting a control in another bank (a route, a pin, a CC mapping) shows that bank on
    // the face. It never launches it.
    if ui_state.inspected != ui_state.bank_revealed {
        ui_state.bank_revealed = ui_state.inspected.clone();
        if let Some((id, param)) = &ui_state.inspected {
            let is_seq = editor
                .state()
                .modules
                .get(id)
                .is_some_and(|m| m.kind == "seq");
            if let Some((b, _)) = seq::bank_of(param).filter(|_| is_seq) {
                ui_state.edit_bank.insert(*id, b);
            }
        }
    }
    ui_state.edit_view = editor
        .state()
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "seq")
        .map(|(&id, _)| (id, ui_state.edit_bank_of(editor.state(), id)))
        .collect();
    let th = theme(ui_state.dark);
    ui.ctx().set_visuals(th.visuals());
    // Escape mid-drag cancels it: revert the gesture's edit and leave no undo entry. The drag
    // stays captured (and inert) until the button is released. Otherwise Escape leaves choose
    // mode without changing the face.
    let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
    if let Some(g) = ui_state.drag.as_mut() {
        if !g.cancelled && esc {
            if g.committed {
                editor.cancel_gesture();
            }
            g.cancelled = true;
        }
    } else if esc && ui_state.choose.is_some() {
        ui_state.choose = None;
    } else if esc && ui_state.learn.is_some() {
        ui_state.learn = None;
    } else if esc
        && ui_state.explain.is_open()
        && ui_state.browser.dialog.is_none()
        && !ui.ctx().egui_wants_keyboard_input()
    {
        ui_state.explain.close();
    }
    let now = ui.input(|i| i.time);
    perform::apply_cc(editor, ui_state, now);
    perform::sync_takeover(editor, ui_state);
    if !ui.input(|i| i.pointer.any_down()) {
        ui_state.drag = None;
    }
    if !ui.ctx().egui_wants_keyboard_input() && ui_state.browser.dialog.is_none() {
        let (undo, redo) = ui.input_mut(|i| {
            let redo = i.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::Z,
            )) || i.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::Y,
            ));
            let undo = i.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::Z,
            ));
            (undo, redo)
        });
        if undo {
            editor.undo();
        }
        if redo {
            editor.redo();
        }
    }
    egui::Panel::top("kabl-toolbar")
        .exact_size(40.0)
        .show(ui, |ui| toolbar(editor, ui_state, ui));
    if ui_state.browser_open {
        egui::Panel::left("kabl-browser")
            .exact_size(browser::PANEL_W)
            .resizable(false)
            .show(ui, |ui| browser::panel(editor, ui_state, ui));
    }

    if ui_state.perform_open {
        egui::Panel::bottom("kabl-perform")
            .exact_size(if ui_state.perform_tall {
                perform::PANEL_TALL_H
            } else {
                perform::PANEL_H
            })
            .resizable(false)
            .show(ui, |ui| perform::panel(editor, ui_state, ui));
    }
    if ui_state.drawer_open {
        egui::Panel::right("kabl-params")
            .exact_size(DRAWER_W)
            .resizable(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Routing");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            ui_state.drawer_open = false;
                        }
                        let on = ui_state.explain.help;
                        let r = ui
                            .add(egui::Button::selectable(on, "? Help"))
                            .on_hover_text("Show what each module and control does");
                        ui_state.record("help".into(), r.rect);
                        if r.clicked() {
                            ui_state.explain.help = !on;
                        }
                    });
                });
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // Rows wrap instead of widening the drawer over the rack.
                    ui.set_max_width(DRAWER_W - 24.0);
                    explain::panel(editor, ui_state, ui);
                    show_param_panel(editor, ui_state, ui);
                    routing::drawer(editor, ui_state, ui);
                });
            });
    }

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(th.rack))
        .show(ui, |ui| show_rack(editor, ui_state, ui, &th));
    browser::dialogs(editor, ui_state, ui.ctx());
    ui_state.hits = std::mem::take(&mut ui_state.frame_hits);
}

fn tool(ui: &mut egui::Ui, ui_state: &mut UiState, key: &str, label: &str, on: bool) -> bool {
    let r = ui.add(egui::Button::selectable(on, label));
    ui_state.record(key.to_string(), r.rect);
    r.clicked()
}

fn toolbar(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui) {
    ui.horizontal_centered(|ui| {
        ui.label(egui::RichText::new("kabl").strong().size(17.0));
        let open = ui_state.browser_open;
        if tool(ui, ui_state, "browser", "Sounds", open) {
            ui_state.browser_open = !open;
        }
        egui::ComboBox::from_id_salt("kind")
            .width(90.0)
            .selected_text(ui_state.selected_kind.clone())
            .show_ui(ui, |ui| {
                for kind in PatchEditor::known_kinds() {
                    ui.selectable_value(&mut ui_state.selected_kind, kind.to_string(), *kind);
                }
            });
        if tool(ui, ui_state, "add", "Add", false) {
            // At the end of the first row.
            let lay = rack::layout(editor.state(), &ui_state.view());
            let x = lay
                .mods
                .iter()
                .filter(|m| m.row == 0)
                .map(|m| m.rect.right())
                .fold(rack::RACK_X, f32::max);
            let id = editor.add_module(&ui_state.selected_kind, Vec2 { x, y: rack::ROW_Y0 });
            ui_state.selected_module = Some(id);
            ui_state.reveal = Some(id);
        }
        ui.separator();
        if ui
            .add_enabled(editor.can_undo(), egui::Button::new("Undo"))
            .clicked()
        {
            editor.undo();
        }
        if ui
            .add_enabled(editor.can_redo(), egui::Button::new("Redo"))
            .clicked()
        {
            editor.redo();
        }
        ui.separator();
        ui.label("Cables");
        for (mode, label) in [
            (CableView::All, "All"),
            (CableView::Focus, "Focus"),
            (CableView::Hidden, "Hidden"),
        ] {
            if tool(
                ui,
                ui_state,
                &format!("view:{label}"),
                label,
                ui_state.cable_view == mode,
            ) {
                ui_state.cable_view = mode;
            }
        }
        ui.separator();
        let c = ui_state.canvas.center();
        if tool(ui, ui_state, "zoom:out", "−", false) {
            ui_state.zoom_about(ui_state.zoom / 1.2, c);
        }
        let pct = format!("{:.0}%", ui_state.zoom * 100.0);
        if tool(ui, ui_state, "zoom:100", &pct, false) {
            ui_state.zoom_about(1.0, c);
        }
        if tool(ui, ui_state, "zoom:in", "+", false) {
            ui_state.zoom_about(ui_state.zoom * 1.2, c);
        }
        if tool(ui, ui_state, "zoom:fit", "Fit", false) {
            let lay = rack::layout(editor.state(), &ui_state.view());
            ui_state.frame_world(lay.bounds.expand(4.0), 1.5);
        }
        let focus_target = ui_state
            .inspected
            .as_ref()
            .map(|(id, _)| *id)
            .or(ui_state.selected_module);
        let r = ui.add_enabled(focus_target.is_some(), egui::Button::new("Focus"));
        ui_state.record("zoom:focus".into(), r.rect);
        if r.on_hover_text("Zoom to the selected module").clicked() {
            let lay = rack::layout(editor.state(), &ui_state.view());
            if let Some(world) = focus_target.and_then(|id| lay.get(id)).map(|m| m.full()) {
                // Readable: at least 100 % unless the module is wider than the canvas.
                let fit = (ui_state.canvas.width() - 48.0) / world.width();
                ui_state.frame_world(
                    world.expand(24.0),
                    fit.clamp(MIN_ZOOM, 1.25).max(fit.min(1.0)),
                );
                let d = ui_state.canvas.center() - ui_state.xf().r(world).center();
                ui_state.pan += d;
            }
        }
        ui.separator();
        if tool(ui, ui_state, "theme:light", "A-light", !ui_state.dark) {
            ui_state.dark = false;
        }
        if tool(ui, ui_state, "theme:dark", "A-dark", ui_state.dark) {
            ui_state.dark = true;
        }
        ui.separator();
        let menu = ui.menu_button("View", |ui| {
            ui.label("Expanded modules");
            let r = ui.radio_value(&mut ui_state.float_expansion, false, "Push neighbours");
            ui_state.record("menu:push".into(), r.rect);
            let r = ui.radio_value(&mut ui_state.float_expansion, true, "Float over neighbours");
            ui_state.record("menu:float".into(), r.rect);
            ui.separator();
            let r = ui.checkbox(&mut ui_state.skins, "Illustrated skins");
            ui_state.record("menu:skins".into(), r.rect);
            ui.add_enabled_ui(ui_state.skins, |ui| {
                let mut on_art = ui_state.skin_labels_on_art.unwrap_or(false);
                let r = ui
                    .checkbox(&mut on_art, "Preview labels_on_art")
                    .on_hover_text("A skin maker's flag; off = theme plates (default)");
                ui_state.record("menu:labels-on-art".into(), r.rect);
                if r.changed() {
                    ui_state.skin_labels_on_art = on_art.then_some(true);
                }
            });
        });
        ui_state.record("view-menu".into(), menu.response.rect);
        ui.separator();
        browser::toolbar(editor, ui_state, ui);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let open = ui_state.drawer_open;
            if tool(ui, ui_state, "routing", "Routing", open) {
                ui_state.drawer_open = !open;
            }
            let open = ui_state.perform_open;
            if tool(ui, ui_state, "perform", "Perform", open) {
                ui_state.perform_open = !open;
            }

            if let Some(rec) = ui_state.recorder.as_ref().filter(|r| r.recording()) {
                let t = rec.elapsed() as u64;
                ui.label(
                    egui::RichText::new(format!("● REC {:02}:{:02}", t / 60, t % 60))
                        .color(Color32::from_rgb(220, 60, 50))
                        .strong(),
                );
            }
            if let Some(msg) = &ui_state.last_message {
                // Truncated to the room left, never over the tools.
                ui.add(egui::Label::new(egui::RichText::new(msg).small()).truncate())
                    .on_hover_text(msg);
            }
        });
    });
}

fn show_param_panel(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui) {
    let Some(id) = ui_state.selected_module else {
        ui.label("No module selected.");
        return;
    };
    let Some(kind) = editor.state().modules.get(&id).map(|m| m.kind.clone()) else {
        ui.label("No module selected.");
        return;
    };
    let Some(info) = registry::info_for(&kind) else {
        ui.label("Unknown module kind.");
        return;
    };
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("{} #{id}", info.name)).strong());
        let r = ui
            .small_button("Explain")
            .on_hover_text("What this module does and what it is connected to");
        ui_state.record(format!("explain-module:{id}"), r.rect);
        if r.clicked() {
            explain::open(ui_state, explain::Subject::Module(id), None);
        }
    });
    if ui_state.explain.help {
        ui.add(egui::Label::new(egui::RichText::new(info.explain).small()).wrap());
    }
    let edit = ui_state.edit_bank_of(editor.state(), id);
    if kind == "seq" {
        seq_panel(editor, ui_state, ui, id, edit);
    }
    if kind == "cues" {
        cues_panel(editor, ui_state, ui, id);
    }
    if kind == "macro" {
        for (k, p) in info.params.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("M{} name", k + 1));
                let key = format!("name.{}", p.name);
                let stored = editor.state().label(id, &key).unwrap_or("").to_string();
                let mut t = ui_state
                    .cue_rename
                    .as_ref()
                    .filter(|(i, n, _)| (*i, *n) == (id, 100 + k))
                    .map_or(stored.clone(), |(_, _, t)| t.clone());
                let r = ui.add(egui::TextEdit::singleline(&mut t).desired_width(120.0));
                ui_state.record(format!("macro-name:{id}.{}", p.name), r.rect);
                if r.changed() {
                    ui_state.cue_rename = Some((id, 100 + k, t.clone()));
                }
                if r.lost_focus() {
                    ui_state.cue_rename = None;
                    let t = t.trim();
                    editor.set_label(id, &key, (!t.is_empty()).then(|| t.to_string()));
                }
            });
        }
    }
    for param in info
        .params
        .iter()
        .filter(|p| rack::visible(info, p.name, edit))
    {
        perform::param_editor(
            editor,
            ui_state,
            ui,
            id,
            param,
            true,
            Some(format!("dparam:{id}.{}", param.name)),
        );
        if let Some(m) = perform::mapping(editor.state(), id, param.name) {
            ui.label(egui::RichText::new(perform::cc_text(m)).small().monospace());
        }
        if ui_state.explain.help {
            if let Some(h) = help::param_help(&kind, param.name) {
                ui.add(egui::Label::new(egui::RichText::new(h).small().weak()).wrap());
            }
        }
    }
    if ui.button("Remove module").clicked() {
        editor.remove_module(id);
    }
}

/// The drawer's sequencer header: which bank the controls below edit, bank edits, and the
/// sequencer's own launch settings.
fn seq_panel(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    id: ModuleId,
    edit: usize,
) {
    let state = editor.state();
    let (playing, queued) = ui_state
        .seq_banks
        .get(&id)
        .copied()
        .unwrap_or((banks::startup(state, id), None));
    ui.horizontal_wrapped(|ui| {
        ui.label("Editing");
        for b in 0..seq::BANKS {
            let r = ui.selectable_label(b == edit, banks::title(state, id, b));
            ui_state.record(format!("drawer-bank:{id}.{b}"), r.rect);
            if r.clicked() {
                ui_state.edit_bank.insert(id, b);
            }
        }
    });
    ui.label(
        egui::RichText::new(format!(
            "Playing {}{}{}",
            banks::title(state, id, playing),
            queued.map_or(String::new(), |q| format!(
                " · queued {}",
                banks::title(state, id, q)
            )),
            if banks::startup(state, id) == playing {
                " · startup".to_string()
            } else {
                format!(
                    " · startup {}",
                    banks::title(state, id, banks::startup(state, id))
                )
            }
        ))
        .small(),
    );
    ui.menu_button(format!("Bank {} ▾", seq::BANK_NAMES[edit]), |ui| {
        bank_menu(editor, ui_state, ui, id, edit)
    });
    let (mut pick, mut tpick) = (None, None);
    ui.horizontal_wrapped(|ui| {
        let state = editor.state();
        ui.label("Launch on");
        let clocks: Vec<ModuleId> = state
            .modules
            .iter()
            .filter(|(_, m)| m.kind == "clock")
            .map(|(&c, _)| c)
            .collect();
        let chosen = state
            .modules
            .get(&id)
            .and_then(|m| m.params.get(banks::LAUNCH_CLOCK))
            .map(|&v| v as ModuleId);
        let current = banks::reference_clock(state, id);
        let text = match (current, chosen) {
            (Some(c), Some(_)) => format!("Clock #{c}"),
            (Some(c), None) => format!("Clock #{c} (patched)"),
            (None, _) => "no clock".into(),
        };
        egui::ComboBox::from_id_salt(("seq-clock", id))
            .selected_text(text)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(chosen.is_none(), "The patched clock")
                    .clicked()
                {
                    pick = Some(None);
                }
                for c in clocks {
                    if ui
                        .selectable_label(chosen == Some(c), format!("Clock #{c}"))
                        .clicked()
                    {
                        pick = Some(Some(c as f32));
                    }
                }
            });
        let timing = banks::timing(state, id);
        egui::ComboBox::from_id_salt(("seq-timing", id))
            .selected_text(
                banks::TIMINGS
                    .iter()
                    .find(|t| t.0 == timing)
                    .map_or("", |t| t.1),
            )
            .show_ui(ui, |ui| {
                for (i, (t, name)) in banks::TIMINGS.iter().enumerate() {
                    if ui.selectable_label(*t == timing, *name).clicked() {
                        tpick = Some(i);
                    }
                }
            });
    });
    if let Some(v) = pick {
        editor.set_presentation(&[(id, banks::LAUNCH_CLOCK.into(), v)]);
    }
    if let Some(i) = tpick {
        let v = (i != 2).then_some(i as f32);
        editor.set_presentation(&[(id, banks::LAUNCH_TIMING.into(), v)]);
    }
    ui.separator();
}

/// A macro knob's name (label `name.m1`), for its knob, jack, pins and route sources.
pub fn macro_name(state: &PatchState, id: ModuleId, param: &str) -> Option<String> {
    (state.modules.get(&id)?.kind == "macro")
        .then(|| state.label(id, &format!("name.{param}")))
        .flatten()
        .map(str::to_string)
}

/// The skin's art for this theme: its dark variant, else the light art (dimmed under A-dark).
fn skin_texture(
    ui: &egui::Ui,
    cache: &mut HashMap<(&'static str, bool), egui::TextureHandle>,
    kind: &'static str,
    skin: &kabl_modules::skin::ModuleSkin,
    dark: bool,
) -> Option<(egui::TextureId, Color32)> {
    let (bytes, variant, tint) = match (dark, skin.background_dark, skin.background_image) {
        (true, Some(d), _) => (d, true, Color32::WHITE),
        (true, None, Some(l)) => (l, false, Color32::from_gray(166)),
        (_, _, Some(l)) => (l, false, Color32::WHITE),
        _ => return None,
    };
    let handle = cache.entry((kind, variant)).or_insert_with(|| {
        let decoded = image::load_from_memory(bytes)
            .expect("skin art must be a valid, embedded PNG")
            .to_rgba8();
        let size = [decoded.width() as usize, decoded.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, decoded.as_raw());
        ui.ctx().load_texture(
            format!("{kind}-{variant}"),
            color_image,
            egui::TextureOptions::LINEAR,
        )
    });
    Some((handle.id(), tint))
}

/// World rect of the inspected control plus, for a knob, its source lanes; keyed by the control
/// and its route count.
fn inspected_extent(
    editor: &PatchEditor,
    ui_state: &UiState,
    lay: &Layout,
) -> Option<(Rect, (ModuleId, String, usize))> {
    let (id, p) = ui_state.inspected.as_ref()?;
    let geo = lay.get(*id)?.ctl(p)?.geo;
    let n = routing::routes_into(editor.state(), *id, p).len();
    let r = match geo {
        Geo::Knob { c, r } if n > 0 => geo.bounds().union(Rect::from_center_size(
            c,
            EguiVec2::splat(2.0 * routing::lanes_outer(r, n)),
        )),
        _ => geo.bounds(),
    };
    Some((r, (*id, p.clone(), n)))
}

/// What the modules drew this frame, for cables and drop targets.
#[derive(Default)]
struct Drawn {
    /// Jack centres by (module, direction, port).
    ports: HashMap<(ModuleId, PortDirection, String), Pos2>,
    /// Route cable ends (knob plugs, or the `+N` button of a collapsed module).
    plugs: Vec<(CableId, Pos2)>,
    /// Output-jack drag released this frame: (source, pointer position).
    drop_at: Option<(PortRef, Pos2)>,
}

fn show_rack(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui, th: &Theme) {
    let canvas = ui.max_rect();
    let now = ui.input(|i| i.time);
    let lay = rack::layout(editor.state(), &ui_state.view());
    if !ui_state.fitted {
        // First frame: the whole patch, never above 100 %.
        ui_state.canvas = canvas;
        ui_state.frame_world(lay.bounds.expand(4.0), 1.0);
        ui_state.fitted = true;
    } else if canvas != ui_state.canvas {
        // Drawer opened or window resized: keep what the user is working on reachable.
        ui_state.canvas = canvas;
        let target = inspected_extent(editor, ui_state, &lay)
            .map(|(r, _)| r)
            .or_else(|| {
                ui_state
                    .selected_module
                    .and_then(|id| Some(lay.get(id)?.face))
            });
        if let Some(t) = target {
            let t = ui_state.xf().r(t);
            ui_state.keep_visible(t);
        }
    }
    // Inspecting a control that is off the face (route selected, drawer) reveals it.
    if ui_state.inspected != ui_state.last_inspected {
        ui_state.last_inspected = ui_state.inspected.clone();
        if let Some((id, p)) = &ui_state.inspected {
            if let Some(h) = lay
                .get(*id)
                .and_then(|m| m.hidden.iter().find(|h| h.name == p))
            {
                ui_state.expanded.insert(*id);
                ui_state.flash = Some((*id, h.name, now));
                ui_state.reveal = Some(*id);
            }
        }
    }
    let lay = rack::layout(editor.state(), &ui_state.view());
    if let Some(id) = ui_state.reveal.take() {
        if let Some(m) = lay.get(id) {
            let t = ui_state.xf().r(m.full());
            ui_state.keep_visible(t);
        }
    }

    // Newly inspected knob, or a route added to it: once the pointer is up, pan its source lanes
    // fully into view so no dot hides under the drawer or the canvas edge.
    if !ui.input(|i| i.pointer.any_down()) {
        match inspected_extent(editor, ui_state, &lay) {
            Some((t, key)) if ui_state.lanes_shown.as_ref() != Some(&key) => {
                let t = ui_state.xf().r(t);
                ui_state.keep_visible(t);
                ui_state.lanes_shown = Some(key);
            }
            Some(_) => {}
            None => ui_state.lanes_shown = None,
        }
    }

    // Wheel pans, Ctrl+wheel / pinch zooms about the pointer.
    let pointer = ui.input(|i| i.pointer.hover_pos());
    if let Some(p) = pointer.filter(|p| canvas.contains(*p)) {
        let (scroll, zd) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta()));
        if zd != 1.0 {
            ui_state.zoom_about(ui_state.zoom * zd, p);
        } else if scroll != EguiVec2::ZERO {
            ui_state.pan += scroll;
        }
    }
    let bg = ui.interact(canvas, Id::new("kabl-canvas-bg"), Sense::click_and_drag());
    if bg.dragged() && ui_state.port_drag.is_none() {
        ui_state.pan += bg.drag_delta();
    }
    if bg.clicked() {
        // Clicking empty rack ends knob inspection (and its source lanes).
        ui_state.inspected = None;
        ui_state.selected_route = None;
    }
    let xf = ui_state.xf();
    let painter = ui.painter_at(canvas);
    draw_rails(&painter, th, xf, canvas, lay.rows.max(2));

    let mut drawn = Drawn::default();
    ui_state.deferred.clear();
    for m in &lay.mods {
        draw_module(editor, ui_state, ui, &painter, th, xf, m, now, &mut drawn);
    }
    // Floating areas' visible screen rects, for drop targeting.
    let floats: Vec<(ModuleId, Rect)> = lay
        .mods
        .iter()
        .filter(|m| m.overlay)
        .filter_map(|m| Some((m.id, xf.r(m.adv?).intersect(canvas))))
        .collect();

    let deferred = std::mem::take(&mut ui_state.deferred);
    draw_cables(editor, ui_state, ui, &painter, th, xf, &lay, &drawn, None);
    painter.extend(deferred);
    // Hidden modulated controls: the lead docks on the `+N` button, which says so.
    for m in &lay.mods {
        let Some(t) = m.toggle else { continue };
        let routed = hidden_routes(editor, m);
        if routed.is_empty() {
            continue;
        }
        let r = xf.r(t);
        painter.rect_stroke(
            r.expand(3.0 * xf.zoom),
            CornerRadius::same(6),
            Stroke::new(2.5 * xf.zoom, th.cv),
            egui::StrokeKind::Outside,
        );
        if ui_state.cable_view == CableView::Hidden {
            routing::defer_badge(
                ui_state,
                &painter,
                pos2(r.center().x, r.bottom() + 12.0 * xf.zoom),
                &format!("{} hidden", routed.len()),
                th.cv,
                xf.zoom,
            );
            painter.extend(std::mem::take(&mut ui_state.deferred));
        }
    }

    for m in lay.mods.iter().filter(|m| m.overlay) {
        draw_float(
            editor, ui_state, ui, th, xf, m, now, &mut drawn, &lay, canvas,
        );
    }

    explain::mark_target(
        ui_state,
        &painter,
        th,
        xf.zoom,
        &lay,
        |r| xf.r(r),
        |id, port| {
            drawn
                .ports
                .get(&(id, PortDirection::Input, port.to_string()))
                .copied()
        },
    );
    if let Some((from, at)) = drawn.drop_at.take() {
        drop_cable(editor, ui_state, from, at, &floats);
    }
    draw_port_drag(editor, ui_state, ui, th, &drawn, &floats);
    // Inspection starts mid-frame; the next frame pans its lanes into view.
    if inspected_extent(editor, ui_state, &lay).map(|(_, k)| k) != ui_state.lanes_shown {
        ui.ctx().request_repaint();
    }
    if ui_state.flash.is_some_and(|(_, _, t)| now - t < 1.6) {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
    }
}

fn draw_rails(p: &egui::Painter, th: &Theme, xf: Xf, canvas: Rect, rows: usize) {
    for row in 0..rows {
        let y = rack::row_y(row);
        for ry in [y - 8.0, y + PANEL_H] {
            let r = Rect::from_min_max(
                pos2(canvas.left(), xf.p(pos2(0.0, ry)).y),
                pos2(canvas.right(), xf.p(pos2(0.0, ry + 8.0)).y),
            );
            p.rect_filled(r, CornerRadius::ZERO, th.rail);
            p.line_segment([r.left_top(), r.right_top()], Stroke::new(1.0, th.rail_hi));
            let step = rack::UNIT * xf.zoom;
            let mut x = canvas.left() + (xf.origin.x - canvas.left()).rem_euclid(step);
            while x < canvas.right() {
                p.circle_filled(pos2(x, r.center().y), 2.0 * xf.zoom, th.hole);
                x += step;
            }
        }
    }
}

/// Routes into params of `m` that currently have no visible control.
fn hidden_routes(editor: &PatchEditor, m: &Placed) -> Vec<(CableId, &'static str)> {
    m.hidden
        .iter()
        .flat_map(|p| {
            routing::routes_into(editor.state(), m.id, p.name)
                .into_iter()
                .map(|r| (r.cable, p.name))
        })
        .collect()
}

fn text(
    p: &egui::Painter,
    pos: Pos2,
    align: egui::Align2,
    s: &str,
    size: f32,
    col: Color32,
    mono: bool,
) -> Rect {
    let f = if mono {
        egui::FontId::monospace(size)
    } else {
        egui::FontId::proportional(size)
    };
    p.text(pos, align, s, f, col)
}

#[allow(clippy::too_many_arguments)]
fn draw_module(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    th: &Theme,
    xf: Xf,
    m: &Placed,
    now: f64,
    drawn: &mut Drawn,
) {
    let z = xf.zoom;
    let rect = xf.r(m.rect);
    let face = xf.r(m.face);
    ui_state.record(format!("module:{}", m.id), face);

    // Body: select, drag to move (rows snap on release), context menu.
    let body = ui.interact(
        rect,
        Id::new(("kabl-module-body", m.id)),
        Sense::click_and_drag(),
    );
    if body.clicked() {
        ui_state.selected_module = Some(m.id);
    }
    if body.drag_started() {
        if let (Some(p), Some(stored)) = (
            ui.input(|i| i.pointer.press_origin()),
            editor.state().modules.get(&m.id).map(|s| s.pos),
        ) {
            ui_state.selected_module = Some(m.id);
            ui_state.moving = Some(Moving {
                id: m.id,
                grab: xf.inv(p) - m.face.min,
                live: stored,
                start: stored,
            });
        }
    }
    if body.dragged() {
        if let (Some(mv), Some(p)) = (
            ui_state.moving.as_mut().filter(|mv| mv.id == m.id),
            body.interact_pointer_pos(),
        ) {
            let w = xf.inv(p) - mv.grab;
            mv.live = Vec2 { x: w.x, y: w.y };
        }
    }
    if body.drag_stopped() {
        if let Some(mv) = ui_state.moving.take().filter(|mv| mv.id == m.id) {
            let to = rack::snap(pos2(mv.live.x, mv.live.y));
            if to != mv.start {
                editor.move_module(m.id, to);
            }
        }
    }
    body.context_menu(|ui| module_menu(editor, ui_state, ui, m));

    let skin = m.skin;
    let on_art = skin.is_some_and(|s| ui_state.skin_labels_on_art.unwrap_or(s.labels_on_art));
    painter.rect_filled(rect, CornerRadius::same(3), th.panel);
    if let Some((tex, tint)) =
        skin.and_then(|s| skin_texture(ui, &mut ui_state.image_cache, m.info.kind, s, th.dark))
    {
        painter.image(
            tex,
            face,
            Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
            tint,
        );
    } else {
        // Brushed texture: faint horizontal strokes.
        let step = (4.0 * z).max(3.0);
        let mut y = rect.top() + step;
        let tex = if th.dark {
            Color32::from_white_alpha(6)
        } else {
            Color32::from_black_alpha(7)
        };
        while y < rect.bottom() {
            painter.line_segment(
                [pos2(rect.left(), y), pos2(rect.right(), y)],
                Stroke::new(1.0, tex),
            );
            y += step;
        }
    }
    painter.rect_stroke(
        rect,
        CornerRadius::same(3),
        Stroke::new(1.0, th.panel_edge),
        egui::StrokeKind::Inside,
    );
    if m.adv.is_some() && !m.overlay {
        // Engraved divider between the face and the advanced area.
        let x = face.right();
        painter.line_segment(
            [
                pos2(x, rect.top() + 12.0 * z),
                pos2(x, rect.bottom() - 12.0 * z),
            ],
            Stroke::new(1.5, th.panel_edge.lerp_to_gamma(Color32::BLACK, 0.2)),
        );
    }
    for (sx, sy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
        let c = pos2(
            rect.left() + 9.0 * z + sx * (rect.width() - 18.0 * z),
            rect.top() + 9.0 * z + sy * (rect.height() - 18.0 * z),
        );
        painter.circle_filled(c, 3.5 * z, th.nut);
        painter.circle_stroke(c, 3.5 * z, Stroke::new(1.0, th.nut_edge));
    }
    let (ink, ink2) = match skin {
        Some(s) if on_art => {
            let c = s.art_ink[usize::from(th.dark)];
            let c = Color32::from_rgb(c[0], c[1], c[2]);
            (c, c)
        }
        _ => (th.ink, th.ink2),
    };
    if skin.is_some() && !on_art {
        let hdr = Rect::from_center_size(
            pos2(face.center().x, face.top() + 33.0 * z),
            vec2(170.0, 38.0) * z,
        );
        painter.rect_filled(hdr, CornerRadius::same(4), th.panel.gamma_multiply(0.96));
    }
    let choosing = ui_state.choose.as_ref().filter(|(id, _)| *id == m.id);
    text(
        painter,
        pos2(face.center().x, face.top() + 26.0 * z),
        egui::Align2::CENTER_CENTER,
        m.info.name,
        15.0 * z,
        ink,
        false,
    );
    let tag = match choosing {
        Some((_, set)) => {
            let n = set.iter().filter(|on| **on).count();
            format!("choose face · {n} on face")
        }
        None if skin.is_some() => format!("{} · skin · #{}", m.info.kind, m.id),
        None if m.info.kind == "seq" => {
            let edit = ui_state.edit_bank_of(editor.state(), m.id);
            let playing = ui_state.seq_banks.get(&m.id).map(|b| b.0);
            if playing.is_some_and(|p| p != edit) {
                format!(
                    "seq · #{} · editing {} (not playing)",
                    m.id,
                    banks::title(editor.state(), m.id, edit)
                )
            } else {
                format!(
                    "seq · #{} · {}",
                    m.id,
                    banks::title(editor.state(), m.id, edit)
                )
            }
        }
        None if m.info.kind == "lfo" => {
            let sync = editor
                .state()
                .modules
                .get(&m.id)
                .and_then(|s| s.params.get("sync"))
                .map_or(0, |v| v.round() as usize);
            match (sync, ui_state.lfo_status.get(&m.id)) {
                (0, _) => format!("lfo · #{}", m.id),
                (s, st) => format!(
                    "lfo · #{} · {} · {}",
                    m.id,
                    kabl_modules::builtins::SYNC_LABELS
                        .get(s)
                        .copied()
                        .unwrap_or("?")
                        .to_lowercase(),
                    match st {
                        Some(LfoSync::Synced) => "synced",
                        Some(LfoSync::Held) => "held",
                        Some(LfoSync::Free) | None => "free rate",
                        Some(LfoSync::Acquiring) => "acquiring",
                    }
                ),
            }
        }
        None => format!("{} · #{}", m.info.kind, m.id),
    };
    text(
        painter,
        pos2(face.center().x, face.top() + 43.0 * z),
        egui::Align2::CENTER_CENTER,
        &tag,
        11.0 * z,
        ink2,
        true,
    );
    if ui_state.selected_module == Some(m.id) {
        painter.rect_stroke(
            rect.expand(1.0),
            CornerRadius::same(4),
            Stroke::new(2.5, th.sel),
            egui::StrokeKind::Outside,
        );
    }
    if skin.is_none() {
        draw_decor(editor, painter, th, xf, m);
    }
    if let Decor::Keys(r) = m.decor {
        // The voice settings this MIDI In gives the chain it drives, including the ones off
        // the face, so it is plain whose settings they are.
        text(
            painter,
            xf.p(pos2(r.center().x, r.bottom() + 11.0)),
            egui::Align2::CENTER_CENTER,
            &midi_in_summary(editor.state(), m),
            10.5 * z,
            ink2,
            true,
        );
    }
    if let Decor::Transport(r) = m.decor {
        draw_transport(ui_state, ui, painter, th, xf, m.id, r);
    }
    if let Decor::Cues(r) = m.decor {
        draw_cues(editor, ui_state, ui, painter, th, xf, m.id, r, now);
    }
    if let Decor::Banks(r) = m.decor {
        draw_banks(editor, ui_state, ui, painter, th, xf, m.id, r, now, drawn);
    }
    if let Decor::Status(p) = m.decor {
        let (lock, ms) = ui_state
            .delay_status
            .get(&m.id)
            .copied()
            .unwrap_or((DelayLock::Unlocked, f32::NAN));
        let time = if ms.is_finite() {
            routing::fmt_value(&m.info.params[0], ms)
        } else {
            "…".into()
        };
        let word = match lock {
            DelayLock::Free => "free",
            DelayLock::Unlocked => "unlocked",
            DelayLock::Locked => "sync",
            DelayLock::Held => "held",
        };
        text(
            painter,
            xf.p(p),
            egui::Align2::LEFT_CENTER,
            &format!("{word} · {time}"),
            11.0 * z,
            if lock == DelayLock::Locked {
                th.gate
            } else {
                ink2
            },
            true,
        );
    }
    // Step light: beside the playing step's pitch label; hollow while the face edits a bank
    // that is not playing.
    if let Some(&s) = ui_state.seq_steps.get(&m.id) {
        let edit = ui_state.edit_bank_of(editor.state(), m.id);
        let playing = ui_state.seq_banks.get(&m.id).map_or(edit, |b| b.0);
        if let Some(c) = m.ctl(seq::bank_param(edit, s.min(seq::STEPS - 1))) {
            let at = xf.p(c.geo.label_pos() - vec2(16.0, 0.0));
            if playing == edit {
                painter.circle_filled(at, 4.0 * z, th.gate);
            } else {
                painter.circle_stroke(at, 3.5 * z, Stroke::new(1.5 * z, th.gate));
            }
        }
    }
    if let Some(p) = m.plate {
        painter.rect_filled(xf.r(p), CornerRadius::same(6), th.plate);
    }

    let look = Look { z, ink, ink2 };
    let plates = skin.is_some() && !on_art;
    for c in m.ctls.iter().filter(|c| c.primary || !m.overlay) {
        draw_control(
            editor, ui_state, ui, painter, th, xf, m, c, look, plates, now, drawn,
        );
    }
    draw_jacks(editor, ui_state, ui, painter, th, xf, m, on_art, drawn);

    if let Some(t) = m.toggle {
        let r = xf.r(t);
        let expanded = m.adv.is_some();
        let label = if expanded {
            "Less".to_string()
        } else {
            format!("+{}", m.hidden.len())
        };
        ui_state.record(format!("toggle:{}", m.id), r);
        let resp = ui.interact(r, Id::new(("kabl-toggle", m.id)), Sense::click());
        painter.rect_filled(
            r,
            CornerRadius::same(4),
            if th.dark { th.btn } else { th.plate },
        );
        if resp.hovered() {
            painter.rect_stroke(
                r,
                CornerRadius::same(4),
                Stroke::new(1.0, th.sel),
                egui::StrokeKind::Inside,
            );
        }
        text(
            painter,
            r.center(),
            egui::Align2::CENTER_CENTER,
            &label,
            12.0 * z,
            th.plate_ink,
            false,
        );
        let routed = hidden_routes(editor, m);
        for (cable, _) in &routed {
            drawn.plugs.push((*cable, r.center()));
        }
        let resp = if routed.is_empty() {
            resp.on_hover_text(if expanded {
                "Hide advanced controls"
            } else {
                "Show advanced controls"
            })
        } else {
            let lines: Vec<String> = routed
                .iter()
                .map(|(cable, p)| {
                    let src = match editor.state().cables.get(cable).map(|c| &c.from) {
                        Some(PortRef::Module { id, port }) => {
                            routing::source_label(editor.state(), *id, port)
                        }
                        _ => "?".into(),
                    };
                    let label = m
                        .info
                        .params
                        .iter()
                        .find(|q| q.name == *p)
                        .map_or(p.to_string(), routing::target_label);
                    format!("Modulated, off the face: {src} > {label}")
                })
                .collect();
            resp.on_hover_text(lines.join("\n"))
        };
        if resp.clicked() {
            if expanded {
                ui_state.expanded.remove(&m.id);
            } else {
                ui_state.expanded.insert(m.id);
                ui_state.reveal = Some(m.id);
                if let Some((_, p)) = routed.first() {
                    ui_state.flash = Some((m.id, p, now));
                }
            }
        }
    }
    if let Some(d) = m.done {
        let r = xf.r(d);
        ui_state.record(format!("done:{}", m.id), r);
        let resp = ui.interact(r, Id::new(("kabl-done", m.id)), Sense::click());
        painter.rect_filled(r, CornerRadius::same(4), th.sel);
        text(
            painter,
            r.center(),
            egui::Align2::CENTER_CENTER,
            "Done",
            12.0 * z,
            Color32::WHITE,
            false,
        );
        if resp.clicked() {
            if let Some((id, set)) = ui_state.choose.take() {
                editor.set_primary(id, &set);
            }
        }
    }
}

fn module_menu(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui, m: &Placed) {
    ui.label(egui::RichText::new(format!("{} #{}", m.info.name, m.id)).strong());
    fn item(ui: &mut egui::Ui, ui_state: &mut UiState, key: &str, label: &str) -> bool {
        let r = ui.button(label);
        ui_state.record(format!("menu:{key}"), r.rect);
        r.clicked()
    }
    if item(ui, ui_state, "explain", "Explain this module") {
        explain::open(ui_state, explain::Subject::Module(m.id), None);
        ui_state.selected_module = Some(m.id);
        ui.close();
    }
    if !m.info.params.is_empty() && item(ui, ui_state, "choose", "Choose primary controls…") {
        let set = editor
            .state()
            .modules
            .get(&m.id)
            .map(|s| rack::primary_set(s, m.info))
            .unwrap_or_default();
        ui_state.choose = Some((m.id, set));
        ui_state.reveal = Some(m.id);
        ui.close();
    }
    if !m.info.params.is_empty() && item(ui, ui_state, "reset-face", "Reset face to module default")
    {
        let defaults: Vec<bool> = m
            .info
            .params
            .iter()
            .map(|p| !m.info.advanced.contains(&p.name))
            .collect();
        editor.set_primary(m.id, &defaults);
        ui.close();
    }
    if m.toggle.is_some() {
        let expanded = ui_state.expanded.contains(&m.id);
        if item(
            ui,
            ui_state,
            "expand",
            if expanded { "Collapse" } else { "Expand" },
        ) {
            if expanded {
                ui_state.expanded.remove(&m.id);
            } else {
                ui_state.expanded.insert(m.id);
                ui_state.reveal = Some(m.id);
            }
            ui.close();
        }
    }
    ui.separator();
    perform::module_menu(editor, ui_state, ui, m.id, m.info);
    ui.separator();
    if item(ui, ui_state, "remove", "Remove module") {
        editor.remove_module(m.id);
        ui.close();
    }
}

/// Run/Stop and Restart. The label follows the audio thread's report, not the click.
fn draw_transport(
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    th: &Theme,
    xf: Xf,
    id: ModuleId,
    r: Rect,
) {
    let running = ui_state.clock_running.get(&id).copied().unwrap_or(true);
    let half = (r.width() - 8.0) / 2.0;
    let buttons = [
        ("run", if running { "Stop" } else { "Run" }, 0.0),
        ("restart", "Restart", half + 8.0),
    ];
    for (key, label, dx) in buttons {
        let b = xf.r(Rect::from_min_size(
            r.min + vec2(dx, 0.0),
            vec2(half, r.height()),
        ));
        ui_state.record(format!("{key}:{id}"), b);
        let resp = ui.interact(b, Id::new(("kabl-transport", key, id)), Sense::click());
        let lit = key == "run" && running;
        painter.rect_filled(
            b,
            CornerRadius::same(4),
            if th.dark { th.btn } else { th.plate },
        );
        if lit {
            painter.circle_filled(
                b.left_center() + vec2(10.0 * xf.zoom, 0.0),
                3.5 * xf.zoom,
                th.gate,
            );
        }
        painter.rect_stroke(
            b,
            CornerRadius::same(4),
            Stroke::new(
                1.0,
                if resp.hovered() {
                    th.sel
                } else {
                    th.panel_edge
                },
            ),
            egui::StrokeKind::Inside,
        );
        text(
            painter,
            b.center() + vec2(if lit { 5.0 * xf.zoom } else { 0.0 }, 0.0),
            egui::Align2::CENTER_CENTER,
            label,
            12.0 * xf.zoom,
            th.plate_ink,
            false,
        );
        let resp = resp.on_hover_text(match key {
            "run" if running => "Stop the clock: gates go low, envelopes release",
            "run" => "Start the clock on the next step",
            _ if running => "Restart every pattern on step 1 now (sends a pulse on reset)",
            _ => "Start on step 1 when the clock runs again",
        });
        if resp.clicked() {
            let t = match key {
                "run" if running => Transport::Stop,
                "run" => Transport::Run,
                _ => Transport::Restart,
            };
            ui_state.transport.push((id, t));
        }
    }
}

/// The sequencer's bank strip. EDIT tabs pick the bank the face shows (view only); PLAY buttons
/// launch a bank with the sequencer's own timing and clock (runtime); the playing bank is lit,
/// a queued one blinks, and Cancel drops the queued launch. Right-click a tab for bank edits.
#[allow(clippy::too_many_arguments)]
fn draw_banks(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    th: &Theme,
    xf: Xf,
    id: ModuleId,
    r: Rect,
    now: f64,
    drawn: &mut Drawn,
) {
    let z = xf.zoom;
    let edit = ui_state.edit_bank_of(editor.state(), id);
    let (playing, queued) = ui_state
        .seq_banks
        .get(&id)
        .copied()
        .unwrap_or((banks::startup(editor.state(), id), None));
    let bw = 26.0;
    let label = |x: f32, s: &str| {
        text(
            painter,
            xf.p(pos2(x, r.center().y)),
            egui::Align2::LEFT_CENTER,
            s,
            10.5 * z,
            th.ink2,
            true,
        );
    };
    label(r.left(), "EDIT");
    let play_x = r.right() - 4.0 * (bw + 4.0);
    label(play_x - 38.0, "PLAY");
    for b in 0..seq::BANKS {
        let name = banks::title(editor.state(), id, b);
        // EDIT tab.
        let t = xf.r(Rect::from_min_size(
            pos2(r.left() + 36.0 + b as f32 * (bw + 4.0), r.top()),
            vec2(bw, r.height()),
        ));
        ui_state.record(format!("bank-edit:{id}.{b}"), t);
        let resp = ui.interact(t, Id::new(("kabl-bank-edit", id, b)), Sense::click());
        let on = b == edit;
        painter.rect_filled(
            t,
            CornerRadius::same(4),
            if on {
                th.sel
            } else if th.dark {
                th.btn
            } else {
                th.plate
            },
        );
        text(
            painter,
            t.center(),
            egui::Align2::CENTER_CENTER,
            seq::BANK_NAMES[b],
            12.0 * z,
            if on { Color32::WHITE } else { th.plate_ink },
            false,
        );
        // Routes into this bank's controls while another bank is on the face plug in here.
        let routed: Vec<CableId> = if on {
            Vec::new()
        } else {
            editor
                .state()
                .cables
                .iter()
                .filter_map(|(&cid, c)| match &c.to {
                    PortRef::Param { id: to, param }
                        if *to == id && seq::bank_of(param).is_some_and(|(k, _)| k == b) =>
                    {
                        Some(cid)
                    }
                    _ => None,
                })
                .collect()
        };
        for &c in &routed {
            drawn.plugs.push((c, t.center_bottom()));
        }
        if !routed.is_empty() {
            painter.circle_filled(t.right_top(), 3.5 * z, th.sel);
        }
        let resp = resp.on_hover_text(format!(
            "Edit bank {name}{}{} (does not launch it). Right-click: copy, clear, rename",
            if b == playing { ", playing" } else { "" },
            if routed.is_empty() {
                String::new()
            } else {
                format!(", {} modulation route(s)", routed.len())
            }
        ));
        if resp.clicked() {
            ui_state.edit_bank.insert(id, b);
        }
        resp.context_menu(|ui| bank_menu(editor, ui_state, ui, id, b));

        // PLAY button.
        let pbtn = xf.r(Rect::from_min_size(
            pos2(play_x + b as f32 * (bw + 4.0), r.top()),
            vec2(bw, r.height()),
        ));
        ui_state.record(format!("bank-play:{id}.{b}"), pbtn);
        let resp = ui.interact(pbtn, Id::new(("kabl-bank-play", id, b)), Sense::click());
        let lit = b == playing;
        painter.rect_filled(
            pbtn,
            CornerRadius::same(4),
            if lit {
                th.gate
            } else if th.dark {
                th.btn
            } else {
                th.plate
            },
        );
        if queued == Some(b) {
            // Blinks twice a second until it lands.
            let a = if (now * 4.0) as i64 % 2 == 0 {
                1.0
            } else {
                0.35
            };
            painter.rect_stroke(
                pbtn.expand(1.5 * z),
                CornerRadius::same(5),
                Stroke::new(2.5 * z, th.gate.gamma_multiply(a)),
                egui::StrokeKind::Outside,
            );
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(120));
        }
        text(
            painter,
            pbtn.center(),
            egui::Align2::CENTER_CENTER,
            seq::BANK_NAMES[b],
            12.0 * z,
            if lit { Color32::WHITE } else { th.plate_ink },
            false,
        );
        let timing = banks::timing(editor.state(), id);
        let when = banks::TIMINGS
            .iter()
            .find(|t| t.0 == timing)
            .map_or("", |t| t.1);
        let resp = resp.on_hover_text(if lit {
            format!("Bank {name} is playing")
        } else if queued == Some(b) {
            format!("Bank {name} is queued ({when})")
        } else {
            format!("Launch bank {name} ({when})")
        });
        if resp.clicked() {
            if let Err(e) = banks::launch(&mut ui_state.launches, editor.state(), id, b) {
                ui_state.last_message = Some(e);
            }
        }
    }
    if queued.is_some() {
        let c = xf.r(Rect::from_min_size(
            pos2(play_x - 96.0, r.top()),
            vec2(54.0, r.height()),
        ));
        ui_state.record(format!("bank-cancel:{id}"), c);
        let resp = ui.interact(c, Id::new(("kabl-bank-cancel", id)), Sense::click());
        painter.rect_stroke(
            c,
            CornerRadius::same(4),
            Stroke::new(
                1.0,
                if resp.hovered() {
                    th.sel
                } else {
                    th.panel_edge
                },
            ),
            egui::StrokeKind::Inside,
        );
        text(
            painter,
            c.center(),
            egui::Align2::CENTER_CENTER,
            "Cancel",
            11.0 * z,
            th.ink,
            false,
        );
        if resp.on_hover_text("Drop the queued launch").clicked() {
            ui_state.launches.push(Command::Cancel(Some(id)));
        }
    }
}

/// A playing (lit) / queued (blinking) launch button on the rack.
#[allow(clippy::too_many_arguments)]
fn pad(
    ui_state: &mut UiState,
    ui: &egui::Ui,
    painter: &egui::Painter,
    th: &Theme,
    z: f32,
    key: String,
    r: Rect,
    label: &str,
    lit: bool,
    queued: bool,
    now: f64,
) -> egui::Response {
    ui_state.record(key.clone(), r);
    let resp = ui.interact(r, Id::new(("kabl-pad", key)), Sense::click());
    painter.rect_filled(
        r,
        CornerRadius::same(5),
        if lit {
            th.gate
        } else if th.dark {
            th.btn
        } else {
            th.plate
        },
    );
    painter.rect_stroke(
        r,
        CornerRadius::same(5),
        Stroke::new(
            1.0,
            if resp.hovered() {
                th.sel
            } else {
                th.panel_edge
            },
        ),
        egui::StrokeKind::Inside,
    );
    if queued {
        let a = if (now * 4.0) as i64 % 2 == 0 {
            1.0
        } else {
            0.35
        };
        painter.rect_stroke(
            r.expand(1.5 * z),
            CornerRadius::same(6),
            Stroke::new(2.5 * z, th.gate.gamma_multiply(a)),
            egui::StrokeKind::Outside,
        );
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(120));
    }
    text(
        painter,
        r.center(),
        egui::Align2::CENTER_CENTER,
        label,
        12.5 * z,
        if lit { Color32::WHITE } else { th.plate_ink },
        false,
    );
    resp
}

/// The cues module's face: eight cue buttons (active lit, queued blinking) and Cancel.
#[allow(clippy::too_many_arguments)]
fn draw_cues(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    th: &Theme,
    xf: Xf,
    id: ModuleId,
    r: Rect,
    now: f64,
) {
    let z = xf.zoom;
    let state = editor.state();
    let all = cues::cues(state, id);
    let bank_of = |s: ModuleId| {
        ui_state
            .seq_banks
            .get(&s)
            .copied()
            .unwrap_or((banks::startup(state, s), None))
    };
    let standing: Vec<(bool, bool)> = all.iter().map(|c| cues::standing(c, &bank_of)).collect();
    let (gap, h) = (8.0, 50.0);
    let w = (r.width() - 3.0 * gap) / 4.0;
    let mut launch = None;
    for n in 1..=kabl_modules::builtins::CUES {
        let (col, row) = ((n - 1) % 4, (n - 1) / 4);
        let b = xf.r(Rect::from_min_size(
            r.min + vec2(col as f32 * (w + gap), row as f32 * (h + gap)),
            vec2(w, h),
        ));
        match all.iter().position(|c| c.n == n) {
            Some(i) => {
                let c = &all[i];
                let (active, queued) = standing[i];
                let resp = pad(
                    ui_state,
                    ui,
                    painter,
                    th,
                    z,
                    format!("cue:{id}.{n}"),
                    b,
                    &c.name,
                    active,
                    queued,
                    now,
                );
                let tip = format!(
                    "Launch {} ({}){}",
                    c.name,
                    banks::TIMINGS
                        .iter()
                        .find(|t| t.0 == c.timing)
                        .map_or("", |t| t.1)
                        .to_lowercase(),
                    perform::button_text(state, id, &format!("cue.{n}"))
                        .map_or(String::new(), |t| format!("\n{t}"))
                );
                if resp.on_hover_text(tip).clicked() {
                    launch = Some(n);
                }
            }
            None => {
                painter.rect_stroke(
                    b,
                    CornerRadius::same(5),
                    Stroke::new(1.0, th.panel_edge),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }
    let y = r.top() + 2.0 * (h + gap) + 6.0;
    let queued = all.iter().zip(&standing).find(|(_, s)| s.1).map(|(c, _)| c);
    let active = all.iter().zip(&standing).find(|(_, s)| s.0).map(|(c, _)| c);
    let line = match (active, queued) {
        (_, Some(q)) => format!("queued: {}", q.name),
        (Some(a), None) => format!("playing: {}", a.name),
        (None, None) if all.is_empty() => "no cues: add them in the drawer".into(),
        (None, None) => "no cue matches what plays".into(),
    };
    text(
        painter,
        xf.p(pos2(r.left(), y + 14.0)),
        egui::Align2::LEFT_CENTER,
        &line,
        11.5 * z,
        th.ink2,
        true,
    );
    if let Some(q) = queued {
        let c = xf.r(Rect::from_min_size(
            pos2(r.right() - 90.0, y),
            vec2(90.0, 28.0),
        ));
        let resp = pad(
            ui_state,
            ui,
            painter,
            th,
            z,
            format!("cue-cancel:{id}"),
            c,
            "Cancel",
            false,
            false,
            now,
        );
        if resp.on_hover_text("Drop the queued cue").clicked() {
            for &(s, _) in &q.targets {
                ui_state.launches.push(Command::Cancel(Some(s)));
            }
        }
    }
    if let Some(n) = launch {
        if let Err(e) = cues::launch(&mut ui_state.launches, editor.state(), id, n) {
            ui_state.last_message = Some(e);
        }
    }
}

/// The drawer's cue editor: per cue, its name, reference clock, timing and a bank (or Keep)
/// per sequencer; add, capture what plays, delete.
fn cues_panel(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui, id: ModuleId) {
    let state = editor.state();
    let all = cues::cues(state, id);
    let clocks: Vec<ModuleId> = state
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "clock")
        .map(|(&c, _)| c)
        .collect();
    let seqs: Vec<ModuleId> = state
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "seq")
        .map(|(&c, _)| c)
        .collect();
    let playing_now: HashMap<ModuleId, usize> = seqs
        .iter()
        .map(|&s| {
            (
                s,
                ui_state
                    .seq_banks
                    .get(&s)
                    .map_or_else(|| banks::startup(state, s), |b| b.0),
            )
        })
        .collect();
    enum Edit {
        Add,
        Delete(usize),
        Capture(usize),
        Clock(usize, ModuleId),
        Timing(usize, kabl_engine::patch_engine::Timing),
        Target(usize, ModuleId, Option<usize>),
        Rename(usize, String),
    }
    let mut edit = None;
    for c in &all {
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("{}", c.n)).strong());
            let mut name = ui_state
                .cue_rename
                .as_ref()
                .filter(|(i, n, _)| (*i, *n) == (id, c.n))
                .map_or_else(|| c.name.clone(), |(_, _, t)| t.clone());
            let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(120.0));
            ui_state.record(format!("cue-name:{id}.{}", c.n), r.rect);
            if r.changed() {
                ui_state.cue_rename = Some((id, c.n, name.clone()));
            }
            if r.lost_focus() {
                ui_state.cue_rename = None;
                if name != c.name {
                    edit = Some(Edit::Rename(c.n, name));
                }
            }
            let r = ui.small_button("Capture");
            ui_state.record(format!("cue-capture:{id}.{}", c.n), r.rect);
            if r.on_hover_text("Set every sequencer to the bank it plays now")
                .clicked()
            {
                edit = Some(Edit::Capture(c.n));
            }
            let r = ui.small_button("Delete");
            ui_state.record(format!("cue-delete:{id}.{}", c.n), r.rect);
            if r.clicked() {
                edit = Some(Edit::Delete(c.n));
            }
        });
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt(("cue-clock", id, c.n))
                .selected_text(
                    c.clock
                        .map_or("choose clock".into(), |k| format!("Clock #{k}")),
                )
                .show_ui(ui, |ui| {
                    for &k in &clocks {
                        if ui
                            .selectable_label(c.clock == Some(k), format!("Clock #{k}"))
                            .clicked()
                        {
                            edit = Some(Edit::Clock(c.n, k));
                        }
                    }
                });
            egui::ComboBox::from_id_salt(("cue-timing", id, c.n))
                .selected_text(
                    banks::TIMINGS
                        .iter()
                        .find(|t| t.0 == c.timing)
                        .map_or("", |t| t.1),
                )
                .show_ui(ui, |ui| {
                    for (t, name) in banks::TIMINGS {
                        if ui.selectable_label(c.timing == t, name).clicked() {
                            edit = Some(Edit::Timing(c.n, t));
                        }
                    }
                });
        });
        for &s in &seqs {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("Sequencer #{s}")).small());
                let current = c.targets.iter().find(|t| t.0 == s).map(|t| t.1);
                let r = ui.selectable_label(current.is_none(), "Keep");
                ui_state.record(format!("cue-bank:{id}.{}.{s}.keep", c.n), r.rect);
                if r.on_hover_text("This cue leaves it playing").clicked() {
                    edit = Some(Edit::Target(c.n, s, None));
                }
                for b in 0..seq::BANKS {
                    let r = ui
                        .selectable_label(current == Some(b), seq::BANK_NAMES[b])
                        .on_hover_text(banks::title(state, s, b));
                    ui_state.record(format!("cue-bank:{id}.{}.{s}.{b}", c.n), r.rect);
                    if r.clicked() {
                        edit = Some(Edit::Target(c.n, s, Some(b)));
                    }
                }
            });
        }
    }
    ui.separator();
    let r = ui.add_enabled(
        all.len() < kabl_modules::builtins::CUES,
        egui::Button::new("Add cue (from what plays)"),
    );
    ui_state.record(format!("cue-add:{id}"), r.rect);
    if r.clicked() {
        edit = Some(Edit::Add);
    }
    match edit {
        Some(Edit::Add) => {
            cues::add(editor, id, &|s| playing_now.get(&s).copied().unwrap_or(0));
        }
        Some(Edit::Delete(n)) => cues::delete(editor, id, n),
        Some(Edit::Capture(n)) => {
            let ops = seqs
                .iter()
                .map(|&s| kabl_core::Op::SetParam {
                    target: kabl_core::ParamTarget::Module {
                        id,
                        param: format!("cue{n}.seq.{s}"),
                    },
                    value: playing_now[&s] as f32,
                })
                .collect();
            editor.edit(ops);
        }
        Some(Edit::Clock(n, k)) => cues::set_clock(editor, id, n, k),
        Some(Edit::Timing(n, t)) => cues::set_timing(editor, id, n, t),
        Some(Edit::Target(n, s, b)) => cues::set_target(editor, id, n, s, b),
        Some(Edit::Rename(n, t)) => cues::rename(editor, id, n, &t),
        None => {}
    }
}

/// Bank edits for sequencer `id`, bank `b`: copy to another bank, clear, rename, startup.
fn bank_menu(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    id: ModuleId,
    b: usize,
) {
    ui.label(egui::RichText::new(format!("Bank {}", banks::title(editor.state(), id, b))).strong());
    ui.menu_button("Copy to", |ui| {
        for to in (0..seq::BANKS).filter(|&to| to != b) {
            let r = ui.button(format!("Bank {}", banks::title(editor.state(), id, to)));
            ui_state.record(format!("menu:bank-copy.{to}"), r.rect);
            if r.clicked() {
                banks::copy(editor, id, b, to);
                ui_state.edit_bank.insert(id, to);
                ui_state.last_message = Some(format!(
                    "copied bank {} to {}",
                    seq::BANK_NAMES[b],
                    seq::BANK_NAMES[to]
                ));
                ui.close();
            }
        }
    });
    let r = ui.button("Clear");
    ui_state.record("menu:bank-clear".into(), r.rect);
    if r.clicked() {
        banks::clear(editor, id, b);
        ui.close();
    }
    let startup = banks::startup(editor.state(), id) == b;
    let r = ui.add_enabled(!startup, egui::Button::new("Set as startup bank"));
    ui_state.record("menu:bank-startup".into(), r.rect);
    if r.on_hover_text("The bank a Load starts on; does not change what is playing")
        .clicked()
    {
        editor.set_param(id, "bank", b as f32);
        ui.close();
    }
    ui.separator();
    ui.label("Name");
    let stored = editor
        .state()
        .label(id, &banks::name_key(b))
        .unwrap_or("")
        .to_string();
    let mut t = match ui_state.bank_rename.take() {
        Some((i, k, t)) if (i, k) == (id, b) => t,
        _ => stored,
    };
    let r = ui.add(egui::TextEdit::singleline(&mut t).desired_width(120.0));
    ui_state.record("menu:bank-name".into(), r.rect);
    let commit = r.lost_focus() || ui.button("Rename").clicked();
    if commit {
        banks::rename(editor, id, b, &t);
    } else {
        ui_state.bank_rename = Some((id, b, t));
    }
}

/// `POLY`, `MONO · LOW`, `LEGATO · LAST · glide 120 ms`: a MIDI In's voice settings.
fn midi_in_summary(state: &kabl_core::PatchState, m: &Placed) -> String {
    let get = |name: &str| {
        let p = m.info.params.iter().find(|p| p.name == name);
        state
            .modules
            .get(&m.id)
            .and_then(|s| s.params.get(name).copied())
            .or(p.map(|p| p.default))
            .unwrap_or(0.0)
    };
    let label = |name: &str| {
        let i = get(name).round().clamp(0.0, 2.0) as usize;
        routing::step_labels("midi.in", name).map_or("?", |l| l[i])
    };
    let mut s = label("mode").to_string();
    if get("mode") >= 0.5 {
        s += " · ";
        s += label("priority");
    }
    if get("glide") >= 0.5 {
        let ms = m.info.params.iter().find(|p| p.name == "glide_ms");
        s += " · glide ";
        s += &ms.map_or(String::new(), |p| routing::fmt_value(p, get("glide_ms")));
    }
    s
}

fn draw_decor(editor: &PatchEditor, p: &egui::Painter, th: &Theme, xf: Xf, m: &Placed) {
    let z = xf.zoom;
    match m.decor {
        Decor::Keys(r) => {
            let r = xf.r(r);
            let n = 7;
            let w = r.width() / n as f32;
            for i in 0..n {
                let k = Rect::from_min_size(
                    pos2(r.left() + w * i as f32, r.top()),
                    vec2(w - 1.0, r.height()),
                );
                p.rect_filled(
                    k,
                    CornerRadius::same(2),
                    Color32::from_rgb(0xf2, 0xee, 0xe6),
                );
                p.rect_stroke(
                    k,
                    CornerRadius::same(2),
                    Stroke::new(1.0, th.tick),
                    egui::StrokeKind::Inside,
                );
            }
            for i in [0, 1, 3, 4, 5] {
                let k = Rect::from_min_size(
                    pos2(r.left() + w * (i as f32 + 0.68), r.top()),
                    vec2(w * 0.62, r.height() * 0.6),
                );
                p.rect_filled(
                    k,
                    CornerRadius::same(1),
                    Color32::from_rgb(0x2a, 0x28, 0x26),
                );
            }
        }
        Decor::Speaker(c) => {
            let c = xf.p(c);
            for r in [40.0, 30.0, 20.0, 10.0] {
                p.circle_stroke(c, r * z, Stroke::new(1.2, th.ink2));
            }
        }
        Decor::Envelope(r) => {
            // Drawn from the knob values (not telemetry).
            let r = xf.r(r);
            p.rect_filled(r, CornerRadius::same(3), th.display);
            let v = |name: &str| {
                m.info
                    .params
                    .iter()
                    .find(|q| q.name == name)
                    .map_or(0.0, |q| routing::base_value(editor.state(), m.id, q))
            };
            let (a, d, s, rel) = (v("attack_ms"), v("decay_ms"), v("sustain"), v("release_ms"));
            let wlog = |ms: f32| (1.0 + ms.max(0.1)).ln();
            let total = wlog(a) + wlog(d) + wlog(rel) + 2.0;
            let sx = |x: f32| r.left() + 6.0 * z + (r.width() - 12.0 * z) * x / total;
            let sy = |y: f32| r.bottom() - 6.0 * z - (r.height() - 12.0 * z) * y;
            let (x1, x2) = (wlog(a), wlog(a) + wlog(d));
            let pts = vec![
                pos2(sx(0.0), sy(0.0)),
                pos2(sx(x1), sy(1.0)),
                pos2(sx(x2), sy(s)),
                pos2(sx(x2 + 2.0), sy(s)),
                pos2(sx(total), sy(0.0)),
            ];
            p.add(egui::Shape::line(pts, Stroke::new(1.6 * z, th.display_ink)));
        }
        Decor::None | Decor::Transport(_) | Decor::Status(_) | Decor::Banks(_) | Decor::Cues(_) => {
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_control(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    th: &Theme,
    xf: Xf,
    m: &Placed,
    c: &rack::Ctl,
    look: Look,
    plates: bool,
    now: f64,
    drawn: &mut Drawn,
) {
    let z = xf.zoom;
    if plates {
        // Theme plate behind each control group: contrast never depends on the art.
        let plate = match c.geo {
            Geo::Knob { c, r } => {
                Rect::from_min_max(c + vec2(-r - 14.0, -58.0), c + vec2(r + 14.0, 50.0))
            }
            Geo::Select { rect } => rect
                .expand2(vec2(6.0, 6.0))
                .union(Rect::from_center_size(c.geo.label_pos(), vec2(70.0, 18.0))),
        };
        painter.rect_filled(
            xf.r(plate),
            CornerRadius::same(5),
            th.panel.gamma_multiply(0.96),
        );
    }
    match c.geo {
        Geo::Knob { c: cen, r } => {
            let plugs = routing::param_knob(
                editor,
                ui_state,
                ui,
                painter,
                m.id,
                c.param,
                xf.p(cen),
                r,
                look,
            );
            drawn.plugs.extend(plugs);
            if ui_state.flash_on(m.id, c.param.name, now) {
                painter.circle_stroke(xf.p(cen), (r + 20.0) * z, Stroke::new(3.0 * z, th.sel));
            }
        }
        Geo::Select { rect } => {
            let plugs = routing::stepped_selector(
                editor,
                ui_state,
                ui,
                painter,
                m.id,
                m.info.kind,
                c.param,
                xf.r(rect),
                look,
            );
            drawn.plugs.extend(plugs);
            if ui_state.flash_on(m.id, c.param.name, now) {
                painter.rect_stroke(
                    xf.r(rect).expand(5.0 * z),
                    CornerRadius::same(6),
                    Stroke::new(3.0 * z, th.sel),
                    egui::StrokeKind::Outside,
                );
            }
        }
    }
    // Choose mode: a pin on every control; filled = on the face.
    let Some((_, set)) = ui_state.choose.as_ref().filter(|(id, _)| *id == m.id) else {
        return;
    };
    let Some(i) = m.info.params.iter().position(|p| p.name == c.param.name) else {
        return;
    };
    let on = set[i];
    let pr = xf.r(c.geo.pin_rect(&routing::param_label(c.param)));
    ui_state.record(format!("pin:{}.{}", m.id, c.param.name), pr);
    let resp = ui.interact(
        pr,
        Id::new(("kabl-pin", m.id, c.param.name)),
        Sense::click(),
    );
    let resp = resp.on_hover_text(if on {
        "On the face: click to move to the advanced area"
    } else {
        "Advanced: click to put on the face"
    });
    painter.circle_filled(pr.center(), 7.5 * z, if on { th.sel } else { th.panel });
    painter.circle_stroke(pr.center(), 7.5 * z, Stroke::new(1.6, th.sel));
    painter.circle_filled(
        pr.center(),
        2.5 * z,
        if on { Color32::WHITE } else { th.sel },
    );
    if resp.clicked() {
        if let Some((_, set)) = ui_state.choose.as_mut() {
            // Every bank of a sequencer shares the slot's choice.
            let slot = rack::face_name(m.info, c.param.name);
            for (k, p) in m.info.params.iter().enumerate() {
                if rack::face_name(m.info, p.name) == slot {
                    set[k] = !on;
                }
            }
        }
    }
}

/// Short text badge for a jack in Hidden view: where its cables go or come from.
fn jack_badge(
    editor: &PatchEditor,
    id: ModuleId,
    port: &str,
    dir: PortDirection,
) -> Option<String> {
    let state = editor.state();
    let name = |mid: ModuleId| {
        state
            .modules
            .get(&mid)
            .map_or("?", |m| routing::short_name(&m.kind))
    };
    let here = PortRef::Module {
        id,
        port: port.to_string(),
    };
    match dir {
        PortDirection::Output => {
            let dests: Vec<ModuleId> = state
                .cables
                .values()
                .filter(|c| c.from == here)
                .map(|c| c.to.module_id())
                .collect();
            match dests.as_slice() {
                [] => None,
                [one] => Some(format!("> {}", name(*one))),
                [first, ..] if dests.iter().all(|d| d == first) => {
                    Some(format!("> {} x{}", name(*first), dests.len()))
                }
                _ => Some(format!("> {} routes", dests.len())),
            }
        }
        PortDirection::Input => state
            .cables
            .values()
            .find(|c| c.to == here)
            .map(|c| format!("< {}", name(c.from.module_id()))),
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_jacks(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    th: &Theme,
    xf: Xf,
    m: &Placed,
    on_art: bool,
    drawn: &mut Drawn,
) {
    let z = xf.zoom;
    for j in &m.jacks {
        let c = xf.p(j.c);
        let port = j.port;
        drawn
            .ports
            .insert((m.id, port.direction, port.name.to_string()), c);
        let on_plate = m.plate.is_some_and(|p| p.contains(j.c));
        let ink = if on_plate {
            th.plate_ink
        } else if on_art {
            let k = m.skin.map_or([0; 3], |s| s.art_ink[usize::from(th.dark)]);
            Color32::from_rgb(k[0], k[1], k[2])
        } else {
            th.ink
        };
        if m.skin.is_some() && !on_art {
            painter.rect_filled(
                Rect::from_center_size(c + vec2(0.0, -10.0) * z, vec2(44.0, 52.0) * z),
                CornerRadius::same(5),
                th.panel.gamma_multiply(0.96),
            );
        }
        let label =
            macro_name(editor.state(), m.id, port.name).unwrap_or_else(|| port_label(port.name));
        if j.label_right {
            text(
                painter,
                c + vec2(20.0 * z, 0.0),
                egui::Align2::LEFT_CENTER,
                &label,
                12.5 * z,
                ink,
                false,
            );
        } else {
            text(
                painter,
                c - vec2(0.0, 24.0 * z),
                egui::Align2::CENTER_CENTER,
                &label,
                12.5 * z,
                ink,
                false,
            );
        }
        let hit = Rect::from_center_size(c, EguiVec2::splat(((JACK_R + 3.0) * 2.0 * z).max(20.0)));
        let dir_key = match port.direction {
            PortDirection::Input => "in",
            PortDirection::Output => "out",
        };
        ui_state.record(format!("{dir_key}:{}.{}", m.id, port.name), hit);
        let sense = Sense::click_and_drag();
        let resp = ui.interact(
            hit,
            Id::new(("kabl-port", m.id, port.direction, port.name)),
            sense,
        );
        let this_ref = PortRef::Module {
            id: m.id,
            port: port.name.to_string(),
        };
        painter.circle_filled(c, JACK_R * z, th.nut);
        painter.circle_stroke(c, JACK_R * z, Stroke::new(1.0, th.nut_edge));
        let badge = (ui_state.cable_view == CableView::Hidden)
            .then(|| jack_badge(editor, m.id, port.name, port.direction))
            .flatten();
        if badge.is_some() {
            painter.circle_stroke(
                c,
                (JACK_R - 2.0) * z,
                Stroke::new(3.0 * z, th.signal(port.port_type)),
            );
        }
        painter.circle_filled(c, 7.0 * z, th.hole_c);
        if resp.hovered() || ui_state.pending_output.as_ref() == Some(&this_ref) {
            painter.circle_stroke(c, (JACK_R + 3.0) * z, Stroke::new(1.5, th.sel));
        }
        if let Some(b) = badge {
            routing::defer_badge(
                ui_state,
                painter,
                c + vec2(0.0, 23.0 * z),
                &b,
                th.signal(port.port_type),
                z,
            );
        }
        if resp.drag_started() {
            ui_state.pending_output = None;
            match port.direction {
                PortDirection::Output => ui_state.port_drag = Some(this_ref.clone()),
                // Pull the plug out of a patched input: move it elsewhere or drop it on bare
                // rack to remove it (VCV Rack, Voltage Modular).
                PortDirection::Input => {
                    if let Some((&id, c)) =
                        editor.state().cables.iter().find(|(_, c)| c.to == this_ref)
                    {
                        ui_state.port_drag = Some(c.from.clone());
                        ui_state.unplug = Some(id);
                    }
                }
            }
        }
        if resp.drag_stopped() {
            if let (Some(from), Some(at)) =
                (ui_state.port_drag.take(), ui.ctx().pointer_latest_pos())
            {
                drawn.drop_at = Some((from, at));
            }
        }
        if resp.clicked() {
            on_port_click(editor, ui_state, this_ref, port.direction);
        }
    }
}

/// `cutoff_cv` -> `Cutoff CV`, `lp` -> `LP`, `in2` -> `In 2`.
fn port_label(name: &str) -> String {
    name.split('_')
        .map(|w| {
            let w = if w == "resonance" { "res" } else { w };
            let (word, digits) = w.split_at(w.trim_end_matches(|c: char| c.is_ascii_digit()).len());
            let mut s = if word.len() <= 2 && word != "in" {
                word.to_uppercase()
            } else {
                let mut s = word.to_string();
                s[..1].make_ascii_uppercase();
                s
            };
            if !digits.is_empty() {
                s = format!("{s} {digits}");
            }
            s
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The advanced area of a module whose expansion floats over its neighbours. It is its own
/// egui layer, above the rack, so it owns every pointer event in its visible area.
#[allow(clippy::too_many_arguments)]
fn draw_float(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    th: &Theme,
    xf: Xf,
    m: &Placed,
    now: f64,
    drawn: &mut Drawn,
    lay: &Layout,
    canvas: Rect,
) {
    let Some(adv) = m.adv else { return };
    let z = xf.zoom;
    let screen = xf.r(adv);
    let visible = screen.intersect(canvas);
    if !visible.is_positive() {
        return;
    }
    ui_state.record(format!("float:{}", m.id), visible);
    egui::Area::new(Id::new(("kabl-float", m.id)))
        .order(egui::Order::Middle)
        .fixed_pos(visible.min)
        .constrain(false)
        .show(ui.ctx(), |ui| {
            ui.set_clip_rect(visible);
            // Claims the whole visible area: nothing underneath can be pressed through it.
            let _ = ui.allocate_rect(visible, Sense::click_and_drag());
            let painter = ui.painter_at(visible);
            painter.rect_filled(
                screen.translate(vec2(6.0, 8.0) * z),
                CornerRadius::same(6),
                Color32::from_black_alpha(110),
            );
            painter.rect_filled(screen, CornerRadius::same(3), th.panel);
            painter.rect_stroke(
                screen,
                CornerRadius::same(3),
                Stroke::new(1.0, th.panel_edge),
                egui::StrokeKind::Inside,
            );
            text(
                &painter,
                screen.left_top() + vec2(10.0, 10.0) * z,
                egui::Align2::LEFT_TOP,
                "advanced · floating",
                10.0 * z,
                th.ink2,
                false,
            );
            let look = Look {
                z,
                ink: th.ink,
                ink2: th.ink2,
            };
            let mut own = Drawn::default();
            let outer = std::mem::take(&mut ui_state.deferred);
            for c in m.ctls.iter().filter(|c| !c.primary) {
                draw_control(
                    editor, ui_state, ui, &painter, th, xf, m, c, look, false, now, &mut own,
                );
            }
            let deferred = std::mem::replace(&mut ui_state.deferred, outer);
            // Leads into the floating controls, drawn above the float panel.
            let both = Drawn {
                ports: drawn.ports.clone(),
                plugs: own.plugs.clone(),
                drop_at: None,
            };
            draw_cables(
                editor,
                ui_state,
                ui,
                &painter,
                th,
                xf,
                lay,
                &both,
                Some(m.id),
            );
            painter.extend(deferred);
            drawn.plugs.extend(own.plugs);
        });
}

fn on_port_click(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    this_ref: PortRef,
    direction: PortDirection,
) {
    match direction {
        PortDirection::Output => {
            if ui_state.pending_output.as_ref() == Some(&this_ref) {
                ui_state.pending_output = None; // clicking the same output again cancels it.
            } else {
                ui_state.pending_output = Some(this_ref);
            }
        }
        PortDirection::Input => {
            if let Some(from) = ui_state.pending_output.take() {
                editor.connect(from, this_ref);
            }
        }
    }
}

/// Target key under `at`: a knob/selector (`knob:`) or input jack (`in:`). Inside a floating
/// advanced area only that module's targets count.
fn target_at(ui_state: &UiState, at: Pos2, floats: &[(ModuleId, Rect)]) -> Option<String> {
    let only = floats
        .iter()
        .find(|(_, r)| r.contains(at))
        .map(|(id, _)| *id);
    let module_of = |k: &str| {
        k.split_once(':')
            .and_then(|(_, rest)| rest.split_once('.'))
            .and_then(|(mid, _)| mid.parse::<ModuleId>().ok())
    };
    ui_state
        .frame_hits
        .iter()
        .filter(|(k, r)| (k.starts_with("knob:") || k.starts_with("in:")) && r.contains(at))
        .find(|(k, _)| only.is_none_or(|id| module_of(k) == Some(id)))
        .map(|(k, _)| k.clone())
}

/// An output-jack drag released: onto a knob/selector makes a route, onto an input jack a cable.
fn drop_cable(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    from: PortRef,
    at: Pos2,
    floats: &[(ModuleId, Rect)],
) {
    let target = target_at(ui_state, at, floats);
    if let Some(old) = ui_state.unplug.take() {
        let to = target.as_deref().and_then(|key| {
            let (kind, rest) = key.split_once(':')?;
            let (mid, name) = rest.split_once('.')?;
            let id: ModuleId = mid.parse().ok()?;
            Some(match kind {
                "knob" => PortRef::Param {
                    id,
                    param: name.to_string(),
                },
                _ => PortRef::Module {
                    id,
                    port: name.to_string(),
                },
            })
        });
        let new = editor.replug(old, to.clone());
        if let (Some(cable), Some(PortRef::Param { id, param })) = (new, to) {
            let routes = routing::routes_into(editor.state(), id, &param);
            routing::inspect(ui_state, &routes, id, &param);
            ui_state.selected_route = Some(cable);
        }
        return;
    }
    let Some(key) = target else {
        return;
    };
    let (kind, rest) = key.split_once(':').unwrap();
    let (mid, name) = rest.split_once('.').unwrap();
    let mid: ModuleId = mid.parse().unwrap();
    if kind == "knob" {
        let cable = editor.connect_route(from, mid, name);
        let routes = routing::routes_into(editor.state(), mid, name);
        routing::inspect(ui_state, &routes, mid, name);
        ui_state.selected_route = Some(cable);
    } else {
        editor.connect(
            from,
            PortRef::Module {
                id: mid,
                port: name.to_string(),
            },
        );
    }
}

fn draw_port_drag(
    editor: &PatchEditor,
    ui_state: &UiState,
    ui: &egui::Ui,
    th: &Theme,
    drawn: &Drawn,
    floats: &[(ModuleId, Rect)],
) {
    let Some(from) = &ui_state.port_drag else {
        return;
    };
    let (Some(&a), Some(b)) = (
        port_ref_pos(&drawn.ports, from, PortDirection::Output),
        ui.ctx().pointer_latest_pos(),
    ) else {
        return;
    };
    let top = ui
        .ctx()
        .layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            Id::new("kabl-port-drag"),
        ))
        .with_clip_rect(ui_state.canvas);
    cable_path(&top, a, b, th.sel, 4.0 * ui_state.zoom, 1.0, false);
    let hot = target_at(ui_state, b, floats);
    for (key, r) in &ui_state.frame_hits {
        if key.starts_with("knob:") || key.starts_with("in:") {
            let is_hot = hot.as_deref() == Some(key.as_str());
            top.rect_stroke(
                r.expand(2.0),
                4.0,
                Stroke::new(
                    if is_hot { 2.0 } else { 1.0 },
                    th.sel.gamma_multiply(if is_hot { 1.0 } else { 0.4 }),
                ),
                egui::StrokeKind::Outside,
            );
        }
    }
    if let Some(key) = hot.filter(|k| k.starts_with("knob:")) {
        let (mid, name) = key["knob:".len()..].split_once('.').unwrap_or(("", ""));
        let label = mid
            .parse::<ModuleId>()
            .ok()
            .and_then(|mid| editor.state().modules.get(&mid))
            .and_then(|m| registry::info_for(&m.kind))
            .and_then(|i| i.params.iter().find(|p| p.name == name))
            .map_or(name.to_string(), routing::target_label);
        let tip = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            Id::new("kabl-drop-hint"),
        ));
        routing::pill(
            &tip,
            b - EguiVec2::new(0.0, 14.0),
            &format!(
                "Release to modulate {label} ({:+.0} %)",
                kabl_engine::compile::DEFAULT_ROUTE_AMOUNT * 100.0
            ),
        );
    }
}

fn port_ref_pos<'a>(
    ports: &'a HashMap<(ModuleId, PortDirection, String), Pos2>,
    port_ref: &PortRef,
    direction: PortDirection,
) -> Option<&'a Pos2> {
    let PortRef::Module { id, port } = port_ref else {
        return None;
    };
    ports.get(&(*id, direction, port.clone()))
}

/// A hanging patch cable: a cubic with both handles pulled down, sagging more when longer.
fn cable_points(a: Pos2, b: Pos2, z: f32) -> Vec<Pos2> {
    let sag = 30.0 * z + 0.25 * (b - a).length();
    let (c1, c2) = (a + vec2(0.0, sag), b + vec2(0.0, sag));
    (0..=40)
        .map(|i| {
            let t = i as f32 / 40.0;
            let u = 1.0 - t;
            (a.to_vec2() * u * u * u
                + c1.to_vec2() * 3.0 * u * u * t
                + c2.to_vec2() * 3.0 * u * t * t
                + b.to_vec2() * t * t * t)
                .to_pos2()
        })
        .collect()
}

fn cable_path(
    p: &egui::Painter,
    a: Pos2,
    b: Pos2,
    col: Color32,
    w: f32,
    alpha: f32,
    bypass: bool,
) -> Vec<Pos2> {
    let pts = cable_points(a, b, w / 5.5);
    if bypass {
        p.extend(egui::Shape::dashed_line(
            &pts,
            Stroke::new(w, Color32::from_gray(140).gamma_multiply(alpha)),
            7.0,
            5.0,
        ));
    } else {
        p.add(egui::Shape::line(
            pts.clone(),
            Stroke::new(w + 2.0, Color32::from_black_alpha((90.0 * alpha) as u8)),
        ));
        p.add(egui::Shape::line(
            pts.clone(),
            Stroke::new(w, col.gamma_multiply(alpha)),
        ));
    }
    for e in [a, b] {
        p.circle_filled(
            e,
            w * 1.35,
            col.lerp_to_gamma(Color32::BLACK, 0.35)
                .gamma_multiply(alpha),
        );
        p.circle_filled(e, w * 0.9, col.gamma_multiply(alpha));
    }
    pts
}

/// Jack cables and route leads. `only`: just the leads into that module's floating area.
#[allow(clippy::too_many_arguments)]
fn draw_cables(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    th: &Theme,
    xf: Xf,
    lay: &Layout,
    drawn: &Drawn,
    only: Option<ModuleId>,
) {
    if ui_state.cable_view == CableView::Hidden {
        return;
    }
    let z = xf.zoom;
    let focus = ui_state.selected_module;
    let view = ui_state.cable_view;
    let alpha_for = |a: ModuleId, b: ModuleId| match view {
        CableView::Focus if focus != Some(a) && focus != Some(b) => 0.12,
        _ => 1.0,
    };
    // A lead ends inside a floating area when its knob is one of that area's controls.
    let floating = |c: &kabl_core::CableState| match &c.to {
        PortRef::Param { id, param } => lay
            .get(*id)
            .is_some_and(|m| m.overlay && m.ctl(param).is_some_and(|c| !c.primary)),
        _ => false,
    };
    let cables: Vec<(CableId, kabl_core::CableState)> = editor
        .state()
        .cables
        .iter()
        .map(|(&id, c)| (id, c.clone()))
        .collect();
    if only.is_none() {
        for (cable_id, c) in &cables {
            if ui_state.unplug == Some(*cable_id) {
                continue; // in the hand, drawn to the pointer
            }
            let (Some(&a), Some(&b)) = (
                port_ref_pos(&drawn.ports, &c.from, PortDirection::Output),
                port_ref_pos(&drawn.ports, &c.to, PortDirection::Input),
            ) else {
                continue;
            };
            let col = port_color(th, editor, &c.from);
            let alpha = alpha_for(c.from.module_id(), c.to.module_id());
            let pts = cable_path(painter, a, b, col, 5.5 * z, alpha, false);
            // Removing is explicit: pull the plug out of its input, or right-click the cable.
            let mid = pts[pts.len() / 2];
            let hit = Rect::from_center_size(mid, EguiVec2::splat(12.0));
            ui_state.record(format!("cable:{cable_id}"), hit);
            let resp = ui.interact(hit, Id::new(("kabl-cable", *cable_id)), Sense::click());
            if resp.hovered() {
                painter.circle_stroke(mid, 7.0, Stroke::new(1.5, th.sel));
            }
            let resp = resp.on_hover_text("Drag its plug out of the input to move or remove it");
            resp.context_menu(|ui| {
                let r = ui.button("Remove cable");
                ui_state.record("menu:remove-cable".into(), r.rect);
                if r.clicked() {
                    editor.disconnect(*cable_id);
                    ui.close();
                }
            });
        }
    }
    for (cable_id, pos) in &drawn.plugs {
        let Some((_, c)) = cables.iter().find(|(id, _)| id == cable_id) else {
            continue;
        };
        if floating(c) != only.is_some() || only.is_some_and(|id| c.to.module_id() != id) {
            continue;
        }
        let Some(&a) = port_ref_pos(&drawn.ports, &c.from, PortDirection::Output) else {
            continue;
        };
        let bypass = c.params.get("bypass").is_some_and(|&b| b >= 0.5);
        let inspected = matches!((&c.to, &ui_state.inspected),
            (PortRef::Param { id, param }, Some((iid, ip))) if id == iid && param == ip);
        let strong = ui_state.selected_route == Some(*cable_id)
            || inspected
            || focus == Some(c.from.module_id());
        let alpha =
            alpha_for(c.from.module_id(), c.to.module_id()) * if strong { 1.0 } else { 0.72 };
        let pts = cable_path(
            painter,
            a,
            *pos,
            cable_color(*cable_id),
            4.0 * z,
            alpha,
            bypass,
        );
        let mid = pts[pts.len() / 2];
        let hit = Rect::from_center_size(mid, EguiVec2::splat(12.0));
        ui_state.record(format!("route:{cable_id}"), hit);
        let resp = ui.interact(hit, Id::new(("kabl-route", *cable_id)), Sense::click());
        if resp.clicked() {
            // Clicking a route selects it (never deletes: removal is explicit in the drawer).
            if let PortRef::Param { id, param } = &c.to {
                ui_state.selected_module = Some(*id);
                ui_state.inspected = Some((*id, param.clone()));
                ui_state.selected_route = Some(*cable_id);
            }
        }
    }
}

fn port_color(th: &Theme, editor: &PatchEditor, from: &PortRef) -> Color32 {
    let PortRef::Module { id, port } = from else {
        return th.cv;
    };
    editor
        .state()
        .modules
        .get(id)
        .and_then(|m| registry::info_for(&m.kind))
        .and_then(|i| {
            i.ports
                .iter()
                .find(|p| p.name == port && p.direction == PortDirection::Output)
        })
        .map_or(th.cv, |p| th.signal(p.port_type))
}

#[cfg(test)]
mod tests {
    use super::port_label;

    #[test]
    fn port_labels_read_like_panel_text() {
        assert_eq!(port_label("cutoff_cv"), "Cutoff CV");
        assert_eq!(port_label("resonance_cv"), "Res CV");
        assert_eq!(port_label("lp"), "LP");
        assert_eq!(port_label("in3"), "In 3");
        assert_eq!(port_label("velocity"), "Velocity");
        assert_eq!(port_label("in"), "In");
        assert_eq!(port_label("a"), "A");
    }
}
