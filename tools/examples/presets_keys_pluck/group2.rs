//! Group 2: the rest of the keys and plucks.

use super::super::*;
use super::{pst, va};

// ---- keys -------------------------------------------------------------------------------------

/// Tin Roof: a Wurlitzer-like electric piano, reedy and barking.
fn tin_roof() -> Rack {
    let mut r = Rack::new();
    // Reed: the reed buzz; Sustain: how long it rings; Bark: the amp overdrive; Chorus: the stereo shimmer.
    let knob = macros(
        &mut r,
        &["Reed", "Sustain", "Bark", "Chorus"],
        &[0.50, 0.50, 0.60, 0.30],
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
    let s = post(
        &mut r,
        knob,
        v,
        &Post {
            drive: Some((9.0, -3.0, 0.4)),
            cutoff: 3800.0,
            resonance: 0.1,
            cut_m1: 0.5,
            trim_db: 5.0,
            chorus: Some((&[("rate_hz", 0.9), ("depth", 35.0), ("mix", 0.0)], None)),
            echo: &[
                ("time_ms", 290.0),
                ("feedback", 25.0),
                ("mix", 0.0),
                ("tone_hz", 4000.0),
            ],
            echo_max: 0.15,
            room_decay: 1.2,
            room_max: 0.25,
            fixed: true,
            old: [0.50, 0.40, 0.30, 0.40],
            ..pst()
        },
    );
    let (o, k) = ("osc.fm6", knob);
    let index = routed(o, "index", 0.5, 0.4, 0.5);
    r.e.set_param(v, "index", index);
    // Reed: a plain electric piano to a buzzing, reedy one: the detuned partner and the reed's own partials.
    r.span(k, 1, v, o, "level5", 0.1, 0.5);
    r.span(k, 1, v, o, "fine5", 0.0, 7.0);
    r.span(k, 1, v, o, "level4", 0.0, 0.35);
    // Sustain: a short, dry tine to a long, held note.
    r.span(k, 2, v, o, "decay1", 200.0, 1200.0);
    r.span(k, 2, v, o, "decay5", 200.0, 1200.0);
    r.level(k, 2, s.trim, -3.0);
    // Bark: a clean tone to an overdriven one.
    let d = routed("drive", "drive_db", 9.0, 0.4, 0.4);
    r.span(k, 3, s.drive.unwrap(), "drive", "drive_db", 0.0, d);
    r.span(k, 3, s.drive.unwrap(), "drive", "trim_db", 3.5, -3.0);
    // Chorus: a dry tone to a deep, fast chorus.
    let ch = s.chorus.unwrap();
    let mix = routed("chorus", "mix", 0.0, 0.5, 0.4);
    r.span(k, 4, ch, "chorus", "mix", 0.0, mix);
    r.span_hi(k, 4, ch, "chorus", "depth", 35.0, 100.0);
    r.span_hi(k, 4, ch, "chorus", "rate_hz", 0.9, 5.0);
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
    // Pump: the kick ducks the chord; Swirl: the phaser; Grit: the drive; Spread: the saw stack.
    let knob = macros(
        &mut r,
        &["Pump", "Swirl", "Grit", "Spread"],
        &[0.45, 0.60, 0.60, 0.35],
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
            fixed: true,
            old: [0.50, 0.50, 0.25, 0.65],
            ..pst()
        },
    );
    let k = knob;
    r.route(v.fenv, "out", s.lad, "cutoff_hz", 0.3);
    r.route(m, "velocity", s.lad, "cutoff_hz", 0.1);
    // Pump: the chord ducks harder on every beat and takes longer to come back.
    r.depth_route(duck, "out", s.trim, "gain_db", k, "m1", -0.1625 / 0.45);
    r.span(k, 1, duck, "env.adsr", "decay_ms", 140.0, 230.0);
    // Swirl: a thin, slow phaser to a deep, fast, ringing one.
    let ph = s.phaser.unwrap();
    r.span(k, 2, ph, "phaser", "mix", 0.0, 40.0);
    r.span(k, 2, ph, "phaser", "rate_hz", 0.1, 0.4);
    r.span(k, 2, ph, "phaser", "feedback", 5.0, 50.0);
    r.span_hi(k, 2, ph, "phaser", "depth", 80.0, 98.0);
    r.level(k, 2, s.trim, 0.0);
    // Grit: a clean saw chord to a crushed one.
    let d = routed("drive", "drive_db", 14.0, 0.35, 0.5);
    r.span(k, 3, s.drive.unwrap(), "drive", "drive_db", 2.0, d);
    r.span(k, 3, s.drive.unwrap(), "drive", "trim_db", 2.5, -5.0);
    // Spread: a tight chord to a wide one, the triangle drifting off the saws.
    r.span(k, 4, v.osc_a, "osc.va", "detune", 2.0, 14.0);
    r.span(k, 4, v.osc_b, "osc.va", "fine", -30.0, -6.0);
    perform(&mut r, knob, Some(clock), 4);
    r
}

