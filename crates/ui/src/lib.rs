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
use kabl_modules::info::{Category, PortDirection, PortType};
use kabl_modules::registry;

const MODULE_WIDTH: f32 = 160.0;
const PORT_ROW_HEIGHT: f32 = 18.0;
const HEADER_HEIGHT: f32 = 26.0;
const PORT_RADIUS: f32 = 5.0;
const ACCENT_HEIGHT: f32 = 4.0;
const KNOB_RADIUS: f32 = 11.0;
const KNOB_ROW_HEIGHT: f32 = 40.0;
const PANEL_FILL: Color32 = Color32::from_rgb(32, 32, 36);
const PANEL_FILL_SELECTED: Color32 = Color32::from_rgb(44, 48, 58);
/// Rack grid cell size in pixels — `Eurorack` view only. A module's `ModuleInfo::width_units`
/// times this is its panel width; row/column snapping rounds to multiples of this.
const UNIT_PX: f32 = 20.0;
/// Headroom past the furthest-out module for the scroll area, plus room to drag one further.
const CANVAS_MARGIN: f32 = 200.0;

/// Two ways to look at the same patch (same `PatchEditor`, same edits) — not two editors. The
/// free-form patchbay is better for building/rearranging from scratch (owner's own framing);
/// the tiled Eurorack view snaps modules to a rack-style grid with real per-module widths, closer
/// to playing an instrument than constructing one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Patchbay,
    Eurorack,
}

/// A module's `Category` (brief section 8's grouping) doubles as its panel's accent color — the
/// same idea real modular hardware uses (Make Noise/Mutable-style panel-color-by-function), so a
/// patch reads at a glance instead of every module being an identical gray box.
fn category_color(category: Category) -> Color32 {
    match category {
        Category::Source => Color32::from_rgb(230, 140, 60),
        Category::Filter => Color32::from_rgb(60, 180, 200),
        Category::Modulator => Color32::from_rgb(170, 100, 220),
        Category::Sequencer => Color32::from_rgb(120, 200, 90),
        Category::Utility => Color32::from_rgb(140, 140, 155),
        Category::Effect => Color32::from_rgb(220, 90, 150),
    }
}

/// A port's `PortType` doubles as its jack color — common Eurorack convention (audio vs. CV vs.
/// gate vs. pitch reading as distinct at a glance, not just by hovering to read a tooltip).
fn port_type_color(port_type: PortType) -> Color32 {
    match port_type {
        PortType::Audio => Color32::from_rgb(235, 235, 235),
        PortType::Cv | PortType::UnipolarCv => Color32::from_rgb(90, 160, 255),
        PortType::Gate => Color32::from_rgb(255, 190, 60),
        PortType::Pitch => Color32::from_rgb(110, 220, 150),
    }
}

/// A fixed palette cables cycle through by `CableId`, so a patch with several cables reads as
/// actually patched (distinguishable cables) rather than one flat color for every connection.
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

fn cable_color(id: CableId) -> Color32 {
    CABLE_COLORS[(id as usize) % CABLE_COLORS.len()]
}

/// Interaction state that persists across frames but isn't part of the patch itself — which
/// module is selected (for the param panel), an in-progress cable connection, an in-progress
/// module drag, which kind is selected in the "add module" palette, and the Save/Load path field.
pub struct UiState {
    pub selected_kind: String,
    pub selected_module: Option<ModuleId>,
    pub view_mode: ViewMode,
    pending_output: Option<PortRef>,
    dragging: Option<Dragging>,
    /// Directory `kabl_core::save`/`load` read and write — a plain text field rather than a
    /// native file-picker dependency (`rfd` and friends), which would be unverifiable in this
    /// container anyway (no display server to test a picker dialog against) and isn't needed for
    /// the underlying save/load logic to be real and correct.
    pub patch_path: String,
    pub last_message: Option<String>,
    /// Decoded skin background textures, keyed by module kind (one skin per kind, see
    /// `kabl_modules::skin`) so a module's art is decoded once and reused every frame, not
    /// re-decoded per instance per frame. `TextureHandle` is ref-counted and frees its GPU texture
    /// on drop, so this cache is also what keeps a loaded skin's texture alive.
    image_cache: std::collections::HashMap<&'static str, egui::TextureHandle>,
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
            // Eurorack is the priority view (owner: better for playing, not building from
            // scratch) — defaults on; Patchbay stays a click away for editing.
            view_mode: ViewMode::Eurorack,
            pending_output: None,
            dragging: None,
            patch_path: "my-patch".to_string(),
            last_message: None,
            image_cache: std::collections::HashMap::new(),
        }
    }
}

