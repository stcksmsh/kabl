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
/// A macro tile's width when its card is not stretched.
const TILE_W: f32 = 156.0;
const BANK_MIN: f32 = 230.0;
const CTRL_W: f32 = 250.0;
/// A bank card weighs this many macro tiles when the row shares out spare width.
const BANK_WEIGHT: f32 = 2.4;
/// Heights of what is not pad, knob or lane: card chrome of the scene row, the macro row, one
/// control row and the gaps between rows. The rest of the window height goes to pads and knobs.
const FIXED_H: f32 = 64.0 + 168.0 + 132.0 + 32.0;
/// The two-column Perform scene (transport over scenes, macros over sequences) needs this much
/// height; the docked strip below the rack uses the flowing rows instead.
const COLUMNS_MIN_H: f32 = 520.0;
/// Height of the transport card; the scenes take the rest of the left column.
const TRANSPORT_H: f32 = 220.0;
/// Most other controls the two-column scene carries (one row below it).
const COLUMNS_MAX_CONTROLS: usize = 4;
/// Knob diameter limits in the macro card, and what the card spends on everything but the knob.
const KNOB_MIN: f32 = 64.0;
const KNOB_MAX: f32 = 138.0;
const MACRO_CHROME_H: f32 = 160.0;

/// Widths for `items` (min, weight) in a row of `avail`: the minimums, plus the spare room by
/// weight. When the minimums do not fit they stay as they are and the row wraps.
fn split(avail: f32, gap: f32, items: &[(f32, f32)]) -> Vec<f32> {
    let mins: f32 = items.iter().map(|i| i.0).sum();
    let spare = avail - mins - gap * items.len().saturating_sub(1) as f32;
    let total: f32 = items.iter().map(|i| i.1).sum::<f32>().max(f32::EPSILON);
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
    let all = perform::pins(editor.state());
    header(
        editor,
        ui_state,
        ui,
        &st,
        all.iter().any(|p| p.key == TRANSPORT),
    );
    kit::gap(ui, &st, 1);
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
    // A little under, so the last row clears the status bar and a scrollbar gutter.
    let var = (ui.available_height() - FIXED_H - 16.0).max(0.0);
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
    // With no scenes the transport joins this row at its own width.
    // Macros that have no sequences beside them keep their own width, and the controls flow
    // next to them, which is what the docked strip needs.
    let spread = !banks.is_empty();
    let lead = if cues.is_empty() && spread {
        transports.len()
    } else {
        0
    };
    let mut items: Vec<(f32, f32)> = vec![(TRANSPORT_W, 0.0); lead];
    items.extend(tiles.iter().map(|t| {
        let n = t.len() as f32;
        let tile = if spread { TILE_MIN } else { TILE_W };
        (
            // Five or more macros share one row: the tiles narrow before the row wraps.
            (n * tile + (n - 1.0) * st.sp(1) + st.sp(4) * 2.0).min(avail),
            if spread { n } else { 0.0 },
        )
    }));
    items.extend(banks.first().map(|_| (BANK_MIN, BANK_WEIGHT)));
    let widths = split(avail, g, &items);
    // Controls: whole columns of at least CTRL_W, all the same width.
    let cols = (((avail + g) / (CTRL_W + g)) as usize).clamp(1, controls.len().max(1));
    // A transport beside bare controls keeps the plain card width so they flow together.
    let alone = (cues.is_empty() && lead == 0 && !transports.is_empty()) || !spread;
    let ctrl_w = if alone {
        CTRL_W
    } else {
        ((avail - g * (cols - 1) as f32) / cols as f32).min(CTRL_W * 1.6)
    };

    let columns = ui.available_height() >= COLUMNS_MIN_H
        && (!transports.is_empty() || !cues.is_empty())
        && (!tiles.is_empty() || !banks.is_empty())
        // More sequencers or other controls than the columns carry keep the flowing rows, which
        // fit them in view.
        && controls.len() <= COLUMNS_MAX_CONTROLS;
    // With other controls below, the columns keep the height the flowing rows had, so those
    // controls stay above the fold; alone they take the whole view.
    let body_h = if controls.is_empty() {
        (ui.available_height() - 16.0).max(COLUMNS_MIN_H - 16.0)
    } else {
        (row1_h + g + row2_h).max(COLUMNS_MIN_H * 0.8)
    };
    egui::ScrollArea::vertical()
        .id_salt("perform-cards")
        .auto_shrink([false, false])
        .show_owned(ui, |ui| {
            if columns {
                // Mockup sizes: 380 px left column at full width, 300 compact.
                let compact = avail < 1180.0;
                let left_w = if compact { 300.0 } else { 380.0 };
                let transport_h = if compact { 200.0 } else { TRANSPORT_H };
                let right_w = avail - left_w - g;
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing = vec2(g, g);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing = vec2(g, g);
                        let cues_h = if cues.is_empty() {
                            0.0
                        } else {
                            body_h - transport_h - g
                        };
                        for &(i, pin) in &transports {
                            let h = if cues.is_empty() { body_h } else { transport_h };
                            transport_card(editor, ui_state, ui, &st, pin, i, n, vec2(left_w, h));
                        }
                        let pad_h = ((cues_h - 64.0) / 4.0 - st.sp(1)).clamp(64.0, 140.0);
                        for &(i, pin) in &cues {
                            cues_card(
                                editor,
                                ui_state,
                                ui,
                                &st,
                                pin,
                                i,
                                n,
                                vec2(left_w, cues_h),
                                pad_h,
                            );
                        }
                    });
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing = vec2(g, g);
                        // Mockup: macros 390 px tall (262 compact), sequences take the rest.
                        let macros_h = if banks.is_empty() {
                            body_h
                        } else if tiles.is_empty() {
                            0.0
                        } else {
                            (if compact { 262.0_f32 } else { 390.0_f32 }).min((body_h - g) * 0.7)
                        };
                        let knob = (macros_h - MACRO_CHROME_H).clamp(KNOB_MIN, KNOB_MAX);
                        let tw = split(right_w, g, &vec![(TILE_MIN, 1.0); tiles.len()]);
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = g;
                            for (mine, &w) in tiles.iter().zip(&tw) {
                                macros_card(
                                    editor,
                                    ui_state,
                                    ui,
                                    &st,
                                    mine,
                                    n,
                                    vec2(w, macros_h),
                                    knob,
                                );
                            }
                        });
                        let rest = if tiles.is_empty() {
                            body_h
                        } else {
                            body_h - macros_h - g
                        };
                        if !banks.is_empty() {
                            let rows: Vec<(usize, &Pin)> = banks.clone();
                            sequences_card(
                                editor,
                                ui_state,
                                ui,
                                &st,
                                &rows,
                                n,
                                vec2(right_w, rest.max(120.0)),
                            );
                        }
                    });
                });
                if !controls.is_empty() {
                    ui.with_layout(
                        Layout::left_to_right(egui::Align::Min).with_main_wrap(true),
                        |ui| {
                            ui.spacing_mut().item_spacing = vec2(g, g);
                            for &(i, pin) in &controls {
                                control_card(editor, ui_state, ui, &st, pin, i, n, ctrl_w);
                            }
                        },
                    );
                }
                return;
            }
            ui.with_layout(
                Layout::left_to_right(egui::Align::Min).with_main_wrap(true),
                |ui| {
                    ui.spacing_mut().item_spacing = vec2(g, g);
                    for (k, &(i, pin)) in transports.iter().enumerate() {
                        let size = if lead > 0 {
                            vec2(widths[k], row2_h)
                        } else {
                            vec2(transport_w, row1_h)
                        };
                        transport_card(editor, ui_state, ui, &st, pin, i, n, size);
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
                    for (mine, &w) in tiles.iter().zip(&widths[lead..]) {
                        macros_card(editor, ui_state, ui, &st, mine, n, vec2(w, row2_h), knob);
                    }
                    if let Some(&w) = widths.get(lead + tiles.len()) {
                        let rows: Vec<(usize, &Pin)> = banks.clone();
                        sequences_card(editor, ui_state, ui, &st, &rows, n, vec2(w, row2_h));
                    }
                    for &(i, pin) in &controls {
                        control_card(editor, ui_state, ui, &st, pin, i, n, ctrl_w);
                    }
                },
            );
        });
}

