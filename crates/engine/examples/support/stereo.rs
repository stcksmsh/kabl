//! The stereo test path shared by `bench_patches` and `stereo_demo`.

use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};

/// The stereo test path: a `pan` before every filter input, and each `in_l`/`in_r` pair fed by
/// one source as one stereo cable; `autopan` adds a slow LFO on every pan. `None` when the patch
/// has no filter to put the path in.
pub fn stereo_variant(p: &PatchState, autopan: bool) -> Option<PatchState> {
    let mut s = p.clone();
    let mut next_module = s.modules.keys().max().map_or(1, |m| m + 1);
    let mut next_cable = s.cables.keys().max().map_or(1, |c| c + 1);
    let filters: Vec<u64> = s
        .modules
        .iter()
        .filter(|(_, m)| m.kind.starts_with("filter."))
        .map(|(&id, _)| id)
        .collect();
    let mut changed = false;
    let mut pans = Vec::new();
    for filter in filters {
        let into: Vec<u64> = s
            .cables
            .iter()
            .filter(|(_, c)| matches!(&c.to, PortRef::Module { id, port } if *id == filter && port == "in"))
            .map(|(&k, _)| k)
            .collect();
        for k in into {
            let pan = next_module;
            next_module += 1;
            s.modules.insert(
                pan,
                ModuleState {
                    kind: "pan".into(),
                    pos: Vec2 { x: 0.0, y: 0.0 },
                    params: [("pan".to_string(), 0.4)].into(),
                },
            );
            let cable = s.cables.get_mut(&k).expect("listed above");
            let port = |id, port: &str| PortRef::Module {
                id,
                port: port.into(),
            };
            let old_to = std::mem::replace(&mut cable.to, port(pan, "in"));
            s.cables.insert(
                next_cable,
                CableState {
                    from: port(pan, "out_st"),
                    to: old_to,
                    params: Default::default(),
                    steps: Vec::new(),
                },
            );
            next_cable += 1;
            pans.push(pan);
            changed = true;
        }
    }
    // Audible stereo: one slow LFO moves every pan.
    if autopan && !pans.is_empty() {
        let lfo = next_module;
        s.modules.insert(
            lfo,
            ModuleState {
                kind: "lfo".into(),
                pos: Vec2 { x: 0.0, y: 0.0 },
                params: [("rate_hz".to_string(), 0.4)].into(),
            },
        );
        for pan in pans {
            s.cables.insert(
                next_cable,
                CableState {
                    from: PortRef::Module {
                        id: lfo,
                        port: "out".into(),
                    },
                    to: PortRef::Param {
                        id: pan,
                        param: "pan".into(),
                    },
                    params: [("amount".to_string(), 1.0)].into(),
                    steps: Vec::new(),
                },
            );
            next_cable += 1;
        }
    }
    changed.then_some(s)
}
