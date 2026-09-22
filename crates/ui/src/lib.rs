//! `kabl-ui`: the patchbay editor (brief section 12's `ui` crate). Split like `kabl-standalone`
//! into a hardware/display-independent core (`editor.rs`'s `PatchEditor` — patch-editing logic
//! over `kabl_core::PatchLog`, testable headlessly) and the `egui` rendering here. `main.rs`
//! wires the actual live-audio (cpal/midir) and windowing (eframe) — neither of which can be
//! verified in this container (no audio device, no display server) — see decisions.md.
//!
//! `show()` itself *is* testable without a display: `egui::Context` is pure Rust and only needs
//! `begin_pass`/`end_pass` (no window, no GPU) to drive a frame — see `tests/editor_and_ui.rs`'s
//! smoke tests for the pattern. What can't be tested here is whether it actually *looks* right on
//! screen or feels right to drag — that needs a real display, flagged not skipped.

pub mod editor;

pub use editor::PatchEditor;

use egui::{Color32, Id, Pos2, Rect, Sense, Stroke, Vec2 as EguiVec2};
use kabl_core::{CableId, ModuleId, PortRef, Vec2};
use kabl_modules::info::PortDirection;
use kabl_modules::registry;

const MODULE_WIDTH: f32 = 160.0;
const PORT_ROW_HEIGHT: f32 = 18.0;
const HEADER_HEIGHT: f32 = 24.0;
const PORT_RADIUS: f32 = 5.0;

/// Interaction state that persists across frames but isn't part of the patch itself — which
/// module is selected (for the param panel), an in-progress cable connection, an in-progress
/// module drag, and which kind is selected in the "add module" palette.
pub struct UiState {
    pub selected_kind: String,
    pub selected_module: Option<ModuleId>,
    pending_output: Option<PortRef>,
    dragging: Option<Dragging>,
}

struct Dragging {
    id: ModuleId,
    /// Position at drag start, before any delta was applied — `MoveModule` only gets appended
    /// once, on release, with the final position; see `editor.rs`'s doc comment on why moves
    /// aren't committed per-frame.
    start_pos: Vec2,
    live_pos: Vec2,
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
            dragging: None,
        }
    }
}

/// Draws the whole patchbay for one frame and applies any user-triggered edits to `editor`
/// directly (immediate-mode: the UI *is* the edit trigger, there's no separate "apply" step the
/// caller needs to remember). Call once per frame, from `eframe::App::ui` — this version of
/// `egui`/`eframe` hands the app a root `&mut Ui`, not a `&Context`, so panels nest inside it via
/// `show_inside` rather than the older `Panel::show(ctx, ...)` pattern.
pub fn show(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui) {
    egui::Panel::top("kabl-toolbar").show(ui, |ui| {
        ui.horizontal(|ui| {
            egui::ComboBox::from_label("kind")
                .selected_text(ui_state.selected_kind.clone())
                .show_ui(ui, |ui| {
                    for kind in PatchEditor::known_kinds() {
                        ui.selectable_value(&mut ui_state.selected_kind, kind.to_string(), *kind);
                    }
                });
            if ui.button("Add module").clicked() {
                // Cascade so repeated adds don't all land exactly on top of each other -- easy
                // to still overlap an existing module you've dragged elsewhere, but at least
                // successive additions are never a literal stack. Modulo keeps it from
                // wandering off canvas after many adds.
                let n = (editor.state().modules.len() % 10) as f32;
                let pos = Vec2 {
                    x: 40.0 + n * 24.0,
                    y: 40.0 + n * 24.0,
                };
                editor.add_module(&ui_state.selected_kind, pos);
            }
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
            if ui_state.pending_output.is_some() {
                ui.label(
                    "Click an input port to connect, or click the output port again to cancel.",
                );
            }
        });
    });

    egui::Panel::right("kabl-params").show(ui, |ui| {
        show_param_panel(editor, ui_state, ui);
    });

    egui::CentralPanel::default().show(ui, |ui| {
        show_canvas(editor, ui_state, ui);
    });
}