/// Hand Bell: an inharmonic FM bell that rings for seconds.
fn hand_bell() -> Rack {
    let mut r = Rack::new();
    // Strike: the clapper; Ring: the ring-out; Space: the hall; Clang: the partials pull out of tune.
    let knob = macros(
        &mut r,
        &["Strike", "Ring", "Shimmer", "Clang"],
        &[0.80, 0.50, 0.25, 0.50],
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
    let s = post(
        &mut r,
        knob,
        v,
        &Post {
            drive: Some((0.0, 0.0, 0.3)),
            cutoff: 7000.0,
            cut_m1: 0.3,
            trim_db: 9.0,
            chorus: Some((&[("rate_hz", 0.3), ("depth", 45.0), ("mix", 0.0)], None)),
            echo: &[
                ("time_ms", 520.0),
                ("feedback", 42.0),
                ("mix", 0.0),
                ("tone_hz", 5000.0),
            ],
            echo_max: 0.3,
            room_decay: 5.0,
            room_max: 0.4,
            fixed: true,
            old: [0.50, 0.15, 0.55, 0.40],
            ..pst()
        },
    );
    let (o, k) = ("osc.fm6", knob);
    // Strike: a muted, dull bell to a hard clapper with bright, ringing metal.
    let index = routed(o, "index", 0.45, 0.5, 0.5);
    r.span(k, 1, v, o, "index", 0.2, index);
    // Ring: a damped bell to one that rings on after it is struck.
    r.span(k, 2, v, o, "release1", 60.0, 600.0);
    r.span(k, 2, v, o, "release3", 60.0, 500.0);
    r.span(k, 2, v, o, "release5", 60.0, 400.0);
    r.level(k, 2, s.trim, -2.5);
    // Shimmer: a dry bell to a deep, fast chorus.
    let ch = s.chorus.unwrap();
    let mix = routed("chorus", "mix", 0.0, 0.5, 0.4);
    r.span(k, 3, ch, "chorus", "mix", 0.0, mix);
    r.span_hi(k, 3, ch, "chorus", "depth", 45.0, 100.0);
    r.span_hi(k, 3, ch, "chorus", "rate_hz", 0.3, 3.0);
    // Clang: a tuned bell to a clangorous one, the partials pulling apart.
    r.span(k, 4, v, o, "fine2", 0.0, 267.0);
    r.span(k, 4, v, o, "fine4", -150.0, 14.0);
    r.span(k, 4, v, o, "fine6", 150.0, -19.0);
    perform(&mut r, knob, None, 4);
    r
}

/// Corduroy: a funk clavinet, a pulse with a snapping filter.
fn corduroy() -> Rack {
    let mut r = Rack::new();
    // Quack: the wah of the filter snap; Pickup: triangle against saw; Damp: the string damper; Phase.
    let knob = macros(
        &mut r,
        &["Quack", "Clack", "Damp", "Phase"],
        &[1.0, 0.0, 0.50, 0.30],
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
                None,
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
            fixed: true,
            old: [0.50, 0.30, 0.25, 0.30],
            ..pst()
        },
    );
    let k = knob;
    r.route(m, "velocity", s.lad, "cutoff_hz", 0.25);
    // Quack: a closed, dull clav whose filter snaps open on every note, to a wide open one.
    let cut = routed("filter.ladder", "cutoff_hz", 1500.0, 0.5, 0.50);
    r.span(k, 1, s.lad, "filter.ladder", "cutoff_hz", 500.0, cut);
    r.depth_route(v.fenv, "out", s.lad, "cutoff_hz", k, "m1", 0.45);
    r.span(k, 1, v.fenv, "env.adsr", "decay_ms", 30.0, 120.0);
    // Clack: each pluck starts a few tens of cents sharp and falls into tune, the clav's clack (off).
    r.depth_route(v.fenv, "out", v.osc_a, "fine", k, "m2", 0.6);
    r.depth_route(v.fenv, "out", v.osc_b, "fine", k, "m2", 0.6);
    // Damp: a choked string to one that rings out.
    r.span(k, 3, v.aenv, "env.adsr", "decay_ms", 20.0, 170.0);
    r.span(k, 3, v.aenv, "env.adsr", "release_ms", 6.0, 50.0);
    r.level(k, 3, s.trim, -9.0);
    // Phase: a dry clav to a deep, fast phaser.
    let ph = s.phaser.unwrap();
    let mix = routed("phaser", "mix", 0.0, 0.55, 0.3);
    r.span(k, 4, ph, "phaser", "mix", 0.0, mix);
    r.span_hi(k, 4, ph, "phaser", "rate_hz", 0.6, 5.0);
    r.span_hi(k, 4, ph, "phaser", "depth", 70.0, 98.0);
    r.span_hi(k, 4, ph, "phaser", "feedback", 35.0, 88.0);
    perform(&mut r, knob, None, 4);
    r
}