/// While a control is learning: Cancel and what is being learnt.
fn learn_row(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui, st: &Style) {
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
}

/// The Perform view's own controls, in the transport card's menu: MIDI input, All notes off,
/// Keep Routing open, and the view size.
fn perform_menu(ui_state: &mut UiState, ui: &mut egui::Ui, st: &Style) {
    kit::section(ui, st, "MIDI in");
    if kit::menu_item(ui, st, "None", ui_state.midi_input.is_none()).clicked() {
        ui_state.midi_select = Some(None);
    }
    for name in ui_state.midi_inputs.clone() {
        let on = ui_state.midi_input.as_ref() == Some(&name);
        let r = kit::menu_item(ui, st, &name, on);
        ui_state.record(format!("midi-input:{name}"), r.rect);
        if r.clicked() {
            ui_state.midi_select = Some(Some(name));
        }
    }
    if let Some(note) = ui_state.midi_note.clone() {
        kit::label(ui, st, Role::Caption, Tone::Warn, note);
    }
    kit::rule(ui, st);
    if ui_state.recorder.is_none() {
        kit::label(
            ui,
            st,
            Role::Caption,
            Tone::Text3,
            "Recording needs an audio device.",
        );
    }
    let r = kit::menu_item(ui, st, "All notes off", false).tip(
        st,
        "Release every MIDI voice (keyboard notes). Sequencers keep playing.",
    );
    ui_state.record("notes-off".into(), r.rect);
    if r.clicked() {
        ui_state.all_notes_off = true;
        ui.close();
    }
    let pinned = ui_state.drawer_pinned;
    let r = kit::menu_item(ui, st, "Keep Routing open", pinned).tip(
        st,
        "The Routing drawer is closed on entering Perform to give it the room; pin it to keep it open",
    );
    ui_state.record("drawer-pin".into(), r.rect);
    if r.clicked() {
        ui_state.drawer_pinned = !pinned;
        ui_state.drawer_open |= !pinned;
        ui_state.drawer_auto_closed = false;
    }
    let full = ui_state.perform_tall;
    let r = kit::menu_item(ui, st, if full { "Show rack" } else { "Full view" }, false);
    ui_state.record("perform-size".into(), r.rect);
    if r.clicked() {
        ui_state.perform_tall = !full;
        ui.close();
    }
}

/// The view's top: nothing when the transport card carries the controls, except while a control
/// learns and when a recorder exists. Without a transport card the full header stays.
fn header(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    has_transport: bool,
) {
    if !has_transport {
        full_header(editor, ui_state, ui, st);
        return;
    }
    if ui_state.learn.is_some() {
        ui.horizontal(|ui| learn_row(editor, ui_state, ui, st));
    }
    if ui_state.recorder.is_some() {
        ui.horizontal(|ui| crate::record::controls(ui_state, ui, st));
    }
}

