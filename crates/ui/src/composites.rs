//! Instance editing and immutable portable definitions, entirely on the control/UI side.
use crate::PatchEditor;
use kabl_core::{Composite, CompositeId, Definition, Exposure, ModuleId, Op, PatchState, PortRef};
use kabl_modules::{registry, PortDirection};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
const MAX_BYTES: usize = 2 * 1024 * 1024 - 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    pub schema: u32,
    pub root: CompositeId,
    pub patch: PatchState,
}

pub fn validate(p: &PatchState) -> Result<(), String> {
    if p.modules.len() > 128
        || p.cables.len() > 512
        || p.modules
            .values()
            .filter(|m| matches!(m.kind.as_str(), "delay" | "chorus" | "reverb"))
            .count()
            > 8
    {
        return Err("Expanded graph/effect limit".into());
    }
    if p.modules
        .keys()
        .chain(p.cables.keys())
        .any(|id| *id == u64::MAX)
        || p.modules.values().any(|m| {
            !m.pos.x.is_finite()
                || !m.pos.y.is_finite()
                || m.params.len() > 256
                || m.params
                    .iter()
                    .any(|(k, v)| k.len() > 128 || !v.is_finite())
        })
        || p.cables.values().any(|c| {
            c.params.len() > 32
                || c.steps.len() > 256
                || c.params.values().chain(&c.steps).any(|v| !v.is_finite())
        })
    {
        return Err("Invalid graph identity/value".into());
    }
    if serde_json::to_vec(p).map_err(|e| e.to_string())?.len() > MAX_BYTES {
        return Err("Composite serialized size limit".into());
    }
    if p.labels.len() > 128
        || p.labels.iter().any(|(id, labels)| {
            !p.modules.contains_key(id)
                || labels.len() > 256
                || labels.iter().any(|(k, v)| k.len() > 128 || v.len() > 4096)
        })
        || p.cables
            .values()
            .any(|c| c.params.keys().any(|k| k.len() > 128))
    {
        return Err("Composite label/cable data limit".into());
    }
    kabl_engine::compile::validate_composites(p)?;
    // Presentation clock/cue references do not enter the compiler's graph.
    for (&id, m) in &p.modules {
        for (key, value) in &m.params {
            let reference = if key == crate::banks::LAUNCH_CLOCK
                || (m.kind == "cues" && key.starts_with("cue") && key.ends_with(".clock"))
            {
                Some((*value as u64, "clock"))
            } else if m.kind == "cues" {
                key.split_once(".seq.")
                    .and_then(|(_, s)| s.parse::<u64>().ok())
                    .map(|id| (id, "seq"))
            } else {
                None
            };
            if let Some((target, kind)) = reference {
                if p.modules.get(&target).is_none_or(|m| m.kind != kind) {
                    return Err(format!("Module {id}: unresolved dependency {key} → {kind} #{target}; include referenced module before export/import"));
                }
            }
        }
    }
    Ok(())
}
fn compile(p: &PatchState) -> Result<(), String> {
    validate(p)?;
    kabl_engine::compile::compile(p, 48000.0, kabl_standalone::DEFAULT_VOICE_COUNT)
        .map(|_| ())
        .map_err(|e| e.to_string())
}
fn commit(editor: &mut PatchEditor, candidate: PatchState) -> Result<(), String> {
    compile(&candidate)?;
    let ops = crate::compare::diff(editor.state(), &candidate)
        .ok_or("Cannot construct atomic composite transaction")?;
    editor.restore_to(ops);
    Ok(())
}
fn next_group(editor: &PatchEditor) -> Result<u64, String> {
    fn visit(op: &Op, max: &mut u64) {
        match op {
            Op::SetComposite { id, .. } => *max = (*max).max(*id),
            Op::Group { ops } => {
                for op in ops {
                    visit(op, max)
                }
            }
            _ => {}
        }
    }
    let mut max = editor.state().composites.keys().copied().max().unwrap_or(0);
    for entry in editor.log().all_entries() {
        visit(&entry.op, &mut max);
        visit(&entry.inverse, &mut max)
    }
    max.checked_add(1)
        .filter(|n| *n < u64::MAX)
        .ok_or_else(|| "Composite identity exhausted".into())
}
fn definition(id: u64) -> Definition {
    Definition {
        id: format!(
            "kabl-{}-{}-{id}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ),
        version: 0,
    }
}
fn expose(c: &mut Composite, target: PortRef, label: String, control: bool) -> Result<(), String> {
    let map = if control {
        &mut c.controls
    } else {
        &mut c.ports
    };
    if map.values().any(|e| e.target == target) {
        return Ok(());
    }
    let id = c.next_interface;
    c.next_interface = id
        .checked_add(1)
        .filter(|n| *n < u64::MAX)
        .ok_or("Interface identity exhausted")?;
    map.insert(id, Exposure { label, target });
    Ok(())
}
pub fn encapsulate(
    editor: &mut PatchEditor,
    members: BTreeSet<ModuleId>,
    children: BTreeSet<CompositeId>,
    name: &str,
) -> Result<CompositeId, String> {
    if members.is_empty() && children.is_empty() {
        return Err("Select modules or composites first".into());
    }
    let mut p = editor.state().clone();
    let id = next_group(editor)?;
    if members
        .iter()
        .any(|m| p.composites.values().any(|c| c.members.contains(m)))
    {
        return Err("Selection contains an owned leaf; select its composite instead".into());
    }
    if children
        .iter()
        .any(|child| p.composites.get(child).is_none_or(|c| c.parent.is_some()))
    {
        return Err("Select only top-level composites for nesting".into());
    }
    let all: BTreeSet<_> = members
        .iter()
        .copied()
        .chain(
            children
                .iter()
                .flat_map(|&child| kabl_core::composite::leaves(&p, child)),
        )
        .collect();
    let pos = all
        .iter()
        .find_map(|m| p.modules.get(m))
        .map(|m| m.pos)
        .ok_or("Missing selected module")?;
    let mut c=Composite{name:name.trim().into(),help:"Open internals to inspect the real modules and routes. Editing affects this instance only.".into(),definition:definition(id),parent:None,members,pos,ports:BTreeMap::new(),controls:BTreeMap::new(),next_interface:1};
    for cable in p.cables.values() {
        let a = all.contains(&cable.from.module_id());
        let b = all.contains(&cable.to.module_id());
        if a != b {
            let target = if a { &cable.from } else { &cable.to };
            expose(&mut c, target.clone(), binding_label(target), false)?;
        }
    }
    // Preserve pinned public controls as aliases of the original parameters.
    for pin in crate::perform::pins(&p)
        .into_iter()
        .filter(|pin| all.contains(&pin.id))
    {
        if registry::info_for(&p.modules[&pin.id].kind)
            .is_some_and(|i| i.params.iter().any(|param| param.name == pin.key))
        {
            expose(
                &mut c,
                PortRef::Param {
                    id: pin.id,
                    param: pin.key.clone(),
                },
                crate::perform::pin_label(&p, &pin),
                true,
            )?;
        }
    }
    for child in children {
        p.composites.get_mut(&child).unwrap().parent = Some(id);
    }
    p.composites.insert(id, c);
    commit(editor, p)?;
    Ok(id)
}
pub fn binding_label(t: &PortRef) -> String {
    match t {
        PortRef::Module { id, port } => format!("{port} · #{id}"),
        PortRef::Param { id, param } => format!("{param} · #{id}"),
    }
}
pub fn set(editor: &mut PatchEditor, id: CompositeId, c: Composite) -> Result<(), String> {
    let mut p = editor.state().clone();
    p.composites.insert(id, c);
    commit(editor, p)
}
pub fn add_exposure(
    editor: &mut PatchEditor,
    id: CompositeId,
    target: PortRef,
    control: bool,
) -> Result<(), String> {
    let mut c = editor
        .state()
        .composites
        .get(&id)
        .cloned()
        .ok_or("Missing composite")?;
    expose(&mut c, target.clone(), binding_label(&target), control)?;
    set(editor, id, c)
}
pub fn remove_exposure(
    editor: &mut PatchEditor,
    id: CompositeId,
    key: u64,
    control: bool,
) -> Result<(), String> {
    let mut c = editor
        .state()
        .composites
        .get(&id)
        .cloned()
        .ok_or("Missing composite")?;
    let map = if control {
        &mut c.controls
    } else {
        &mut c.ports
    };
    if !control {
        if let Some(e) = map.get(&key) {
            let owned = kabl_core::composite::leaves(editor.state(), id);
            if editor.state().cables.values().any(|c| {
                (c.from == e.target || c.to == e.target)
                    && owned.contains(&c.from.module_id()) != owned.contains(&c.to.module_id())
            }) {
                return Err(
                    "Boundary port is connected; disconnect its external routes first".into(),
                );
            }
        }
    }
    map.remove(&key);
    set(editor, id, c)
}
fn snapshot(p: &PatchState, root: CompositeId) -> Result<Package, String> {
    if !p.composites.contains_key(&root) {
        return Err("Missing composite".into());
    }
    let groups = kabl_core::composite::descendants(p, root);
    let leaves = kabl_core::composite::leaves(p, root);
    let mut snapshot = p.clone();
    snapshot.modules.retain(|id, _| leaves.contains(id));
    snapshot.labels.retain(|id, _| leaves.contains(id));
    snapshot
        .cables
        .retain(|_, c| leaves.contains(&c.from.module_id()) && leaves.contains(&c.to.module_id()));
    snapshot.composites.retain(|id, _| groups.contains(id));
    snapshot.composites.get_mut(&root).unwrap().parent = None;
    Ok(Package {
        schema: 1,
        root,
        patch: snapshot,
    })
}
pub fn package(p: &PatchState, root: CompositeId) -> Result<Package, String> {
    let pkg = snapshot(p, root)?;
    compile(&pkg.patch)?;
    Ok(pkg)
}
pub fn decode(bytes: &[u8]) -> Result<Package, String> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err("Composite package size limit".into());
    }
    let pkg: Package = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if pkg.schema != 1
        || !pkg.patch.composites.contains_key(&pkg.root)
        || pkg.patch.composites[&pkg.root].parent.is_some()
        || kabl_core::composite::descendants(&pkg.patch, pkg.root).len()
            != pkg.patch.composites.len()
        || kabl_core::composite::leaves(&pkg.patch, pkg.root).len() != pkg.patch.modules.len()
    {
        return Err("Unsupported or incomplete composite package".into());
    }
    compile(&pkg.patch)?;
    Ok(pkg)
}
pub fn insert(editor: &mut PatchEditor, pkg: &Package) -> Result<CompositeId, String> {
    let pkg = decode(&serde_json::to_vec(pkg).map_err(|e| e.to_string())?)?;
    insert_inner(editor, &pkg, false)
}
fn insert_inner(
    editor: &mut PatchEditor,
    pkg: &Package,
    external: bool,
) -> Result<CompositeId, String> {
    kabl_engine::compile::validate_composites(&pkg.patch)?;
    let (mut next_module, mut next_cable) = editor.reserved_ids();
    let mut next_group = next_group(editor)?;
    let mut modules = BTreeMap::new();
    let mut groups = BTreeMap::new();
    for &id in pkg.patch.modules.keys() {
        modules.insert(id, next_module);
        next_module = next_module
            .checked_add(1)
            .filter(|n| *n < u64::MAX)
            .ok_or("Module identity exhausted")?;
    }
    for &id in pkg.patch.composites.keys() {
        groups.insert(id, next_group);
        next_group = next_group
            .checked_add(1)
            .filter(|n| *n < u64::MAX)
            .ok_or("Composite identity exhausted")?;
    }
    let remap = |t: &PortRef| -> PortRef {
        match t {
            PortRef::Module { id, port } => PortRef::Module {
                id: modules[id],
                port: port.clone(),
            },
            PortRef::Param { id, param } => PortRef::Param {
                id: modules[id],
                param: param.clone(),
            },
        }
    };
    let mut p = editor.state().clone();
    // Put the copy below existing modules so opening either instance never covers the other.
    let old_top = pkg
        .patch
        .modules
        .values()
        .map(|m| m.pos.y)
        .fold(f32::INFINITY, f32::min);
    let bottom = p.modules.values().map(|m| m.pos.y).fold(-340.0, f32::max);
    let offset_y = bottom + 340.0 - old_top;
    let mut pin_order = crate::perform::pins(&p)
        .iter()
        .map(|pin| pin.order)
        .fold(-1.0, f32::max)
        + 1.0;
    for (&id, m) in &pkg.patch.modules {
        let mut m = m.clone();
        m.pos.y += offset_y;
        let mut params = BTreeMap::new();
        for (mut key, mut value) in m.params {
            if key.starts_with("cc.") || key.starts_with("btn.") {
                continue;
            }
            if key.starts_with("pin.") {
                value = pin_order;
                pin_order += 1.0;
            }
            if key == crate::banks::LAUNCH_CLOCK
                || (m.kind == "cues" && key.starts_with("cue") && key.ends_with(".clock"))
            {
                let old = value as u64;
                let fresh = modules
                    .get(&old)
                    .copied()
                    .or_else(|| external.then_some(old))
                    .ok_or_else(|| format!("Unresolved clock #{old}; include dependency"))?;
                value = fresh as f32;
                if value as u64 != fresh {
                    return Err("Clock identity cannot be represented exactly".into());
                }
            }
            if m.kind == "cues" {
                if let Some((prefix, old)) = key.split_once(".seq.") {
                    let old = old
                        .parse::<u64>()
                        .map_err(|_| "Invalid sequencer dependency")?;
                    key = format!(
                        "{prefix}.seq.{}",
                        modules
                            .get(&old)
                            .copied()
                            .or_else(|| external.then_some(old))
                            .ok_or("Unresolved sequencer dependency; include module")?
                    );
                }
            }
            params.insert(key, value);
        }
        m.params = params;
        p.modules.insert(modules[&id], m);
        if let Some(labels) = pkg.patch.labels.get(&id) {
            p.labels.insert(modules[&id], labels.clone());
        }
    }
    for c in pkg.patch.cables.values() {
        let mut c = c.clone();
        c.from = remap(&c.from);
        c.to = remap(&c.to);
        p.cables.insert(next_cable, c);
        next_cable = next_cable
            .checked_add(1)
            .filter(|n| *n < u64::MAX)
            .ok_or("Cable identity exhausted")?;
    }
    for (&id, c) in &pkg.patch.composites {
        let mut c = c.clone();
        c.parent = c.parent.map(|id| groups[&id]);
        c.members = c.members.iter().map(|id| modules[id]).collect();
        c.pos.y += offset_y;
        for e in c.controls.values_mut().chain(c.ports.values_mut()) {
            e.target = remap(&e.target)
        }
        p.composites.insert(groups[&id], c);
    }
    commit(editor, p)?;
    Ok(groups[&pkg.root])
}
pub fn duplicate(editor: &mut PatchEditor, id: CompositeId) -> Result<CompositeId, String> {
    insert_inner(editor, &snapshot(editor.state(), id)?, true)
}

