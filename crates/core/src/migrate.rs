//! Schema 7 to 8 (docs/stereo/DESIGN.md section 4): the stereo pairs (`in_l`/`in_r`,
//! `left`/`right`) became one width-2 port (`in`, `out`). `migrate_state` rewrites a
//! `PatchState` so it names only v8 ports and sounds the same; `migrate_v7_to_v8` applies it to a
//! loaded log as one undoable-never group of ordinary ops.
//!
//! Rules, per module with a stereo input pair `(a, b)`:
//! - both patched from one mono source, or from `X.left` and `X.right` of one stereo module, and
//!   neither cable is functional: one cable into `in`;
//! - anything else: a `join` module (`a` into `left`, `b` into `right`, an unwired side stays
//!   silent) and one cable from `join.out` into `in`.
//!
//! Every cable still leaving `X.left` or `X.right` of a stereo-output module then goes through one
//! `split` of `X`. A stereo cable never meets a mono port, so nothing is averaged. A second cable
//! into one port was never read, so it is dropped.

use std::collections::BTreeMap;

use crate::{CableId, CableState, Composite, ModuleId, ModuleState, Op, ParamTarget, PatchState};
use crate::{PatchLog, PortRef, Source, Vec2};

const STEREO_IN: &[&str] = &[
    "chorus",
    "reverb",
    "eq",
    "flanger",
    "phaser",
    "limiter",
    "bitcrusher",
    "compressor",
    "vocoder",
];
const STEREO_OUT: &[&str] = &[
    "chorus",
    "reverb",
    "eq",
    "flanger",
    "phaser",
    "limiter",
    "bitcrusher",
    "compressor",
    "vocoder",
    "audio.in",
    "delay",
    "pan",
];

/// Names of the old input pair of `kind`.
fn in_pair(kind: &str) -> Option<(&'static str, &'static str)> {
    if kind == "out" {
        Some(("left", "right"))
    } else {
        STEREO_IN.contains(&kind).then_some(("in_l", "in_r"))
    }
}

fn has_out_pair(kind: &str) -> bool {
    STEREO_OUT.contains(&kind)
}

fn module_port(r: &PortRef) -> Option<(ModuleId, &str)> {
    match r {
        PortRef::Module { id, port } => Some((*id, port)),
        _ => None,
    }
}

fn port(id: ModuleId, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

struct Ctx<'a> {
    s: &'a mut PatchState,
    next_module: ModuleId,
    next_cable: CableId,
}

impl Ctx<'_> {
    fn kind(&self, id: ModuleId) -> &str {
        self.s.modules.get(&id).map_or("", |m| m.kind.as_str())
    }

    fn add_module(&mut self, kind: &str, pos: Vec2) -> ModuleId {
        let id = self.next_module;
        self.next_module += 1;
        self.s.modules.insert(
            id,
            ModuleState {
                kind: kind.into(),
                pos,
                params: BTreeMap::new(),
            },
        );
        id
    }

    fn add_cable(&mut self, from: PortRef, to: PortRef) {
        let id = self.next_cable;
        self.next_cable += 1;
        self.s.cables.insert(
            id,
            CableState {
                from,
                to,
                params: BTreeMap::new(),
                steps: Vec::new(),
            },
        );
    }

    /// The cable read into `(m, name)` (the lowest id); any others there were never read and go.
    fn single_cable_into(&mut self, m: ModuleId, name: &str) -> Option<CableId> {
        let ids: Vec<CableId> = self
            .s
            .cables
            .iter()
            .filter(|(_, c)| module_port(&c.to) == Some((m, name)))
            .map(|(&k, _)| k)
            .collect();
        for k in ids.iter().skip(1) {
            self.s.cables.remove(k);
        }
        ids.first().copied()
    }

    /// Plain cable: no parameters, no pattern, nothing routed into its parameters.
    fn plain(&self, k: CableId) -> bool {
        let c = &self.s.cables[&k];
        c.params.is_empty()
            && c.steps.is_empty()
            && !self
                .s
                .cables
                .values()
                .any(|o| matches!(&o.to, PortRef::CableParam { cable, .. } if *cable == k))
    }

    /// `(m, a)` and `(m, b)` of the old pair become the one port `in`.
    fn migrate_inputs(&mut self, m: ModuleId, a: &str, b: &str) {
        let (ca, cb) = (self.single_cable_into(m, a), self.single_cable_into(m, b));
        let at = self.s.modules[&m].pos;
        if let (Some(ca), Some(cb)) = (ca, cb) {
            if self.plain(ca) && self.plain(cb) {
                let (fa, fb) = (
                    self.s.cables[&ca].from.clone(),
                    self.s.cables[&cb].from.clone(),
                );
                let pair = match (module_port(&fa), module_port(&fb)) {
                    (Some((x, "left")), Some((y, "right"))) => x == y && has_out_pair(self.kind(x)),
                    _ => false,
                };
                let mono = fa == fb && !self.is_pair_port(&fa);
                if pair || mono {
                    if pair {
                        let (x, _) = module_port(&fa).expect("pair source is a module port");
                        self.s.cables.get_mut(&ca).expect("listed").from = port(x, "out");
                    }
                    self.s.cables.get_mut(&ca).expect("listed").to = port(m, "in");
                    self.s.cables.remove(&cb);
                    return;
                }
            }
        }
        if ca.is_none() && cb.is_none() {
            return;
        }
        let join = self.add_module(
            "join",
            Vec2 {
                x: at.x - 130.0,
                y: at.y + 40.0,
            },
        );
        for (k, side) in [(ca, "left"), (cb, "right")] {
            if let Some(k) = k {
                self.s.cables.get_mut(&k).expect("listed").to = port(join, side);
            }
        }
        self.add_cable(port(join, "out"), port(m, "in"));
    }

    fn is_pair_port(&self, r: &PortRef) -> bool {
        matches!(module_port(r), Some((x, "left" | "right")) if has_out_pair(self.kind(x)))
    }

    /// Cables still leaving `X.left` / `X.right` leave a `split` of `X.out` instead.
    fn migrate_outputs(&mut self, x: ModuleId) {
        let users: Vec<CableId> = self
            .s
            .cables
            .iter()
            .filter(
                |(_, c)| matches!(module_port(&c.from), Some((id, "left" | "right")) if id == x),
            )
            .map(|(&k, _)| k)
            .collect();
        if users.is_empty() {
            return;
        }
        let at = self.s.modules[&x].pos;
        let split = self.add_module(
            "split",
            Vec2 {
                x: at.x + 170.0,
                y: at.y + 50.0,
            },
        );
        self.add_cable(port(x, "out"), port(split, "in"));
        for k in users {
            let c = self.s.cables.get_mut(&k).expect("listed");
            let side = module_port(&c.from).expect("module port").1.to_string();
            c.from = port(split, &side);
        }
    }
}

