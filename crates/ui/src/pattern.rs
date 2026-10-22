//! The cable pattern editor: a route's step pattern, chance, morph and glide, drawn with the
//! chrome kit from the active `Style`. Every value is an ordinary cable param edit (undo, save
//! and the engine's in-place path see exactly what is shown); nothing here changes what a
//! pattern means or how it is stored.

use crate::kit::{self, Tone};
use crate::routing::{cable_param_label, cable_param_routes, source_label};
use crate::style::{alpha, mix, Role, Style};
use crate::{PatchEditor, UiState};
use egui::{pos2, vec2, Align2, Rect, Sense, Stroke, Ui};
use kabl_core::{CableId, ParamTarget, PortRef};

const NUM_H: f32 = 12.0;
const LEVEL_H: f32 = 40.0;
const CHANCE_H: f32 = 16.0;

/// One edit a grid asks for: `(slot, value, starts_a_new_undo_entry)`.
type Edit = (usize, f32, bool);

/// The step of cable pattern `len` steps long that the reference clock is on, while it runs.
/// Steps advance on each pulse (a 16th) since the last restart: pulse `t` plays step `t % len`.
pub(crate) fn playing_step(editor: &PatchEditor, ui_state: &UiState, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    // The engine follows the lowest-id clock (`compile.rs`, `reference_clock`).
    let clock = editor
        .state()
        .modules
        .iter()
        .find(|(_, m)| m.kind == "clock")
        .map(|(&id, _)| id)?;
    if !ui_state.clock_running.get(&clock).copied().unwrap_or(true) {
        return None;
    }
    let pos = *ui_state.clock_pos.get(&clock)?;
    Some(pos.floor().max(0.0) as usize % len)
}

/// "Filter #4 cutoff", for the end a cable goes to.
fn dest_label(editor: &PatchEditor, to: &PortRef) -> String {
    match to {
        PortRef::Module { id, port } => source_label(editor.state(), *id, port),
        PortRef::Param { id, param } => source_label(editor.state(), *id, param),
        PortRef::CableParam { cable, param } => {
            format!("{} of cable #{cable}", cable_param_label(param))
        }
    }
}

struct Lane<'a> {
    cable: CableId,
    tag: &'a str,
    level0: usize,
    chance0: usize,
    levels: Vec<f32>,
    chances: Vec<f32>,
    /// The level each step has once morph blends pattern B in, when it differs from `levels`.
    blend: Option<Vec<f32>>,
    hue: egui::Color32,
    playing: Option<usize>,
}

