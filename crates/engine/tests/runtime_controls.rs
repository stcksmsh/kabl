//! D04 runtime values on the real `PatchEngine` path: classification of document changes,
//! in-place application with the compiler's exact values, ramps, revision ordering against
//! active / fading / pending graphs, stale and reused targets, bounded draining and
//! allocation-freedom of every audio-thread call.
//!
//! Probe patch as in `live_edit.rs`: `midi.in.gate -> vca.in`, `vca.out -> out`, one voice, so
//! the output is exactly `gate × gain` and a value is readable from the samples.

use std::collections::BTreeMap;

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::{Collector, Handle, Owned};
use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::compile::{compile, CompiledPatch, RAMP_MS};
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::PatchEngine;
use kabl_engine::runtime::{runtime_changes, Feedback, ParamSet, RuntimeTarget, ToAudio};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;
/// Every fade and ramp done.
const SETTLE: usize = 40;

fn module(kind: &str, params: &[(&str, f32)]) -> ModuleState {
    ModuleState {
        kind: kind.to_string(),
        pos: Vec2 { x: 0.0, y: 0.0 },
        params: params.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
    }
}

fn cable(from: (u64, &str), to: PortRef) -> CableState {
    CableState {
        from: PortRef::Module {
            id: from.0,
            port: from.1.into(),
        },
        to,
        params: BTreeMap::new(),
        steps: Vec::new(),
    }
}

