//! The `patches/interlocking` demo: one clock, a 7-step bass line on every 16th and a 5-step
//! line on every 8th (through `clock.div`), so the two drift against each other and meet again
//! every 70 sixteenths. `clock.reset` goes to the divider and both sequencers.

use kabl_core::{ModuleId, PortRef, Vec2};
use kabl_ui::PatchEditor;

fn port(id: ModuleId, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

fn at(x: f32, row: usize) -> Vec2 {
    Vec2 {
        x,
        y: kabl_ui::rack::row_y(row),
    }
}

pub fn interlocking() -> PatchEditor {
    let mut e = PatchEditor::new();
    let clock = e.add_module("clock", at(24.0, 0));
    let div = e.add_module("clock.div", at(174.0, 0));
    let bass = e.add_module("seq", at(444.0, 0));
    let lead = e.add_module("seq", at(24.0, 1));
    let lfo = e.add_module("lfo", at(684.0, 1));
    let mixer = e.add_module("mixer", at(894.0, 1));
    let out = e.add_module("out", at(1134.0, 1));
    let osc_a = e.add_module("osc.va", at(24.0, 2));
    let filt_a = e.add_module("filter.svf", at(234.0, 2));
    let env_a = e.add_module("env.adsr", at(474.0, 2));
    let vca_a = e.add_module("vca", at(744.0, 2));
    let osc_b = e.add_module("osc.va", at(24.0, 3));
    let filt_b = e.add_module("filter.svf", at(234.0, 3));
    let env_b = e.add_module("env.adsr", at(474.0, 3));
    let vca_b = e.add_module("vca", at(744.0, 3));

    let set = |e: &mut PatchEditor, id, p: &str, v: f32| e.set_param(id, p, v);
    set(&mut e, clock, "bpm", 116.0);
    set(&mut e, div, "div", 2.0);

    // Bass: E minor pentatonic around E2, 7 steps on 16ths, all on.
    for (k, p) in [4.0, 16.0, 11.0, 4.0, 14.0, 7.0, 9.0].iter().enumerate() {
        set(&mut e, bass, &format!("p{}", k + 1), *p);
    }
    set(&mut e, bass, "length", 7.0);
    set(&mut e, bass, "transpose", -24.0);

    // Lead: 5 steps on 8ths with one rest, E4 up, two octaves above the bass.
    for (k, p) in [4.0, 7.0, 11.0, 9.0, 14.0].iter().enumerate() {
        set(&mut e, lead, &format!("p{}", k + 1), *p);
    }
    set(&mut e, lead, "g4", 0.0);
    set(&mut e, lead, "length", 5.0);

    // Voice A: saw bass, short plucky filter envelope, slow LFO sweep on the cutoff.
    set(&mut e, osc_a, "waveform", 2.0);
    set(&mut e, filt_a, "cutoff_hz", 380.0);
    set(&mut e, filt_a, "resonance", 0.55);
    set(&mut e, env_a, "attack_ms", 1.0);
    set(&mut e, env_a, "decay_ms", 140.0);
    set(&mut e, env_a, "sustain", 0.0);
    set(&mut e, env_a, "release_ms", 60.0);
    set(&mut e, vca_a, "gain", 0.0);

    // Voice B: square lead, brighter, longer tail.
    set(&mut e, osc_b, "waveform", 3.0);
    set(&mut e, filt_b, "cutoff_hz", 1400.0);
    set(&mut e, filt_b, "resonance", 0.35);
    set(&mut e, env_b, "attack_ms", 4.0);
    set(&mut e, env_b, "decay_ms", 260.0);
    set(&mut e, env_b, "sustain", 0.3);
    set(&mut e, env_b, "release_ms", 320.0);
    set(&mut e, vca_b, "gain", 0.0);

    set(&mut e, lfo, "rate_hz", 0.08);
    set(&mut e, lfo, "waveform", 1.0);

    // Clock and reset: the reset line is patched, not implied.
    e.connect(port(clock, "gate"), port(bass, "clock"));
    e.connect(port(clock, "gate"), port(div, "clock"));
    e.connect(port(div, "gate"), port(lead, "clock"));
    for to in [div, bass, lead] {
        e.connect(port(clock, "reset"), port(to, "reset"));
    }

    for (seq, osc, filt, env, vca, ch) in [
        (bass, osc_a, filt_a, env_a, vca_a, "in1"),
        (lead, osc_b, filt_b, env_b, vca_b, "in2"),
    ] {
        e.connect(port(seq, "pitch"), port(osc, "pitch"));
        e.connect(port(seq, "gate"), port(env, "gate"));
        e.connect(port(osc, "out"), port(filt, "in"));
        e.connect(port(filt, "lp"), port(vca, "in"));
        e.connect(port(env, "out"), port(vca, "cv"));
        e.connect(port(vca, "out"), port(mixer, ch));
    }
    let r = e.connect_route(port(env_a, "out"), filt_a, "cutoff_hz");
    e.set_route_amount(r, 0.35, true);
    let r = e.connect_route(port(lfo, "out"), filt_a, "cutoff_hz");
    e.set_route_amount(r, 0.15, true);
    let r = e.connect_route(port(lfo, "out"), filt_b, "cutoff_hz");
    e.set_route_amount(r, -0.2, true);

    set(&mut e, mixer, "level1", 0.22);
    set(&mut e, mixer, "level2", 0.12);
    e.connect(port(mixer, "out"), port(out, "left"));
    e.connect(port(mixer, "out"), port(out, "right"));
    e
}

/// Not a check: writes `patches/interlocking`.
/// `cargo test -p kabl-ui --test interlocking -- --ignored`.
#[test]
#[ignore]
fn write_interlocking_patch() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/interlocking");
    kabl_core::save(&dir, interlocking().log()).unwrap();
}

/// The committed patch is what `interlocking()` builds.
#[test]
fn committed_patch_matches_the_builder() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/interlocking");
    let saved = kabl_core::load(&dir).unwrap();
    assert_eq!(saved.state(), interlocking().state());
    for m in saved.state().modules.values() {
        let info = kabl_modules::registry::info_for(&m.kind).unwrap();
        for p in info.params {
            if let Some(&v) = m.params.get(p.name) {
                assert!((p.min..=p.max).contains(&v), "{} {} = {v}", m.kind, p.name);
            }
        }
    }
}
