//! Conservative tail to -120 dB. A finite tail is advertised only when every output route
//! crosses a zero-base VCA driven solely by a MIDI-gated envelope. Other racks may sustain
//! themselves (free oscillators, clocks, feedback, bypass routes) and remain active.
use kabl_core::{PatchState, PortRef};
use std::collections::{BTreeMap, BTreeSet};

fn input<'a>(p: &'a PatchState, id: u64, port: &str) -> Vec<&'a PortRef> {
    p.cables.values().filter(|c| matches!(&c.to, PortRef::Module { id: target, port: name } if *target == id && name == port)).map(|c| &c.from).collect()
}
fn gated(p: &PatchState, id: u64) -> bool {
    let m = &p.modules[&id];
    if m.kind != "vca" || m.params.get("gain").copied().unwrap_or(1.0) != 0.0
        || p.cables.values().any(|c| matches!(&c.to, PortRef::Param { id: target, param } if *target == id && param == "gain")) {
        return false;
    }
    let cv = input(p, id, "cv");
    let [PortRef::Module { id: env, port }] = cv.as_slice() else {
        return false;
    };
    if port != "out" || p.modules.get(env).is_none_or(|m| m.kind != "env.adsr") {
        return false;
    }
    let gate = input(p, *env, "gate");
    matches!(gate.as_slice(), [PortRef::Module { id: midi, port }] if port == "gate" && p.modules.get(midi).is_some_and(|m| m.kind == "midi.in"))
}
fn finite_route(
    p: &PatchState,
    id: u64,
    visiting: &mut BTreeSet<u64>,
    memo: &mut BTreeMap<u64, bool>,
) -> bool {
    if let Some(&finite) = memo.get(&id) {
        return finite;
    }
    if gated(p, id) {
        memo.insert(id, true);
        return true;
    }
    if !visiting.insert(id) {
        return false;
    }
    // A modulation path into a generator does not silence its audio. Only explicitly
    // silence-preserving audio processors participate; filters may self-oscillate.
    let Some(module) = p.modules.get(&id) else {
        return false;
    };
    if !matches!(
        module.kind.as_str(),
        "out" | "gain" | "mixer" | "vca" | "delay" | "reverb" | "chorus" | "drive" | "ringmod"
    ) {
        visiting.remove(&id);
        return false;
    }
    let routes: Vec<_> = p.cables.values().filter(|c| matches!(&c.to,
        PortRef::Module { id: target, port } if *target == id && matches!(port.as_str(), "in" | "in_l" | "in_r" | "left" | "right" | "a" | "b" | "c" | "d" | "in1" | "in2" | "in3" | "in4"))) .collect();
    let finite = !routes.is_empty()
        && routes.iter().all(|c| {
            c.from
                .module_id()
                .is_some_and(|f| finite_route(p, f, visiting, memo))
        });
    visiting.remove(&id);
    memo.insert(id, finite);
    finite
}
fn value(p: &PatchState, id: u64, name: &str, default: f64, max: f64) -> f64 {
    if p.cables.values().any(
        |c| matches!(&c.to, PortRef::Param { id: target, param } if *target == id && param == name),
    ) {
        max
    } else {
        (p.modules[&id]
            .params
            .get(name)
            .copied()
            .map(f64::from)
            .unwrap_or(default))
        .clamp(0.0, max)
    }
}
pub fn samples(p: &PatchState, rate: f32) -> u64 {
    let outputs: Vec<_> = p
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "out")
        .map(|(&id, _)| id)
        .collect();
    let mut memo = BTreeMap::new();
    if outputs
        .iter()
        .any(|&id| !finite_route(p, id, &mut BTreeSet::new(), &mut memo))
    {
        return u64::MAX;
    }
    // Sum all effects, even parallel/disconnected ones: conservative for cascades. Include
    // control smoothing, 64-frame Timeline latency and a one-second settling margin.
    let mut seconds = 1.0;
    for (&id, m) in &p.modules {
        seconds += match m.kind.as_str() {
            // FullAdsr times are exponential time constants, settling at 1e-4 (9.21 tau).
            "env.adsr" => {
                10.0 * if value(p, id, "timing", 0.0, 1.0) >= 0.5 {
                    10000.0
                } else {
                    value(p, id, "release_ms", 200.0, 10000.0)
                } / 1000.0
            }
            "delay" => {
                // Sync/modulation may reach the delay's 4-second clamp.
                let time = if value(p, id, "sync", 0.0, 4.0) != 0.0 {
                    4.0
                } else {
                    value(p, id, "time_ms", 375.0, 4000.0) / 1000.0
                };
                let feedback = value(p, id, "feedback", 40.0, 95.0) / 100.0;
                time * if feedback > 0.0 {
                    (1e-6f64.ln() / feedback.ln()).ceil() + 1.0
                } else {
                    1.0
                }
            }
            // Tank diffusion/loop interpretation: four decay periods conservatively exceed
            // the nominal -120 dB decay, plus maximum predelay.
            "reverb" => 4.0 * value(p, id, "decay_s", 3.5, 30.0) + 0.25,
            "chorus" => 0.1,
            _ => 0.0,
        };
    }
    (seconds * f64::from(rate)).ceil() as u64 + kabl_engine::timeline::LATENCY as u64
}