fn jack(id: u64, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

fn probe(gain: f32) -> PatchState {
    let mut p = PatchState::new();
    p.modules.insert(7, module("midi.in", &[]));
    p.modules.insert(2, module("vca", &[("gain", gain)]));
    p.modules.insert(3, module("out", &[]));
    p.cables.insert(1, cable((7, "gate"), jack(2, "in")));
    p.cables.insert(2, cable((2, "out"), jack(3, "left")));
    p
}

const GAIN: RuntimeTarget = RuntimeTarget::Param {
    id: 2,
    kind: "vca",
    index: 0,
};

fn set(rev: u64, value: f32) -> ParamSet {
    ParamSet {
        rev,
        target: GAIN,
        value,
    }
}

fn graph(h: &Handle, patch: &PatchState, rev: u64) -> Owned<CompiledPatch> {
    let mut g = compile(patch, SR, 1).unwrap();
    g.rev = rev;
    Owned::new(h, g)
}

fn engine(h: &Handle, gain: f32, rev: u64) -> PatchEngine {
    let mut g = compile(&probe(gain), SR, 1).unwrap();
    g.rev = rev;
    let mut e = PatchEngine::with_compiled(h, g, 1);
    e.note_on(0, 0.0, 1.0);
    e
}

fn block(e: &mut PatchEngine) -> [f32; BLOCK] {
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    e.process_block(&mut l, &mut r);
    l
}

fn settle(e: &mut PatchEngine) -> f32 {
    let mut last = 0.0;
    for _ in 0..SETTLE {
        last = block(e)[BLOCK - 1];
    }
    last
}

/// Which values ramp in a playing graph: continuous levels and times; never stepped choices,
/// sequencer step data (a ramp could play a note between two values) or params the module
/// smooths itself.
#[test]
fn only_continuous_unsmoothed_values_ramp() {
    use kabl_engine::runtime::ramped;
    let p = |kind: &str, name: &str| {
        *kabl_modules::registry::info_for(kind)
            .unwrap()
            .params
            .iter()
            .find(|p| p.name == name)
            .unwrap()
    };
    assert!(ramped("vca", &p("vca", "gain")));
    assert!(ramped("mixer", &p("mixer", "level1")));
    assert!(ramped("env.adsr", &p("env.adsr", "sustain")));
    assert!(ramped("filter.svf", &p("filter.svf", "cutoff_hz")));
    assert!(!ramped("vca", &p("vca", "exponential")));
    assert!(!ramped("seq", &p("seq", "transpose")));
    assert!(!ramped("seq", &p("seq", "p1")));
    assert!(!ramped("gain", &p("gain", "gain_db")));
    assert!(!ramped("macro", &p("macro", "m1")));
    assert!(!ramped("osc.va", &p("osc.va", "pw")));
}

#[test]
fn only_runtime_values_are_runtime_changes() {
    let base = probe(0.5);
    // A knob: one value.
    let mut p = base.clone();
    p.modules
        .get_mut(&2)
        .unwrap()
        .params
        .insert("gain".into(), 0.8);
    assert_eq!(runtime_changes(&base, &p), Some(vec![(GAIN, 0.8)]));
    // The same value again: nothing.
    assert_eq!(runtime_changes(&p, &p.clone()), Some(vec![]));
    // Unset back to the default (1.0): the default is the value.
    let mut q = base.clone();
    q.modules.get_mut(&2).unwrap().params.remove("gain");
    assert_eq!(runtime_changes(&base, &q), Some(vec![(GAIN, 1.0)]));
    // Presentation params and positions: nothing for the engine.
    let mut q = base.clone();
    let m = q.modules.get_mut(&2).unwrap();
    m.params.insert("pin.gain".into(), 0.0);
    m.params.insert("cc.gain".into(), 7.0);
    m.pos = Vec2 { x: 5.0, y: 5.0 };
    assert_eq!(runtime_changes(&base, &q), Some(vec![]));
    // Keyboard configuration: compile. Glide time: runtime.
    let mut q = base.clone();
    q.modules
        .get_mut(&7)
        .unwrap()
        .params
        .insert("mode".into(), 1.0);
    assert_eq!(runtime_changes(&base, &q), None);
    let mut q = base.clone();
    q.modules
        .get_mut(&7)
        .unwrap()
        .params
        .insert("glide_ms".into(), 200.0);
    assert!(runtime_changes(&base, &q).is_some_and(|v| v.len() == 1));
    // Topology: compile.
    let mut q = base.clone();
    q.modules.insert(9, module("lfo", &[]));
    assert_eq!(runtime_changes(&base, &q), None);
    let mut q = base.clone();
    q.cables.remove(&1);
    assert_eq!(runtime_changes(&base, &q), None);
    let mut q = base.clone();
    q.modules.get_mut(&2).unwrap().kind = "gain".into();
    assert_eq!(runtime_changes(&base, &q), None);
    // A route: its amount is runtime, bypass and wiring are not; a bypassed route's amount
    // is not compiled at all.
    let mut r = base.clone();
    r.modules.insert(9, module("lfo", &[]));
    r.cables.insert(
        5,
        cable(
            (9, "out"),
            PortRef::Param {
                id: 2,
                param: "gain".into(),
            },
        ),
    );
    let mut q = r.clone();
    q.cables
        .get_mut(&5)
        .unwrap()
        .params
        .insert("amount".into(), -0.5);
    assert_eq!(
        runtime_changes(&r, &q),
        Some(vec![(RuntimeTarget::Route { cable: 5 }, -0.5)])
    );
    let mut b = r.clone();
    b.cables
        .get_mut(&5)
        .unwrap()
        .params
        .insert("bypass".into(), 1.0);
    assert_eq!(runtime_changes(&r, &b), None);
    let mut b2 = b.clone();
    b2.cables
        .get_mut(&5)
        .unwrap()
        .params
        .insert("amount".into(), 0.9);
    assert_eq!(runtime_changes(&b, &b2), Some(vec![]));
}

/// A value set before a graph plays is exactly what the compiler produces for that value:
/// the two render bit-identically, modulated or not.
#[test]
fn a_set_value_renders_like_a_compile_of_it() {
    let mut p = probe(0.3);
    p.modules.insert(9, module("lfo", &[("rate_hz", 3.0)]));
    p.cables.insert(
        5,
        cable(
            (9, "out"),
            PortRef::Param {
                id: 2,
                param: "gain".into(),
            },
        ),
    );
    let mut q = p.clone();
    q.modules
        .get_mut(&2)
        .unwrap()
        .params
        .insert("gain".into(), 0.7);
    q.cables
        .get_mut(&5)
        .unwrap()
        .params
        .insert("amount".into(), -0.4);
    let changes = runtime_changes(&p, &q).unwrap();
    assert_eq!(changes.len(), 2);
    let mut a = compile(&p, SR, 1).unwrap();
    for &(t, v) in &changes {
        // What `PatchEngine::set` does: a graph that has not played is set, not ramped.
        let ramp = a.started();
        assert!(a.set_runtime(t, v, ramp));
    }
    assert_eq!(a.ramps(), 0);
    let mut b = compile(&q, SR, 1).unwrap();
    for g in [&mut a, &mut b] {
        g.note_on(0, 0.0, 1.0);
    }
    for _ in 0..200 {
        a.process_block();
        b.process_block();
        assert_eq!(a.left().map(f32::to_bits), b.left().map(f32::to_bits));
    }
}

/// A playing graph ramps a runtime value over `RAMP_MS` in knob travel and ends on the exact
/// value; the voice, its note and the graph stay (no swap, no fade).
#[test]
fn a_playing_graph_ramps_and_lands_exactly() {
    let collector = Collector::new();
    let h = collector.handle();
    let mut e = engine(&h, 0.2, 1);
    assert_eq!(settle(&mut e), 0.2);
    assert!(e.set(&set(2, 0.6)));
    assert!(!e.is_swapping());
    let first = block(&mut e);
    assert!(
        first[0] > 0.2 && first[0] < 0.6,
        "moving, not jumped: {}",
        first[0]
    );
    let blocks = (RAMP_MS / 1000.0 * SR / BLOCK as f32).round() as usize;
    for _ in 1..blocks - 1 {
        block(&mut e);
    }
    assert_eq!(e.active_mut().ramps(), 1, "still ramping before the end");
    block(&mut e);
    assert_eq!(e.active_mut().ramps(), 0);
    assert_eq!(e.active_mut().param_value(2, 0), Some(0.6));
    assert_eq!(block(&mut e)[0], 0.6);
    // A stepped param is never ramped.
    let exp = RuntimeTarget::Param {
        id: 2,
        kind: "vca",
        index: 1,
    };
    assert!(e.active_mut().set_runtime(exp, 1.0, true));
    assert_eq!(e.active_mut().ramps(), 0);
    assert_eq!(block(&mut e)[0], 0.36, "exponential: gain²");
}

/// Revision order across active, fading and pending graphs.
#[test]
fn values_and_graphs_keep_revision_order() {
    let collector = Collector::new();
    let h = collector.handle();
    let mut e = engine(&h, 0.1, 1);
    settle(&mut e);
    // Graph rev 5 (gain 0.5) starts fading in; graph rev 7 (gain 0.7) queues behind it.
    e.receive_swap(graph(&h, &probe(0.5), 5));
    block(&mut e);
    e.receive_swap(graph(&h, &probe(0.7), 7));
    assert!(e.is_swapping());
    // A value from before rev 5 (a delayed message): the newer graphs keep their own value;
    // only the outgoing graph, older still, takes it.
    assert!(e.set(&set(4, 0.4)));
    // A value newer than both graphs reaches all three, set in the pending one.
    assert!(e.set(&set(8, 0.8)));
    assert_eq!(
        settle(&mut e),
        0.8,
        "the queued graph did not overwrite the newer value"
    );
    // A graph compiled from an older document than one received: refused.
    e.receive_swap(graph(&h, &probe(0.3), 6));
    assert!(!e.is_swapping());
    assert_eq!(settle(&mut e), 0.8);
    // A value of the same revision as the graph fading in reaches only the outgoing graph.
    e.receive_swap(graph(&h, &probe(0.9), 9));
    assert!(e.set(&set(9, 0.2)));
    assert_eq!(settle(&mut e), 0.9);
}

/// A value whose target is not in the graph, or is a reused id of another kind, resolves
/// nowhere and changes nothing.
#[test]
fn stale_and_reused_targets_are_rejected() {
    let collector = Collector::new();
    let h = collector.handle();
    let mut e = engine(&h, 0.5, 1);
    settle(&mut e);
    let absent = ParamSet {
        rev: 2,
        target: RuntimeTarget::Param {
            id: 40,
            kind: "vca",
            index: 0,
        },
        value: 0.0,
    };
    assert!(!e.set(&absent));
    let wrong_kind = ParamSet {
        rev: 3,
        target: RuntimeTarget::Param {
            id: 2,
            kind: "gain",
            index: 0,
        },
        value: -60.0,
    };
    assert!(!e.set(&wrong_kind));
    assert!(!e.set(&ParamSet {
        rev: 4,
        target: RuntimeTarget::Route { cable: 1 },
        value: 1.0,
    }));
    assert_eq!(settle(&mut e), 0.5);
}

/// `drain` takes at most `max` messages per call, in order, and never allocates or frees:
/// a saturated queue drains over several callbacks and ends on the last value.
#[test]
fn draining_is_bounded_ordered_and_allocation_free() {
    let collector = Collector::new();
    let h = collector.handle();
    let mut e = engine(&h, 0.5, 1);
    settle(&mut e);
    let (mut tx, mut rx) = rtrb::RingBuffer::<ToAudio>::new(64);
    let fb = Feedback::default();
    tx.push(ToAudio::Graph(graph(&h, &probe(0.25), 2))).ok();
    for k in 0..63u64 {
        let _ = tx.push(ToAudio::Set(set(3 + k, k as f32 / 100.0)));
    }
    assert!(tx.is_full());
    let mut rounds = 0;
    while !rx.is_empty() {
        assert_no_alloc(|| {
            e.drain(&mut rx, 16, &fb);
            block(&mut e);
        });
        rounds += 1;
    }
    assert_eq!(rounds, 4, "16 per callback");
    assert_eq!(Feedback::get(&fb.graphs_taken), 1);
    assert_eq!(Feedback::get(&fb.sets_taken), 63);
    assert_eq!(Feedback::get(&fb.applied_rev), 65);
    assert_no_alloc(|| {
        for _ in 0..SETTLE {
            block(&mut e);
        }
    });
    assert_eq!(block(&mut e)[0], 0.62);
    drop(e);
    let mut collector = collector;
    collector.collect();
}

/// A voiced module takes a value on every voice lane.
#[test]
fn every_voice_lane_takes_the_value() {
    let mut p = probe(0.5);
    let mut g = compile(&p, SR, 4).unwrap();
    assert!(g.set_runtime(GAIN, 0.25, false));
    p.modules
        .get_mut(&2)
        .unwrap()
        .params
        .insert("gain".into(), 0.25);
    let c = compile(&p, SR, 4).unwrap();
    for lane in 0..4 {
        assert_eq!(g.param_values(2, 0)[lane], 0.25);
    }
    assert_eq!(g.param_values(2, 0), c.param_values(2, 0));
    assert_eq!(g.param_values(2, 0).len(), 4);
}

/// Writes docs/runtime-controls/classification.md: every param of every built-in with its
/// class, whether a runtime value ramps and why. `cargo test -p kabl-engine --test
/// runtime_controls write_classification -- --ignored`.
#[test]
#[ignore]
fn write_classification() {
    use kabl_engine::runtime::{param_class, ramped, smoothed_by_module, ParamClass};
    use kabl_modules::Taper;
    let mut out = String::from(
        "# Parameter classification (generated)\n\n\
         Written by `cargo test -p kabl-engine --test runtime_controls write_classification -- \
         --ignored` from `kabl_engine::runtime` and the module registry. Rules and reasons: \
         [design.md](design.md).\n\n\
         | Module | Param | Taper | Class | Runtime value | Reason |\n|---|---|---|---|---|---|\n",
    );
    let mut counts = [0usize; 3];
    for kind in kabl_modules::registry::KNOWN_KINDS {
        let info = kabl_modules::registry::info_for(kind).unwrap();
        if info.params.is_empty() {
            out += &format!("| `{kind}` | (none) | | | | no params |\n");
            continue;
        }
        // Sequencer banks B–D repeat bank A's params: summarized.
        for p in info.params {
            if *kind == "seq" && p.name.contains('.') {
                continue;
            }
            let taper = match p.taper {
                Taper::Linear => "linear",
                Taper::Exponential => "exponential",
                Taper::Stepped => "stepped",
            };
            let (class, how, why) = match param_class(kind, p.name) {
                ParamClass::Structural => {
                    counts[2] += 1;
                    (
                        "structural",
                        "compile",
                        "keyboard configuration: read by the keyboard outside the graph when a graph is installed (voice assignment; a mode change releases)",
                    )
                }
                ParamClass::Runtime if ramped(kind, p) => {
                    counts[0] += 1;
                    (
                        "runtime",
                        "ramped 15 ms",
                        "read every block; the module does not smooth it",
                    )
                }
                ParamClass::Runtime => {
                    counts[1] += 1;
                    let why = if p.taper == Taper::Stepped {
                        "read every block; a choice, set at once (never through invalid states)"
                    } else if smoothed_by_module(kind, p.name) {
                        "read every block; the module smooths it itself"
                    } else if *kind == "seq" {
                        "step data, read when a step plays; set at once (a ramp could play a note between values)"
                    } else {
                        "read every block"
                    };
                    ("runtime", "set at once", why)
                }
            };
            out += &format!(
                "| `{kind}` | `{}` | {taper} | {class} | {how} | {why} |\n",
                p.name
            );
        }
        if *kind == "seq" {
            out += "| `seq` | `b.*`, `c.*`, `d.*` (banks B–D) | as bank A | runtime | set at once | same as bank A's params |\n";
        }
    }
    out += &format!(
        "\n{} runtime params ramp, {} are set at once, {} are structural (bank A of `seq` \
         counted; banks B–D behave the same).\n\n\
         ## Cable parameters\n\n\
         | Cable | Param | Class | Reason |\n|---|---|---|---|\n\
         | route (into a knob) | `amount` | runtime, ramped 15 ms (as a scale) | read every block from the route's scale; a bypassed route is not compiled, so its amount changes nothing |\n\
         | route | `bypass` | structural | a bypassed route leaves the compiled graph: scheduling and feedback (DFS back edges) change |\n\
         | route | creation / deletion | structural | scheduling, cycle breaking and buffers change |\n\
         | jack cable | any | structural (wiring); its params are not read | the compiler reads no jack cable params |\n\n\
         ## Presentation (never reaches the engine)\n\n\
         `face.*`, `pin.*`, `cc.*`, `btn.*`, `launch.*`, `cue*`, labels and positions: \
         `rack::is_presentation` / `Op::SetLabel` / `Op::MoveModule`. The compiler does not read \
         them, so `runtime_changes` finds nothing to send.\n",
        counts[0], counts[1], counts[2]
    );
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/runtime-controls");
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("classification.md"), out).unwrap();
}