/// Matinee: a wide detuned-saw keyboard with a soft filter and a big room.
fn matinee() -> Rack {
    let mut r = Rack::new();
    // Swell: the chord pushes in slowly (off); Glass: a dark pad to a bright glassy one; Square: the saws give
    // way to a hollow square; Hall: a small room and one echo to a huge hall.
    let knob = macros(
        &mut r,
        &["Swell", "Glass", "Square", "Hall"],
        &[0.0, 1.0, 0.50, 0.60],
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
            fixed: true,
            old: [0.55, 0.25, 0.60, 0.50],
            ..pst()
        },
    );
    let k = knob;
    r.route(v.fenv, "out", s.lad, "cutoff_hz", 0.25);
    r.route(m, "velocity", s.lad, "cutoff_hz", 0.15);
    // Swell: each chord struck at once to one that pushes in over a second and more (level-type).
    r.rise(k, 1, v.aenv, "env.adsr", "attack_ms", 8.0, 1400.0);
    r.rise(k, 1, v.fenv, "env.adsr", "attack_ms", 8.0, 1400.0);
    // Glass: a dark, closed pad to a bright, glassy one.
    let cut = routed("filter.ladder", "cutoff_hz", 2800.0, 0.5, 0.55);
    r.span(k, 2, s.lad, "filter.ladder", "cutoff_hz", 450.0, cut);
    // Square: a bright saw stack to a hollow, woody square pad, the saws backing off (the old Width was the detune).
    let det = routed("osc.va", "detune", 12.0, 0.5, 0.50);
    r.e.set_param(v.osc_a, "detune", det);
    r.span(k, 3, v.mix, "mixer", "level2", 0.0, 0.3);
    r.span(k, 3, v.mix, "mixer", "level1", 0.75, 0.5);
    r.level(k, 3, s.trim, -3.0);
    // Hall: a small room and a short echo to a huge, ringing hall (the wet level stays).
    r.span_hi(k, 4, s.verb, "reverb", "decay_s", 3.5, 14.0);
    r.span(k, 4, s.delay, "delay", "feedback", 8.0, 38.0);
    perform(&mut r, knob, None, 4);
    r
}

/// Snowmelt: a soft, slow electric piano in a big room.
fn snowmelt() -> Rack {
    let mut r = Rack::new();
    // Tine: the hammer's glass; Bloom: the chord grows in slowly (off); Drift: the partner tine and the chorus
    // drift apart; Hall: the six-second room grows huge.
    let knob = macros(
        &mut r,
        &["Tine", "Bloom", "Drift", "Hall"],
        &[0.40, 0.0, 0.50, 0.65],
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
    let s = post(
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
            fixed: true,
            old: [0.40, 0.20, 0.65, 0.50],
            ..pst()
        },
    );
    let (o, k) = ("osc.fm6", knob);
    // The old Drift detuned the whole voice by a fixed amount at its stored position: stays as it was.
    let fine = routed(o, "fine", 0.0, 0.2, 0.50);
    r.e.set_param(v, "fine", fine);
    // Tine: a soft, round key to a glassy one, the hammer's tine ringing out.
    let index = routed(o, "index", 0.3, 0.6, 0.40);
    r.span(k, 1, v, o, "index", 0.1, index);
    r.span(k, 1, v, o, "level4", 0.0, 0.15);
    // Bloom: every key struck at once to a chord that grows in over a second (the hammer thud stays; level-type).
    for n in [1, 2, 3, 5, 6] {
        r.rise(k, 2, v, o, &format!("attack{n}"), 12.0, 1500.0);
    }
    // Drift: a still tone to a beating partner tine under a slow, deep chorus.
    let ch = s.chorus.unwrap();
    let mix = routed("chorus", "mix", 0.0, 0.6, 0.50);
    r.span(k, 3, ch, "chorus", "mix", 0.0, mix);
    r.span_hi(k, 3, ch, "chorus", "depth", 55.0, 100.0);
    r.span_hi(k, 3, ch, "chorus", "rate_hz", 0.25, 0.9);
    r.span_hi(k, 3, v, o, "fine5", 3.0, 28.0);
    // Hall: a small room to a huge hall that rings for ten seconds (the wet level stays).
    r.span_hi(k, 4, s.verb, "reverb", "decay_s", 6.0, 20.0);
    r.span(k, 4, s.delay, "delay", "feedback", 8.0, 45.0);
    r.level(k, 4, s.trim, -2.5);
    perform(&mut r, knob, None, 4);
    r
}