/// Decodes `png_bytes` (a skin's embedded panel art) into an `egui` texture the first time `kind`
/// is seen, and reuses the cached handle on every later call — decoding a PNG is real work, not
/// something to repeat every frame for every instance of a skinned module.
fn skin_texture(
    ui: &egui::Ui,
    cache: &mut std::collections::HashMap<&'static str, egui::TextureHandle>,
    kind: &'static str,
    png_bytes: &'static [u8],
) -> egui::TextureId {
    let handle = cache.entry(kind).or_insert_with(|| {
        let decoded = image::load_from_memory(png_bytes)
            .expect("skin background image must be a valid, embedded PNG")
            .to_rgba8();
        let size = [decoded.width() as usize, decoded.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, decoded.as_raw());
        ui.ctx()
            .load_texture(kind, color_image, egui::TextureOptions::LINEAR)
    });
    handle.id()
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
            ui.separator();
            ui.selectable_value(&mut ui_state.view_mode, ViewMode::Eurorack, "Eurorack");
            ui.selectable_value(&mut ui_state.view_mode, ViewMode::Patchbay, "Patchbay");
        });
        ui.horizontal(|ui| {
            ui.label("patch dir:");
            ui.text_edit_singleline(&mut ui_state.patch_path);
            if ui.button("Save").clicked() {
                let path = std::path::Path::new(&ui_state.patch_path);
                ui_state.last_message = Some(match kabl_core::save(path, editor.log()) {
                    Ok(()) => format!("saved to {}", path.display()),
                    Err(err) => format!("save failed: {err:?}"),
                });
            }
            if ui.button("Load").clicked() {
                let path = std::path::Path::new(&ui_state.patch_path);
                match kabl_core::load(path) {
                    Ok(log) => {
                        *editor = PatchEditor::from_log(log);
                        // `from_log` starts clean (right for the initial startup seed, whose
                        // caller compiles the seeded patch directly) -- but a Load here replaces
                        // a *live* patch, and the audio host only ever rebuilds by checking
                        // `take_dirty()`, so this needs to mark it explicitly or the running
                        // graph would silently keep playing the old patch.
                        editor.mark_dirty();
                        ui_state.selected_module = None;
                        ui_state.pending_output = None;
                        ui_state.last_message = Some(format!("loaded {}", path.display()));
                    }
                    Err(err) => {
                        ui_state.last_message = Some(format!("load failed: {err:?}"));
                    }
                }
            }
            if let Some(msg) = &ui_state.last_message {
                ui.label(msg);
            }
        });
    });

    egui::Panel::right("kabl-params").show(ui, |ui| {
        show_param_panel(editor, ui_state, ui);
    });

    egui::CentralPanel::default().show(ui, |ui| {
        let available = ui.available_size();
        let content_size = canvas_content_size(editor).max(available);
        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            ui.set_min_size(content_size);
            show_canvas(editor, ui_state, ui);
        });
    });
}

