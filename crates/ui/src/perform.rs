//! The performance panel and MIDI CC control. Both are views of real parameters: a pin
//! (`pin.<param>` = its sort order) and a CC mapping (`cc.<param>` = channel × 128 + CC number)
//! are presentation params stored on the module they point at, so they save with the patch,
//! undo like any edit, never rebuild audio (`rack::is_presentation`), and vanish with their
//! module. A clock's `pin.transport` pins its Run/Stop and Restart buttons.
//!
//! A CC drives the stored base value through `PatchEditor::set_param_gesture`, like a mouse
//! drag: routes still add on top, and a continuous turn is one undo step. Soft takeover: a
//! mapping only moves its parameter once the hardware reaches the parameter's position (within
//! 1.5 steps of 127, or by crossing it). Any other change to the value (mouse, undo, load)
//! drops the pickup again, so the next turn never jumps.

use std::collections::HashMap;

use kabl_core::{ModuleId, ParamTarget, PatchState};
use kabl_modules::builtins::Transport;
use kabl_modules::{registry, ParamInfo, Taper};

use crate::{routing, PatchEditor, UiState};

pub const PIN_PREFIX: &str = "pin.";
pub const CC_PREFIX: &str = "cc.";
/// MIDI button mapping of a runtime action: `btn.<action>` = channel × 128 + CC, on the module
/// the action belongs to (`bank.<0..3>` on a sequencer, `cue.<1..8>` and `cancel` on a cues
/// module, `run` and `restart` on a clock).
pub const BTN_PREFIX: &str = "btn.";
/// After a (re)connect, messages this long only set button states, never fire: a controller
/// that reports its button positions on connect cannot trigger anything.
const BUTTON_GUARD_S: f64 = 0.5;
/// Pin key for a clock's transport buttons.
pub const TRANSPORT: &str = "transport";
/// Pin key for a sequencer's bank launch buttons.
pub const BANKS: &str = "banks";
/// Pin key for a cues module's cue buttons.
pub const CUE_PADS: &str = "cues";
/// Height of the docked performance panel.
pub const PANEL_H: f32 = 340.0;
/// How close (in 0..1 travel) the hardware must come to pick a parameter up.
const PICKUP: f32 = 1.5 / 127.0;
/// A CC message this long after the previous one on the same mapping starts a new undo step.
const GESTURE_GAP_S: f64 = 1.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Pin {
    pub id: ModuleId,
    /// A param name, or `TRANSPORT`.
    pub key: String,
    pub order: f32,
}

