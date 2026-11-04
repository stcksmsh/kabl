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

pub mod about;
pub mod banks;
pub mod browser;
mod cable_look;
pub mod compare;
pub mod composites;
pub mod control;
pub mod cues;
pub mod displays;
mod drawer;
mod drum_displays;
mod drum_grid;
pub mod editor;
pub mod explain;
pub mod filebrowser;
mod fm6;
mod fx_displays;
pub mod help;
pub mod inspect;
pub mod kit;
pub mod library;
pub mod panels;
mod pattern;
#[cfg(test)]
mod pattern_tests;
pub mod perform;
mod perform_view;
pub mod rack;
mod rack_editor;
mod wheel;
use wheel::OwnedScroll;
pub mod recipes;
pub mod record;
pub mod routing;
mod seq_editor;
pub mod style;
pub mod tables;
pub mod theme;
#[cfg(test)]
mod theme_tests;

pub use editor::PatchEditor;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use egui::{pos2, vec2, Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke, Vec2 as EguiVec2};
use kabl_core::{CableId, ModuleId, PatchState, PortRef, Vec2};
use kabl_engine::patch_engine::Command;
use kabl_modules::builtins::{seq, DelayLock, LfoSync, Transport};
use kabl_modules::info::PortDirection;
use kabl_modules::registry;
use kit::Tip;
use rack::{Decor, Geo, Layout, Placed, JACK_R, PANEL_H};
use routing::Look;
use style::Style;
use theme::{theme, Theme};

/// Width of the routing drawer (right).
pub const DRAWER_W: f32 = 320.0;
/// Window width below which (the plugin's 1180 px) the drawer floats over the rack and the
/// rack fits itself to the window.
pub const COMPACT_W: f32 = 1280.0;
pub const MIN_ZOOM: f32 = 0.5;
pub const MAX_ZOOM: f32 = 2.0;
/// Largest zoom the automatic fit may pick: small patches fill the window, as in the mockup.
const FIT_MAX_ZOOM: f32 = 1.5;

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

/// One macro a cue drives: where it is, where it is heading and how far along (0..=1; 1 =
/// arrived, held there).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MacroLive {
    pub value: f32,
    pub to: f32,
    pub progress: f32,
}

impl UiState {
    /// What the audio thread says about macro `k` of `id`, while its reports keep coming.
    pub fn live_macro(&self, id: ModuleId, k: u8) -> Option<MacroLive> {
        let (m, at) = self.macro_live.get(&(id, k))?;
        (at.elapsed() < std::time::Duration::from_millis(400)).then_some(*m)
    }

    /// Where param `p` of `id` is now: a macro a cue is driving reads as the cue moved it,
    /// anything else as stored.
    pub fn param_now(&self, state: &PatchState, id: ModuleId, p: &kabl_modules::ParamInfo) -> f32 {
        let live = state
            .modules
            .get(&id)
            .filter(|m| m.kind == "macro")
            .and_then(|_| p.name.strip_prefix('m')?.parse::<u8>().ok())
            .and_then(|k| self.live_macro(id, k.checked_sub(1)?));
        live.map_or_else(|| routing::base_value(state, id, p), |m| m.value)
    }

    /// Where macro `k` of `id` is now: the cue's morph if one is driving it, else the knob.
    pub fn macro_now(&self, state: &PatchState, id: ModuleId, k: u8) -> f32 {
        self.live_macro(id, k).map_or_else(
            || {
                let p = &kabl_modules::builtins::MACRO_INFO.params[k as usize];
                routing::base_value(state, id, p)
            },
            |m| m.value,
        )
    }
}

/// Interaction and view state that is not part of the patch.
pub struct UiState {
    pub composites: composites::View,
    pub selected_kind: String,
    pub selected_module: Option<ModuleId>,
    pub(crate) selected_composite: Option<kabl_core::CompositeId>,
    pub(crate) scope: Option<kabl_core::CompositeId>,
    pub(crate) navigation: Vec<rack_editor::ReturnView>,
    face_order: Option<Vec<usize>>,
    navigation_editor: u64,
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
    /// Messages as toasts (errors stay, warnings 8 s, success 3 s); fed from `last_message`.
    pub toasts: Vec<kit::g4::Toast>,
    /// The `last_message` text already turned into a toast.
    toast_seen: Option<String>,
    /// The empty rack's Add module button asks the toolbar to open its menu.
    add_menu_req: bool,
    pub cable_view: CableView,
    /// The knob (module, param) whose routes the drawer lists and whose ring edits
    /// `selected_route`.
    pub inspected: Option<(ModuleId, String)>,
    /// The route the ring and the drawer's highlight refer to. Never re-pointed silently: it is
    /// cleared when that route disappears, and only set by an explicit pick or by inspecting a
    /// knob that has exactly one route.
    pub selected_route: Option<CableId>,
    /// The cable whose pattern and probability the drawer edits (`routing::cable_panel`).
    pub cable_fn: Option<CableId>,
    /// The About box.
    pub about: about::About,
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
    /// Each clock's position in 16ths since its last restart, as the audio thread reports it.
    pub clock_pos: HashMap<ModuleId, f64>,
    /// Macros a cue is moving or holding, as the audio thread reports them: `(macro module,
    /// output)` to what was last reported and when. The stored knob value is the hand's.
    pub macro_live: HashMap<(ModuleId, u8), (MacroLive, std::time::Instant)>,
    /// The cue last launched on each `cues` module: the one a macro-only cue (no sequencer to
    /// show it playing) counts as current, and the one "Store macros" writes to.
    pub last_cue: HashMap<ModuleId, usize>,
    /// What the engine's face taps last reported, for the live displays.
    pub live: displays::Live,
    pub wants: displays::Wants,
    /// Host owns clock transport in the plugin; standalone defaults to Free.
    pub host_clock: bool,
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
    /// Dark mode of the chosen theme.
    pub dark: bool,
    /// Id of the chosen theme in `themes`.
    pub theme: String,
    /// Every theme the user can pick: the built-ins plus those in the themes folder.
    pub themes: std::sync::Arc<style::Themes>,
    /// Problems found while loading user themes, shown in the settings menu.
    pub theme_notes: Vec<String>,
    /// Where the choice is remembered; `None` (the default) never touches the disk.
    pub prefs_dir: Option<std::path::PathBuf>,
    prefs_saved: Option<style::Prefs>,
    /// The active style, rebuilt when `theme` or `dark` changes.
    pub style: std::sync::Arc<style::Style>,
    pub panels: panels::View,
    pub tables: tables::View,
    /// The shared file browser window, while one is open (`filebrowser`).
    pub files: Option<filebrowser::Request>,
    /// What it picked, for the place that asked.
    pub picked: Option<filebrowser::Picked>,
    /// The folder last used per place, remembered in `settings.toml`.
    pub folders: std::collections::BTreeMap<String, std::path::PathBuf>,
    /// `folders` changed since it was last written to the settings file.
    folders_dirty: bool,
    pub zoom: f32,
    /// Screen offset of the rack origin from the canvas' top-left.
    pub pan: EguiVec2,
    pub drawer_open: bool,
    /// The user pinned the drawer: entering the full Perform view leaves it open.
    pub drawer_pinned: bool,
    /// Perform closed the drawer on entry; leaving the full view reopens it.
    drawer_auto_closed: bool,
    perform_full_seen: bool,
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
    /// World ends of every drawn cable (output, input), from the last frame: what the fit
    /// needs to keep the sagging ropes inside the viewport.
    cable_ends: Vec<(Pos2, Pos2)>,
    /// Paint audit of the last rack frame: the clip rect the cables were painted with, the
    /// shape index where module content ended, where the cables started and where they ended.
    pub cable_audit: (Rect, usize, usize, usize),
    /// A fit of the whole patch that still has to account for the cables: (world, max zoom).
    pending_fit: Option<(Rect, f32)>,
    /// Compact window (below `COMPACT_W`), set each frame.
    compact: bool,
    /// Compact windows refit the rack on resize until the user zooms.
    pub auto_fit: bool,
    compact_seen: bool,
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
    /// MIDI input ports seen, the connected one, and a switch the user asked for.
    pub midi_inputs: Vec<String>,
    pub midi_input: Option<String>,
    /// `Some(None)` = disconnect.
    pub midi_select: Option<Option<String>>,
    /// Final output meter (fed by `main.rs` from the audio callback).
    pub meter: record::Meter,
    /// The host feeds `meter` and `sample_rate` (standalone): the status bar shows them.
    pub status_meter: bool,
    /// The host's audio status line (device, rate, buffer), shown in the status bar.
    pub status_audio: String,
    /// The drawer's Help, Compare and Learn switches are folded away until "More" is pressed.
    pub drawer_more: bool,
    /// Status bar readouts, each shown only when set. The host feeds the DSP load in percent,
    /// the voices in use and available, and the callback size in frames; `show` derives the
    /// tempo and the song position (bar . beat . tick) from the patch's first clock.
    pub status_cpu: Option<f32>,
    pub status_voices: Option<(u32, u32)>,
    pub status_buffer: Option<u32>,
    pub status_bpm: Option<f32>,
    pub status_pos: Option<String>,
    /// Stereo output recorder; `None` without an audio device.
    pub recorder: Option<record::Recorder>,
    /// The open "what does this control change" explanation and inline help (view only).
    pub explain: explain::Explain,
    /// The selected signal and its measurements (D03, view only).
    pub inspect: inspect::Inspect,
    /// The comparison reference (D03, session only).
    pub compare: compare::Compare,
    /// The listening recipes (D03, view only).
    pub recipes: recipes::Learn,
}

struct Moving {
    item: rack_editor::Item,
    grab: EguiVec2,
    live: Vec2,
    origin: Pos2,
    width: f32,
    cancelled: bool,
}

impl Default for UiState {
    fn default() -> Self {
        UiState {
            composites: composites::View::default(),
            selected_kind: PatchEditor::known_kinds()
                .first()
                .copied()
                .unwrap_or("osc.va")
                .to_string(),
            selected_module: None,
            selected_composite: None,
            scope: None,
            navigation: Vec::new(),
            face_order: None,
            navigation_editor: 0,
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
            toasts: Vec::new(),
            toast_seen: None,
            add_menu_req: false,
            cable_view: CableView::All,
            inspected: None,
            selected_route: None,
            cable_fn: None,
            about: about::About::default(),
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
            clock_pos: HashMap::new(),
            macro_live: HashMap::new(),
            last_cue: HashMap::new(),
            live: displays::Live::default(),
            wants: displays::Wants::default(),
            host_clock: false,
            transport: Vec::new(),
            delay_status: HashMap::new(),
            lfo_status: HashMap::new(),
            loaded: false,
            dark: false,
            theme: style::BUILTIN_A_ID.into(),
            themes: std::sync::Arc::default(),
            theme_notes: Vec::new(),
            prefs_dir: None,
            prefs_saved: None,
            style: std::sync::Arc::new(style::builtin_a(false)),
            panels: panels::View::default(),
            tables: tables::View::default(),
            files: None,
            picked: None,
            folders: std::collections::BTreeMap::new(),
            folders_dirty: false,
            zoom: 1.0,
            pan: EguiVec2::ZERO,
            drawer_open: true,
            drawer_pinned: false,
            drawer_auto_closed: false,
            perform_full_seen: false,
            expanded: BTreeSet::new(),
            float_expansion: false,
            skins: true,
            skin_labels_on_art: None,
            choose: None,
            flash: None,
            reveal: None,
            canvas: Rect::NOTHING,
            fitted: false,
            cable_ends: Vec::new(),
            cable_audit: (Rect::NOTHING, 0, 0, 0),
            pending_fit: None,
            compact: false,
            auto_fit: true,
            compact_seen: false,
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
            midi_inputs: Vec::new(),
            midi_input: None,
            midi_select: None,
            recorder: None,
            meter: Default::default(),
            status_meter: false,
            status_audio: String::new(),
            drawer_more: false,
            status_cpu: None,
            status_buffer: None,
            status_voices: None,
            status_bpm: None,
            status_pos: None,
            explain: Default::default(),
            inspect: Default::default(),
            compare: Default::default(),
            recipes: Default::default(),
        }
    }
}

impl UiState {
    pub fn record(&mut self, key: String, rect: Rect) {
        self.frame_hits.insert(key, rect);
    }

    /// Drops selections that point at things undo, delete or load removed.
    fn validate(&mut self, editor: &PatchEditor) {
        let state = editor.state();
        if self.navigation_editor != editor.instance() {
            self.navigation_editor = editor.instance();
            self.navigation.clear();
            self.scope = None;
            self.selected_composite = None;
            self.moving = None;
            self.expanded.clear();
            self.fitted = false;
        }
        while self
            .scope
            .is_some_and(|id| !state.composites.contains_key(&id))
        {
            if self.navigation.is_empty() {
                self.scope = None;
                self.fitted = false;
                break;
            }
            self.back();
        }
        if self
            .selected_composite
            .is_some_and(|id| !state.composites.contains_key(&id))
        {
            self.selected_composite = None;
        }
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
                    (PortRef::Param { id, param }, Some((iid, ip)))
                        if id == iid && routing::reaches(state, *id, param, ip))
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
        self.auto_fit = false;
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan = at - self.canvas.min - world.to_vec2() * self.zoom;
    }

    /// Zooms so `world` fits the canvas (at most `max_zoom`), anchored top-left under the toolbar.
    fn frame_world(&mut self, world: Rect, max_zoom: f32) {
        if !world.min.x.is_finite()
            || !world.min.y.is_finite()
            || !world.max.x.is_finite()
            || !world.max.y.is_finite()
            || world.width() <= 0.0
            || world.height() <= 0.0
        {
            self.zoom = 1.0_f32.min(max_zoom).max(MIN_ZOOM);
            self.pan = vec2(12.0, 12.0);
            return;
        }
        let c = self.canvas.shrink(12.0);
        let z = (c.width() / world.width())
            .min(c.height() / world.height())
            .clamp(MIN_ZOOM, max_zoom);
        self.zoom = z;
        self.pan = c.min - self.canvas.min - world.min.to_vec2() * z;
    }

    /// `frame_world` for the whole patch: the module fit now, refined with the cable ropes
    /// once they are drawn (`refit_with_cables`).
    fn frame_patch(&mut self, world: Rect, max_zoom: f32) {
        self.frame_world(world, max_zoom);
        self.pending_fit = Some((world, max_zoom));
    }

    /// Screen bounds of `world` and every cable rope at the current zoom and pan.
    fn rack_bounds(&self, world: Rect) -> Rect {
        let xf = self.xf();
        let sag = self.style.slots.cable.sag;
        let mut r = xf.r(world);
        for (a, b) in &self.cable_ends {
            for p in cable_look::points(xf.p(*a), xf.p(*b), xf.zoom, sag) {
                r.extend_with(p);
            }
        }
        r
    }

    /// Shrinks the fit until the modules and the sagging cables are inside the canvas (never
    /// below `MIN_ZOOM`), anchored top-left under the toolbar.
    fn refit_with_cables(&mut self, world: Rect, max_zoom: f32) {
        let c = self.canvas.shrink(12.0);
        for _ in 0..8 {
            let r = self.rack_bounds(world);
            if r.width() <= c.width() + 0.5 && r.height() <= c.height() + 0.5 {
                break;
            }
            let k = (c.width() / r.width()).min(c.height() / r.height()) * 0.99;
            self.zoom = (self.zoom * k).clamp(MIN_ZOOM, max_zoom);
        }
        let r = self.rack_bounds(world);
        self.pan += c.min - r.min;
    }

    /// Screen bounds of every drawn cable rope (none when no cable is drawn).
    pub fn cable_bounds(&self) -> Option<Rect> {
        let xf = self.xf();
        let sag = self.style.slots.cable.sag;
        let mut r: Option<Rect> = None;
        for (a, b) in &self.cable_ends {
            for p in cable_look::points(xf.p(*a), xf.p(*b), xf.zoom, sag) {
                match r.as_mut() {
                    Some(r) => r.extend_with(p),
                    None => r = Some(Rect::from_min_max(p, p)),
                }
            }
        }
        r
    }

    /// The rack viewport of the last frame, screen px.
    pub fn rack_canvas(&self) -> Rect {
        self.canvas
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
            expanded: None,
            float: true,
            skins: self.skins,
            choose: None,
            moving: None,
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

    /// Read the user themes folder and the remembered theme choice from the per-user config
    /// folder (`style::config_dir`), and remember later changes there. The choice is global
    /// to the user: the standalone and every plugin instance share it, because look is a
    /// personal preference and songs do not carry it.
    pub fn use_user_prefs(&mut self) {
        let dir = style::config_dir();
        let (themes, notes) = style::Themes::with_user_dir(&dir.join("themes"));
        for n in &notes {
            log::warn!("theme: {n}");
        }
        let mut p = style::Prefs::load(&dir);
        // `KABL_THEME=ID` or `ID:dark` / `ID:light` starts in that theme without changing the
        // remembered choice until the user picks one (capture scripts, bug reports).
        if let Ok(v) = std::env::var("KABL_THEME") {
            let (id, mode) = v.split_once(':').unwrap_or((&v, ""));
            if themes.get(id).is_some() {
                p.theme = id.into();
                p.dark = mode == "dark" || (mode != "light" && p.dark);
            } else {
                log::warn!("KABL_THEME={v}: no such theme");
            }
        }
        self.themes = std::sync::Arc::new(themes);
        self.theme_notes = notes;
        self.theme.clone_from(&p.theme);
        self.dark = p.dark;
        // A remembered folder that is gone is dropped: its place starts in home again.
        self.folders = p.folders.clone();
        self.folders.retain(|_, d| d.is_dir());
        self.prefs_saved = Some(p);
        self.prefs_dir = Some(dir);
    }

    fn save_prefs(&mut self) {
        let Some(dir) = &self.prefs_dir else { return };
        let p = style::Prefs {
            theme: self.theme.clone(),
            dark: self.dark,
            folders: self.folders.clone(),
        };
        if self.prefs_saved.as_ref() != Some(&p) {
            if let Err(e) = p.save(dir) {
                log::warn!("could not remember the theme in {}: {e}", dir.display());
            }
            self.prefs_saved = Some(p);
        }
    }

    /// Remembers `dir` as the last folder of `place`.
    pub fn folders_remember(&mut self, place: filebrowser::Place, dir: &std::path::Path) {
        if self.folders.get(place.key()).map(|d| d.as_path()) != Some(dir) {
            self.folders.insert(place.key().into(), dir.to_path_buf());
            self.folders_dirty = true;
        }
    }

    fn flash_on(&self, id: ModuleId, param: &str, now: f64) -> bool {
        matches!(self.flash, Some((fid, p, t)) if fid == id && p == param && now - t < 1.5)
    }
}

