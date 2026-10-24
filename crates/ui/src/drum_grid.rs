//! The `drum.grid` face: a step grid drawn on the module panel. Click toggles a step, drag
//! paints (starting on an empty cell writes the brush, starting on a cell that already holds the
//! brush erases), the scroll wheel sets a step's velocity, a right click cycles its chance, and
//! shift-click sets the row's length. Every edit is an ordinary param edit of the module, so
//! undo, save and the engine's in-place path see exactly what is shown.

use crate::pattern::playing_step;
use crate::theme::Theme;
use crate::{PatchEditor, UiState, Xf};
use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, FontId, Id, Painter, Rect, Sense, Stroke,
    StrokeKind, Ui,
};
use kabl_core::{ModuleId, ParamTarget};
use kabl_modules::builtins::{
    cell_index, code_in, len_index, levels, make_code, pattern_index, with_code, CHANCE,
    DRUM_GRID_INFO, GRID_ROWS, GRID_ROW_NAMES, GRID_STEPS, PATTERN_NAMES, VELOCITY,
};
use std::collections::HashMap;

const LABEL_W: f32 = 50.0;
const HEAD_H: f32 = 20.0;
const ROW_GAP: f32 = 2.0;
const PER_CHUNK: usize = 4;
const CHUNKS: usize = GRID_STEPS / PER_CHUNK;

/// Face-space height the grid needs.
pub const HEIGHT: f32 = HEAD_H + 6.0 + GRID_ROWS as f32 * (18.0 + ROW_GAP);

/// What a drag writes into each cell it crosses.
#[derive(Clone, Copy, Default)]
enum Paint {
    Write(u32),
    #[default]
    Erase,
}

fn row_colour(row: usize, dark: bool) -> Color32 {
    let (s, v) = if dark { (0.55, 0.92) } else { (0.62, 0.72) };
    egui::ecolor::Hsva::new(((row as f32 * 0.115 + 0.02) % 1.0).max(0.0), s, v, 1.0).into()
}

fn with_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// The chunk params of `row` in pattern `pat` as they stand, edits of this frame included.
struct Cells<'a> {
    id: ModuleId,
    editor: &'a PatchEditor,
    pending: HashMap<usize, f32>,
}

