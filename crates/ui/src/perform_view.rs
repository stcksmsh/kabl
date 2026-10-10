//! The Perform view: pinned controls as section cards (transport, scenes, macros, sequences,
//! other controls), drawn through `Style` and the widget kit. What each pin does is in
//! `perform.rs`; this file only draws it and routes clicks back to the same functions.

use egui::{vec2, Layout, RichText, Sense};
use kabl_core::{ModuleId, ParamTarget, PortRef};
use kabl_engine::patch_engine::Command;
use kabl_modules::builtins::{seq, Transport};
use kabl_modules::{registry, ParamInfo, Taper};

use crate::kit::{self, Ic, PadState, Step, Tip, Tone};
use crate::perform::{
    self, button_mapping, button_text, cc_text, learnable, mapping, move_pin, pin_label, pin_param,
    pin_source, rename_pin, toggle_pin, unlearn, Pin, BANKS, BTN_PREFIX, CUE_PADS, TRANSPORT,
};
use crate::style::{Role, Style};
use crate::wheel::OwnedScroll;
use crate::{routing, PatchEditor, UiState};

const TRANSPORT_W: f32 = 290.0;
/// Narrowest a macro tile and a bank card get before the row wraps.
const TILE_MIN: f32 = 128.0;
const BANK_MIN: f32 = 230.0;
const CTRL_W: f32 = 250.0;
/// A bank card weighs this many macro tiles when the row shares out spare width.
const BANK_WEIGHT: f32 = 2.4;
/// Heights of what is not pad, knob or lane: card chrome of the scene row, the macro row, one
/// control row and the gaps between rows. The rest of the window height goes to pads and knobs.
const FIXED_H: f32 = 64.0 + 168.0 + 132.0 + 32.0;

/// Widths for `items` (min, weight) in a row of `avail`: the minimums, plus the spare room by
/// weight. When the minimums do not fit they stay as they are and the row wraps.
fn split(avail: f32, gap: f32, items: &[(f32, f32)]) -> Vec<f32> {
    let mins: f32 = items.iter().map(|i| i.0).sum();
    let spare = avail - mins - gap * items.len().saturating_sub(1) as f32;
    let total: f32 = items.iter().map(|i| i.1).sum();
    items
        .iter()
        .map(|&(min, w)| {
            if spare > 0.0 {
                min + spare * w / total
            } else {
                min
            }
        })
        .collect()
}