/// Paints and edits one pattern: per step a level bar (click toggles the step, drag up and
/// down sets it, drag across steps paints) over a chance bar, with the values in numbers.
fn step_grid(ui: &mut Ui, st: &Style, ui_state: &mut UiState, l: &Lane) -> Vec<Edit> {
    let r = &st.roles;
    let n = l.levels.len();
    let per_row = if n > 8 { n.div_ceil(2) } else { n };
    let gap = st.sp(1) * 0.75;
    // The drawer clips at its scroll area: never run past it, whatever the nominal width.
    let w = ui
        .available_width()
        .min(ui.clip_rect().right() - ui.cursor().left())
        - ui.spacing().item_spacing.x;
    let cw = (w - gap * (per_row as f32 - 1.0)) / per_row as f32;
    let mut edits = Vec::new();
    for row in 0..n.div_ceil(per_row) {
        let first = row * per_row;
        let cnt = (n - first).min(per_row);
        let height = NUM_H + LEVEL_H + CHANCE_H + NUM_H + gap * 3.0;
        let (rect, _) = ui.allocate_exact_size(vec2(w, height), Sense::hover());
        let bars = Rect::from_min_size(pos2(rect.left(), rect.top() + NUM_H), vec2(w, LEVEL_H));
        let odds = Rect::from_min_size(pos2(rect.left(), bars.bottom() + gap), vec2(w, CHANCE_H));
        let cell_x = |i: usize| rect.left() + i as f32 * (cw + gap);
        let step_at =
            |x: f32| (((x - rect.left()) / (cw + gap)).floor().max(0.0) as usize).min(cnt - 1);
        for (lane, is_level) in [(bars, true), (odds, false)] {
            let id = ui
                .id()
                .with(("pattern-lane", l.cable, l.tag, row, is_level));
            let resp = ui.interact(lane, id, Sense::click_and_drag());
            let mem = id.with("last");
            let prev: Option<usize> = ui.data(|d| d.get_temp(mem));
            let value_at = |y: f32| {
                (((lane.bottom() - y) / lane.height()).clamp(0.0, 1.0) * 100.0).round() / 100.0
            };
            if let Some(p) = resp.interact_pointer_pos() {
                let k = step_at(p.x);
                if resp.dragged() || (resp.clicked() && !is_level) {
                    let (a, b) = prev.map_or((k, k), |q| (q.min(k), q.max(k)));
                    for j in a..=b {
                        let slot = first + j;
                        let v = value_at(p.y);
                        let (idx, scale) = if is_level {
                            (l.level0 + slot, 1.0)
                        } else {
                            (l.chance0 + slot, 100.0)
                        };
                        edits.push((idx, v * scale, prev != Some(j)));
                    }
                    ui.data_mut(|d| d.insert_temp(mem, k));
                } else if resp.clicked() && is_level {
                    // A click toggles the gate: closing keeps the level to come back to.
                    let slot = first + k;
                    let key = ui.id().with(("pattern-mem", l.cable, l.level0 + slot));
                    if l.levels[slot] > 0.0 {
                        ui.data_mut(|d| d.insert_temp(key, l.levels[slot]));
                        edits.push((l.level0 + slot, 0.0, true));
                    } else {
                        let back = ui.data(|d| d.get_temp::<f32>(key)).unwrap_or(1.0);
                        edits.push((l.level0 + slot, back, true));
                    }
                }
            }
            if resp.drag_stopped() {
                ui.data_mut(|d| d.remove_temp::<usize>(mem));
            }
        }
        let p = ui.painter();
        for i in 0..cnt {
            let k = first + i;
            let x = cell_x(i);
            let level = l.levels[k];
            let chance = l.chances[k];
            let on = level > 0.0;
            let now = l.playing == Some(k);
            let cell = Rect::from_min_size(pos2(x, bars.top()), vec2(cw, LEVEL_H));
            let odd = Rect::from_min_size(pos2(x, odds.top()), vec2(cw, CHANCE_H));
            ui_state.record(format!("fn-level{}:{}:{k}", l.tag, l.cable), cell);
            ui_state.record(format!("fn-chance{}:{}:{k}", l.tag, l.cable), odd);
            p.rect_filled(cell, 3.0, if now { r.accent_soft } else { r.inset });
            p.rect_filled(odd, 3.0, r.inset);
            let h = if on {
                (cell.height() * level).max(3.0)
            } else {
                3.0
            };
            let col = if on {
                mix(alpha(l.hue, 120), l.hue, chance / 100.0)
            } else {
                r.line2
            };
            p.rect_filled(
                Rect::from_min_max(pos2(cell.left(), cell.bottom() - h), cell.max),
                3.0,
                col,
            );
            if let Some(b) = l.blend.as_ref().map(|b| b[k]) {
                let y = cell.bottom() - cell.height() * b;
                p.line_segment(
                    [pos2(cell.left(), y), pos2(cell.right(), y)],
                    Stroke::new(2.0, kit::tone(st, Tone::Text)),
                );
            }
            let ch = (odd.height() * chance / 100.0).max(2.0);
            p.rect_filled(
                Rect::from_min_max(pos2(odd.left(), odd.bottom() - ch), odd.max),
                3.0,
                alpha(l.hue, if on { 150 } else { 60 }),
            );
            if now {
                p.rect_stroke(
                    cell,
                    3.0,
                    Stroke::new(1.5, kit::tone(st, Tone::Text)),
                    egui::StrokeKind::Inside,
                );
            }
            let num = |text: String, y: f32, tone: Tone| {
                p.text(
                    pos2(cell.center().x, y),
                    Align2::CENTER_CENTER,
                    text,
                    st.font(Role::Caption),
                    kit::tone(st, tone),
                );
            };
            num(
                if on {
                    format!("{:.0}", level * 100.0)
                } else {
                    "off".into()
                },
                rect.top() + NUM_H / 2.0,
                if now { Tone::Text } else { Tone::Text2 },
            );
            num(
                format!("{chance:.0}"),
                odd.bottom() + gap + NUM_H / 2.0,
                if on { Tone::Text2 } else { Tone::Text3 },
            );
        }
        // The step under the pointer, named with its values.
        if let Some(q) = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|q| rect.contains(*q))
        {
            let k = first + step_at(q.x);
            let tip = format!(
                "Step {}: level {:.0} %, chance {:.0} %",
                k + 1,
                l.levels[k] * 100.0,
                l.chances[k]
            );
            egui::Tooltip::always_open(
                ui.ctx().clone(),
                ui.layer_id(),
                ui.id().with(("pattern-tip", l.cable, l.tag, row)),
                egui::PopupAnchor::Pointer,
            )
            .gap(12.0)
            .show(|ui| {
                ui.label(tip);
            });
        }
    }
    edits
}