fn full_header(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui, st: &Style) {
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
            let pinned = ui_state.drawer_pinned;
            let r = ui
                .add(
                    kit::Button::new(st, "Keep Routing open")
                        .icon(Ic::Pin)
                        .selected(pinned),
                )
                .tip(
                    st,
                    "The Routing drawer is closed on entering Perform to give it the room; \
                     pin it to keep it open",
                );
            ui_state.record("drawer-pin".into(), r.rect);
            if r.clicked() {
                ui_state.drawer_pinned = !pinned;
                ui_state.drawer_open |= !pinned;
                ui_state.drawer_auto_closed = false;
            }
            let r = ui.add(kit::Button::new(st, "All notes off")).tip(
                st,
                "Release every MIDI voice (keyboard notes). Sequencers keep playing.",
            );
            ui_state.record("notes-off".into(), r.rect);
            if r.clicked() {
                ui_state.all_notes_off = true;
            }
            learn_row(editor, ui_state, ui, st);
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
        // The mockup's Perform cards (`card()` in directions.html) pad 16 at the sides; the
        // card's own 12 above and below keeps the title on the mockup's line.
        let f = kit::card_frame(st)
            .inner_margin(egui::Margin::symmetric(st.sp(4) as i8, st.sp(3) as i8));
        let m = f.inner_margin.sum();
        f.show(ui, |ui| {
            ui.set_width(size.x - m.x);
            ui.set_min_height(size.y - m.y);
            ui.spacing_mut().item_spacing.y = st.sp(1);
            add(ui, ui_state);
        });
    });
    note_hover(ui, key, r.response.rect);
    ui_state.record(format!("pcard:{key}"), r.response.rect);
    r.response.rect
}

/// How visible a card's tools (explain, menu) are: 0 until the pointer is over the card, then 1.
/// The tools stay in the layout either way, so they keep their hit targets.
fn tool_fade(ui: &egui::Ui, key: &str) -> f32 {
    let id = egui::Id::new(("pcard-hover", key));
    let on = ui.ctx().data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    ui.ctx().animate_bool_with_time(id.with("fade"), on, 0.12)
}

/// Records whether the pointer is over the card `rect`, for next frame.
fn note_hover(ui: &egui::Ui, key: &str, rect: egui::Rect) {
    let over = ui
        .input(|i| i.pointer.hover_pos())
        .is_some_and(|p| rect.contains(p));
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new(("pcard-hover", key)), over));
}

/// Extra menu items a card adds to its "…" menu.
type Extra<'a> = Option<&'a mut dyn FnMut(&mut egui::Ui, &mut PatchEditor, &mut UiState)>;

/// The items of a pin's menu: rename, move, unpin.
#[allow(clippy::too_many_arguments)]
fn pin_menu(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    pin: &Pin,
    index: usize,
    count: usize,
    title: &str,
) {
    let key = format!("{}.{}", pin.id, pin.key);
    let r = kit::menu_item(ui, st, "Rename…", false);
    ui_state.record(format!("prename:{key}"), r.rect);
    if r.clicked() {
        ui_state.renaming = Some((pin.id, pin.key.clone(), title.to_string()));
        ui.close();
    }
    let r = kit::menu_item_tone(ui, st, "Move left", false, Tone::Text, index > 0);
    ui_state.record(format!("pleft:{key}"), r.rect);
    if r.clicked() {
        move_pin(editor, index, -1);
        ui_state.pin_reveal = Some((pin.id, pin.key.clone()));
        ui.close();
    }
    let r = kit::menu_item_tone(ui, st, "Move right", false, Tone::Text, index + 1 < count);
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
}