/// The view: header, then the pinned cards wrapped in rows.
pub fn panel(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui) {
    let st = ui_state.style.clone();
    if let Some(guide) = editor
        .state()
        .modules
        .keys()
        .find_map(|&id| editor.state().label(id, "guide"))
    {
        kit::paragraph(ui, &st, Role::Caption, Tone::Text2, guide);
    }
    header(editor, ui_state, ui, &st);
    kit::gap(ui, &st, 1);
    let all = perform::pins(editor.state());
    if all.is_empty() {
        kit::paragraph(
            ui,
            &st,
            Role::Body,
            Tone::Text2,
            "No pinned controls. Right-click a module and choose Pin to Perform, \
             or MIDI learn to map a hardware control.",
        );
        return;
    }
    let n = all.len();
    let macro_ids: Vec<ModuleId> = editor
        .state()
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "macro")
        .map(|(&id, _)| id)
        .collect();
    let is_macro = |p: &Pin| {
        !matches!(p.key.as_str(), TRANSPORT | BANKS | CUE_PADS) && macro_ids.contains(&p.id)
    };
    let macro_mods: Vec<ModuleId> = {
        let mut v: Vec<ModuleId> = Vec::new();
        for p in all.iter().filter(|p| is_macro(p)) {
            if !v.contains(&p.id) {
                v.push(p.id);
            }
        }
        v
    };
    let g = st.sp(2);
    // Half a pixel short, so a row that sums to the full width does not wrap on rounding.
    let avail = ui.available_width() - 14.0 - 0.5;
    let var = (ui.available_height() - FIXED_H).max(0.0);
    let pad_h = (var * 0.57).clamp(84.0, 140.0);
    let knob = (var * 0.43).clamp(64.0, 110.0);
    let row1_h = 64.0 + pad_h;
    let row2_h = knob + 168.0;

    let transports: Vec<(usize, &Pin)> = all
        .iter()
        .enumerate()
        .filter(|(_, p)| p.key == TRANSPORT)
        .collect();
    let cues: Vec<(usize, &Pin)> = all
        .iter()
        .enumerate()
        .filter(|(_, p)| p.key == CUE_PADS)
        .collect();
    let banks: Vec<(usize, &Pin)> = all
        .iter()
        .enumerate()
        .filter(|(_, p)| p.key == BANKS)
        .collect();
    let controls: Vec<(usize, &Pin)> = all
        .iter()
        .enumerate()
        .filter(|(_, p)| !matches!(p.key.as_str(), TRANSPORT | BANKS | CUE_PADS) && !is_macro(p))
        .collect();
    let tiles: Vec<Vec<(usize, &Pin)>> = macro_mods
        .iter()
        .map(|&id| {
            all.iter()
                .enumerate()
                .filter(|(_, p)| p.id == id && is_macro(p))
                .collect()
        })
        .collect();

    // Scene row: the transport at a fixed share, the scenes take the rest.
    let transport_w = if cues.is_empty() {
        TRANSPORT_W
    } else {
        (avail * 0.25).clamp(TRANSPORT_W - 10.0, 360.0)
    };
    let cues_w = ((avail - transports.len() as f32 * (transport_w + g)) / cues.len().max(1) as f32
        - g * cues.len().saturating_sub(1) as f32)
        .max(0.0);
    // Macro row: macros and sequences share the width.
    let mut items: Vec<(f32, f32)> = tiles
        .iter()
        .map(|t| {
            let n = t.len() as f32;
            (n * TILE_MIN + (n - 1.0) * st.sp(1) + st.sp(3) * 2.0, n)
        })
        .collect();
    items.extend(banks.iter().map(|_| (BANK_MIN, BANK_WEIGHT)));
    let widths = split(avail, g, &items);
    // Controls: whole columns of at least CTRL_W, all the same width.
    let cols = (((avail + g) / (CTRL_W + g)) as usize).max(1);
    let ctrl_w = (avail - g * (cols - 1) as f32) / cols as f32;

    egui::ScrollArea::vertical()
        .id_salt("perform-cards")
        .auto_shrink([false, false])
        .show_owned(ui, |ui| {
            ui.with_layout(
                Layout::left_to_right(egui::Align::Min).with_main_wrap(true),
                |ui| {
                    ui.spacing_mut().item_spacing = vec2(g, g);
                    for &(i, pin) in &transports {
                        transport_card(
                            editor,
                            ui_state,
                            ui,
                            &st,
                            pin,
                            i,
                            n,
                            vec2(transport_w, row1_h),
                        );
                    }
                    for &(i, pin) in &cues {
                        cues_card(
                            editor,
                            ui_state,
                            ui,
                            &st,
                            pin,
                            i,
                            n,
                            vec2(cues_w, row1_h),
                            pad_h,
                        );
                    }
                    for (mine, &w) in tiles.iter().zip(&widths) {
                        macros_card(editor, ui_state, ui, &st, mine, n, vec2(w, row2_h), knob);
                    }
                    for (k, (&(i, pin), &w)) in banks.iter().zip(&widths[tiles.len()..]).enumerate()
                    {
                        // One hue per sequencer, from the theme's signal colours.
                        let hue = [st.roles.audio, st.roles.cv, st.roles.gate][k % 3];
                        banks_card(editor, ui_state, ui, &st, pin, i, n, vec2(w, row2_h), hue);
                    }
                    for &(i, pin) in &controls {
                        control_card(editor, ui_state, ui, &st, pin, i, n, ctrl_w);
                    }
                },
            );
        });
}

fn header(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui, st: &Style) {
    ui.horizontal(|ui| {
        kit::label(ui, st, Role::Title, Tone::Text, "Perform");
        kit::gap(ui, st, 2);
        kit::label(ui, st, Role::Label, Tone::Text2, "MIDI in");
        let current = ui_state.midi_input.clone().unwrap_or_else(|| "none".into());
        let mut pick = None;
        let r = kit::dropdown(ui, st, "midi-input", &current, 190.0, |ui| {
            if kit::menu_item(ui, st, "none", ui_state.midi_input.is_none()).clicked() {
                pick = Some(None);
            }
            for name in &ui_state.midi_inputs {
                let on = ui_state.midi_input.as_ref() == Some(name);
                if kit::menu_item(ui, st, name, on).clicked() {
                    pick = Some(Some(name.clone()));
                }
            }
        });
        r.clone().on_hover_text(current);
        ui_state.record("midi-input".into(), r.rect);
        if let Some(p) = pick {
            ui_state.midi_select = Some(p);
        }
        if let Some(note) = ui_state.midi_note.clone() {
            kit::label(ui, st, Role::Caption, Tone::Warn, note);
        }
        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            let full = ui_state.perform_tall;
            let r = ui
                .add(kit::Button::new(
                    st,
                    if full { "Show rack" } else { "Full view" },
                ))
                .tip(
                    st,
                    if full {
                        "Dock Perform under the rack"
                    } else {
                        "Perform fills the window, rack hidden"
                    },
                );
            ui_state.record("perform-size".into(), r.rect);
            if r.clicked() {
                ui_state.perform_tall = !full;
            }
            let r = ui.add(kit::Button::new(st, "All notes off")).tip(
                st,
                "Release every MIDI voice (keyboard notes). Sequencers keep playing.",
            );
            ui_state.record("notes-off".into(), r.rect);
            if r.clicked() {
                ui_state.all_notes_off = true;
            }
            if let Some((id, param)) = ui_state.learn.clone() {
                let r = ui.add(kit::Button::new(st, "Cancel learn").primary());
                ui_state.record("learn-cancel".into(), r.rect);
                if r.clicked() {
                    ui_state.learn = None;
                }
                let button = param.strip_prefix(BTN_PREFIX).and_then(|a| {
                    perform::button_actions(editor.state(), id)
                        .into_iter()
                        .find(|(k, _)| k == a)
                        .map(|(_, l)| format!("{l} (press a button)"))
                });
                let label = button.unwrap_or_else(|| {
                    registry::info_for(
                        editor
                            .state()
                            .modules
                            .get(&id)
                            .map_or("", |m| m.kind.as_str()),
                    )
                    .and_then(|i| i.params.iter().find(|p| p.name == param))
                    .map_or(param.clone(), routing::target_label)
                });
                kit::label_truncated(
                    ui,
                    st,
                    Role::Label,
                    Tone::Accent,
                    format!(
                        "Learning {} #{id} {label}: move a control…",
                        perform::module_name(editor.state(), id)
                    ),
                );
            }
        });
    });
    ui.horizontal(|ui| crate::record::controls(ui_state, ui, st));
}

