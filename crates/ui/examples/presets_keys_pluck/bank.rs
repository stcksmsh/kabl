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
    }
}

/// A subtractive voice: two `osc.va` into a mixer, an amplitude envelope on a VCA, and a filter
/// envelope the caller routes into the ladder returned by `post`.
struct Va {
    out: ModuleId,
    osc_a: ModuleId,
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
    Va { out: vca, osc_a: oa, aenv, fenv }
}

// ---- keys -------------------------------------------------------------------------------------

/// Velvet Tine: a soft Rhodes-like electric piano with a phaser on the Phase macro.
fn velvet_tine() -> Rack {
    let mut r = Rack::new();
    let knob = macros(&mut r, &["Tine", "Drive", "Space", "Phase"], &[0.50, 0.20, 0.45, 0.35]);
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
                Some(("mix", 0.5)),
            )),
            echo: &[("time_ms", 300.0), ("feedback", 28.0), ("mix", 0.0), ("tone_hz", 4500.0)],
            echo_max: 0.2,
            room_decay: 2.4,
            room_max: 0.35,
            ..pst()
        },
    );
    r.route(knob, "m1", v, "index", 0.5);
    let _ = s;
    perform(&mut r, knob, None, 4);
    r
}

/// Checkerboard: a house piano stab, driven and compressed until it barks.
fn checkerboard() -> Rack {
    let mut r = Rack::new();
    let knob = macros(&mut r, &["Cutoff", "Crunch", "Room", "Length"], &[0.50, 0.55, 0.25, 0.40]);
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
            echo: &[("time_ms", 161.0), ("feedback", 25.0), ("mix", 0.0), ("tone_hz", 4000.0)],
            echo_max: 0.25,
            room_decay: 0.6,
            room_max: 0.15,
            ..pst()
        },
    );
    r.route(v.fenv, "out", s.lad, "cutoff_hz", 0.35);
    r.route(m, "velocity", s.lad, "cutoff_hz", 0.15);
    r.route(knob, "m4", v.aenv, "decay_ms", 0.5);
    perform(&mut r, knob, None, 4);
    r
}

/// Linoleum: a phased disco electric piano, driven and compressed.
fn linoleum() -> Rack {
    let mut r = Rack::new();
    let knob = macros(&mut r, &["Bright", "Crunch", "Room", "Swirl"], &[0.50, 0.45, 0.25, 0.40]);
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
    post(
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
                Some(("rate_hz", 0.5)),
            )),
            comp: Some(&[
                ("threshold_db", -24.0),
                ("ratio", 5.0),
                ("attack_ms", 6.0),
                ("release_ms", 110.0),
                ("makeup_db", 5.0),
            ]),
            echo: &[("time_ms", 244.0), ("feedback", 30.0), ("mix", 0.0), ("tone_hz", 4500.0)],
            echo_max: 0.2,
            room_decay: 0.7,
            room_max: 0.15,
            ..pst()
        },
    );
    r.route(knob, "m1", v, "index", 0.4);
    perform(&mut r, knob, None, 4);
    r
}

/// Cellar Door: a dark, slow-wobbling electric piano for hip-hop.
fn cellar_door() -> Rack {
    let mut r = Rack::new();
    let knob = macros(&mut r, &["Dark", "Dirt", "Room", "Wobble"], &[0.35, 0.30, 0.30, 0.40]);
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
    post(
        &mut r,
        knob,
        v,
        &Post {
            drive: Some((6.0, -2.0, 0.4)),
            cutoff: 1800.0,
            resonance: 0.12,
            cut_m1: 0.6,
            trim_db: 8.5,
            chorus: Some((&[("rate_hz", 0.35), ("depth", 70.0), ("mix", 0.0)], Some(0.6))),
            echo: &[("time_ms", 340.0), ("feedback", 32.0), ("mix", 0.0), ("tone_hz", 3000.0)],
            echo_max: 0.2,
            room_decay: 1.4,
            room_max: 0.3,
            ..pst()
        },
    );
    r.route(knob, "m1", v, "index", 0.5);
    perform(&mut r, knob, None, 4);
    r
}