/// The explain button and the "…" menu of a pin, right to left. Called inside a right-to-left
/// layout; faded by `fade`.
#[allow(clippy::too_many_arguments)]
fn pin_tools(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    pin: &Pin,
    index: usize,
    count: usize,
    title: &str,
    fade: f32,
    extra: Extra,
) {
    let key = format!("{}.{}", pin.id, pin.key);
    ui.scope(|ui| {
        ui.set_opacity(fade.max(0.001));
        ui.spacing_mut().item_spacing.x = st.sp(1);
        let menu = kit::icon_menu(ui, st, Ic::More, |ui| {
            pin_menu(editor, ui_state, ui, st, pin, index, count, title);
            if let Some(extra) = extra {
                kit::rule(ui, st);
                extra(ui, editor, ui_state);
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
    });
}

/// A pin's name: a label (double-click renames) or, while renaming, a text field. `font`
/// overrides the role's font.
#[allow(clippy::too_many_arguments)]
fn pin_title(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    pin: &Pin,
    title: &str,
    role: Role,
    section: bool,
    font: Option<egui::FontId>,
) {
    let key = format!("{}.{}", pin.id, pin.key);
    let source = pin_source(editor.state(), pin);
    let renaming = ui_state
        .renaming
        .as_ref()
        .is_some_and(|(id, k, _)| *id == pin.id && *k == pin.key);
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
        // Focus only while renaming: a field that is gone must not keep the keyboard (undo,
        // Escape) after Enter or Escape.
        if ui_state.renaming.is_some() {
            r.request_focus();
        } else {
            r.surrender_focus();
        }
        return;
    }
    let text = if section {
        let tr = &st.types.section;
        RichText::new(if tr.caps {
            title.to_uppercase()
        } else {
            title.to_string()
        })
        .font(st.section_font())
        .color(st.roles.text2)
        .extra_letter_spacing(tr.tracking)
    } else {
        let t = kit::rich(st, role, Tone::Text, title);
        match font {
            Some(f) => t.font(f),
            None => t,
        }
    };
    let r = ui
        .add(
            egui::Label::new(text)
                .truncate()
                .selectable(false)
                .sense(Sense::click()),
        )
        .tip(st, &format!("{title}\n{source}\nDouble-click to rename"));
    ui_state.record(format!("plabel:{key}"), r.rect);
    if r.double_clicked() {
        ui_state.renaming = Some((pin.id, pin.key.clone(), title.to_string()));
    }
}

/// Title row of a card: the kicker (its name in caps) at the left, a muted note at the right
/// that gives way to the explain button and the menu while the pointer is over the card.
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
    note: &str,
    extra: Extra,
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
    let fade = tool_fade(ui, &key);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = st.sp(1);
        let row = ui.max_rect();
        if !note.is_empty() && fade < 0.99 {
            ui.painter().text(
                egui::pos2(row.right(), row.center().y),
                egui::Align2::RIGHT_CENTER,
                note,
                st.font(Role::Caption),
                crate::style::alpha(st.roles.text3, (255.0 * (1.0 - fade)) as u8),
            );
        }
        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            pin_tools(
                editor, ui_state, ui, st, pin, index, count, &title, fade, extra,
            );
            ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                pin_title(editor, ui_state, ui, st, pin, &title, role, section, None);
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
    let compact = size.x < 340.0;
    let rect = shell(ui, ui_state, st, &key, size, |ui, ui_state| {
        let note = if ui_state.host_clock {
            "Host sync"
        } else {
            "Internal clock"
        };
        let mut extra = |ui: &mut egui::Ui, _: &mut PatchEditor, ui_state: &mut UiState| {
            perform_menu(ui_state, ui, st);
        };
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
            note,
            Some(&mut extra),
        );
        let run = running(ui_state, id);
        let pos = ui_state.clock_pos.get(&id).copied();
        // The tempo is read from across the room: 68 px (54 compact), as in the mockup.
        let big = if compact { 54.0 } else { 68.0 };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = st.sp(1);
            let text = bpm.map_or("--".to_string(), |b| format!("{b:.1}"));
            ui.label(kit::rich(st, Role::Display, Tone::Text, text).size(big));
            ui.vertical(|ui| {
                ui.add_space(big * 0.55);
                kit::label(ui, st, Role::Caption, Tone::Text3, "BPM");
            });
        });
        // Bar . beat, and the four beats of the bar.
        let (bar, beat, _) = pos.map_or((0, 0, 0.0), bar_beat);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = st.sp(2);
            let text = if pos.is_some() {
                format!("{bar} . {beat}")
            } else {
                "-- . --".to_string()
            };
            let size = if compact { 20.0 } else { 24.0 };
            ui.label(kit::rich(st, Role::Value, Tone::Text, text).size(size));
            ui.add_space(st.sp(1));
            for i in 0..4u64 {
                let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                let on = pos.is_some() && beat == i + 1;
                ui.painter()
                    .rect_filled(r, 3.0, if on { st.roles.accent } else { st.roles.inset });
            }
        });
        if run {
            // The readout moves with the clock; the report arrives every audio callback.
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(33));
        }
        // The buttons sit at the foot of the card.
        let d = st.metrics.large_h;
        ui.add_space((ui.available_height() - d - st.sp(1)).max(st.sp(2)));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = st.sp(2);
            let enabled = !ui_state.host_clock;
            // Play (accent), Stop and Restart, as in the mockup's transport card.
            let r = kit::round_button(ui, st, Ic::Play, true, d, enabled).tip(
                st,
                if run {
                    "The clock is running"
                } else {
                    "Run the clock"
                },
            );
            ui_state.record(format!("prun:{id}"), r.rect);
            if r.clicked() && !run {
                ui_state.transport.push((id, Transport::Run));
            }
            let r = kit::round_button(ui, st, Ic::Stop, false, d - 4.0, enabled)
                .tip(st, "Stop the clock");
            ui_state.record(format!("pstop:{id}"), r.rect);
            if r.clicked() && run {
                ui_state.transport.push((id, Transport::Stop));
            }
            let r = kit::round_button(ui, st, Ic::Restart, false, d - 4.0, enabled)
                .tip(st, "Restart from the first bar");
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
    _pad_h: f32,
) {
    let key = format!("{}.{}", pin.id, pin.key);
    let id = pin.id;
    let all = crate::cues::cues(editor.state(), id);
    // Pads two across, as many rows as the scenes need; they share the card's height.
    let per_row = 2;
    let gap = 10.0;
    let pad_w = ((size.x - st.sp(4) * 2.0 - 4.0 - gap) / per_row as f32).max(60.0);
    let rows = all.len().div_ceil(per_row).max(1);
    let pad_h = ((size.y - 64.0 - gap * (rows as f32 - 1.0)) / rows as f32).clamp(44.0, 150.0);
    let state = editor.state().clone();
    let standing: Vec<(bool, bool)> = {
        let bank_of = |s: ModuleId| {
            ui_state
                .seq_banks
                .get(&s)
                .copied()
                .unwrap_or((crate::banks::startup(&state, s), None))
        };
        all.iter()
            .map(|c| crate::cues::standing_now(c, &bank_of, ui_state.last_cue.get(&id).copied()))
            .collect()
    };
    let current = all.iter().zip(&standing).find(|(_, s)| s.0).map(|(c, _)| c);
    let any_queued = standing.iter().any(|s| s.1);
    let rect = shell(ui, ui_state, st, &key, size, |ui, ui_state| {
        // Cancel and Store macros live in the card's menu.
        let mut extra = |ui: &mut egui::Ui, editor: &mut PatchEditor, ui_state: &mut UiState| {
            let r = kit::menu_item_tone(
                ui,
                st,
                "Cancel the queued scene",
                false,
                Tone::Text,
                any_queued,
            );
            if any_queued {
                ui_state.record(format!("pcue-cancel:{id}"), r.rect);
            }
            if r.clicked() && any_queued {
                for c in all.iter().zip(&standing).filter(|(_, s)| s.1) {
                    for &(s, _) in &c.0.targets {
                        ui_state.launches.push(Command::Cancel(Some(s)));
                    }
                }
                ui.close();
            }
            let label = current.map_or_else(
                || "Store macros (launch a scene first)".to_string(),
                |c| format!("Store macros into {}", c.name),
            );
            let store = kit::menu_item_tone(ui, st, &label, false, Tone::Text, current.is_some());
            ui_state.record(format!("pcue-store:{id}"), store.rect);
            if let (true, Some(c)) = (store.clicked(), current) {
                let now = |m, k| ui_state.macro_now(&state, m, k);
                crate::cues::store_macros(editor, id, c.n, &now);
                ui.close();
            }
        };
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
            Some("Scenes".to_string()),
            "Next bar",
            Some(&mut extra),
        );
        if all.is_empty() {
            kit::label(
                ui,
                st,
                Role::Caption,
                Tone::Text3,
                "No cues yet: add them in the drawer",
            );
        }
        ui.spacing_mut().item_spacing = vec2(gap, gap);
        for (row, flags) in all.chunks(per_row).zip(standing.chunks(per_row)) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for (c, &(active, queued)) in row.iter().zip(flags) {
                    let tag = button_mapping(&state, id, &format!("cue.{}", c.n)).map(cc_text);
                    let hover = format!(
                        "Launch {}{}",
                        c.name,
                        button_text(&state, id, &format!("cue.{}", c.n))
                            .map_or(String::new(), |t| format!("\n{t}"))
                    );
                    let (ps, sub) = if active {
                        (PadState::Playing, "Playing")
                    } else if queued {
                        (PadState::Queued, "Queued")
                    } else {
                        (PadState::Idle, "Ready")
                    };
                    // A queued pad fills toward the bar line it launches on; a playing one
                    // fills as its macros arrive.
                    let progress = (queued && !active)
                        .then(|| c.clock.and_then(|k| ui_state.clock_pos.get(&k)))
                        .flatten()
                        .map(|&p| p.rem_euclid(16.0) as f32 / 16.0)
                        .or_else(|| {
                            active
                                .then(|| {
                                    c.macros
                                        .iter()
                                        .filter_map(|&(m, k, to)| {
                                            ui_state
                                                .live_macro(m, k)
                                                .filter(|l| l.progress < 1.0 && l.to == to)
                                        })
                                        .map(|l| l.progress)
                                        .reduce(f32::min)
                                })
                                .flatten()
                        });
                    let hover = match &tag {
                        Some(t) => format!("{hover}\n{t}"),
                        None => hover,
                    };
                    let r =
                        kit::scene_pad(ui, st, vec2(pad_w, pad_h), &c.name, c.n, sub, ps, progress)
                            .tip(st, &hover);
                    ui_state.record(format!("pcue:{id}.{}", c.n), r.rect);
                    if r.clicked() {
                        match crate::cues::launch(&mut ui_state.launches, &state, id, c.n) {
                            Ok(()) => {
                                ui_state.last_cue.insert(id, c.n);
                            }
                            Err(e) => ui_state.last_message = Some(e),
                        }
                    }
                }
            });
        }
    });
    after_card(ui, ui_state, st, pin, rect);
}