/// A card shell of a fixed size: runs `add` inside the card frame and records `pcard:<key>`.
fn shell(
    ui: &mut egui::Ui,
    ui_state: &mut UiState,
    st: &Style,
    key: &str,
    size: egui::Vec2,
    add: impl FnOnce(&mut egui::Ui, &mut UiState),
) -> egui::Rect {
    let r = ui.allocate_ui_with_layout(size, Layout::top_down(egui::Align::Min), |ui| {
        let f = kit::card_frame(st);
        let m = f.inner_margin.sum();
        f.show(ui, |ui| {
            ui.set_width(size.x - m.x);
            ui.set_min_height(size.y - m.y);
            ui.spacing_mut().item_spacing.y = st.sp(1);
            add(ui, ui_state);
        });
    });
    ui_state.record(format!("pcard:{key}"), r.response.rect);
    r.response.rect
}

/// Title row of a pin: its name (double-click renames), the explain button and the menu.
#[allow(clippy::too_many_arguments)]
fn pin_head(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    pin: &Pin,
    index: usize,
    count: usize,
    role: Role,
    section: bool,
    fallback: Option<String>,
) {
    let key = format!("{}.{}", pin.id, pin.key);
    let named = editor
        .state()
        .label(pin.id, &format!("{}{}", perform::PIN_PREFIX, pin.key))
        .is_some();
    let title = match fallback {
        Some(f) if !named => f,
        _ => pin_label(editor.state(), pin),
    };
    let source = pin_source(editor.state(), pin);
    let renaming = ui_state
        .renaming
        .as_ref()
        .is_some_and(|(id, k, _)| *id == pin.id && *k == pin.key);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = st.sp(1);
        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            let menu = kit::icon_menu(ui, st, Ic::More, |ui| {
                let r = kit::menu_item(ui, st, "Rename…", false);
                ui_state.record(format!("prename:{key}"), r.rect);
                if r.clicked() {
                    ui_state.renaming = Some((pin.id, pin.key.clone(), title.clone()));
                    ui.close();
                }
                let r = kit::menu_item_tone(ui, st, "Move left", false, Tone::Text, index > 0);
                ui_state.record(format!("pleft:{key}"), r.rect);
                if r.clicked() {
                    move_pin(editor, index, -1);
                    ui_state.pin_reveal = Some((pin.id, pin.key.clone()));
                    ui.close();
                }
                let r =
                    kit::menu_item_tone(ui, st, "Move right", false, Tone::Text, index + 1 < count);
                ui_state.record(format!("pright:{key}"), r.rect);
                if r.clicked() {
                    move_pin(editor, index, 1);
                    ui_state.pin_reveal = Some((pin.id, pin.key.clone()));
                    ui.close();
                }
                let r = kit::menu_item(ui, st, "Unpin", false);
                ui_state.record(format!("punpin:{key}"), r.rect);
                if r.clicked() {
                    toggle_pin(editor, pin.id, &pin.key);
                    ui.close();
                }
            })
            .tip(st, "Rename, move, unpin");
            ui_state.record(format!("pmenu:{key}"), menu.rect);
            let explaining = ui_state.explain.card.as_ref() == Some(&(pin.id, pin.key.clone()))
                && ui_state.explain.is_open();
            let r = ui
                .add(
                    kit::Button::new(st, "?")
                        .small()
                        .ghost()
                        .selected(explaining),
                )
                .tip(st, "Explain: what this control really changes, and where");
            ui_state.record(format!("pexplain:{key}"), r.rect);
            if r.clicked() {
                if explaining {
                    ui_state.explain.close();
                } else {
                    crate::explain::open(
                        ui_state,
                        crate::explain::Subject::Control {
                            id: pin.id,
                            key: pin.key.clone(),
                        },
                        Some((pin.id, pin.key.clone())),
                    );
                }
            }
            ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                if renaming {
                    let (_, _, text) = ui_state.renaming.as_mut().unwrap();
                    let r = kit::field(ui, st, text, "Name", None, Some(ui.available_width()));
                    ui_state.record(format!("plabel-edit:{key}"), r.rect);
                    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
                    if esc {
                        ui_state.renaming = None;
                    } else if enter || r.lost_focus() {
                        if let Some((id, k, text)) = ui_state.renaming.take() {
                            if text.trim() != title {
                                rename_pin(editor, id, &k, &text);
                            }
                        }
                    }
                    // Focus only while renaming: a field that is gone must not keep the
                    // keyboard (undo, Escape) after Enter or Escape.
                    if ui_state.renaming.is_some() {
                        r.request_focus();
                    } else {
                        r.surrender_focus();
                    }
                } else {
                    let text = if section {
                        let tr = &st.types.section;
                        RichText::new(if tr.caps {
                            title.to_uppercase()
                        } else {
                            title.clone()
                        })
                        .font(st.section_font())
                        .color(st.roles.text2)
                    } else {
                        kit::rich(st, role, Tone::Text, &title)
                    };
                    let r = ui
                        .add(
                            egui::Label::new(text)
                                .truncate()
                                .selectable(false)
                                .sense(Sense::click()),
                        )
                        .tip(st, &format!("{source}\nDouble-click to rename"));
                    ui_state.record(format!("plabel:{key}"), r.rect);
                    if r.double_clicked() {
                        ui_state.renaming = Some((pin.id, pin.key.clone(), title.clone()));
                    }
                }
            });
        });
    });
}