/// Draws the rack editor for one frame and applies user edits to `editor` directly. Call once
/// per frame from `eframe::App::ui`.
pub fn show(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui) {
    show_with(editor, ui_state, ui, |_, _| {});
}

/// `show`, with the host's own controls at the right end of the status bar (audio device,
/// diagnostics). The host's closure draws right to left.
pub fn show_with(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    status_right: impl FnOnce(&mut egui::Ui, &mut UiState),
) {
    wheel::begin(ui.ctx());
    theme::ensure_fonts(ui.ctx());
    let ready = theme::fonts_ready(ui.ctx());
    if ui_state.style.dark != ui_state.dark
        || ui_state.style.id != ui_state.theme
        || ui_state.style.fonts_ready != ready
    {
        let th = ui_state.themes.resolve(&ui_state.theme);
        ui_state.style = std::sync::Arc::new(Style {
            fonts_ready: ready,
            ..th.style(ui_state.dark).clone()
        });
        ui_state.theme = ui_state.style.id.clone();
        ui_state.save_prefs();
    }
    if std::mem::take(&mut ui_state.folders_dirty) {
        ui_state.save_prefs();
    }
    let st = ui_state.style.clone();
    kit::apply(ui.ctx(), &st);
    composites::panel(editor, ui_state, ui.ctx());
    about::show(ui.ctx(), ui_state);
    panels::editor(editor, ui_state, ui.ctx());
    filebrowser::window(ui_state, ui.ctx());
    rack_editor::edit_face(editor, ui_state, ui.ctx());
    browser::frame_input(editor, ui_state, ui);
    ui_state.validate(editor);
    ui_state.explain.validate(editor);
    ui_state.compare.validate(editor);
    recipes::frame(editor, ui_state);
    let now = ui.input(|i| i.time);
    if let Some(c) = ui_state.inspect.frame(editor, ui_state.drawer_open, now) {
        ui_state.launches.push(c);
    }
    let audio = ui_state.inspect.audio;
    if let Some(c) = ui_state.wants.frame(audio, now) {
        ui_state.launches.push(c);
    }
    if ui_state.wants.active {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(33));
    }
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
    let th = theme(&ui_state.style);
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
        && !egui::Popup::is_any_open(ui.ctx())
        // egui drops a text field's focus on Escape before this frame runs: that Escape
        // was the field's.
        && !ui_state.explain.typing
    {
        ui_state.explain.close();
    }
    // MIDI CCs are applied by the control layer (`control::midi`), not here: pickup only
    // follows edits made in this frame.
    perform::sync_takeover(editor, ui_state);
    if !ui.input(|i| i.pointer.any_down()) {
        ui_state.drag = None;
    }
    if !ui.ctx().egui_wants_keyboard_input()
        && ui_state.browser.dialog.is_none()
        && ui_state.moving.is_none()
    {
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
        .exact_size(st.metrics.toolbar_h)
        .frame(kit::bar_frame(&st))
        .show(ui, |ui| toolbar(editor, ui_state, ui, &st));
    if let Some((bpm, pos)) = transport_readout(editor.state(), ui_state) {
        // Under a host clock the module's own tempo is not the one playing.
        ui_state.status_bpm = (!ui_state.host_clock).then_some(bpm);
        ui_state.status_pos = Some(pos);
    }
    egui::Panel::bottom("kabl-status")
        .exact_size(st.metrics.status_h)
        .frame(kit::bar_frame(&st))
        .show(ui, |ui| status_bar(ui_state, ui, &st, status_right));
    egui::Panel::right("kabl-rail")
        .exact_size(st.metrics.rail_w)
        .resizable(false)
        .frame(kit::panel_frame(&st).inner_margin(egui::Margin::symmetric(0, st.sp(2) as i8)))
        .show(ui, |ui| rail(editor, ui_state, ui, &st));

    let compact = ui.max_rect().width() < COMPACT_W;
    ui_state.compact = compact;
    if compact && !std::mem::replace(&mut ui_state.compact_seen, true) {
        // The plugin window starts with the drawer shut: the rack gets the room.
        ui_state.drawer_open = false;
    }
    let perform_full = ui_state.perform_open && ui_state.perform_tall;
    // The drawer gives its 320 px to the Perform view unless pinned, and comes back with the rack.
    if perform_full != ui_state.perform_full_seen {
        ui_state.perform_full_seen = perform_full;
        if perform_full && ui_state.drawer_open && !ui_state.drawer_pinned {
            ui_state.drawer_open = false;
            ui_state.drawer_auto_closed = true;
        } else if !perform_full && std::mem::take(&mut ui_state.drawer_auto_closed) {
            ui_state.drawer_open = true;
        }
    }
    if ui_state.perform_open && !perform_full {
        egui::Panel::bottom("kabl-perform")
            .exact_size(perform::PANEL_H)
            .resizable(false)
            .frame(kit::panel_frame(&st))
            .show(ui, |ui| perform_view::panel(editor, ui_state, ui));
    }
    // The Sounds scene has its own detail column: the drawer waits behind it.
    if ui_state.drawer_open && !ui_state.browser_open {
        if compact {
            // Below 1280 px the drawer floats over the rack instead of splitting the window.
            let avail = ui.available_rect_before_wrap();
            let frame = kit::panel_frame(&st).shadow(egui::Shadow {
                offset: [-6, 0],
                blur: 16,
                spread: 0,
                color: egui::Color32::from_black_alpha(70),
            });
            let rect = egui::Rect::from_min_max(
                egui::pos2(avail.right() - DRAWER_W, avail.top()),
                avail.max,
            );
            egui::Area::new(egui::Id::new("kabl-params-overlay"))
                .order(egui::Order::Foreground)
                .fade_in(false)
                .fixed_pos(rect.min)
                .show(ui.ctx(), |ui| {
                    frame.show(ui, |ui| {
                        ui.set_min_size(rect.size() - frame.total_margin().sum());
                        ui.set_max_size(rect.size() - frame.total_margin().sum());
                        drawer(editor, ui_state, ui, &st, now);
                    });
                });
        } else {
            egui::Panel::right("kabl-params")
                .exact_size(DRAWER_W)
                .resizable(false)
                .frame(kit::panel_frame(&st))
                .show(ui, |ui| drawer(editor, ui_state, ui, &st, now));
        }
    }

    let page = ui.available_rect_before_wrap();
    // The wavetable dock spans the rack's width, under the rack.
    if !ui_state.browser_open && !perform_full {
        tables::show(editor, ui_state, ui);
    }
    if ui_state.browser_open {
        egui::CentralPanel::default()
            .frame(kit::panel_frame(&st))
            .show(ui, |ui| browser::panel(editor, ui_state, ui));
    } else if perform_full {
        // The Perform view's cards sit 16 from the window edges (mockup `performScreen`).
        egui::CentralPanel::default()
            .frame(kit::page_frame(&st).inner_margin(egui::Margin::same(16)))
            .show(ui, |ui| perform_view::panel(editor, ui_state, ui));
    } else {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(st.roles.rack))
            .show(ui, |ui| show_rack(editor, ui_state, ui, &th));
    }
    browser::dialogs(editor, ui_state, ui.ctx());
    show_toasts(ui_state, ui, &st, page);
    ui_state.explain.typing =
        ui.ctx().egui_wants_keyboard_input() || egui::Popup::is_any_open(ui.ctx());
    ui_state.hits = std::mem::take(&mut ui_state.frame_hits);
}

/// The routing drawer: its header (help, inspect, compare, learn, close) and the scrolling
/// sections below it.
fn drawer(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    now: f64,
) {
    drawer::tabs(ui_state, ui, st);
    if ui_state.recipes.open {
        // Above the drawer's scrolling content, so Show (which scrolls the drawer
        // to the routes) never scrolls the recipe away.
        let h = ui.available_height() * 0.4;
        egui::ScrollArea::vertical()
            .id_salt("kabl-recipes")
            .max_height(h)
            .show_owned(ui, |ui| {
                ui.set_max_width(DRAWER_W - 24.0);
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    recipes::panel(editor, ui_state, ui, now);
                });
            });
    }
    if ui_state.explain.is_open() {
        // Its own scroll area, at most about half the drawer: the module and
        // routing sections below stay in reach while it is open.
        let h = ui.available_height() * 0.55;
        egui::ScrollArea::vertical()
            .id_salt("kabl-explain")
            .max_height(h)
            .show_owned(ui, |ui| {
                ui.set_max_width(DRAWER_W - 24.0);
                explain::panel(editor, ui_state, ui);
            });
    }
    egui::ScrollArea::vertical().show_owned(ui, |ui| {
        // Rows wrap instead of widening the drawer over the rack.
        ui.set_max_width(DRAWER_W - 24.0);
        if ui_state.inspect.open {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                inspect::panel(editor, ui_state, ui, now);
            });
        }
        if ui_state.compare.open {
            compare::panel(editor, ui_state, ui, st);
        }
        // The patch-wide lists lead when no control is being edited, and follow its editor
        // otherwise, so the control's routes stay in view.
        let lists = !ui_state.explain.is_open() && !ui_state.inspect.open;
        // The seq and cues editors count as editing: they lead the drawer, as in G4.
        let module_editor = ui_state
            .selected_module
            .and_then(|id| editor.state().modules.get(&id))
            .is_some_and(|m| matches!(m.kind.as_str(), "seq" | "cues"));
        let editing = ui_state.inspected.is_some() || ui_state.cable_fn.is_some() || module_editor;
        if lists && !editing {
            drawer::lists(editor, ui_state, ui, st);
            drawer::why_card(editor, ui_state, ui, st);
        }
        // The module's own editor lives under Inspect; the faces edit values in place. The
        // seq and cues editors (G4) also open under Routing when their module is selected.
        if ui_state.inspect.open || module_editor {
            show_param_panel(editor, ui_state, ui);
        }
        pattern::panel(editor, ui_state, ui);
        routing::drawer(editor, ui_state, ui);
        // An open cable editor (G4) shows its own routes; the patch-wide ones and the lists
        // wait behind its More disclosure.
        let cable_open = ui_state
            .cable_fn
            .is_some_and(|c| !pattern::more_open(ui.ctx(), c));
        if !cable_open {
            routing::cable_routes(editor, ui_state, ui);
        }
        // The seq and cues editors take the whole drawer (G4): no lists under them.
        if lists && editing && !module_editor && !cable_open {
            drawer::lists(editor, ui_state, ui, st);
            drawer::why_card(editor, ui_state, ui, st);
        }
    });
}

/// The world rectangle to fit: the patch's modules, or two empty rows when there are none.
fn fit_world(bounds: Rect) -> Rect {
    if bounds.is_positive() {
        bounds.expand(4.0)
    } else {
        Rect::from_min_max(
            pos2(0.0, rack::row_y(0) - 8.0),
            pos2(1200.0, rack::row_y(1) + PANEL_H + 8.0),
        )
    }
}

/// The empty rack: the rails stay, a centred note offers the two ways to start.
fn empty_rack(ui_state: &mut UiState, ui: &mut egui::Ui, canvas: Rect) {
    let st = ui_state.style.clone();
    let area = Rect::from_center_size(canvas.center() - vec2(0.0, 20.0), vec2(420.0, 240.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
        kit::g4::empty_state(
            ui,
            &st,
            kit::Ic::Plug,
            "Empty rack",
            "Add a module, or start from a sound.",
            |ui| {
                let add = ui.add(
                    kit::Button::new(&st, "Add module")
                        .icon(kit::Ic::Plus)
                        .primary(),
                );
                ui_state.record("empty-add".into(), add.rect);
                if add.clicked() {
                    ui_state.add_menu_req = true;
                }
                let browse = ui.add(
                    kit::Button::new(&st, "Browse sounds")
                        .icon(kit::Ic::Search)
                        .g4(32.0),
                );
                ui_state.record("empty-browse".into(), browse.rect);
                if browse.clicked() {
                    ui_state.browser_open = true;
                }
            },
        );
        ui.add_space(14.0);
        ui.vertical_centered(|ui| {
            kit::label(
                ui,
                &st,
                style::Role::Caption,
                kit::Tone::Text3,
                "Tip: right-click the rack to add a module where you click.",
            );
        });
    });
}

/// Severity, title and mono detail of a status message: what failed is an error, a hazard a
/// warning, anything else a quiet success. "Could not load: why" splits at the colon.
fn toast_for(msg: &str) -> (kit::g4::Sev, String, Option<String>) {
    use kit::g4::Sev;
    let m = msg.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| m.contains(w));
    let sev = if has(&[
        "error",
        "failed",
        "could not",
        "couldn't",
        "cannot",
        "can't",
        "not found",
        "unsupported",
        "refused",
        "denied",
        "invalid",
        "unavailable",
        "lost",
    ]) {
        Sev::Bad
    } else if has(&[
        "warning", "no sound", "stopped", "busy", "muted", "skipped", "ignored",
    ]) {
        Sev::Warn
    } else {
        Sev::Good
    };
    match msg.split_once(": ") {
        Some((t, d)) => (sev, t.to_string(), Some(d.to_string())),
        None => (sev, msg.to_string(), None),
    }
}

/// Action links of a toast: a failed load can show its detail or open another sound.
fn toast_actions(sev: kit::g4::Sev, title: &str, has_detail: bool) -> Vec<String> {
    let mut a = vec![];
    if sev == kit::g4::Sev::Bad && has_detail {
        a.push("Show details".to_string());
        if title.to_lowercase().contains("load") {
            a.push("Open another…".to_string());
        }
    }
    a
}

