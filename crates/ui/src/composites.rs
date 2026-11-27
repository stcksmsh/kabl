//! Instance editing and immutable portable definitions, entirely on the control/UI side.
use crate::PatchEditor;
use kabl_core::{
    CableId, Composite, CompositeId, Definition, Exposure, ModuleId, Op, PatchLog, PatchState,
    PortRef, Source,
};
use kabl_modules::{registry, PortDirection};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
// Leave room for complete host-state lanes, native bases and envelope fields.
const MAX_BYTES: usize = 2 * 1024 * 1024 - 16 * 1024;
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
            c.params.len() > kabl_cables::MAX_CABLE_PARAMS
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
    // A face draws a cable parameter only as a route's amount.
    for e in p.composites.values().flat_map(|c| c.controls.values()) {
        if matches!(e.target, PortRef::CableParam { .. }) {
            crate::panels::param_info(p, &e.target)
                .map_err(|err| format!("Control \"{}\": {err}", e.label))?;
        }
    }
    crate::panels::validate(p)?;
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
fn next_group(log: &PatchLog) -> Result<u64, String> {
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
    let mut max = log.state().composites.keys().copied().max().unwrap_or(0);
    for entry in log.all_entries() {
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
/// Adds one exposure feeding `targets`, less those an existing one already feeds; nothing when
/// none is left. `label` defaults to the first target's binding label.
fn expose(
    c: &mut Composite,
    targets: Vec<PortRef>,
    label: Option<String>,
    control: bool,
) -> Result<(), String> {
    let map = if control {
        &mut c.controls
    } else {
        &mut c.ports
    };
    let mut fresh: Vec<PortRef> = Vec::new();
    for t in targets {
        if !fresh.contains(&t) && !map.values().any(|e| e.targets().any(|x| *x == t)) {
            fresh.push(t);
        }
    }
    if fresh.is_empty() {
        return Ok(());
    }
    let target = fresh.remove(0);
    let id = c.next_interface;
    c.next_interface = id
        .checked_add(1)
        .filter(|n| *n < u64::MAX)
        .ok_or("Interface identity exhausted")?;
    map.insert(
        id,
        Exposure {
            label: label.unwrap_or_else(|| binding_label(&target)),
            target,
            also: fresh,
        },
    );
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
    let id = next_group(editor.log())?;
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
    let mut c=Composite{name:name.trim().into(),help:"Open internals to inspect the real modules and routes. Editing affects this instance only.".into(),definition:definition(id),parent:None,members,pos,ports:BTreeMap::new(),controls:BTreeMap::new(),next_interface:1,panel:None,snapshots:Vec::new()};
    // One port per outside source (fan-in), and one per inside output, in cable order.
    let mut groups: Vec<(Option<PortRef>, Vec<PortRef>)> = Vec::new();
    for (cid, inside) in crossings(&p, &all) {
        let from = &p.cables[&cid].from;
        let source = (*from != inside).then(|| from.clone());
        match groups
            .iter_mut()
            .find(|(s, _)| source.is_some() && *s == source)
        {
            Some((_, targets)) => targets.push(inside),
            None => groups.push((source, vec![inside])),
        }
    }
    // A source feeding more inputs than one port holds spreads over several ports.
    for (_, targets) in groups {
        for chunk in targets.chunks(kabl_core::composite::MAX_ALSO + 1) {
            expose(&mut c, chunk.to_vec(), None, false)?;
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
                vec![PortRef::Param {
                    id: pin.id,
                    param: pin.key.clone(),
                }],
                Some(crate::perform::pin_label(&p, &pin)),
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
/// Cables that cross the boundary of `inside`, in cable-id order, each with its inside end. A
/// route into a cable parameter ends where that cable ends, so it crosses when its source and
/// that module are on different sides; its inside end is then the cable parameter itself.
fn crossings(p: &PatchState, inside: &BTreeSet<ModuleId>) -> Vec<(kabl_core::CableId, PortRef)> {
    let mut out = Vec::new();
    for (&cid, cable) in &p.cables {
        let a = cable.from.module_id().is_some_and(|m| inside.contains(&m));
        let b = p.end_module(cid).is_some_and(|m| inside.contains(&m));
        if a != b {
            out.push((cid, if a { &cable.from } else { &cable.to }.clone()));
        }
    }
    out
}
/// Wraps the leaf modules `members` into a new top-level composite whose whole interface is
/// curated, and appends it to `log` as one `SetComposite` entry, leaving every earlier entry,
/// leaf id and cable id as it was. For patch generators: the same `log` and arguments always
/// append the same bytes. The entry reuses the last entry's timestamp, and the composite id is
/// the next one `log` has never used.
///
/// `ports` then `controls` become interface keys `1..=n` in the order given (so a `panel` can
/// place them). Nothing is exposed that the caller did not list. Refused, with nothing
/// appended: a target outside `members`, a control that is not a parameter or has several
/// targets, a repeated label or target, a boundary crossing no port covers and a pinned
/// parameter no control covers. A port lists every input one outside source feeds in its
/// `target` and `also` (fan-in). A cable parameter belongs to the module its cable ends on; it
/// can be a port, a control (a route's `amount`, drawn as a knob) or both.
pub fn encapsulate_with(
    log: &mut PatchLog,
    name: &str,
    definition: Definition,
    members: BTreeSet<ModuleId>,
    ports: &[Exposure],
    controls: &[Exposure],
    panel: Option<kabl_core::panel::Panel>,
) -> Result<CompositeId, String> {
    let p = log.state();
    if members.is_empty() {
        return Err("Select modules first".into());
    }
    for m in &members {
        if !p.modules.contains_key(m) {
            return Err(format!("Module {m} does not exist"));
        }
        if p.composites.values().any(|c| c.members.contains(m)) {
            return Err(format!("Module {m} is already owned by a composite"));
        }
    }
    let mut labels = BTreeSet::new();
    for (list, control) in [(ports, false), (controls, true)] {
        let mut targets = Vec::new();
        for e in list {
            if let Some(t) = e
                .targets()
                .find(|t| !kabl_core::composite::owner(p, t).is_some_and(|m| members.contains(&m)))
            {
                return Err(format!(
                    "Exposure \"{}\": target {} is not inside the wrapped modules",
                    e.label,
                    end(t)
                ));
            }
            if control && (!e.target.is_route() || !e.also.is_empty()) {
                return Err(format!(
                    "Control \"{}\": {} is not a parameter",
                    e.label,
                    end(&e.target)
                ));
            }
            if !labels.insert(e.label.as_str()) {
                return Err(format!("Two exposures are labelled \"{}\"", e.label));
            }
            for t in e.targets() {
                if targets.contains(&t) {
                    return Err(format!(
                        "Exposure \"{}\": {} is exposed twice",
                        e.label,
                        end(t)
                    ));
                }
                targets.push(t);
            }
        }
    }
    let crossing = crossings(p, &members);
    let uncovered: Vec<String> = crossing
        .iter()
        .filter(|(_, inside)| !ports.iter().any(|e| e.targets().any(|t| t == inside)))
        .map(|(cid, inside)| {
            let cable = &p.cables[cid];
            let outside = if cable.from == *inside {
                &cable.to
            } else {
                &cable.from
            };
            format!(
                "cable {cid} ({} inside, {} outside)",
                end(inside),
                end(outside)
            )
        })
        .collect();
    if !uncovered.is_empty() {
        return Err(format!("No curated port covers {}", uncovered.join("; ")));
    }
    // Interactive encapsulation keeps pins as controls; a curated interface must list them.
    for pin in crate::perform::pins(p)
        .into_iter()
        .filter(|pin| members.contains(&pin.id))
    {
        let target = PortRef::Param {
            id: pin.id,
            param: pin.key.clone(),
        };
        if registry::info_for(&p.modules[&pin.id].kind)
            .is_some_and(|i| i.params.iter().any(|param| param.name == pin.key))
            && !controls.iter().any(|e| e.target == target)
        {
            return Err(format!(
                "Pinned parameter {} is not a curated control",
                end(&target)
            ));
        }
    }
    let id = next_group(log)?;
    let mut c = Composite {
        name: name.trim().into(),
        help: "Open internals to inspect the real modules and routes. Editing affects this \
               instance only."
            .into(),
        definition,
        parent: None,
        pos: p.modules[members.first().unwrap()].pos,
        members,
        ports: BTreeMap::new(),
        controls: BTreeMap::new(),
        next_interface: 1,
        panel: panel.map(Box::new),
        snapshots: Vec::new(),
    };
    for (list, control) in [(ports, false), (controls, true)] {
        for e in list {
            expose(
                &mut c,
                e.targets().cloned().collect(),
                Some(e.label.clone()),
                control,
            )?;
        }
    }
    let mut candidate = p.clone();
    candidate.composites.insert(id, c.clone());
    compile(&candidate)?;
    let t_ms = log.entries().last().map_or(0, |e| e.t_ms);
    log.append_new(Op::SetComposite { id, value: Some(c) }, t_ms, Source::User);
    Ok(id)
}
/// `#<module>.<port or param>`, as refusals name the ends of a crossing.
fn end(t: &PortRef) -> String {
    match t {
        PortRef::Module { id, port } => format!("#{id}.{port}"),
        PortRef::Param { id, param } => format!("#{id}.{param}"),
        PortRef::CableParam { cable, param } => format!("cable #{cable}.{param}"),
    }
}
pub fn binding_label(t: &PortRef) -> String {
    match t {
        PortRef::Module { id, port } => format!("{port} · #{id}"),
        PortRef::Param { id, param } => format!("{param} · #{id}"),
        PortRef::CableParam { cable, param } => format!("{param} · cable #{cable}"),
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
    expose(&mut c, vec![target], None, control)?;
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
            let state = editor.state();
            if state.cables.iter().any(|(cid, c)| {
                e.targets().any(|t| c.from == *t || c.to == *t)
                    && c.from.module_id().is_some_and(|m| owned.contains(&m))
                        != state.end_module(*cid).is_some_and(|m| owned.contains(&m))
            }) {
                return Err(
                    "Boundary port is connected; disconnect its external routes first".into(),
                );
            }
        }
    }
    map.remove(&key);
    if let Some(panel) = &mut c.panel {
        panel.placements.remove(&key);
    }
    for s in &mut c.snapshots {
        s.values.remove(&key);
    }
    set(editor, id, c)
}
/// The composite `root` with its members and the cables between them. A port bound to the
/// parameter of a cable that crosses the boundary is dropped, because that cable stays behind;
/// duplicate and publish go through here, so their copies lack that port.
fn snapshot(p: &PatchState, root: CompositeId) -> Result<Package, String> {
    if !p.composites.contains_key(&root) {
        return Err("Missing composite".into());
    }
    let groups = kabl_core::composite::descendants(p, root);
    let leaves = kabl_core::composite::leaves(p, root);
    let mut snapshot = p.clone();
    snapshot.modules.retain(|id, _| leaves.contains(id));
    snapshot.labels.retain(|id, _| leaves.contains(id));
    let keep: Vec<kabl_core::CableId> = snapshot
        .cables
        .iter()
        .filter(|(&id, c)| {
            c.from.module_id().is_some_and(|m| leaves.contains(&m))
                && snapshot.end_module(id).is_some_and(|m| leaves.contains(&m))
        })
        .map(|(&id, _)| id)
        .collect();
    snapshot.cables.retain(|id, _| keep.contains(id));
    // A route into a cable that crosses the boundary stays behind with that cable, and so does
    // a port or control bound to that cable's parameter.
    loop {
        let ids: BTreeSet<kabl_core::CableId> = snapshot.cables.keys().copied().collect();
        let before = ids.len();
        snapshot.cables.retain(
            |_, c| !matches!(&c.to, PortRef::CableParam { cable, .. } if !ids.contains(cable)),
        );
        if snapshot.cables.len() == before {
            break;
        }
    }
    let ids: BTreeSet<kabl_core::CableId> = snapshot.cables.keys().copied().collect();
    for c in snapshot.composites.values_mut() {
        let mut gone = Vec::new();
        for (&key, e) in c.ports.iter_mut().chain(c.controls.iter_mut()) {
            if !e.retain_targets(
                |t| !matches!(t, PortRef::CableParam { cable, .. } if !ids.contains(cable)),
            ) {
                gone.push(key);
            }
        }
        for key in gone {
            c.ports.remove(&key);
            c.controls.remove(&key);
            if let Some(panel) = &mut c.panel {
                panel.placements.remove(&key);
            }
            for s in &mut c.snapshots {
                s.values.remove(&key);
            }
        }
    }
    snapshot.composites.retain(|id, _| groups.contains(id));
    snapshot.composites.get_mut(&root).unwrap().parent = None;
    // Like the patch format, a package moves to its next schema only when it uses what an older
    // reader refuses, so that reader says "unsupported" rather than "invalid public binding".
    let uses = |f: fn(&Composite) -> bool| snapshot.composites.values().any(f);
    let schema = if uses(kabl_core::composite::needs_faces) {
        3
    } else if uses(|c| {
        kabl_core::composite::binds_cable_param(c) || kabl_core::composite::fans_in(c)
    }) {
        2
    } else {
        1
    };
    Ok(Package {
        schema,
        root,
        patch: snapshot,
    })
}
pub fn package(p: &PatchState, root: CompositeId) -> Result<Package, String> {
    let mut pkg = snapshot(p, root)?;
    // A snapshot clock outside the composite stays behind; the snapshot jumps until the
    // author picks a clock after insert.
    let modules = &pkg.patch.modules;
    for c in pkg.patch.composites.values_mut() {
        for s in &mut c.snapshots {
            s.clock = s.clock.filter(|m| modules.contains_key(m));
        }
    }
    compile(&pkg.patch)?;
    Ok(pkg)
}
pub fn decode(bytes: &[u8]) -> Result<Package, String> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err("Composite package size limit".into());
    }
    let pkg: Package = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if !(1..=3).contains(&pkg.schema)
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
    let (p, id) = inserted(editor, pkg, external)?;
    commit(editor, p)?;
    Ok(id)
}
/// The patch with a fresh-id copy of `pkg` added, and the copy's root.
fn inserted(
    editor: &PatchEditor,
    pkg: &Package,
    external: bool,
) -> Result<(PatchState, CompositeId), String> {
    // A duplicate's snapshot clocks may sit outside the copy, where the package alone cannot
    // resolve them; the patch they land in is validated on commit.
    let mut own = pkg.patch.clone();
    for c in own.composites.values_mut() {
        for s in &mut c.snapshots {
            s.clock = s.clock.filter(|m| own.modules.contains_key(m));
        }
    }
    kabl_engine::compile::validate_composites(&own)?;
    let (mut next_module, mut next_cable) = editor.reserved_ids();
    let mut next_group = next_group(editor.log())?;
    let mut modules = BTreeMap::new();
    let mut groups = BTreeMap::new();
    for &id in pkg.patch.modules.keys() {
        modules.insert(id, next_module);
        next_module = next_module
            .checked_add(1)
            .filter(|n| *n < u64::MAX)
            .ok_or("Module identity exhausted")?;
    }
    // Cables are numbered in the order they are inserted below, so routes into a cable's
    // parameters and composite bindings of those parameters follow the copy.
    let mut cables = BTreeMap::new();
    for (n, &id) in pkg.patch.cables.keys().enumerate() {
        cables.insert(id, next_cable.saturating_add(n as u64));
    }
    let targets = pkg.patch.cables.values().map(|c| &c.to).chain(
        pkg.patch
            .composites
            .values()
            .flat_map(|c| c.ports.values().chain(c.controls.values()))
            .flat_map(Exposure::targets),
    );
    for t in targets {
        if let PortRef::CableParam { cable, .. } = t {
            if !cables.contains_key(cable) {
                return Err(format!(
                    "Package routes into cable {cable}, which it does not contain"
                ));
            }
        }
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
            PortRef::CableParam { cable, param } => PortRef::CableParam {
                cable: cables[cable],
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
            let autogenerated = e.label == binding_label(&e.target);
            e.target = remap(&e.target);
            e.also = e.also.iter().map(remap).collect();
            if autogenerated {
                e.label = binding_label(&e.target);
            }
        }
        for s in &mut c.snapshots {
            // Duplicate keeps a clock outside the copy; an insert has none (export drops it).
            s.clock = s
                .clock
                .and_then(|m| modules.get(&m).copied().or(external.then_some(m)));
        }
        if let Some(panel) = &mut c.panel {
            for pl in panel.placements.values_mut() {
                match &mut pl.source {
                    Some(kabl_core::panel::Source::Module { id, .. }) => *id = modules[id],
                    Some(kabl_core::panel::Source::Face { composite, .. }) => {
                        *composite = groups[composite]
                    }
                    _ => {}
                }
            }
        }
        p.composites.insert(groups[&id], c);
    }
    Ok((p, groups[&pkg.root]))
}
pub fn duplicate(editor: &mut PatchEditor, id: CompositeId) -> Result<CompositeId, String> {
    insert_inner(editor, &snapshot(editor.state(), id)?, true)
}

pub fn publish(p: &PatchState, id: CompositeId, root: &Path) -> Result<std::path::PathBuf, String> {
    let mut pkg = package(p, id)?;
    let identity = pkg.patch.composites[&id].definition.id.clone();
    if identity.starts_with(FACTORY_PREFIX) {
        return Err(format!(
            "{identity} is a read-only factory definition; the instance in this patch stays editable"
        ));
    }
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

/// Factory definition ids are `kabl.<name>`. User ids never contain a dot (see `publish`), so
/// neither kind can shadow or overwrite the other.
pub const FACTORY_PREFIX: &str = "kabl.";
/// One read-only factory definition, `<bank>/<name>/v<N>.json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FactoryEntry {
    pub path: std::path::PathBuf,
    pub definition: Definition,
}
/// The factory composite bank: `KABL_FACTORY_COMPOSITES_DIR`, else
/// the installed layout's `factory-composites` (`paths::factory_candidates_in`), else the source checkout's `composites/` (a
/// development build). Not `share/kabl/composites`: installed under `~/.local` that is the user
/// library.
pub fn find_factory_bank() -> Option<std::path::PathBuf> {
    crate::library::paths::find_factory(
        "KABL_FACTORY_COMPOSITES_DIR",
        "factory-composites",
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../composites"),
        false,
    )
}
/// Every `<name>/v<N>.json` in the bank, sorted by id then version. The id and version come
/// from the path alone, so they are the same in every load and every process. A missing bank
/// lists nothing.
pub fn factory_entries(bank: &Path) -> Vec<FactoryEntry> {
    let mut out = Vec::new();
    for dir in std::fs::read_dir(bank)
        .into_iter()
        .flatten()
        .flatten()
        .take(128)
    {
        let name = dir.file_name();
        let Some(name) = name.to_str().filter(|n| {
            !n.is_empty()
                && n.len() <= 64
                && n.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        }) else {
            continue;
        };
        for file in std::fs::read_dir(dir.path())
            .into_iter()
            .flatten()
            .flatten()
            .take(128)
        {
            let version = file.file_name().to_str().and_then(|f| {
                let digits = f.strip_prefix('v')?.strip_suffix(".json")?;
                // `v01` would alias `v1`.
                digits
                    .parse::<u32>()
                    .ok()
                    .filter(|v| *v > 0 && v.to_string() == digits)
            });
            if let Some(version) = version {
                out.push(FactoryEntry {
                    path: file.path(),
                    definition: Definition {
                        id: format!("{FACTORY_PREFIX}{name}"),
                        version,
                    },
                });
            }
        }
    }
    out.sort_by(|a, b| a.definition.cmp(&b.definition));
    out
}
/// Reads a factory definition. The file's root definition must match its path, so the id an
/// instance embeds is the one the bank lists.
pub fn read_factory(entry: &FactoryEntry) -> Result<Package, String> {
    let pkg = crate::panels::read_package(&entry.path)?;
    let found = &pkg.patch.composites[&pkg.root].definition;
    if *found != entry.definition {
        return Err(format!(
            "{}: root definition is {} v{}, its place in the bank says {} v{}",
            entry.path.display(),
            found.id,
            found.version,
            entry.definition.id,
            entry.definition.version
        ));
    }
    Ok(pkg)
}
pub fn direction(p: &PatchState, t: &PortRef) -> Option<PortDirection> {
    match t {
        PortRef::Param { .. } | PortRef::CableParam { .. } => Some(PortDirection::Input),
        PortRef::Module { id, port } => registry::info_for(&p.modules.get(id)?.kind)?
            .ports
            .iter()
            .find(|p| p.name == port)
            .map(|p| p.direction),
    }
}

/// Whether composite `id` is drawn as a face: closed, with every enclosing composite open.
fn face_shown(p: &PatchState, open: &BTreeSet<CompositeId>, id: CompositeId) -> bool {
    let mut at = Some(id);
    for depth in 0..=8 {
        let Some(c) = at.and_then(|id| p.composites.get(&id)) else {
            return depth > 0;
        };
        let id = at.unwrap();
        if open.contains(&id) == (depth == 0) {
            return false;
        }
        at = c.parent;
    }
    false
}

/// What a cable into `target` patches when `target` is reached through a composite face: every
/// target of the public port behind it. Elsewhere, `target` alone.
pub fn face_targets(
    p: &PatchState,
    open: &BTreeSet<CompositeId>,
    target: &PortRef,
) -> Vec<PortRef> {
    p.composites
        .iter()
        .filter(|(&id, _)| face_shown(p, open, id))
        .flat_map(|(_, c)| c.ports.values())
        .find(|e| e.targets().any(|t| t == target))
        .map_or_else(|| vec![target.clone()], |e| e.targets().cloned().collect())
}

/// Ops that patch `from` into each of `targets`, with ids from `first`. A jack input holds one
/// cable and a port holds one route per source into each parameter, so what a new cable would
/// duplicate is removed first, except the cables in `gone`, which go anyway.
fn connect_ops(
    p: &PatchState,
    from: &PortRef,
    targets: Vec<PortRef>,
    first: CableId,
    gone: &[CableId],
) -> Vec<Op> {
    let mut ops = Vec::new();
    for (next, to) in (first..).zip(targets) {
        let module = matches!(to, PortRef::Module { .. });
        ops.extend(
            p.cables
                .iter()
                .filter(|(id, c)| !gone.contains(id) && c.to == to && (module || c.from == *from))
                .map(|(&id, _)| Op::Disconnect { id }),
        );
        ops.push(Op::Connect {
            id: next,
            from: from.clone(),
            to,
        });
    }
    ops
}

/// The cables a face port's wiring holds alongside `cable`: for each other target of the port,
/// the first cable from the same source. Empty when `cable` does not enter a closed face.
fn siblings(p: &PatchState, open: &BTreeSet<CompositeId>, cable: CableId) -> Vec<CableId> {
    let Some(c) = p.cables.get(&cable) else {
        return Vec::new();
    };
    face_targets(p, open, &c.to)
        .iter()
        .filter(|t| **t != c.to)
        .filter_map(|t| {
            p.cables
                .iter()
                .find(|(_, o)| o.from == c.from && o.to == *t)
                .map(|(&id, _)| id)
        })
        .collect()
}

/// Connects `from` to every target of the face port behind `to` as one undo step. A jack input
/// holds one cable, so a cable already in one is replaced, as `PatchEditor::connect` does; a
/// route from the same source into a parameter target is replaced too, so repeated drops do not
/// stack. Returns the cable into the first target.
pub fn connect(
    editor: &mut PatchEditor,
    open: &BTreeSet<CompositeId>,
    from: PortRef,
    to: PortRef,
) -> CableId {
    let targets = face_targets(editor.state(), open, &to);
    if targets.len() == 1 {
        return editor.connect(from, to);
    }
    let first = editor.reserved_ids().1;
    let ops = connect_ops(editor.state(), &from, targets, first, &[]);
    editor.restore_to(ops);
    first
}

/// Removes `cable` and, when it enters a closed composite face, the port's cable from the same
/// source into each other target: one undo step. Anywhere else only `cable` goes.
pub fn disconnect(editor: &mut PatchEditor, open: &BTreeSet<CompositeId>, cable: CableId) {
    let more = siblings(editor.state(), open, cable);
    if more.is_empty() {
        editor.disconnect(cable);
    } else {
        let ops = std::iter::once(cable)
            .chain(more)
            .map(|id| Op::Disconnect { id })
            .collect();
        editor.edit(ops);
    }
}

/// Moves the plug of `old` to `to` (or pulls it when `None`). A plug on a closed face port
/// carries the port's other cables with it, and a drop on one patches every target, as one undo
/// step. Returns the cable into the first target.
pub fn replug(
    editor: &mut PatchEditor,
    open: &BTreeSet<CompositeId>,
    old: CableId,
    to: Option<PortRef>,
) -> Option<CableId> {
    let p = editor.state();
    let c = p.cables.get(&old)?;
    if to.as_ref() == Some(&c.to) {
        return Some(old);
    }
    let more = siblings(p, open, old);
    let targets = to.as_ref().map(|t| face_targets(p, open, t));
    if more.is_empty() && targets.as_ref().is_none_or(|t| t.len() == 1) {
        return editor.replug(old, to);
    }
    let from = c.from.clone();
    let gone: Vec<CableId> = std::iter::once(old).chain(more).collect();
    let mut ops: Vec<Op> = gone.iter().map(|&id| Op::Disconnect { id }).collect();
    let first = editor.reserved_ids().1;
    if let Some(targets) = targets {
        ops.extend(connect_ops(editor.state(), &from, targets, first, &gone));
    }
    let made = ops.iter().any(|o| matches!(o, Op::Connect { .. }));
    editor.restore_to(ops);
    made.then_some(first)
}

/// Cards participate in rack packing without moving saved leaf modules.
pub fn faces(
    p: &PatchState,
    open: &BTreeSet<CompositeId>,
    lay: &crate::rack::Layout,
) -> BTreeMap<CompositeId, egui::Rect> {
    let mut occupied: Vec<_> = lay
        .mods
        .iter()
        .filter(|m| !hidden(p, open, m.id))
        .map(|m| m.full())
        .collect();
    let mut faces = BTreeMap::new();
    for (&id, c) in &p.composites {
        if open.contains(&id) {
            continue;
        }
        let mut parent = c.parent;
        let mut visible = true;
        for _ in 0..8 {
            let Some(id) = parent else {
                break;
            };
            if !open.contains(&id) {
                visible = false;
                break;
            }
            parent = p.composites.get(&id).and_then(|c| c.parent);
        }
        if !visible {
            continue;
        }
        let pos = crate::rack::snap(egui::pos2(c.pos.x, c.pos.y));
        let mut rect = egui::Rect::from_min_size(
            egui::pos2(pos.x, pos.y),
            egui::vec2(
                c.panel.as_ref().map_or(240.0, |p| p.shown_width(false)),
                crate::rack::PANEL_H,
            ),
        );
        loop {
            let next = occupied
                .iter()
                .filter(|r| r.intersects(rect))
                .map(|r| r.right())
                .fold(rect.left(), f32::max);
            if next <= rect.left() {
                break;
            }
            rect = rect.translate(egui::vec2(next - rect.left(), 0.0));
        }
        occupied.push(rect);
        faces.insert(id, rect);
    }
    faces
}
pub fn hidden(p: &PatchState, open: &BTreeSet<CompositeId>, leaf: ModuleId) -> bool {
    p.composites
        .keys()
        .any(|id| !open.contains(id) && kabl_core::composite::leaves(p, *id).contains(&leaf))
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
    if let Some(p) = crate::filebrowser::take(view, crate::filebrowser::Purpose::Package) {
        view.panels.path = p.to_string_lossy().into_owned();
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
                        // A second page would drop the open one's draft without asking.
                        let r = ui.add_enabled(view.panels.page.is_none(), egui::Button::new("Edit face"));
                        view.record(format!("composite:{id}:list-design"), r.rect);
                        if r.clicked() { crate::panels::open(view,id,c); }
                        ui.label(&c.help);
                        ui.label(format!("Embedded definition {} v{}", c.definition.id, c.definition.version));
                        ui.horizontal(|ui| {
                            if ui.button("Open internals").clicked() { view.enter_composite(editor.state(),id); }
                            if ui.button("Close").clicked() && view.scope==Some(id) {view.back();}
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
            ui.separator();
            ui.label("Import complete portable package (definition, panel, artwork and help)");
            ui.horizontal(|ui| {
                let r=ui.text_edit_singleline(&mut view.panels.path);view.record("package:path".into(),r.rect);
                let r=ui.button("Browse");view.record("package:browse".into(),r.rect);
                if r.clicked() {
                    let hint=view.panels.path.clone();
                    crate::filebrowser::open(view,crate::filebrowser::Purpose::Package,"Choose a package",crate::filebrowser::Mode::Open,crate::filebrowser::PACKAGE,"",&hint);
                }
            });
            let r=ui.button("Import package file");view.record("package:import".into(),r.rect);
            if r.clicked() {
                let result=crate::panels::read_package(Path::new(&view.panels.path)).and_then(|pkg|insert(editor,&pkg));
                view.last_message=Some(match result {Ok(id)=>format!("Imported independent instance #{id}"),Err(e)=>e});
            }
            ui.separator(); ui.label("Insert a library version as an independent embedded instance");
            let factory = find_factory_bank().map(|bank| factory_entries(&bank)).unwrap_or_default();
            if !factory.is_empty() {
                ui.label("Factory (read-only)");
                for entry in factory {
                    let Definition { id, version } = &entry.definition;
                    let r = ui.button(format!("{id} v{version}"));
                    view.record(format!("composite:factory:{id}:v{version}"), r.rect);
                    if r.clicked() {
                        let result = read_factory(&entry).and_then(|p| insert(editor, &p));
                        view.last_message = Some(match result {
                            Ok(id) => format!("Inserted instance #{id}; it keeps this version even if the factory bank changes."),
                            Err(e) => e,
                        });
                    }
                }
                ui.label("Your library");
            }
            let root = view.library.as_ref().map(|l| l.user_root.clone()).unwrap_or_else(crate::library::default_user_root);
            for path in entries(&root) {
                let label = path.strip_prefix(&root).unwrap_or(&path).display().to_string();
                if ui.button(label).clicked() {
                    let result = crate::panels::read_package(&path).and_then(|p| insert(editor, &p));
                    view.last_message = Some(match result {
                        Ok(id) => format!("Inserted instance #{id}; external cables and controller mappings omitted."),
                        Err(e) => e,
                    });
                }
            }
        });
    view.composites.panel = open;
}

#[cfg(test)]
mod tests {
    use super::*;
    use kabl_core::composite::Snapshot;
    use kabl_core::panel::{Kind, Panel, Placement, Source};

    fn placement(kind: Kind, x: f32, source: Option<Source>) -> Placement {
        Placement {
            kind,
            x,
            y: 72.0,
            width: 120.0,
            height: 120.0,
            source,
            label: None,
            icons: Vec::new(),
        }
    }

    fn composite(parent: Option<CompositeId>, members: &[ModuleId]) -> Composite {
        Composite {
            name: "Face".into(),
            help: String::new(),
            definition: definition(parent.map_or(1, |_| 2)),
            parent,
            members: members.iter().copied().collect(),
            pos: kabl_core::Vec2::default(),
            ports: BTreeMap::new(),
            controls: BTreeMap::new(),
            next_interface: 1,
            panel: None,
            snapshots: Vec::new(),
        }
    }

    /// Composite 1 (an inner clock) with a display on the filter and a button pressing
    /// nested composite 2's button 2; composite 2 (osc, filter) with snapshot 3 on the inner
    /// clock and snapshot 4 on a clock outside both. Returns the editor and the inner clock,
    /// outer clock and filter.
    fn faces() -> (PatchEditor, ModuleId, ModuleId, ModuleId) {
        let mut e = PatchEditor::seed_from(&PatchState::new());
        let pos = kabl_core::Vec2::default();
        let inner_clock = e.add_module("clock", pos);
        let outer_clock = e.add_module("clock", pos);
        let osc = e.add_module("osc.va", pos);
        let filter = e.add_module("filter.svf", pos);
        let mut parent = composite(None, &[inner_clock]);
        parent.next_interface = 3;
        parent.panel = Some(Box::new(Panel {
            width: 600.0,
            placements: BTreeMap::from([
                (
                    1,
                    placement(
                        Kind::Display,
                        12.0,
                        Some(Source::Module {
                            id: filter,
                            action: None,
                        }),
                    ),
                ),
                (
                    2,
                    placement(
                        Kind::Button,
                        140.0,
                        Some(Source::Face {
                            composite: 2,
                            key: 2,
                        }),
                    ),
                ),
            ]),
            ..Panel::default()
        }));
        let mut child = composite(Some(1), &[osc, filter]);
        child.controls.insert(
            1,
            Exposure {
                label: "Cutoff".into(),
                target: PortRef::Param {
                    id: filter,
                    param: "cutoff_hz".into(),
                },
                also: Vec::new(),
            },
        );
        child.next_interface = 5;
        child.snapshots = [(3, inner_clock), (4, outer_clock)]
            .into_iter()
            .map(|(id, clock)| Snapshot {
                id,
                name: format!("S{id}"),
                values: BTreeMap::from([(1, 900.0)]),
                morph_beats: 2.0,
                clock: Some(clock),
            })
            .collect();
        child.panel = Some(Box::new(Panel {
            placements: BTreeMap::from([(
                2,
                placement(Kind::Button, 12.0, Some(Source::Snapshot { id: 3 })),
            )]),
            ..Panel::default()
        }));
        e.edit(vec![
            Op::SetComposite {
                id: 1,
                value: Some(parent),
            },
            Op::SetComposite {
                id: 2,
                value: Some(child),
            },
        ]);
        kabl_core::composite::validate(e.state()).unwrap();
        (e, inner_clock, outer_clock, filter)
    }

    /// Insert and duplicate give the copy's face elements and snapshots the copy's own ids:
    /// a module source, a nested face target and a snapshot clock inside the copy follow it; a
    /// clock outside stays on a duplicate and is dropped on an insert.
    #[test]
    fn insert_and_duplicate_remap_sources_face_targets_and_snapshot_clocks() {
        let (e, inner_clock, outer_clock, filter) = faces();
        let pkg = snapshot(e.state(), 1).unwrap();
        for external in [true, false] {
            let (p, root) = inserted(&e, &pkg, external).unwrap();
            kabl_core::composite::validate(&p).unwrap();
            let (&copy_child, c) = p
                .composites
                .iter()
                .find(|(_, c)| c.parent == Some(root))
                .unwrap();
            assert!(root > 2 && copy_child > 2);
            let copy_filter = *c
                .members
                .iter()
                .find(|m| p.modules[m].kind == "filter.svf")
                .unwrap();
            let copy_clock = *p.composites[&root].members.first().unwrap();
            assert!(copy_filter != filter && copy_clock != inner_clock);
            let face = &p.composites[&root].panel.as_ref().unwrap().placements;
            assert_eq!(
                face[&1].source,
                Some(Source::Module {
                    id: copy_filter,
                    action: None
                })
            );
            assert_eq!(
                face[&2].source,
                Some(Source::Face {
                    composite: copy_child,
                    key: 2
                })
            );
            let clocks: Vec<_> = c.snapshots.iter().map(|s| (s.id, s.clock)).collect();
            let outside = external.then_some(outer_clock);
            assert_eq!(clocks, [(3, Some(copy_clock)), (4, outside)], "{external}");
        }
    }

    fn exported(p: &PatchState) -> u32 {
        let pkg = package(p, 1).unwrap();
        let bytes = serde_json::to_vec(&pkg).unwrap();
        assert_eq!(decode(&bytes).unwrap().schema, pkg.schema);
        pkg.schema
    }

    /// `faces()` without its faces: export runs `panels::validate`, which refuses the new
    /// kinds until they draw. Snapshots stay. Also returns the inner clock.
    fn faceless() -> (PatchState, ModuleId) {
        let (e, inner_clock, ..) = faces();
        let mut p = e.state().clone();
        for c in p.composites.values_mut() {
            c.panel = None;
        }
        (p, inner_clock)
    }

    /// Like the patch format, a package moves to schema 3 only when it uses a face field.
    #[test]
    fn a_package_using_faces_is_schema_3_and_a_plain_one_is_not() {
        let (snapshots, _) = faceless();
        assert_eq!(exported(&snapshots), 3);
        let mut plain = snapshots.clone();
        plain.composites.get_mut(&2).unwrap().snapshots.clear();
        assert_eq!(exported(&plain), 1);
        let mut labelled = plain.clone();
        labelled.composites.get_mut(&2).unwrap().panel = Some(Box::new(Panel {
            placements: BTreeMap::from([(
                1,
                Placement {
                    kind: Kind::Knob,
                    width: 84.0,
                    height: 110.0,
                    label: Some("Tone".into()),
                    ..placement(Kind::Knob, 12.0, None)
                },
            )]),
            ..Panel::default()
        }));
        assert_eq!(exported(&labelled), 3);
        let mut compact = plain;
        compact.composites.get_mut(&1).unwrap().panel = Some(Box::new(Panel {
            width: 300.0,
            compact_width: Some(180.0),
            ..Panel::default()
        }));
        assert_eq!(exported(&compact), 3);
    }

    #[test]
    fn decode_accepts_package_schema_3_and_refuses_4() {
        let (p, _) = faceless();
        let mut pkg = package(&p, 1).unwrap();
        assert_eq!(pkg.schema, 3);
        let back = decode(&serde_json::to_vec(&pkg).unwrap()).unwrap();
        assert_eq!((back.schema, back.patch), (3, pkg.patch.clone()));
        pkg.schema = 4;
        let err = decode(&serde_json::to_vec(&pkg).unwrap()).unwrap_err();
        assert!(err.contains("Unsupported"), "{err}");
    }

    /// The exported package keeps a snapshot clock inside the composite and drops one outside.
    #[test]
    fn export_drops_a_snapshot_clock_outside_the_composite() {
        let (p, inner_clock) = faceless();
        let pkg = package(&p, 1).unwrap();
        let clocks: Vec<_> = pkg.patch.composites[&2]
            .snapshots
            .iter()
            .map(|s| (s.id, s.clock))
            .collect();
        assert_eq!(clocks, [(3, Some(inner_clock)), (4, None)]);
    }
}