fn show_param_panel(editor: &mut PatchEditor, ui_state: &UiState, ui: &mut egui::Ui) {
    let Some(id) = ui_state.selected_module else {
        ui.label("No module selected.");
        return;
    };
    // Snapshot before mutating `editor` below -- same reasoning as `show_canvas`'s snapshot.
    let Some((kind, params)) = editor
        .state()
        .modules
        .get(&id)
        .map(|m| (m.kind.clone(), m.params.clone()))
    else {
        ui.label("No module selected.");
        return;
    };
    ui.heading(format!("{kind} (#{id})"));
    let Some(info) = registry::info_for(&kind) else {
        ui.label("Unknown module kind.");
        return;
    };
    for param in info.params {
        let current = params.get(param.name).copied().unwrap_or(param.default);
        let mut value = current;
        ui.add(
            egui::Slider::new(&mut value, param.min..=param.max)
                .text(param.name)
                .logarithmic(matches!(
                    param.taper,
                    kabl_modules::info::Taper::Exponential
                )),
        );
        if value != current {
            editor.set_param(id, param.name, value);
        }
    }
    if ui.button("Remove module").clicked() {
        editor.remove_module(id);
    }
}

fn show_canvas(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui) {
    let origin = ui.min_rect().min;
    let painter = ui.painter().clone();

    // Snapshot layout data before mutating `editor` mid-frame (immediate-mode + a shared
    // PatchLog don't mix well otherwise: we'd need `editor` borrowed both immutably, for
    // reading positions/ports while drawing, and mutably, for applying an edit a click just
    // triggered, at the same time). Collecting positions once per frame is cheap (this is a UI
    // frame, not the audio thread) and keeps the borrow simple.
    let modules: Vec<(ModuleId, String, Vec2)> = editor
        .state()
        .modules
        .iter()
        .map(|(&id, m)| (id, m.kind.clone(), m.pos))
        .collect();
    let cables: Vec<(CableId, PortRef, PortRef)> = editor
        .state()
        .cables
        .iter()
        .map(|(&id, c)| (id, c.from.clone(), c.to.clone()))
        .collect();

    // Port screen positions this frame, keyed by (module id, direction, port name) -- needed
    // both to draw cables and to hit-test port clicks.
    let mut port_pos: std::collections::HashMap<(ModuleId, PortDirection, String), Pos2> =
        std::collections::HashMap::new();

    for (id, kind, stored_pos) in &modules {
        let pos = match &ui_state.dragging {
            Some(d) if d.id == *id => d.live_pos,
            _ => *stored_pos,
        };
        let info = registry::info_for(kind);
        let inputs: Vec<&str> = info
            .map(|i| {
                i.ports
                    .iter()
                    .filter(|p| p.direction == PortDirection::Input)
                    .map(|p| p.name)
                    .collect()
            })
            .unwrap_or_default();
        let outputs: Vec<&str> = info
            .map(|i| {
                i.ports
                    .iter()
                    .filter(|p| p.direction == PortDirection::Output)
                    .map(|p| p.name)
                    .collect()
            })
            .unwrap_or_default();
        let port_rows = inputs.len().max(outputs.len()).max(1) as f32;
        let size = EguiVec2::new(MODULE_WIDTH, HEADER_HEIGHT + port_rows * PORT_ROW_HEIGHT);
        let rect = Rect::from_min_size(origin + EguiVec2::new(pos.x, pos.y), size);

        let body_id = Id::new(("kabl-module-body", *id));
        let response = ui.interact(rect, body_id, Sense::click_and_drag());
        if response.clicked() {
            ui_state.selected_module = Some(*id);
        }
        if response.drag_started() {
            ui_state.dragging = Some(Dragging {
                id: *id,
                start_pos: *stored_pos,
                live_pos: *stored_pos,
            });
        }
        if response.dragged() {
            if let Some(d) = ui_state.dragging.as_mut().filter(|d| d.id == *id) {
                d.live_pos.x += response.drag_delta().x;
                d.live_pos.y += response.drag_delta().y;
            }
        }
        if response.drag_stopped() {
            if let Some(d) = ui_state.dragging.take().filter(|d| d.id == *id) {
                if d.live_pos != d.start_pos {
                    editor.move_module(*id, d.live_pos);
                }
            }
        }

        let selected = ui_state.selected_module == Some(*id);
        let fill = if selected {
            Color32::from_rgb(70, 90, 120)
        } else {
            Color32::from_rgb(50, 50, 55)
        };
        painter.rect_filled(rect, 4.0, fill);
        painter.rect_stroke(
            rect,
            4.0,
            Stroke::new(1.0, Color32::from_gray(200)),
            egui::StrokeKind::Outside,
        );
        painter.text(
            rect.min + EguiVec2::new(6.0, 4.0),
            egui::Align2::LEFT_TOP,
            format!("{kind} #{id}"),
            egui::FontId::proportional(12.0),
            Color32::WHITE,
        );

        for (row, name) in inputs.iter().enumerate() {
            let p = rect.min
                + EguiVec2::new(
                    0.0,
                    HEADER_HEIGHT + row as f32 * PORT_ROW_HEIGHT + PORT_ROW_HEIGHT / 2.0,
                );
            port_pos.insert((*id, PortDirection::Input, name.to_string()), p);
            draw_port(&painter, ui, p, name, PortDirection::Input, |pt| {
                on_port_click(editor, ui_state, *id, PortDirection::Input, name, pt)
            });
        }
        for (row, name) in outputs.iter().enumerate() {
            let p = rect.min
                + EguiVec2::new(
                    MODULE_WIDTH,
                    HEADER_HEIGHT + row as f32 * PORT_ROW_HEIGHT + PORT_ROW_HEIGHT / 2.0,
                );
            port_pos.insert((*id, PortDirection::Output, name.to_string()), p);
            draw_port(&painter, ui, p, name, PortDirection::Output, |pt| {
                on_port_click(editor, ui_state, *id, PortDirection::Output, name, pt)
            });
        }
    }

    for (cable_id, from, to) in &cables {
        let (Some(&a), Some(&b)) = (
            port_ref_pos(&port_pos, from, PortDirection::Output),
            port_ref_pos(&port_pos, to, PortDirection::Input),
        ) else {
            continue;
        };
        painter.line_segment([a, b], Stroke::new(2.0, Color32::from_rgb(200, 180, 80)));
        // Small midpoint hit-target to disconnect -- clicking the cable removes it. A dedicated
        // "delete" affordance beats needing a whole separate selection mode for cables.
        let mid = a + (b - a) * 0.5;
        let hit = Rect::from_center_size(mid, EguiVec2::splat(10.0));
        let resp = ui.interact(hit, Id::new(("kabl-cable", *cable_id)), Sense::click());
        if resp.clicked() {
            editor.disconnect(*cable_id);
        }
        if resp.hovered() {
            painter.circle_stroke(mid, 6.0, Stroke::new(1.5, Color32::RED));
        }
    }
}