/// Turns a new `last_message` into a toast and draws the stack at the rack's top right.
fn show_toasts(ui_state: &mut UiState, ui: &mut egui::Ui, st: &Style, area: Rect) {
    let now = ui.input(|i| i.time);
    if ui_state.last_message != ui_state.toast_seen {
        ui_state.toast_seen.clone_from(&ui_state.last_message);
        if let Some(msg) = &ui_state.last_message {
            let (sev, title, detail) = toast_for(msg);
            let actions = toast_actions(sev, &title, detail.is_some());
            ui_state.toasts.push(kit::g4::Toast {
                sev,
                title,
                detail,
                actions,
                expanded: false,
                born: now,
            });
        }
    }
    if ui_state.toasts.is_empty() {
        return;
    }
    if let Some((i, a)) = kit::g4::toasts(ui.ctx(), st, area, &mut ui_state.toasts, now) {
        let t = &mut ui_state.toasts[i];
        match t.actions.remove(a).as_str() {
            "Show details" => t.expanded = true,
            _ => {
                ui_state.toasts.remove(i);
                ui_state.browser_open = true;
            }
        }
    }
    if ui_state.toasts.iter().any(|t| t.sev.ttl().is_some()) {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(500));
    }
}

fn vsep(ui: &mut egui::Ui, st: &Style) {
    let (r, _) = ui.allocate_exact_size(
        egui::vec2(1.0, st.metrics.control_h - 8.0),
        egui::Sense::hover(),
    );
    ui.painter().rect_filled(r, 0.0, st.roles.line2);
}

fn tool_icon(
    ui: &mut egui::Ui,
    st: &Style,
    ui_state: &mut UiState,
    key: &str,
    ic: kit::Ic,
    tip: &str,
    enabled: bool,
) -> bool {
    let r = kit::icon_button(ui, st, ic, false, enabled).tip(st, tip);
    ui_state.record(key.to_string(), r.rect);
    r.clicked()
}

/// Which scene the toolbar shows controls for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scene {
    Rack,
    Perform,
    Sounds,
}

/// The brand: the rebuilt kabl lockup in the text colour, 32 px high (G4 logo mockup,
/// "Redraw lockup, text colour"); orange is for the app icon only.
fn brand(ui: &mut egui::Ui, st: &Style) {
    kit::logo(ui, st, 32.0);
}

/// Adds a module of `kind` at the end of the first row and selects it.
fn add_module_end(editor: &mut PatchEditor, ui_state: &mut UiState, kind: &str) {
    let lay = composite_layout(editor.state(), ui_state);
    let x = rack_editor::items(&lay)
        .into_iter()
        .filter(|(_, r)| rack::row_of(r.top()) == 0)
        .map(|(_, r)| r.right())
        .fold(rack::RACK_X, f32::max);
    let id = editor.add_module_in_scope(kind, Vec2 { x, y: rack::ROW_Y0 }, ui_state.scope);
    ui_state.selected_module = Some(id);
    ui_state.reveal = Some(id);
}

/// "Add module" search field with its key hint; the popup lists the module kinds that match.
fn add_search(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    wide: bool,
) {
    let w = if wide { 180.0 } else { st.metrics.control_h };
    let (rect, r) =
        ui.allocate_exact_size(egui::vec2(w, st.metrics.control_h), egui::Sense::click());
    let radius = st.shape_radius(st.slots.controls.field, rect.height());
    let hover = ui.ctx().animate_bool_with_time(
        r.id.with("hover"),
        r.hovered(),
        Style::secs(st.motion.hover),
    );
    let p = ui.painter();
    p.rect_filled(rect, radius, st.roles.surface);
    p.rect_stroke(
        rect,
        radius,
        egui::Stroke::new(1.0, style::mix(st.roles.line, st.roles.text3, hover)),
        egui::StrokeKind::Inside,
    );
    let ic_c = egui::pos2(rect.left() + st.sp(2) + 6.0, rect.center().y);
    if !wide {
        kit::icon(
            p,
            kit::Ic::Search,
            rect.center(),
            st.metrics.icon,
            st.roles.text2,
        );
    } else {
        kit::icon(p, kit::Ic::Search, ic_c, st.metrics.icon, st.roles.text3);
        let g = p.layout_no_wrap(
            "Add module".into(),
            st.font(style::Role::Body),
            st.roles.text3,
        );
        p.galley(
            egui::pos2(ic_c.x + 14.0, rect.center().y - g.size().y / 2.0),
            g,
            st.roles.text3,
        );
        let key = egui::Rect::from_center_size(
            egui::pos2(rect.right() - st.sp(2) - 10.0, rect.center().y),
            egui::vec2(20.0, 16.0),
        );
        p.rect_filled(key, 4.0, st.roles.inset);
        let g = p.layout_no_wrap("A".into(), st.font(style::Role::Caption), st.roles.text2);
        p.galley(key.center() - g.size() / 2.0, g, st.roles.text2);
    }
    let r = r.tip(st, "Add a module (A)");
    ui_state.record("add-menu".into(), r.rect);
    let pop = egui::Popup::default_response_id(&r);
    if std::mem::take(&mut ui_state.add_menu_req)
        || (!ui.ctx().egui_wants_keyboard_input()
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::A)))
    {
        egui::Popup::open_id(ui.ctx(), pop);
    }
    let fid = r.id.with("filter");
    egui::Popup::menu(&r)
        .id(pop)
        .frame(kit::popover_frame(st))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .width(228.0)
        .show(|ui| {
            let mut q: String = ui.data(|d| d.get_temp(fid)).unwrap_or_default();
            let te = ui.add(
                egui::TextEdit::singleline(&mut q)
                    .hint_text("Add module")
                    .desired_width(f32::INFINITY),
            );
            te.request_focus();
            let needle = q.to_lowercase();
            let kinds: Vec<&str> = PatchEditor::known_kinds()
                .iter()
                .copied()
                .filter(|k| k.to_lowercase().contains(&needle))
                .collect();
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .show(ui, |ui| {
                    for (i, kind) in kinds.iter().enumerate() {
                        let it = kit::menu_item(ui, st, kind, false);
                        ui_state.record(format!("add:{kind}"), it.rect);
                        if it.clicked() || (enter && i == 0) {
                            add_module_end(editor, ui_state, kind);
                            q.clear();
                            ui.close();
                        }
                    }
                });
            ui.data_mut(|d| d.insert_temp(fid, q));
        });
}

fn toolbar(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui, st: &Style) {
    let full = ui.max_rect();
    let scene = if ui_state.browser_open {
        Scene::Sounds
    } else if ui_state.perform_open {
        Scene::Perform
    } else {
        Scene::Rack
    };
    let wide = full.width() >= 1300.0;
    let lay = |l| egui::UiBuilder::new().layout(l);
    // Centre: the view switch, centred in the window.
    let sw = egui::Rect::from_center_size(full.center(), egui::vec2(270.0, st.metrics.control_h));
    ui.scope_builder(
        lay(egui::Layout::left_to_right(egui::Align::Min)).max_rect(sw),
        |ui| {
            kit::view_switch(ui, st, |ui| {
                let r = kit::seg_large(ui, st, "Rack", scene == Scene::Rack);
                ui_state.record("rack".into(), r.rect);
                if r.clicked() {
                    ui_state.perform_open = false;
                    ui_state.browser_open = false;
                }
                let r = kit::seg_large(ui, st, "Perform", scene == Scene::Perform);
                ui_state.record("perform".into(), r.rect);
                if r.clicked() {
                    // The switch opens Perform as a view; "Show rack" in its header docks it.
                    ui_state.perform_open = true;
                    ui_state.perform_tall = true;
                    ui_state.browser_open = false;
                }
                let r = kit::seg_large(ui, st, "Sounds", scene == Scene::Sounds);
                ui_state.record("browser".into(), r.rect);
                if r.clicked() {
                    ui_state.browser_open = !ui_state.browser_open;
                }
            });
        },
    );
    // Left: brand and the sound dropdown.
    let left = egui::Rect::from_min_max(full.min, egui::pos2(sw.left() - 8.0, full.max.y));
    ui.scope_builder(
        lay(egui::Layout::left_to_right(egui::Align::Center)).max_rect(left),
        |ui| {
            ui.spacing_mut().item_spacing.x = st.sp(3);
            brand(ui, st);
            browser::doc_field(editor, ui_state, ui, st);
        },
    );
    // Right, read left to right: search, undo/redo, cable tools, fit, zoom, Save.
    let right = egui::Rect::from_min_max(egui::pos2(sw.right() + 8.0, full.min.y), full.max);
    ui.scope_builder(
        lay(egui::Layout::right_to_left(egui::Align::Center)).max_rect(right),
        |ui| {
            ui.spacing_mut().item_spacing.x = st.sp(1);
            browser::save_button(editor, ui_state, ui, st);
            ui.add_space(st.sp(1));
            match scene {
                Scene::Rack => {
                    let c = ui_state.canvas.center();
                    if tool_icon(ui, st, ui_state, "zoom:in", kit::Ic::Plus, "Zoom in", true) {
                        ui_state.zoom_about(ui_state.zoom * 1.2, c);
                    }
                    let pct = format!("{:.0} %", ui_state.zoom * 100.0);
                    let r = ui
                        .add(kit::Button::new(st, pct).ghost().small())
                        .tip(st, "Zoom to 100%");
                    ui_state.record("zoom:100".into(), r.rect);
                    if r.clicked() {
                        ui_state.zoom_about(1.0, c);
                    }
                    if tool_icon(
                        ui,
                        st,
                        ui_state,
                        "zoom:out",
                        kit::Ic::Minus,
                        "Zoom out",
                        true,
                    ) {
                        ui_state.zoom_about(ui_state.zoom / 1.2, c);
                    }
                    {
                        if tool_icon(
                            ui,
                            st,
                            ui_state,
                            "zoom:fit",
                            kit::Ic::Fit,
                            "Fit the whole patch",
                            true,
                        ) {
                            let lay = composite_layout(editor.state(), ui_state);
                            ui_state.frame_world(lay.bounds.expand(4.0), 1.5);
                        }
                        vsep(ui, st);
                    }
                    cable_tools(ui, st, ui_state);
                    vsep(ui, st);
                    undo_redo(editor, ui, st);
                    add_search(editor, ui_state, ui, st, wide);
                }
                Scene::Perform => {
                    let r = kit::icon_menu(ui, st, kit::Ic::Midi, |ui| {
                        let none = ui_state.midi_input.is_none();
                        if kit::menu_item(ui, st, "No MIDI input", none).clicked() {
                            ui_state.midi_select = Some(None);
                        }
                        for port in ui_state.midi_inputs.clone() {
                            let on = ui_state.midi_input.as_deref() == Some(port.as_str());
                            if kit::menu_item(ui, st, &port, on).clicked() {
                                ui_state.midi_select = Some(Some(port));
                            }
                        }
                    })
                    .tip(st, "MIDI input");
                    ui_state.record("midi-menu".into(), r.rect);
                }
                Scene::Sounds => {
                    let on = ui_state.browser.filter == browser::Filter::Favorites;
                    let r =
                        kit::icon_button(ui, st, kit::Ic::Heart, on, true).tip(st, "Favourites");
                    ui_state.record("favorites".into(), r.rect);
                    if r.clicked() {
                        ui_state.browser.filter = if on {
                            browser::Filter::All
                        } else {
                            browser::Filter::Favorites
                        };
                    }
                }
            }
        },
    );
}

/// Cable view as a segmented control of three icons: All, Focus, Hidden.
fn cable_tools(ui: &mut egui::Ui, st: &Style, ui_state: &mut UiState) {
    kit::segmented(ui, st, |ui| {
        // Added left to right inside a right-to-left parent, so reverse the order.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for (mode, label, ic) in [
                (CableView::Hidden, "Hidden", kit::Ic::CableOff),
                (CableView::Focus, "Focus", kit::Ic::Focus),
                (CableView::All, "All", kit::Ic::Cable),
            ] {
                let on = ui_state.cable_view == mode;
                let r = kit::icon_button(ui, st, ic, on, true).tip(st, &format!("Cables: {label}"));
                ui_state.record(format!("view:{label}"), r.rect);
                if r.clicked() {
                    ui_state.cable_view = mode;
                }
            }
        });
    });
}

/// Called in a right-to-left layout: redo goes first so undo ends up on its left.
fn undo_redo(editor: &mut PatchEditor, ui: &mut egui::Ui, st: &Style) {
    let r = kit::icon_button(ui, st, kit::Ic::Redo, false, editor.can_redo())
        .tip(st, "Redo (Ctrl+Shift+Z)");
    if r.clicked() {
        editor.redo();
    }
    let r =
        kit::icon_button(ui, st, kit::Ic::Undo, false, editor.can_undo()).tip(st, "Undo (Ctrl+Z)");
    if r.clicked() {
        editor.undo();
    }
}

/// The side rail: composites, add module, zoom, and the view and theme settings.
fn rail(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui, st: &Style) {
    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing.y = st.sp(1);
        let on = ui_state.composites.panel;
        let r = kit::icon_button(ui, st, kit::Ic::Link, on, true).tip(st, "Composites");
        ui_state.record("composites".into(), r.rect);
        if r.clicked() {
            ui_state.composites.panel = !on;
        }
        let on = ui_state.drawer_open;
        let r = kit::icon_button(ui, st, kit::Ic::Cable, on, true).tip(st, "Routing");
        ui_state.record("routing".into(), r.rect);
        if r.clicked() {
            ui_state.drawer_open = !on;
            ui_state.drawer_auto_closed = false;
        }
        kit::rule(ui, st);
        let focus_target = ui_state
            .inspected
            .as_ref()
            .map(|(id, _)| *id)
            .or(ui_state.selected_module);
        if tool_icon(
            ui,
            st,
            ui_state,
            "zoom:focus",
            kit::Ic::Focus,
            "Zoom to the selected module",
            focus_target.is_some(),
        ) {
            let lay = composite_layout(editor.state(), ui_state);
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
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            let menu = kit::icon_menu(ui, st, kit::Ic::Gear, |ui| {
                kit::segmented(ui, st, |ui| {
                    let r = kit::seg(ui, st, "Light", !ui_state.dark);
                    ui_state.record("theme:light".into(), r.rect);
                    if r.clicked() {
                        ui_state.dark = false;
                    }
                    let r = kit::seg(ui, st, "Dark", ui_state.dark);
                    ui_state.record("theme:dark".into(), r.rect);
                    if r.clicked() {
                        ui_state.dark = true;
                    }
                });
                let themes = ui_state.themes.clone();
                for t in &themes.list {
                    let r = kit::menu_item(ui, st, &t.name, t.id == st.id);
                    ui_state.record(format!("theme-id:{}", t.id), r.rect);
                    if r.clicked() {
                        ui_state.theme.clone_from(&t.id);
                    }
                }
                let notes = ui_state
                    .theme_notes
                    .iter()
                    .take(4)
                    .cloned()
                    .collect::<Vec<_>>();
                for n in notes {
                    kit::paragraph(ui, st, style::Role::Caption, kit::Tone::Text2, &n);
                }
                kit::gap(ui, st, 1);
                kit::rule(ui, st);
                kit::gap(ui, st, 1);
                kit::paragraph(
                    ui,
                    st,
                    style::Role::Caption,
                    kit::Tone::Text2,
                    "More controls opens a focused view; rack layout stays fixed.",
                );
                kit::gap(ui, st, 1);
                let r = kit::toggle(ui, st, &mut ui_state.skins, "Illustrated skins");
                ui_state.record("menu:skins".into(), r.rect);
                ui.add_enabled_ui(ui_state.skins, |ui| {
                    let mut on_art = ui_state.skin_labels_on_art.unwrap_or(false);
                    let r = kit::toggle(ui, st, &mut on_art, "Preview labels_on_art")
                        .tip(st, "A skin maker's flag; off = theme plates (default)");
                    ui_state.record("menu:labels-on-art".into(), r.rect);
                    if r.changed() {
                        ui_state.skin_labels_on_art = on_art.then_some(true);
                    }
                });
                kit::gap(ui, st, 1);
                kit::rule(ui, st);
                let r = kit::menu_item(ui, st, "About kabl...", false);
                ui_state.record("menu:about".into(), r.rect);
                if r.clicked() {
                    ui_state.about.open = true;
                    ui.close();
                }
            })
            .tip(st, "Theme and view settings");
            ui_state.record("view-menu".into(), menu.rect);
        });
    });
}