/// Scaffold: a dark, wide stab for drum and bass.
fn scaffold() -> Rack {
    let mut r = Rack::new();
    // Shut: the filter closes after each stab; Stack: the two saw stacks pull apart; Swirl: the phaser;
    // Tail: the stab from a tick to a held chord.
    let knob = macros(
        &mut r,
        &["Shut", "Stack", "Swirl", "Tail"],
        &[0.78, 0.50, 0.50, 0.60],
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
            fixed: true,
            old: [0.45, 0.55, 0.25, 0.35],
            ..pst()
        },
    );
    let k = knob;
    r.route(v.fenv, "out", s.lad, "cutoff_hz", 0.4);
    r.route(m, "velocity", s.lad, "cutoff_hz", 0.15);
    // Shut: a shut, boxed-in stab to one whose filter snaps open and closes after every hit.
    let cut = routed("filter.ladder", "cutoff_hz", 1100.0, 0.55, 0.45);
    r.span(k, 1, s.lad, "filter.ladder", "cutoff_hz", 200.0, cut);
    r.span(k, 1, v.fenv, "env.adsr", "decay_ms", 50.0, 260.0);
    // Stack: a tight pair of stacks to a vast, ripping wall.
    r.span(k, 2, v.osc_a, "osc.va", "detune", 3.0, 26.0);
    r.span(k, 2, v.osc_b, "osc.va", "detune", 2.0, 18.0);
    r.span(k, 2, v.osc_b, "osc.va", "fine", -2.0, -12.0);
    // Swirl: a still, thin stab to a deep, fast, ringing phaser.
    let ph = s.phaser.unwrap();
    r.span(k, 3, ph, "phaser", "mix", 0.0, 35.0);
    r.span(k, 3, ph, "phaser", "rate_hz", 0.1, 0.2);
    r.span_hi(k, 3, ph, "phaser", "depth", 75.0, 98.0);
    r.span_hi(k, 3, ph, "phaser", "feedback", 45.0, 88.0);
    // Tail: a clipped tick to a stab that rings on after the key is up.
    let dec = routed("env.adsr", "decay_ms", 230.0, 0.5, 0.35);
    r.e.set_param(v.aenv, "decay_ms", dec);
    r.span(k, 4, v.aenv, "env.adsr", "release_ms", 12.0, 110.0);
    r.span(k, 4, v.fenv, "env.adsr", "release_ms", 12.0, 110.0);
    r.level(k, 4, s.trim, -3.0);
    perform(&mut r, knob, None, 4);
    r
}

/// Brass Filament: a sustained, metallic, slightly unstable FM tone.
fn brass_filament() -> Rack {
    let mut r = Rack::new();
    // Blow: struck at once to bowed in slowly (off); Alloy: the partials pulled out of tune into a clangorous
    // metal; Space: the only macro that owns the echo and the room; Warp: the feedback.
    let knob = macros(
        &mut r,
        &["Blow", "Alloy", "Space", "Warp"],
        &[0.0, 0.50, 0.50, 0.50],
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
    let s = post(
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
            fixed: true,
            old: [0.45, 0.25, 0.45, 0.30],
            ..pst()
        },
    );
    let (o, k) = ("osc.fm6", knob);
    let index = routed(o, "index", 0.4, 0.5, 0.45);
    r.e.set_param(v, "index", index);
    // Blow: a hard strike to a tone bowed in over most of a second (off by default; level-type).
    for n in 1..=6 {
        r.rise(k, 1, v, o, &format!("attack{n}"), 3.0, 450.0);
    }
    // Alloy: a tuned tone to a clangorous metal, the inharmonic partials pulled further out of tune.
    r.span(k, 2, v, o, "fine2", 0.0, 140.0);
    r.span(k, 2, v, o, "fine4", 0.0, -90.0);
    r.span(k, 2, v, o, "fine5", 0.0, 30.0);
    // Space: a dry, close tone to a wide echo and a huge, ringing room (the one macro that owns the space).
    let e = routed("delay", "mix", 0.0, 0.25, 0.45);
    let vm = routed("reverb", "mix", 0.0, 0.35, 0.45);
    r.span(k, 3, s.delay, "delay", "mix", 0.0, e);
    r.span(k, 3, s.verb, "reverb", "mix", 0.0, vm);
    r.span_hi(k, 3, s.verb, "reverb", "decay_s", 3.0, 14.0);
    r.span(k, 3, s.delay, "delay", "feedback", 8.0, 40.0);
    r.level(k, 3, s.trim, -2.5);
    // Warp: the sound fed back into itself: smooth to a rough, harsh, tearing metal.
    let fb = routed(o, "feedback", 0.3, 0.7, 0.30);
    r.span_hi(k, 4, v, o, "feedback", fb, 1.0);
    let cut = routed("filter.ladder", "cutoff_hz", 5500.0, 0.4, 0.45);
    r.span(k, 4, s.lad, "filter.ladder", "cutoff_hz", 20000.0, cut);
    perform(&mut r, knob, None, 4);
    r
}