impl Cells<'_> {
    fn value(&self, index: usize) -> f32 {
        let p = &DRUM_GRID_INFO.params[index];
        self.pending.get(&index).copied().unwrap_or_else(|| {
            self.editor
                .state()
                .modules
                .get(&self.id)
                .and_then(|m| m.params.get(p.name).copied())
                .unwrap_or(p.default)
        })
    }

    fn code(&self, pat: usize, row: usize, step: usize) -> u32 {
        let mut chunks = [0.0; CHUNKS];
        for (c, v) in chunks.iter_mut().enumerate() {
            *v = self.value(cell_index(pat, row, c));
        }
        code_in(chunks, step)
    }

    fn set_code(&mut self, pat: usize, row: usize, step: usize, code: u32) {
        let i = cell_index(pat, row, step / PER_CHUNK);
        let v = with_code(self.value(i), step % PER_CHUNK, code);
        self.pending.insert(i, v);
    }

    fn length(&self, row: usize) -> usize {
        self.value(len_index(row))
            .round()
            .clamp(1.0, GRID_STEPS as f32) as usize
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut Ui,
    painter: &Painter,
    th: &Theme,
    xf: Xf,
    id: ModuleId,
    r: Rect,
) {
    let z = xf.zoom;
    let font = |s: f32| FontId::proportional(s * z);
    let small = 10.5 * z >= 8.5;
    let mut cells = Cells {
        id,
        editor: &*editor,
        pending: HashMap::new(),
    };
    let pat = cells.value(pattern_index()).round().clamp(0.0, 3.0) as usize;
    let brush_id = Id::new(("kabl-grid-brush", id));
    let (mut bv, mut bc) = ui.data(|d| d.get_temp::<(usize, usize)>(brush_id).unwrap_or((3, 0)));
    let mut set_pattern = None;
    let mut first_gesture = false;

    // Header: pattern tabs on the left, the brush on the right.
    let mut button = |ui: &mut Ui, rect: Rect, key: &str, on: bool, label: &str| -> bool {
        let rect = xf.r(rect);
        ui_state.record(key.to_string(), rect);
        let resp = ui.interact(rect, Id::new((key, id)), Sense::click());
        painter.rect_filled(
            rect,
            CornerRadius::same(3),
            if on {
                th.sel
            } else if th.dark {
                th.btn
            } else {
                th.plate
            },
        );
        if small {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                label,
                font(10.5),
                if on { th.sel_ink } else { th.plate_ink },
            );
        }
        resp.clicked()
    };
    let hy = r.top();
    for (p, name) in PATTERN_NAMES.iter().enumerate() {
        let b = Rect::from_min_size(pos2(r.left() + p as f32 * 28.0, hy), vec2(24.0, HEAD_H));
        if button(ui, b, &format!("grid-pattern:{id}.{p}"), p == pat, name) {
            set_pattern = Some(p);
        }
    }
    // Brush: four velocity levels, four chances.
    let bw = 30.0;
    let bx = r.right() - 8.0 * (bw + 3.0) - 44.0;
    if small {
        painter.text(
            xf.p(pos2(bx - 6.0, hy + HEAD_H / 2.0)),
            Align2::RIGHT_CENTER,
            "BRUSH",
            font(9.5),
            th.ink2,
        );
    }
    for (v, level) in VELOCITY.iter().enumerate() {
        let b = Rect::from_min_size(pos2(bx + v as f32 * (bw + 3.0), hy), vec2(bw, HEAD_H));
        let label = format!("{:.0}", level * 100.0);
        if button(ui, b, &format!("grid-vel:{id}.{v}"), v == bv, &label) {
            bv = v;
        }
    }
    for (c, chance) in CHANCE.iter().enumerate() {
        let b = Rect::from_min_size(
            pos2(bx + (4 + c) as f32 * (bw + 3.0) + 8.0, hy),
            vec2(bw, HEAD_H),
        );
        let label = format!("{}%", chance);
        if button(ui, b, &format!("grid-chance:{id}.{c}"), c == bc, &label) {
            bc = c;
        }
    }
    ui.data_mut(|d| d.insert_temp(brush_id, (bv, bc)));

    // Grid geometry.
    let gx = r.left() + LABEL_W;
    let gy = r.top() + HEAD_H + 6.0;
    let cw = (r.right() - gx) / GRID_STEPS as f32;
    let rh = 18.0;
    let cell_rect = |row: usize, step: usize| {
        Rect::from_min_size(
            pos2(
                gx + step as f32 * cw + 1.0,
                gy + row as f32 * (rh + ROW_GAP),
            ),
            vec2(cw - 2.0, rh),
        )
    };
    let area = Rect::from_min_max(
        pos2(gx, gy),
        pos2(
            gx + cw * GRID_STEPS as f32,
            gy + GRID_ROWS as f32 * (rh + ROW_GAP),
        ),
    );
    let area_s = xf.r(area);
    ui_state.record(format!("grid:{id}"), area_s);
    let resp = ui.interact(area_s, Id::new(("kabl-grid", id)), Sense::click_and_drag());
    let cell_at = |p: egui::Pos2| -> Option<(usize, usize)> {
        let w = xf.inv(p);
        if !area.contains(w) {
            return None;
        }
        let row = (((w.y - gy) / (rh + ROW_GAP)).floor().max(0.0) as usize).min(GRID_ROWS - 1);
        let step = (((w.x - gx) / cw).floor().max(0.0) as usize).min(GRID_STEPS - 1);
        Some((row, step))
    };
    let paint_id = Id::new(("kabl-grid-paint", id));
    let mem_id = Id::new(("kabl-grid-last", id));
    let brush_code = make_code(bv, bc);
    let shift = ui.input(|i| i.modifiers.shift);
    let started = resp.drag_started() || resp.clicked();
    let origin = if resp.drag_started() {
        ui.input(|i| i.pointer.press_origin())
    } else {
        resp.interact_pointer_pos()
    };
    if let (true, Some((row, step))) = (started, origin.and_then(cell_at)) {
        first_gesture = true;
        if shift {
            cells.pending.insert(len_index(row), (step + 1) as f32);
            ui.data_mut(|d| d.remove_temp::<Paint>(paint_id));
        } else {
            let mode = if cells.code(pat, row, step) == brush_code {
                Paint::Erase
            } else {
                Paint::Write(brush_code)
            };
            ui.data_mut(|d| {
                d.insert_temp(paint_id, mode);
                d.remove_temp::<(usize, usize)>(mem_id);
            });
        }
    }
    if resp.dragged() || resp.clicked() {
        let mode = ui.data(|d| d.get_temp::<Paint>(paint_id));
        if let (Some(mode), Some((row, step))) =
            (mode, resp.interact_pointer_pos().and_then(cell_at))
        {
            let prev = ui.data(|d| d.get_temp::<(usize, usize)>(mem_id));
            let (a, b) = match prev {
                Some((pr, ps)) if pr == row => (ps.min(step), ps.max(step)),
                _ => (step, step),
            };
            for s in a..=b {
                cells.set_code(
                    pat,
                    row,
                    s,
                    match mode {
                        Paint::Write(c) => c,
                        Paint::Erase => 0,
                    },
                );
            }
            ui.data_mut(|d| d.insert_temp(mem_id, (row, step)));
        }
    }
    if resp.drag_stopped() || resp.clicked() {
        ui.data_mut(|d| d.remove_temp::<(usize, usize)>(mem_id));
    }
    // Right click: the next chance. Scroll: velocity.
    if let Some((row, step)) = resp.hover_pos().and_then(cell_at) {
        let code = cells.code(pat, row, step);
        if code != 0 {
            let (v, c) = levels(code);
            if resp.secondary_clicked() {
                cells.set_code(pat, row, step, make_code(v, (c + 1) % 4));
                first_gesture = true;
            }
            // A wheel notch arrives smoothed over frames: one level per 24 px of it.
            let acc_id = Id::new(("kabl-grid-scroll", id));
            let acc = ui.data(|d| d.get_temp::<f32>(acc_id).unwrap_or(0.0))
                + ui.input(|i| i.smooth_scroll_delta.y);
            let scroll = if acc.abs() >= 24.0 { acc.signum() } else { 0.0 };
            ui.data_mut(|d| d.insert_temp(acc_id, if scroll != 0.0 { 0.0 } else { acc }));
            if scroll != 0.0 {
                let nv = if scroll > 0.0 {
                    (v + 1).min(3)
                } else {
                    v.saturating_sub(1)
                };
                cells.set_code(pat, row, step, make_code(nv, c));
                first_gesture = true;
            }
        }
        let (v, c) = levels(code.max(1));
        let tip = if code == 0 {
            format!(
                "{} step {}: off. Click or drag to write the brush; shift-click sets the row length.",
                GRID_ROW_NAMES[row],
                step + 1
            )
        } else {
            format!(
                "{} step {}: velocity {:.0} %, chance {} %. Scroll for velocity, right-click for chance.",
                GRID_ROW_NAMES[row],
                step + 1,
                VELOCITY[v] * 100.0,
                CHANCE[c]
            )
        };
        resp.clone().on_hover_text(tip);
    }

    // Paint.
    let live = playing_step_rows(editor, ui_state, &cells);
    for row in 0..GRID_ROWS {
        let hue = row_colour(row, th.dark);
        let len = cells.length(row);
        let label_c = xf.p(pos2(r.left() + 6.0, cell_rect(row, 0).center().y));
        painter.circle_filled(label_c, 3.0 * z, hue);
        if small {
            painter.text(
                label_c + vec2(8.0 * z, 0.0),
                Align2::LEFT_CENTER,
                GRID_ROW_NAMES[row],
                font(10.5),
                th.ink,
            );
        }
        for step in 0..GRID_STEPS {
            let rect = xf.r(cell_rect(row, step));
            let beat = step % 4 == 0;
            let live_len = step < len;
            let code = if live_len {
                cells.code(pat, row, step)
            } else {
                0
            };
            let base = if th.dark {
                lerp(th.display, th.ink, if beat { 0.10 } else { 0.05 })
            } else {
                lerp(th.display, th.ink, if beat { 0.14 } else { 0.07 })
            };
            let base = if live_len { base } else { with_alpha(base, 70) };
            painter.rect_filled(rect, CornerRadius::same(2), base);
            if code != 0 {
                let (v, c) = levels(code);
                let h = rect.height() * (0.3 + 0.7 * VELOCITY[v]);
                let fill = Rect::from_min_max(pos2(rect.left(), rect.bottom() - h), rect.max);
                painter.rect_filled(
                    fill,
                    CornerRadius::same(2),
                    lerp(base, hue, 0.45 + 0.55 * VELOCITY[v]),
                );
                // Chance: a bar along the top, full at 100 %.
                let bar = Rect::from_min_size(
                    rect.min,
                    vec2(rect.width() * CHANCE[c] as f32 / 100.0, (2.0 * z).max(1.0)),
                );
                painter.rect_filled(bar, CornerRadius::ZERO, th.ink);
            }
            if live[row] == Some(step) && live_len {
                painter.rect_stroke(
                    rect.expand(0.5),
                    CornerRadius::same(2),
                    Stroke::new((1.6 * z).max(1.0), th.ink),
                    StrokeKind::Inside,
                );
            }
        }
    }

    // Apply this frame's edits.
    let mut order: Vec<_> = cells.pending.iter().map(|(&i, &v)| (i, v)).collect();
    order.sort_unstable_by_key(|e| e.0);
    drop(cells);
    for (k, (i, v)) in order.into_iter().enumerate() {
        editor.set_param_gesture(
            ParamTarget::Module {
                id,
                param: DRUM_GRID_INFO.params[i].name.to_string(),
            },
            v,
            first_gesture && k == 0,
        );
    }
    if let Some(p) = set_pattern {
        editor.set_param_gesture(
            ParamTarget::Module {
                id,
                param: DRUM_GRID_INFO.params[pattern_index()].name.to_string(),
            },
            p as f32,
            true,
        );
    }
}

/// The step each row is on, while the reference clock runs.
fn playing_step_rows(
    editor: &PatchEditor,
    ui_state: &UiState,
    cells: &Cells,
) -> [Option<usize>; GRID_ROWS] {
    let mut out = [None; GRID_ROWS];
    for (row, o) in out.iter_mut().enumerate() {
        *o = playing_step(editor, ui_state, cells.length(row));
    }
    out
}