/// Bounding box over every module's position + a generous margin, so the scroll area knows how
/// far there is to scroll — a patch (or a wide rack in Eurorack view) bigger than the window used
/// to just get clipped with no way to reach the rest of it.
fn canvas_content_size(editor: &PatchEditor) -> EguiVec2 {
    let mut max_x = 0.0f32;
    let mut max_y = 0.0f32;
    for m in editor.state().modules.values() {
        let width = registry::info_for(&m.kind).map_or(MODULE_WIDTH, |i| {
            (i.width_units as f32 * UNIT_PX).max(MODULE_WIDTH)
        });
        max_x = max_x.max(m.pos.x + width);
        max_y = max_y.max(m.pos.y + 300.0); // generous fixed bound, exact height doesn't matter here
    }
    EguiVec2::new(max_x + CANVAS_MARGIN, max_y + CANVAS_MARGIN)
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
    let eurorack = ui_state.view_mode == ViewMode::Eurorack;

    if eurorack {
        draw_rack_grid(&painter, ui.clip_rect(), origin);
    }

    // Snapshot layout data before mutating `editor` mid-frame (immediate-mode + a shared
    // PatchLog don't mix well otherwise: we'd need `editor` borrowed both immutably, for
    // reading positions/ports while drawing, and mutably, for applying an edit a click just
    // triggered, at the same time). Collecting positions once per frame is cheap (this is a UI
    // frame, not the audio thread) and keeps the borrow simple.
    let modules: Vec<(
        ModuleId,
        String,
        Vec2,
        std::collections::BTreeMap<String, f32>,
    )> = editor
        .state()
        .modules
        .iter()
        .map(|(&id, m)| (id, m.kind.clone(), m.pos, m.params.clone()))
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

    for (id, kind, stored_pos, params) in &modules {
        let mut pos = match &ui_state.dragging {
            Some(d) if d.id == *id => d.live_pos,
            _ => *stored_pos,
        };
        // Eurorack only: snap to the rack grid for display *and* interaction. The underlying
        // `ModuleState.pos` this commits on drag-release is the snapped value too (real per-cell
        // placement, not just a visual overlay) -- but only while this view is active; Patchbay
        // stays free-form on the same field, unaffected when the owner switches back to it.
        if eurorack {
            pos = eurorack_snap(pos);
        }
        let info = registry::info_for(kind);

        if let Some(skin) = info.and_then(|i| i.skin) {
            draw_skinned_module(
                editor,
                ui_state,
                ui,
                &painter,
                &mut port_pos,
                *id,
                info.expect("skin implies info is Some"),
                skin,
                pos,
                *stored_pos,
                params,
                origin,
            );
            continue;
        }

        let inputs: Vec<&kabl_modules::info::PortInfo> = info
            .map(|i| {
                i.ports
                    .iter()
                    .filter(|p| p.direction == PortDirection::Input)
                    .collect()
            })
            .unwrap_or_default();
        let outputs: Vec<&kabl_modules::info::PortInfo> = info
            .map(|i| {
                i.ports
                    .iter()
                    .filter(|p| p.direction == PortDirection::Output)
                    .collect()
            })
            .unwrap_or_default();
        let port_rows = inputs.len().max(outputs.len()).max(1) as f32;
        let has_knobs = info.is_some_and(|i| !i.params.is_empty());
        let body_height = HEADER_HEIGHT
            + port_rows * PORT_ROW_HEIGHT
            + if has_knobs { KNOB_ROW_HEIGHT } else { 0.0 };
        // Eurorack: real per-module rack width (`width_units`, snapped to the grid cell size).
        // Patchbay: the same uniform width every module has always used there.
        let module_width = if eurorack {
            info.map_or(MODULE_WIDTH, |i| i.width_units as f32 * UNIT_PX)
        } else {
            MODULE_WIDTH
        };
        let size = EguiVec2::new(module_width, body_height);
        let rect = Rect::from_min_size(origin + EguiVec2::new(pos.x, pos.y), size);

        let selected = interact_body(ui, editor, ui_state, *id, *stored_pos, rect);
        let accent = category_color(info.map(|i| i.category).unwrap_or(Category::Utility));
        let fill = if selected {
            PANEL_FILL_SELECTED
        } else {
            PANEL_FILL
        };
        // Faceplate: dark body, a category-colored accent strip along the top (the "which kind
        // of module is this, at a glance" cue real modular panels use color for), a brighter
        // outline when selected instead of a flat gray one always.
        painter.rect_filled(rect, 5.0, fill);
        let accent_rect = Rect::from_min_size(rect.min, EguiVec2::new(module_width, ACCENT_HEIGHT));
        painter.rect_filled(
            accent_rect,
            egui::CornerRadius {
                nw: 5,
                ne: 5,
                sw: 0,
                se: 0,
            },
            accent,
        );
        painter.rect_stroke(
            rect,
            5.0,
            Stroke::new(
                if selected { 2.0 } else { 1.0 },
                if selected {
                    accent
                } else {
                    Color32::from_gray(90)
                },
            ),
            egui::StrokeKind::Outside,
        );
        painter.text(
            rect.min + EguiVec2::new(6.0, ACCENT_HEIGHT + 3.0),
            egui::Align2::LEFT_TOP,
            format!("{kind} #{id}"),
            egui::FontId::proportional(12.0),
            Color32::WHITE,
        );

        for (row, port) in inputs.iter().enumerate() {
            let p = rect.min
                + EguiVec2::new(
                    0.0,
                    HEADER_HEIGHT + row as f32 * PORT_ROW_HEIGHT + PORT_ROW_HEIGHT / 2.0,
                );
            port_pos.insert((*id, PortDirection::Input, port.name.to_string()), p);
            draw_port(&painter, ui, p, port, PortDirection::Input, |pt| {
                on_port_click(editor, ui_state, *id, PortDirection::Input, port.name, pt)
            });
        }
        for (row, port) in outputs.iter().enumerate() {
            let p = rect.min
                + EguiVec2::new(
                    module_width,
                    HEADER_HEIGHT + row as f32 * PORT_ROW_HEIGHT + PORT_ROW_HEIGHT / 2.0,
                );
            port_pos.insert((*id, PortDirection::Output, port.name.to_string()), p);
            draw_port(&painter, ui, p, port, PortDirection::Output, |pt| {
                on_port_click(editor, ui_state, *id, PortDirection::Output, port.name, pt)
            });
        }

        // On-panel knobs, one per param, in a row below the jacks -- brief's "specify where to
        // put ... switches/readouts" in miniature: today every built-in only has continuous
        // params, so `Knob` is the one control kind actually wired up (see `skin.rs`'s doc
        // comment for the fuller custom-panel design this is a first slice of).
        if let Some(info) = info.filter(|_| has_knobs) {
            let knob_y = rect.min.y + HEADER_HEIGHT + port_rows * PORT_ROW_HEIGHT
                - PORT_ROW_HEIGHT / 2.0
                + KNOB_ROW_HEIGHT / 2.0
                + 6.0;
            let n = info.params.len() as f32;
            for (i, param) in info.params.iter().enumerate() {
                let cx = rect.min.x + module_width * (i as f32 + 0.5) / n;
                let center = Pos2::new(cx, knob_y);
                let current = params.get(param.name).copied().unwrap_or(param.default);
                let frac = if param.max > param.min {
                    ((current - param.min) / (param.max - param.min)).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let knob_id = Id::new(("kabl-knob", *id, param.name));
                if let Some(delta_frac) = draw_knob(&painter, ui, center, frac, param.name, knob_id)
                {
                    let new_value = (current + delta_frac * (param.max - param.min))
                        .clamp(param.min, param.max);
                    if new_value != current {
                        editor.set_param(*id, param.name, new_value);
                    }
                }
            }
        }
    }

    for (cable_id, from, to) in &cables {
        let (Some(&a), Some(&b)) = (
            port_ref_pos(&port_pos, from, PortDirection::Output),
            port_ref_pos(&port_pos, to, PortDirection::Input),
        ) else {
            continue;
        };
        let color = cable_color(*cable_id);
        let points = cable_curve_points(a, b);
        painter.add(egui::Shape::line(points.clone(), Stroke::new(2.5, color)));
        // Small hit-target at the curve's own midpoint (not the straight-line one, now that
        // cables sag) to disconnect -- clicking the cable removes it.
        let mid = points[points.len() / 2];
        let hit = Rect::from_center_size(mid, EguiVec2::splat(12.0));
        let resp = ui.interact(hit, Id::new(("kabl-cable", *cable_id)), Sense::click());
        if resp.clicked() {
            editor.disconnect(*cable_id);
        }
        if resp.hovered() {
            painter.circle_stroke(mid, 7.0, Stroke::new(1.5, Color32::RED));
        }
    }
}

/// Samples a quadratic bezier from `a` to `b` with a single control point pulled downward from
/// the midpoint (a cheap stand-in for gravity droop) -- a hanging patch cable, not a ruler-straight
/// wire. `sag` scales with horizontal distance so a short cable barely dips and a long one hangs
/// visibly, clamped so it never gets silly on a very wide patch.
fn cable_curve_points(a: Pos2, b: Pos2) -> Vec<Pos2> {
    const SAMPLES: usize = 24;
    let sag = ((b.x - a.x).abs() * 0.18).clamp(8.0, 50.0);
    let control = Pos2::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0 + sag);
    (0..=SAMPLES)
        .map(|i| {
            let t = i as f32 / SAMPLES as f32;
            let mt = 1.0 - t;
            Pos2::new(
                mt * mt * a.x + 2.0 * mt * t * control.x + t * t * b.x,
                mt * mt * a.y + 2.0 * mt * t * control.y + t * t * b.y,
            )
        })
        .collect()
}

