//! Composite face authoring and asset preparation. Called only on UI/control threads.
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
fn button(ui: &mut egui::Ui, view: &mut UiState, key: &str, label: &str) -> bool {
    let r = ui.button(label);
    view.record(key.into(), r.rect);
    r.clicked()
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
    if let Some(id) = view.panels.selected {
        let Some(c) = editor.state().composites.get(&id).cloned() else {
            view.panels.selected = None;
            return;
        };
        let mut shown = true;
        egui::Window::new(format!("Panel authoring · {} #{id}",c.name)).id(egui::Id::new("panel-authoring"))
            .open(&mut shown).default_width(560.0).default_height(640.0).vscroll(true).show(ctx,|ui| {
            ui.label("340-point face. Coordinates include label/value footprints. Apply = one project Undo step.");
            if let Some(e)=&view.panels.error { ui.colored_label(egui::Color32::LIGHT_RED,e); }
            ui.horizontal(|ui| {
                ui.label("Width");
                let r=ui.add(egui::DragValue::new(&mut view.panels.draft.width).speed(30.0).range(180.0..=840.0));view.record("panel:width".into(),r.rect);
                if button(ui,view,"panel:light","Preview light") { view.dark=false; }
                if button(ui,view,"panel:dark","Preview dark") { view.dark=true; }
            });
            ui.label("PNG import or complete package export path (local file)");
            let r=ui.text_edit_singleline(&mut view.panels.path);view.record("panel:path".into(),r.rect);
            ui.horizontal(|ui| {
                for (dark,label) in [(false,"Import light PNG"),(true,"Import dark PNG")] {
                    if button(ui,view,if dark {"panel:import-dark"} else {"panel:import-light"},label) {
                        match import_art(Path::new(&view.panels.path)) { Ok(a)=>{if dark {view.panels.draft.dark=Some(a)}else{view.panels.draft.light=Some(a)};view.panels.error=None},Err(e)=>view.panels.error=Some(e) }
                    }
                }
                if button(ui,view,"panel:clear-art","Clear artwork") {view.panels.draft.light=None;view.panels.draft.dark=None;}
            });
            ui.label(format!("Light: {} · Dark: {}",view.panels.draft.light.as_ref().map_or("missing",|a|a.name.as_str()),view.panels.draft.dark.as_ref().map_or("missing",|a|a.name.as_str())));
            ui.separator();
            for (&key,e) in c.controls.iter().chain(&c.ports) {
                ui.push_id(key,|ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!("ID {key}"));
                        let label=view.panels.labels.entry(key).or_insert_with(||e.label.clone());
                        let r=ui.add(egui::TextEdit::singleline(label).desired_width(210.0));view.record(format!("panel:{key}:label"),r.rect);
                        if !view.panels.draft.placements.contains_key(&key) && button(ui,view,&format!("panel:place:{key}"),"Place") {
                            let kind=if c.ports.contains_key(&key) {Kind::Jack} else {
                                let PortRef::Param{id:leaf,param}=&e.target else{return};
                                let stepped=registry::info_for(&editor.state().modules[leaf].kind).is_some_and(|i|i.params.iter().any(|p|p.name==param&&p.taper==Taper::Stepped));
                                if stepped {Kind::Selector}else{Kind::Knob}
                            };
                            let (w,h)=match kind {Kind::Knob=>(84.0,110.0),Kind::Selector=>(180.0,55.0),Kind::Jack=>(64.0,58.0)};
                            let x=12.0;let y=72.0;
                            view.panels.draft.placements.insert(key,Placement{kind,x,y,width:w,height:h});
                        }
                    });
                    if let Some(p)=view.panels.draft.placements.get_mut(&key) {
                        let mut remove=false;
                        let mut recorded=Vec::new();
                        ui.horizontal(|ui| {
                            ui.label(format!("{:?}",p.kind));
                            for (name,value) in [("x",&mut p.x),("y",&mut p.y),("w",&mut p.width),("h",&mut p.height)] {
                                ui.label(name);let r=ui.add(egui::DragValue::new(value).speed(1.0));recorded.push((format!("panel:{key}:{name}"),r.rect));
                            }
                            remove=ui.small_button("Unplace").clicked();
                        });
                        for (key,rect) in recorded {view.record(key,rect);}
                        if remove {view.panels.draft.placements.remove(&key);}
                    }
                });
            }
            ui.separator();
            ui.horizontal(|ui| {
                if button(ui,view,"panel:apply","Apply panel / Undoable") {
                    let mut latest=editor.state().composites[&id].clone();latest.panel=Some(Box::new(view.panels.draft.clone()));
                    for (key,label) in &view.panels.labels {
                        if let Some(e)=latest.controls.get_mut(key).or_else(||latest.ports.get_mut(key)) {e.label=label.clone();}
                    }
                    view.panels.error=composites::set(editor,id,latest).err();
                }
                if button(ui,view,"panel:reload","Reload saved face") {let latest=editor.state().composites[&id].clone();open(view,id,&latest);}
                if button(ui,view,"panel:done","Done") { view.panels.selected=None; }
                if button(ui,view,"panel:remove","Use default face") {
                    let mut latest=editor.state().composites[&id].clone();latest.panel=None;view.panels.error=composites::set(editor,id,latest).err();
                }
            });
            ui.label("Preview uses applied face. Unplaced controls remain reachable in Public controls. Edit labels/help in instance tools.");
            if button(ui,view,"panel:export","Export complete package (new file)") {view.panels.error=export(editor,id,Path::new(&view.panels.path)).err();}
            if button(ui,view,"panel:import-package","Import complete package as new instance") {
                let result=read_package(Path::new(&view.panels.path)).and_then(|pkg|composites::insert(editor,&pkg));
                match result {Ok(id)=>view.last_message=Some(format!("Imported independent instance #{id}")),Err(e)=>view.panels.error=Some(e)}
            }
        });
        if !shown {
            view.panels.selected = None;
        }
    }
    if let Some(id) = view.panels.fallback {
        let Some(c) = editor.state().composites.get(&id).cloned() else {
            view.panels.fallback = None;
            return;
        };
        let mut shown = true;
        egui::Window::new(format!("Public controls · {}", c.name))
            .id(egui::Id::new("panel-fallback"))
            .open(&mut shown)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.label(&c.help);
                ui.label(
                    "All exposed controls remain available regardless of artwork or placement.",
                );
                for (&key, e) in &c.controls {
                    if let PortRef::Param { id: leaf, param } = &e.target {
                        if let Some(p) = registry::info_for(&editor.state().modules[leaf].kind)
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
                    view.composites.open.insert(id);
                }
            });
        if !shown {
            view.panels.fallback = None;
        }
    }
}
