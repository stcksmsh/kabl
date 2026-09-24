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

use egui::RichText;
use kabl_core::{ModuleId, ParamTarget, PatchState};
use kabl_modules::builtins::Transport;
use kabl_modules::{registry, ParamInfo, Taper};

use crate::{routing, PatchEditor, UiState};

pub const PIN_PREFIX: &str = "pin.";
pub const CC_PREFIX: &str = "cc.";
/// Pin key for a clock's transport buttons.
pub const TRANSPORT: &str = "transport";
/// Height of the performance panel.
pub const PANEL_H: f32 = 206.0;
const CARD_W: f32 = 184.0;
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
            let valid = if key == TRANSPORT {
                m.kind == "clock"
            } else {
                info.params.iter().any(|p| p.name == key)
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

/// Applies incoming CC messages `(channel, controller, value)`: a pending learn takes the
/// first one; otherwise each drives the params mapped to it.
pub fn apply_cc(editor: &mut PatchEditor, ui_state: &mut UiState, now: f64) {
    let events = std::mem::take(&mut ui_state.midi_cc);
    for (ch, cc, value) in events {
        let hw = value.min(127) as f32 / 127.0;
        if let Some((id, param)) = ui_state.learn.take() {
            learn(editor, ui_state, id, &param, ch, cc, hw);
            continue;
        }
        sync_takeover(editor, ui_state);
        for (id, p) in mapped(editor.state(), ch, cc) {
            let key = (id, p.name.to_string());
            let base = routing::base_value(editor.state(), id, p);
            let t = ui_state.takeover.entry(key.clone()).or_default();
            let was = t.picked;
            let Some(n) = t.feed(hw, p.to_norm(base)) else {
                continue;
            };
            let value = p.from_norm(n);
            t.sent = Some(value);
            if value == base {
                continue;
            }
            let first = !was
                || !matches!(&ui_state.cc_gesture,
                    Some((g, at)) if *g == key && now - at < GESTURE_GAP_S);
            let target = ParamTarget::Module {
                id,
                param: p.name.into(),
            };
            editor.set_param_gesture(target, value, first);
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
        routing::param_label(p)
    ));
}

pub fn unlearn(editor: &mut PatchEditor, id: ModuleId, param: &str) {
    editor.set_presentation(&[(id, format!("{CC_PREFIX}{param}"), None)]);
}

fn module_name(state: &PatchState, id: ModuleId) -> &'static str {
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
                if ui.selectable_label(current.round() == opt, text).clicked()
                    && current.round() != opt
                {
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

/// The panel: header (MIDI input, learn state, recorder) and one card per pin.
pub fn panel(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui) {
    let th = crate::theme::theme(ui_state.dark);
    ui.horizontal(|ui| {
        ui.heading("Perform");
        ui.separator();
        ui.label("MIDI in");
        let current = ui_state.midi_input.clone().unwrap_or_else(|| "none".into());
        let combo = egui::ComboBox::from_id_salt("midi-input")
            .width(180.0)
            .selected_text(current)
            .show_ui(ui, |ui| {
                let mut pick = None;
                if ui
                    .selectable_label(ui_state.midi_input.is_none(), "none")
                    .clicked()
                {
                    pick = Some(None);
                }
                for name in &ui_state.midi_inputs {
                    let on = ui_state.midi_input.as_ref() == Some(name);
                    if ui.selectable_label(on, name).clicked() {
                        pick = Some(Some(name.clone()));
                    }
                }
                pick
            });
        ui_state.record("midi-input".into(), combo.response.rect);
        if let Some(pick) = combo.inner.flatten() {
            ui_state.midi_select = Some(pick);
        }
        if let Some((id, param)) = ui_state.learn.clone() {
            let label = registry::info_for(
                editor
                    .state()
                    .modules
                    .get(&id)
                    .map_or("", |m| m.kind.as_str()),
            )
            .and_then(|i| i.params.iter().find(|p| p.name == param))
            .map_or(param.clone(), routing::param_label);
            ui.label(
                RichText::new(format!(
                    "Learning {} #{id} {label}: move a control…",
                    module_name(editor.state(), id)
                ))
                .color(th.cv)
                .strong(),
            );
            let r = ui.button("Cancel");
            ui_state.record("learn-cancel".into(), r.rect);
            if r.clicked() {
                ui_state.learn = None;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            crate::record::controls(ui_state, ui, &th);
        });
    });
    ui.separator();
    let all = pins(editor.state());
    if all.is_empty() {
        ui.label(
            "No pinned controls. Right-click a module and choose Pin to Perform, \
             or MIDI learn to map a hardware control.",
        );
        return;
    }
    // Transport stays in reach at the left; the other cards scroll.
    let n = all.len();
    ui.horizontal_top(|ui| {
        for (i, pin) in all.iter().enumerate().filter(|(_, p)| p.key == TRANSPORT) {
            card(editor, ui_state, ui, &th, pin, i, n);
        }
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal_top(|ui| {
                for (i, pin) in all.iter().enumerate().filter(|(_, p)| p.key != TRANSPORT) {
                    card(editor, ui_state, ui, &th, pin, i, n);
                }
            });
        });
    });
}

