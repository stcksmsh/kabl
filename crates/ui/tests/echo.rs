//! The `patches/echo` demo: `patches/interlocking` with a delay on the lead. The lead's VCA
//! feeds the delay instead of the mixer; the delay's left and right outputs go to two mixers
//! (one per side), each with the bass on channel 1, so the bass stays dry and centred. The
//! clock's gate is patched to the delay's clock input. The slow LFO has a small route on the
//! delay time, which only moves it when sync is FREE.

#[path = "interlocking.rs"]
mod interlocking;

use kabl_core::{ModuleId, PortRef, Vec2};
use kabl_ui::PatchEditor;

// Module ids `interlocking()` assigns.
const CLOCK: ModuleId = 1;
const LFO: ModuleId = 5;
const MIXER: ModuleId = 6;
const OUT: ModuleId = 7;
const VCA_A: ModuleId = 11;
const VCA_B: ModuleId = 15;

fn port(id: ModuleId, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

pub fn echo() -> PatchEditor {
    let mut e = interlocking::interlocking();
    for (id, kind) in [
        (CLOCK, "clock"),
        (LFO, "lfo"),
        (MIXER, "mixer"),
        (OUT, "out"),
        (VCA_A, "vca"),
        (VCA_B, "vca"),
    ] {
        assert_eq!(e.state().modules[&id].kind, kind);
    }
    let into = |e: &PatchEditor, to: PortRef| {
        e.state()
            .cables
            .iter()
            .find(|(_, c)| c.to == to)
            .map(|(&id, _)| id)
            .unwrap()
    };
    let delay = e.add_module(
        "delay",
        Vec2 {
            x: 954.0,
            y: kabl_ui::rack::row_y(3),
        },
    );
    let mixer_r = e.add_module(
        "mixer",
        Vec2 {
            x: 954.0,
            y: kabl_ui::rack::row_y(2),
        },
    );

    // Lead through the delay, left and right to a mixer each; the bass on both, dry.
    let c = into(&e, port(MIXER, "in2"));
    e.disconnect(c);
    let c = into(&e, port(OUT, "right"));
    e.disconnect(c);
    e.connect(port(VCA_B, "out"), port(delay, "in"));
    e.connect(port(CLOCK, "gate"), port(delay, "clock"));
    e.connect(port(delay, "left"), port(MIXER, "in2"));
    e.connect(port(delay, "right"), port(mixer_r, "in2"));
    e.connect(port(VCA_A, "out"), port(mixer_r, "in1"));
    e.connect(port(mixer_r, "out"), port(OUT, "right"));

    // Dotted eighths against the lead's straight eighths, bouncing left and right; repeats
    // darker than the lead and gone in about two seconds.
    e.set_param(delay, "sync", 3.0);
    e.set_param(delay, "feedback", 45.0);
    e.set_param(delay, "mix", 32.0);
    e.set_param(delay, "tone_hz", 3200.0);
    e.set_param(delay, "mode", 1.0);
    e.set_param(delay, "time_ms", 420.0);
    let r = e.connect_route(port(LFO, "out"), delay, "time_ms");
    e.set_route_amount(r, 0.02, true);

    for (ch, v) in [("level1", 0.22), ("level2", 0.14)] {
        e.set_param(MIXER, ch, v);
        e.set_param(mixer_r, ch, v);
    }
    e
}

/// Not a check: writes `patches/echo`.
/// `cargo test -p kabl-ui --test echo write_echo_patch -- --ignored` (the name filter keeps the
/// included interlocking writer from rewriting `patches/interlocking`).
#[test]
#[ignore]
fn write_echo_patch() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/echo");
    kabl_core::save(&dir, echo().log()).unwrap();
}

/// The committed patch is what `echo()` builds.
#[test]
fn committed_echo_patch_matches_the_builder() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/echo");
    let saved = kabl_core::load(&dir).unwrap();
    assert_eq!(saved.state(), echo().state());
}