/// What a macro moves: `Filter · Cutoff, Delay · Mix`, from the routes leaving its output.
fn macro_moves(state: &kabl_core::PatchState, id: ModuleId, port: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in state.cables.values() {
        let PortRef::Module { id: from, port: p } = &c.from else {
            continue;
        };
        if *from != id || p != port {
            continue;
        }
        // A route into another route's amount: "Depth: LFO to Ladder · Cutoff".
        if let PortRef::CableParam { cable, .. } = &c.to {
            if let Some(PortRef::Param { id: to, param }) = state.cables.get(cable).map(|r| &r.to) {
                let src = match state.cables.get(cable).map(|r| &r.from) {
                    Some(PortRef::Module { id, .. }) => module_short(state, *id),
                    _ => "?".into(),
                };
                let label = format!("Depth: {src} to {}", param_short(state, *to, param));
                if !out.contains(&label) {
                    out.push(label);
                }
            }
            continue;
        }
        let PortRef::Param { id: to, param } = &c.to else {
            continue;
        };
        if !state.modules.contains_key(to) {
            continue;
        }
        let label = param_short(state, *to, param);
        if !out.contains(&label) {
            out.push(label);
        }
    }
    out
}

/// "LFO", "Ladder": a module's short kind name.
fn module_short(state: &kabl_core::PatchState, id: ModuleId) -> String {
    let Some(m) = state.modules.get(&id) else {
        return "?".into();
    };
    match (routing::short_name(&m.kind), registry::info_for(&m.kind)) {
        ("?", Some(info)) => info.name.into(),
        (s, _) => s.into(),
    }
}