fn card(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    th: &crate::theme::Theme,
    pin: &Pin,
    index: usize,
    count: usize,
) {
    let key = format!("{}.{}", pin.id, pin.key);
    let size = egui::vec2(CARD_W + 18.0, PANEL_H - 60.0);
    let frame = ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
        egui::Frame::group(ui.style())
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.set_width(CARD_W);
                ui.set_min_height(PANEL_H - 70.0);
                let name = format!("{} #{}", module_name(editor.state(), pin.id), pin.id);
                egui::Sides::new().show(
                    ui,
                    |ui| {
                        ui.label(RichText::new(name).weak().small());
                    },
                    |ui| {
                        let small = |t: &str| egui::Button::new(RichText::new(t).small());
                        let r = ui.add(small("✕")).on_hover_text("Unpin");
                        ui_state.record(format!("punpin:{key}"), r.rect);
                        if r.clicked() {
                            toggle_pin(editor, pin.id, &pin.key);
                        }
                        let r = ui
                            .add_enabled(index + 1 < count, small("▶"))
                            .on_hover_text("Move right");
                        ui_state.record(format!("pright:{key}"), r.rect);
                        if r.clicked() {
                            move_pin(editor, index, 1);
                        }
                        let r = ui
                            .add_enabled(index > 0, small("◀"))
                            .on_hover_text("Move left");
                        ui_state.record(format!("pleft:{key}"), r.rect);
                        if r.clicked() {
                            move_pin(editor, index, -1);
                        }
                    },
                );
                if pin.key == TRANSPORT {
                    transport_card(ui_state, ui, th, pin.id);
                    return;
                }
                let Some(p) = editor
                    .state()
                    .modules
                    .get(&pin.id)
                    .and_then(|m| registry::info_for(&m.kind))
                    .and_then(|i| i.params.iter().find(|p| p.name == pin.key))
                else {
                    return;
                };
                ui.label(RichText::new(routing::param_label(p)).strong().size(16.0));
                if let Some(src) = mixer_source(editor.state(), pin.id, p.name) {
                    ui.label(RichText::new(format!("from {src}")).small());
                }
                ui.spacing_mut().slider_width = CARD_W - 70.0;
                param_editor(
                    editor,
                    ui_state,
                    ui,
                    pin.id,
                    p,
                    false,
                    Some(format!("pslider:{key}")),
                );
                if learnable(p) {
                    midi_row(editor, ui_state, ui, th, pin.id, p);
                }
            });
    });
    ui_state.record(format!("pcard:{key}"), frame.response.rect);
    if ui_state.pin_reveal.as_ref() == Some(&(pin.id, pin.key.clone())) {
        ui.scroll_to_rect(frame.response.rect, None);
        ui_state.pin_reveal = None;
    }
}

/// CC assignment, Learn/Clear, and the pickup state.
fn midi_row(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    th: &crate::theme::Theme,
    id: ModuleId,
    p: &'static ParamInfo,
) {
    let key = format!("{id}.{}", p.name);
    let map = mapping(editor.state(), id, p.name);
    let learning = ui_state.learn.as_ref() == Some(&(id, p.name.to_string()));
    ui.horizontal(|ui| {
        match map {
            Some(m) => ui.label(RichText::new(cc_text(m)).monospace()),
            None => ui.label(RichText::new("no CC").weak()),
        };
        let r = ui.selectable_label(learning, if learning { "Learning…" } else { "Learn" });
        ui_state.record(format!("plearn:{key}"), r.rect);
        if r.clicked() {
            ui_state.learn = (!learning).then(|| (id, p.name.to_string()));
        }
        if map.is_some() {
            let r = ui.button("Clear").on_hover_text("Remove the MIDI mapping");
            ui_state.record(format!("pclear:{key}"), r.rect);
            if r.clicked() {
                unlearn(editor, id, p.name);
            }
        }
    });
    if map.is_none() {
        return;
    }
    let t = ui_state
        .takeover
        .get(&(id, p.name.to_string()))
        .copied()
        .unwrap_or_default();
    let text = if t.picked {
        RichText::new("● hardware in control").color(th.gate)
    } else {
        let at = p.to_norm(routing::base_value(editor.state(), id, p));
        let hint = match t.hw {
            Some(h) if h < at => format!("pickup: turn up to {:.0} %", at * 100.0),
            Some(_) => format!("pickup: turn down to {:.0} %", at * 100.0),
            None => format!("pickup at {:.0} %", at * 100.0),
        };
        RichText::new(hint).color(th.cv)
    };
    let r = ui.label(text.small());
    ui_state.record(format!("ppickup:{key}"), r.rect);
}