/// Spinet: a bright harpsichord-like key with a buzzy pluck and no dynamics to speak of.
fn spinet() -> Rack {
    let mut r = Rack::new();
    // Jack: the filter snap of each pluck; Honky: the second string drifts out of tune (honky-tonk); Sustain: a
    // damped tick to a ring-out; Double: the strings doubled by a chorus.
    let knob = macros(
        &mut r,
        &["Jack", "Honky", "Sustain", "Double"],
        &[1.0, 0.15, 0.50, 0.35],
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
            fixed: true,
            old: [0.50, 0.30, 0.35, 0.35],
            ..pst()
        },
    );
    let k = knob;
    r.route(v.fenv, "out", s.lad, "cutoff_hz", 0.3);
    // Jack: a dull, boxed key to a bright pluck whose filter snaps open and shut on every note.
    let cut = routed("filter.ladder", "cutoff_hz", 4200.0, 0.45, 0.50);
    r.span(k, 1, s.lad, "filter.ladder", "cutoff_hz", 700.0, cut);
    r.span(k, 1, v.fenv, "env.adsr", "decay_ms", 40.0, 300.0);
    // Honky: two strings in tune to a second one pulled out of tune, a honky-tonk beating (the drive stays as it was).
    let dr = routed("drive", "drive_db", 6.0, 0.35, 0.30);
    r.e.set_param(s.drive.unwrap(), "drive_db", dr);
    r.span(k, 2, v.osc_b, "osc.va", "fine", 2.0, 9.0);
    // Sustain: a damped tick to a string that rings out after the key is up.
    r.span(k, 3, v.aenv, "env.adsr", "release_ms", 6.0, 60.0);
    r.level(k, 3, s.trim, -3.0);
    // Double: one string to two, a deep chorus doubling it.
    let ch = s.chorus.unwrap();
    let mix = routed("chorus", "mix", 0.0, 0.5, 0.35);
    r.span(k, 4, ch, "chorus", "mix", 0.0, mix);
    r.span(k, 4, ch, "chorus", "depth", 10.0, 30.0);
    r.span_hi(k, 4, ch, "chorus", "rate_hz", 0.7, 2.5);
    perform(&mut r, knob, None, 4);
    r
}

// ---- plucks -----------------------------------------------------------------------------------

/// Willow: a soft harp.
fn willow() -> Rack {
    let mut r = Rack::new();
    // Nail: a soft finger to a bright nail; Ring: how long the strings sing; Halo: glassy overtones above;
    // Hall: the room.
    let knob = macros(
        &mut r,
        &["Nail", "Ring", "Halo", "Hall"],
        &[0.50, 1.0, 0.35, 0.60],
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
    let s = post(
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
            fixed: true,
            old: [0.50, 0.15, 0.60, 0.45],
            ..pst()
        },
    );
    let (o, k) = ("osc.fm6", knob);
    // Nail: a soft fingertip to a bright, clicking nail.
    let index = routed(o, "index", 0.4, 0.5, 0.50);
    r.span(k, 1, v, o, "index", 0.1, index);
    r.span(k, 1, v, o, "level2", 0.0, 0.3);
    r.span(k, 1, v, o, "level4", 0.0, 0.2);
    // Ring: a damped, quickly dying string to one that sings for many seconds.
    let d1 = routed(o, "decay1", 1800.0, 0.5, 0.45);
    let d3 = routed(o, "decay3", 1100.0, 0.5, 0.45);
    r.span(k, 2, v, o, "decay1", 250.0, d1);
    r.span(k, 2, v, o, "decay3", 200.0, d3);
    r.span(k, 2, v, o, "release1", 60.0, 500.0);
    r.level(k, 2, s.trim, -6.0);
    // Halo: a plain string to one with a shimmering halo, the octave partial drifting out of tune.
    r.span_hi(k, 3, v, o, "fine3", 4.0, 40.0);
    // Hall: a small room to a huge hall with a long echo (the wet level stays).
    r.span_hi(k, 4, s.verb, "reverb", "decay_s", 4.5, 16.0);
    r.span(k, 4, s.delay, "delay", "feedback", 8.0, 40.0);
    perform(&mut r, knob, None, 4);
    r
}

/// Dry Creek: a nylon-string guitar pluck.
fn dry_creek() -> Rack {
    let mut r = Rack::new();
    // Pick: the click of the pick; Nylon: the bright stored string to a dark, round nylon one (off); Ring: let the string ring;
    // Double: a second string beside it and a chorus.
    let knob = macros(
        &mut r,
        &["Pick", "Nylon", "Ring", "Double"],
        &[0.50, 0.0, 0.50, 0.30],
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
    let s = post(
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
            fixed: true,
            old: [0.50, 0.20, 0.40, 0.30],
            ..pst()
        },
    );
    let (o, k) = ("osc.fm6", knob);
    // Pick: a soft fingertip to a hard, clicking pick.
    let index = routed(o, "index", 0.5, 0.5, 0.50);
    r.e.set_param(v, "index", index);
    r.span(k, 1, v, o, "level4", 0.0, 0.3);
    r.span(k, 1, v, o, "level6", 0.0, 0.2);
    // Nylon: the bright, edgy stored string to a dark, round nylon one (off by default).
    let cut = routed("filter.ladder", "cutoff_hz", 4500.0, 0.45, 0.50);
    r.rise(k, 2, s.lad, "filter.ladder", "cutoff_hz", cut, 700.0);
    let dr = routed("drive", "drive_db", 3.0, 0.35, 0.20);
    r.rise(k, 2, s.drive.unwrap(), "drive", "drive_db", dr, 0.0);
    r.level(k, 2, s.trim, 3.0);
    // Ring: a muted, palm-damped note to a string let ring.
    r.span(k, 3, v, o, "decay1", 150.0, 900.0);
    r.span(k, 3, v, o, "decay5", 150.0, 900.0);
    r.span(k, 3, v, o, "release1", 20.0, 120.0);
    r.level(k, 3, s.trim, -9.0);
    // Double: one string to a doubled, chorused pair.
    let ch = s.chorus.unwrap();
    let mix = routed("chorus", "mix", 0.0, 0.5, 0.30);
    r.span(k, 4, ch, "chorus", "mix", 0.0, mix);
    r.span_hi(k, 4, ch, "chorus", "depth", 30.0, 100.0);
    r.span_hi(k, 4, ch, "chorus", "rate_hz", 0.5, 2.5);
    r.span(k, 4, v, o, "fine5", 0.0, 3.0);
    perform(&mut r, knob, None, 4);
    r
}