/// The v8 name of an exposed old pair port, if `port` is one.
fn renamed(kind: &str, name: &str) -> Option<&'static str> {
    if let Some((a, b)) = in_pair(kind) {
        if name == a || name == b {
            return Some("in");
        }
    }
    (has_out_pair(kind) && matches!(name, "left" | "right")).then_some("out")
}

fn migrate_composite(c: &mut Composite, modules: &BTreeMap<ModuleId, ModuleState>) {
    for e in c.ports.values_mut() {
        if let PortRef::Module { id, port } = &mut e.target {
            let kind = modules.get(id).map_or("", |m| m.kind.as_str());
            if let Some(new) = renamed(kind, port) {
                *port = new.into();
            }
        }
    }
    let mut seen = Vec::new();
    c.ports.retain(|_, e| {
        let fresh = !seen.contains(&e.target);
        if fresh {
            seen.push(e.target.clone());
        }
        fresh
    });
}

/// Rewrites `s` to name only schema-8 ports. Returns whether anything changed. Pure and
/// deterministic: the same state always gives the same new module and cable ids.
pub fn migrate_state(s: &mut PatchState) -> bool {
    let before = s.clone();
    let mut cx = Ctx {
        next_module: s.modules.keys().max().map_or(1, |m| m + 1),
        next_cable: s.cables.keys().max().map_or(1, |c| c + 1),
        s,
    };
    let ids: Vec<ModuleId> = cx.s.modules.keys().copied().collect();
    for &m in &ids {
        if let Some((a, b)) = in_pair(&cx.s.modules[&m].kind.clone()) {
            cx.migrate_inputs(m, a, b);
        }
    }
    for &m in &ids {
        if has_out_pair(cx.kind(m)) {
            cx.migrate_outputs(m);
        }
    }
    let modules = cx.s.modules.clone();
    for c in cx.s.composites.values_mut() {
        migrate_composite(c, &modules);
    }
    *cx.s != before
}

/// Applies `migrate_state` to a loaded log as one group of ordinary ops (cables are connected
/// over their own ids, so routes into them survive), then makes it an undo barrier.
pub fn migrate_v7_to_v8(log: &mut PatchLog) {
    let old = log.state().clone();
    let mut new = old.clone();
    if !migrate_state(&mut new) {
        return;
    }
    let mut ops = Vec::new();
    for (&id, m) in &new.modules {
        if !old.modules.contains_key(&id) {
            ops.push(Op::AddModule {
                id,
                kind: m.kind.clone(),
                pos: m.pos,
            });
        }
    }
    for (&id, c) in &new.cables {
        if old.cables.get(&id) == Some(c) {
            continue;
        }
        ops.push(Op::Connect {
            id,
            from: c.from.clone(),
            to: c.to.clone(),
        });
        for (param, &value) in &c.params {
            ops.push(Op::SetParam {
                target: ParamTarget::Cable {
                    id,
                    param: param.clone(),
                },
                value,
            });
        }
        if !c.steps.is_empty() {
            ops.push(Op::SetCablePattern {
                id,
                steps: c.steps.clone(),
            });
        }
    }
    for &id in old.cables.keys().filter(|id| !new.cables.contains_key(id)) {
        ops.push(Op::Disconnect { id });
    }
    for (&id, c) in &new.composites {
        if old.composites.get(&id) != Some(c) {
            ops.push(Op::SetComposite {
                id,
                value: Some(c.clone()),
            });
        }
    }
    ops.push(Op::Annotate {
        text: "migrated to schema 8".into(),
    });
    let t_ms = log.all_entries().last().map_or(0, |e| e.t_ms);
    log.append_new(Op::Group { ops }, t_ms, Source::Director);
    log.set_undo_floor();
}