/// For a mixer channel level: the sequencer, MIDI input or oscillator its input comes from,
/// traced upstream through the audio cables (nearest sequencer or MIDI input first).
fn mixer_source(state: &PatchState, id: ModuleId, param: &str) -> Option<String> {
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

fn transport_card(
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    th: &crate::theme::Theme,
    id: ModuleId,
) {
    let running = ui_state.clock_running.get(&id).copied().unwrap_or(true);
    ui.label(RichText::new("Transport").strong().size(16.0));
    let state = RichText::new(if running { "● running" } else { "stopped" });
    ui.label(if running {
        state.color(th.gate)
    } else {
        state.weak()
    });
    ui.horizontal(|ui| {
        let big =
            |t: &str| egui::Button::new(RichText::new(t).size(15.0)).min_size([76.0, 34.0].into());
        let r = ui.add(big(if running { "Stop" } else { "Run" }));
        ui_state.record(format!("prun:{id}"), r.rect);
        if r.clicked() {
            ui_state.transport.push((
                id,
                if running {
                    Transport::Stop
                } else {
                    Transport::Run
                },
            ));
        }
        let r = ui.add(big("Restart"));
        ui_state.record(format!("prestart:{id}"), r.rect);
        if r.clicked() {
            ui_state.transport.push((id, Transport::Restart));
        }
    });
}

/// Right-click menu entries for a module: pin toggles and MIDI learn.
pub fn module_menu(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    id: ModuleId,
    info: &'static kabl_modules::ModuleInfo,
) {
    let r = ui.menu_button("Pin to Perform", |ui| {
        if info.kind == "clock" {
            let mut on = is_pinned(editor.state(), id, TRANSPORT);
            let r = ui.checkbox(&mut on, "Transport (Run/Stop, Restart)");
            ui_state.record(format!("menu:pin:{TRANSPORT}"), r.rect);
            if r.clicked() {
                toggle_pin(editor, id, TRANSPORT);
                ui_state.perform_open = true;
                ui_state.pin_reveal = Some((id, TRANSPORT.to_string()));
            }
        }
        for p in info.params {
            let mut on = is_pinned(editor.state(), id, p.name);
            let r = ui.checkbox(&mut on, routing::param_label(p));
            ui_state.record(format!("menu:pin:{}", p.name), r.rect);
            if r.clicked() {
                toggle_pin(editor, id, p.name);
                ui_state.perform_open = true;
                ui_state.pin_reveal = Some((id, p.name.to_string()));
            }
        }
    });
    ui_state.record("menu:pin".into(), r.response.rect);
    if info.params.iter().any(learnable) {
        let r = ui.menu_button("MIDI learn", |ui| {
            for p in info.params.iter().filter(|p| learnable(p)) {
                let text = match mapping(editor.state(), id, p.name) {
                    Some(m) => format!("{}  ({})", routing::param_label(p), cc_text(m)),
                    None => routing::param_label(p),
                };
                let r = ui.button(text);
                ui_state.record(format!("menu:learn:{}", p.name), r.rect);
                if r.clicked() {
                    ui_state.learn = Some((id, p.name.to_string()));
                    ui_state.perform_open = true;
                    ui_state.pin_reveal = Some((id, p.name.to_string()));
                    ui.close();
                }
            }
        });
        ui_state.record("menu:learn".into(), r.response.rect);
    }
}

/// Runtime MIDI pickup state, keyed by (module, param).
pub type TakeoverMap = HashMap<(ModuleId, String), Takeover>;

#[cfg(test)]
mod tests {
    use super::*;

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