/// Gumdrop: a short, bouncy pop pluck.
fn gumdrop() -> Rack {
    let mut r = Rack::new();
    // Bounce: the resonant filter squelches on each note; Hold: the note length; Boing: each note drops in from
    // above (off); Chorus: the stereo shimmer.
    let knob = macros(
        &mut r,
        &["Bounce", "Hold", "Boing", "Chorus"],
        &[0.85, 0.80, 0.0, 0.30],
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
            fixed: true,
            old: [0.55, 0.20, 0.45, 0.40],
            ..pst()
        },
    );
    let k = knob;
    r.route(v.fenv, "out", s.lad, "cutoff_hz", 0.4);
    r.route(m, "velocity", s.lad, "cutoff_hz", 0.15);
    // Bounce: a flat, dull pluck to a squelching, resonant bounce on every note.
    let cut = routed("filter.ladder", "cutoff_hz", 1500.0, 0.5, 0.55);
    r.span(k, 1, s.lad, "filter.ladder", "cutoff_hz", 300.0, cut);
    r.span(k, 1, s.lad, "filter.ladder", "resonance", 0.1, 0.35);
    // Hold: a clipped dot to a long, bouncing note.
    let dec = routed("env.adsr", "decay_ms", 260.0, 0.5, 0.40);
    r.span(k, 2, v.aenv, "env.adsr", "decay_ms", 130.0, dec);
    r.span(k, 2, v.aenv, "env.adsr", "release_ms", 80.0, 120.0);
    r.level(k, 2, s.trim, -5.0);
    // Boing: each note drops in from above, up to 120 ct, and falls into tune (off by default).
    r.depth_route(v.fenv, "out", v.osc_a, "fine", k, "m3", 0.6);
    r.depth_route(v.fenv, "out", v.osc_b, "fine", k, "m3", 0.6);
    // Chorus: a dry pop pluck to a deep, fast chorus.
    let ch = s.chorus.unwrap();
    r.span(k, 4, ch, "chorus", "mix", 0.0, 25.0);
    r.span(k, 4, ch, "chorus", "depth", 10.0, 35.0);
    r.span_hi(k, 4, ch, "chorus", "rate_hz", 0.6, 3.0);
    perform(&mut r, knob, None, 4);
    r
}

/// Cold Cut: a dark, short bell-pluck for trap.
fn cold_cut() -> Rack {
    let mut r = Rack::new();
    // Click: the metallic click of the strike; Ring: how long the bell lasts; Grit: the drive; Dark: the filter.
    let knob = macros(
        &mut r,
        &["Click", "Ring", "Grit", "Dark"],
        &[0.50, 1.0, 0.40, 0.17],
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
    let s = post(
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
            fixed: true,
            old: [0.40, 0.35, 0.30, 0.40],
            ..pst()
        },
    );
    let (o, k) = ("osc.fm6", knob);
    let index = routed(o, "index", 0.45, 0.5, 0.40);
    r.e.set_param(v, "index", index);
    // Click: a hollow, soft knock to a hard, metallic click on top of it.
    r.span(k, 1, v, o, "level2", 0.0, 0.5);
    // Ring: a dead tick to a bell that hangs in the air.
    let d1 = routed(o, "decay1", 700.0, 0.5, 0.40);
    let d3 = routed(o, "decay3", 500.0, 0.5, 0.40);
    r.span(k, 2, v, o, "decay1", 120.0, d1);
    r.span(k, 2, v, o, "decay3", 100.0, d3);
    r.span(k, 2, v, o, "release1", 30.0, 150.0);
    r.level(k, 2, s.trim, -7.0);
    // Grit: a clean, hollow bell to a crushed, blown-out one, the trim following the drive down.
    let dr = routed("drive", "drive_db", 10.0, 0.4, 0.35);
    r.span(k, 3, s.drive.unwrap(), "drive", "drive_db", 2.0, dr);
    r.span(k, 3, s.drive.unwrap(), "drive", "trim_db", 0.0, -4.0);
    // Dark: the open stored tone to a muffled, closed bell (the gain follows the filter down).
    let cut = routed("filter.ladder", "cutoff_hz", 2800.0, 0.55, 0.40);
    r.span(k, 4, s.lad, "filter.ladder", "cutoff_hz", 20000.0, cut);
    r.level(k, 4, s.trim, 8.0);
    perform(&mut r, knob, None, 4);
    r
}