/// "Ladder · Cutoff": a module's short name and the label of one of its params.
fn param_short(state: &kabl_core::PatchState, id: ModuleId, param: &str) -> String {
    let name = state
        .modules
        .get(&id)
        .and_then(|m| registry::info_for(&m.kind))
        .and_then(|info| info.params.iter().find(|q| q.name == param))
        .map_or(param.to_string(), routing::target_label);
    format!("{} · {name}", module_short(state, id))
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
        (size.x - st.sp(4) * 2.0 - st.sp(1) * (pins.len() as f32 - 1.0)) / pins.len() as f32;
    // A knob never wider than its tile (eight macros share a narrow card).
    let knob = knob.min(tile_w - st.sp(2) * 2.0).max(KNOB_MIN / 2.0);
    // The card says which MIDI CCs the macros answer, when they are all mapped in a run.
    let ccs: Vec<u8> = pins
        .iter()
        .filter_map(|(_, p)| mapping(editor.state(), p.id, &p.key).map(|m| m.1))
        .collect();
    let note = match (ccs.iter().min(), ccs.iter().max()) {
        (Some(a), Some(b)) if ccs.len() == pins.len() && a != b => format!("MIDI CC {a}–{b}"),
        (Some(a), _) if ccs.len() == pins.len() => format!("MIDI CC {a}"),
        _ => String::new(),
    };
    ui.allocate_ui_with_layout(size, Layout::top_down(egui::Align::Min), |ui| {
        // The mockup's Perform cards (`card()` in directions.html) pad 16 at the sides; the
        // card's own 12 above and below keeps the title on the mockup's line.
        let f = kit::card_frame(st)
            .inner_margin(egui::Margin::symmetric(st.sp(4) as i8, st.sp(3) as i8));
        let m = f.inner_margin.sum();
        f.show(ui, |ui| {
            ui.set_width(size.x - m.x);
            ui.set_min_height(size.y - m.y);
            let row = ui.horizontal(|ui| {
                kit::section(ui, st, "Macros");
            });
            if !note.is_empty() {
                let r = row.response.rect;
                ui.painter().text(
                    egui::pos2(r.left() + size.x - m.x, r.center().y),
                    egui::Align2::RIGHT_CENTER,
                    &note,
                    st.font(Role::Caption),
                    st.roles.text3,
                );
            }
            ui.add_space(st.sp(1));
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = st.sp(1);
                for &(i, pin) in pins {
                    macro_tile(editor, ui_state, ui, st, pin, i, count, tile_w, knob);
                }
            });
        });
    });
}

/// The CC chip of a macro: the mapping (or "Learn") on a pill; click to learn another control.
/// The pickup state is in its tooltip, Clear in its menu.
fn cc_chip(
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
    let text = match (learning, map) {
        (true, _) => "Learning…".to_string(),
        (false, Some((_, cc))) => format!("CC {cc}"),
        (false, None) => "Learn".to_string(),
    };
    let font = egui::FontId::new(10.5, egui::FontFamily::Monospace);
    let g = ui.painter().layout_no_wrap(
        text,
        font,
        if learning {
            st.roles.on_accent
        } else {
            st.roles.text2
        },
    );
    let (rect, r) = ui.allocate_exact_size(vec2(g.size().x + 12.0, 18.0), Sense::click());
    let fill = if learning {
        st.roles.accent
    } else {
        st.roles.inset
    };
    ui.painter().rect_filled(rect, 9.0, fill);
    let ink = if learning {
        st.roles.on_accent
    } else {
        st.roles.text2
    };
    ui.painter().galley(rect.center() - g.size() / 2.0, g, ink);
    let tip = match map {
        Some(m) => {
            let t = ui_state
                .takeover
                .get(&(id, p.name.to_string()))
                .copied()
                .unwrap_or_default();
            let at = p.to_norm(ui_state.param_now(editor.state(), id, p));
            let pickup = if t.picked {
                "The hardware controls it".to_string()
            } else {
                format!(
                    "Pickup: move the hardware to {:.0} % to take over (it won't jump)",
                    at * 100.0
                )
            };
            format!(
                "{}: click to learn another control; right-click to clear\n{pickup}",
                cc_text(m)
            )
        }
        None => "Click, then move a hardware control".into(),
    };
    let r = r.tip(st, &tip);
    ui_state.record(format!("plearn:{key}"), r.rect);
    if r.clicked() {
        ui_state.learn = (!learning).then(|| (id, p.name.to_string()));
    }
    if map.is_some() {
        egui::Popup::context_menu(&r)
            .frame(kit::popover_frame(st))
            .show(|ui| {
                let c = kit::menu_item(ui, st, "Clear the MIDI mapping", false);
                ui_state.record(format!("pclear:{key}"), c.rect);
                if c.clicked() {
                    unlearn(editor, id, p.name);
                    ui.close();
                }
            });
    }
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
    let compact = knob < 110.0;
    let title = pin_label(editor.state(), pin);
    let fade = tool_fade(ui, &key);
    let rect = ui
        .allocate_ui_with_layout(
            vec2(tile_w, knob + if compact { 104.0 } else { 150.0 }),
            Layout::top_down(egui::Align::Center),
            |ui| {
                ui.set_width(tile_w);
                ui.spacing_mut().item_spacing.y = st.sp(1);
                let tile = ui.max_rect();
                // A cue may be moving this macro: show it where it is, with where it is heading
                // as the knob's range. A hand on the knob ends the morph (the engine does the
                // same when the value arrives).
                let k = p.name.as_bytes()[1] - b'1';
                let live = ui_state.live_macro(pin.id, k);
                let heading = live.filter(|m| m.progress < 1.0);
                let cur = live.map_or_else(
                    || routing::base_value(editor.state(), pin.id, p),
                    |m| m.value,
                );
                let span = heading.map(|m| (p.to_norm(m.value), p.to_norm(m.to)));
                let (resp, new) = kit::macro_knob(ui, st, p.to_norm(cur), knob, span);
                ui_state.record(format!("pslider:{key}"), resp.rect);
                let continuing = continuing(ui, &resp);
                if let Some(v) = new {
                    ui_state.macro_live.remove(&(pin.id, k));
                    let target = ParamTarget::Module {
                        id: pin.id,
                        param: p.name.into(),
                    };
                    editor.set_param_gesture(target, p.from_norm(v), !continuing);
                }
                // The value sits inside the knob, in the text colour of the raised body.
                let vs = if compact { 16.0 } else { 20.0 };
                ui.painter().text(
                    resp.rect.center() + vec2(0.0, if compact { 4.0 } else { 6.0 }),
                    egui::Align2::CENTER_CENTER,
                    format!("{:.0}", (p.to_norm(cur) * 100.0).clamp(0.0, 100.0)),
                    egui::FontId::new(vs, crate::style::Font::SansSemibold.family(st.fonts_ready)),
                    st.roles.text,
                );
                if let Some(m) = heading {
                    kit::label(
                        ui,
                        st,
                        Role::Caption,
                        Tone::Accent,
                        format!(
                            "→ {} · {:.0}%",
                            routing::fmt_value(p, m.to),
                            m.progress * 100.0
                        ),
                    )
                    .tip(st, "A cue is moving this macro; turn it to take over");
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_millis(30));
                }
                // The name, centred: 22 px semibold (17 compact).
                let font = egui::FontId::new(
                    if compact { 17.0 } else { 22.0 },
                    crate::style::Font::SansSemibold.family(st.fonts_ready),
                );
                ui.add_space(st.sp(1));
                pin_title(
                    editor,
                    ui_state,
                    ui,
                    st,
                    pin,
                    &title,
                    Role::Title,
                    false,
                    Some(font),
                );
                cc_chip(editor, ui_state, ui, st, pin.id, p);
                let moves = macro_moves(editor.state(), pin.id, p.name);
                let tone = if moves.is_empty() {
                    Tone::Text3
                } else {
                    Tone::Text2
                };
                let tip = if moves.is_empty() {
                    "Route this macro's output to controls in the drawer".to_string()
                } else {
                    moves.join("\n")
                };
                ui.add_space(st.sp(1));
                if moves.is_empty() {
                    kit::label(ui, st, Role::Caption, tone, "moves nothing yet").tip(st, &tip);
                } else if compact {
                    let t = if moves.len() == 1 {
                        moves[0].clone()
                    } else {
                        format!("{} +{}", moves[0], moves.len() - 1)
                    };
                    kit::label_truncated(ui, st, Role::Caption, tone, t).tip(st, &tip);
                } else {
                    // One destination per line, two at most; the rest as "+n more".
                    let shown = if moves.len() > 2 { 1 } else { moves.len() };
                    for m in &moves[..shown] {
                        kit::label_truncated(ui, st, Role::Caption, tone, m.clone()).tip(st, &tip);
                    }
                    if moves.len() > shown {
                        kit::label(
                            ui,
                            st,
                            Role::Caption,
                            tone,
                            format!("+{} more", moves.len() - shown),
                        )
                        .tip(st, &tip);
                    }
                }
                // Explain and the menu, at the tile's top right while the pointer is over it.
                let tools = egui::Rect::from_min_size(
                    egui::pos2(tile.right() - 60.0, tile.top()),
                    vec2(60.0, 26.0),
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(tools)
                        .layout(Layout::right_to_left(egui::Align::Min)),
                    |ui| {
                        pin_tools(
                            editor, ui_state, ui, st, pin, index, count, &title, fade, None,
                        );
                    },
                );
            },
        )
        .response
        .rect;
    note_hover(ui, &key, rect);
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
            let at = p.to_norm(ui_state.param_now(editor.state(), id, p));
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
                    None => "↔",
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
            "",
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