/// Pulpit: a drawbar-style organ (six added sines) through a rotor.
fn pulpit() -> Rack {
    let mut r = Rack::new();
    let knob = macros(&mut r, &["Brightness", "Overdrive", "Room", "Rotor"], &[0.55, 0.20, 0.35, 0.50]);
    let m = r.add("midi.in", &[("bend", 2.0)]);
    // Sub, fundamental, 2nd, 3rd, 4th and 6th harmonics: ratio, fine, level, then a flat envelope.
    let bars = [(0.0, 0.6), (1.0, 1.0), (2.0, 0.8), (3.0, 0.5), (4.0, 0.4), (6.0, 0.25)];
    let ops: [Op; 6] = bars.map(|(ratio, level)| [ratio, 0.0, level, 3.0, 50.0, 1.0, 40.0, 0.0]);
    let v = fm(&mut r, m, 7.0, 1.0, 0.0, &ops);
    post(
        &mut r,
        knob,
        v,
        &Post {
            drive: Some((4.0, -2.0, 0.55)),
            cutoff: 3500.0,
            resonance: 0.05,
            cut_m1: 0.6,
            trim_db: 1.0,
            chorus: Some((&[("rate_hz", 5.5), ("depth", 22.0), ("mix", 0.0)], Some(0.55))),
            echo: &[("time_ms", 300.0), ("feedback", 25.0), ("mix", 0.0), ("tone_hz", 4500.0)],
            echo_max: 0.1,
            room_decay: 1.6,
            room_max: 0.3,
            ..pst()
        },
    );
    perform(&mut r, knob, None, 4);
    r
}

// ---- plucks -----------------------------------------------------------------------------------

/// Rosewood: a deep marimba; the 4th partial rings a moment, the Ring macro lengthens the body.
fn rosewood() -> Rack {
    let mut r = Rack::new();
    let knob = macros(&mut r, &["Mallet", "Warmth", "Room", "Ring"], &[0.50, 0.20, 0.40, 0.45]);
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
    post(
        &mut r,
        knob,
        v,
        &Post {
            drive: Some((0.0, 0.0, 0.3)),
            cutoff: 4500.0,
            cut_m1: 0.4,
            trim_db: 9.0,
            echo: &[("time_ms", 330.0), ("feedback", 28.0), ("mix", 0.0), ("tone_hz", 4500.0)],
            echo_max: 0.15,
            room_decay: 2.2,
            room_max: 0.4,
            ..pst()
        },
    );
    r.route(knob, "m1", v, "index", 0.5);
    r.route(knob, "m4", v, "decay1", 0.5);
    r.route(knob, "m4", v, "decay3", 0.4);
    perform(&mut r, knob, None, 4);
    r
}

/// Rain Gutter: a kalimba with a long, wet tail.
fn rain_gutter() -> Rack {
    let mut r = Rack::new();
    let knob = macros(&mut r, &["Tine", "Warmth", "Space", "Shimmer"], &[0.50, 0.15, 0.60, 0.40]);
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
    post(
        &mut r,
        knob,
        v,
        &Post {
            drive: Some((0.0, 0.0, 0.3)),
            cutoff: 6000.0,
            cut_m1: 0.35,
            trim_db: 11.0,
            chorus: Some((&[("rate_hz", 0.4), ("depth", 40.0), ("mix", 0.0)], Some(0.55))),
            echo: &[("time_ms", 420.0), ("feedback", 42.0), ("mix", 0.0), ("tone_hz", 5000.0)],
            echo_max: 0.3,
            room_decay: 4.0,
            room_max: 0.45,
            ..pst()
        },
    );
    r.route(knob, "m1", v, "index", 0.5);
    perform(&mut r, knob, None, 4);
    r
}