/// Every valid pin in panel order. Pins naming a param the module doesn't have are skipped.
pub fn pins(state: &PatchState) -> Vec<Pin> {
    let mut out = Vec::new();
    for (&id, m) in &state.modules {
        let Some(info) = registry::info_for(&m.kind) else {
            continue;
        };
        for (name, &order) in m.params.range(PIN_PREFIX.to_string()..) {
            let Some(key) = name.strip_prefix(PIN_PREFIX) else {
                break;
            };
            let valid = match key {
                TRANSPORT => m.kind == "clock",
                BANKS => m.kind == "seq",
                CUE_PADS => m.kind == "cues",
                _ => info.params.iter().any(|p| p.name == key),
            };
            if valid {
                out.push(Pin {
                    id,
                    key: key.to_string(),
                    order,
                });
            }
        }
    }
    out.sort_by(|a, b| {
        (a.order, a.id, &a.key)
            .partial_cmp(&(b.order, b.id, &b.key))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

pub fn is_pinned(state: &PatchState, id: ModuleId, key: &str) -> bool {
    state
        .modules
        .get(&id)
        .is_some_and(|m| m.params.contains_key(&format!("{PIN_PREFIX}{key}")))
}

/// Pins at the end of the panel, or unpins.
pub fn toggle_pin(editor: &mut PatchEditor, id: ModuleId, key: &str) {
    let k = format!("{PIN_PREFIX}{key}");
    if is_pinned(editor.state(), id, key) {
        editor.set_presentation(&[(id, k, None)]);
    } else {
        let next = pins(editor.state())
            .last()
            .map_or(0.0, |p| p.order.floor() + 1.0);
        editor.set_presentation(&[(id, k, Some(next))]);
    }
}

/// Moves the pin at `index` one place left (`-1`) or right (`+1`): one undo step that
/// renumbers every pin.
pub fn move_pin(editor: &mut PatchEditor, index: usize, dir: isize) {
    let mut all = pins(editor.state());
    let Some(j) = index.checked_add_signed(dir).filter(|&j| j < all.len()) else {
        return;
    };
    all.swap(index, j);
    let changes: Vec<_> = all
        .iter()
        .enumerate()
        .map(|(i, p)| (p.id, format!("{PIN_PREFIX}{}", p.key), Some(i as f32)))
        .collect();
    editor.set_presentation(&changes);
}

pub fn learnable(p: &ParamInfo) -> bool {
    p.taper != Taper::Stepped
}

/// `(channel 0..15, controller 0..127)` mapped to this param.
pub fn mapping(state: &PatchState, id: ModuleId, param: &str) -> Option<(u8, u8)> {
    let v = *state
        .modules
        .get(&id)?
        .params
        .get(&format!("{CC_PREFIX}{param}"))?;
    let code = v as u32;
    (v >= 0.0 && code < 16 * 128).then_some(((code / 128) as u8, (code % 128) as u8))
}

pub fn cc_text((ch, cc): (u8, u8)) -> String {
    format!("CC {cc} · ch {}", ch + 1)
}

/// Every param mapped to this CC.
fn mapped(state: &PatchState, ch: u8, cc: u8) -> Vec<(ModuleId, &'static ParamInfo)> {
    let mut out = Vec::new();
    for (&id, m) in &state.modules {
        let Some(info) = registry::info_for(&m.kind) else {
            continue;
        };
        for p in info.params {
            if mapping(state, id, p.name) == Some((ch, cc)) {
                out.push((id, p));
            }
        }
    }
    out
}

/// Soft-takeover state of one mapping (runtime only).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Takeover {
    pub picked: bool,
    /// Last hardware position, 0..1.
    pub hw: Option<f32>,
    /// The value this mapping last wrote; a different stored value means something else
    /// changed it.
    pub sent: Option<f32>,
}

impl Takeover {
    /// Feeds one hardware position against the parameter's position (both 0..1 of travel).
    /// Returns the position to set once picked up.
    pub fn feed(&mut self, hw: f32, param: f32) -> Option<f32> {
        if !self.picked {
            let near = (hw - param).abs() <= PICKUP;
            let crossed = self.hw.is_some_and(|h| (h - param) * (hw - param) <= 0.0);
            self.picked = near || crossed;
        }
        self.hw = Some(hw);
        self.picked.then_some(hw)
    }
}

/// Drops the pickup of every mapping whose value was changed by something else, and forgets
/// mappings that no longer exist.
pub fn sync_takeover(editor: &PatchEditor, ui_state: &mut UiState) {
    let state = editor.state();
    ui_state.takeover.retain(|(id, param), t| {
        let Some(p) = state
            .modules
            .get(id)
            .and_then(|m| registry::info_for(&m.kind))
            .and_then(|i| i.params.iter().find(|p| p.name == param))
        else {
            return false;
        };
        if mapping(state, *id, param).is_none() {
            return false;
        }
        if t.picked && t.sent != Some(routing::base_value(state, *id, p)) {
            t.picked = false;
        }
        true
    });
}

/// After a (re)connect (`UiState::button_rearm`): forget button states and start the guard
/// from `now`. The control thread calls it every few milliseconds, so the guard starts at the
/// connect, not at the first message.
pub fn rearm(ui_state: &mut UiState, now: f64) {
    if std::mem::take(&mut ui_state.button_rearm) {
        ui_state.button_high.clear();
        ui_state.button_guard = now + BUTTON_GUARD_S;
    }
}

/// Applies incoming CC messages `(channel, controller, value)`: a pending learn takes the
/// first one; otherwise each drives the params mapped to it. Runs on the control thread
/// (`control::midi`), not in an editor frame; `now` is seconds on one steady clock.
pub fn apply_cc(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    events: &[(u8, u8, u8)],
    now: f64,
) {
    rearm(ui_state, now);
    for &(ch, cc, value) in events {
        let hw = value.min(127) as f32 / 127.0;
        let high = value >= 64;
        let was_high = ui_state.button_high.insert((ch, cc), high).unwrap_or(false);
        if let Some((id, param)) = ui_state.learn.take() {
            match param.strip_prefix(BTN_PREFIX) {
                Some(action) => learn_button(editor, ui_state, id, action, ch, cc),
                None => learn(editor, ui_state, id, &param, ch, cc, hw),
            }
            continue;
        }
        // Buttons: one action per press (low → high); release and repeats do nothing.
        if high && !was_high && now >= ui_state.button_guard {
            for (id, action) in mapped_buttons(editor.state(), ch, cc) {
                fire(editor, ui_state, id, &action);
            }
        }
        sync_takeover(editor, ui_state);
        for (id, p) in mapped(editor.state(), ch, cc) {
            let key = (id, p.name.to_string());
            let base = routing::base_value(editor.state(), id, p);
            let t = ui_state.takeover.entry(key.clone()).or_default();
            let was = t.picked;
            let at = p.to_norm(base);
            let Some(n) = t.feed(hw, at) else {
                continue;
            };
            // Picked up right at the value: nothing to write (no nudge, no undo step).
            if !was && (n - at).abs() <= PICKUP {
                t.sent = Some(base);
                continue;
            }
            let value = p.from_norm(n);
            t.sent = Some(value);
            if value == base {
                continue;
            }
            // One gesture while CCs keep coming (on any mappings) with gaps under a second.
            let continuing = ui_state
                .cc_gesture
                .as_ref()
                .is_some_and(|(_, at)| now - at < GESTURE_GAP_S);
            let target = ParamTarget::Module {
                id,
                param: p.name.into(),
            };
            editor.set_param_cc(target, value, continuing);
            ui_state.cc_gesture = Some((key, now));
        }
    }
}

fn learn(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    id: ModuleId,
    param: &str,
    ch: u8,
    cc: u8,
    hw: f32,
) {
    let state = editor.state();
    let Some(p) = state
        .modules
        .get(&id)
        .and_then(|m| registry::info_for(&m.kind))
        .and_then(|i| i.params.iter().find(|p| p.name == param))
    else {
        return;
    };
    // One CC drives one param: the new mapping replaces any other use of it.
    let mut changes: Vec<_> = mapped(state, ch, cc)
        .into_iter()
        .filter(|&(oid, op)| (oid, op.name) != (id, param))
        .map(|(oid, op)| (oid, format!("{CC_PREFIX}{}", op.name), None))
        .collect();
    changes.extend(
        mapped_buttons(state, ch, cc)
            .into_iter()
            .map(|(oid, a)| (oid, format!("{BTN_PREFIX}{a}"), None)),
    );
    changes.push((
        id,
        format!("{CC_PREFIX}{param}"),
        Some((ch as u32 * 128 + cc as u32) as f32),
    ));
    let norm = p.to_norm(routing::base_value(state, id, p));
    let mut t = Takeover::default();
    t.feed(hw, norm);
    t.sent = Some(routing::base_value(state, id, p));
    editor.set_presentation(&changes);
    ui_state.takeover.insert((id, param.to_string()), t);
    ui_state.last_message = Some(format!(
        "{} → {} #{id} {}",
        cc_text((ch, cc)),
        module_name(editor.state(), id),
        routing::target_label(p)
    ));
}

/// The runtime actions module `id` offers to MIDI buttons: (action key, label).
pub fn button_actions(state: &PatchState, id: ModuleId) -> Vec<(String, String)> {
    let Some(m) = state.modules.get(&id) else {
        return Vec::new();
    };
    match m.kind.as_str() {
        "seq" => (0..kabl_modules::builtins::seq::BANKS)
            .map(|b| {
                (
                    format!("bank.{b}"),
                    format!("Launch bank {}", crate::banks::title(state, id, b)),
                )
            })
            .collect(),
        "cues" => crate::cues::cues(state, id)
            .into_iter()
            .map(|c| (format!("cue.{}", c.n), format!("Launch {}", c.name)))
            .chain([("cancel".to_string(), "Cancel queued launches".to_string())])
            .collect(),
        "clock" => vec![
            ("run".into(), "Run / Stop".into()),
            ("restart".into(), "Restart".into()),
        ],
        _ => Vec::new(),
    }
}

/// `(channel, controller)` of the button mapped to `action` on module `id`.
pub fn button_mapping(state: &PatchState, id: ModuleId, action: &str) -> Option<(u8, u8)> {
    let v = *state
        .modules
        .get(&id)?
        .params
        .get(&format!("{BTN_PREFIX}{action}"))?;
    let code = v as u32;
    (v >= 0.0 && code < 16 * 128).then_some(((code / 128) as u8, (code % 128) as u8))
}

/// Every (module, action) whose button is this CC.
fn mapped_buttons(state: &PatchState, ch: u8, cc: u8) -> Vec<(ModuleId, String)> {
    let code = (ch as u32 * 128 + cc as u32) as f32;
    let mut out = Vec::new();
    for (&id, m) in &state.modules {
        for (k, &v) in m.params.range(BTN_PREFIX.to_string()..) {
            let Some(action) = k.strip_prefix(BTN_PREFIX) else {
                break;
            };
            if v == code {
                out.push((id, action.to_string()));
            }
        }
    }
    out
}

fn learn_button(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    id: ModuleId,
    action: &str,
    ch: u8,
    cc: u8,
) {
    let state = editor.state();
    let Some((_, label)) = button_actions(state, id)
        .into_iter()
        .find(|(a, _)| a == action)
    else {
        return;
    };
    // One CC, one use: continuous mappings and other buttons on it are cleared.
    let mut changes: Vec<_> = mapped(state, ch, cc)
        .into_iter()
        .map(|(oid, op)| (oid, format!("{CC_PREFIX}{}", op.name), None))
        .collect();
    changes.extend(
        mapped_buttons(state, ch, cc)
            .into_iter()
            .filter(|(oid, a)| (*oid, a.as_str()) != (id, action))
            .map(|(oid, a)| (oid, format!("{BTN_PREFIX}{a}"), None)),
    );
    changes.push((
        id,
        format!("{BTN_PREFIX}{action}"),
        Some((ch as u32 * 128 + cc as u32) as f32),
    ));
    editor.set_presentation(&changes);
    ui_state.last_message = Some(format!(
        "{} button → {} #{id} · {label}",
        cc_text((ch, cc)),
        module_name(editor.state(), id)
    ));
}

/// Runs a button's action: a launch, a cancel or a transport command (runtime only).
fn fire(editor: &PatchEditor, ui_state: &mut UiState, id: ModuleId, action: &str) {
    if ui_state.host_clock && matches!(action, "run" | "restart") {
        ui_state.last_message = Some("Host clock: use REAPER transport or switch to Free".into());
        return;
    }
    let state = editor.state();
    let result = match action.split_once('.') {
        Some(("bank", b)) => b
            .parse()
            .map_err(|_| "bad action".to_string())
            .and_then(|b| crate::banks::launch(&mut ui_state.launches, state, id, b)),
        Some(("cue", n)) => n
            .parse()
            .map_err(|_| "bad action".to_string())
            .and_then(|n| crate::cues::launch(&mut ui_state.launches, state, id, n)),
        _ => {
            match action {
                "cancel" => ui_state
                    .launches
                    .push(kabl_engine::patch_engine::Command::Cancel(None)),
                // Decided on the audio thread from the clock's state: a report the UI has not
                // drawn yet cannot make the button do the wrong thing.
                "run" => ui_state.transport.push((id, Transport::Toggle)),
                "restart" => ui_state.transport.push((id, Transport::Restart)),
                _ => {}
            }
            Ok(())
        }
    };
    if let Err(e) = result {
        ui_state.last_message = Some(e);
    }
}

/// A button mapping's hover text: `CC 40 · ch 1 → Launch bank B`.
pub fn button_text(state: &PatchState, id: ModuleId, action: &str) -> Option<String> {
    let m = button_mapping(state, id, action)?;
    Some(format!("MIDI button {}", cc_text(m)))
}

pub fn unlearn(editor: &mut PatchEditor, id: ModuleId, param: &str) {
    editor.set_presentation(&[(id, format!("{CC_PREFIX}{param}"), None)]);
}

pub(crate) fn module_name(state: &PatchState, id: ModuleId) -> &'static str {
    state
        .modules
        .get(&id)
        .and_then(|m| registry::info_for(&m.kind))
        .map_or("?", |i| i.name)
}

/// A param's value control: option buttons for a stepped param, else a slider with the knob's
/// text. Edits go through the same gesture path as a knob drag.
pub(crate) fn param_editor(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    id: ModuleId,
    param: &'static ParamInfo,
    label: bool,
    hit: Option<String>,
) {
    let Some(kind) = editor.state().modules.get(&id).map(|m| m.kind.clone()) else {
        return;
    };
    let current = routing::base_value(editor.state(), id, param);
    if param.taper == Taper::Stepped {
        let r = ui.horizontal_wrapped(|ui| {
            if label {
                ui.label(routing::param_label(param));
            }
            let labels = routing::step_labels(&kind, param.name);
            let n = (param.max - param.min).round() as usize + 1;
            for k in 0..n {
                let opt = param.min + k as f32;
                let text = labels
                    .and_then(|l| l.get(k).copied())
                    .map_or(format!("{opt}"), str::to_string);
                let r = ui.selectable_label(current.round() == opt, text);
                if let Some(h) = &hit {
                    ui_state.record(format!("{h}.{k}"), r.rect);
                }
                if r.clicked() && current.round() != opt {
                    editor.set_param(id, param.name, opt);
                }
            }
        });
        if let Some(h) = hit {
            ui_state.record(h, r.response.rect);
        }
        return;
    }
    let mut value = current;
    // No `step_by`: egui snaps the shown value and reports it as a change every frame, which
    // would rewrite a stored 12.8 as 13 and undo could never get back past it.
    let mut slider = egui::Slider::new(&mut value, param.min..=param.max)
        .logarithmic(param.taper == Taper::Exponential)
        .custom_formatter(|v, _| routing::fmt_value(param, v as f32))
        .custom_parser(|t| routing::parse_value(param, t).map(f64::from));
    if label {
        slider = slider.text(routing::param_label(param));
    }
    let resp = ui.add(slider);
    if let Some(h) = hit {
        ui_state.record(h, resp.rect);
    }
    // Only a real user change: the log slider's round trip can differ from `current` (at the
    // range ends), and writing that back every frame flooded undo.
    if resp.changed() && value != current {
        let target = ParamTarget::Module {
            id,
            param: param.name.into(),
        };
        let continuing = resp.dragged() && !resp.drag_started();
        editor.set_param_gesture(target, value, !continuing);
    }
}

/// The label a pin shows: the user's, else the control's own name.
pub fn pin_label(state: &PatchState, pin: &Pin) -> String {
    if let Some(l) = state.label(pin.id, &format!("{PIN_PREFIX}{}", pin.key)) {
        return l.to_string();
    }
    match pin.key.as_str() {
        TRANSPORT => return "Transport".into(),
        BANKS => return "Banks".into(),
        CUE_PADS => return "Cues".into(),
        _ => {}
    }
    if let Some(name) = crate::macro_name(state, pin.id, &pin.key) {
        return name;
    }
    pin_param(state, pin).map_or(pin.key.clone(), routing::target_label)
}

pub(crate) fn pin_param(state: &PatchState, pin: &Pin) -> Option<&'static ParamInfo> {
    state
        .modules
        .get(&pin.id)
        .and_then(|m| registry::info_for(&m.kind))
        .and_then(|i| i.params.iter().find(|p| p.name == pin.key))
}