/// The ring and the reveal that follow every pin card: the explain origin outline and the
/// scroll to a freshly pinned card.
fn after_card(ui: &mut egui::Ui, ui_state: &mut UiState, st: &Style, pin: &Pin, rect: egui::Rect) {
    let origin = ui_state.explain.is_open()
        && ui_state.explain.card.as_ref() == Some(&(pin.id, pin.key.clone()));
    let now = ui.input(|i| i.time);
    let flash = ui_state
        .explain
        .card_flash
        .as_ref()
        .is_some_and(|(id, k, t)| (*id, k) == (pin.id, &pin.key) && now - t < 1.5);
    if origin || flash {
        let a = if flash && (now * 4.0) as i64 % 2 == 1 {
            0.4
        } else {
            1.0
        };
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            st.radius(crate::style::Rad::Md),
            egui::Stroke::new(2.5, st.roles.cv.gamma_multiply(a)),
            egui::StrokeKind::Inside,
        );
        if flash {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(120));
        }
    }
    if ui_state.pin_reveal.as_ref() == Some(&(pin.id, pin.key.clone())) {
        ui.scroll_to_rect(rect, None);
        ui_state.pin_reveal = None;
    }
}

/// True from the second frame of a press on the control: edits then extend one undo step.
fn continuing(ui: &egui::Ui, r: &egui::Response) -> bool {
    let id = r.id.with("gesture");
    let down = r.is_pointer_button_down_on();
    let was = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    ui.data_mut(|d| d.insert_temp(id, down));
    down && was
}

/// Bar number (from 1), beat in the bar (1..=4) and beats into the bar (0..4), from a clock
/// position in 16ths. A bar is 16 of them, as launch boundaries count.
fn bar_beat(pos: f64) -> (u64, u64, f32) {
    let in_bar = pos.rem_euclid(16.0);
    (
        (pos / 16.0).floor() as u64 + 1,
        in_bar as u64 / 4 + 1,
        in_bar as f32 / 4.0,
    )
}

fn running(ui_state: &UiState, id: ModuleId) -> bool {
    ui_state.clock_running.get(&id).copied().unwrap_or(true)
}