/// A labelled slider row; the returned value is `Some` when the hand moved it, with whether
/// that starts a new undo entry.
#[allow(clippy::too_many_arguments)]
fn slider_row(
    ui: &mut Ui,
    st: &Style,
    ui_state: &mut UiState,
    key: String,
    name: &str,
    value: f32,
    range: std::ops::RangeInclusive<f64>,
    step: f64,
    readout: String,
) -> Option<(f32, bool)> {
    let mut v = value as f64;
    let resp = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = st.sp(2);
            let (label, _) =
                ui.allocate_exact_size(vec2(st.sp(8) * 1.5, st.metrics.control_h), Sense::hover());
            ui.painter().text(
                label.left_center(),
                Align2::LEFT_CENTER,
                name,
                st.font(Role::Label),
                kit::tone(st, Tone::Text2),
            );
            kit::slider(ui, st, &mut v, range, Some(step), &readout)
        })
        .inner;
    ui_state.record(key, resp.rect);
    resp.changed()
        .then_some((v as f32, !resp.dragged() || resp.drag_started()))
}

/// The pattern and probability of the cable `ui_state.cable_fn`, opened from a cable's menu or
/// a route row.
pub(crate) fn panel(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut Ui) {
    let Some(cable) = ui_state.cable_fn else {
        return;
    };
    let Some(c) = editor.state().cables.get(&cable).cloned() else {
        ui_state.cable_fn = None;
        return;
    };
    let st = ui_state.style.clone();
    let th = crate::theme::theme(&st);
    let hue = crate::cable_look::legible_cached(
        ui.ctx(),
        &st,
        crate::port_color(&th, editor, &c.from),
        1.0,
    );
    let has_clock = editor.state().modules.values().any(|m| m.kind == "clock");
    let get = |slot| kabl_cables::slot_value(&c.params, slot);
    let mut edits: Vec<Edit> = Vec::new();

    ui.add_space(st.sp(2));
    kit::card(ui, &st, |ui| {
        ui.horizontal(|ui| {
            let (dot, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
            ui.painter().circle_filled(dot.center(), 5.0, hue);
            kit::label(ui, &st, Role::H3, Tone::Text, format!("Cable #{cable}"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let r = ui.add(kit::Button::new(&st, "Close").ghost().small());
                ui_state.record(format!("fn-close:{cable}"), r.rect);
                if r.clicked() {
                    ui_state.cable_fn = None;
                }
            });
        });
        let from = match &c.from {
            PortRef::Module { id, port } => source_label(editor.state(), *id, port),
            other => dest_label(editor, other),
        };
        kit::label_truncated(
            ui,
            &st,
            Role::Caption,
            Tone::Text2,
            format!("{from}  \u{2192}  {}", dest_label(editor, &c.to)),
        );
    });
    if ui_state.cable_fn != Some(cable) {
        return;
    }
    ui.add_space(st.sp(1));
    if ui_state.explain.help {
        kit::paragraph(
            &mut *ui,
            &st,
            Role::Caption,
            Tone::Text3,
            "Steps advance on each pulse of the patch clock (a 16th note) and restart with it. \
             Click a step to switch it on or off; drag a bar to set its level, drag across \
             steps to paint. The lower bar is the chance the step passes.",
        );
    }
    if !has_clock {
        kit::paragraph(
            &mut *ui,
            &st,
            Role::Caption,
            Tone::Warn,
            "No Clock in this patch: the cable passes everything.",
        );
    }
    ui.add_space(st.sp(1));

    let len = get(0);
    if let Some((v, first)) = slider_row(
        ui,
        &st,
        ui_state,
        format!("fn-length:{cable}"),
        "Steps",
        len,
        0.0..=kabl_cables::MAX_STEPS as f64,
        1.0,
        if len < 1.0 {
            "off".into()
        } else {
            format!("{len:.0}")
        },
    ) {
        edits.push((0, v.round(), first));
    }
    let prob = get(1);
    if let Some((v, first)) = slider_row(
        ui,
        &st,
        ui_state,
        format!("fn-prob:{cable}"),
        "Chance",
        prob,
        0.0..=100.0,
        1.0,
        format!("{prob:.0} %"),
    ) {
        edits.push((1, v, first));
    }
    cable_param_routes(ui, editor, ui_state, cable, "prob");

    let (len_a, len_b) = (get(0) as usize, get(kabl_cables::B_LENGTH) as usize);
    let morph = get(kabl_cables::MORPH);
    let (play_a, play_b) = (
        playing_step(editor, ui_state, len_a),
        playing_step(editor, ui_state, len_b),
    );
    let lane = |tag, len: usize, level0, chance0, blend: Option<Vec<f32>>, playing| Lane {
        cable,
        tag,
        level0,
        chance0,
        levels: (0..len).map(|k| get(level0 + k)).collect(),
        chances: (0..len).map(|k| get(chance0 + k)).collect(),
        blend,
        hue,
        playing,
    };
    if len_a > 0 {
        ui.add_space(st.sp(1));
        kit::section(ui, &st, "Pattern");
        let blend = (len_b > 0 && morph > 0.0).then(|| {
            (0..len_a)
                .map(|k| {
                    let b = get(kabl_cables::B_LEVEL0 + k % len_b);
                    (1.0 - morph) * get(kabl_cables::LEVEL0 + k) + morph * b
                })
                .collect()
        });
        let g = lane(
            "",
            len_a,
            kabl_cables::LEVEL0,
            kabl_cables::CHANCE0,
            blend,
            play_a,
        );
        if g.playing.is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(33));
        }
        edits.extend(step_grid(ui, &st, ui_state, &g));
    }

    ui.add_space(st.sp(1));
    kit::rule(ui, &st);
    kit::section(ui, &st, "Morph");
    if let Some((v, first)) = slider_row(
        ui,
        &st,
        ui_state,
        format!("fn-morph:{cable}"),
        "A to B",
        morph * 100.0,
        0.0..=100.0,
        1.0,
        format!("{:.0} %", morph * 100.0),
    ) {
        edits.push((kabl_cables::MORPH, v / 100.0, first));
    }
    cable_param_routes(ui, editor, ui_state, cable, "morph");
    let glide = get(kabl_cables::GLIDE);
    if let Some((v, first)) = slider_row(
        ui,
        &st,
        ui_state,
        format!("fn-glide:{cable}"),
        "Glide",
        glide,
        0.0..=2000.0,
        1.0,
        format!("{glide:.0} ms"),
    ) {
        edits.push((kabl_cables::GLIDE, v, first));
    }
    cable_param_routes(ui, editor, ui_state, cable, "glide_ms");
    let lb = get(kabl_cables::B_LENGTH);
    if let Some((v, first)) = slider_row(
        ui,
        &st,
        ui_state,
        format!("fn-b-length:{cable}"),
        "B steps",
        lb,
        0.0..=kabl_cables::MAX_STEPS as f64,
        1.0,
        if lb < 1.0 {
            "off".into()
        } else {
            format!("{lb:.0}")
        },
    ) {
        edits.push((kabl_cables::B_LENGTH, v.round(), first));
    }
    if len_b > 0 {
        let g = lane(
            "-b",
            len_b,
            kabl_cables::B_LEVEL0,
            kabl_cables::B_CHANCE0,
            None,
            play_b,
        );
        if g.playing.is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(33));
        }
        edits.extend(step_grid(ui, &st, ui_state, &g));
    }

    for (slot, v, first) in edits {
        editor.set_param_gesture(
            ParamTarget::Cable {
                id: cable,
                param: kabl_cables::slot_name(slot),
            },
            v,
            first,
        );
    }
}