/// What a pin really is: `Mixer #26 · Level 1 · from Sequencer #3`.
pub fn pin_source(state: &PatchState, pin: &Pin) -> String {
    let control = match pin.key.as_str() {
        TRANSPORT => "Run/Stop, Restart".to_string(),
        BANKS => "Launch A–D".to_string(),
        CUE_PADS => "Launch cues".to_string(),
        _ => pin_param(state, pin).map_or(pin.key.clone(), routing::target_label),
    };
    let mut s = format!("{} #{} · {control}", module_name(state, pin.id), pin.id);
    if let Some(src) = mixer_source(state, pin.id, &pin.key) {
        s.push_str(&format!(" · from {src}"));
    }
    s
}

/// Renames a pin (empty = back to the control's own name). One undo step, no audio rebuild.
pub fn rename_pin(editor: &mut PatchEditor, id: ModuleId, key: &str, text: &str) {
    let text = text.trim();
    editor.set_label(
        id,
        &format!("{PIN_PREFIX}{key}"),
        (!text.is_empty()).then(|| text.to_string()),
    );
}

/// For a mixer channel level: the sequencer, MIDI input or oscillator its input comes from,
/// traced upstream through the audio cables (nearest sequencer or MIDI input first).
pub(crate) fn mixer_source(state: &PatchState, id: ModuleId, param: &str) -> Option<String> {
    let ch = param.strip_prefix("level")?;
    if state.modules.get(&id)?.kind != "mixer" {
        return None;
    }
    let into = |id: ModuleId, port: Option<&str>| -> Vec<ModuleId> {
        state
            .cables
            .values()
            .filter_map(|c| match (&c.to, &c.from) {
                (
                    kabl_core::PortRef::Module { id: to, port: p },
                    kabl_core::PortRef::Module { id: from, .. },
                ) if *to == id && port.is_none_or(|q| q == p) => Some(*from),
                _ => None,
            })
            .collect()
    };
    let mut queue: std::collections::VecDeque<ModuleId> = into(id, Some(&format!("in{ch}"))).into();
    let first = *queue.front()?;
    let mut seen = std::collections::BTreeSet::new();
    let mut osc = None;
    while let Some(m) = queue.pop_front() {
        if !seen.insert(m) {
            continue;
        }
        match state.modules.get(&m).map(|s| s.kind.as_str()) {
            Some("seq" | "midi.in") => return Some(routing::source_label(state, m, "out")),
            Some("osc.va") if osc.is_none() => osc = Some(m),
            _ => {}
        }
        queue.extend(into(m, None));
    }
    Some(routing::source_label(state, osc.unwrap_or(first), "out"))
}

