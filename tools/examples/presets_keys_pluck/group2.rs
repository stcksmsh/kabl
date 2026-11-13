//! Group 2: the rest of the keys and plucks.

use super::super::*;
use super::{pst, va};

// ---- keys -------------------------------------------------------------------------------------

/// Tin Roof: a Wurlitzer-like electric piano, reedy and barking.
fn tin_roof() -> Rack {
    let mut r = Rack::new();
    let knob = macros(
        &mut r,
        &["Tone", "Bark", "Room", "Chorus"],
        &[0.50, 0.40, 0.30, 0.40],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let ops: [Op; 6] = [
        [1.0, 0.0, 1.0, 0.5, 1200.0, 0.22, 120.0, 0.3],
        [1.0, 0.0, 0.7, 0.5, 320.0, 0.1, 100.0, 0.95],
        [2.0, 0.0, 0.45, 0.5, 700.0, 0.12, 100.0, 0.3],
        [1.0, 0.0, 0.35, 0.5, 90.0, 0.0, 60.0, 0.9],
        [1.0, 7.0, 0.5, 0.5, 1200.0, 0.22, 120.0, 0.3],
        [1.0, 0.0, 0.2, 0.5, 400.0, 0.05, 100.0, 0.5],
    ];
    let v = fm(&mut r, m, 1.0, 0.5, 0.0, &ops);
    post(
        &mut r,
        knob,
        v,
        &Post {
            drive: Some((9.0, -3.0, 0.4)),
            cutoff: 3800.0,
            resonance: 0.1,
            cut_m1: 0.5,
            trim_db: 5.0,
            chorus: Some((
                &[("rate_hz", 0.9), ("depth", 35.0), ("mix", 0.0)],
                Some(0.5),
            )),
            echo: &[
                ("time_ms", 290.0),
                ("feedback", 25.0),
                ("mix", 0.0),
                ("tone_hz", 4000.0),
            ],
            echo_max: 0.15,
            room_decay: 1.2,
            room_max: 0.25,
            ..pst()
        },
    );
    r.route(knob, "m1", v, "index", 0.4);
    perform(&mut r, knob, None, 4);
    r
}

/// A clock and a beat envelope (`clock.div` into a fast-decay `env.adsr`) that ducks the music
/// silently; no drum is built in. Returns (clock, envelope).
fn beat_duck(r: &mut Rack, bpm: f32) -> (ModuleId, ModuleId) {
    let clock = r.add("clock", &[("bpm", bpm), ("swing", 25.0)]);
    let beat = r.add("clock.div", &[("div", 4.0)]);
    r.wire(clock, "gate", beat, "clock");
    r.wire(clock, "reset", beat, "reset");
    let env = r.add("env.adsr", &adsr(1.0, 230.0, 0.0, 50.0));
    r.wire(beat, "gate", env, "gate");
    (clock, env)
}

/// Turnstile: a held chord that ducks on every beat of the host clock: a silent envelope, no drum.
fn turnstile() -> Rack {
    let mut r = Rack::new();
    let knob = macros(
        &mut r,
        &["Cutoff", "Crunch", "Room", "Pump"],
        &[0.50, 0.50, 0.25, 0.65],
    );
    let (clock, duck) = beat_duck(&mut r, 123.0);
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let v = va(
        &mut r,
        m,
        &[("waveform", 2.0), ("unison", 5.0), ("detune", 14.0)],
        &[("waveform", 3.0), ("fine", -6.0)],
        (0.5, 0.35),
        adsr(2.0, 300.0, 0.7, 120.0),
        adsr(2.0, 250.0, 0.4, 120.0),
    );
    let s = post(
        &mut r,
        knob,
        v.out,
        &Post {
            drive: Some((14.0, -5.0, 0.35)),
            cutoff: 1900.0,
            resonance: 0.2,
            cut_m1: 0.5,
            trim_db: 9.0,
            phaser: Some((
                &[
                    ("stages", 4.0),
                    ("rate_hz", 0.4),
                    ("depth", 80.0),
                    ("feedback", 50.0),
                    ("mix", 40.0),
                ],
                None,
            )),
            comp: Some(&[
                ("threshold_db", -34.0),
                ("ratio", 1.0),
                ("attack_ms", 1.0),
                ("release_ms", 230.0),
                ("knee_db", 3.0),
                ("makeup_db", 4.0),
            ]),
            echo: &[
                ("sync", 3.0),
                ("time_ms", 370.0),
                ("feedback", 28.0),
                ("mix", 0.0),
                ("tone_hz", 3800.0),
            ],
            echo_max: 0.2,
            clock: Some(clock),
            room_decay: 0.6,
            room_max: 0.15,
            ..pst()
        },
    );
    r.route(v.fenv, "out", s.lad, "cutoff_hz", 0.3);
    r.route(m, "velocity", s.lad, "cutoff_hz", 0.1);
    r.depth_route(duck, "out", s.trim, "gain_db", knob, "m4", -0.25);
    perform(&mut r, knob, Some(clock), 4);
    r
}

/// Hand Bell: an inharmonic FM bell that rings for seconds.
fn hand_bell() -> Rack {
    let mut r = Rack::new();
    let knob = macros(
        &mut r,
        &["Strike", "Grit", "Space", "Shimmer"],
        &[0.50, 0.15, 0.55, 0.40],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    // Three inharmonic pairs, carrier and modulator: 1 : 3.5, 2 : 5.04, 3.1 : 8.9.
    let ops: [Op; 6] = [
        [1.0, 0.0, 1.0, 0.5, 5500.0, 0.0, 600.0, 0.3],
        [3.0, 267.0, 0.6, 0.5, 1800.0, 0.0, 300.0, 0.8],
        [2.0, 0.0, 0.7, 0.5, 4200.0, 0.0, 500.0, 0.3],
        [5.0, 14.0, 0.5, 0.5, 1200.0, 0.0, 250.0, 0.8],
        [3.0, 57.0, 0.5, 0.5, 3000.0, 0.0, 400.0, 0.3],
        [9.0, -19.0, 0.4, 0.5, 700.0, 0.0, 200.0, 0.8],
    ];
    let v = fm(&mut r, m, 1.0, 0.45, 0.0, &ops);
    post(
        &mut r,
        knob,
        v,
        &Post {
            drive: Some((0.0, 0.0, 0.3)),
            cutoff: 7000.0,
            cut_m1: 0.3,
            trim_db: 9.0,
            chorus: Some((
                &[("rate_hz", 0.3), ("depth", 45.0), ("mix", 0.0)],
                Some(0.5),
            )),
            echo: &[
                ("time_ms", 520.0),
                ("feedback", 42.0),
                ("mix", 0.0),
                ("tone_hz", 5000.0),
            ],
            echo_max: 0.3,
            room_decay: 5.0,
            room_max: 0.4,
            ..pst()
        },
    );
    r.route(knob, "m1", v, "index", 0.5);
    perform(&mut r, knob, None, 4);
    r
}

/// Corduroy: a funk clavinet, a pulse with a snapping filter.
fn corduroy() -> Rack {
    let mut r = Rack::new();
    let knob = macros(
        &mut r,
        &["Cutoff", "Drive", "Room", "Phase"],
        &[0.50, 0.30, 0.25, 0.30],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let v = va(
        &mut r,
        m,
        &[("waveform", 1.0), ("pw", 28.0)],
        &[("waveform", 2.0), ("fine", 4.0)],
        (0.6, 0.3),
        adsr(0.5, 170.0, 0.2, 50.0),
        adsr(0.5, 120.0, 0.0, 50.0),
    );
    let s = post(
        &mut r,
        knob,
        v.out,
        &Post {
            drive: Some((8.0, -3.0, 0.35)),
            cutoff: 1500.0,
            resonance: 0.3,
            cut_m1: 0.5,
            trim_db: 11.0,
            phaser: Some((
                &[
                    ("stages", 4.0),
                    ("rate_hz", 0.6),
                    ("depth", 70.0),
                    ("feedback", 35.0),
                    ("mix", 0.0),
                ],
                Some(("mix", 0.55)),
            )),
            echo: &[
                ("time_ms", 300.0),
                ("feedback", 22.0),
                ("mix", 0.0),
                ("tone_hz", 4000.0),
            ],
            echo_max: 0.1,
            room_decay: 0.8,
            room_max: 0.2,
            ..pst()
        },
    );
    r.route(v.fenv, "out", s.lad, "cutoff_hz", 0.45);
    r.route(m, "velocity", s.lad, "cutoff_hz", 0.25);
    perform(&mut r, knob, None, 4);
    r
}

/// Matinee: a wide detuned-saw keyboard with a soft filter and a big room.
fn matinee() -> Rack {
    let mut r = Rack::new();
    let knob = macros(
        &mut r,
        &["Cutoff", "Warmth", "Space", "Width"],
        &[0.55, 0.25, 0.60, 0.50],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let v = va(
        &mut r,
        m,
        &[("waveform", 2.0), ("unison", 7.0), ("detune", 12.0)],
        &[("waveform", 3.0), ("fine", 4.0)],
        (0.5, 0.3),
        adsr(8.0, 500.0, 0.6, 400.0),
        adsr(8.0, 600.0, 0.4, 400.0),
    );
    let s = post(
        &mut r,
        knob,
        v.out,
        &Post {
            drive: Some((4.0, -2.0, 0.4)),
            cutoff: 2800.0,
            resonance: 0.12,
            cut_m1: 0.5,
            trim_db: 6.0,
            chorus: Some((&[("rate_hz", 0.6), ("depth", 50.0), ("mix", 30.0)], None)),
            echo: &[
                ("time_ms", 430.0),
                ("feedback", 38.0),
                ("mix", 0.0),
                ("tone_hz", 3500.0),
            ],
            echo_max: 0.3,
            room_decay: 3.5,
            room_max: 0.4,
            ..pst()
        },
    );
    r.route(v.fenv, "out", s.lad, "cutoff_hz", 0.25);
    r.route(m, "velocity", s.lad, "cutoff_hz", 0.15);
    r.route(knob, "m4", v.osc_a, "detune", 0.5);
    perform(&mut r, knob, None, 4);
    r
}

/// Snowmelt: a soft, slow electric piano in a big room.
fn snowmelt() -> Rack {
    let mut r = Rack::new();
    let knob = macros(
        &mut r,
        &["Tine", "Warmth", "Space", "Drift"],
        &[0.40, 0.20, 0.65, 0.50],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let ops: [Op; 6] = [
        [1.0, 0.0, 1.0, 12.0, 3500.0, 0.2, 900.0, 0.2],
        [1.0, 0.0, 0.4, 12.0, 900.0, 0.05, 600.0, 0.6],
        [2.0, 0.0, 0.3, 12.0, 2600.0, 0.15, 700.0, 0.2],
        [1.0, 0.0, 0.15, 5.0, 200.0, 0.0, 150.0, 0.5],
        [1.0, 3.0, 0.45, 12.0, 3500.0, 0.2, 900.0, 0.2],
        [1.0, 0.0, 0.2, 12.0, 1400.0, 0.1, 600.0, 0.3],
    ];
    let v = fm(&mut r, m, 1.0, 0.3, 0.0, &ops);
    post(
        &mut r,
        knob,
        v,
        &Post {
            drive: Some((0.0, 0.0, 0.25)),
            cutoff: 3200.0,
            cut_m1: 0.5,
            trim_db: 7.0,
            chorus: Some((
                &[("rate_hz", 0.25), ("depth", 55.0), ("mix", 0.0)],
                Some(0.6),
            )),
            echo: &[
                ("time_ms", 640.0),
                ("feedback", 45.0),
                ("mix", 0.0),
                ("tone_hz", 3500.0),
            ],
            echo_max: 0.3,
            room_decay: 6.0,
            room_max: 0.5,
            ..pst()
        },
    );
    r.route(knob, "m1", v, "index", 0.6);
    r.route(knob, "m4", v, "fine", 0.2);
    perform(&mut r, knob, None, 4);
    r
}

/// Scaffold: a dark, wide stab for drum and bass.
fn scaffold() -> Rack {
    let mut r = Rack::new();
    let knob = macros(
        &mut r,
        &["Cutoff", "Grit", "Room", "Length"],
        &[0.45, 0.55, 0.25, 0.35],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let v = va(
        &mut r,
        m,
        &[("waveform", 2.0), ("unison", 6.0), ("detune", 26.0)],
        &[
            ("waveform", 2.0),
            ("fine", -12.0),
            ("unison", 2.0),
            ("detune", 18.0),
        ],
        (0.45, 0.4),
        adsr(1.0, 230.0, 0.0, 110.0),
        adsr(1.0, 260.0, 0.1, 110.0),
    );
    let s = post(
        &mut r,
        knob,
        v.out,
        &Post {
            drive: Some((10.0, -4.0, 0.4)),
            cutoff: 1100.0,
            resonance: 0.3,
            cut_m1: 0.55,
            trim_db: 9.0,
            phaser: Some((
                &[
                    ("stages", 6.0),
                    ("rate_hz", 0.2),
                    ("depth", 75.0),
                    ("feedback", 45.0),
                    ("mix", 35.0),
                ],
                None,
            )),
            comp: Some(&[
                ("threshold_db", -22.0),
                ("ratio", 4.0),
                ("attack_ms", 8.0),
                ("release_ms", 120.0),
                ("makeup_db", 4.0),
            ]),
            echo: &[
                ("time_ms", 258.0),
                ("feedback", 30.0),
                ("mix", 0.0),
                ("tone_hz", 3500.0),
            ],
            echo_max: 0.2,
            room_decay: 0.9,
            room_max: 0.2,
            ..pst()
        },
    );
    r.route(v.fenv, "out", s.lad, "cutoff_hz", 0.4);
    r.route(m, "velocity", s.lad, "cutoff_hz", 0.15);
    r.route(knob, "m4", v.aenv, "decay_ms", 0.5);
    perform(&mut r, knob, None, 4);
    r
}

/// Brass Filament: a sustained, metallic, slightly unstable FM tone.
fn brass_filament() -> Rack {
    let mut r = Rack::new();
    let knob = macros(
        &mut r,
        &["Strike", "Rust", "Space", "Warp"],
        &[0.45, 0.25, 0.45, 0.30],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    // Algorithm 5: operator 6 modulates 1 to 5. Inharmonic ratios and a long fade.
    let ops: [Op; 6] = [
        [1.0, 0.0, 1.0, 3.0, 2500.0, 0.3, 500.0, 0.2],
        [3.0, 140.0, 0.6, 3.0, 2000.0, 0.25, 400.0, 0.4],
        [2.0, 0.0, 0.5, 3.0, 1800.0, 0.2, 400.0, 0.3],
        [5.0, -90.0, 0.4, 3.0, 1200.0, 0.1, 300.0, 0.5],
        [7.0, 30.0, 0.25, 3.0, 800.0, 0.0, 250.0, 0.6],
        [1.0, 9.0, 0.5, 3.0, 1800.0, 0.3, 400.0, 0.5],
    ];
    let v = fm(&mut r, m, 5.0, 0.4, 0.3, &ops);
    post(
        &mut r,
        knob,
        v,
        &Post {
            drive: Some((5.0, -2.0, 0.4)),
            cutoff: 5500.0,
            cut_m1: 0.4,
            trim_db: 7.0,
            chorus: Some((&[("rate_hz", 0.5), ("depth", 40.0), ("mix", 25.0)], None)),
            echo: &[
                ("time_ms", 360.0),
                ("feedback", 40.0),
                ("mix", 0.0),
                ("tone_hz", 4500.0),
            ],
            echo_max: 0.25,
            room_decay: 3.0,
            room_max: 0.35,
            ..pst()
        },
    );
    r.route(knob, "m1", v, "index", 0.5);
    r.route(knob, "m4", v, "feedback", 0.7);
    perform(&mut r, knob, None, 4);
    r
}

/// Spinet: a bright harpsichord-like key with a buzzy pluck and no dynamics to speak of.
fn spinet() -> Rack {
    let mut r = Rack::new();
    let knob = macros(
        &mut r,
        &["Tone", "Buzz", "Room", "Chorus"],
        &[0.50, 0.30, 0.35, 0.35],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let v = va(
        &mut r,
        m,
        &[("waveform", 1.0), ("pw", 20.0)],
        &[("waveform", 2.0), ("fine", 9.0)],
        (0.5, 0.35),
        adsr(0.5, 650.0, 0.0, 60.0),
        adsr(0.5, 300.0, 0.15, 60.0),
    );
    let s = post(
        &mut r,
        knob,
        v.out,
        &Post {
            drive: Some((6.0, -3.0, 0.35)),
            cutoff: 4200.0,
            resonance: 0.12,
            cut_m1: 0.45,
            trim_db: 9.0,
            chorus: Some((
                &[("rate_hz", 0.7), ("depth", 30.0), ("mix", 0.0)],
                Some(0.5),
            )),
            echo: &[
                ("time_ms", 330.0),
                ("feedback", 25.0),
                ("mix", 0.0),
                ("tone_hz", 5000.0),
            ],
            echo_max: 0.15,
            room_decay: 1.8,
            room_max: 0.35,
            ..pst()
        },
    );
    r.route(v.fenv, "out", s.lad, "cutoff_hz", 0.3);
    perform(&mut r, knob, None, 4);
    r
}

// ---- plucks -----------------------------------------------------------------------------------

/// Willow: a soft harp.
fn willow() -> Rack {
    let mut r = Rack::new();
    let knob = macros(
        &mut r,
        &["Pluck", "Warmth", "Space", "Stretch"],
        &[0.50, 0.15, 0.60, 0.45],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let ops: [Op; 6] = [
        [1.0, 0.0, 1.0, 0.5, 1800.0, 0.0, 500.0, 0.25],
        [2.0, 0.0, 0.3, 0.5, 90.0, 0.0, 60.0, 0.8],
        [2.0, 4.0, 0.4, 0.5, 1100.0, 0.0, 400.0, 0.25],
        [3.0, 0.0, 0.2, 0.5, 60.0, 0.0, 40.0, 0.9],
        [4.0, 0.0, 0.15, 0.5, 500.0, 0.0, 250.0, 0.3],
        [1.0, 0.0, 0.0, 0.5, 100.0, 0.0, 50.0, 0.0],
    ];
    let v = fm(&mut r, m, 1.0, 0.4, 0.0, &ops);
    post(
        &mut r,
        knob,
        v,
        &Post {
            drive: Some((0.0, 0.0, 0.3)),
            cutoff: 6500.0,
            cut_m1: 0.35,
            trim_db: 10.0,
            chorus: Some((&[("rate_hz", 0.35), ("depth", 35.0), ("mix", 18.0)], None)),
            echo: &[
                ("time_ms", 460.0),
                ("feedback", 40.0),
                ("mix", 0.0),
                ("tone_hz", 5000.0),
            ],
            echo_max: 0.3,
            room_decay: 4.5,
            room_max: 0.45,
            ..pst()
        },
    );
    r.route(knob, "m1", v, "index", 0.5);
    r.route(knob, "m4", v, "decay1", 0.5);
    r.route(knob, "m4", v, "decay3", 0.5);
    perform(&mut r, knob, None, 4);
    r
}

/// Dry Creek: a nylon-string guitar pluck.
fn dry_creek() -> Rack {
    let mut r = Rack::new();
    let knob = macros(
        &mut r,
        &["Pick", "Warmth", "Room", "Chorus"],
        &[0.50, 0.20, 0.40, 0.30],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let ops: [Op; 6] = [
        [1.0, 0.0, 1.0, 0.5, 900.0, 0.0, 120.0, 0.3],
        [1.0, 0.0, 0.5, 0.5, 220.0, 0.0, 60.0, 0.8],
        [2.0, 0.0, 0.4, 0.5, 520.0, 0.0, 100.0, 0.3],
        [3.0, 0.0, 0.3, 0.5, 45.0, 0.0, 30.0, 0.9],
        [1.0, 3.0, 0.4, 0.5, 900.0, 0.0, 120.0, 0.3],
        [5.0, 0.0, 0.2, 0.5, 30.0, 0.0, 20.0, 0.8],
    ];
    let v = fm(&mut r, m, 1.0, 0.5, 0.0, &ops);
    post(
        &mut r,
        knob,
        v,
        &Post {
            drive: Some((3.0, -1.0, 0.35)),
            cutoff: 4500.0,
            cut_m1: 0.45,
            trim_db: 11.0,
            chorus: Some((
                &[("rate_hz", 0.5), ("depth", 30.0), ("mix", 0.0)],
                Some(0.5),
            )),
            echo: &[
                ("time_ms", 310.0),
                ("feedback", 25.0),
                ("mix", 0.0),
                ("tone_hz", 4500.0),
            ],
            echo_max: 0.15,
            room_decay: 1.8,
            room_max: 0.35,
            ..pst()
        },
    );
    r.route(knob, "m1", v, "index", 0.5);
    perform(&mut r, knob, None, 4);
    r
}

/// Gumdrop: a short, bouncy pop pluck.
fn gumdrop() -> Rack {
    let mut r = Rack::new();
    let knob = macros(
        &mut r,
        &["Cutoff", "Drive", "Space", "Decay"],
        &[0.55, 0.20, 0.45, 0.40],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let v = va(
        &mut r,
        m,
        &[
            ("waveform", 1.0),
            ("pw", 40.0),
            ("unison", 2.0),
            ("detune", 8.0),
        ],
        &[("waveform", 2.0), ("fine", 6.0)],
        (0.5, 0.3),
        adsr(0.5, 260.0, 0.0, 120.0),
        adsr(0.5, 200.0, 0.0, 120.0),
    );
    let s = post(
        &mut r,
        knob,
        v.out,
        &Post {
            drive: Some((4.0, -2.0, 0.4)),
            cutoff: 1500.0,
            resonance: 0.35,
            cut_m1: 0.5,
            trim_db: 10.0,
            chorus: Some((&[("rate_hz", 0.6), ("depth", 35.0), ("mix", 25.0)], None)),
            echo: &[
                ("time_ms", 360.0),
                ("feedback", 32.0),
                ("mix", 0.0),
                ("tone_hz", 5000.0),
            ],
            echo_max: 0.25,
            room_decay: 2.2,
            room_max: 0.3,
            ..pst()
        },
    );
    r.route(v.fenv, "out", s.lad, "cutoff_hz", 0.4);
    r.route(m, "velocity", s.lad, "cutoff_hz", 0.15);
    r.route(knob, "m4", v.aenv, "decay_ms", 0.5);
    perform(&mut r, knob, None, 4);
    r
}

/// Cold Cut: a dark, short bell-pluck for trap.
fn cold_cut() -> Rack {
    let mut r = Rack::new();
    let knob = macros(
        &mut r,
        &["Tone", "Dirt", "Room", "Decay"],
        &[0.40, 0.35, 0.30, 0.40],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let ops: [Op; 6] = [
        [1.0, 0.0, 1.0, 0.5, 700.0, 0.0, 150.0, 0.3],
        [3.0, 267.0, 0.5, 0.5, 200.0, 0.0, 80.0, 0.8],
        [2.0, 0.0, 0.4, 0.5, 500.0, 0.0, 120.0, 0.3],
        [1.0, 0.0, 0.3, 0.5, 120.0, 0.0, 60.0, 0.7],
        [1.0, -8.0, 0.4, 0.5, 700.0, 0.0, 150.0, 0.3],
        [1.0, 0.0, 0.0, 0.5, 100.0, 0.0, 50.0, 0.0],
    ];
    let v = fm(&mut r, m, 1.0, 0.45, 0.0, &ops);
    post(
        &mut r,
        knob,
        v,
        &Post {
            drive: Some((10.0, -4.0, 0.4)),
            cutoff: 2800.0,
            resonance: 0.1,
            cut_m1: 0.55,
            trim_db: 11.0,
            comp: Some(&[
                ("threshold_db", -24.0),
                ("ratio", 3.0),
                ("attack_ms", 5.0),
                ("release_ms", 100.0),
                ("makeup_db", 3.0),
            ]),
            echo: &[
                ("time_ms", 290.0),
                ("feedback", 32.0),
                ("mix", 0.0),
                ("tone_hz", 3200.0),
            ],
            echo_max: 0.2,
            room_decay: 1.3,
            room_max: 0.25,
            ..pst()
        },
    );
    r.route(knob, "m1", v, "index", 0.5);
    r.route(knob, "m4", v, "decay1", 0.5);
    r.route(knob, "m4", v, "decay3", 0.5);
    perform(&mut r, knob, None, 4);
    r
}

/// Rivet Gun: a bright, phased, compressed staccato pluck.
fn rivet_gun() -> Rack {
    let mut r = Rack::new();
    let knob = macros(
        &mut r,
        &["Cutoff", "Crunch", "Room", "Phase"],
        &[0.55, 0.50, 0.25, 0.45],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let v = va(
        &mut r,
        m,
        &[
            ("waveform", 1.0),
            ("pw", 30.0),
            ("unison", 3.0),
            ("detune", 14.0),
        ],
        &[("waveform", 2.0), ("fine", 7.0)],
        (0.5, 0.35),
        adsr(0.5, 120.0, 0.0, 40.0),
        adsr(0.5, 90.0, 0.0, 40.0),
    );
    let s = post(
        &mut r,
        knob,
        v.out,
        &Post {
            drive: Some((14.0, -6.0, 0.35)),
            cutoff: 2200.0,
            resonance: 0.3,
            cut_m1: 0.5,
            trim_db: 10.0,
            phaser: Some((
                &[
                    ("stages", 4.0),
                    ("rate_hz", 0.5),
                    ("depth", 80.0),
                    ("feedback", 55.0),
                    ("mix", 0.0),
                ],
                Some(("mix", 0.6)),
            )),
            comp: Some(&[
                ("threshold_db", -22.0),
                ("ratio", 6.0),
                ("attack_ms", 3.0),
                ("release_ms", 80.0),
                ("makeup_db", 9.5),
            ]),
            echo: &[
                ("time_ms", 242.0),
                ("feedback", 26.0),
                ("mix", 0.0),
                ("tone_hz", 4000.0),
            ],
            echo_max: 0.25,
            room_decay: 0.5,
            room_max: 0.12,
            ..pst()
        },
    );
    r.route(v.fenv, "out", s.lad, "cutoff_hz", 0.4);
    r.route(m, "velocity", s.lad, "cutoff_hz", 0.15);
    perform(&mut r, knob, None, 4);
    r
}

/// Tuning Fork: tuned metal bars, soft and slightly detuned, with a slow wobble.
fn tuning_fork() -> Rack {
    let mut r = Rack::new();
    let knob = macros(
        &mut r,
        &["Strike", "Warmth", "Space", "Wobble"],
        &[0.45, 0.15, 0.55, 0.40],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    // Six added partials of a free bar: 1, 2.76, 5.4, 8.9 and two detuned doubles.
    let ops: [Op; 6] = [
        [1.0, 0.0, 1.0, 0.5, 3500.0, 0.0, 400.0, 0.3],
        [3.0, -140.0, 0.45, 0.5, 1600.0, 0.0, 300.0, 0.6],
        [5.0, 70.0, 0.3, 0.5, 700.0, 0.0, 200.0, 0.7],
        [9.0, -19.0, 0.18, 0.5, 300.0, 0.0, 100.0, 0.8],
        [1.0, 5.0, 0.6, 0.5, 3300.0, 0.0, 400.0, 0.3],
        [2.0, 8.0, 0.2, 0.5, 1900.0, 0.0, 300.0, 0.4],
    ];
    let v = fm(&mut r, m, 7.0, 1.0, 0.0, &ops);
    post(
        &mut r,
        knob,
        v,
        &Post {
            drive: Some((0.0, 0.0, 0.3)),
            cutoff: 6000.0,
            cut_m1: 0.5,
            trim_db: 12.0,
            chorus: Some((
                &[("rate_hz", 0.9), ("depth", 70.0), ("mix", 0.0)],
                Some(0.6),
            )),
            echo: &[
                ("time_ms", 480.0),
                ("feedback", 42.0),
                ("mix", 0.0),
                ("tone_hz", 4800.0),
            ],
            echo_max: 0.3,
            room_decay: 4.0,
            room_max: 0.4,
            ..pst()
        },
    );
    perform(&mut r, knob, None, 4);
    r
}

// ---- phrases ----------------------------------------------------------------------------------

fn tin_phrase() -> Vec<Hit> {
    // 100 BPM, a funky two-bar vamp.
    comp(
        100.0,
        92,
        &[
            (0.0, 0.75, &[52, 59, 62, 66]),
            (1.0, 0.5, &[52, 59, 62, 66]),
            (2.5, 0.75, &[52, 59, 62, 66]),
            (4.0, 0.75, &[57, 61, 64, 67]),
            (5.0, 0.5, &[57, 61, 64, 67]),
            (6.5, 1.0, &[55, 59, 62, 67]),
            (8.0, 3.0, &[52, 59, 62, 66, 71]),
        ],
    )
}

fn turnstile_phrase() -> Vec<Hit> {
    comp(
        123.0,
        90,
        &[
            (0.0, 4.0, &[52, 59, 62, 67]),
            (4.0, 4.0, &[50, 57, 62, 66]),
            (8.0, 4.0, &[48, 55, 59, 64]),
            (12.0, 4.0, &[50, 57, 62, 66]),
        ],
    )
}

fn bell_phrase() -> Vec<Hit> {
    let notes = [
        (0.0, 60u8),
        (1.3, 67),
        (2.1, 64),
        (3.8, 72),
        (4.6, 71),
        (6.5, 62),
        (8.0, 55),
    ];
    line(&notes.map(|(t, n)| (t, 0.4, n, 88)))
}

fn corduroy_phrase() -> Vec<Hit> {
    comp(
        104.0,
        96,
        &[
            (0.0, 0.5, &[52, 59, 64]),
            (0.75, 0.25, &[52, 59, 64]),
            (1.5, 0.5, &[55, 62, 67]),
            (2.0, 0.25, &[52, 59, 64]),
            (3.0, 0.5, &[50, 57, 62]),
            (4.0, 0.5, &[52, 59, 64]),
            (4.75, 0.25, &[52, 59, 64]),
            (5.5, 0.5, &[57, 64, 69]),
            (6.5, 0.5, &[55, 62, 67]),
            (7.0, 0.5, &[52, 59, 64]),
        ],
    )
}

fn matinee_phrase() -> Vec<Hit> {
    comp(
        90.0,
        85,
        &[
            (0.0, 4.0, &[45, 57, 60, 64]),
            (4.0, 4.0, &[41, 57, 60, 65]),
            (8.0, 4.0, &[48, 55, 60, 64]),
            (12.0, 4.0, &[43, 55, 59, 62]),
        ],
    )
}

fn snow_phrase() -> Vec<Hit> {
    comp(
        70.0,
        80,
        &[
            (0.0, 5.0, &[48, 55, 59, 62, 67]),
            (6.0, 5.0, &[45, 52, 57, 60, 64]),
            (12.0, 5.0, &[41, 48, 55, 57, 62]),
        ],
    )
}

fn scaffold_phrase() -> Vec<Hit> {
    // 174 BPM, a rolling two-bar stab.
    comp(
        174.0,
        100,
        &[
            (0.0, 0.5, &[45, 57, 60, 64]),
            (1.5, 0.5, &[45, 57, 60, 64]),
            (3.0, 0.5, &[45, 57, 60, 64]),
            (4.0, 0.5, &[43, 55, 59, 62]),
            (5.5, 0.5, &[43, 55, 59, 62]),
            (7.0, 0.5, &[41, 53, 57, 60]),
            (8.0, 0.5, &[45, 57, 60, 64]),
            (9.5, 0.5, &[45, 57, 60, 64]),
            (11.0, 0.5, &[48, 60, 64, 67]),
            (12.0, 1.0, &[45, 57, 60, 64]),
        ],
    )
}

fn filament_phrase() -> Vec<Hit> {
    comp(
        60.0,
        90,
        &[
            (0.0, 5.0, &[48, 55]),
            (5.0, 5.0, &[50, 57, 65]),
            (10.0, 6.0, &[46, 53, 60]),
        ],
    )
}

fn spinet_phrase() -> Vec<Hit> {
    let notes = [
        64u8, 67, 71, 74, 72, 69, 66, 64, 62, 66, 69, 74, 71, 67, 64, 59,
    ];
    line(
        &notes
            .iter()
            .enumerate()
            .map(|(i, &n)| (i as f32 * 0.2, 0.16, n, 90))
            .collect::<Vec<_>>(),
    )
}

fn willow_phrase() -> Vec<Hit> {
    // Arpeggiated chords, rolled up.
    let mut v = Vec::new();
    for (k, chord) in [
        [48u8, 55, 60, 64, 67, 72],
        [45, 52, 57, 60, 64, 69],
        [41, 48, 53, 57, 60, 65],
    ]
    .iter()
    .enumerate()
    {
        for (i, &n) in chord.iter().enumerate() {
            let t = k as f32 * 2.4 + i as f32 * 0.12;
            v.extend(line(&[(t, 0.2, n, 75 + (i as u8 * 4))]));
        }
    }
    v
}

fn creek_phrase() -> Vec<Hit> {
    let pat: [(f32, u8); 16] = [
        (0.0, 45),
        (0.3, 52),
        (0.6, 57),
        (0.9, 60),
        (1.2, 64),
        (1.5, 60),
        (1.8, 57),
        (2.1, 52),
        (2.4, 43),
        (2.7, 50),
        (3.0, 55),
        (3.3, 59),
        (3.6, 62),
        (3.9, 59),
        (4.2, 55),
        (4.5, 50),
    ];
    line(&pat.map(|(t, n)| (t, 0.25, n, 85)))
}

fn gumdrop_phrase() -> Vec<Hit> {
    // 110 BPM, a bouncing pop riff.
    let beat = 60.0 / 110.0;
    let riff: [(f32, u8); 12] = [
        (0.0, 64),
        (0.5, 64),
        (1.0, 67),
        (1.5, 71),
        (2.0, 69),
        (2.5, 67),
        (3.0, 64),
        (4.0, 62),
        (4.5, 62),
        (5.0, 66),
        (5.5, 69),
        (6.0, 67),
    ];
    line(&riff.map(|(b, n)| (b * beat, beat * 0.4, n, 92)))
}

fn cut_phrase() -> Vec<Hit> {
    // 140 BPM half-time: a minor figure with room to ring.
    let beat = 60.0 / 140.0;
    let riff: [(f32, u8); 9] = [
        (0.0, 69),
        (1.5, 72),
        (3.0, 76),
        (4.0, 69),
        (5.5, 72),
        (6.0, 67),
        (8.0, 69),
        (9.5, 76),
        (11.0, 74),
    ];
    line(&riff.map(|(b, n)| (b * beat, beat * 0.9, n, 95)))
}

fn rivet_phrase() -> Vec<Hit> {
    let s16 = 60.0 / 125.0 / 4.0;
    let pat: [(u32, u8, u8); 14] = [
        (0, 64, 110),
        (2, 64, 90),
        (3, 71, 100),
        (6, 69, 100),
        (8, 64, 110),
        (10, 67, 90),
        (11, 71, 100),
        (14, 74, 100),
        (16, 62, 110),
        (18, 62, 90),
        (19, 69, 100),
        (22, 67, 100),
        (24, 62, 110),
        (27, 66, 100),
    ];
    line(&pat.map(|(s, n, v)| (s as f32 * s16, s16 * 0.8, n, v)))
}

fn fork_phrase() -> Vec<Hit> {
    let notes = [
        (0.0, 72u8),
        (1.0, 79),
        (2.2, 76),
        (3.5, 84),
        (5.0, 72),
        (5.6, 77),
        (7.0, 69),
    ];
    line(&notes.map(|(t, n)| (t, 0.3, n, 85)))
}

// ---- the list ---------------------------------------------------------------------------------

pub fn presets() -> Vec<Preset> {
    vec![
        Preset {
            slug: "tin-roof",
            name: "Tin Roof",
            category: "Keys",
            tags: &["poly", "fm", "reedy", "funk", "disco"],
            description: "A Wurlitzer-like electric piano: a reedy attack that barks harder when you play harder, and a quick fall to a soft sustain. Play funky stabs and short vamps. Tone opens the FM brightness and the filter, Bark pushes the drive, Room sets the echo and a short room, Chorus brings in a light chorus.",
            listen: "The bark on hard hits against the soft tone of gentle ones",
            group: 2,
            build: tin_roof,
            phrase: tin_phrase,
            secs: 13.0,
        },
        Preset {
            slug: "turnstile",
            name: "Turnstile",
            category: "Keys",
            tags: &["poly", "sequenced", "pumping", "phased", "french touch", "123bpm", "signature"],
            description: "Hold a chord and press Run: a driven, phased saw chord that ducks on every beat (a silent envelope on the host clock at 123 BPM with swing; no drum is built in). Cutoff opens the filter, Crunch pushes the drive, Room sets a synced echo and a tight room, Pump sets how hard the beat ducks the chord.",
            listen: "The chord ducking on every beat and swelling back between beats; Pump at 0 holds it steady",
            group: 2,
            build: turnstile,
            phrase: turnstile_phrase,
            secs: 10.0,
        },
        Preset {
            slug: "hand-bell",
            name: "Hand Bell",
            category: "Keys",
            tags: &["poly", "fm", "bell", "ambient", "experimental"],
            description: "An inharmonic FM bell: three pairs of operators that ring for seconds and darken as they fade. Play slow, sparse notes. Strike sets the brightness of the attack, Grit adds a little drive, Space sets the echo and a long room, Shimmer brings in a slow chorus.",
            listen: "The metal attack and the long, darkening ring; each note blurring into the room",
            group: 2,
            build: hand_bell,
            phrase: bell_phrase,
            secs: 14.0,
        },
        Preset {
            slug: "corduroy",
            name: "Corduroy",
            category: "Keys",
            tags: &["poly", "clav", "pulse", "phased", "funk"],
            description: "A funk clavinet: a narrow pulse with a hard filter snap and a short sustain. Play 16th-note chops. Cutoff opens the filter, Drive adds the bite, Room sets a short room and slapback, Phase brings in a phaser.",
            listen: "The snap of the filter on each chop, harder on loud notes",
            group: 2,
            build: corduroy,
            phrase: corduroy_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "matinee",
            name: "Matinee",
            category: "Keys",
            tags: &["poly", "supersaw", "chorus", "synthwave"],
            description: "A wide keyboard from seven detuned saws, a soft filter and a big room. Play slow chords. Cutoff opens the filter, Warmth adds a little drive, Space sets the echo and the long room, Width pulls the saws apart.",
            listen: "The width of each chord and the room trailing behind it; Width at 0 narrows it",
            group: 2,
            build: matinee,
            phrase: matinee_phrase,
            secs: 17.0,
        },
        Preset {
            slug: "snowmelt",
            name: "Snowmelt",
            category: "Keys",
            tags: &["poly", "fm", "soft", "ambient", "cinematic"],
            description: "A slow, soft electric piano in a big room: a gentle attack, a long fade and a drifting tuning. Play slow open chords. Tine opens the FM brightness and the filter, Warmth adds a little drive, Space sets the echo and a six-second room, Drift detunes the voice and slows the chorus.",
            listen: "The soft bloom of each chord and the room carrying it",
            group: 2,
            build: snowmelt,
            phrase: snow_phrase,
            secs: 19.0,
        },
        Preset {
            slug: "scaffold",
            name: "Scaffold",
            category: "Keys",
            tags: &["poly", "stab", "phased", "compressed", "drum and bass", "174bpm"],
            description: "A dark, wide stab for drum and bass: eight detuned saws, a closing filter, a phaser and a compressor. Play short rolling chords. Cutoff opens the filter, Grit pushes the drive, Room sets a slapback and a tight room, Length stretches the stab.",
            listen: "The dark body of each stab and the filter closing after it",
            group: 2,
            build: scaffold,
            phrase: scaffold_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "brass-filament",
            name: "Brass Filament",
            category: "Keys",
            tags: &["poly", "fm", "metallic", "experimental"],
            description: "A metallic FM tone that rings and wavers: inharmonic partials, a little feedback and a slow fade. Play sparse two-note intervals. Strike sets the brightness, Rust adds drive, Space sets the echo and room, Warp feeds the sound back into itself.",
            listen: "The unstable, slightly out-of-tune shimmer; Warp up makes it harsher",
            group: 2,
            build: brass_filament,
            phrase: filament_phrase,
            secs: 16.0,
        },
        Preset {
            slug: "spinet",
            name: "Spinet",
            category: "Keys",
            tags: &["poly", "pulse", "harpsichord", "world"],
            description: "A bright harpsichord-like key: a thin pulse and a saw, an instant attack and a plucked fall, with a faint buzz. Play counterpoint lines. Tone opens the filter, Buzz adds drive, Room sets the echo and a room, Chorus doubles the strings.",
            listen: "The quick pluck of each key and the buzzing ring",
            group: 2,
            build: spinet,
            phrase: spinet_phrase,
            secs: 6.0,
        },
        Preset {
            slug: "willow",
            name: "Willow",
            category: "Pluck",
            tags: &["poly", "fm", "harp", "ambient", "cinematic"],
            description: "A soft harp: a quick pluck and a long, clean ring. Play rolled chords. Pluck sets the brightness of the attack, Warmth adds a little drive, Space sets the echo and a long room, Stretch lengthens the ring.",
            listen: "The rolled chords ringing into each other",
            group: 2,
            build: willow,
            phrase: willow_phrase,
            secs: 11.0,
        },
        Preset {
            slug: "dry-creek",
            name: "Dry Creek",
            category: "Pluck",
            tags: &["poly", "fm", "guitar", "pop", "world"],
            description: "A nylon-string guitar pluck: a soft pick, a woody body and a quick decay. Play arpeggios. Pick sets the attack brightness, Warmth adds a little drive, Room sets the echo and a room, Chorus doubles the string.",
            listen: "The pick on each note and the quick, round decay",
            group: 2,
            build: dry_creek,
            phrase: creek_phrase,
            secs: 6.0,
        },
        Preset {
            slug: "gumdrop",
            name: "Gumdrop",
            category: "Pluck",
            tags: &["poly", "pluck", "chorus", "pop"],
            description: "A short, bouncy pop pluck: two pulses and a saw, a resonant filter that bounces on each note. Play catchy riffs. Cutoff opens the filter, Drive adds the bite, Space sets the echo and room, Decay stretches the note.",
            listen: "The bounce of the filter on every note",
            group: 2,
            build: gumdrop,
            phrase: gumdrop_phrase,
            secs: 6.0,
        },
        Preset {
            slug: "cold-cut",
            name: "Cold Cut",
            category: "Pluck",
            tags: &["poly", "fm", "bell", "dark", "hip-hop", "trap"],
            description: "A dark, short bell-pluck for trap: a metallic click, a hollow body and a quick fall. Play minor figures with room to ring. Tone opens the FM brightness and the filter, Dirt pushes the drive, Room sets the echo and a room, Decay lengthens the note.",
            listen: "The hollow click and the room around a sparse line",
            group: 2,
            build: cold_cut,
            phrase: cut_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "rivet-gun",
            name: "Rivet Gun",
            category: "Pluck",
            tags: &["poly", "pluck", "phased", "compressed", "french touch", "125bpm", "signature"],
            description: "A bright, staccato, phased pluck, driven and squashed: a narrow pulse with a hard filter snap. Play short syncopated riffs. Cutoff opens the filter, Crunch pushes the drive, Room sets a slapback and a tight room, Phase brings in the phaser.",
            listen: "The phaser on the staccato riff and the compressor holding it close",
            group: 2,
            build: rivet_gun,
            phrase: rivet_phrase,
            secs: 5.0,
        },
        Preset {
            slug: "tuning-fork",
            name: "Tuning Fork",
            category: "Pluck",
            tags: &["poly", "fm", "metallic", "ambient", "experimental"],
            description: "Tuned metal bars: six added partials of a free bar, a soft strike and a long ring. Play slow melodies. Strike opens the brightness, Warmth adds a little drive, Space sets the echo and a long room, Wobble brings in a slow chorus.",
            listen: "The pure ring of each bar and the slow wobble across notes",
            group: 2,
            build: tuning_fork,
            phrase: fork_phrase,
            secs: 11.0,
        },
    ]
}
