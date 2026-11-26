//! Composite face authoring and asset preparation. Called only on UI/control threads.
use crate::wheel::OwnedScroll;
use crate::{composites, PatchEditor, UiState};
use kabl_core::{
    panel::{Artwork, Kind, Panel},
    Composite, CompositeId, PortRef,
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
        // Stored and checked by `Panel::validate`; each kind is lifted here once it draws.
        if !matches!(
            p.kind,
            Kind::Knob
                | Kind::Selector
                | Kind::Jack
                | Kind::Slider
                | Kind::Toggle
                | Kind::Icons
                | Kind::Display
        ) {
            return Err(format!(
                "Binding {id}: {:?} elements are not supported yet",
                p.kind
            ));
        }
        if p.kind == Kind::Display {
            // `Panel::validate` has checked the source is a module without an action.
            if let Some(kabl_core::panel::Source::Module { id: leaf, .. }) = p.source {
                let m = state.modules.get(&leaf).ok_or_else(|| {
                    format!("Binding {id}: the display's module {leaf} is missing")
                })?;
                if !crate::displays::has_display(&m.kind) {
                    return Err(format!("Binding {id}: {} modules have no display", m.kind));
                }
            }
        }
        if let Some(e) = c.controls.get(&id) {
            let target = param_info(state, &e.target)?;
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
/// The module parameter a control exposure aliases.
pub(crate) fn param_info(
    state: &kabl_core::PatchState,
    target: &PortRef,
) -> Result<&'static kabl_modules::ParamInfo, String> {
    let PortRef::Param { id: leaf, param } = target else {
        return Err("Control target must be a parameter".into());
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