pub fn publish(p: &PatchState, id: CompositeId, root: &Path) -> Result<std::path::PathBuf, String> {
    let mut pkg = package(p, id)?;
    let identity = pkg.patch.composites[&id].definition.id.clone();
    if !identity
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err("Invalid library definition ID".into());
    }
    let dir = root.join("composites").join(&identity);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut version = 1u32;
    while dir.join(format!("v{version}.json")).exists() {
        version = version.checked_add(1).ok_or("Version exhausted")?;
    }
    pkg.patch
        .composites
        .get_mut(&id)
        .unwrap()
        .definition
        .version = version;
    let bytes = serde_json::to_vec_pretty(&pkg).map_err(|e| e.to_string())?;
    decode(&bytes)?;
    let target = dir.join(format!("v{version}.json"));
    let stage = dir.join(format!(
        ".v{version}-{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    use std::io::Write;
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&stage)
            .map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        decode(&std::fs::read(&stage).map_err(|e| e.to_string())?)?;
        std::fs::hard_link(&stage, &target).map_err(|e| e.to_string())?;
        Ok(target)
    })();
    let _ = std::fs::remove_file(stage);
    result
}
pub fn entries(root: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    if let Ok(dirs) = std::fs::read_dir(root.join("composites")) {
        for dir in dirs.flatten().take(128) {
            if let Ok(entries) = std::fs::read_dir(dir.path()) {
                for entry in entries.flatten().take(128) {
                    if entry.path().extension().is_some_and(|x| x == "json") {
                        files.push(entry.path())
                    }
                }
            }
        }
    }
    files.sort();
    files
}
pub fn direction(p: &PatchState, t: &PortRef) -> Option<PortDirection> {
    match t {
        PortRef::Param { .. } => Some(PortDirection::Input),
        PortRef::Module { id, port } => registry::info_for(&p.modules.get(id)?.kind)?
            .ports
            .iter()
            .find(|p| p.name == port)
            .map(|p| p.direction),
    }
}