/// Spark Plug: a blunt, driven, compressed pluck.
fn spark_plug() -> Rack {
    let mut r = Rack::new();
    let knob = macros(&mut r, &["Cutoff", "Crunch", "Room", "Decay"], &[0.50, 0.60, 0.25, 0.40]);
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
            echo: &[("time_ms", 242.0), ("feedback", 28.0), ("mix", 0.0), ("tone_hz", 3800.0)],
            echo_max: 0.25,
            room_decay: 0.5,
            room_max: 0.12,
            ..pst()
        },
    );
    r.route(v.fenv, "out", s.lad, "cutoff_hz", 0.4);
    r.route(m, "velocity", s.lad, "cutoff_hz", 0.15);
    r.route(knob, "m4", v.aenv, "decay_ms", 0.5);
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
        v.extend(comp(124.0, 100, &[
            (b0 as f32 * 4.0 + 0.0, 0.5, c),
            (b0 as f32 * 4.0 + 0.75, 0.5, c),
            (b0 as f32 * 4.0 + 1.5, 0.5, c),
            (b0 as f32 * 4.0 + 2.5, 0.5, c),
            (b0 as f32 * 4.0 + 3.5, 0.25, c),
        ]));
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
    let notes = [57u8, 60, 64, 69, 67, 64, 60, 62, 65, 69, 72, 69, 67, 64, 62, 60];
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
            .map(|(i, &n)| (i as f32 * 0.4 + if i % 3 == 2 { 0.1 } else { 0.0 }, 0.3, n, 70 + (i as u8 * 7) % 30))
            .collect::<Vec<_>>(),
    )
}

fn spark_phrase() -> Vec<Hit> {
    // 124 BPM sixteenths, off-beat accents.
    let s16 = 60.0 / 124.0 / 4.0;
    let pat: [(u32, u8, u8); 12] = [
        (0, 45, 110), (3, 45, 90), (6, 52, 100), (8, 48, 110), (11, 48, 90), (14, 55, 100),
        (16, 45, 110), (19, 57, 90), (22, 52, 100), (24, 43, 110), (27, 50, 90), (30, 55, 100),
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
            description: "A soft Rhodes-like electric piano: a mellow body, a tine that fades in a tenth of a second, velocity-sensitive bark. Play chords. Tine opens the FM brightness and the filter, Drive adds warmth, Space sets the echo and room, Phase brings in a slow phaser.",
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
            description: "A house piano stab: two saws and a square, driven hard and squashed until it barks. Play short syncopated chords. Cutoff opens the filter, Crunch pushes the drive, Room sets a slapback and a tight room, Length stretches the stab.",
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
            description: "A phased disco electric piano, driven and compressed: bright, clipped, with a six-stage phaser always on. Play two-beat chord stabs. Bright opens the FM index and the filter, Crunch pushes the drive, Room sets a tight room and a slapback, Swirl speeds the phaser.",
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
            description: "A dark electric piano that sits low in a beat: a closed filter, a little drive, a slow pitch wobble from the chorus. Play slow minor chords. Dark opens the filter and the FM index, Dirt adds drive, Room sets the echo and room, Wobble brings in the slow chorus.",
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
            description: "A drawbar-style organ from six added sines, played through a rotary-speaker chorus. Hold chords, or play a house line. Brightness opens the filter, Overdrive adds the crunch of a pushed tube, Room sets a short room, Rotor brings in the speaker and speeds it up.",
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
            description: "A deep marimba: a round body, a tuned overtone that rings for a moment, and a soft mallet. Play it with velocity. Mallet sets the hardness of the strike, Warmth adds a little drive, Room sets the echo and a medium room, Ring lengthens the body.",
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
            description: "A kalimba: a short metallic click, a long clear ring, and a wet tail that blurs the notes into each other. Play slow, sparse notes. Tine sets the brightness of the click, Warmth adds a little drive, Space sets the echo and the long room, Shimmer brings in the slow chorus.",
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
            description: "A blunt, driven synth pluck: two saws and a square, 18 dB of drive, a fast compressor. Play short low notes. Cutoff opens the filter, Crunch pushes the drive, Room sets a slapback and a tight room, Decay lengthens the note.",
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
