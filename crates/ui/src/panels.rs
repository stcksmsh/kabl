//! Composite face authoring and asset preparation. Called only on UI/control threads.
use crate::kit::{self, g4, g4::Sev, Tone};
use crate::style::Role;
use crate::wheel::OwnedScroll;
use crate::{composites, PatchEditor, UiState};
use kabl_core::{
    panel::{Artwork, Kind, Panel, Placement},
    CompositeId, PortRef,
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
            panel.validate(c)?;
            for (&id, p) in &panel.placements {
                if let Some(e) = c.controls.get(&id) {
                    let PortRef::Param { id: leaf, param } = &e.target else {
                        return Err("Control target must be a parameter".into());
                    };
                    let info =
                        registry::info_for(&state.modules[leaf].kind).ok_or("Missing module")?;
                    let target = info
                        .params
                        .iter()
                        .find(|p| p.name == param)
                        .ok_or("Missing parameter")?;
                    if (p.kind == Kind::Selector) != (target.taper == Taper::Stepped) {
                        return Err(format!("Binding {id}: stepped parameters need selectors; continuous parameters need knobs"));
                    }
                    if p.kind == Kind::Selector && p.width < (target.max - target.min + 1.0) * 18.0
                    {
                        return Err(format!(
                            "Binding {id}: selector needs at least 18 points per choice"
                        ));
                    }
                }
            }
            for art in [&panel.light, &panel.dark].into_iter().flatten() {
                decode_art(art)?;
            }
        }
    }
    Ok(())
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
    pub selected: Option<CompositeId>,
    pub draft: Panel,
    pub path: String,
    pub labels: BTreeMap<u64, String>,
    pub error: Option<String>,
    pub fallback: Option<CompositeId>,
    pub textures: BTreeMap<(CompositeId, bool), (Artwork, egui::TextureHandle, egui::Color32)>,
    editor: u64,
}
/// A numeric field: edits a text copy while focused, then shows the clamped value again.
fn num_field(
    ui: &mut egui::Ui,
    st: &crate::style::Style,
    key: &str,
    v: &mut f32,
    lo: f32,
    hi: f32,
) -> egui::Response {
    let id = egui::Id::new(("panel-num", key));
    let mut s = ui
        .data_mut(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| format!("{}", *v));
    let r = kit::field(ui, st, &mut s, "", None, Some(72.0));
    if r.has_focus() {
        if let Ok(x) = s.trim().parse::<f32>() {
            *v = x.clamp(lo, hi);
        }
        ui.data_mut(|d| d.insert_temp(id, s));
    } else {
        ui.data_mut(|d| d.remove::<String>(id));
    }
    r
}
pub fn open(view: &mut UiState, id: CompositeId, c: &kabl_core::Composite) {
    view.panels.selected = Some(id);
    view.panels.draft = c.panel.as_deref().cloned().unwrap_or_default();
    view.panels.error = None;
    view.panels.labels = c
        .controls
        .iter()
        .chain(&c.ports)
        .map(|(&id, e)| (id, e.label.clone()))
        .collect();
}
pub fn editor(editor: &mut PatchEditor, view: &mut UiState, ctx: &egui::Context) {
    if view.panels.editor != editor.instance() {
        view.panels = View {
            editor: editor.instance(),
            ..View::default()
        };
    }
    view.panels.textures.retain(|(id, dark), (old, _, _)| {
        editor
            .state()
            .composites
            .get(id)
            .and_then(|c| c.panel.as_ref())
            .and_then(|p| {
                if *dark {
                    p.dark.as_ref()
                } else {
                    p.light.as_ref()
                }
            })
            .is_some_and(|art| art == old)
    });
    if let Some(p) = crate::filebrowser::take(view, crate::filebrowser::Purpose::PanelArt) {
        view.panels.path = p.to_string_lossy().into_owned();
    }
    if let Some(id) = view.panels.selected {
        let Some(c) = editor.state().composites.get(&id).cloned() else {
            view.panels.selected = None;
            return;
        };
        let mut shown = true;
        let st = view.style.clone();
        let st = &*st;
        let title = format!("Panel authoring · {} #{id}", c.name);
        let width = 528.0_f32.min(ctx.content_rect().width() - 112.0);
        g4::tool_window(
            ctx,
            st,
            "panel-authoring",
            (&title, Some("ADVANCED")),
            egui::pos2(40.0, 80.0),
            &mut shown,
            |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                if let Some(e) = view.panels.error.clone() {
                    g4::banner(ui, st, Sev::Bad, "Could not apply", Some(&e));
                }
                let row = |ui: &mut egui::Ui, label: &str, add: &mut dyn FnMut(&mut egui::Ui)| {
                    ui.horizontal(|ui| {
                        ui.set_min_height(32.0);
                        let (r, _) =
                            ui.allocate_exact_size(egui::vec2(112.0, 32.0), egui::Sense::hover());
                        ui.painter().text(
                            r.left_center(),
                            egui::Align2::LEFT_CENTER,
                            label,
                            st.font(Role::Body),
                            kit::tone(st, Tone::Text2),
                        );
                        add(ui);
                    });
                };
                row(ui, "Width", &mut |ui| {
                    let r = num_field(
                        ui,
                        st,
                        "panel-width",
                        &mut view.panels.draft.width,
                        180.0,
                        840.0,
                    );
                    view.record("panel:width".into(), r.rect);
                    kit::label(ui, st, Role::Value, Tone::Text3, "px");
                });
                row(ui, "Preview", &mut |ui| {
                    kit::segmented(ui, st, |ui| {
                        let r = kit::seg(ui, st, "Light", !view.dark);
                        view.record("panel:light".into(), r.rect);
                        if r.clicked() {
                            view.dark = false;
                        }
                        let r = kit::seg(ui, st, "Dark", view.dark);
                        view.record("panel:dark".into(), r.rect);
                        if r.clicked() {
                            view.dark = true;
                        }
                    });
                });
                kit::rule(ui, st);
                kit::section(ui, st, "Artwork");
                row(ui, "PNG or package", &mut |ui| {
                    let w = ui.available_width() - 100.0 - 8.0;
                    let r = kit::field(
                        ui,
                        st,
                        &mut view.panels.path,
                        "Local file path",
                        None,
                        Some(w),
                    );
                    view.record("panel:path".into(), r.rect);
                    let r = ui.add(kit::Button::new(st, "Browse…").g4(32.0).min_width(100.0));
                    view.record("panel:browse".into(), r.rect);
                    if r.clicked() {
                        let hint = view.panels.path.clone();
                        crate::filebrowser::open(
                            view,
                            crate::filebrowser::Purpose::PanelArt,
                            "Choose artwork",
                            crate::filebrowser::Mode::Open,
                            crate::filebrowser::IMAGE,
                            "",
                            &hint,
                        );
                    }
                });
                for dark in [false, true] {
                    let have = if dark {
                        view.panels.draft.dark.as_ref()
                    } else {
                        view.panels.draft.light.as_ref()
                    }
                    .map(|a| a.name.clone());
                    let key = if dark {
                        "panel:import-dark"
                    } else {
                        "panel:import-light"
                    };
                    row(ui, if dark { "Dark PNG" } else { "Light PNG" }, &mut |ui| {
                        let dot = if have.is_some() {
                            st.roles.good
                        } else {
                            st.roles.warn
                        };
                        g4::chip_label(
                            ui,
                            st,
                            have.as_deref().unwrap_or("missing"),
                            true,
                            Some(dot),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let r =
                                ui.add(kit::Button::new(st, "Import…").g4(32.0).min_width(100.0));
                            view.record(key.into(), r.rect);
                            if r.clicked() {
                                match import_art(Path::new(&view.panels.path)) {
                                    Ok(a) => {
                                        if dark {
                                            view.panels.draft.dark = Some(a);
                                        } else {
                                            view.panels.draft.light = Some(a);
                                        }
                                        view.panels.error = None;
                                    }
                                    Err(e) => view.panels.error = Some(e),
                                }
                            }
                        });
                    });
                }
                if view.panels.draft.light.is_some() || view.panels.draft.dark.is_some() {
                    let r = ui.add(kit::Button::new(st, "Clear artwork").g4(32.0).ghost());
                    view.record("panel:clear-art".into(), r.rect);
                    if r.clicked() {
                        view.panels.draft.light = None;
                        view.panels.draft.dark = None;
                    }
                }
                kit::rule(ui, st);
                kit::section(ui, st, "Controls and jacks");
                egui::ScrollArea::vertical()
                    .max_height((ctx.content_rect().height() - 520.0).clamp(120.0, 260.0))
                    .show_owned(ui, |ui| {
                        for (&key, e) in c.controls.iter().chain(&c.ports) {
                            ui.push_id(key, |ui| {
                                let placed = view.panels.draft.placements.contains_key(&key);
                                ui.horizontal(|ui| {
                                    ui.set_min_height(32.0);
                                    kit::label(ui, st, Role::Value, Tone::Text3, format!("{key}"));
                                    let label = view
                                        .panels
                                        .labels
                                        .entry(key)
                                        .or_insert_with(|| e.label.clone());
                                    let r = kit::field(ui, st, label, "Label", None, Some(200.0));
                                    view.record(format!("panel:{key}:label"), r.rect);
                                    if placed {
                                        g4::chip_label(ui, st, "placed", true, Some(st.roles.good));
                                    }
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let (k, text) = if placed {
                                                (format!("panel:unplace:{key}"), "Unplace")
                                            } else {
                                                (format!("panel:place:{key}"), "Place")
                                            };
                                            let r = ui.add(
                                                kit::Button::new(st, text)
                                                    .g4(32.0)
                                                    .min_width(100.0),
                                            );
                                            view.record(k, r.rect);
                                            if !r.clicked() {
                                                return;
                                            }
                                            if placed {
                                                view.panels.draft.placements.remove(&key);
                                                return;
                                            }
                                            let kind = if c.ports.contains_key(&key) {
                                                Kind::Jack
                                            } else {
                                                let PortRef::Param { id: leaf, param } = &e.target
                                                else {
                                                    return;
                                                };
                                                let stepped = registry::info_for(
                                                    &editor.state().modules[leaf].kind,
                                                )
                                                .is_some_and(|i| {
                                                    i.params.iter().any(|p| {
                                                        p.name == param && p.taper == Taper::Stepped
                                                    })
                                                });
                                                if stepped {
                                                    Kind::Selector
                                                } else {
                                                    Kind::Knob
                                                }
                                            };
                                            let (w, h) = match kind {
                                                Kind::Knob => (84.0, 110.0),
                                                Kind::Selector => (180.0, 55.0),
                                                Kind::Jack => (64.0, 58.0),
                                            };
                                            view.panels.draft.placements.insert(
                                                key,
                                                Placement {
                                                    kind,
                                                    x: 12.0,
                                                    y: 72.0,
                                                    width: w,
                                                    height: h,
                                                },
                                            );
                                        },
                                    );
                                });
                                if let Some(mut p) = view.panels.draft.placements.get(&key).cloned()
                                {
                                    let mut recorded = Vec::new();
                                    ui.horizontal(|ui| {
                                        kit::label(
                                            ui,
                                            st,
                                            Role::Caption,
                                            Tone::Text3,
                                            format!("{:?}", p.kind),
                                        );
                                        for (name, value) in [
                                            ("x", &mut p.x),
                                            ("y", &mut p.y),
                                            ("w", &mut p.width),
                                            ("h", &mut p.height),
                                        ] {
                                            kit::label(ui, st, Role::Caption, Tone::Text3, name);
                                            let r = num_field(
                                                ui,
                                                st,
                                                &format!("panel-{key}-{name}"),
                                                value,
                                                0.0,
                                                2000.0,
                                            );
                                            recorded.push((format!("panel:{key}:{name}"), r.rect));
                                        }
                                    });
                                    for (k, rect) in recorded {
                                        view.record(k, rect);
                                    }
                                    view.panels.draft.placements.insert(key, p);
                                }
                            });
                        }
                    });
                kit::paragraph(
                    ui,
                    st,
                    Role::Caption,
                    Tone::Text3,
                    "Apply is one project Undo step. Unplaced controls stay reachable in Public controls.",
                );
                kit::rule(ui, st);
                ui.horizontal(|ui| {
                    ui.set_min_height(32.0);
                    let r = ui.add(kit::Button::new(st, "Export package…").g4(32.0).ghost());
                    view.record("panel:export".into(), r.rect);
                    if r.clicked() {
                        view.panels.error = export(editor, id, Path::new(&view.panels.path)).err();
                    }
                    let r = ui.add(kit::Button::new(st, "Import package…").g4(32.0).ghost());
                    view.record("panel:import-package".into(), r.rect);
                    if r.clicked() {
                        let result = read_package(Path::new(&view.panels.path))
                            .and_then(|pkg| composites::insert(editor, &pkg));
                        match result {
                            Ok(id) => {
                                view.last_message =
                                    Some(format!("Imported independent instance #{id}"))
                            }
                            Err(e) => view.panels.error = Some(e),
                        }
                    }
                    let r = ui.add(kit::Button::new(st, "Default face").g4(32.0).ghost());
                    view.record("panel:remove".into(), r.rect);
                    if r.clicked() {
                        let mut latest = editor.state().composites[&id].clone();
                        latest.panel = None;
                        view.panels.error = composites::set(editor, id, latest).err();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let r = ui.add(kit::Button::new(st, "Apply").g4(32.0).primary());
                        view.record("panel:apply".into(), r.rect);
                        if r.clicked() {
                            let mut latest = editor.state().composites[&id].clone();
                            latest.panel = Some(Box::new(view.panels.draft.clone()));
                            for (key, label) in &view.panels.labels {
                                if let Some(e) = latest
                                    .controls
                                    .get_mut(key)
                                    .or_else(|| latest.ports.get_mut(key))
                                {
                                    e.label = label.clone();
                                }
                            }
                            view.panels.error = composites::set(editor, id, latest).err();
                        }
                        let r = ui.add(kit::Button::new(st, "Reload").g4(32.0));
                        view.record("panel:reload".into(), r.rect);
                        if r.clicked() {
                            let latest = editor.state().composites[&id].clone();
                            open(view, id, &latest);
                        }
                    });
                });
            },
        );
        if !shown {
            view.panels.selected = None;
        }
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
                                    editor.connect(from, e.target.clone());
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