/// Rounds a position to the nearest rack grid cell — Eurorack view only. Applied to both the
/// stored and the live-drag position, so dragging itself feels snapped, not just the final rest
/// position.
fn eurorack_snap(pos: Vec2) -> Vec2 {
    Vec2 {
        x: (pos.x / UNIT_PX).round() * UNIT_PX,
        y: (pos.y / UNIT_PX).round() * UNIT_PX,
    }
}

/// Faint grid lines every `UNIT_PX`, covering only what's actually visible (`clip_rect`, which a
/// `ScrollArea` already keeps correct) — the "just that grid" background for Eurorack view, not a
/// photorealistic rack texture.
fn draw_rack_grid(painter: &egui::Painter, clip_rect: Rect, origin: Pos2) {
    let stroke = Stroke::new(1.0, Color32::from_rgb(48, 48, 54));
    let start_x = origin.x + ((clip_rect.min.x - origin.x) / UNIT_PX).floor() * UNIT_PX;
    let mut x = start_x;
    while x < clip_rect.max.x {
        painter.line_segment(
            [Pos2::new(x, clip_rect.min.y), Pos2::new(x, clip_rect.max.y)],
            stroke,
        );
        x += UNIT_PX;
    }
    let start_y = origin.y + ((clip_rect.min.y - origin.y) / UNIT_PX).floor() * UNIT_PX;
    let mut y = start_y;
    while y < clip_rect.max.y {
        painter.line_segment(
            [Pos2::new(clip_rect.min.x, y), Pos2::new(clip_rect.max.x, y)],
            stroke,
        );
        y += UNIT_PX;
    }
}