#[allow(clippy::too_many_arguments)]
fn transport_card(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    pin: &Pin,
    index: usize,
    count: usize,
    size: egui::Vec2,
) {
    let key = format!("{}.{}", pin.id, pin.key);
    let id = pin.id;
    let bpm = registry::info_for("clock")
        .and_then(|i| i.params.iter().find(|p| p.name == "bpm"))
        .map(|p| routing::base_value(editor.state(), id, p));
    let rect = shell(ui, ui_state, st, &key, size, |ui, ui_state| {
        pin_head(
            editor,
            ui_state,
            ui,
            st,
            pin,
            index,
            count,
            Role::H3,
            true,
            None,
        );
        let run = running(ui_state, id);
        let pos = ui_state.clock_pos.get(&id).copied();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = st.sp(1);
            let text = bpm.map_or("--".to_string(), |b| format!("{b:.0}"));
            // The tempo is read from across the room: a big step above the display role.
            let big = st.type_role(Role::Display).size * 1.7;
            ui.label(kit::rich(st, Role::Display, Tone::Text, text).size(big));
            ui.vertical(|ui| {
                kit::label(ui, st, Role::Label, Tone::Text2, "bpm");
                kit::label(
                    ui,
                    st,
                    Role::Caption,
                    if run { Tone::Good } else { Tone::Text3 },
                    if run { "running" } else { "stopped" },
                );
            });
            let room = vec2(ui.available_width(), 40.0);
            ui.allocate_ui_with_layout(room, Layout::top_down(egui::Align::Max), |ui| {
                ui.vertical(|ui| {
                    let (bar, beat) = pos.map_or(("--".into(), "--".into()), |p| {
                        let (b, beat, _) = bar_beat(p);
                        (b.to_string(), beat.to_string())
                    });
                    kit::label(ui, st, Role::Value, Tone::Text, format!("BAR {bar}"));
                    kit::label(ui, st, Role::Value, Tone::Text2, format!("BEAT {beat}"));
                });
            });
        });
        let w = ui.available_width();
        kit::bar_meter(ui, st, vec2(w, 6.0), pos.map_or(0.0, |p| bar_beat(p).2));
        if run {
            // The readout moves with the clock; the report arrives every audio callback.
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(33));
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = st.sp(2);
            let enabled = !ui_state.host_clock;
            let r = ui.add(
                kit::Button::new(st, if run { "Stop" } else { "Run" })
                    .icon(if run { Ic::Stop } else { Ic::Play })
                    .primary()
                    .min_width(92.0)
                    .enabled(enabled),
            );
            ui_state.record(format!("prun:{id}"), r.rect);
            if r.clicked() {
                ui_state
                    .transport
                    .push((id, if run { Transport::Stop } else { Transport::Run }));
            }
            let r = ui.add(
                kit::Button::new(st, "Restart")
                    .min_width(92.0)
                    .enabled(enabled),
            );
            ui_state.record(format!("prestart:{id}"), r.rect);
            if r.clicked() {
                ui_state.transport.push((id, Transport::Restart));
            }
        });
        if ui_state.host_clock {
            kit::label(
                ui,
                st,
                Role::Caption,
                Tone::Text2,
                "Host clock: transport controlled by REAPER",
            );
        }
    });
    after_card(ui, ui_state, st, pin, rect);
}

#[allow(clippy::too_many_arguments)]
fn cues_card(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    pin: &Pin,
    index: usize,
    count: usize,
    size: egui::Vec2,
    pad_h: f32,
) {
    let key = format!("{}.{}", pin.id, pin.key);
    let id = pin.id;
    let all = crate::cues::cues(editor.state(), id);
    // One row up to five cues (the usual set), else rows of four.
    let per_row = if all.len() <= 5 { all.len().max(4) } else { 4 };
    // The pads share the width left beside the Cancel button.
    let fixed = 84.0 + st.sp(2) + st.sp(3) * 2.0 + 4.0;
    let pad_w = ((size.x - fixed) / per_row as f32 - st.sp(1)).max(80.0);
    let rows = all.len().div_ceil(per_row).max(1);
    // Rows share the height the row was given.
    let pad_h = pad_h
        .min((size.y - 64.0) / rows as f32 - st.sp(1))
        .max(64.0);
    let rect = shell(ui, ui_state, st, &key, size, |ui, ui_state| {
        let who = "Scenes".to_string();
        pin_head(
            editor,
            ui_state,
            ui,
            st,
            pin,
            index,
            count,
            Role::H3,
            true,
            Some(who),
        );
        let state = editor.state().clone();
        let bank_of = |s: ModuleId| {
            ui_state
                .seq_banks
                .get(&s)
                .copied()
                .unwrap_or((crate::banks::startup(&state, s), None))
        };
        let standing: Vec<(bool, bool)> = all
            .iter()
            .map(|c| crate::cues::standing(c, &bank_of))
            .collect();
        let any_queued = standing.iter().any(|s| s.1);
        if all.is_empty() {
            kit::label(
                ui,
                st,
                Role::Caption,
                Tone::Text3,
                "No cues yet: add them in the drawer",
            );
        }
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = vec2(st.sp(1), st.sp(1));
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = vec2(st.sp(1), st.sp(1));
                for (row, flags) in all.chunks(per_row).zip(standing.chunks(per_row)) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = st.sp(1);
                        for (c, &(active, queued)) in row.iter().zip(flags) {
                            let tag =
                                button_mapping(&state, id, &format!("cue.{}", c.n)).map(cc_text);
                            let hover = format!(
                                "Launch {}{}",
                                c.name,
                                button_text(&state, id, &format!("cue.{}", c.n))
                                    .map_or(String::new(), |t| format!("\n{t}"))
                            );
                            let (ps, sub) = if active {
                                (PadState::Playing, Some("playing"))
                            } else if queued {
                                (PadState::Queued, Some("next bar"))
                            } else {
                                (PadState::Idle, None)
                            };
                            // A queued pad fills toward the bar line it launches on.
                            let progress = (queued && !active)
                                .then(|| c.clock.and_then(|k| ui_state.clock_pos.get(&k)))
                                .flatten()
                                .map(|&p| p.rem_euclid(16.0) as f32 / 16.0);
                            let r = kit::pad(
                                ui,
                                st,
                                vec2(pad_w, pad_h),
                                &c.name,
                                tag.as_deref(),
                                sub,
                                ps,
                                progress,
                            )
                            .tip(st, &hover);
                            ui_state.record(format!("pcue:{id}.{}", c.n), r.rect);
                            if r.clicked() {
                                if let Err(e) =
                                    crate::cues::launch(&mut ui_state.launches, &state, id, c.n)
                                {
                                    ui_state.last_message = Some(e);
                                }
                            }
                        }
                    });
                }
            });
            let r = ui
                .add(
                    kit::Button::new(st, "Cancel")
                        .min_width(84.0)
                        .enabled(any_queued),
                )
                .tip(st, "Drop the queued cue");
            if any_queued {
                ui_state.record(format!("pcue-cancel:{id}"), r.rect);
            }
            if r.clicked() {
                for c in all.iter().zip(&standing).filter(|(_, s)| s.1) {
                    for &(s, _) in &c.0.targets {
                        ui_state.launches.push(Command::Cancel(Some(s)));
                    }
                }
            }
        });
    });
    after_card(ui, ui_state, st, pin, rect);
}