/// One "Sequences" card with a row per sequencer: its name, the A-D bank chips and the step
/// lane (level bars, probability as brightness, the playing step outlined).
#[allow(clippy::too_many_arguments)]
fn sequences_card(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    banks: &[(usize, &Pin)],
    count: usize,
    size: egui::Vec2,
) {
    let n = banks.len().max(1) as f32;
    let row_h = ((size.y - 64.0) / n).clamp(44.0, 110.0);
    let hues = [st.roles.gate, st.roles.audio, st.roles.pitch];
    let rect = shell(ui, ui_state, st, "sequences", size, |ui, ui_state| {
        let state = editor.state().clone();
        let title_of = |pin: &Pin| {
            let named = state
                .label(pin.id, &format!("{}{}", perform::PIN_PREFIX, pin.key))
                .is_some();
            if named {
                pin_label(&state, pin)
            } else if banks.len() > 1 {
                format!("{} #{}", perform::module_name(&state, pin.id), pin.id)
            } else {
                perform::module_name(&state, pin.id).to_string()
            }
        };
        // Explain and the menu sit in the kicker row, for the sequencer under the pointer
        // (the first one otherwise); they fade in while the pointer is over the card.
        let active_id = egui::Id::new("seq-active");
        let active = ui
            .ctx()
            .data(|d| d.get_temp::<usize>(active_id))
            .unwrap_or(0)
            .min(banks.len().saturating_sub(1));
        let fade = tool_fade(ui, "sequences");
        ui.horizontal(|ui| {
            kit::section(ui, st, "Sequences");
            if let Some(&(index, pin)) = banks.get(active) {
                let id = pin.id;
                let title = title_of(pin);
                let queued_now = ui_state.seq_banks.get(&id).is_some_and(|b| b.1.is_some());
                let mut extra = |ui: &mut egui::Ui, _: &mut PatchEditor, ui_state: &mut UiState| {
                    let r = kit::menu_item_tone(
                        ui,
                        st,
                        "Cancel the queued bank",
                        false,
                        Tone::Text,
                        queued_now,
                    );
                    if queued_now {
                        ui_state.record(format!("pbank-cancel:{id}"), r.rect);
                    }
                    if r.clicked() && queued_now {
                        ui_state.launches.push(Command::Cancel(Some(id)));
                        ui.close();
                    }
                };
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    pin_tools(
                        editor,
                        ui_state,
                        ui,
                        st,
                        pin,
                        index,
                        count,
                        &title,
                        fade,
                        Some(&mut extra),
                    );
                });
            }
        });
        ui.add_space(st.sp(1));
        for (k, &(_, pin)) in banks.iter().enumerate() {
            let id = pin.id;
            let key = format!("{}.{}", pin.id, pin.key);
            let title = title_of(pin);
            let (playing, queued) = ui_state
                .seq_banks
                .get(&id)
                .copied()
                .unwrap_or((crate::banks::startup(&state, id), None));
            let steps = lane(&state, id, playing);
            let row = ui
                .allocate_ui_with_layout(
                    vec2(ui.available_width(), row_h),
                    Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.spacing_mut().item_spacing.x = st.sp(1);
                        // A fixed name column, so every lane starts at the same x.
                        let (name_rect, _) =
                            ui.allocate_exact_size(vec2(74.0, row_h), Sense::hover());
                        // A child that does not claim space in the row again.
                        let mut name_ui = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(name_rect)
                                .layout(Layout::left_to_right(egui::Align::Center)),
                        );
                        let font = egui::FontId::new(
                            14.0,
                            crate::style::Font::SansSemibold.family(st.fonts_ready),
                        );
                        pin_title(
                            editor,
                            ui_state,
                            &mut name_ui,
                            st,
                            pin,
                            &title,
                            Role::Body,
                            false,
                            Some(font),
                        );
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
                            let r = bank_chip(ui, st, seq::BANK_NAMES[b], ps).tip(st, &hover);
                            ui_state.record(format!("pbank:{id}.{b}"), r.rect);
                            if r.clicked() {
                                if let Err(e) =
                                    crate::banks::launch(&mut ui_state.launches, &state, id, b)
                                {
                                    ui_state.last_message = Some(e);
                                }
                            }
                        }
                        ui.add_space(st.sp(2));
                        let w = ui.available_width();
                        kit::seq_lane(
                            ui,
                            st,
                            vec2(w, row_h),
                            &steps,
                            ui_state.seq_steps.get(&id).copied(),
                            hues[k % 3],
                        );
                    },
                )
                .response
                .rect;
            let over = ui
                .input(|i| i.pointer.hover_pos())
                .is_some_and(|p| row.contains(p));
            if over {
                ui.ctx().data_mut(|d| d.insert_temp(active_id, k));
            }
            ui_state.record(format!("pcard:{key}"), row);
            after_card(ui, ui_state, st, pin, row);
        }
    });
    let _ = rect;
}