/// Rivet Gun: a bright, phased, compressed staccato pluck.
fn rivet_gun() -> Rack {
    let mut r = Rack::new();
    // Gate: the length of each rivet; Phase: the phaser; Snap: the filter; Stack: the unison pulls apart.
    let knob = macros(
        &mut r,
        &["Gate", "Phase", "Snap", "Stack"],
        &[0.50, 0.45, 1.0, 0.40],
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
            fixed: true,
            old: [0.55, 0.50, 0.25, 0.45],
            ..pst()
        },
    );
    let k = knob;
    r.route(v.fenv, "out", s.lad, "cutoff_hz", 0.4);
    r.route(m, "velocity", s.lad, "cutoff_hz", 0.15);
    // Gate: a clipped tick to a rivet that holds and rings on.
    r.span(k, 1, v.aenv, "env.adsr", "decay_ms", 30.0, 120.0);
    r.span(k, 1, v.aenv, "env.adsr", "release_ms", 8.0, 40.0);
    r.level(k, 1, s.trim, -7.0);
    // Phase: a dry rivet to a deep, fast, ringing phaser.
    let ph = s.phaser.unwrap();
    let mix = routed("phaser", "mix", 0.0, 0.6, 0.45);
    r.span(k, 2, ph, "phaser", "mix", 0.0, mix);
    r.span_hi(k, 2, ph, "phaser", "rate_hz", 0.5, 4.0);
    r.span_hi(k, 2, ph, "phaser", "feedback", 55.0, 90.0);
    r.level(k, 2, s.trim, 4.0);
    // Snap: a closed, dull rivet to one whose filter snaps open on every hit (the drive stays as it was).
    let dr = routed("drive", "drive_db", 14.0, 0.35, 0.50);
    r.e.set_param(s.drive.unwrap(), "drive_db", dr);
    let cut = routed("filter.ladder", "cutoff_hz", 2200.0, 0.5, 0.55);
    r.span(k, 3, s.lad, "filter.ladder", "cutoff_hz", 450.0, cut);
    r.level(k, 3, s.trim, 0.0);
    // Stack: a tight pulse to a wide, ratty stack.
    r.span(k, 4, v.osc_a, "osc.va", "detune", 2.0, 14.0);
    r.span(k, 4, v.osc_b, "osc.va", "fine", 0.0, 7.0);
    perform(&mut r, knob, None, 4);
    r
}