/// Right-click menu entries for a module: pin toggles and MIDI learn.
pub fn module_menu(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    id: ModuleId,
    info: &'static kabl_modules::ModuleInfo,
) {
    let edit = ui_state.edit_bank_of(editor.state(), id);
    let shown = |p: &&ParamInfo| crate::rack::visible(info, p.name, edit);
    let st = ui_state.style.clone();
    let st = &*st;
    let r = crate::kit::submenu(ui, st, "Pin to Perform", |ui| {
        let special = match info.kind {
            "clock" => Some((TRANSPORT, "Transport (Run/Stop, Restart)")),
            "seq" => Some((BANKS, "Bank launch buttons")),
            "cues" => Some((CUE_PADS, "Cue buttons")),
            _ => None,
        };
        if let Some((key, text)) = special {
            let mut on = is_pinned(editor.state(), id, key);
            let r = crate::kit::toggle(ui, st, &mut on, text);
            ui_state.record(format!("menu:pin:{key}"), r.rect);
            if r.clicked() {
                toggle_pin(editor, id, key);
                ui_state.perform_open = true;
                ui_state.pin_reveal = Some((id, key.to_string()));
            }
        }
        for p in info.params.iter().filter(shown) {
            let mut on = is_pinned(editor.state(), id, p.name);
            let r = crate::kit::toggle(ui, st, &mut on, &routing::param_label(p));
            ui_state.record(format!("menu:pin:{}", p.name), r.rect);
            if r.clicked() {
                toggle_pin(editor, id, p.name);
                ui_state.perform_open = true;
                ui_state.pin_reveal = Some((id, p.name.to_string()));
            }
        }
    });
    ui_state.record("menu:pin".into(), r.rect);
    let actions = button_actions(editor.state(), id);
    if !actions.is_empty() {
        let r = crate::kit::submenu(ui, st, "MIDI button", |ui| {
            for (action, label) in actions {
                let map = button_mapping(editor.state(), id, &action);
                let text = match map {
                    Some(m) => format!("{label}  ({})", cc_text(m)),
                    None => label,
                };
                let r = crate::kit::menu_item(ui, st, &text, false);
                ui_state.record(format!("menu:button:{action}"), r.rect);
                if r.clicked() {
                    ui_state.learn = Some((id, format!("{BTN_PREFIX}{action}")));
                    ui_state.perform_open = true;
                    ui.close();
                }
                if map.is_some() {
                    let r = crate::kit::menu_item_tone(
                        ui,
                        st,
                        "Clear mapping",
                        false,
                        crate::kit::Tone::Text2,
                        true,
                    );
                    ui_state.record(format!("menu:button-clear:{action}"), r.rect);
                    if r.clicked() {
                        editor.set_presentation(&[(id, format!("{BTN_PREFIX}{action}"), None)]);
                        ui.close();
                    }
                }
            }
        });
        ui_state.record("menu:button".into(), r.rect);
    }
    if info.params.iter().any(learnable) {
        let r = crate::kit::submenu(ui, st, "MIDI learn", |ui| {
            for p in info.params.iter().filter(shown).filter(|p| learnable(p)) {
                let text = match mapping(editor.state(), id, p.name) {
                    Some(m) => format!("{}  ({})", routing::target_label(p), cc_text(m)),
                    None => routing::target_label(p),
                };
                let r = crate::kit::menu_item(ui, st, &text, false);
                ui_state.record(format!("menu:learn:{}", p.name), r.rect);
                if r.clicked() {
                    ui_state.learn = Some((id, p.name.to_string()));
                    ui_state.perform_open = true;
                    ui_state.pin_reveal = Some((id, p.name.to_string()));
                    ui.close();
                }
            }
        });
        ui_state.record("menu:learn".into(), r.rect);
    }
}