/// What a macro moves: `Filter · Cutoff, Delay · Mix`, from the routes leaving its output.
fn macro_moves(state: &kabl_core::PatchState, id: ModuleId, port: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in state.cables.values() {
        let (PortRef::Module { id: from, port: p }, PortRef::Param { id: to, param }) =
            (&c.from, &c.to)
        else {
            continue;
        };
        if *from != id || p != port {
            continue;
        }
        let Some(m) = state.modules.get(to) else {
            continue;
        };
        let Some(info) = registry::info_for(&m.kind) else {
            continue;
        };
        let name = info
            .params
            .iter()
            .find(|q| q.name == param)
            .map_or(param.clone(), routing::target_label);
        let short = match routing::short_name(&m.kind) {
            "?" => info.name,
            s => s,
        };
        let label = format!("{short} · {name}");
        if !out.contains(&label) {
            out.push(label);
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn macros_card(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    pins: &[(usize, &Pin)],
    count: usize,
    size: egui::Vec2,
    knob: f32,
) {
    let tile_w =
        (size.x - st.sp(3) * 2.0 - st.sp(1) * (pins.len() as f32 - 1.0)) / pins.len() as f32;
    ui.allocate_ui_with_layout(size, Layout::top_down(egui::Align::Min), |ui| {
        let f = kit::card_frame(st);
        let m = f.inner_margin.sum();
        f.show(ui, |ui| {
            ui.set_width(size.x - m.x);
            ui.set_min_height(size.y - m.y);
            kit::section(ui, st, "Macros");
            kit::label(
                ui,
                st,
                Role::Caption,
                Tone::Text3,
                "turn to hear · hover a name to see what moves",
            );
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = st.sp(1);
                for &(i, pin) in pins {
                    macro_tile(editor, ui_state, ui, st, pin, i, count, tile_w, knob);
                }
            });
        });
    });
}

#[allow(clippy::too_many_arguments)]
fn macro_tile(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    pin: &Pin,
    index: usize,
    count: usize,
    tile_w: f32,
    knob: f32,
) {
    let key = format!("{}.{}", pin.id, pin.key);
    let Some(p) = pin_param(editor.state(), pin) else {
        return;
    };
    let rect = ui
        .allocate_ui_with_layout(
            vec2(tile_w, knob + 130.0),
            Layout::top_down(egui::Align::Center),
            |ui| {
                ui.set_width(tile_w);
                ui.spacing_mut().item_spacing.y = st.sp(1);
                pin_head(
                    editor,
                    ui_state,
                    ui,
                    st,
                    pin,
                    index,
                    count,
                    Role::H3,
                    false,
                    None,
                );
                let cur = routing::base_value(editor.state(), pin.id, p);
                let (resp, new) = kit::arc_knob(ui, st, p.to_norm(cur), knob, None);
                ui_state.record(format!("pslider:{key}"), resp.rect);
                let continuing = continuing(ui, &resp);
                if let Some(v) = new {
                    let target = ParamTarget::Module {
                        id: pin.id,
                        param: p.name.into(),
                    };
                    editor.set_param_gesture(target, p.from_norm(v), !continuing);
                }
                kit::label(ui, st, Role::Value, Tone::Text, routing::fmt_value(p, cur));
                let moves = macro_moves(editor.state(), pin.id, p.name);
                let text = match moves.len() {
                    0 => "moves nothing yet".to_string(),
                    1..=3 => moves.join(" · "),
                    n => format!("{} · +{}", moves[..2].join(" · "), n - 2),
                };
                kit::label_truncated(
                    ui,
                    st,
                    Role::Caption,
                    if moves.is_empty() {
                        Tone::Text3
                    } else {
                        Tone::Text2
                    },
                    text,
                )
                .tip(
                    st,
                    &if moves.is_empty() {
                        "Route this macro's output to controls in the drawer".to_string()
                    } else {
                        moves.join("\n")
                    },
                );
                midi_row(editor, ui_state, ui, st, pin.id, p);
            },
        )
        .response
        .rect;
    ui_state.record(format!("pcard:{key}"), rect);
    after_card(ui, ui_state, st, pin, rect);
}

/// CC assignment: a tag that learns on click, Clear, and the pickup state.
fn midi_row(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    id: ModuleId,
    p: &'static ParamInfo,
) {
    if !learnable(p) {
        return;
    }
    let key = format!("{id}.{}", p.name);
    let map = mapping(editor.state(), id, p.name);
    let learning = ui_state.learn.as_ref() == Some(&(id, p.name.to_string()));
    // Wraps in a narrow tile instead of widening it.
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = st.sp(1);
        let text = match (learning, map) {
            (true, _) => "Learning…".to_string(),
            (false, Some((ch, cc))) => format!("CC {cc} · {}", ch + 1),
            (false, None) => "Learn".to_string(),
        };
        let r = ui
            .add(
                kit::Button::new(st, text)
                    .small()
                    .selected(learning || map.is_some()),
            )
            .tip(
                st,
                &match map {
                    Some(m) => format!("{}: click to learn another control", cc_text(m)),
                    None => "Click, then move a hardware control".into(),
                },
            );
        ui_state.record(format!("plearn:{key}"), r.rect);
        if r.clicked() {
            ui_state.learn = (!learning).then(|| (id, p.name.to_string()));
        }
        if map.is_some() {
            let r = ui
                .add(kit::Button::new(st, "Clear").small().ghost())
                .tip(st, "Remove the MIDI mapping");
            ui_state.record(format!("pclear:{key}"), r.rect);
            if r.clicked() {
                unlearn(editor, id, p.name);
            }
            let t = ui_state
                .takeover
                .get(&(id, p.name.to_string()))
                .copied()
                .unwrap_or_default();
            let at = p.to_norm(routing::base_value(editor.state(), id, p));
            let (text, tone, tip) = if t.picked {
                (
                    "live".to_string(),
                    Tone::Good,
                    "The hardware controls it".to_string(),
                )
            } else {
                let arrow = match t.hw {
                    Some(h) if h < at => "↑",
                    Some(_) => "↓",
                    None => "→",
                };
                (
                    format!("{arrow} {:.0}%", at * 100.0),
                    Tone::Warn,
                    format!(
                        "Pickup: move the hardware to {:.0} % to take over (it won't jump)",
                        at * 100.0
                    ),
                )
            };
            let r = kit::label(ui, st, Role::Caption, tone, text).tip(st, &tip);
            ui_state.record(format!("ppickup:{key}"), r.rect);
        }
    });
}