/// Selection + drag handling shared by both the auto-layout panel and a skinned one -- the same
/// logic either way, just parameterized on the panel's `rect` (which comes from a fixed
/// `MODULE_WIDTH`-based layout in one case and `ModuleSkin.panel_size` in the other). Returns
/// whether this module is the selected one, for the caller to use when drawing its own
/// selection-outline style.
fn interact_body(
    ui: &mut egui::Ui,
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    id: ModuleId,
    stored_pos: Vec2,
    rect: Rect,
) -> bool {
    let eurorack = ui_state.view_mode == ViewMode::Eurorack;
    let body_id = Id::new(("kabl-module-body", id));
    let response = ui.interact(rect, body_id, Sense::click_and_drag());
    if response.clicked() {
        ui_state.selected_module = Some(id);
    }
    if response.drag_started() {
        ui_state.dragging = Some(Dragging {
            id,
            start_pos: stored_pos,
            live_pos: stored_pos,
        });
    }
    if response.dragged() {
        if let Some(d) = ui_state.dragging.as_mut().filter(|d| d.id == id) {
            d.live_pos.x += response.drag_delta().x;
            d.live_pos.y += response.drag_delta().y;
        }
    }
    if response.drag_stopped() {
        if let Some(d) = ui_state.dragging.take().filter(|d| d.id == id) {
            // Committed position snaps too in Eurorack view, not just the on-screen rendering
            // while dragging -- real grid placement in the saved patch, not a visual-only
            // overlay on top of wherever the raw drag happened to end.
            let final_pos = if eurorack {
                eurorack_snap(d.live_pos)
            } else {
                d.live_pos
            };
            if final_pos != d.start_pos {
                editor.move_module(id, final_pos);
            }
        }
    }
    ui_state.selected_module == Some(id)
}