/// Free Bar (was Tuning Fork): tuned metal bars, soft and slightly detuned, with a slow wobble.
fn free_bar() -> Rack {
    let mut r = Rack::new();
    // Overtones: a pure fork to a clangorous bar; Ring: how long the bar sings; Beat: the doubled partials drift
    // apart and beat; Bowed: each bar struck to one that is bowed in (off).
    let knob = macros(
        &mut r,
        &["Overtones", "Ring", "Beat", "Bowed"],
        &[0.50, 1.0, 0.10, 0.0],
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
    let s = post(
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
            fixed: true,
            old: [0.45, 0.15, 0.55, 0.40],
            ..pst()
        },
    );
    let (o, k) = ("osc.fm6", knob);
    // Overtones: a pure, soft fork to a clangorous bar, the inharmonic partials rising.
    r.span(k, 1, v, o, "level2", 0.0, 0.45);
    r.span(k, 1, v, o, "level3", 0.0, 0.3);
    r.span(k, 1, v, o, "level4", 0.0, 0.18);
    // Ring: a damped bar to a bar that sings for many seconds.
    r.span(k, 2, v, o, "decay1", 120.0, 6000.0);
    r.span(k, 2, v, o, "decay5", 120.0, 6000.0);
    r.span(k, 2, v, o, "decay2", 80.0, 2500.0);
    r.span(k, 2, v, o, "decay3", 60.0, 700.0);
    r.span(k, 2, v, o, "release1", 20.0, 400.0);
    r.span(k, 2, v, o, "release5", 20.0, 400.0);
    r.level(k, 2, s.trim, -4.0);
    // Beat: the doubled partials drift apart from a pure unison to a slow, wide beating.
    r.span(k, 3, v, o, "fine5", 0.0, 5.0);
    r.span(k, 3, v, o, "fine6", 0.0, 8.0);
    r.level(k, 3, s.trim, 1.5);
    // Bowed: a struck bar to one that is bowed in over a second (off by default; level-type).
    for n in 1..=6 {
        r.rise(k, 4, v, o, &format!("attack{n}"), 0.5, 250.0);
    }
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
            description: "A Wurlitzer-like electric piano: a reedy attack that barks harder when you play harder, and a quick fall to a soft sustain. Play funky stabs and short vamps. Reed brings in the reed buzz and the FM brightness, Sustain lengthens the note, Bark pushes the amp drive, Chorus brings in a deep, fast chorus.",
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
            description: "Hold a chord and press Run: a driven, phased saw chord that ducks on every beat (a silent envelope on the host clock at 123 BPM with swing; no drum is built in). Pump sets how hard and how long the beat ducks the chord, Swirl runs the phaser from thin and slow to deep and fast, Grit pushes the drive, Spread widens the saw stack.",
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
            description: "An inharmonic FM bell: three pairs of operators that ring for seconds and darken as they fade. Play slow, sparse notes. Strike goes from a dull, muted bell to a hard clapper with bright metal, Ring lets the bell ring on after the strike, Shimmer brings in a deep, fast chorus, Clang pulls the partials out of tune.",
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
            description: "A funk clavinet: a narrow pulse with a hard filter snap and a short sustain. Play 16th-note chops. Quack goes from a closed, dull clav to an open one as the filter snaps on every note, Clack starts each pluck a little sharp, Damp goes from a choked string to one that rings out, Phase brings in a phaser.",
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
            description: "A wide keyboard from seven detuned saws, a soft filter and a big room. Play slow chords. Swell pushes each chord in slowly instead of striking it, Glass opens a dark pad into a bright, glassy one, Square lets a hollow square take over from the saws at the same loudness, Hall grows the room from small to huge.",
            listen: "The width of each chord and the room trailing behind it; Square at 0 is the bright saw stack",
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
            description: "A slow, soft electric piano in a big room: a gentle attack, a long fade and a drifting tuning. Play slow open chords. Tine grows a glassy hammer tine out of the soft key, Bloom lets each chord grow in over a second, Drift sets a partner tine beating under a deep, slow chorus, Hall grows the six-second room into a huge one.",
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
            description: "A dark, wide stab for drum and bass: eight detuned saws, a closing filter, a phaser and a compressor. Play short rolling chords. Shut goes from a boxed-in stab to one whose filter snaps open and closes on every hit, Stack pulls the two saw stacks apart into a wall, Swirl runs the phaser from still to deep and fast, Tail lets the stab ring on after the key is up.",
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
            description: "A metallic FM tone that rings and wavers: inharmonic partials, a little feedback and a slow fade. Play sparse two-note intervals. Blow bows the tone in slowly instead of striking it, Alloy pulls the partials out of tune into clangorous metal, Space opens a wide echo and a huge, ringing room, Warp feeds the sound back into itself.",
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
            description: "A bright harpsichord-like key: a thin pulse and a saw, an instant attack and a plucked fall, with a faint buzz. Play counterpoint lines. Jack goes from a dull, boxed key to a bright pluck whose filter snaps on every note, Honky pulls the second string out of tune, Sustain lets the strings ring out after the key is up, Double brings in a deep chorus.",
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
            description: "A soft harp: a quick pluck and a long, clean ring. Play rolled chords. Nail goes from a soft fingertip to a bright, clicking nail, Ring lets the strings sing for many seconds, Halo sets a shimmering halo of overtones, Hall grows the room from small to huge.",
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
            description: "A nylon-string guitar pluck: a soft pick, a woody body and a quick decay. Play arpeggios. Pick goes from a soft fingertip to a hard, clicking pick, Nylon turns the bright string into a dark, round nylon one, Ring lets the string ring, Double brings in a second string and a chorus.",
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
            description: "A short, bouncy pop pluck: two pulses and a saw, a resonant filter that bounces on each note. Play catchy riffs. Bounce goes from a flat pluck to a squelching, resonant bounce, Hold stretches the note from a dot to a long bounce, Boing drops each note in from above, Chorus brings in a deep, fast shimmer.",
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
            description: "A dark, short bell-pluck for trap: a metallic click, a hollow body and a quick fall. Play minor figures with room to ring. Click goes from a hollow knock to a hard, metallic click, Ring lets the bell hang in the air, Grit crushes it, Dark closes the filter into a muffled bell.",
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
            description: "A bright, staccato, phased pluck, driven and squashed: a narrow pulse with a hard filter snap. Play short syncopated riffs. Gate goes from a clipped tick to a rivet that holds and rings on, Phase runs the phaser from dry to deep and fast, Snap closes the filter until it snaps open on every hit, Stack pulls the unison apart.",
            listen: "The phaser on the staccato riff and the compressor holding it close",
            group: 2,
            build: rivet_gun,
            phrase: rivet_phrase,
            secs: 5.0,
        },
        Preset {
            slug: "free-bar",
            name: "Free Bar",
            category: "Pluck",
            tags: &["poly", "fm", "metallic", "ambient", "experimental"],
            description: "Tuned metal bars: six added partials of a free bar, a soft strike and a long ring. Play slow melodies. Overtones goes from a pure fork to a clangorous bar, Ring lets the bar sing for many seconds, Beat pulls the doubled partials apart into a wide beating, Bowed pushes each bar in slowly instead of striking it.",
            listen: "The pure ring of each bar and the slow beating across notes",
            group: 2,
            build: free_bar,
            phrase: fork_phrase,
            secs: 11.0,
        },
    ]
}