fn port_ref_pos<'a>(
    port_pos: &'a std::collections::HashMap<(ModuleId, PortDirection, String), Pos2>,
    port_ref: &PortRef,
    direction: PortDirection,
) -> Option<&'a Pos2> {
    let PortRef::Module { id, port } = port_ref;
    port_pos.get(&(*id, direction, port.clone()))
}

#[allow(clippy::too_many_arguments)]
fn draw_port(
    painter: &egui::Painter,
    ui: &mut egui::Ui,
    pos: Pos2,
    name: &str,
    direction: PortDirection,
    on_click: impl FnOnce(Pos2),
) {
    let rect = Rect::from_center_size(pos, EguiVec2::splat(PORT_RADIUS * 2.5));
    let id = Id::new(("kabl-port", direction, name, pos.x as i32, pos.y as i32));
    let response = ui.interact(rect, id, Sense::click());
    let color = if response.hovered() {
        Color32::YELLOW
    } else {
        Color32::LIGHT_BLUE
    };
    painter.circle_filled(pos, PORT_RADIUS, color);
    if response.clicked() {
        on_click(pos);
    }
}

fn on_port_click(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    id: ModuleId,
    direction: PortDirection,
    name: &str,
    _pos: Pos2,
) {
    let this_ref = PortRef::Module {
        id,
        port: name.to_string(),
    };
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
