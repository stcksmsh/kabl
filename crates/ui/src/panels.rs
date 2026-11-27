//! Composite face authoring and asset preparation. Called only on UI/control threads.
use crate::wheel::OwnedScroll;
use crate::{composites, PatchEditor, UiState};
use kabl_core::{
    panel::{Artwork, Kind, Panel, Placement, Source},
    Composite, CompositeId, ModuleId, PatchState, PortRef,
};
use kabl_modules::{registry, Taper};
use std::{collections::BTreeMap, path::Path};

pub fn decode_art(art: &Artwork) -> Result<image::RgbaImage, String> {
    art.validate()?;
    image::load_from_memory_with_format(&art.png, image::ImageFormat::Png)
        .map(|i| i.to_rgba8())
        .map_err(|e| format!("Invalid PNG: {e}"))
}
pub fn validate(state: &kabl_core::PatchState) -> Result<(), String> {
    for c in state.composites.values() {
        if let Some(panel) = &c.panel {
            check(state, c, panel)?;
            let icons = panel.placements.values().flat_map(|p| &p.icons);
            for art in [&panel.light, &panel.dark]
                .into_iter()
                .flatten()
                .chain(icons)
            {
                decode_art(art)?;
            }
        }
    }
    Ok(())
}
/// `Panel::validate` and the UI's kind rules for `panel` as the face of `c`. Artwork is
/// header-checked only, not decoded.
pub fn check(state: &kabl_core::PatchState, c: &Composite, panel: &Panel) -> Result<(), String> {
    panel.validate(c)?;
    for (&id, p) in &panel.placements {
        if p.kind == Kind::Display {
            // `Panel::validate` has checked the source is a module without an action.
            if let Some(Source::Module { id: leaf, .. }) = p.source {
                let m = state.modules.get(&leaf).ok_or_else(|| {
                    format!("Binding {id}: the display's module {leaf} is missing")
                })?;
                if !crate::displays::has_display(&m.kind) {
                    return Err(format!("Binding {id}: {} modules have no display", m.kind));
                }
            }
        }
        if let Some((module, min)) = block(p.kind) {
            // `Panel::validate` has checked the source is a module without an action.
            if let Some(Source::Module { id: leaf, .. }) = p.source {
                let m = state
                    .modules
                    .get(&leaf)
                    .ok_or_else(|| format!("Binding {id}: the block's module {leaf} is missing"))?;
                if m.kind != module {
                    return Err(format!(
                        "Binding {id}: {:?} blocks need a {module} module",
                        p.kind
                    ));
                }
            }
            // The module face's drawing has fixed parts that would spill over a neighbour.
            if p.width < min.0 || p.height < min.1 {
                return Err(format!(
                    "Binding {id}: {:?} blocks need at least {} x {} points",
                    p.kind, min.0, min.1
                ));
            }
        }
        if p.kind == Kind::Button {
            // `Panel::validate` has checked the source shape and `composite::validate` that a
            // face target is a nested button.
            if let Some(Source::Module {
                id: leaf,
                action: Some(action),
            }) = &p.source
            {
                let m = state.modules.get(leaf).ok_or_else(|| {
                    format!("Binding {id}: the button's module {leaf} is missing")
                })?;
                if !offers(&m.kind, action) {
                    return Err(format!(
                        "Binding {id}: {} modules have no {action} button",
                        m.kind
                    ));
                }
            }
        }
        if let Some(e) = c.controls.get(&id) {
            let target = param_info(state, &e.target)?;
            if matches!(e.target, PortRef::CableParam { .. }) && p.kind != Kind::Knob {
                return Err(format!("Binding {id}: a route's amount is drawn as a knob"));
            }
            let stepped = target.taper == Taper::Stepped;
            let options = (target.max - target.min).round() as usize + 1;
            match p.kind {
                Kind::Toggle => {
                    if !stepped || target.max - target.min != 1.0 {
                        return Err(format!(
                            "Binding {id}: toggles need a stepped parameter with exactly two options"
                        ));
                    }
                }
                Kind::Icons => {
                    if !stepped {
                        return Err(format!(
                            "Binding {id}: icon selectors need a stepped parameter"
                        ));
                    }
                    if p.icons.is_empty() && !glyphs(state, &e.target) {
                        return Err(format!(
                            "Binding {id}: no built-in icons for this parameter; import a PNG per option"
                        ));
                    }
                    if !p.icons.is_empty() && p.icons.len() != options {
                        return Err(format!(
                            "Binding {id}: {} icons for {options} options",
                            p.icons.len()
                        ));
                    }
                }
                k => {
                    if (k == Kind::Selector) != stepped {
                        return Err(format!("Binding {id}: stepped parameters need selectors; continuous parameters need knobs or sliders"));
                    }
                }
            }
            if matches!(p.kind, Kind::Selector | Kind::Icons) && p.width < options as f32 * 18.0 {
                return Err(format!(
                    "Binding {id}: selector needs at least 18 points per choice"
                ));
            }
        }
    }
    Ok(())
}
/// The module kind a whole-block element shows and the least footprint its drawing needs.
pub(crate) fn block(kind: Kind) -> Option<(&'static str, (f32, f32))> {
    match kind {
        // Two rows of 50-point pads and the status line under them.
        Kind::CuePads => Some(("cues", (240.0, 150.0))),
        Kind::Banks => Some(("seq", (crate::BANKS_W, 18.0))),
        Kind::Transport => Some(("clock", (120.0, 32.0))),
        _ => None,
    }
}
/// Whether the parameter `target` names has built-in icon glyphs.
pub(crate) fn glyphs(state: &kabl_core::PatchState, target: &PortRef) -> bool {
    let PortRef::Param { id, param } = target else {
        return false;
    };
    state
        .modules
        .get(id)
        .is_some_and(|m| crate::displays::has_glyphs(&m.kind, param))
}
/// Whether `kind` modules have the button `action` (`perform::button_actions`). A cue button is
/// checked by number only: deleting the cue leaves a button that says "no such cue" when pressed
/// instead of a patch whose composites can no longer be edited.
fn offers(kind: &str, action: &str) -> bool {
    let index = |prefix: &str, range: std::ops::Range<usize>| {
        action
            .strip_prefix(prefix)
            .and_then(|n| n.parse::<usize>().ok())
            .is_some_and(|i| range.contains(&i))
    };
    match kind {
        "cues" => action == "cancel" || index("cue.", 1..kabl_modules::builtins::CUES + 1),
        "seq" => index("bank.", 0..kabl_modules::builtins::seq::BANKS),
        "clock" => matches!(action, "run" | "restart"),
        _ => false,
    }
}
/// Where a button's chain of face targets ends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum End {
    /// A module action, fired as the module face fires it.
    Action(ModuleId, String),
    /// Snapshot `.1` of composite `.0`.
    Snapshot(CompositeId, u64),
}
/// Where a press of button `key` on composite `id`'s face ends, following face targets into
/// nested composites; None when a link of the chain is gone.
pub fn resolve(state: &PatchState, id: CompositeId, key: u64) -> Option<End> {
    chain(state, id, placement(state, id, key)?)
}
fn placement(state: &PatchState, id: CompositeId, key: u64) -> Option<&Placement> {
    state
        .composites
        .get(&id)?
        .panel
        .as_ref()?
        .placements
        .get(&key)
}
/// Where button `p` on composite `id`'s face ends. `p` need not be in the document: the design
/// page draws its draft's buttons.
fn chain<'a>(state: &'a PatchState, mut id: CompositeId, mut p: &'a Placement) -> Option<End> {
    // Every hop goes strictly deeper and nesting is at most 8, so a valid chain ends by then.
    for _ in 0..=8 {
        if p.kind != Kind::Button {
            return None;
        }
        match p.source.as_ref()? {
            Source::Module {
                id: leaf,
                action: Some(action),
            } => return Some(End::Action(*leaf, action.clone())),
            Source::Face { composite, key } => {
                id = *composite;
                p = placement(state, id, *key)?;
            }
            Source::Snapshot { id: snap } => {
                let c = state.composites.get(&id)?;
                return c
                    .snapshots
                    .iter()
                    .any(|s| s.id == *snap)
                    .then_some(End::Snapshot(id, *snap));
            }
            _ => return None,
        }
    }
    None
}
/// Presses button `key` on composite `id`'s face: the action its chain ends on, fired as a module
/// face fires it (host-clock refusal and cue timing included), or the snapshot it ends on,
/// recalled. A broken chain does nothing.
pub fn press(editor: &mut PatchEditor, view: &mut UiState, id: CompositeId, key: u64) {
    match resolve(editor.state(), id, key) {
        Some(End::Action(leaf, action)) => crate::perform::fire(editor, view, leaf, &action),
        Some(End::Snapshot(c, snap)) => {
            crate::snapshots::recall(editor, view, c, snap);
        }
        None => {}
    }
}
/// `(lit, queued)` of button `p` on composite `id`'s face: the state of the cue or bank its
/// chain ends on, as the module face shows it, or whether its snapshot was the last recalled.
pub fn lit(view: &UiState, state: &PatchState, id: CompositeId, p: &Placement) -> (bool, bool) {
    let (leaf, action) = match chain(state, id, p) {
        Some(End::Action(leaf, action)) => (leaf, action),
        Some(End::Snapshot(c, snap)) => return (view.last_snapshot.get(&c) == Some(&snap), false),
        None => return (false, false),
    };
    let bank_of = |s: ModuleId| {
        view.seq_banks
            .get(&s)
            .copied()
            .unwrap_or((crate::banks::startup(state, s), None))
    };
    match action.split_once('.') {
        Some(("cue", n)) => {
            let all = crate::cues::cues(state, leaf);
            let standing =
                crate::cues::standing_all(&all, &bank_of, view.last_cue.get(&leaf).copied());
            n.parse::<usize>()
                .ok()
                .and_then(|n| all.iter().position(|c| c.n == n))
                .map_or((false, false), |i| standing[i])
        }
        Some(("bank", b)) => {
            let b = b.parse::<usize>().ok();
            let (playing, queued) = bank_of(leaf);
            (b == Some(playing), b.is_some() && queued == b)
        }
        _ => (false, false),
    }
}
/// The name button `p` prints: its label, else the name of the button it targets, else its
/// action's name; empty when its chain is broken.
pub fn button_name(state: &PatchState, p: &Placement) -> String {
    name_in(state, None, p)
}
/// `button_name` of button `p` on composite `id`'s face, which also names a snapshot it ends on.
pub fn face_button_name(state: &PatchState, id: CompositeId, p: &Placement) -> String {
    name_in(state, Some(id), p)
}
fn name_in<'a>(state: &'a PatchState, mut id: Option<CompositeId>, mut p: &'a Placement) -> String {
    for _ in 0..=8 {
        if let Some(label) = &p.label {
            return label.clone();
        }
        match &p.source {
            Some(Source::Face { composite, key }) => match placement(state, *composite, *key) {
                Some(next) => {
                    id = Some(*composite);
                    p = next;
                }
                None => break,
            },
            Some(Source::Snapshot { id: snap }) => {
                return id
                    .and_then(|id| state.composites.get(&id))
                    .and_then(|c| c.snapshots.iter().find(|s| s.id == *snap))
                    .map(|s| s.name.clone())
                    .unwrap_or_default()
            }
            Some(Source::Module {
                id: leaf,
                action: Some(action),
            }) => {
                return crate::perform::button_actions(state, *leaf)
                    .into_iter()
                    .find_map(|(a, name)| (a == *action).then_some(name))
                    .unwrap_or_else(|| action.clone())
            }
            _ => break,
        }
    }
    String::new()
}
/// A route's signed amount as a control's parameter: the knob travel `set_route_amount` takes.
pub(crate) static ROUTE_AMOUNT: kabl_modules::ParamInfo = kabl_modules::ParamInfo {
    name: kabl_engine::compile::DEPTH,
    min: -1.0,
    max: 1.0,
    default: kabl_engine::compile::DEFAULT_ROUTE_AMOUNT,
    unit: "",
    taper: Taper::Linear,
    smoothing_ms: 0.0,
};
/// The parameter a control exposure aliases: a module parameter, or a route's amount
/// (`ROUTE_AMOUNT`).
pub(crate) fn param_info(
    state: &kabl_core::PatchState,
    target: &PortRef,
) -> Result<&'static kabl_modules::ParamInfo, String> {
    let (leaf, param) = match target {
        PortRef::Param { id, param } => (id, param),
        PortRef::CableParam { cable, param } if param == ROUTE_AMOUNT.name => {
            return state
                .cables
                .contains_key(cable)
                .then_some(&ROUTE_AMOUNT)
                .ok_or_else(|| "Missing cable".into());
        }
        PortRef::CableParam { .. } => {
            return Err("Only a route's amount can be a control on a cable".into())
        }
        PortRef::Module { .. } => return Err("Control target must be a parameter".into()),
    };
    let info = state
        .modules
        .get(leaf)
        .and_then(|m| registry::info_for(&m.kind))
        .ok_or("Missing module")?;
    Ok(info
        .params
        .iter()
        .find(|p| p.name == param)
        .ok_or("Missing parameter")?)
}
/// The stored amount of route `cable`, the engine's default when unset. Not clamped: a value
/// outside -1..1 is shown and kept as stored until the knob writes a new one.
pub(crate) fn route_amount(state: &PatchState, cable: kabl_core::CableId) -> f32 {
    state
        .cables
        .get(&cable)
        .and_then(|c| c.params.get(ROUTE_AMOUNT.name))
        .copied()
        .unwrap_or(ROUTE_AMOUNT.default)
}
/// A route's amount as a face reads it: a signed percentage of knob travel.
pub(crate) fn fmt_amount(v: f32) -> String {
    format!("{:+.0} %", v * 100.0 + 0.0)
}
/// A composite face knob on the amount of route `cable`, of radius `r` (world units) at screen
/// `center`: a vertical drag writes the amount through `PatchEditor::set_route_amount`, one
/// undo step per gesture.
#[allow(clippy::too_many_arguments)]
pub(crate) fn face_route_knob(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    cable: kabl_core::CableId,
    center: egui::Pos2,
    r: f32,
    look: crate::routing::Look,
) {
    let z = look.z;
    let rect = egui::Rect::from_center_size(center, egui::Vec2::splat((r + 8.0) * z * 2.0));
    ui_state.record(format!("knob:cable{cable}.{}", ROUTE_AMOUNT.name), rect);
    let resp = ui.interact(
        rect,
        egui::Id::new(("kabl-route-knob", cable)),
        egui::Sense::click_and_drag(),
    );
    if resp.dragged() {
        let fine = if ui.input(|i| i.modifiers.shift) {
            0.1
        } else {
            1.0
        };
        // 150 points of travel sweep the whole range, as on a module knob.
        let step = -resp.drag_delta().y / 150.0 * 2.0 * fine;
        if step != 0.0 {
            let v = route_amount(editor.state(), cable) + step;
            editor.set_route_amount(cable, v, first_of_gesture(ui, &resp));
        }
    }
    let v = route_amount(editor.state(), cable);
    resp.widget_info(|| egui::WidgetInfo::slider(true, f64::from(v), "Route amount"));
    let on = resp.hovered() || resp.dragged();
    let hover = crate::kit::smooth_to(
        ui.ctx(),
        egui::Id::new(("kabl-route-knob-hover", cable)),
        if on { 1.0 } else { 0.0 },
        ui_state.style.motion.hover,
    );
    crate::kit::face::knob(
        painter,
        &ui_state.style,
        &crate::kit::face::Knob {
            c: center,
            rs: r * z,
            z,
            value: ROUTE_AMOUNT.to_norm(v).clamp(0.0, 1.0),
            hover,
            inspected: false,
            focused: false,
            arc: ui_state.style.roles.accent,
        },
    );
    let vh = crate::kit::face::selector_font(&ui_state.style, z).map_or(0.0, |f| f.size * 1.075);
    let at = center + egui::vec2(0.0, crate::rack::knob_text_dy(r, z, vh));
    // Line panels read values in the dimmer text colour, as on a module knob.
    let ink = if ui_state.style.slots.face.material == crate::style::FaceMaterial::Line {
        look.ink2
    } else {
        look.ink
    };
    if let Some(g) = crate::kit::face::readout(
        painter,
        &ui_state.style,
        &fmt_amount(v),
        z,
        look.slot * z - 4.0,
        ink,
    ) {
        let rect = egui::Align2::CENTER_CENTER.anchor_size(at, g.size());
        painter.add(egui::Shape::galley(rect.min, g, ink));
    }
}
/// A plain editor for the amount of route `cable` (the list face and "More controls"): one
/// undo step per drag.
pub(crate) fn route_amount_editor(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    cable: kabl_core::CableId,
    hit: String,
) {
    let mut v = route_amount(editor.state(), cable);
    let resp = ui.add(
        egui::Slider::new(&mut v, ROUTE_AMOUNT.min..=ROUTE_AMOUNT.max)
            .custom_formatter(|v, _| fmt_amount(v as f32)),
    );
    ui_state.record(hit, resp.rect);
    if resp.changed() {
        editor.set_route_amount(cable, v, first_of_gesture(ui, &resp));
    }
}
/// Whether an edit through `resp` this frame starts an undo step: any edit outside a drag, and
/// a drag's first write, which need not come on the frame the drag starts.
fn first_of_gesture(ui: &egui::Ui, resp: &egui::Response) -> bool {
    if !resp.dragged() {
        return true;
    }
    // A drag is told apart from the one before by the time its press began.
    let id = resp.id.with("kabl-gesture");
    let start = ui.input(|i| i.pointer.press_start_time());
    let first = ui.data(|d| d.get_temp::<Option<f64>>(id)) != Some(start);
    ui.data_mut(|d| d.insert_temp(id, start));
    first
}
/// A composite face toggle for two-option stepped `param` in `rect` (screen): the kit pill,
/// on at the upper option. A click anywhere in `rect` sets the other option of the base value,
/// one undo step per click.
#[allow(clippy::too_many_arguments)]
pub(crate) fn face_toggle(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    id: kabl_core::ModuleId,
    param: &'static kabl_modules::ParamInfo,
    rect: egui::Rect,
    look: crate::routing::Look,
) -> Vec<(kabl_core::CableId, egui::Pos2)> {
    debug_assert!(param.taper == Taper::Stepped && param.max - param.min == 1.0);
    let key = format!("{id}.{}", param.name);
    ui_state.record(format!("knob:{key}"), rect);
    ui_state.record(format!("toggle:{key}"), rect);
    let resp = ui.interact(
        rect,
        egui::Id::new(("kabl-toggle", id, param.name)),
        egui::Sense::click(),
    );
    let mut on = crate::routing::base_value(editor.state(), id, param).round() == param.max;
    if resp.clicked() {
        let routes = crate::routing::routes_into(editor.state(), id, param.name);
        crate::routing::inspect(ui_state, &routes, id, param.name);
        on = !on;
        editor.set_param_gesture(
            kabl_core::ParamTarget::Module {
                id,
                param: param.name.into(),
            },
            if on { param.max } else { param.min },
            true,
        );
    }
    let h = (rect.height() * 0.8).min(rect.width() / 2.0);
    let pill = egui::Rect::from_center_size(rect.center(), egui::vec2(2.0 * h, h));
    crate::kit::parts::toggle(painter, &ui_state.style, pill, on);
    if resp.hovered() {
        painter.rect_stroke(
            pill.expand(1.0),
            egui::CornerRadius::same(255),
            egui::Stroke::new(1.0, ui_state.style.roles.accent),
            egui::StrokeKind::Outside,
        );
    }
    let module = editor
        .state()
        .modules
        .get(&id)
        .and_then(|m| registry::info_for(&m.kind))
        .map_or("", |i| i.name);
    resp.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            true,
            on,
            format!("{module} {}", crate::routing::param_label(param)),
        )
    });
    let routes = crate::routing::routes_into(editor.state(), id, param.name);
    crate::routing::selector_plug(ui_state, painter, look, &routes, rect)
}
/// The textures of icon selector `key` on composite `id`'s face, one per option, each PNG's
/// alpha as a white mask the selector tints with its ink. Empty when `icons` is, or when a PNG
/// does not decode (`validate` refuses such a face).
pub(crate) fn icon_textures(
    view: &mut UiState,
    ctx: &egui::Context,
    id: CompositeId,
    key: u64,
    icons: &[Artwork],
) -> Vec<egui::load::SizedTexture> {
    let cache = &mut view.panels.icons;
    if cache.get(&(id, key)).is_none_or(|(old, _)| old != icons) {
        let decoded: Vec<_> = icons
            .iter()
            .map(decode_art)
            .collect::<Result<_, _>>()
            .unwrap_or_default();
        let handles = decoded.iter().enumerate().map(|(k, image)| {
            let size = [image.width() as usize, image.height() as usize];
            let mask = image
                .pixels()
                .map(|p| egui::Color32::from_rgba_unmultiplied(255, 255, 255, p.0[3]))
                .collect();
            ctx.load_texture(
                format!("icon-{id}-{key}-{k}"),
                egui::ColorImage::new(size, mask),
                egui::TextureOptions::LINEAR,
            )
        });
        cache.insert((id, key), (icons.to_vec(), handles.collect()));
    }
    cache[&(id, key)]
        .1
        .iter()
        .map(egui::load::SizedTexture::from_handle)
        .collect()
}
/// Display element `key` of composite `id` in `footprint` (world units): asks for its leaf's tap
/// while on screen, as a module face does, then draws the leaf's display inside the footprint.
/// A source that is gone or has no display draws nothing.
#[allow(clippy::too_many_arguments)]
pub(crate) fn face_display(
    editor: &PatchEditor,
    view: &mut UiState,
    painter: &egui::Painter,
    xf: crate::Xf,
    id: CompositeId,
    key: u64,
    p: &kabl_core::panel::Placement,
    footprint: egui::Rect,
    face: egui::Color32,
    now: f64,
) {
    let Some(kabl_core::panel::Source::Module { id: leaf, .. }) = p.source else {
        return;
    };
    let Some(info) = editor
        .state()
        .modules
        .get(&leaf)
        .and_then(|m| registry::info_for(&m.kind))
        .filter(|i| crate::displays::has_display(i.kind))
    else {
        return;
    };
    let screen = xf.r(footprint);
    let painter = painter.with_clip_rect(painter.clip_rect().intersect(screen));
    if painter.clip_rect().is_positive() {
        view.wants.display(leaf, info.kind);
        view.wants.feeds(editor.state(), leaf, info.kind);
    }
    view.live.master = view.meter.level;
    crate::displays::draw(
        editor.state(),
        &view.style,
        &view.live,
        now,
        &painter,
        xf,
        leaf,
        info,
        footprint,
        face,
    );
    view.record(format!("composite:{id}:display:{key}"), screen);
}
pub fn import_art(path: &Path) -> Result<Artwork, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut png = Vec::new();
    file.take((kabl_core::panel::MAX_IMAGE_BYTES + 1) as u64)
        .read_to_end(&mut png)
        .map_err(|e| e.to_string())?;
    let art = Artwork {
        name: path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("Invalid filename")?
            .into(),
        png,
    };
    decode_art(&art)?;
    Ok(art)
}
pub fn read_package(path: &Path) -> Result<composites::Package, String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    composites::decode(&bytes)
}
pub fn export(editor: &PatchEditor, id: CompositeId, path: &Path) -> Result<(), String> {
    let pkg = composites::package(editor.state(), id)?;
    let bytes = serde_json::to_vec_pretty(&pkg).map_err(|e| e.to_string())?;
    composites::decode(&bytes)?;
    // Do not truncate an existing package on a failed or accidental export.
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    if let Err(e) = file.write_all(&bytes) {
        let _ = std::fs::remove_file(path);
        return Err(e.to_string());
    }
    Ok(())
}
#[derive(Default)]
pub struct View {
    /// The composite design page, while open.
    pub page: Option<crate::design_page::Page>,
    pub path: String,
    pub error: Option<String>,
    pub fallback: Option<CompositeId>,
    pub textures: BTreeMap<(CompositeId, bool), (Artwork, egui::TextureHandle, egui::Color32)>,
    /// Icon selector PNGs per (composite, placement key), as alpha masks.
    icons: BTreeMap<(CompositeId, u64), (Vec<Artwork>, Vec<egui::TextureHandle>)>,
    editor: u64,
}
/// Opens the design page for `c`, its face (or the default face) as the starting draft.
pub fn open(view: &mut UiState, id: CompositeId, c: &Composite) {
    view.panels.page = Some(crate::design_page::Page::new(id, c));
    view.panels.error = None;
}
pub fn editor(editor: &mut PatchEditor, view: &mut UiState, ctx: &egui::Context) {
    if view.panels.editor != editor.instance() {
        view.panels = View {
            editor: editor.instance(),
            ..View::default()
        };
    }
    let page = view.panels.page.as_ref();
    view.panels.textures.retain(|(id, dark), (old, _, _)| {
        let art = |p: &Panel| {
            if *dark {
                p.dark.clone()
            } else {
                p.light.clone()
            }
        };
        // The page draws its draft's artwork under the same key: that texture stays too.
        let drafted = page
            .filter(|p| p.id == *id)
            .and_then(|p| art(&p.draft.panel));
        editor
            .state()
            .composites
            .get(id)
            .and_then(|c| c.panel.as_deref())
            .and_then(art)
            .is_some_and(|a| a == *old)
            || drafted.is_some_and(|a| a == *old)
    });
    view.panels.icons.retain(|(id, key), (old, _)| {
        let icons = |p: &Panel| p.placements.get(key).map(|p| p.icons.clone());
        let drafted = page
            .filter(|p| p.id == *id)
            .and_then(|p| icons(&p.draft.panel));
        editor
            .state()
            .composites
            .get(id)
            .and_then(|c| c.panel.as_deref())
            .and_then(icons)
            .is_some_and(|a| a == *old)
            || drafted.is_some_and(|a| a == *old)
    });
    if let Some(p) = crate::filebrowser::take(view, crate::filebrowser::Purpose::PanelArt) {
        view.panels.path = p.to_string_lossy().into_owned();
    }
    if let Some(id) = view.panels.fallback {
        let Some(c) = editor.state().composites.get(&id).cloned() else {
            view.panels.fallback = None;
            return;
        };
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new("panel-fallback"))
            .area(crate::rack_editor::focused_area(
                ctx,
                egui::Id::new("panel-fallback"),
                480.,
            ))
            .show(ctx, |ui| {
                crate::rack_editor::focused_height(ui);
                ui.set_width(480.0_f32.min(ctx.content_rect().width() - 60.));
                ui.horizontal(|ui| {
                    ui.heading(format!("More controls · {}", c.name));
                    let r = ui.button("Close");
                    view.record("more:close".into(), r.rect);
                    close = r.clicked();
                });
                egui::ScrollArea::vertical()
                    .max_height(ui.available_height())
                    .show_owned(ui, |ui| {
                        ui.label(&c.help);
                        ui.label(
                        "All exposed controls remain available regardless of artwork or placement.",
                    );
                        for (&key, e) in &c.controls {
                            if let PortRef::CableParam { cable, .. } = &e.target {
                                ui.label(&e.label);
                                route_amount_editor(
                                    editor,
                                    view,
                                    ui,
                                    *cable,
                                    format!("fallback:{id}:{key}"),
                                );
                            }
                            if let PortRef::Param { id: leaf, param } = &e.target {
                                if let Some(p) =
                                    registry::info_for(&editor.state().modules[leaf].kind)
                                        .and_then(|i| i.params.iter().find(|p| p.name == param))
                                {
                                    ui.label(&e.label);
                                    crate::perform::param_editor(
                                        editor,
                                        view,
                                        ui,
                                        *leaf,
                                        p,
                                        false,
                                        Some(format!("fallback:{id}:{key}")),
                                    );
                                }
                            }
                        }
                        for (&key, e) in &c.ports {
                            let dir = composites::direction(editor.state(), &e.target)
                                .unwrap_or(kabl_modules::PortDirection::Input);
                            let r = ui.button(format!(
                                "{} · {}",
                                if dir == kabl_modules::PortDirection::Output {
                                    "OUT"
                                } else {
                                    "IN"
                                },
                                e.label
                            ));
                            view.record(format!("fallback:{id}:port:{key}"), r.rect);
                            if r.clicked() {
                                if dir == kabl_modules::PortDirection::Output {
                                    view.pending_output = Some(e.target.clone());
                                } else if let Some(from) = view.pending_output.take() {
                                    let open = &view.composites.open;
                                    composites::connect(editor, open, from, e.target.clone());
                                }
                            }
                        }
                        if ui.button("Inspect internals / all routes").clicked() {
                            view.enter_composite(editor.state(), id);
                            close = true;
                        }
                    });
            });
        if close || modal.should_close() {
            view.panels.fallback = None;
            view.last_inspected = view.inspected.clone();
            let lay = crate::composite_layout(editor.state(), view);
            view.lanes_shown = crate::inspected_extent(editor, view, &lay).map(|(_, key)| key);
        }
    }
}
