//! The keys and plucks of the factory bank. One function per preset, then the list.

use super::*;

mod group2;

const ECHO: &[(&str, f32)] = &[
    ("time_ms", 250.0),
    ("feedback", 30.0),
    ("mix", 0.0),
    ("tone_hz", 5000.0),
];

/// The common tail with neutral defaults; presets override what they need.
fn pst() -> Post<'static> {
    Post {
        drive: None,
        cutoff: 6000.0,
        resonance: 0.1,
        cut_m1: 0.4,
        trim_db: 0.0,
        phaser: None,
        chorus: None,
        comp: None,
        duck: None,
        echo: ECHO,
        echo_max: 0.2,
        clock: None,
        room_decay: 2.0,
        room_max: 0.3,
        fixed: false,
        old: [0.0; 4],
        space: None,
    }
}

/// A subtractive voice: two `osc.va` into a mixer, an amplitude envelope on a VCA, and a filter
/// envelope the caller routes into the ladder returned by `post`.
struct Va {
    out: ModuleId,
    osc_a: ModuleId,
    osc_b: ModuleId,
    mix: ModuleId,
    aenv: ModuleId,
    fenv: ModuleId,
}

#[allow(clippy::too_many_arguments)]
fn va(
    r: &mut Rack,
    m: ModuleId,
    a: &[(&str, f32)],
    b: &[(&str, f32)],
    levels: (f32, f32),
    amp: [(&'static str, f32); 4],
    filt: [(&'static str, f32); 4],
) -> Va {
    let oa = r.add("osc.va", a);
    let ob = r.add("osc.va", b);
    let mix = r.add("mixer", &[("level1", levels.0), ("level2", levels.1)]);
    let aenv = r.add("env.adsr", &amp);
    let fenv = r.add("env.adsr", &filt);
    let vca = r.add("vca", &[("gain", 0.0)]);
    r.wire(m, "pitch", oa, "pitch");
    r.wire(m, "pitch", ob, "pitch");
    r.wire(oa, "out", mix, "in1");
    r.wire(ob, "out", mix, "in2");
    r.wire(mix, "out", vca, "in");
    r.wire(m, "gate", aenv, "gate");
    r.wire(m, "gate", fenv, "gate");
    r.wire(aenv, "out", vca, "cv");
    Va {
        out: vca,
        osc_a: oa,
        osc_b: ob,
        mix,
        aenv,
        fenv,
    }
}

// ---- keys -------------------------------------------------------------------------------------

/// Velvet Tine: a soft Rhodes-like electric piano with a phaser on the Phase macro.
fn velvet_tine() -> Rack {
    let mut r = Rack::new();
    // Tine: the hammer's tine; Ring: how long it sings; Detune: the partner tine drifts off (off); Phase.
    let knob = macros(
        &mut r,
        &["Tine", "Ring", "Detune", "Phase"],
        &[0.40, 0.50, 0.0, 0.25],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let ops: [Op; 6] = [
        [1.0, 0.0, 1.0, 1.0, 2200.0, 0.12, 300.0, 0.3],
        [1.0, 0.0, 0.55, 1.0, 420.0, 0.04, 200.0, 0.85],
        [2.0, 0.0, 0.4, 1.0, 1400.0, 0.08, 260.0, 0.3],
        [3.0, 0.0, 0.3, 0.5, 110.0, 0.0, 80.0, 0.9],
        [1.0, 4.0, 0.5, 1.0, 2400.0, 0.12, 300.0, 0.3],
        [1.0, 0.0, 0.25, 1.0, 800.0, 0.05, 200.0, 0.5],
    ];
    let v = fm(&mut r, m, 1.0, 0.45, 0.0, &ops);
    let s = post(
        &mut r,
        knob,
        v,
        &Post {
            drive: Some((0.0, -1.0, 0.3)),
            cutoff: 5500.0,
            cut_m1: 0.3,
            trim_db: 11.5,
            phaser: Some((
                &[
                    ("stages", 4.0),
                    ("rate_hz", 0.35),
                    ("depth", 60.0),
                    ("feedback", 25.0),
                    ("mix", 0.0),
                ],
                None,
            )),
            echo: &[
                ("time_ms", 300.0),
                ("feedback", 28.0),
                ("mix", 0.0),
                ("tone_hz", 4500.0),
            ],
            echo_max: 0.2,
            room_decay: 2.4,
            room_max: 0.35,
            fixed: true,
            old: [0.50, 0.20, 0.45, 0.35],
            ..pst()
        },
    );
    let (o, k) = ("osc.fm6", knob);
    // Tine: the metallic tine grows out of the note (index and the 3rd-harmonic strike).
    let index = routed(o, "index", 0.45, 0.5, 0.5);
    r.span(k, 1, v, o, "index", 0.3, index);
    r.span(k, 1, v, o, "level4", 0.0, 0.3);
    // Ring: the two carriers decay from a short thud to a long singing note.
    r.span_hi(k, 2, v, o, "decay1", 2200.0, 9500.0);
    r.level(k, 2, s.trim, -2.0);
    r.span_hi(k, 2, v, o, "decay5", 2400.0, 9500.0);
    // Detune: the detuned partner of the first carrier drifts off, a plain tine to a beating, chorused one (off).
    r.rise(k, 3, v, o, "fine5", 4.0, 90.0);
    r.level(k, 3, s.trim, 0.8);
    // Phase: a still, thin phaser to a slow, deep swirl.
    let ph = s.phaser.unwrap();
    let mix = routed("phaser", "mix", 0.0, 0.5, 0.35);
    r.span(k, 4, ph, "phaser", "mix", 0.0, mix);
    r.span_hi(k, 4, ph, "phaser", "rate_hz", 0.35, 3.0);
    r.span_hi(k, 4, ph, "phaser", "depth", 60.0, 98.0);
    r.span_hi(k, 4, ph, "phaser", "feedback", 25.0, 88.0);
    perform(&mut r, knob, None, 4);
    r
}

/// Checkerboard: a house piano stab, driven and compressed until it barks.
fn checkerboard() -> Rack {
    let mut r = Rack::new();
    // Bite: the filter snap of each stab; Hollow: the pulse under it; Squash: the compressor; Spread: the unison.
    let knob = macros(
        &mut r,
        &["Bite", "Hollow", "Squash", "Spread"],
        &[1.0, 0.35, 0.70, 0.0],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let v = va(
        &mut r,
        m,
        &[("waveform", 2.0), ("unison", 3.0), ("detune", 12.0)],
        &[("waveform", 1.0), ("fine", 5.0)],
        (0.55, 0.35),
        adsr(1.0, 260.0, 0.0, 90.0),
        adsr(1.0, 180.0, 0.1, 90.0),
    );
    let s = post(
        &mut r,
        knob,
        v.out,
        &Post {
            drive: Some((15.0, -6.0, 0.35)),
            cutoff: 1500.0,
            resonance: 0.25,
            cut_m1: 0.5,
            trim_db: 9.0,
            comp: Some(&[
                ("threshold_db", -22.0),
                ("ratio", 6.0),
                ("attack_ms", 4.0),
                ("release_ms", 90.0),
                ("makeup_db", 5.0),
            ]),
            echo: &[
                ("time_ms", 161.0),
                ("feedback", 25.0),
                ("mix", 0.0),
                ("tone_hz", 4000.0),
            ],
            echo_max: 0.25,
            room_decay: 0.6,
            room_max: 0.15,
            fixed: true,
            old: [0.50, 0.55, 0.25, 0.40],
            ..pst()
        },
    );
    let k = knob;
    // The old Length route (4.6 s on a stab a quarter second long) did nothing: the decay stays at its stored length.
    let dec = routed("env.adsr", "decay_ms", 260.0, 0.5, 0.4);
    r.e.set_param(v.aenv, "decay_ms", dec);
    // Bite: a closed, thudding stab whose filter snaps open on every note, to a wide open bark.
    let cut = routed("filter.ladder", "cutoff_hz", 1500.0, 0.5, 0.50);
    r.span(k, 1, s.lad, "filter.ladder", "cutoff_hz", 400.0, cut);
    r.depth_route(v.fenv, "out", s.lad, "cutoff_hz", k, "m1", 0.35);
    r.span(k, 1, v.fenv, "env.adsr", "decay_ms", 40.0, 180.0);
    r.route(m, "velocity", s.lad, "cutoff_hz", 0.15);
    // Hollow: a timbre change at constant loudness: the triangle (odd harmonics, no top) takes over from the saws, the gain following.
    let mix = v.mix;
    r.span(k, 2, mix, "mixer", "level2", 0.0, 0.35);
    r.span(k, 2, mix, "mixer", "level1", 0.6577, 0.55);
    r.level(k, 2, s.trim, -1.0);
    // Squash: the compressor from a light touch to a hard clamp, the makeup following.
    let c = s.comp.unwrap();
    r.span(k, 3, c, "compressor", "threshold_db", -8.0, -22.0);
    r.span(k, 3, c, "compressor", "ratio", 2.0, 6.0);
    r.span(k, 3, c, "compressor", "makeup_db", 2.0, 5.0);
    r.level(k, 3, s.trim, 4.0);
    // Spread: the saw stack widens and drifts apart (off by default).
    r.rise(k, 4, v.osc_a, "osc.va", "detune", 12.0, 65.0);
    r.rise(k, 4, v.osc_a, "osc.va", "width", 0.0, 100.0);
    r.rise(k, 4, v.osc_b, "osc.va", "fine", 5.0, 30.0);
    perform(&mut r, knob, None, 4);
    r
}

/// Linoleum: a phased disco electric piano, driven and compressed.
fn linoleum() -> Rack {
    let mut r = Rack::new();
    // Swirl: the phaser; Chime: the bell partial; Hold: how long it rings; Squeeze: the compressor.
    let knob = macros(
        &mut r,
        &["Swirl", "Chime", "Hold", "Squeeze"],
        &[0.80, 0.60, 0.60, 0.90],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let ops: [Op; 6] = [
        [1.0, 0.0, 1.0, 0.5, 900.0, 0.2, 140.0, 0.3],
        [2.0, 0.0, 0.6, 0.5, 260.0, 0.05, 100.0, 0.9],
        [1.0, 6.0, 0.7, 0.5, 900.0, 0.2, 140.0, 0.3],
        [7.0, 0.0, 0.4, 0.5, 70.0, 0.0, 50.0, 0.9],
        [1.0, -6.0, 0.5, 0.5, 900.0, 0.2, 140.0, 0.3],
        [1.0, 0.0, 0.3, 0.5, 400.0, 0.05, 100.0, 0.5],
    ];
    let v = fm(&mut r, m, 1.0, 0.5, 0.0, &ops);
    let s = post(
        &mut r,
        knob,
        v,
        &Post {
            drive: Some((12.0, -4.0, 0.35)),
            cutoff: 4200.0,
            resonance: 0.2,
            cut_m1: 0.4,
            trim_db: 10.0,
            phaser: Some((
                &[
                    ("stages", 6.0),
                    ("rate_hz", 0.3),
                    ("depth", 85.0),
                    ("feedback", 60.0),
                    ("mix", 55.0),
                ],
                None,
            )),
            comp: Some(&[
                ("threshold_db", -24.0),
                ("ratio", 5.0),
                ("attack_ms", 6.0),
                ("release_ms", 110.0),
                ("makeup_db", 5.0),
            ]),
            echo: &[
                ("time_ms", 244.0),
                ("feedback", 30.0),
                ("mix", 0.0),
                ("tone_hz", 4500.0),
            ],
            echo_max: 0.2,
            room_decay: 0.7,
            room_max: 0.15,
            fixed: true,
            old: [0.50, 0.45, 0.25, 0.40],
            ..pst()
        },
    );
    let (o, k) = ("osc.fm6", knob);
    // Swirl: a thin, slow phaser to a fast, deep, ringing one.
    let ph = s.phaser.unwrap();
    let rate = routed("phaser", "rate_hz", 0.3, 0.5, 0.4);
    r.span(k, 1, ph, "phaser", "rate_hz", 0.1, rate);
    r.span(k, 1, ph, "phaser", "mix", 5.0, 55.0);
    r.span(k, 1, ph, "phaser", "feedback", 0.0, 60.0);
    r.span(k, 1, ph, "phaser", "depth", 57.0, 85.0);
    // Chime: the seventh-partial bell grows out of the electric piano.
    let index = routed(o, "index", 0.5, 0.4, 0.5);
    r.span(k, 2, v, o, "index", 0.3, index);
    r.span(k, 2, v, o, "level4", 0.0, 0.4);
    // Hold: a damped tick to a long ring.
    for op in ["decay1", "decay3", "decay5"] {
        r.span(k, 3, v, o, op, 220.0, 900.0);
    }
    // Squeeze: a light touch to a hard clamp.
    let c = s.comp.unwrap();
    r.span(k, 4, c, "compressor", "threshold_db", -10.0, -24.0);
    r.span(k, 4, c, "compressor", "ratio", 2.0, 5.0);
    r.span(k, 4, c, "compressor", "makeup_db", 2.0, 5.0);
    r.level(k, 4, s.trim, 4.0);
    perform(&mut r, knob, None, 4);
    r
}

/// Cellar Door: a dark, slow-wobbling electric piano for hip-hop.
fn cellar_door() -> Rack {
    let mut r = Rack::new();
    // Dark: the lid on the sound; Felt: the hammer's softness; Wobble: tape pitch drift (off); Smear: the chorus.
    let knob = macros(
        &mut r,
        &["Dark", "Felt", "Wobble", "Smear"],
        &[0.75, 0.0, 0.0, 0.30],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let ops: [Op; 6] = [
        [1.0, 0.0, 1.0, 2.0, 1800.0, 0.15, 280.0, 0.25],
        [1.0, 0.0, 0.5, 2.0, 380.0, 0.03, 180.0, 0.7],
        [2.0, 0.0, 0.3, 2.0, 900.0, 0.06, 200.0, 0.25],
        [1.0, 0.0, 0.2, 1.0, 100.0, 0.0, 60.0, 0.8],
        [1.0, 5.0, 0.5, 2.0, 1800.0, 0.15, 280.0, 0.25],
        [1.0, 0.0, 0.2, 2.0, 600.0, 0.05, 180.0, 0.4],
    ];
    let v = fm(&mut r, m, 1.0, 0.35, 0.0, &ops);
    let s = post(
        &mut r,
        knob,
        v,
        &Post {
            drive: Some((6.0, -2.0, 0.4)),
            cutoff: 1800.0,
            resonance: 0.12,
            cut_m1: 0.6,
            trim_db: 8.5,
            chorus: Some((&[("rate_hz", 0.35), ("depth", 70.0), ("mix", 0.0)], None)),
            echo: &[
                ("time_ms", 340.0),
                ("feedback", 32.0),
                ("mix", 0.0),
                ("tone_hz", 3000.0),
            ],
            echo_max: 0.2,
            room_decay: 1.4,
            room_max: 0.3,
            fixed: true,
            old: [0.35, 0.30, 0.30, 0.40],
            ..pst()
        },
    );
    let (o, k) = ("osc.fm6", knob);
    // Dark: the filter and the FM index close together, a muffled felt tone to an open one.
    let cut = routed("filter.ladder", "cutoff_hz", 1800.0, 0.6, 0.35);
    r.span(k, 1, s.lad, "filter.ladder", "cutoff_hz", 450.0, cut);
    let index = routed(o, "index", 0.35, 0.5, 0.35);
    r.span(k, 1, v, o, "index", 0.12, index);
    // Felt: a hammer that hits at once to one that pushes the note in slowly (off by default).
    for op in ["attack1", "attack2", "attack3", "attack5", "attack6"] {
        r.rise(k, 2, v, o, op, 2.0, 250.0);
    }
    // Wobble: a tape pitch drift that fades in and speeds up (off by default).
    let lfo = r.add("lfo", &[("rate_hz", 0.4)]);
    r.depth_route(lfo, "out", v, "fine", k, "m3", 0.35);
    r.rise(k, 3, lfo, "lfo", "rate_hz", 0.4, 3.0);
    // Smear: a dry key to a deep, slow chorus.
    let ch = s.chorus.unwrap();
    let mix = routed("chorus", "mix", 0.0, 0.6, 0.4);
    r.span(k, 4, ch, "chorus", "mix", 0.0, mix);
    r.span_hi(k, 4, ch, "chorus", "depth", 70.0, 98.0);
    r.span_hi(k, 4, ch, "chorus", "rate_hz", 0.35, 2.5);
    perform(&mut r, knob, None, 4);
    r
}

/// Pulpit: a drawbar-style organ (six added sines) through a rotor.
fn pulpit() -> Rack {
    let mut r = Rack::new();
    // Bars: the drawbars; Perc: the percussion click (off); Rotor: the Leslie; Overdrive: the tubes.
    let knob = macros(
        &mut r,
        &["Bars", "Perc", "Rotor", "Overdrive"],
        &[0.55, 0.0, 0.75, 0.30],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    // Sub, fundamental, 2nd, 3rd, 4th and 6th harmonics: ratio, fine, level, then a flat envelope.
    let bars = [
        (0.0, 0.6),
        (1.0, 1.0),
        (2.0, 0.8),
        (3.0, 0.5),
        (4.0, 0.4),
        (6.0, 0.25),
    ];
    let ops: [Op; 6] = bars.map(|(ratio, level)| [ratio, 0.0, level, 3.0, 50.0, 1.0, 40.0, 0.0]);
    let v = fm(&mut r, m, 7.0, 1.0, 0.0, &ops);
    let s = post(
        &mut r,
        knob,
        v,
        &Post {
            drive: Some((4.0, -2.0, 0.55)),
            cutoff: 3500.0,
            resonance: 0.05,
            cut_m1: 0.6,
            trim_db: 1.0,
            chorus: Some((&[("rate_hz", 5.5), ("depth", 22.0), ("mix", 0.0)], None)),
            echo: &[
                ("time_ms", 300.0),
                ("feedback", 25.0),
                ("mix", 0.0),
                ("tone_hz", 4500.0),
            ],
            echo_max: 0.1,
            room_decay: 1.6,
            room_max: 0.3,
            fixed: true,
            old: [0.55, 0.20, 0.35, 0.50],
            ..pst()
        },
    );
    let (o, k) = ("osc.fm6", knob);
    // Bars: the sub and the 2nd harmonic bar come out, a thin flute-like organ to a full, deep one.
    r.span(k, 1, v, o, "level1", 0.12, 0.6);
    r.level(k, 1, s.trim, -2.0);
    r.span_hi(k, 1, v, o, "level3", 0.8, 1.0);
    // Perc: the third harmonic strikes and fades (off by default: it holds at full level).
    for op in ["4", "5", "6"] {
        r.rise(k, 2, v, o, &format!("sustain{op}"), 1.0, 0.0);
        r.rise(k, 2, v, o, &format!("decay{op}"), 50.0, 150.0);
        r.rise(
            k,
            2,
            v,
            o,
            &format!("level{op}"),
            if op == "4" {
                0.5
            } else if op == "5" {
                0.4
            } else {
                0.25
            },
            0.9,
        );
    }
    // Rotor: the Leslie from slow and thin to fast and swirling.
    let ch = s.chorus.unwrap();
    let mix = routed("chorus", "mix", 0.0, 0.55, 0.5);
    r.span(k, 3, ch, "chorus", "rate_hz", 2.0, 5.5);
    r.span(k, 3, ch, "chorus", "depth", 6.0, 22.0);
    r.span(k, 3, ch, "chorus", "mix", 4.0, mix);
    // Overdrive: a clean organ to a hot, tubey one.
    let dr = s.drive.unwrap();
    let d = routed("drive", "drive_db", 4.0, 0.55, 0.2);
    r.span(k, 4, dr, "drive", "drive_db", 0.0, d);
    r.span(k, 4, dr, "drive", "trim_db", -0.5, -2.0);
    perform(&mut r, knob, None, 4);
    r
}

// ---- plucks -----------------------------------------------------------------------------------

/// Rosewood: a deep marimba; the 4th partial rings a moment, the Ring macro lengthens the body.
fn rosewood() -> Rack {
    let mut r = Rack::new();
    // Mallet: hard or soft; Ring: the bars ring out; Beat: the partials beat; Roll: a tremolo roll (off).
    let knob = macros(
        &mut r,
        &["Mallet", "Ring", "Beat", "Roll"],
        &[0.80, 0.50, 0.08, 0.0],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let ops: [Op; 6] = [
        [1.0, 0.0, 1.0, 0.5, 900.0, 0.0, 160.0, 0.3],
        [4.0, 0.0, 0.5, 0.5, 130.0, 0.0, 60.0, 0.8],
        [4.0, 12.0, 0.4, 0.5, 260.0, 0.0, 100.0, 0.3],
        [10.0, 0.0, 0.3, 0.5, 25.0, 0.0, 20.0, 0.9],
        [1.0, 0.0, 0.0, 0.5, 100.0, 0.0, 50.0, 0.0],
        [1.0, 0.0, 0.0, 0.5, 100.0, 0.0, 50.0, 0.0],
    ];
    let v = fm(&mut r, m, 1.0, 0.55, 0.0, &ops);
    let s = post(
        &mut r,
        knob,
        v,
        &Post {
            drive: Some((0.0, 0.0, 0.3)),
            cutoff: 4500.0,
            cut_m1: 0.4,
            trim_db: 9.0,
            echo: &[
                ("time_ms", 330.0),
                ("feedback", 28.0),
                ("mix", 0.0),
                ("tone_hz", 4500.0),
            ],
            echo_max: 0.15,
            room_decay: 2.2,
            room_max: 0.4,
            fixed: true,
            old: [0.50, 0.20, 0.40, 0.45],
            ..pst()
        },
    );
    let (o, k) = ("osc.fm6", knob);
    // The old Ring decays (10 s for the first bar, 2.6 s for the second) were longer than any
    // note, so they stay at their stored length; the macro works on the release instead.
    let d1 = routed(o, "decay1", 900.0, 0.5, 0.45);
    let d3 = routed(o, "decay3", 260.0, 0.4, 0.45);
    r.e.set_param(v, "decay1", d1);
    r.e.set_param(v, "decay3", d3);
    // Mallet: a soft felt mallet to a hard one: more FM, a sharper click at the front.
    let index = routed(o, "index", 0.55, 0.5, 0.5);
    r.span(k, 1, v, o, "index", 0.6, index);
    r.span(k, 1, v, o, "level4", 0.0, 0.3);
    // Ring: a damped bar to a bar that rings on after the key is up.
    r.span_hi(k, 2, v, o, "release1", 160.0, 700.0);
    r.span_hi(k, 2, v, o, "release3", 100.0, 500.0);
    // Beat: the two fourth partials drift apart and beat against each other.
    r.span(k, 3, v, o, "fine3", 0.0, 12.0);
    r.span_hi(k, 3, v, o, "level3", 0.4, 0.8);
    // Roll: a tremolo on the level, the mallet roll of a marimba (off by default).
    let lfo = r.add("lfo", &[("rate_hz", 5.0)]);
    r.depth_route(lfo, "out", s.trim, "gain_db", k, "m4", 0.045);
    r.rise(k, 4, lfo, "lfo", "rate_hz", 5.0, 13.0);
    perform(&mut r, knob, None, 4);
    r
}

/// Rain Gutter: a kalimba with a long, wet tail.
fn rain_gutter() -> Rack {
    let mut r = Rack::new();
    // Tine: the metal of the kalimba; Ring: how long it rings; Space: the long tail; Shimmer.
    let knob = macros(
        &mut r,
        &["Tine", "Ring", "Echo", "Shimmer"],
        &[0.65, 0.50, 0.60, 0.30],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let ops: [Op; 6] = [
        [1.0, 0.0, 1.0, 0.5, 1700.0, 0.0, 250.0, 0.3],
        [5.0, 40.0, 0.5, 0.5, 70.0, 0.0, 40.0, 0.9],
        [1.0, 5.0, 0.45, 0.5, 2000.0, 0.0, 250.0, 0.3],
        [7.0, 0.0, 0.3, 0.5, 40.0, 0.0, 30.0, 0.9],
        [2.0, 0.0, 0.15, 0.5, 600.0, 0.0, 150.0, 0.4],
        [1.0, 0.0, 0.0, 0.5, 100.0, 0.0, 50.0, 0.0],
    ];
    let v = fm(&mut r, m, 1.0, 0.5, 0.0, &ops);
    let s = post(
        &mut r,
        knob,
        v,
        &Post {
            drive: Some((0.0, 0.0, 0.3)),
            cutoff: 6000.0,
            cut_m1: 0.35,
            trim_db: 11.0,
            chorus: Some((&[("rate_hz", 0.4), ("depth", 40.0), ("mix", 0.0)], None)),
            echo: &[
                ("time_ms", 420.0),
                ("feedback", 42.0),
                ("mix", 0.0),
                ("tone_hz", 5000.0),
            ],
            echo_max: 0.3,
            room_decay: 4.0,
            room_max: 0.45,
            fixed: true,
            old: [0.50, 0.15, 0.60, 0.40],
            ..pst()
        },
    );
    let (o, k) = ("osc.fm6", knob);
    // Tine: a soft, round thumb piano to a bright, pinging one.
    let index = routed(o, "index", 0.5, 0.5, 0.5);
    r.span(k, 1, v, o, "index", 0.3, index);
    r.span(k, 1, v, o, "level2", 0.0, 0.5);
    r.span(k, 1, v, o, "level4", 0.0, 0.3);
    // Ring: a damped tick to a tine that rings for seconds.
    r.span(k, 2, v, o, "decay1", 300.0, 1700.0);
    r.span(k, 2, v, o, "decay3", 400.0, 2000.0);
    r.level(k, 2, s.trim, -7.0);
    // Echo: a short echo and a small room to a long, ringing tail (the wet level stays).
    r.span(k, 3, s.verb, "reverb", "decay_s", 0.6, 4.0);
    r.span(k, 3, s.delay, "delay", "feedback", 0.0, 42.0);
    // Shimmer: a dry tone to a deep, slow chorus.
    let ch = s.chorus.unwrap();
    let mix = routed("chorus", "mix", 0.0, 0.55, 0.4);
    r.span(k, 4, ch, "chorus", "mix", 0.0, mix);
    r.span_hi(k, 4, ch, "chorus", "depth", 40.0, 100.0);
    r.span_hi(k, 4, ch, "chorus", "rate_hz", 0.4, 3.0);
    perform(&mut r, knob, None, 4);
    r
}

/// Spark Plug: a blunt, driven, compressed pluck.
fn spark_plug() -> Rack {
    let mut r = Rack::new();
    // Snap: a thud whose filter snaps open; Grit: the drive; Squash: the compressor; Zap: a pitch spark (off).
    let knob = macros(
        &mut r,
        &["Snap", "Grit", "Ring", "Zap"],
        &[1.0, 0.70, 0.60, 0.0],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let v = va(
        &mut r,
        m,
        &[("waveform", 2.0), ("unison", 2.0), ("detune", 10.0)],
        &[("waveform", 1.0), ("pw", 35.0), ("fine", -5.0)],
        (0.5, 0.4),
        adsr(0.5, 140.0, 0.0, 60.0),
        adsr(0.5, 110.0, 0.0, 60.0),
    );
    let s = post(
        &mut r,
        knob,
        v.out,
        &Post {
            drive: Some((18.0, -7.0, 0.3)),
            cutoff: 1300.0,
            resonance: 0.3,
            cut_m1: 0.5,
            trim_db: 10.0,
            comp: Some(&[
                ("threshold_db", -24.0),
                ("ratio", 7.0),
                ("attack_ms", 3.0),
                ("release_ms", 80.0),
                ("makeup_db", 6.5),
            ]),
            echo: &[
                ("time_ms", 242.0),
                ("feedback", 28.0),
                ("mix", 0.0),
                ("tone_hz", 3800.0),
            ],
            echo_max: 0.25,
            room_decay: 0.5,
            room_max: 0.12,
            fixed: true,
            old: [0.50, 0.60, 0.25, 0.40],
            ..pst()
        },
    );
    let k = knob;
    // The old Decay (4.5 s on a stab a tenth of a second long) did nothing: the decay stays at its stored length.
    let dec = routed("env.adsr", "decay_ms", 140.0, 0.5, 0.4);
    r.e.set_param(v.aenv, "decay_ms", dec);
    r.route(m, "velocity", s.lad, "cutoff_hz", 0.15);
    // Snap: a closed, dull thud whose filter snaps open on every note, to a wide open crack.
    let cut = routed("filter.ladder", "cutoff_hz", 1300.0, 0.5, 0.50);
    r.span(k, 1, s.lad, "filter.ladder", "cutoff_hz", 300.0, cut);
    r.depth_route(v.fenv, "out", s.lad, "cutoff_hz", k, "m1", 0.4);
    r.span(k, 1, v.fenv, "env.adsr", "decay_ms", 30.0, 110.0);
    // Grit: a clean blunt pluck to a crushed one.
    let d = routed("drive", "drive_db", 18.0, 0.3, 0.60);
    r.span(k, 2, s.drive.unwrap(), "drive", "drive_db", 6.0, d);
    r.span(k, 2, s.drive.unwrap(), "drive", "trim_db", 5.0, -7.0);
    // Ring: a staccato tick to a pluck that rings on after the key is up (the compressor stays as it was).
    r.span(k, 3, v.aenv, "env.adsr", "release_ms", 12.0, 60.0);
    r.span(k, 3, v.fenv, "env.adsr", "release_ms", 12.0, 60.0);
    r.level(k, 3, s.trim, -5.0);
    // Zap: each pluck starts a few tens of cents sharp and falls into tune (off by default).
    r.depth_route(v.fenv, "out", v.osc_a, "fine", k, "m4", 1.0);
    r.depth_route(v.fenv, "out", v.osc_b, "fine", k, "m4", 1.0);
    perform(&mut r, knob, None, 4);
    r
}

// ---- phrases ----------------------------------------------------------------------------------

fn tine_phrase() -> Vec<Hit> {
    comp(
        84.0,
        88,
        &[
            (0.0, 3.0, &[52, 59, 63, 66]),
            (3.0, 1.0, &[54, 61, 65, 68]),
            (4.0, 3.0, &[49, 56, 61, 65]),
            (7.0, 1.0, &[52, 59, 64, 68]),
            (8.0, 4.0, &[52, 59, 63, 66, 71]),
        ],
    )
}

fn checker_phrase() -> Vec<Hit> {
    // 124 BPM, syncopated stabs: a bar of Am9, a bar of Dm9.
    let am: &[u8] = &[57, 60, 64, 67, 71];
    let dm: &[u8] = &[53, 60, 64, 69];
    let mut v = Vec::new();
    for bar in 0..4 {
        let (c, b0) = if bar % 2 == 0 { (am, bar) } else { (dm, bar) };
        v.extend(comp(
            124.0,
            100,
            &[
                (b0 as f32 * 4.0 + 0.0, 0.5, c),
                (b0 as f32 * 4.0 + 0.75, 0.5, c),
                (b0 as f32 * 4.0 + 1.5, 0.5, c),
                (b0 as f32 * 4.0 + 2.5, 0.5, c),
                (b0 as f32 * 4.0 + 3.5, 0.25, c),
            ],
        ));
    }
    v
}

fn lino_phrase() -> Vec<Hit> {
    comp(
        122.0,
        96,
        &[
            (0.0, 1.5, &[57, 64, 67, 72]),
            (1.5, 0.5, &[57, 64, 67, 72]),
            (2.0, 1.5, &[55, 62, 67, 71]),
            (3.5, 0.5, &[55, 62, 67, 71]),
            (4.0, 1.5, &[53, 60, 65, 69]),
            (5.5, 0.5, &[53, 60, 65, 69]),
            (6.0, 2.0, &[52, 59, 64, 68]),
        ],
    )
}

fn cellar_phrase() -> Vec<Hit> {
    comp(
        80.0,
        80,
        &[
            (0.0, 4.0, &[50, 57, 60, 64]),
            (4.0, 4.0, &[47, 54, 58, 62]),
            (8.0, 4.0, &[48, 55, 59, 64]),
        ],
    )
}

fn pulpit_phrase() -> Vec<Hit> {
    comp(
        120.0,
        90,
        &[
            (0.0, 4.0, &[48, 55, 64, 67]),
            (4.0, 4.0, &[53, 57, 65, 69]),
            (8.0, 4.0, &[55, 59, 62, 67]),
            (12.0, 4.0, &[48, 55, 64, 67]),
        ],
    )
}

fn rosewood_phrase() -> Vec<Hit> {
    let notes = [
        57u8, 60, 64, 69, 67, 64, 60, 62, 65, 69, 72, 69, 67, 64, 62, 60,
    ];
    let mut v = line(
        &notes
            .iter()
            .enumerate()
            .map(|(i, &n)| (i as f32 * 0.22, 0.18, n, if i % 4 == 0 { 105 } else { 80 }))
            .collect::<Vec<_>>(),
    );
    v.extend(chord(4.0, 1.5, 100, &[45, 52, 60]));
    v
}

fn gutter_phrase() -> Vec<Hit> {
    let notes = [60u8, 67, 72, 71, 67, 64, 62, 64, 67, 72, 76, 74, 71, 67];
    line(
        &notes
            .iter()
            .enumerate()
            .map(|(i, &n)| {
                (
                    i as f32 * 0.4 + if i % 3 == 2 { 0.1 } else { 0.0 },
                    0.3,
                    n,
                    70 + (i as u8 * 7) % 30,
                )
            })
            .collect::<Vec<_>>(),
    )
}

fn spark_phrase() -> Vec<Hit> {
    // 124 BPM sixteenths, off-beat accents.
    let s16 = 60.0 / 124.0 / 4.0;
    let pat: [(u32, u8, u8); 12] = [
        (0, 45, 110),
        (3, 45, 90),
        (6, 52, 100),
        (8, 48, 110),
        (11, 48, 90),
        (14, 55, 100),
        (16, 45, 110),
        (19, 57, 90),
        (22, 52, 100),
        (24, 43, 110),
        (27, 50, 90),
        (30, 55, 100),
    ];
    line(&pat.map(|(s, n, v)| (s as f32 * s16, s16 * 0.7, n, v)))
}

// ---- the list ---------------------------------------------------------------------------------

pub fn presets() -> Vec<Preset> {
    let mut all = vec![
        Preset {
            slug: "velvet-tine",
            name: "Velvet Tine",
            category: "Keys",
            tags: &["poly", "fm", "phased", "pop", "soul"],
            description: "A soft Rhodes-like electric piano: a mellow body, a tine that fades in a tenth of a second, velocity-sensitive bark. Play chords. Tine grows a metallic tine out of each note, Ring lengthens the note from a thud to a long, ringing one, Detune pulls the second tine out of tune so the note beats and chorus, Phase brings in a slow phaser.",
            listen: "The tine on the attack against the soft body; the phaser slowly breathing on held chords",
            group: 1,
            build: velvet_tine,
            phrase: tine_phrase,
            secs: 15.0,
        },
        Preset {
            slug: "checkerboard",
            name: "Checkerboard",
            category: "Keys",
            tags: &["poly", "stab", "distorted", "compressed", "french touch", "124bpm", "signature"],
            description: "A house piano stab: two saws and a square, driven hard and squashed until it barks. Play short syncopated chords. Bite goes from a closed, thudding stab to a wide open bark as the filter snaps open on every note, Hollow turns the stab hollow: a triangle (odd harmonics, no top) takes over from the saws at the same loudness, Squash clamps the compressor harder, Spread widens and detunes the saw stack.",
            listen: "The bark of the compressor on every stab and the filter snapping shut after each one",
            group: 1,
            build: checkerboard,
            phrase: checker_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "linoleum",
            name: "Linoleum",
            category: "Keys",
            tags: &["poly", "fm", "phased", "compressed", "french touch", "122bpm", "signature"],
            description: "A phased disco electric piano, driven and compressed: bright, clipped, with a six-stage phaser always on. Play two-beat chord stabs. Swirl runs the phaser from slow and thin to fast and ringing, Chime grows the bell partial of the electric piano, Hold lengthens the ring, Squeeze clamps the compressor harder.",
            listen: "The phaser sweeping through each held stab, and the compressor pulling the tails up between them",
            group: 1,
            build: linoleum,
            phrase: lino_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "cellar-door",
            name: "Cellar Door",
            category: "Keys",
            tags: &["poly", "fm", "dark", "hip-hop", "lofi"],
            description: "A dark electric piano that sits low in a beat: a closed filter, a little drive, a slow pitch wobble from the chorus. Play slow minor chords. Dark closes the filter and the FM index into a muffled felt tone, Felt softens the hammer into a slow push, Wobble fades in a slow tape pitch drift, Smear brings in a deep, slow chorus.",
            listen: "The soft, closed tone and the slow chorus wobbling on the long chords",
            group: 1,
            build: cellar_door,
            phrase: cellar_phrase,
            secs: 15.0,
        },
        Preset {
            slug: "pulpit",
            name: "Pulpit",
            category: "Keys",
            tags: &["poly", "organ", "additive", "house", "pop"],
            description: "A drawbar-style organ from six added sines, played through a rotary-speaker chorus. Hold chords, or play a house line. Bars brings in the sub and the second drawbar, Perc adds a percussive attack to the upper bars, Rotor speeds the Leslie from slow to fast, Overdrive heats the tubes.",
            listen: "The steady flat organ tone and the rotor shimmering across each chord",
            group: 1,
            build: pulpit,
            phrase: pulpit_phrase,
            secs: 17.0,
        },
        Preset {
            slug: "rosewood",
            name: "Rosewood",
            category: "Pluck",
            tags: &["poly", "fm", "marimba", "world", "pop"],
            description: "A deep marimba: a round body, a tuned overtone that rings for a moment, and a soft mallet. Play it with velocity. Mallet goes from a felt mallet to a hard one with a sharper click, Ring lets the bars ring on after the key is up, Beat pulls the fourth partials apart so they beat, Roll fades in a tremolo roll.",
            listen: "The round body under each strike; hard hits are brighter than soft ones",
            group: 1,
            build: rosewood,
            phrase: rosewood_phrase,
            secs: 8.0,
        },
        Preset {
            slug: "rain-gutter",
            name: "Rain Gutter",
            category: "Pluck",
            tags: &["poly", "fm", "kalimba", "ambient", "pop"],
            description: "A kalimba: a short metallic click, a long clear ring, and a wet tail that blurs the notes into each other. Play slow, sparse notes. Tine goes from a soft thumb piano to a bright, pinging one, Ring lengthens the ring of each tine, Echo stretches the echo and the long room, Shimmer brings in a deep, slow chorus.",
            listen: "The click of each tine against its long ring, and the echo answering a half-beat later",
            group: 1,
            build: rain_gutter,
            phrase: gutter_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "spark-plug",
            name: "Spark Plug",
            category: "Pluck",
            tags: &["poly", "pluck", "distorted", "compressed", "french touch", "124bpm", "signature"],
            description: "A blunt, driven synth pluck: two saws and a square, 18 dB of drive, a fast compressor. Play short low notes. Snap goes from a dull thud to a crack as the filter snaps open on every note, Grit pushes the drive, Ring lets each pluck ring on after the key is up, Zap starts each pluck sharp and lets it fall into tune.",
            listen: "The filter snap on every note and the compressor holding the dirt steady",
            group: 1,
            build: spark_plug,
            phrase: spark_phrase,
            secs: 6.0,
        },
    ];
    all.extend(group2::presets());
    all
}