/// Renders a module that declares a `ModuleSkin` — custom panel art (if any) stretched to
/// `skin.panel_size`, with every control drawn at its explicitly declared normalized position
/// instead of the auto-layout row scheme `show_canvas`'s main loop uses for everything else. This
/// is the "custom modules can use their own images as their background and specify where to put
/// their jacks/ins/outs/switches/readouts" mechanism (owner ask) — see decisions.md "Module
/// skins: custom panel art".
#[allow(clippy::too_many_arguments)]
fn draw_skinned_module(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    port_pos: &mut std::collections::HashMap<(ModuleId, PortDirection, String), Pos2>,
    id: ModuleId,
    info: &'static kabl_modules::info::ModuleInfo,
    skin: &'static kabl_modules::skin::ModuleSkin,
    pos: Vec2,
    stored_pos: Vec2,
    params: &std::collections::BTreeMap<String, f32>,
    origin: Pos2,
) {
    let size = EguiVec2::new(skin.panel_size.0, skin.panel_size.1);
    let rect = Rect::from_min_size(origin + EguiVec2::new(pos.x, pos.y), size);

    let selected = interact_body(ui, editor, ui_state, id, stored_pos, rect);
    let accent = category_color(info.category);

    match skin.background_image {
        Some(png) => {
            let tex = skin_texture(ui, &mut ui_state.image_cache, info.kind, png);
            painter.image(
                tex,
                rect,
                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        None => {
            painter.rect_filled(
                rect,
                5.0,
                if selected {
                    PANEL_FILL_SELECTED
                } else {
                    PANEL_FILL
                },
            );
        }
    }
    // Selection feedback on top of either background: a plain rect_stroke would look identical
    // to the auto-layout panel's, but a skin's art may already fill right up to its own edge, so
    // draw it a hair outside the panel rather than risk it being covered.
    if selected {
        painter.rect_stroke(
            rect.expand(1.5),
            5.0,
            Stroke::new(2.0, accent),
            egui::StrokeKind::Outside,
        );
    }
    painter.text(
        rect.right_top() + EguiVec2::new(-4.0, 4.0),
        egui::Align2::RIGHT_TOP,
        format!("#{id}"),
        egui::FontId::proportional(10.0),
        Color32::from_gray(210),
    );

    for control in skin.controls {
        let center = rect.min
            + EguiVec2::new(
                control.pos.0 * skin.panel_size.0,
                control.pos.1 * skin.panel_size.1,
            );
        match control.kind {
            kabl_modules::skin::ControlKind::Jack => {
                let Some(port) = info.ports.iter().find(|p| p.name == control.id) else {
                    continue;
                };
                port_pos.insert((id, port.direction, port.name.to_string()), center);
                draw_port(painter, ui, center, port, port.direction, |pt| {
                    on_port_click(editor, ui_state, id, port.direction, port.name, pt)
                });
            }
            kabl_modules::skin::ControlKind::Knob => {
                let Some(param) = info.params.iter().find(|p| p.name == control.id) else {
                    continue;
                };
                let current = params.get(param.name).copied().unwrap_or(param.default);
                let frac = if param.max > param.min {
                    ((current - param.min) / (param.max - param.min)).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let knob_id = Id::new(("kabl-knob", id, param.name));
                if let Some(delta_frac) = draw_knob(painter, ui, center, frac, param.name, knob_id)
                {
                    let new_value = (current + delta_frac * (param.max - param.min))
                        .clamp(param.min, param.max);
                    if new_value != current {
                        editor.set_param(id, param.name, new_value);
                    }
                }
            }
            // Declared, not yet rendered -- see kabl_modules::skin's module doc.
            kabl_modules::skin::ControlKind::Switch | kabl_modules::skin::ControlKind::Readout => {}
        }
    }
}

fn port_ref_pos<'a>(
    port_pos: &'a std::collections::HashMap<(ModuleId, PortDirection, String), Pos2>,
    port_ref: &PortRef,
    direction: PortDirection,
) -> Option<&'a Pos2> {
    let PortRef::Module { id, port } = port_ref else {
        return None;
    };
    port_pos.get(&(*id, direction, port.clone()))
}

#[allow(clippy::too_many_arguments)]
fn draw_port(
    painter: &egui::Painter,
    ui: &mut egui::Ui,
    pos: Pos2,
    port: &kabl_modules::info::PortInfo,
    direction: PortDirection,
    on_click: impl FnOnce(Pos2),
) {
    let rect = Rect::from_center_size(pos, EguiVec2::splat(PORT_RADIUS * 2.5));
    let id = Id::new((
        "kabl-port",
        direction,
        port.name,
        pos.x as i32,
        pos.y as i32,
    ));
    let response = ui.interact(rect, id, Sense::click());
    let base = port_type_color(port.port_type);
    // Ring style (outer color ring, dark center) instead of a flat dot -- reads more like a real
    // 1/4"/3.5mm jack; hovering brightens the ring rather than swapping to an unrelated color, so
    // the port-type color stays legible even while highlighted.
    let ring = if response.hovered() {
        Color32::from_rgb(
            base.r().saturating_add(30),
            base.g().saturating_add(30),
            base.b().saturating_add(30),
        )
    } else {
        base
    };
    painter.circle_filled(pos, PORT_RADIUS, Color32::from_rgb(20, 20, 22));
    painter.circle_stroke(pos, PORT_RADIUS, Stroke::new(2.0, ring));
    let label_offset = match direction {
        PortDirection::Input => EguiVec2::new(PORT_RADIUS + 4.0, 0.0),
        PortDirection::Output => EguiVec2::new(-(PORT_RADIUS + 4.0), 0.0),
    };
    let align = match direction {
        PortDirection::Input => egui::Align2::LEFT_CENTER,
        PortDirection::Output => egui::Align2::RIGHT_CENTER,
    };
    painter.text(
        pos + label_offset,
        align,
        port.name,
        egui::FontId::proportional(9.0),
        Color32::from_gray(190),
    );
    if response.clicked() {
        on_click(pos);
    }
}

/// A draggable rotary knob: dark body, a ring in the module's accent-independent neutral color,
/// and a pointer swept 270° (from lower-left at `frac=0` to lower-right at `frac=1`, straight up
/// at `frac=0.5`) -- the same sweep convention real synth knobs use. Dragging vertically (up =
/// increase, matching a real knob's feel) returns `Some(delta_frac)` for the caller to scale by
/// the param's actual range and apply; `None` when not being dragged this frame.
fn draw_knob(
    painter: &egui::Painter,
    ui: &mut egui::Ui,
    center: Pos2,
    frac: f32,
    label: &str,
    id: Id,
) -> Option<f32> {
    const DRAG_PIXELS_FOR_FULL_SWEEP: f32 = 150.0;
    let rect = Rect::from_center_size(center, EguiVec2::splat(KNOB_RADIUS * 2.0));
    let response = ui.interact(rect, id, Sense::drag());
    let ring = if response.hovered() || response.dragged() {
        Color32::from_gray(230)
    } else {
        Color32::from_gray(170)
    };
    painter.circle_filled(center, KNOB_RADIUS, Color32::from_rgb(22, 22, 25));
    painter.circle_stroke(center, KNOB_RADIUS, Stroke::new(1.5, ring));
    let angle = (-135.0 + frac.clamp(0.0, 1.0) * 270.0).to_radians();
    let dir = EguiVec2::new(angle.sin(), -angle.cos());
    painter.line_segment(
        [center, center + dir * (KNOB_RADIUS * 0.75)],
        Stroke::new(2.0, Color32::WHITE),
    );
    painter.text(
        center + EguiVec2::new(0.0, KNOB_RADIUS + 9.0),
        egui::Align2::CENTER_TOP,
        label,
        egui::FontId::proportional(8.0),
        Color32::from_gray(170),
    );
    response
        .dragged()
        .then(|| -response.drag_delta().y / DRAG_PIXELS_FOR_FULL_SWEEP)
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

#[cfg(test)]
mod eurorack_grid_tests {
    use super::*;

    #[test]
    fn snap_rounds_to_the_nearest_grid_cell() {
        assert_eq!(
            eurorack_snap(Vec2 { x: 9.0, y: 11.0 }),
            Vec2 { x: 0.0, y: 20.0 }
        );
        assert_eq!(
            eurorack_snap(Vec2 { x: 213.0, y: 4.0 }),
            Vec2 { x: 220.0, y: 0.0 }
        );
        assert_eq!(
            eurorack_snap(Vec2 { x: -5.0, y: -16.0 }),
            Vec2 { x: 0.0, y: -20.0 }
        );
    }

    #[test]
    fn snap_is_idempotent() {
        let once = eurorack_snap(Vec2 { x: 137.0, y: -48.0 });
        assert_eq!(eurorack_snap(once), once);
    }
}