/// A bank chip, 28 x 24: the playing bank is filled with the accent, the queued one outlined.
fn bank_chip(ui: &mut egui::Ui, st: &Style, text: &str, state: PadState) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 24.0), Sense::click());
    let r = &st.roles;
    let hover = ui
        .ctx()
        .animate_bool_with_time(resp.id.with("hover"), resp.hovered(), 0.1);
    let (fill, line, ink) = match state {
        PadState::Playing => (r.accent, r.accent, r.on_accent),
        PadState::Queued => (r.raised, r.accent, r.text),
        PadState::Idle => (
            crate::style::mix(r.raised, r.text, 0.05 * hover),
            r.line,
            r.text2,
        ),
    };
    ui.painter().rect_filled(rect, 6.0, fill);
    ui.painter().rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(if state == PadState::Queued { 2.0 } else { 1.0 }, line),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::new(12.0, egui::FontFamily::Monospace),
        ink,
    );
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_shares_spare_width_by_weight_and_keeps_minimums_when_short() {
        let w = split(1000.0, 10.0, &[(100.0, 0.0), (200.0, 1.0), (200.0, 3.0)]);
        assert_eq!(w[0], 100.0, "weight 0 stays fixed");
        assert!((w.iter().sum::<f32>() + 20.0 - 1000.0).abs() < 0.01);
        assert!(w[2] - 200.0 > 2.9 * (w[1] - 200.0));
        assert_eq!(
            split(300.0, 10.0, &[(200.0, 1.0), (200.0, 1.0)]),
            [200.0, 200.0]
        );
    }

    #[test]
    fn position_in_sixteenths_gives_bar_beat_and_place_in_the_bar() {
        assert_eq!(bar_beat(0.0), (1, 1, 0.0));
        assert_eq!(bar_beat(5.0), (1, 2, 1.25));
        assert_eq!(bar_beat(16.0), (2, 1, 0.0));
        assert_eq!(bar_beat(31.9).0, 2);
        assert_eq!(bar_beat(31.9).1, 4);
    }

    /// A macro whose only route moves another route's depth says what that depth is for.
    #[test]
    fn a_macro_that_drives_a_depth_says_what_it_moves() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/macros-eight");
        let log = kabl_core::load(&dir).unwrap();
        let state = log.state();
        let id = state
            .modules
            .iter()
            .find(|(_, m)| m.kind == "macro")
            .map(|(&id, _)| id)
            .unwrap();
        let moves = macro_moves(state, id, "m7");
        assert_eq!(moves.len(), 1, "{moves:?}");
        assert!(
            moves[0].starts_with("Depth: LFO to ") && moves[0].ends_with("Cutoff"),
            "{moves:?}"
        );
        assert_eq!(macro_moves(state, id, "m1").len(), 1);
    }
}
