//! Timing harness for the modulation-slice benchmark (docs/modulation-slice/README.md). The
//! same `routeless`/`bench` code ran on the baseline (e6db377) for the "before" number.
//! `cargo run --release -p kabl-ui --example bench_reference`
use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::compile::compile;
use kabl_modules::builtins::MidiIn;
use std::collections::BTreeMap;
use std::time::Instant;

fn m(kind: &str, p: &[(&str, f32)]) -> ModuleState {
    ModuleState {
        kind: kind.into(),
        pos: Vec2 { x: 0.0, y: 0.0 },
        params: p.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
    }
}
fn c(f: (u64, &str), t: (u64, &str)) -> CableState {
    CableState {
        from: PortRef::Module {
            id: f.0,
            port: f.1.into(),
        },
        to: PortRef::Module {
            id: t.0,
            port: t.1.into(),
        },
        params: BTreeMap::new(),
        steps: vec![],
    }
}
/// The reference patch without its modulation routes.
pub fn routeless() -> PatchState {
    let mut p = PatchState::new();
    p.modules.insert(1, m("midi.in", &[]));
    p.modules.insert(2, m("osc.va", &[("waveform", 2.0)]));
    p.modules.insert(
        3,
        m("filter.svf", &[("cutoff_hz", 1400.0), ("resonance", 0.35)]),
    );
    p.modules.insert(
        4,
        m(
            "env.adsr",
            &[
                ("attack_ms", 60.0),
                ("decay_ms", 350.0),
                ("sustain", 0.6),
                ("release_ms", 450.0),
            ],
        ),
    );
    p.modules.insert(5, m("vca", &[("gain", 0.0)]));
    p.modules.insert(6, m("out", &[]));
    p.modules
        .insert(7, m("lfo", &[("rate_hz", 0.4), ("waveform", 1.0)]));
    p.modules.insert(8, m("lfo", &[("rate_hz", 3.1)]));
    for (i, (f, t)) in [
        ((1, "pitch"), (2, "pitch")),
        ((2, "out"), (3, "in")),
        ((3, "lp"), (5, "in")),
        ((1, "gate"), (4, "gate")),
        ((4, "out"), (5, "cv")),
        ((5, "out"), (6, "left")),
        ((5, "out"), (6, "right")),
    ]
    .into_iter()
    .enumerate()
    {
        p.cables.insert(i as u64 + 1, c(f, t));
    }
    p
}

pub fn bench(name: &str, p: &PatchState) {
    let mut cp = compile(p, 48000.0, 8).unwrap();
    for v in 0..8 {
        let md = cp.module_mut(1, Some(v)).unwrap();
        md.as_any_mut()
            .downcast_mut::<MidiIn>()
            .unwrap()
            .note_on(v as f32, 0.8);
    }
    for _ in 0..2000 {
        cp.process_block();
    }
    let mut runs = Vec::new();
    for _ in 0..25 {
        let t = Instant::now();
        for _ in 0..4000 {
            cp.process_block();
        }
        runs.push(t.elapsed().as_nanos() as f64 / 4000.0);
    }
    runs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let med = runs[runs.len() / 2];
    println!("{name}: median {:.0} ns/block (min {:.0}, max {:.0}) = {:.2} % of the 1333 us block budget", med, runs[0], runs[runs.len()-1], med / 1_333_333.0 * 100.0);
}

fn main() {
    bench("current, routeless reference", &routeless());
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/reference");
    let full = kabl_core::load(&dir).unwrap().state().clone();
    bench("current, reference with 6 routes", &full);
    let mut key = full.clone();
    key.modules
        .get_mut(&4)
        .unwrap()
        .params
        .insert("timing".into(), 1.0);
    bench("current, reference, key-trigger", &key);

    // Swap cost on the audio thread: receive_swap with no fade in flight runs carry_state.
    let collector = basedrop::Collector::new();
    let h = collector.handle();
    let mut engine = kabl_engine::patch_engine::PatchEngine::new(&h, &full, 48000.0, 8).unwrap();
    let (mut l, mut r) = ([0.0; 64], [0.0; 64]);
    let mut times = Vec::new();
    for _ in 0..200 {
        let g = basedrop::Owned::new(&h, compile(&full, 48000.0, 8).unwrap());
        let t = Instant::now();
        engine.receive_swap(g);
        times.push(t.elapsed().as_nanos() as f64);
        for _ in 0..13 {
            engine.process_block(&mut l, &mut r);
        }
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "receive_swap (audio-thread state carry, 8 voices): median {:.0} ns, max {:.0} ns",
        times[times.len() / 2],
        times[times.len() - 1]
    );
}