fn param_value(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    id: ModuleId,
    p: &'static ParamInfo,
    hit: String,
) {
    let Some(kind) = editor.state().modules.get(&id).map(|m| m.kind.clone()) else {
        return;
    };
    let cur = routing::base_value(editor.state(), id, p);
    if p.taper == Taper::Stepped {
        let labels = routing::step_labels(&kind, p.name);
        let n = (p.max - p.min).round() as usize + 1;
        let mut whole: Option<egui::Rect> = None;
        kit::segmented(ui, st, |ui| {
            for k in 0..n {
                let opt = p.min + k as f32;
                let text = labels
                    .and_then(|l| l.get(k).copied())
                    .map_or(format!("{opt}"), str::to_string);
                let r = kit::seg(ui, st, &text, cur.round() == opt);
                ui_state.record(format!("{hit}.{k}"), r.rect);
                whole = Some(whole.map_or(r.rect, |w| w.union(r.rect)));
                if r.clicked() && cur.round() != opt {
                    editor.set_param(id, p.name, opt);
                }
            }
        });
        if let Some(w) = whole {
            ui_state.record(hit, w);
        }
        return;
    }
    let mut v = f64::from(p.to_norm(cur));
    let readout = routing::fmt_value(p, cur);
    let r = kit::slider(ui, st, &mut v, 0.0..=1.0, None, &readout);
    ui_state.record(hit, r.rect);
    let continuing = continuing(ui, &r);
    if r.changed() {
        let target = ParamTarget::Module {
            id,
            param: p.name.into(),
        };
        editor.set_param_gesture(target, p.from_norm(v as f32), !continuing);
    }
}