#[derive(Default)]
pub struct View {
    pub panel: bool,
    pub open: BTreeSet<CompositeId>,
    pub members: BTreeSet<ModuleId>,
    pub children: BTreeSet<CompositeId>,
    pub name: String,
    editor: u64,
}
pub fn panel(editor: &mut PatchEditor, view: &mut crate::UiState, ctx: &egui::Context) {
    if view.composites.editor != editor.instance() {
        view.composites = View {
            editor: editor.instance(),
            ..View::default()
        };
    }
    if !view.composites.panel {
        return;
    }
    let mut open = true;
    egui::Window::new("Composites · instance tools")
        .open(&mut open).default_width(380.0).default_height(520.0).vscroll(true)
        .show(ctx, |ui| {
            if let Some(message) = &view.last_message { ui.label(message); }
            ui.label("Select ungrouped modules, or top-level composites to nest.");
            let state = editor.state().clone();
            for (&id, m) in &state.modules {
                if state.composites.values().any(|c| c.members.contains(&id)) { continue; }
                let mut selected = view.composites.members.contains(&id);
                let r = ui.checkbox(&mut selected, format!("{} #{id}", m.kind));
                view.record(format!("composite:select:{id}"), r.rect);
                if r.changed() {
                    if selected { view.composites.members.insert(id); }
                    else { view.composites.members.remove(&id); }
                }
            }
            for (&id, c) in state.composites.iter().filter(|(_, c)| c.parent.is_none()) {
                let mut selected = view.composites.children.contains(&id);
                if ui.checkbox(&mut selected, format!("{} · composite #{id}", c.name)).changed() {
                    if selected { view.composites.children.insert(id); }
                    else { view.composites.children.remove(&id); }
                }
            }
            let r = ui.text_edit_singleline(&mut view.composites.name);
            view.record("composite:name".into(), r.rect);
            let r = ui.button("Encapsulate selection");
            view.record("composite:encapsulate".into(), r.rect);
            if r.clicked() {
                match encapsulate(editor, view.composites.members.clone(), view.composites.children.clone(), &view.composites.name) {
                    Ok(_) => {
                        view.composites.members.clear(); view.composites.children.clear();
                        view.selected_module = None; view.inspected = None;
                    }
                    Err(e) => view.last_message = Some(e),
                }
            }
            ui.separator();
            for (&id, c) in &state.composites {
                let r = ui.push_id(id, |ui| {
                    ui.collapsing(format!("{} · instance #{id}", c.name), |ui| {
                        ui.label(&c.help);
                        ui.label(format!("Embedded definition {} v{}", c.definition.id, c.definition.version));
                        ui.horizontal(|ui| {
                            if ui.button("Open internals").clicked() { view.composites.open.insert(id); }
                            if ui.button("Close").clicked() {
                                view.composites.open.remove(&id); view.selected_module = None; view.inspected = None;
                            }
                            let r = ui.button("Duplicate");
                            view.record(format!("composite:{id}:duplicate"), r.rect);
                            if r.clicked() {
                                view.last_message = Some(match duplicate(editor, id) {
                                    Ok(_) => "Copy created: external cables, CC/buttons and native automation assignments omitted; pins copied with new positions.".into(),
                                    Err(e) => e,
                                });
                            }
                        });
                        let mut renamed = c.clone();
                        ui.text_edit_singleline(&mut renamed.name);
                        ui.text_edit_multiline(&mut renamed.help);
                        if renamed != *c {
                            if let Err(e) = set(editor, id, renamed) { view.last_message = Some(e); }
                        }
                        ui.label("Expose a real target (IDs stay fixed when renamed):");
                        for leaf in kabl_core::composite::leaves(&state, id) {
                            let m = &state.modules[&leaf];
                            if let Some(info) = registry::info_for(&m.kind) {
                                ui.collapsing(format!("{} #{leaf}", info.name), |ui| {
                                    for param in info.params {
                                        if ui.small_button(format!("Control: {}", param.name)).clicked() {
                                            if let Err(e) = add_exposure(editor, id, PortRef::Param { id: leaf, param: param.name.into() }, true) { view.last_message = Some(e); }
                                        }
                                    }
                                    for port in info.ports {
                                        if ui.small_button(format!("Port: {}", port.name)).clicked() {
                                            if let Err(e) = add_exposure(editor, id, PortRef::Module { id: leaf, port: port.name.into() }, false) { view.last_message = Some(e); }
                                        }
                                    }
                                    for param in info.params {
                                        if ui.small_button(format!("Modulation input: {}", param.name)).clicked() {
                                            if let Err(e) = add_exposure(editor, id, PortRef::Param { id: leaf, param: param.name.into() }, false) { view.last_message = Some(e); }
                                        }
                                    }
                                });
                            }
                        }
                        for (control, map) in [(true, &c.controls), (false, &c.ports)] {
                            for (&key, e) in map {
                                ui.push_id((control, key), |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(format!("ID {key}"));
                                        let mut label = e.label.clone();
                                        if ui.text_edit_singleline(&mut label).changed() {
                                            let mut latest = editor.state().composites[&id].clone();
                                            let map = if control { &mut latest.controls } else { &mut latest.ports };
                                            map.get_mut(&key).unwrap().label = label;
                                            if let Err(e) = set(editor, id, latest) { view.last_message = Some(e); }
                                        }
                                        if ui.small_button("Remove").clicked() {
                                            if let Err(e) = remove_exposure(editor, id, key, control) { view.last_message = Some(e); }
                                        }
                                    });
                                });
                            }
                        }
                        let r = ui.button("Publish a new immutable version");
                        view.record(format!("composite:{id}:publish"), r.rect);
                        if r.clicked() {
                            let root = view.library.as_ref().map(|l| l.user_root.clone()).unwrap_or_else(crate::library::default_user_root);
                            view.last_message = Some(match publish(editor.state(), id, &root) {
                                Ok(path) => format!("Published {}. Existing songs/instances unchanged.", path.display()),
                                Err(e) => e,
                            });
                        }
                    }).header_response
                });
                view.record(format!("composite:{id}:tools"), r.inner.rect);
            }
            ui.separator(); ui.label("Insert a library version as an independent embedded instance");
            let root = view.library.as_ref().map(|l| l.user_root.clone()).unwrap_or_else(crate::library::default_user_root);
            for path in entries(&root) {
                let label = path.strip_prefix(&root).unwrap_or(&path).display().to_string();
                if ui.button(label).clicked() {
                    let result = std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| decode(&b)).and_then(|p| insert(editor, &p));
                    view.last_message = Some(match result {
                        Ok(id) => format!("Inserted instance #{id}; external cables and controller mappings omitted."),
                        Err(e) => e,
                    });
                }
            }
        });
    view.composites.panel = open;
}