/// Tempo and song position of the patch's first clock (the one the engine follows), from the
/// clock's own `bpm` and the position the audio thread reports in pulses (four to a beat, four
/// beats to a bar). `None` without a clock or before it has reported.
fn transport_readout(state: &PatchState, ui_state: &UiState) -> Option<(f32, String)> {
    let (&id, m) = state.modules.iter().find(|(_, m)| m.kind == "clock")?;
    let pos = *ui_state.clock_pos.get(&id)?;
    let bpm = m.params.get("bpm").copied().unwrap_or(120.0);
    let pulse = pos.max(0.0) as u64;
    let (beat, tick) = (pulse / 4, pulse % 4);
    Some((
        bpm,
        format!("{} . {} . {}", beat / 4 + 1, beat % 4 + 1, tick + 1),
    ))
}

/// The status bar: output meter and peak, sample rate, MIDI input, the last message and the
/// recording clock; the host's controls at the right end.
fn status_bar(
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    right: impl FnOnce(&mut egui::Ui, &mut UiState),
) {
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = st.sp(3);
        if ui_state.status_meter {
            let out = kit::label(ui, st, style::Role::Label, kit::Tone::Text2, "Out");
            if ui_state.recorder.is_none() {
                out.on_hover_text("Recording needs an audio device.");
            }
            record::meter_ui(ui, st, &mut ui_state.meter);
            let peak = ui_state
                .meter
                .max_db
                .map_or("no signal".to_string(), |m| format!("{m:+.1} dB"));
            kit::label(ui, st, style::Role::Value, kit::Tone::Text2, peak);
            let khz = ui_state.sample_rate / 1000.0;
            let rate = match ui_state.status_buffer {
                Some(frames) => format!("{khz:.1} kHz · {frames}"),
                None => format!("{khz:.1} kHz"),
            };
            kit::label(ui, st, style::Role::Value, kit::Tone::Text2, rate);
        }
        if let Some(port) = ui_state.midi_input.clone() {
            let (dot, _) =
                ui.allocate_exact_size(egui::Vec2::splat(st.sp(2)), egui::Sense::hover());
            ui.painter()
                .circle_filled(dot.center(), st.sp(1), st.roles.good);
            kit::label_truncated(ui, st, style::Role::Caption, kit::Tone::Text2, &port);
        }
        if !ui_state.status_audio.is_empty() {
            let text = ui_state.status_audio.clone();
            kit::label_truncated(ui, st, style::Role::Caption, kit::Tone::Text2, &text)
                .on_hover_text(&text);
        }
        if let Some(cpu) = ui_state.status_cpu {
            kit::label(
                ui,
                st,
                style::Role::Caption,
                kit::Tone::Text2,
                format!("CPU {cpu:.0} %"),
            );
        }
        if let Some((used, all)) = ui_state.status_voices {
            kit::label(
                ui,
                st,
                style::Role::Caption,
                kit::Tone::Text2,
                format!("Voices {used} / {all}"),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Tempo and position are the rightmost, as in the mockup; the host's controls follow.
            if let Some(pos) = ui_state.status_pos.clone() {
                kit::label(ui, st, style::Role::Value, kit::Tone::Text2, pos);
            }
            if let Some(bpm) = ui_state.status_bpm {
                kit::label(
                    ui,
                    st,
                    style::Role::Value,
                    kit::Tone::Text2,
                    format!("{bpm:.1} BPM"),
                );
            }
            right(ui, ui_state);
            if let Some(rec) = ui_state.recorder.as_ref().filter(|r| r.recording()) {
                let t = rec.elapsed() as u64;
                kit::label(
                    ui,
                    st,
                    style::Role::Label,
                    kit::Tone::Bad,
                    format!("• REC {:02}:{:02}", t / 60, t % 60),
                );
            }
            let errors = kit::g4::error_count(&ui_state.toasts);
            if errors > 0 {
                let word = if errors == 1 { "error" } else { "errors" };
                kit::label(
                    ui,
                    st,
                    style::Role::Caption,
                    kit::Tone::Bad,
                    format!("• {errors} {word}"),
                );
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
    if kind == "cues" {
        // The G4 cues editor draws its own header and controls.
        seq_editor::cues_panel(editor, ui_state, ui, id);
        return;
    }
    let Some(info) = registry::info_for(&kind) else {
        ui.label("Unknown module kind.");
        return;
    };
    if kind == "seq" {
        // The G4 seq editor draws its own header and controls.
        let edit = ui_state.edit_bank_of(editor.state(), id);
        seq_editor::seq_panel(editor, ui_state, ui, id, edit);
        return;
    }
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
    let geo = authored_control_geo(editor.state(), ui_state, lay, *id, p)
        .or_else(|| lay.get(*id)?.ctl(p).map(|c| c.geo))?;
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
    rack_editor::boundary(editor, ui_state, ui);
    let canvas = ui.available_rect_before_wrap();
    let now = ui.input(|i| i.time);
    let lay = composite_layout(editor.state(), ui_state);
    rack_editor::update_drag(editor, ui_state, ui, &lay);
    let lay = composite_layout(editor.state(), ui_state);
    if !ui_state.fitted {
        // First frame: the whole patch, as large as the window allows.
        ui_state.canvas = canvas;
        ui_state.frame_patch(fit_world(lay.bounds), FIT_MAX_ZOOM);
        ui_state.fitted = true;
    } else if canvas != ui_state.canvas {
        // Drawer opened or window resized: keep what the user is working on reachable.
        ui_state.canvas = canvas;
        if ui_state.auto_fit {
            ui_state.frame_patch(fit_world(lay.bounds), FIT_MAX_ZOOM);
        }
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
    if let Some((leaf, param)) = ui_state.inspected.clone() {
        if (ui_state.inspected != ui_state.last_inspected || ui_state.reveal == Some(leaf))
            && lay.get(leaf).is_none()
            && authored_control_geo(editor.state(), ui_state, &lay, leaf, &param).is_none()
        {
            if let Some((&owner, _)) = editor
                .state()
                .composites
                .iter()
                .find(|(_, c)| c.members.contains(&leaf))
            {
                ui_state.enter_composite(editor.state(), owner);
                ui_state.inspected = Some((leaf, param));
            }
        }
    }
    let lay = composite_layout(editor.state(), ui_state);
    // Inspecting a control that is off the face (route selected, drawer) reveals it.
    if ui_state.inspected != ui_state.last_inspected {
        ui_state.last_inspected = ui_state.inspected.clone();
        if let Some((id, p)) = &ui_state.inspected {
            if authored_control_geo(editor.state(), ui_state, &lay, *id, p).is_none() {
                if let Some(h) = lay
                    .get(*id)
                    .and_then(|m| m.hidden.iter().find(|h| h.name == p))
                {
                    ui_state.expanded.clear();
                    ui_state.expanded.insert(*id);
                    ui_state.flash = Some((*id, h.name, now));
                    // Focused controls do not pan the outer rack.
                }
            }
        }
    }
    let lay = composite_layout(editor.state(), ui_state);
    if let Some(id) = ui_state
        .reveal
        .take()
        .filter(|_| ui_state.expanded.is_empty() && ui_state.panels.fallback.is_none())
    {
        if let Some(m) = lay.get(id) {
            let t = ui_state.xf().r(m.full());
            ui_state.keep_visible(t);
        }
    }

    // Newly inspected knob, or a route added to it: once the pointer is up, pan its source lanes
    // fully into view so no dot hides under the drawer or the canvas edge.
    if !ui.input(|i| i.pointer.any_down())
        && ui_state.expanded.is_empty()
        && ui_state.panels.fallback.is_none()
    {
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

    let bg = ui.interact(canvas, Id::new("kabl-canvas-bg"), Sense::click_and_drag());
    if bg.dragged()
        && ui_state.port_drag.is_none()
        && ui_state.moving.is_none()
        && ui_state.drag.is_none()
        && ui_state.expanded.is_empty()
        && ui_state.panels.fallback.is_none()
        && ui_state.choose.is_none()
        && ui_state.panels.selected.is_none()
    {
        ui_state.pan += bg.drag_delta();
    }
    if bg.clicked()
        && ui_state.moving.is_none()
        && ui_state.expanded.is_empty()
        && ui_state.panels.fallback.is_none()
        && ui_state.choose.is_none()
        && ui_state.panels.selected.is_none()
    {
        ui_state.selected_module = None;
        ui_state.selected_composite = None;
        // Clicking empty rack ends knob inspection (and its source lanes).
        ui_state.inspected = None;
        ui_state.selected_route = None;
    }
    let xf = ui_state.xf();
    let painter = ui.painter_at(canvas);
    // Empty rows continue below the patch to the bottom of the window, like a rack with free slots.
    let mut rows = lay.rows.max(1);
    while rows < 24 && xf.p(pos2(0.0, rack::row_y(rows - 1) + PANEL_H + 8.0)).y < canvas.bottom() {
        rows += 1;
    }
    draw_rails(&painter, th, xf, canvas, rows);
    if editor.state().modules.is_empty() {
        empty_rack(ui_state, ui, canvas);
    }

    let focused_at_frame_start = !ui_state.expanded.is_empty();
    let lifted_at_frame_start = ui_state.moving.is_some();
    let mut drawn = Drawn::default();
    ui_state.deferred.clear();
    draw_composites(editor, ui_state, ui, xf, &mut drawn);
    for m in &lay.mods {
        if ui_state
            .moving
            .as_ref()
            .is_some_and(|v| v.item == rack_editor::Item::Module(m.id) && !v.cancelled)
        {
            rack_editor::body(ui_state, ui, rack_editor::Item::Module(m.id), m.face, xf);
            painter.rect_filled(xf.r(m.face), 3., th.panel.linear_multiply(0.20));
            continue;
        }
        if ui_state.expanded.contains(&m.id) {
            let pt = theme::panel_theme(&ui_state.style, m.info.kind);
            kit::face::surface(
                &painter,
                &ui_state.style,
                xf.r(m.face),
                pt.panel,
                pt.panel_edge,
                xf.zoom,
            );
            text(
                &painter,
                xf.p(m.face.left_top() + vec2(12., 24.)),
                egui::Align2::LEFT_CENTER,
                m.info.name,
                12. * xf.zoom,
                th.ink,
                false,
            );
        } else {
            draw_module(editor, ui_state, ui, &painter, th, xf, m, now, &mut drawn);
        }
    }
    // Floating areas' visible screen rects, for drop targeting.
    let floats: Vec<(ModuleId, Rect)> = lay
        .mods
        .iter()
        .filter(|m| m.overlay)
        .filter_map(|m| Some((m.id, xf.r(m.adv?).intersect(canvas))))
        .collect();

    let deferred = std::mem::take(&mut ui_state.deferred);
    let modules_end = ui
        .ctx()
        .graphics(|g| g.get(painter.layer_id()).map_or(0, |l| l.next_idx().0));
    // Ropes paint above all module content (trays, plates, titles, knobs), cut by nothing but
    // the rack viewport; only the drawer, toolbar and status bar sit over them. Plugs go on top.
    let next_idx = |ui: &egui::Ui| {
        ui.ctx()
            .graphics(|g| g.get(painter.layer_id()).map_or(0, |l| l.next_idx().0))
    };
    let cables_start = next_idx(ui);
    let plugs = draw_cables(editor, ui_state, ui, &painter, th, xf, &lay, &drawn, None);
    if let Some((world, max_zoom)) = ui_state.pending_fit.take() {
        // The cables are known now: fit them too; the next frame draws the result.
        ui_state.refit_with_cables(world, max_zoom);
        ui.ctx().request_repaint();
    }
    painter.extend(plugs);
    ui_state.cable_audit = (painter.clip_rect(), modules_end, cables_start, next_idx(ui));
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

    if let Some(mv) = &ui_state.moving {
        let plan = rack_editor::drop_plan(&lay, mv.item, pos2(mv.live.x, mv.live.y), mv.width);
        if let Some((_, r)) = plan.iter().find(|(i, _)| *i == mv.item) {
            let screen = xf.r(*r);
            ui_state.record("rack:drop-preview".into(), screen);
            painter.rect_filled(screen, 3., th.sel.gamma_multiply(0.15));
            painter.rect_stroke(
                screen,
                3.,
                Stroke::new(2., th.sel),
                egui::StrokeKind::Inside,
            );
        }
        let next = Rect::from_min_size(
            pos2(rack::RACK_X, rack::row_y(lay.rows)),
            vec2(600., PANEL_H),
        );
        painter.rect_stroke(
            xf.r(next),
            3.,
            Stroke::new(1., th.rail_hi),
            egui::StrokeKind::Inside,
        );
        text(
            &painter,
            xf.p(next.left_top() + vec2(12., 22.)),
            egui::Align2::LEFT_TOP,
            "Drop into next row · drag near edge to scroll",
            12. * xf.zoom,
            th.ctext,
            false,
        );
    }

    if let Some(mv) = ui_state.moving.as_ref().filter(|_| lifted_at_frame_start) {
        let item = mv.item;
        let live = pos2(mv.live.x, mv.live.y);
        let cancelled = mv.cancelled;
        if !cancelled {
            egui::Area::new(Id::new("lifted-rack-item"))
                .order(egui::Order::Foreground)
                .fade_in(false)
                .fixed_pos(xf.p(live))
                .constrain(false)
                .sense(Sense::hover())
                .show(ui.ctx(), |overlay| {
                    overlay.disable();
                    overlay.set_opacity(0.88);
                    let rect = Rect::from_min_size(
                        xf.p(live),
                        vec2(mv_width(item, &lay), PANEL_H) * xf.zoom,
                    );
                    overlay.painter().rect_filled(
                        rect.translate(vec2(6., 8.)),
                        4.,
                        Color32::from_black_alpha(80),
                    );
                    match item {
                        rack_editor::Item::Module(id) => {
                            if let Some(m) = lay.get(id) {
                                let mut copy = m.clone();
                                copy.translate(live - copy.face.min);
                                draw_module(
                                    editor,
                                    ui_state,
                                    overlay,
                                    &overlay.painter().clone(),
                                    th,
                                    xf,
                                    &copy,
                                    now,
                                    &mut Drawn::default(),
                                );
                            }
                        }
                        rack_editor::Item::Composite(id) => {
                            let c = editor.state().composites[&id].clone();
                            let face = Rect::from_min_size(
                                live,
                                vec2(c.panel.as_ref().map_or(240., |p| p.width), PANEL_H),
                            );
                            if c.panel.is_some() {
                                draw_authored_face(
                                    editor,
                                    ui_state,
                                    overlay,
                                    xf,
                                    &mut Drawn::default(),
                                    id,
                                    &c,
                                    face,
                                );
                            } else {
                                draw_default_face(
                                    editor,
                                    ui_state,
                                    overlay,
                                    &mut Drawn::default(),
                                    id,
                                    &c,
                                    &editor.state().clone(),
                                    xf.r(face),
                                );
                            }
                        }
                    }
                });
        }
    }
    if focused_at_frame_start {
        rack_editor::more(editor, ui_state, ui, th, xf, &lay, &mut drawn);
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
    // Children and menus consume first; only an uncovered rack receives the remainder.
    // Wheel pans, Ctrl+wheel / pinch zooms about the pointer.
    let pointer = ui.input(|i| i.pointer.hover_pos());
    if let Some(p) = pointer.filter(|p| {
        canvas.contains(*p)
            && ui.rect_contains_pointer(canvas)
            && !wheel::claimed(ui.ctx())
            && ui_state.expanded.is_empty()
            && ui_state.panels.fallback.is_none()
            && ui_state.panels.selected.is_none()
            && ui_state.choose.is_none()
            && ui_state.moving.is_none()
    }) {
        let (scroll, zd) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta()));
        if zd != 1.0 {
            ui_state.zoom_about(ui_state.zoom * zd, p);
            ui.ctx().request_repaint();
        } else if scroll != EguiVec2::ZERO {
            ui_state.pan += scroll;
            ui.input_mut(|i| i.smooth_scroll_delta = EguiVec2::ZERO);
            ui.ctx().request_repaint();
        }
    }
}

fn mv_width(item: rack_editor::Item, lay: &Layout) -> f32 {
    match item {
        rack_editor::Item::Module(id) => lay.get(id).map_or(180., |m| m.face.width()),
        rack_editor::Item::Composite(id) => lay.faces.get(&id).map_or(240., |r| r.width()),
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
    let sectional = theme::panel_theme(&ui_state.style, m.info.kind);
    let th = if m.skin.is_none() { &sectional } else { th };
    let z = xf.zoom;
    let rect = xf.r(m.rect);
    let face = xf.r(m.face);
    ui_state.record(format!("module:{}", m.id), face);

    if ui.layer_id().order != egui::Order::Foreground {
        let body = rack_editor::body(ui_state, ui, rack_editor::Item::Module(m.id), m.face, xf);
        {
            let st = ui_state.style.clone();
            kit::context_menu(&body, &st, |ui| module_menu(editor, ui_state, ui, m));
        }
    }
    let skin = m.skin;
    let on_art = skin.is_some_and(|s| ui_state.skin_labels_on_art.unwrap_or(s.labels_on_art));
    let st = ui_state.style.clone();
    if let Some((tex, tint)) =
        skin.and_then(|s| skin_texture(ui, &mut ui_state.image_cache, m.info.kind, s, th.dark))
    {
        painter.rect_filled(rect, CornerRadius::same(3), th.panel);
        painter.image(
            tex,
            face,
            Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
            tint,
        );
        painter.rect_stroke(
            rect,
            CornerRadius::same(3),
            Stroke::new(1.0, th.panel_edge),
            egui::StrokeKind::Inside,
        );
        kit::face::hardware(painter, &st, rect, z);
    } else {
        kit::face::surface(painter, &st, rect, th.panel, th.panel_edge, z);
    }
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
    let choice = ui_state.choose.clone();
    let choosing = choice.as_ref().filter(|(id, _)| *id == m.id);
    if !m.info.params.is_empty() {
        let edit = Rect::from_center_size(
            pos2(face.center().x, face.bottom() - 11. * z),
            vec2(64., 16.) * z,
        );
        let r = ui.interact(edit, Id::new(("native-edit-face", m.id)), Sense::click());
        ui_state.record(format!("edit-face:{}", m.id), edit);
        // Printed on hover only; the context menu always has it.
        if let (true, Some(font)) = (
            r.hovered(),
            kit::face::face_font(&st, style::Role::Caption, z),
        ) {
            painter.text(
                edit.center(),
                egui::Align2::CENTER_CENTER,
                "Edit face",
                font,
                ink,
            );
        }
        if r.clicked() {
            ui_state.choose = Some((
                m.id,
                rack::primary_set(&editor.state().modules[&m.id], m.info),
            ));
            ui_state.face_order = None;
            ui_state.expanded.clear();
        }
    }
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
    // What a left-aligned title must not touch: control labels, jacks with labels, displays.
    let mut avoid: Vec<Rect> = m
        .ctls
        .iter()
        .map(|c| {
            let b = c.geo.bounds();
            b.expand2(vec2(4.0, 0.0))
        })
        .chain(
            m.jacks
                .iter()
                .map(|j| Rect::from_center_size(j.c, vec2(48.0, 20.0)).translate(vec2(0.0, -14.0))),
        )
        .collect();
    if let Decor::Keys(r)
    | Decor::Display(r)
    | Decor::Transport(r)
    | Decor::Banks(r)
    | Decor::Operators(r)
    | Decor::Grid(r)
    | Decor::Cues(r) = m.decor
    {
        avoid.push(r);
    }
    let avoid: Vec<Rect> = avoid.into_iter().map(|r| xf.r(r)).collect();
    let ti = kit::face::family_ink(&st, m.info.kind, ink);
    let title_r = kit::face::title(painter, &st, face, z, m.info.name, &tag, ti, ink2, &avoid);
    if ui_state.selected_module == Some(m.id) {
        painter.rect_stroke(
            rect.expand(1.0),
            CornerRadius::same(4),
            Stroke::new(2.5, th.sel),
            egui::StrokeKind::Outside,
        );
    }
    if skin.is_none() {
        draw_decor(painter, th, xf, m);
        if let Decor::Operators(r) = m.decor {
            fm6::draw(editor, ui_state, ui, painter, xf, m, r);
        }
        let scope = match m.decor {
            Decor::Display(r) => Some(r),
            _ => None,
        };
        if let Some(r) = scope {
            if painter.clip_rect().intersects(face) {
                ui_state.wants.display(m.id, m.info.kind);
                ui_state.wants.feeds(editor.state(), m.id, m.info.kind);
            }
            displays::draw(
                editor.state(),
                &st,
                &ui_state.live,
                now,
                painter,
                xf,
                m,
                r,
                th.panel,
            );
            if m.info.kind == "osc.wt" && z >= 0.45 {
                // A chip in the display's corner opens the table editor.
                let d = xf.r(r);
                let chip = Rect::from_min_size(
                    pos2(d.right() - 30. * z - 3., d.top() + 3.),
                    vec2(30., 13.) * z,
                );
                let hit = ui.interact(chip, Id::new(("wt-edit-table", m.id)), Sense::click());
                ui_state.record(format!("edit-table:{}", m.id), chip);
                ui_state.record(format!("wt-display:{}", m.id), d);
                let (back, ink) = (st.roles.disp_bg, st.roles.disp_text);
                painter.rect_filled(
                    chip,
                    3.,
                    style::alpha(back, if hit.hovered() { 255 } else { 190 }),
                );
                painter.rect_stroke(
                    chip,
                    3.,
                    Stroke::new(1., kit::face::trace_color(&st)),
                    egui::StrokeKind::Inside,
                );
                if let Some(font) = kit::face::face_font(&st, style::Role::Caption, z) {
                    painter.text(
                        chip.center(),
                        egui::Align2::CENTER_CENTER,
                        "Edit",
                        font,
                        ink,
                    );
                }
                if hit.clicked() {
                    tables::open(editor, ui_state, m.id);
                }
            }
        }
    }
    if let Decor::Keys(r) = m.decor {
        // The voice settings this MIDI In gives the chain it drives, including the ones off
        // the face, so it is plain whose settings they are.
        let (voice, midi) = midi_in_summary(editor.state(), m);
        for (line, dy) in [(voice, 9.0), (midi, 20.0)] {
            text(
                painter,
                xf.p(pos2(r.center().x, r.bottom() + dy)),
                egui::Align2::CENTER_CENTER,
                &line,
                10.5 * z,
                ink2,
                true,
            );
        }
    }
    if let Decor::Transport(r) = m.decor {
        draw_transport(ui_state, ui, painter, th, xf, m.id, r);
    }
    if let Decor::Cues(r) = m.decor {
        draw_cues(editor, ui_state, ui, painter, xf, m.id, r, now);
    }
    if let Decor::Grid(r) = m.decor {
        drum_grid::draw(editor, ui_state, ui, painter, th, xf, m.id, r);
    }
    if let Decor::Banks(r) = m.decor {
        draw_banks(editor, ui_state, ui, painter, xf, m.id, r, now, drawn);
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
    if m.info.kind == "seq" {
        draw_seq_steps(editor, ui_state, painter, xf, m, ink, ink2);
    }
    if skin.is_some() {
        if let Some(p) = m.plate {
            painter.rect_filled(xf.r(p), CornerRadius::same(6), th.plate);
        }
    } else if let Some(t) = m.tray {
        kit::parts::tray(painter, &st, xf.r(t), m.plate.map(|p| xf.r(p)), z);
    } else if let Some(p) = m.plate {
        let r = xf.r(p);
        painter.line_segment(
            [r.left_top(), r.right_top()],
            Stroke::new(0.8, th.panel_edge),
        );
    }

    let look = Look {
        z,
        ink,
        ink2,
        slot: 76.0,
    };
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
        // The overview tier of a compact-text style draws no face text, so no chip either.
        let chip = !kit::face::overview(&ui_state.style, z)
            && (!kit::face::compact_tier(&ui_state.style, z) || title_r.max.x + 1.5 <= r.min.x);
        if chip {
            // G4 mockup: outlined pill (face edge, accent on hover), mono 11 `text2`.
            let roles = &ui_state.style.roles;
            let edge = face_edge(&ui_state.style);
            painter.rect_stroke(
                r,
                CornerRadius::same((r.height() / 2.0) as u8),
                Stroke::new(1.0, if resp.hovered() { roles.accent } else { edge }),
                egui::StrokeKind::Inside,
            );
            text(
                painter,
                r.center(),
                egui::Align2::CENTER_CENTER,
                &label,
                11.0 * z,
                roles.text2,
                true,
            );
        }
        let routed = hidden_routes(editor, m);
        for (cable, _) in &routed {
            drawn.plugs.push((*cable, r.center()));
        }
        let resp = if routed.is_empty() {
            resp.on_hover_text(if expanded {
                "Hide advanced controls"
            } else {
                "More controls · focused view"
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
                ui_state.expanded.clear();
                ui_state.expanded.insert(m.id);
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
            th.sel_ink,
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
    let st = ui_state.style.clone();
    let st = &*st;
    kit::label(
        ui,
        st,
        style::Role::H3,
        kit::Tone::Text,
        format!("{} #{}", m.info.name, m.id),
    );
    kit::menu_rule(ui, st);
    fn item(ui: &mut egui::Ui, ui_state: &mut UiState, key: &str, label: &str) -> bool {
        let st = ui_state.style.clone();
        let r = kit::menu_item(ui, &st, label, false);
        ui_state.record(format!("menu:{key}"), r.rect);
        r.clicked()
    }
    if item(ui, ui_state, "explain", "Explain this module") {
        explain::open(ui_state, explain::Subject::Module(m.id), None);
        ui_state.selected_module = Some(m.id);
        ui.close();
    }
    for p in m
        .info
        .ports
        .iter()
        .filter(|p| p.direction == PortDirection::Output)
    {
        if item(
            ui,
            ui_state,
            &format!("inspect:{}", p.name),
            &format!("Inspect output \"{}\"", p.name),
        ) {
            ui_state.selected_module = Some(m.id);
            ui_state.drawer_open = true;
            let now = ui.input(|i| i.time);
            ui_state.inspect.select(
                inspect::Sel {
                    id: m.id,
                    port: p.name.to_string(),
                },
                now,
            );
            ui.close();
        }
    }
    if !m.info.params.is_empty() && item(ui, ui_state, "choose", "Edit face…") {
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
        let changes: Vec<_> = editor.state().modules[&m.id]
            .params
            .keys()
            .filter(|k| k.starts_with(rack::FACE_PREFIX))
            .map(|k| (m.id, k.clone(), None))
            .collect();
        editor.set_presentation(&changes);
        ui.close();
    }
    if m.toggle.is_some() {
        let expanded = ui_state.expanded.contains(&m.id);
        if item(
            ui,
            ui_state,
            "expand",
            if expanded {
                "Close More controls"
            } else {
                "More controls"
            },
        ) {
            if expanded {
                ui_state.expanded.remove(&m.id);
            } else {
                ui_state.expanded.clear();
                ui_state.expanded.insert(m.id);
            }
            ui.close();
        }
    }
    kit::menu_rule(ui, st);
    perform::module_menu(editor, ui_state, ui, m.id, m.info);
    kit::menu_rule(ui, st);
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
        (
            "run",
            if ui_state.host_clock {
                "Host"
            } else if running {
                "Stop"
            } else {
                "Run"
            },
            0.0,
        ),
        (
            "restart",
            if ui_state.host_clock {
                "Synced"
            } else {
                "Restart"
            },
            half + 8.0,
        ),
    ];
    for (key, label, dx) in buttons {
        let b = xf.r(Rect::from_min_size(
            r.min + vec2(dx, 0.0),
            vec2(half, r.height()),
        ));
        ui_state.record(format!("{key}:{id}"), b);
        let resp = ui.interact(
            b,
            Id::new(("kabl-transport", key, id)),
            if ui_state.host_clock {
                Sense::hover()
            } else {
                Sense::click()
            },
        );
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
        let resp = resp.on_hover_text(if ui_state.host_clock {
            "Host clock: use REAPER transport or switch to Free"
        } else {
            match key {
                "run" if running => "Stop the clock: gates go low, envelopes release",
                "run" => "Start the clock on the next step",
                _ if running => "Restart every pattern on step 1 now (sends a pulse on reset)",
                _ => "Start on step 1 when the clock runs again",
            }
        });
        if resp.clicked() && !ui_state.host_clock {
            let t = match key {
                "run" if running => Transport::Stop,
                "run" => Transport::Run,
                _ => Transport::Restart,
            };
            ui_state.transport.push((id, t));
        }
    }
}

/// Sequencer header groups, measured from the G4 mockup (`g4-surfaces.html`, seq face): each
/// group is a mono 10 caption, 6 px, then an 84 x 18 pill track of four 21 px options; 14 px
/// between groups. Three groups: EDIT, PLAY and PAGE.
const BANK_CAP_W: f32 = 24.0;
const BANK_SEG_W: f32 = 84.0;
const BANK_GROUP_W: f32 = BANK_CAP_W + 6.0 + BANK_SEG_W;
const BANK_GROUP_GAP: f32 = 14.0;
/// Width of the sequencer header row (`rack::timing` places it).
pub(crate) const BANKS_W: f32 = 3.0 * BANK_GROUP_W + 2.0 * BANK_GROUP_GAP;

/// The sequencer's header row. EDIT picks the bank the face shows (view only); PLAY launches a
/// bank with the sequencer's own timing and clock (runtime); the playing bank is lit, a queued
/// one blinks, and Cancel drops the queued launch. Right-click an EDIT option for bank edits.
/// The third group's place is kept for PAGE: the engine has no step pages yet.
#[allow(clippy::too_many_arguments)]
fn draw_banks(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    xf: Xf,
    id: ModuleId,
    r: Rect,
    now: f64,
    drawn: &mut Drawn,
) {
    let z = xf.zoom;
    let roles = ui_state.style.roles.clone();
    let track = seg_track(&ui_state.style);
    let edit = ui_state.edit_bank_of(editor.state(), id);
    let (playing, queued) = ui_state
        .seq_banks
        .get(&id)
        .copied()
        .unwrap_or((banks::startup(editor.state(), id), None));
    let sw = BANK_SEG_W / seq::BANKS as f32;
    // One group: caption, track, and the option rects in screen space.
    let group = |gx: f32, cap: &str| -> Vec<Rect> {
        text(
            painter,
            xf.p(pos2(gx, r.center().y)),
            egui::Align2::LEFT_CENTER,
            cap,
            10.0 * z,
            roles.text2,
            true,
        );
        let tx = gx + BANK_CAP_W + 6.0;
        painter.rect_filled(
            xf.r(Rect::from_min_size(
                pos2(tx, r.top()),
                vec2(BANK_SEG_W, r.height()),
            )),
            CornerRadius::same((r.height() / 2.0 * z) as u8),
            track,
        );
        (0..seq::BANKS)
            .map(|b| {
                xf.r(Rect::from_min_size(
                    pos2(tx + b as f32 * sw, r.top()),
                    vec2(sw, r.height()),
                ))
            })
            .collect()
    };
    // The selected option: accent pill inset 2 px; the others are plain `text2` letters.
    let option = |t: Rect, s: &str, on: bool| {
        if on {
            let pill = t.shrink(2.0 * z);
            painter.rect_filled(
                pill,
                CornerRadius::same((pill.height() / 2.0) as u8),
                roles.accent,
            );
        }
        text(
            painter,
            t.center(),
            egui::Align2::CENTER_CENTER,
            s,
            11.0 * z,
            if on { roles.on_accent } else { roles.text2 },
            true,
        );
    };
    let edit_tabs = group(r.left(), "EDIT");
    let play_btns = group(r.left() + BANK_GROUP_W + BANK_GROUP_GAP, "PLAY");
    for b in 0..seq::BANKS {
        let name = banks::title(editor.state(), id, b);
        // EDIT option.
        let t = edit_tabs[b];
        ui_state.record(format!("bank-edit:{id}.{b}"), t);
        let resp = ui.interact(t, Id::new(("kabl-bank-edit", id, b)), Sense::click());
        let on = b == edit;
        option(t, seq::BANK_NAMES[b], on);
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
            painter.circle_filled(t.right_top(), 2.0 * z, roles.accent);
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
        {
            let st = ui_state.style.clone();
            kit::context_menu(&resp, &st, |ui| bank_menu(editor, ui_state, ui, id, b));
        }

        // PLAY option.
        let pbtn = play_btns[b];
        ui_state.record(format!("bank-play:{id}.{b}"), pbtn);
        let resp = ui.interact(pbtn, Id::new(("kabl-bank-play", id, b)), Sense::click());
        let lit = b == playing;
        option(pbtn, seq::BANK_NAMES[b], lit);
        if queued == Some(b) {
            // Blinks twice a second until it lands.
            let a = if (now * 4.0) as i64 % 2 == 0 {
                1.0
            } else {
                0.35
            };
            let pill = pbtn.shrink(2.0 * z);
            painter.rect_stroke(
                pill,
                CornerRadius::same((pill.height() / 2.0) as u8),
                Stroke::new(1.5 * z, roles.accent.gamma_multiply(a)),
                egui::StrokeKind::Inside,
            );
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(120));
        }
        let timing = banks::timing(editor.state(), id);
        let when = banks::TIMINGS
            .iter()
            .find(|t| t.0 == timing)
            .map_or("", |t| t.1);
        let resp = resp.on_hover_text(if lit {
            format!("Bank {name} is playing")
        } else if queued == Some(b) {
            format!("Bank {name} is queued ({when}). Click to cancel")
        } else {
            format!("Launch bank {name} ({when})")
        });
        // The queued bank cancels itself: the header has no room for a Cancel chip.
        if resp.clicked() && queued == Some(b) {
            ui_state.launches.push(Command::Cancel(Some(id)));
        } else if resp.clicked() {
            if let Err(e) = banks::launch(&mut ui_state.launches, editor.state(), id, b) {
                ui_state.last_message = Some(e);
            }
        }
    }
}

/// Line colour of the G4 mockup's line faces (`edge`): text at 26 % (graphite) / 30 % (paper).
fn face_edge(st: &Style) -> Color32 {
    style::alpha(st.roles.text, if st.dark { 66 } else { 77 })
}

/// The G4 mockup's selector track (`segT`): graphite `inset`; paper #D6D2C8, which has no
/// token, so `line`, the nearest.
fn seg_track(st: &Style) -> Color32 {
    if st.dark {
        st.roles.inset
    } else {
        st.roles.line
    }
}

/// The sequencer body behind its controls (G4 mockup, seq body): the left column's divider,
/// then per step a dot (accent on the playing step), the step number, and the playing step's
/// column tint. Shown only while the face edits the playing bank.
fn draw_seq_steps(
    editor: &PatchEditor,
    ui_state: &UiState,
    painter: &egui::Painter,
    xf: Xf,
    m: &Placed,
    ink: Color32,
    ink2: Color32,
) {
    let st = &ui_state.style;
    let z = xf.zoom;
    let edge = face_edge(st);
    let f = m.face;
    let edit = ui_state.edit_bank_of(editor.state(), m.id);
    let playing_bank = ui_state.seq_banks.get(&m.id).map_or(edit, |b| b.0);
    let now = ui_state
        .seq_steps
        .get(&m.id)
        .copied()
        .filter(|_| playing_bank == edit);
    let at = |x: f32, y: f32| xf.p(f.min + vec2(x, y));
    painter.line_segment([at(136.0, 58.0), at(136.0, 272.0)], Stroke::new(1.0, edge));
    let cw = (f.width() - 12.0 - 148.0) / seq::STEPS as f32;
    for k in 0..seq::STEPS {
        let cx = 148.0 + cw * (k as f32 + 0.5);
        let on = now == Some(k);
        if on {
            painter.rect_filled(
                xf.r(Rect::from_min_size(
                    f.min + vec2(cx - cw / 2.0 + 3.0, 54.0),
                    vec2(cw - 6.0, 218.0),
                )),
                CornerRadius::same((6.0 * z) as u8),
                st.roles.accent.gamma_multiply(0.07),
            );
        }
        painter.circle_filled(
            at(cx, 64.0),
            3.0 * z,
            if on { st.roles.accent } else { edge },
        );
        text(
            painter,
            at(cx, 82.0),
            egui::Align2::CENTER_CENTER,
            &(k + 1).to_string(),
            11.0 * z,
            if on { ink } else { ink2 },
            true,
        );
    }
}

/// A sequencer step's gate pill (click: on/off; on = gate signal colour, off = the selector
/// track) or velocity bar (click or drag; filled from the bottom in the gate colour), from the
/// G4 mockup. Routed cables plug in at its centre.
#[allow(clippy::too_many_arguments)]
fn seq_step_cell(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    xf: Xf,
    id: ModuleId,
    param: &'static kabl_modules::ParamInfo,
    rect: Rect,
    drawn: &mut Drawn,
) {
    let st = ui_state.style.clone();
    let z = xf.zoom;
    let r = xf.r(rect);
    let slot = seq::slot_name(param.name);
    let step = &slot[1..];
    let v = routing::base_value(editor.state(), id, param);
    let key = format!("{id}.{}", param.name);
    ui_state.record(format!("step:{key}"), r);
    if slot.starts_with('g') {
        let on = v >= 0.5;
        painter.rect_filled(
            r,
            CornerRadius::same((r.height() / 2.0) as u8),
            if on { st.roles.gate } else { seg_track(&st) },
        );
        let resp = ui
            .interact(
                r.expand(2.0 * z),
                Id::new(("kabl-seq-gate", id, param.name)),
                Sense::click(),
            )
            .on_hover_text(format!(
                "Step {step} gate: {}. Click to switch it {}",
                if on { "on" } else { "off (a rest)" },
                if on { "off" } else { "on" }
            ));
        if resp.clicked() {
            editor.set_param(id, param.name, if on { 0.0 } else { 1.0 });
        }
    } else {
        let n = param.to_norm(v).clamp(0.0, 1.0);
        let round = CornerRadius::same((r.width() / 2.0) as u8);
        painter.rect_filled(r, round, face_edge(&st));
        if n > 0.0 {
            painter.rect_filled(
                Rect::from_min_max(pos2(r.left(), r.bottom() - r.height() * n), r.max),
                round,
                st.roles.gate,
            );
        }
        let resp = ui
            .interact(
                r.expand2(vec2(10.0, 4.0) * z),
                Id::new(("kabl-seq-vel", id, param.name)),
                Sense::click_and_drag(),
            )
            .on_hover_text(format!(
                "Step {step} velocity: {}. Click or drag to set",
                routing::fmt_value(param, v)
            ));
        if resp.clicked() || resp.dragged() {
            if let Some(p) = resp.interact_pointer_pos() {
                let n = ((r.bottom() - p.y) / r.height()).clamp(0.0, 1.0);
                editor.set_param_gesture(
                    kabl_core::ParamTarget::Module {
                        id,
                        param: param.name.to_string(),
                    },
                    param.min + n * (param.max - param.min),
                    resp.clicked() || resp.drag_started(),
                );
            }
        }
    }
    for rt in routing::routes_into(editor.state(), id, param.name) {
        drawn.plugs.push((rt.cable, r.center()));
    }
}

/// A launch button on the rack, as a G3 kit chip: neutral (inset, hairline), playing (accent
/// fill) or queued (blinking accent outline).
#[allow(clippy::too_many_arguments)]
fn pad(
    ui_state: &mut UiState,
    ui: &egui::Ui,
    painter: &egui::Painter,
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
    let roles = &ui_state.style.roles;
    let radius = CornerRadius::same((6.0 * z) as u8);
    painter.rect_filled(r, radius, if lit { roles.accent } else { roles.inset });
    if !lit {
        painter.rect_stroke(
            r,
            radius,
            Stroke::new(
                1.0,
                if resp.hovered() {
                    roles.accent
                } else {
                    roles.line2
                },
            ),
            egui::StrokeKind::Inside,
        );
    }
    if queued {
        let a = if (now * 4.0) as i64 % 2 == 0 {
            1.0
        } else {
            0.35
        };
        painter.rect_stroke(
            r,
            radius,
            Stroke::new(1.5 * z, roles.accent.gamma_multiply(a)),
            egui::StrokeKind::Inside,
        );
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(120));
    }
    text(
        painter,
        r.center(),
        egui::Align2::CENTER_CENTER,
        label,
        12.0 * z,
        if lit { roles.on_accent } else { roles.text },
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
    let last = ui_state.last_cue.get(&id).copied();
    let standing: Vec<(bool, bool)> = all
        .iter()
        .map(|c| cues::standing_now(c, &bank_of, last))
        .collect();
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
                    CornerRadius::same((6.0 * z) as u8),
                    Stroke::new(1.0, ui_state.style.roles.line),
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
        (None, None) if all.iter().any(|c| c.targets.is_empty()) => "no cue launched yet".into(),
        (None, None) => "no cue matches what plays".into(),
    };
    text(
        painter,
        xf.p(pos2(r.left(), y + 14.0)),
        egui::Align2::LEFT_CENTER,
        &line,
        11.5 * z,
        ui_state.style.roles.text2,
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
        match cues::launch(&mut ui_state.launches, editor.state(), id, n) {
            Ok(()) => {
                ui_state.last_cue.insert(id, n);
            }
            Err(e) => ui_state.last_message = Some(e),
        }
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
    let st = ui_state.style.clone();
    let st = &*st;
    kit::label(
        ui,
        st,
        style::Role::H3,
        kit::Tone::Text,
        format!("Bank {}", banks::title(editor.state(), id, b)),
    );
    kit::menu_rule(ui, st);
    kit::submenu(ui, st, "Copy to", |ui| {
        for to in (0..seq::BANKS).filter(|&to| to != b) {
            let r = kit::menu_item(
                ui,
                st,
                &format!("Bank {}", banks::title(editor.state(), id, to)),
                false,
            );
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
    let r = kit::menu_item(ui, st, "Clear", false);
    ui_state.record("menu:bank-clear".into(), r.rect);
    if r.clicked() {
        banks::clear(editor, id, b);
        ui.close();
    }
    let startup = banks::startup(editor.state(), id) == b;
    let r = kit::menu_item_tone(
        ui,
        st,
        "Set as startup bank",
        false,
        kit::Tone::Text,
        !startup,
    );
    ui_state.record("menu:bank-startup".into(), r.rect);
    if r.tip(
        st,
        "The bank a Load starts on; does not change what is playing",
    )
    .clicked()
    {
        editor.set_param(id, "bank", b as f32);
        ui.close();
    }
    kit::menu_rule(ui, st);
    kit::label(ui, st, style::Role::Label, kit::Tone::Text2, "Name");
    let stored = editor
        .state()
        .label(id, &banks::name_key(b))
        .unwrap_or("")
        .to_string();
    let mut t = match ui_state.bank_rename.take() {
        Some((i, k, t)) if (i, k) == (id, b) => t,
        _ => stored,
    };
    let r = kit::field(ui, st, &mut t, "", None, Some(160.0));
    ui_state.record("menu:bank-name".into(), r.rect);
    let commit = r.lost_focus() || ui.add(kit::Button::new(st, "Rename")).clicked();
    if commit {
        banks::rename(editor, id, b, &t);
    } else {
        ui_state.bank_rename = Some((id, b, t));
    }
}

/// A MIDI In's voice settings (`POLY`, `MONO · LOW`, `LEGATO · LAST · glide 120 ms`), then its
/// bend range and channel (`bend ±2 st · ALL ch`, `bend ±12 st · ch 2`).
fn midi_in_summary(state: &kabl_core::PatchState, m: &Placed) -> (String, String) {
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
    let fmt = |name: &str| {
        let p = m.info.params.iter().find(|p| p.name == name);
        p.map_or(String::new(), |p| routing::fmt_value(p, get(name)))
    };
    let channel = match fmt("channel") {
        c if c == "ALL" => "ALL ch".to_string(),
        c => c,
    };
    (s, format!("bend {} · {channel}", fmt("bend")))
}

fn draw_decor(p: &egui::Painter, th: &Theme, xf: Xf, m: &Placed) {
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
        Decor::None
        | Decor::Operators(_)
        | Decor::Display(_)
        | Decor::Transport(_)
        | Decor::Status(_)
        | Decor::Banks(_)
        | Decor::Grid(_)
        | Decor::Cues(_) => {}
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
    let look = Look {
        slot: m.slot_width(c),
        ..look
    };
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
    if let (true, Geo::Select { rect }) = (m.info.kind == "seq", c.geo) {
        let slot = seq::slot_name(c.param.name);
        if slot.len() == 2 && matches!(slot.as_bytes()[0], b'g' | b'v') {
            seq_step_cell(
                editor, ui_state, ui, painter, xf, m.id, c.param, rect, drawn,
            );
            return;
        }
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
    let pr = xf.r(c.geo.pin_rect(&routing::face_label(m.info.kind, c.param)));
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
    painter.circle_filled(pr.center(), 2.5 * z, if on { th.sel_ink } else { th.sel });
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
                .iter()
                .filter(|(_, c)| c.from == here)
                .filter_map(|(&id, _)| state.end_module(id))
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
            .map(|c| format!("< {}", name(c.from.module_id().unwrap_or_default()))),
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
    let st = ui_state.style.clone();
    // Ports of this module that carry a cable, drawn with the signal's colour as a rim.
    let cabled: Vec<(PortDirection, &str)> = editor
        .state()
        .cables
        .values()
        .flat_map(|c| {
            let out = match &c.from {
                PortRef::Module { id, port } if *id == m.id => {
                    Some((PortDirection::Output, port.as_str()))
                }
                _ => None,
            };
            let inn = match &c.to {
                PortRef::Module { id, port } if *id == m.id => {
                    Some((PortDirection::Input, port.as_str()))
                }
                _ => None,
            };
            [out, inn]
        })
        .flatten()
        .collect();
    let cabled: Vec<(PortDirection, String)> = cabled
        .into_iter()
        .map(|(d, p)| (d, p.to_string()))
        .collect();
    for j in &m.jacks {
        let c = xf.p(j.c);
        let port = j.port;
        drawn
            .ports
            .insert((m.id, port.direction, port.name.to_string()), c);
        let on_plate = m.skin.is_some() && m.plate.is_some_and(|p| p.contains(j.c));
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
        let label = macro_name(editor.state(), m.id, port.name)
            .unwrap_or_else(|| face_port_label(m.info.kind, port.name));
        if let Some(font) = kit::face::face_font(&st, style::Role::Label, z) {
            if j.label_right {
                painter.text(
                    c + vec2(20.0 * z, 0.0),
                    egui::Align2::LEFT_CENTER,
                    &label,
                    font,
                    ink,
                );
            } else {
                painter.text(
                    c - vec2(0.0, rack::jack_text_dy(z, font.size * 1.075)),
                    egui::Align2::CENTER_CENTER,
                    &label,
                    font,
                    ink,
                );
            }
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
        let hot = resp.hovered() || ui_state.pending_output.as_ref() == Some(&this_ref);
        let patched = cabled
            .iter()
            .any(|(d, p)| *d == port.direction && p == port.name);
        kit::face::jack_ch(
            painter,
            &st,
            c,
            JACK_R * z,
            z,
            th.signal(port.port_type),
            patched,
            hot,
            port.width == 2 || port.name == "stereo",
        );
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
/// The label a face draws on a jack: the family's short name where it has one, else `port_label`.
pub fn face_port_label(kind: &str, name: &str) -> String {
    rack::short_name(kind, name).map_or_else(|| port_label(name), str::to_string)
}

fn port_label(name: &str) -> String {
    name.split('_')
        .map(|w| {
            let w = match w {
                "resonance" => "res",
                // Four jacks share the MIDI In face.
                "velocity" => "vel",
                w => w,
            };
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
    cable_path(&top, &ui_state.style, a, b, th.sel, ui_state.zoom);
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
            &ui_state.style,
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

/// A cable drawn straight into `p`, in the theme's lit width: the one in the hand while a plug
/// is dragged, which is always fully visible.
fn cable_path(
    p: &egui::Painter,
    st: &Style,
    a: Pos2,
    b: Pos2,
    col: Color32,
    zoom: f32,
) -> Vec<Pos2> {
    let pts = cable_look::points(a, b, zoom, st.slots.cable.sag);
    let mut caps = Vec::new();
    cable_look::paint(
        p,
        &mut caps,
        st,
        &pts,
        col,
        st.slots.cable.width * zoom,
        1.0,
        0.35,
        false,
        1.0,
        zoom,
        false,
    );
    p.extend(caps);
    pts
}

/// One cable to draw this frame.
struct Run<'a> {
    id: CableId,
    c: &'a kabl_core::CableState,
    /// A route lead into a knob rather than a jack-to-jack cable.
    route: bool,
    pts: Vec<Pos2>,
}

/// Jack cables and route leads. `only`: just the leads into that module's floating area.
///
/// Cables rest thin and slightly faint, in a colour kept legible on every face. A cable lights
/// (full width, full opacity, halo) when its module is hovered or selected, when one of its
/// jacks is hovered, when the pointer is on it, or when it is the selected route. Focus pins the
/// selected module's cables lit and dims the rest, whatever the pointer does. Returns the plugs
/// and marks, which the caller paints above the faces; the rope itself lies beneath them.
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
) -> Vec<egui::Shape> {
    let mut top = Vec::new();
    if only.is_none() {
        ui_state.cable_ends.clear();
    }
    if ui_state.cable_view == CableView::Hidden {
        return top;
    }
    let z = xf.zoom;
    let st = ui_state.style.clone();
    let slots = st.slots.cable.clone();
    let ctx = ui.ctx().clone();
    let pinned = ui_state.cable_view == CableView::Focus;
    let selected = ui_state.selected_module;
    // Hover lights cables only while no button is down, so a drag never flickers the rack.
    let pointer = ctx
        .input(|i| {
            (!i.pointer.any_down())
                .then(|| i.pointer.hover_pos())
                .flatten()
        })
        .filter(|p| ui_state.canvas.contains(*p));
    let hover_module = pointer.and_then(|p| {
        lay.mods
            .iter()
            .find(|m| xf.r(m.face).contains(p))
            .map(|m| m.id)
    });
    let hot_port = pointer.and_then(|p| {
        drawn
            .ports
            .iter()
            .find(|(_, &pos)| (pos - p).length() <= JACK_R * z * 1.4)
            .map(|(k, _)| k.clone())
    });
    let cables: Vec<(CableId, kabl_core::CableState)> = editor
        .state()
        .cables
        .iter()
        .map(|(&id, c)| (id, c.clone()))
        .collect();
    let mut runs: Vec<Run> = Vec::new();
    for (cable_id, c) in &cables {
        if only.is_some_and(|id| c.from.module_id() != Some(id) && c.to.module_id() != Some(id)) {
            continue;
        }
        if ui_state.unplug == Some(*cable_id) {
            continue; // in the hand, drawn to the pointer
        }
        let Some(&a) = port_ref_pos(&drawn.ports, &c.from, PortDirection::Output) else {
            continue;
        };
        if let Some(&b) = port_ref_pos(&drawn.ports, &c.to, PortDirection::Input) {
            if only.is_none() {
                ui_state.cable_ends.push((xf.inv(a), xf.inv(b)));
            }
            let pts = cable_look::points(a, b, z, slots.sag);
            runs.push(Run {
                id: *cable_id,
                c,
                route: false,
                pts,
            });
        }
    }
    for (cable_id, pos) in &drawn.plugs {
        let Some((_, c)) = cables.iter().find(|(id, _)| id == cable_id) else {
            continue;
        };
        if only.is_some_and(|id| c.to.module_id() != Some(id)) {
            continue;
        }
        let Some(&a) = port_ref_pos(&drawn.ports, &c.from, PortDirection::Output) else {
            continue;
        };
        if only.is_none() {
            ui_state.cable_ends.push((xf.inv(a), xf.inv(*pos)));
        }
        let pts = cable_look::points(a, *pos, z, slots.sag);
        runs.push(Run {
            id: *cable_id,
            c,
            route: true,
            pts,
        });
    }
    // The cable under the pointer, if several, the nearest.
    let reach = (slots.width * z * 0.5).max(6.0);
    let hover_cable = pointer.and_then(|p| {
        runs.iter()
            .map(|r| (r.id, cable_look::distance_to(&r.pts, p)))
            .filter(|&(_, d)| d <= reach)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    });
    let lit_of = |r: &Run| -> bool {
        let touches =
            |m: Option<ModuleId>| m.is_some() && (m == selected || (!pinned && m == hover_module));
        let (from, to) = (r.c.from.module_id(), r.c.to.module_id());
        let inspected = matches!((&r.c.to, &ui_state.inspected),
            (PortRef::Param { id, param }, Some((iid, ip))) if id == iid && param == ip);
        let jack = |dir: PortDirection, p: &PortRef| match (p, &hot_port) {
            (PortRef::Module { id, port }, Some((hid, hdir, hport))) => {
                id == hid && dir == *hdir && port == hport
            }
            _ => false,
        };
        touches(from)
            || touches(to)
            || ui_state.selected_route == Some(r.id)
            || inspected
            || (!pinned
                && (hover_cable == Some(r.id)
                    || jack(PortDirection::Output, &r.c.from)
                    || jack(PortDirection::Input, &r.c.to)))
    };
    let lit: Vec<bool> = runs.iter().map(&lit_of).collect();
    let dim_others = pinned && selected.is_some();
    let secs = 0.12;
    // Resting cables first, lit ones on top.
    let mut order: Vec<usize> = (0..runs.len()).collect();
    order.sort_by_key(|&i| lit[i]);
    let bases: Vec<Color32> = runs
        .iter()
        .map(|r| {
            if r.route {
                cable_color(r.id)
            } else {
                port_color(th, editor, &r.c.from)
            }
        })
        .collect();
    // Per-cable look this frame: lit fraction, opacity, width, colour, casing.
    let look = |i: usize| {
        let r = &runs[i];
        let on = lit[i];
        let kind = if r.route { 0.75 } else { 1.0 };
        let t = ctx.animate_bool_with_time(Id::new(("kabl-cable-lit", r.id)), on, secs);
        let alpha_to = if on {
            1.0
        } else if dim_others {
            slots.dimmed
        } else {
            slots.rest_alpha
        };
        let alpha =
            ctx.animate_value_with_time(Id::new(("kabl-cable-alpha", r.id)), alpha_to, secs);
        let w = ((slots.rest_width + (slots.width - slots.rest_width) * t) * kind * z).max(2.0);
        let (col, rest_casing) = cable_look::resting(&ctx, &st, bases[i], slots.rest_alpha);
        let casing = (rest_casing * (alpha / slots.rest_alpha).min(1.0) * (1.0 + 0.8 * t)).min(1.0);
        (t, alpha, w, col, casing)
    };
    // Themed: all glows first, then all cores.
    if slots.style == style::CableStyle::Themed {
        for &i in &order {
            let (t, alpha, _, col, _) = look(i);
            cable_look::glow(painter, &st, &runs[i].pts, col, alpha, t, z);
        }
    }
    let themed = slots.style == style::CableStyle::Themed;
    for i in order {
        let r = &runs[i];
        let (t, alpha, w, col, casing) = look(i);
        let stereo = !r.route && port_width(editor, &r.c.from) == 2;
        let bypass = r.route && r.c.params.get("bypass").is_some_and(|&b| b >= 0.5);
        cable_look::paint(
            painter, &mut top, &st, &r.pts, col, w, alpha, casing, bypass, t, z, stereo,
        );
        let mid = r.pts[r.pts.len() / 2];
        let steps = kabl_cables::Settings::from_params(&r.c.params);
        let len = kabl_cables::slot_value(&r.c.params, 0) as usize;
        if themed && routing::is_functional(r.c) && len > 0 {
            let seed = kabl_cables::seed_of(r.id);
            let on: Vec<bool> = (0..len)
                .map(|k| steps.level_at(seed, k as u64) > 0.0)
                .collect();
            cable_look::beads(&mut top, &st, &r.pts, col, th.sel, &on, None, z);
        } else if routing::is_functional(r.c) {
            top.extend(routing::functional_mark(
                r.pts[r.pts.len() / 3],
                z,
                th.sel,
                th.panel.gamma_multiply(0.85),
            ));
        }
        let hit = Rect::from_center_size(mid, EguiVec2::splat(12.0));
        if r.route {
            ui_state.record(format!("route:{}", r.id), hit);
            let resp = ui.interact(hit, Id::new(("kabl-route", r.id)), Sense::click());
            if resp.clicked() {
                // Clicking a route selects it (never deletes: removal is explicit in the drawer).
                if let PortRef::Param { id, param } = &r.c.to {
                    ui_state.selected_module = Some(*id);
                    ui_state.inspected = Some((*id, param.clone()));
                    ui_state.selected_route = Some(r.id);
                }
            }
            continue;
        }
        ui_state.record(format!("cable:{}", r.id), hit);
        let resp = ui.interact(hit, Id::new(("kabl-cable", r.id)), Sense::click());
        if resp.hovered() {
            top.push(egui::Shape::circle_stroke(
                mid,
                7.0,
                Stroke::new(1.5, th.sel),
            ));
        }
        let resp = resp.on_hover_text("Drag its plug out of the input to move or remove it");
        let cable_id = r.id;
        kit::context_menu(&resp, &st, |ui| {
            let m = kit::menu_item(ui, &st, "Pattern & probability...", false);
            ui_state.record("menu:cable-pattern".into(), m.rect);
            if m.clicked() {
                ui_state.cable_fn = Some(cable_id);
                ui.close();
            }
            let m = kit::menu_item(ui, &st, "Remove cable", false);
            ui_state.record("menu:remove-cable".into(), m.rect);
            if m.clicked() {
                editor.disconnect(cable_id);
                ui.close();
            }
        });
    }
    top
}

/// Channels of an output jack: 2 for a stereo jack.
fn port_width(editor: &PatchEditor, from: &PortRef) -> u8 {
    let PortRef::Module { id, port } = from else {
        return 1;
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
        .map_or(1, |p| p.width)
}

pub(crate) fn port_color(th: &Theme, editor: &PatchEditor, from: &PortRef) -> Color32 {
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

fn composite_layout(p: &PatchState, view: &UiState) -> rack::Layout {
    rack_editor::layout(p, view)
}
/// Shared world geometry for drawing, focus/reveal and zoomed hit tests.
fn authored_control_geo(
    p: &PatchState,
    _v: &UiState,
    lay: &Layout,
    leaf: ModuleId,
    param: &str,
) -> Option<Geo> {
    let target = PortRef::Param {
        id: leaf,
        param: param.into(),
    };
    for (id, face) in lay.faces.clone() {
        let c = &p.composites[&id];
        let Some(panel) = &c.panel else { continue };
        for (&key, e) in &c.controls {
            if e.target != target {
                continue;
            };
            let Some(place) = panel.placements.get(&key) else {
                continue;
            };
            return Some(panel_control_geo(face, place));
        }
    }
    None
}
fn panel_control_geo(face: Rect, p: &kabl_core::panel::Placement) -> Geo {
    let r = Rect::from_min_size(face.min + vec2(p.x, p.y), vec2(p.width, p.height));
    match p.kind {
        kabl_core::panel::Kind::Knob => Geo::Knob {
            c: pos2(r.center().x, r.top() + 56.0),
            r: ((p.width - 32.0) / 2.0).clamp(17.0, 32.0),
        },
        _ => Geo::Select {
            rect: Rect::from_min_max(r.min + vec2(2.0, 27.0), r.max - vec2(2.0, 5.0)),
        },
    }
}

fn draw_composites(
    editor: &mut PatchEditor,
    view: &mut UiState,
    ui: &mut egui::Ui,
    xf: Xf,
    drawn: &mut Drawn,
) {
    let snapshot = editor.state().clone();
    let lay = composite_layout(&snapshot, view);
    for (id, face) in lay.faces {
        let c = &snapshot.composites[&id];
        let rect = xf.r(face);
        rack_editor::body(view, ui, rack_editor::Item::Composite(id), face, xf);
        view.record(format!("composite:{id}:body"), rect);
        if view
            .moving
            .as_ref()
            .is_some_and(|mv| mv.item == rack_editor::Item::Composite(id) && !mv.cancelled)
        {
            ui.painter()
                .rect_filled(rect, 3., theme(&view.style).panel.linear_multiply(0.20));
            continue;
        }
        if view.selected_composite == Some(id) {
            ui.painter().rect_stroke(
                rect.expand(2.),
                3.,
                Stroke::new(2., theme(&view.style).sel),
                egui::StrokeKind::Outside,
            );
        }
        if c.panel.is_some() {
            draw_authored_face(editor, view, ui, xf, drawn, id, c, face);
            continue;
        }
        draw_default_face(editor, view, ui, drawn, id, c, &snapshot, rect);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_default_face(
    editor: &mut PatchEditor,
    view: &mut UiState,
    ui: &mut egui::Ui,
    drawn: &mut Drawn,
    id: kabl_core::CompositeId,
    c: &kabl_core::Composite,
    snapshot: &PatchState,
    rect: Rect,
) {
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .id_salt(("composite-face", id)),
        |ui| {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width((rect.width() - 16.0).max(120.0));
                ui.set_max_height(rect.height() - 16.0);
                ui.spacing_mut().slider_width = (rect.width() - 92.0).max(20.0);
                ui.set_clip_rect(ui.clip_rect().intersect(rect.expand(4.0)));
                ui.add(
                    egui::Label::new(egui::RichText::new(format!("{} · #{id}", c.name)).strong())
                        .selectable(false),
                );
                let r = ui.small_button("Edit face");
                view.record(format!("composite:{id}:design"), r.rect);
                if r.clicked() {
                    panels::open(view, id, c);
                }
                let r = ui.button("Inspect internals");
                view.record(format!("composite:{id}:open"), r.rect);
                if r.clicked() {
                    view.enter_composite(editor.state(), id);
                }
                egui::ScrollArea::vertical()
                    .max_height((rect.height() - 80.0).max(100.0))
                    .show_owned(ui, |ui| {
                        for (&key, e) in &c.ports {
                            let dir = composites::direction(snapshot, &e.target)
                                .unwrap_or(PortDirection::Input);
                            let label = if dir == PortDirection::Output {
                                format!("OUT {}", e.label)
                            } else {
                                format!("IN {}", e.label)
                            };
                            let r = ui.button(label);
                            if !ui.clip_rect().intersects(r.rect) {
                                continue;
                            }
                            view.record(format!("composite:{id}:port:{key}"), r.rect);
                            match &e.target {
                                PortRef::Module { id, port } => {
                                    drawn
                                        .ports
                                        .insert((*id, dir, port.clone()), r.rect.center());
                                    view.record(
                                        format!(
                                            "{}:{id}.{port}",
                                            if dir == PortDirection::Output {
                                                "out"
                                            } else {
                                                "in"
                                            }
                                        ),
                                        r.rect,
                                    );
                                }
                                PortRef::CableParam { .. } => {}
                                PortRef::Param { id, param } => {
                                    view.record(format!("knob:{id}.{param}"), r.rect);
                                    for (&cid, cable) in &snapshot.cables {
                                        if cable.to == e.target {
                                            drawn.plugs.push((cid, r.rect.center()));
                                        }
                                    }
                                }
                            }
                            if r.clicked() {
                                if dir == PortDirection::Output {
                                    view.pending_output = Some(e.target.clone());
                                    view.last_message =
                                        Some("Choose an input to connect this output.".into());
                                } else if let Some(from) = view.pending_output.take() {
                                    editor.connect(from, e.target.clone());
                                }
                            }
                            let st = view.style.clone();
                            kit::context_menu(&r, &st, |ui| {
                                if kit::menu_item(ui, &st, "Inspect bound module", false).clicked()
                                {
                                    let leaf = e.target.module_id().unwrap_or_default();
                                    let owner = editor
                                        .state()
                                        .composites
                                        .iter()
                                        .find(|(_, c)| c.members.contains(&leaf))
                                        .map_or(id, |(&id, _)| id);
                                    view.enter_composite(editor.state(), owner);
                                    view.selected_module = Some(leaf);
                                    view.reveal = Some(leaf);
                                    view.drawer_open = true;
                                    ui.close();
                                }
                            });
                        }
                        for (&key, e) in &c.controls {
                            if let PortRef::Param { id: leaf, param } = &e.target {
                                if let Some(info) = registry::info_for(&snapshot.modules[leaf].kind)
                                {
                                    if let Some(param) =
                                        info.params.iter().find(|p| p.name == param)
                                    {
                                        ui.label(&e.label);
                                        perform::param_editor(
                                            editor,
                                            view,
                                            ui,
                                            *leaf,
                                            param,
                                            false,
                                            Some(format!("composite:{id}:control:{key}")),
                                        );
                                        if ui.small_button("Reveal target / routes").clicked() {
                                            view.enter_composite(editor.state(), id);
                                            view.inspected = Some((*leaf, param.name.into()));
                                            view.reveal = Some(*leaf);
                                            view.drawer_open = true;
                                        }
                                    }
                                }
                            }
                        }
                    });
            });
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_authored_face(
    editor: &mut PatchEditor,
    view: &mut UiState,
    ui: &mut egui::Ui,
    xf: Xf,
    drawn: &mut Drawn,
    id: kabl_core::CompositeId,
    c: &kabl_core::Composite,
    face: Rect,
) {
    use kabl_core::panel::Kind;
    let panel = c.panel.as_ref().unwrap();
    let mut th = theme::panel_theme(&view.style, "filter.svf");
    let rect = xf.r(face);
    let z = xf.zoom;
    let painter = ui.painter_at(rect.intersect(ui.clip_rect()));
    kit::face::surface(&painter, &view.style, rect, th.panel, th.panel_edge, z);
    let art = if view.dark { &panel.dark } else { &panel.light };
    let mut missing = art.is_none();
    if let Some(art) = art {
        let key = (id, view.dark);
        let changed = view
            .panels
            .textures
            .get(&key)
            .is_none_or(|(old, _, _)| old != art);
        if changed {
            view.panels.textures.remove(&key);
            match panels::decode_art(art) {
                Ok(image) => {
                    let size = [image.width() as usize, image.height() as usize];
                    let tex = ui.ctx().load_texture(
                        format!("panel-{id}-{}", view.dark),
                        egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw()),
                        egui::TextureOptions::LINEAR,
                    );
                    let pixel = image.get_pixel(0, 0).0;
                    let tone = Color32::from_rgb(pixel[0], pixel[1], pixel[2]);
                    view.panels.textures.insert(key, (art.clone(), tex, tone));
                }
                Err(_) => missing = true,
            }
        }
        if let Some((_, tex, tone)) = view.panels.textures.get(&key) {
            th.panel = *tone;
            let linear = |v: u8| {
                let x = v as f32 / 255.0;
                if x <= 0.04045 {
                    x / 12.92
                } else {
                    ((x + 0.055) / 1.055).powf(2.4)
                }
            };
            let luminance =
                0.2126 * linear(tone.r()) + 0.7152 * linear(tone.g()) + 0.0722 * linear(tone.b());
            th.ink = if luminance > 0.179 {
                Color32::BLACK
            } else {
                Color32::WHITE
            };
            th.ink2 = th.ink;
            painter.image(
                tex.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }
    // A quiet printed header/footer also provide guaranteed contrast over arbitrary art.
    let header = xf.r(Rect::from_min_size(face.min, vec2(panel.width, 60.0)));
    painter.rect_filled(header, 0.0, th.panel);
    let title = if c.name.chars().count() > 32 {
        format!("{}…", c.name.chars().take(31).collect::<String>())
    } else {
        c.name.clone()
    };
    text(
        &painter,
        header.left_top() + vec2(12.0, 17.0) * z,
        egui::Align2::LEFT_CENTER,
        &title.to_uppercase(),
        15.0 * z,
        th.ink,
        false,
    );
    let open = Rect::from_min_size(
        header.left_top() + vec2(10.0, 31.0) * z,
        vec2(112.0, 23.0) * z,
    );
    let r = ui.interact(open, Id::new(("face-internals", id)), Sense::click());
    view.record(format!("composite:{id}:open"), open);
    text(
        &painter,
        open.center(),
        egui::Align2::CENTER_CENTER,
        "Inspect internals",
        11.0 * z,
        th.ink2,
        false,
    );
    if r.clicked() {
        view.enter_composite(editor.state(), id);
    }
    let edit = Rect::from_min_size(
        pos2(header.right() - 66.0 * z, header.top() + 31.0 * z),
        vec2(56.0, 23.0) * z,
    );
    let r = ui.interact(edit, Id::new(("face-design", id)), Sense::click());
    view.record(format!("composite:{id}:design"), edit);
    if r.hovered() {
        text(
            &painter,
            edit.center(),
            egui::Align2::CENTER_CENTER,
            "Edit face",
            11.0 * z,
            th.ink2,
            false,
        );
    }
    if r.clicked() {
        panels::open(view, id, c);
    }
    let fallback = xf.r(Rect::from_min_size(
        pos2(face.left() + 8.0, face.bottom() - 28.0),
        vec2(panel.width - 16.0, 22.0),
    ));
    painter.rect_filled(fallback, 3.0, th.panel);
    let r = ui.interact(fallback, Id::new(("face-fallback", id)), Sense::click());
    view.record(format!("composite:{id}:fallback"), fallback);
    text(
        &painter,
        fallback.center(),
        egui::Align2::CENTER_CENTER,
        if missing {
            "No artwork · Public controls / repair"
        } else {
            "More controls / help"
        },
        11.0 * z,
        th.ink,
        false,
    );
    if r.clicked() {
        view.panels.fallback = Some(id);
    }
    for (&key, e) in &c.ports {
        if !panel.placements.contains_key(&key) {
            register_face_port(
                drawn,
                &e.target,
                composites::direction(editor.state(), &e.target).unwrap_or(PortDirection::Input),
                fallback.center(),
                editor.state(),
            );
        }
    }
    for (&key, p) in &panel.placements {
        let footprint = Rect::from_min_size(face.min + vec2(p.x, p.y), vec2(p.width, p.height));
        let screen = xf.r(footprint);
        let local = ui.new_child(egui::UiBuilder::new().max_rect(screen).id_salt((
            "face-control",
            id,
            key,
        )));
        let mut local = local;
        local.set_clip_rect(ui.clip_rect().intersect(rect));
        if p.kind == Kind::Jack {
            let Some(e) = c.ports.get(&key) else { continue };
            let dir =
                composites::direction(editor.state(), &e.target).unwrap_or(PortDirection::Input);
            let center = pos2(screen.center().x, screen.bottom() - 19.0 * z);
            let resp = local.interact(
                Rect::from_center_size(center, vec2(30.0, 30.0) * z),
                Id::new(("face-port", id, key)),
                Sense::click_and_drag(),
            );
            view.record(format!("composite:{id}:port:{key}"), resp.rect);
            painter.circle_filled(center, 13.0 * z, th.skirt);
            painter.circle_stroke(center, 11.0 * z, Stroke::new(1.0 * z, th.nut_edge));
            painter.circle_filled(center, 7.0 * z, th.hole_c);
            if resp.hovered() || resp.has_focus() {
                painter.circle_stroke(center, 15.0 * z, Stroke::new(1.5, th.sel));
            }
            register_face_port(drawn, &e.target, dir, center, editor.state());
            match &e.target {
                PortRef::Module { id: leaf, port } => view.record(
                    format!(
                        "{}:{leaf}.{port}",
                        if dir == PortDirection::Output {
                            "out"
                        } else {
                            "in"
                        }
                    ),
                    resp.rect,
                ),
                PortRef::Param { id: leaf, param } => {
                    view.record(format!("knob:{leaf}.{param}"), resp.rect)
                }
                PortRef::CableParam { .. } => {}
            }
            if resp.clicked() {
                if dir == PortDirection::Output {
                    view.pending_output = Some(e.target.clone());
                } else if let Some(from) = view.pending_output.take() {
                    editor.connect(from, e.target.clone());
                }
            }
            if resp.drag_started() && dir == PortDirection::Output {
                view.port_drag = Some(e.target.clone());
            }
            if resp.drag_stopped() {
                if let Some(from) = view.port_drag.take() {
                    drawn.drop_at = resp.interact_pointer_pos().map(|at| (from, at));
                }
            }
            face_label(
                &painter,
                Rect::from_min_size(screen.min, vec2(screen.width(), 22.0 * z)),
                &format!(
                    "{} {}",
                    if dir == PortDirection::Output {
                        "OUT"
                    } else {
                        "IN"
                    },
                    e.label
                ),
                z,
                &th,
            );
            resp.on_hover_text(format!("{} · {:?}", e.label, e.target));
        } else {
            let Some(e) = c.controls.get(&key) else {
                continue;
            };
            let PortRef::Param { id: leaf, param } = &e.target else {
                continue;
            };
            let Some(info) = registry::info_for(&editor.state().modules[leaf].kind) else {
                continue;
            };
            let Some(param) = info.params.iter().find(|p| p.name == param) else {
                continue;
            };
            let look = Look {
                z,
                ink: th.ink,
                ink2: th.ink2,
                slot: 76.0,
            };
            // Contrast plates guarantee labels and values remain usable over any supplied image.
            if p.kind == Kind::Knob {
                let value = Rect::from_min_size(
                    screen.min + vec2(0.0, 83.0) * z,
                    vec2(screen.width(), 24.0 * z),
                );
                painter.rect_filled(value, 2.0, th.panel);
            }
            let plugs = match panel_control_geo(face, p) {
                Geo::Knob { c: cen, r } => routing::param_knob(
                    editor,
                    view,
                    &mut local,
                    &painter,
                    *leaf,
                    param,
                    xf.p(cen),
                    r,
                    look,
                ),
                Geo::Select { rect } => routing::stepped_selector(
                    editor,
                    view,
                    &mut local,
                    &painter,
                    *leaf,
                    info.kind,
                    param,
                    xf.r(rect),
                    look,
                ),
            };
            drawn.plugs.extend(plugs);
            view.record(format!("composite:{id}:control:{key}"), screen);
            face_label(
                &painter,
                Rect::from_min_size(screen.min, vec2(screen.width(), 22.0 * z)),
                &e.label,
                z,
                &th,
            );
        }
    }
}
fn face_label(p: &egui::Painter, r: Rect, label: &str, z: f32, th: &Theme) {
    p.rect_filled(r, 2.0, th.panel);
    let mut job = egui::text::LayoutJob::simple_singleline(
        label.into(),
        egui::FontId::proportional(12.0 * z),
        th.ink,
    );
    job.wrap.max_width = r.width() - 4.0 * z;
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = p.layout_job(job);
    p.galley(
        pos2(
            r.center().x - galley.size().x / 2.0,
            r.center().y - galley.size().y / 2.0,
        ),
        galley,
        th.ink,
    );
}
fn register_face_port(
    drawn: &mut Drawn,
    target: &PortRef,
    dir: PortDirection,
    at: Pos2,
    state: &PatchState,
) {
    match target {
        PortRef::Module { id, port } => {
            drawn.ports.insert((*id, dir, port.clone()), at);
        }
        PortRef::Param { .. } => {
            for (&cid, c) in &state.cables {
                if &c.to == target {
                    drawn.plugs.push((cid, at));
                }
            }
        }
        // A cable's own parameter has no face port: its routes live in the pattern editor.
        PortRef::CableParam { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::port_label;

    #[test]
    fn messages_become_toasts_of_the_right_severity() {
        use crate::kit::g4::Sev;
        let (sev, title, detail) = super::toast_for("Could not load patch: unknown module kind");
        assert_eq!((sev, title.as_str()), (Sev::Bad, "Could not load patch"));
        assert_eq!(detail.as_deref(), Some("unknown module kind"));
        assert_eq!(super::toast_for("Audio device busy").0, Sev::Warn);
        assert_eq!(super::toast_for("opened \"Evolving Pad\"").0, Sev::Good);
    }

    #[test]
    fn transport_readout_counts_bars_beats_and_ticks() {
        let mut editor = crate::PatchEditor::new();
        let mut view = super::UiState::default();
        assert!(super::transport_readout(editor.state(), &view).is_none());
        let id = editor.add_module("clock", kabl_core::Vec2 { x: 0.0, y: 0.0 });
        assert!(
            super::transport_readout(editor.state(), &view).is_none(),
            "no report yet"
        );
        editor.set_param(id, "bpm", 100.0);
        // Pulse 0 is 1 . 1 . 1; 4 pulses to a beat, 16 to a bar: pulse 37 is bar 3, beat 2, tick 2.
        view.clock_pos.insert(id, 0.0);
        assert_eq!(
            super::transport_readout(editor.state(), &view),
            Some((100.0, "1 . 1 . 1".into()))
        );
        view.clock_pos.insert(id, 37.9);
        assert_eq!(
            super::transport_readout(editor.state(), &view).unwrap().1,
            "3 . 2 . 2"
        );
    }

    #[test]
    fn empty_rack_fit_keeps_import_coordinates_finite() {
        let mut view = super::UiState {
            canvas: egui::Rect::from_min_size(egui::pos2(0., 60.), egui::vec2(1280., 700.)),
            ..Default::default()
        };
        view.frame_world(egui::Rect::NOTHING, 1.5);
        assert!(view.pan.is_finite());
        assert!(view.zoom.is_finite());
        assert!(view.xf().p(egui::pos2(12., 10.)).is_finite());
    }

    #[test]
    fn authored_geometry_round_trips_zoom_and_desktop_scale() {
        let face = egui::Rect::from_min_size(egui::pos2(230., 380.), egui::vec2(420., 340.));
        let placement = kabl_core::panel::Placement {
            kind: kabl_core::panel::Kind::Knob,
            x: 112.,
            y: 72.,
            width: 84.,
            height: 110.,
        };
        let super::Geo::Knob { c, r } = super::panel_control_geo(face, &placement) else {
            panic!("knob")
        };
        assert_eq!(c, egui::pos2(384., 508.));
        assert_eq!(r, 26.);
        for zoom in [0.25, 0.82, 1., 1.25, 2.] {
            for scale in [1., 1.25, 1.5, 2.] {
                let xf = super::Xf {
                    origin: egui::pos2(31., 63.),
                    zoom,
                };
                let physical = xf.p(c) * scale;
                let hit = physical / scale;
                assert!(xf.inv(hit).distance(c) < 0.001);
                assert!(xf.r(super::Geo::Knob { c, r }.bounds()).contains(hit));
            }
        }
    }

    #[test]
    fn port_labels_read_like_panel_text() {
        assert_eq!(port_label("cutoff_cv"), "Cutoff CV");
        assert_eq!(port_label("resonance_cv"), "Res CV");
        assert_eq!(port_label("lp"), "LP");
        assert_eq!(port_label("in3"), "In 3");
        assert_eq!(port_label("velocity"), "Vel");
        assert_eq!(port_label("in"), "In");
        assert_eq!(port_label("a"), "A");
    }
}