#[allow(clippy::too_many_arguments)]
fn control_card(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    pin: &Pin,
    index: usize,
    count: usize,
    w: f32,
) {
    let key = format!("{}.{}", pin.id, pin.key);
    let Some(p) = pin_param(editor.state(), pin) else {
        return;
    };
    let rect = shell(ui, ui_state, st, &key, vec2(w, 132.0), |ui, ui_state| {
        pin_head(
            editor,
            ui_state,
            ui,
            st,
            pin,
            index,
            count,
            Role::H3,
            true,
            None,
        );
        kit::label_truncated(
            ui,
            st,
            Role::Caption,
            Tone::Text3,
            pin_source(editor.state(), pin),
        );
        param_value(
            editor,
            ui_state,
            ui,
            st,
            pin.id,
            p,
            format!("pslider:{key}"),
        );
        midi_row(editor, ui_state, ui, st, pin.id, p);
    });
    after_card(ui, ui_state, st, pin, rect);
}

/// The bank step lane data: level, probability and gate of the playing bank's eight steps.
fn lane(state: &kabl_core::PatchState, id: ModuleId, bank: usize) -> Vec<Step> {
    let Some(info) = registry::info_for("seq") else {
        return Vec::new();
    };
    let val = |name: String| {
        info.params
            .iter()
            .find(|p| p.name == name)
            .map(|p| (p.to_norm(routing::base_value(state, id, p)), p.max))
    };
    let prefix = ["", "b.", "c.", "d."][bank.min(3)];
    (1..=seq::STEPS)
        .map(|k| Step {
            level: val(format!("{prefix}v{k}")).map_or(1.0, |v| v.0),
            prob: val(format!("{prefix}r{k}")).map_or(1.0, |v| v.0),
            gate: val(format!("{prefix}g{k}")).is_none_or(|v| v.0 >= 0.5),
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn banks_card(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    pin: &Pin,
    index: usize,
    count: usize,
    size: egui::Vec2,
    hue: egui::Color32,
) {
    let key = format!("{}.{}", pin.id, pin.key);
    let id = pin.id;
    let rect = shell(ui, ui_state, st, &key, size, |ui, ui_state| {
        let who = format!("Banks · {} #{id}", perform::module_name(editor.state(), id));
        pin_head(
            editor,
            ui_state,
            ui,
            st,
            pin,
            index,
            count,
            Role::H3,
            true,
            Some(who),
        );
        let state = editor.state().clone();
        let (playing, queued) = ui_state
            .seq_banks
            .get(&id)
            .copied()
            .unwrap_or((crate::banks::startup(&state, id), None));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = st.sp(1);
            for b in 0..seq::BANKS {
                let name = crate::banks::title(&state, id, b);
                let hover = format!(
                    "Launch bank {name}{}",
                    button_text(&state, id, &format!("bank.{b}"))
                        .map_or(String::new(), |t| format!("\n{t}"))
                );
                let ps = if b == playing {
                    PadState::Playing
                } else if queued == Some(b) {
                    PadState::Queued
                } else {
                    PadState::Idle
                };
                let r = kit::pad(
                    ui,
                    st,
                    vec2(44.0, 38.0),
                    seq::BANK_NAMES[b],
                    None,
                    None,
                    ps,
                    None,
                )
                .tip(st, &hover);
                ui_state.record(format!("pbank:{id}.{b}"), r.rect);
                if r.clicked() {
                    if let Err(e) = crate::banks::launch(&mut ui_state.launches, &state, id, b) {
                        ui_state.last_message = Some(e);
                    }
                }
            }
            if queued.is_some() {
                let r = ui.add(kit::Button::new(st, "Cancel").small());
                ui_state.record(format!("pbank-cancel:{id}"), r.rect);
                if r.clicked() {
                    ui_state.launches.push(Command::Cancel(Some(id)));
                }
            }
        });
        let text = match queued {
            Some(q) => format!("next: {}", crate::banks::title(&state, id, q)),
            None => format!("playing: {}", crate::banks::title(&state, id, playing)),
        };
        kit::label_truncated(ui, st, Role::Caption, Tone::Text2, text);
        let steps = lane(&state, id, playing);
        let w = ui.available_width();
        kit::step_lane(
            ui,
            st,
            vec2(w, (size.y - 150.0).max(64.0)),
            &steps,
            ui_state.seq_steps.get(&id).copied(),
            hue,
        );
        kit::label(
            ui,
            st,
            Role::Caption,
            Tone::Text3,
            "bar height = level · brightness = probability",
        );
    });
    after_card(ui, ui_state, st, pin, rect);
}