/// Runtime MIDI pickup state, keyed by (module, param).
pub type TakeoverMap = HashMap<(ModuleId, String), Takeover>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_clock_blocks_rack_and_midi_transport_only() {
        let editor = PatchEditor::new();
        let mut view = UiState {
            host_clock: true,
            ..UiState::default()
        };
        fire(&editor, &mut view, 1, "run");
        fire(&editor, &mut view, 1, "restart");
        assert!(view.transport.is_empty());
        fire(&editor, &mut view, 1, "cancel");
        assert_eq!(view.launches.len(), 1);
        view.host_clock = false;
        fire(&editor, &mut view, 1, "run");
        assert_eq!(view.transport.len(), 1);
    }

    #[test]
    fn takeover_waits_for_the_hardware_to_reach_the_value() {
        let mut t = Takeover::default();
        // Hardware far below the parameter: nothing moves until it crosses 0.5.
        assert_eq!(t.feed(0.1, 0.5), None);
        assert_eq!(t.feed(0.3, 0.5), None);
        assert_eq!(t.feed(0.6, 0.5), Some(0.6));
        assert_eq!(t.feed(0.2, 0.6), Some(0.2));
        // Near enough counts at once, from either side.
        let mut t = Takeover::default();
        assert_eq!(t.feed(0.505, 0.5), Some(0.505));
        let mut t = Takeover::default();
        assert_eq!(t.feed(0.9, 0.5), None);
        assert_eq!(t.feed(0.45, 0.5), Some(0.45));
    }
}
