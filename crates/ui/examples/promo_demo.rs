//! The promo piece "Morph Suite": one patch, built through the real `PatchEditor`, a timeline of
//! morph gestures, and an offline render of that timeline. The same gestures are replayed by hand
//! in the real app by `docs/promo-demo/scripts/make-drive-script.py`.
//!
//! ```sh
//! cargo run --release -p kabl-ui --example promo_demo -- write docs/promo-demo/patch/morph-suite docs/promo-demo/timeline.json
//! cargo run --release -p kabl-ui --example promo_demo -- render out.wav
//! ```

use std::collections::HashMap;
use std::path::Path;

use kabl_core::{CableId, ParamTarget, PortRef, Vec2};
use kabl_engine::compile::compile;
use kabl_engine::graph::BLOCK;
use kabl_ui::PatchEditor;

const SR: f32 = 48000.0;
const BPM: f32 = 100.0;
const BAR_S: f32 = 16.0 * 60.0 / (BPM * 4.0);
/// Piece length in bars (the last bars are the tail of the echo and the reverb).
const BARS: f32 = 70.0;

/// One hand movement of one cable's "Morph A to B" value, in bars. Gestures never overlap in time
/// (the editor shows one cable at a time) and start at least two bars after the one before ends
/// (time to open the next cable's editor).
struct Gesture {
    cable: &'static str,
    from: f32,
    to: f32,
    start: f32,
    end: f32,
    what: &'static str,
}

const fn g(
    cable: &'static str,
    from: f32,
    to: f32,
    start: f32,
    end: f32,
    what: &'static str,
) -> Gesture {
    Gesture {
        cable,
        from,
        to,
        start,
        end,
        what,
    }
}

/// The piece. Bars 0-4: intro (a sparse arp, a held pad). 4-31 build. 31-36 peak. 36-44 break.
/// 44-53 return (snaps in on the downbeat of bar 44). 53-70 outro.
const GESTURES: [Gesture; 12] = [
    g(
        "echo",
        0.0,
        1.0,
        4.0,
        7.0,
        "echoes: one throw per cycle to every step",
    ),
    g(
        "arp",
        0.0,
        1.0,
        9.0,
        11.0,
        "arp: sparse and chancy to every note",
    ),
    g("bass", 0.0, 1.0, 13.0, 15.0, "bass: silent to full"),
    g(
        "filter",
        0.0,
        1.0,
        17.0,
        21.0,
        "bass filter: shut to a busy five against four",
    ),
    g(
        "bells",
        0.0,
        1.0,
        23.0,
        25.5,
        "bells: none to a chancy sixteen",
    ),
    g("pad", 0.0, 1.0, 28.0, 31.0, "pad: held to chopped"),
    g(
        "bus",
        1.0,
        0.0,
        35.6,
        36.0,
        "break: the band cut to two stabs on the phrase, the echoes ring",
    ),
    g(
        "bus",
        0.0,
        1.0,
        43.6,
        44.0,
        "return: everything back on the downbeat of the phrase",
    ),
    g("pad", 1.0, 0.0, 53.0, 54.5, "pad: back to held"),
    g("bass", 1.0, 0.0, 56.5, 58.5, "bass: out"),
    g("bells", 1.0, 0.0, 60.5, 62.0, "bells: out"),
    g("arp", 1.0, 0.0, 64.0, 65.5, "arp: thins out"),
];

const CABLES: [&str; 7] = ["echo", "arp", "bass", "filter", "bells", "pad", "bus"];

/// A cable's morph (0..1) at `bar`, from the gestures.
fn morph_at(cable: &str, bar: f32) -> f32 {
    let mut v = if cable == "bus" { 1.0 } else { 0.0 };
    for t in GESTURES.iter().filter(|t| t.cable == cable) {
        if bar >= t.end {
            v = t.to;
        } else if bar > t.start {
            v = t.from + (t.to - t.from) * (bar - t.start) / (t.end - t.start);
        }
    }
    v
}

fn jack(id: u64, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

fn at(x: f32, y: f32) -> Vec2 {
    Vec2 { x, y }
}

fn cable_params(e: &mut PatchEditor, cable: CableId, params: &[(String, f32)]) {
    for (name, v) in params {
        e.set_param_gesture(
            ParamTarget::Cable {
                id: cable,
                param: name.clone(),
            },
            *v,
            true,
        );
    }
}

/// Pattern params: A levels, B levels, optional `(step 1-based, chance %)` for each, glide, and
/// the starting morph.
fn pattern(
    a: &[f32],
    b: &[f32],
    a_chance: &[(usize, f32)],
    b_chance: &[(usize, f32)],
    glide: f32,
    morph: f32,
) -> Vec<(String, f32)> {
    let mut p = vec![
        ("length".to_string(), a.len() as f32),
        ("b.length".to_string(), b.len() as f32),
        ("glide_ms".to_string(), glide),
        ("morph".to_string(), morph),
    ];
    for (k, v) in a.iter().enumerate() {
        p.push((format!("s{}", k + 1), *v));
    }
    for (k, v) in b.iter().enumerate() {
        p.push((format!("b.s{}", k + 1), *v));
    }
    for (k, v) in a_chance {
        p.push((format!("r{k}"), *v));
    }
    for (k, v) in b_chance {
        p.push((format!("b.r{k}"), *v));
    }
    p
}

/// The patch, and the cable id of each named pattern cable.
fn build() -> (PatchEditor, HashMap<&'static str, CableId>) {
    let mut e = PatchEditor::new();
    let mut ids = HashMap::new();
    let clock = e.add_module("clock", at(20.0, 20.0));
    let div8 = e.add_module("clock.div", at(20.0, 160.0));
    let div2 = e.add_module("clock.div", at(20.0, 300.0));
    let chords = e.add_module("seq", at(220.0, 20.0));
    let arp_seq = e.add_module("seq", at(220.0, 300.0));
    let arp = e.add_module("osc.va", at(420.0, 20.0));
    let arp_filter = e.add_module("filter.svf", at(620.0, 20.0));
    let arp_env = e.add_module("env.adsr", at(820.0, 20.0));
    let arp_vca = e.add_module("vca", at(1020.0, 20.0));
    let delay = e.add_module("delay", at(1220.0, 20.0));
    let bass_seq = e.add_module("seq", at(220.0, 560.0));
    let bass = e.add_module("osc.va", at(420.0, 300.0));
    let bass2 = e.add_module("osc.va", at(420.0, 430.0));
    let bass_mix = e.add_module("mixer", at(520.0, 430.0));
    let bass_filter = e.add_module("filter.ladder", at(620.0, 300.0));
    let bass_env = e.add_module("env.adsr", at(820.0, 300.0));
    let bass_vca = e.add_module("vca", at(1020.0, 300.0));
    let drive = e.add_module("drive", at(1220.0, 300.0));
    let knob = e.add_module("macro", at(620.0, 560.0));
    let bell = e.add_module("osc.va", at(420.0, 560.0));
    let bell_env = e.add_module("env.adsr", at(820.0, 560.0));
    let bell_vca = e.add_module("vca", at(1020.0, 560.0));
    let bell_gain = e.add_module("gain", at(1120.0, 560.0));
    let pad_a = e.add_module("osc.va", at(20.0, 820.0));
    let pad_b = e.add_module("osc.va", at(220.0, 820.0));
    let pad_c = e.add_module("osc.va", at(420.0, 820.0));
    let pad_mix = e.add_module("mixer", at(620.0, 820.0));
    let pad_filter = e.add_module("filter.ladder", at(820.0, 820.0));
    let slow = e.add_module("lfo", at(1020.0, 820.0));
    let rhythm = e.add_module("mixer", at(1220.0, 560.0));
    let bus = e.add_module("mixer", at(1420.0, 160.0));
    let master = e.add_module("mixer", at(1420.0, 420.0));
    let chorus = e.add_module("chorus", at(1420.0, 680.0));
    let verb = e.add_module("reverb", at(1620.0, 160.0));
    let out = e.add_module("out", at(1620.0, 420.0));

    e.set_param(clock, "bpm", BPM);
    e.set_param(div8, "div", 8.0);
    e.set_param(div2, "div", 2.0);
    // Chords: A minor, F, C, G (pitch relative to A); one per bar.
    e.set_param(chords, "length", 4.0);
    for (k, p) in [0.0, -4.0, 3.0, -2.0].into_iter().enumerate() {
        e.set_param(chords, &format!("p{}", k + 1), p);
    }
    // Arp: A minor tones, C4 = 0.
    for (k, p) in [-3.0, 0.0, 4.0, 9.0, 7.0, 4.0, 0.0, 2.0]
        .into_iter()
        .enumerate()
    {
        e.set_param(arp_seq, &format!("p{}", k + 1), p);
    }
    e.set_param(arp, "waveform", 2.0);
    e.set_param(arp_filter, "cutoff_hz", 4200.0);
    e.set_param(arp_filter, "resonance", 0.25);
    e.set_param(arp_env, "attack_ms", 2.0);
    e.set_param(arp_env, "decay_ms", 190.0);
    e.set_param(arp_env, "sustain", 0.0);
    e.set_param(arp_env, "release_ms", 140.0);
    e.set_param(arp_vca, "gain", 0.0);
    e.set_param(delay, "sync", 3.0);
    e.set_param(delay, "feedback", 64.0);
    e.set_param(delay, "mix", 100.0);
    e.set_param(delay, "tone_hz", 3000.0);
    e.set_param(delay, "mode", 1.0);
    // Bass: the chord's root (A3 = 220 Hz, an octave up so small speakers carry it); the
    // sequencer only supplies the rhythm.
    e.set_param(bass_seq, "length", 8.0);
    for (k, on) in [1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 1.0]
        .into_iter()
        .enumerate()
    {
        e.set_param(bass_seq, &format!("g{}", k + 1), on);
    }
    e.set_param(bass, "waveform", 2.0);
    e.set_param(bass, "base_hz", 220.0);
    e.set_param(bass2, "waveform", 2.0);
    e.set_param(bass2, "base_hz", 440.0);
    e.set_param(bass_mix, "level1", 1.0);
    e.set_param(bass_mix, "level2", 0.8);
    e.set_param(bass_filter, "cutoff_hz", 220.0);
    e.set_param(bass_filter, "resonance", 0.45);
    e.set_param(bass_env, "attack_ms", 4.0);
    e.set_param(bass_env, "decay_ms", 400.0);
    e.set_param(bass_env, "sustain", 0.7);
    e.set_param(bass_env, "release_ms", 160.0);
    e.set_param(bass_vca, "gain", 0.0);
    e.set_param(drive, "drive_db", 18.0);
    e.set_param(drive, "mix", 100.0);
    e.set_param(drive, "trim_db", -2.0);
    e.set_param(knob, "m1", 1.0);
    // Bells: a bright stab on the chord root (A5), with a short decay.
    e.set_param(bell, "waveform", 2.0);
    e.set_param(bell, "base_hz", 880.0);
    e.set_param(bell_env, "attack_ms", 1.0);
    e.set_param(bell_env, "decay_ms", 180.0);
    e.set_param(bell_env, "sustain", 0.0);
    e.set_param(bell_env, "release_ms", 80.0);
    e.set_param(bell_vca, "gain", 0.0);
    e.set_param(bell_gain, "gain_db", 4.0);
    // Pad: open fifths over the chord root, detuned unison, under a slowly moving filter.
    for (osc, hz) in [(pad_a, 329.63), (pad_b, 440.0), (pad_c, 659.25)] {
        e.set_param(osc, "waveform", 2.0);
        e.set_param(osc, "base_hz", hz);
        e.set_param(osc, "unison", 3.0);
        e.set_param(osc, "detune", 14.0);
    }
    e.set_param(pad_filter, "cutoff_hz", 1800.0);
    e.set_param(pad_filter, "resonance", 0.15);
    e.set_param(slow, "rate_hz", 0.07);
    e.set_param(slow, "waveform", 1.0);
    for (k, v) in [0.34, 0.3, 0.26].into_iter().enumerate() {
        e.set_param(pad_mix, &format!("level{}", k + 1), v);
    }
    e.set_param(rhythm, "level1", 1.0);
    e.set_param(rhythm, "level2", 1.0);
    e.set_param(bus, "level1", 0.145);
    e.set_param(bus, "level2", 0.7);
    e.set_param(bus, "level3", 0.11);
    e.set_param(bus, "level4", 0.11);
    e.set_param(master, "level1", 1.0);
    e.set_param(master, "level2", 1.0);
    e.set_param(master, "level3", 0.8);
    e.set_param(verb, "decay_s", 5.0);
    e.set_param(verb, "mix", 30.0);

    // Clocks: the 16th pulse, and one pulse per bar for the chords.
    for s in [arp_seq, bass_seq] {
        e.connect(jack(clock, "gate"), jack(s, "clock"));
        e.connect(jack(clock, "reset"), jack(s, "reset"));
    }
    e.connect(jack(clock, "gate"), jack(delay, "clock"));
    e.connect(jack(clock, "gate"), jack(div8, "clock"));
    e.connect(jack(clock, "reset"), jack(div8, "reset"));
    e.connect(jack(div8, "gate"), jack(div2, "clock"));
    e.connect(jack(clock, "reset"), jack(div2, "reset"));
    e.connect(jack(div2, "gate"), jack(chords, "clock"));
    e.connect(jack(clock, "reset"), jack(chords, "reset"));
    // Arp.
    e.connect(jack(arp_seq, "pitch"), jack(arp, "pitch"));
    e.connect(jack(arp_seq, "gate"), jack(arp_env, "gate"));
    e.connect(jack(arp, "out"), jack(arp_filter, "in"));
    e.connect(jack(arp_filter, "lp"), jack(arp_vca, "in"));
    e.connect(jack(arp_env, "out"), jack(arp_vca, "cv"));
    // The arp's level: sparse and chancy, to every note.
    let c = e.connect(jack(arp_vca, "out"), jack(master, "in3"));
    let p = pattern(
        &[0.8, 0.0, 0.5, 0.0, 0.8, 0.3, 0.0, 0.5],
        &[1.0; 8],
        &[(1, 70.0), (3, 50.0), (5, 70.0), (6, 40.0), (8, 50.0)],
        &[],
        0.0,
        0.0,
    );
    cable_params(&mut e, c, &p);
    ids.insert("arp", c);
    // The echo send: one throw per cycle, then every step.
    let c = e.connect(jack(arp_vca, "out"), jack(delay, "in"));
    let a = [0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let b_chance: Vec<(usize, f32)> = (1..=8).map(|k| (k, 90.0)).collect();
    let p = pattern(&a, &[1.0; 8], &[(1, 60.0)], &b_chance, 0.0, 0.0);
    cable_params(&mut e, c, &p);
    ids.insert("echo", c);
    e.connect(jack(delay, "left"), jack(bus, "in3"));
    e.connect(jack(delay, "right"), jack(bus, "in4"));
    // Bass.
    e.connect(jack(chords, "pitch"), jack(bass, "pitch"));
    e.connect(jack(bass_seq, "gate"), jack(bass_env, "gate"));
    e.connect(jack(chords, "pitch"), jack(bass2, "pitch"));
    e.connect(jack(bass, "out"), jack(bass_mix, "in1"));
    e.connect(jack(bass2, "out"), jack(bass_mix, "in2"));
    e.connect(jack(bass_mix, "out"), jack(bass_filter, "in"));
    e.connect(jack(bass_filter, "out"), jack(bass_vca, "in"));
    e.connect(jack(bass_env, "out"), jack(bass_vca, "cv"));
    e.connect(jack(bass_vca, "out"), jack(drive, "in"));
    // Bass level: silent, then full.
    let c = e.connect(jack(drive, "out"), jack(rhythm, "in1"));
    let p = pattern(&[0.0; 4], &[1.0; 4], &[], &[], 20.0, 0.0);
    cable_params(&mut e, c, &p);
    ids.insert("bass", c);
    // Bass filter: A is four shut steps, B is five uneven ones against the eight-step bass.
    let c = e.connect_route(jack(knob, "m1"), bass_filter, "cutoff_hz");
    e.set_route_amount(c, 1.0, true);
    let p = pattern(
        &[0.08; 4],
        &[1.0, 0.05, 0.8, 0.3, 1.0],
        &[],
        &[(2, 70.0)],
        30.0,
        0.0,
    );
    cable_params(&mut e, c, &p);
    ids.insert("filter", c);
    // Bells: struck on every sixteenth, a cable decides which are heard. A: none. B: sixteen
    // steps, some by chance.
    e.connect(jack(clock, "gate"), jack(bell_env, "gate"));
    e.connect(jack(chords, "pitch"), jack(bell, "pitch"));
    e.connect(jack(bell, "out"), jack(bell_vca, "in"));
    e.connect(jack(bell_env, "out"), jack(bell_vca, "cv"));
    e.connect(jack(bell_vca, "out"), jack(bell_gain, "in"));
    let c = e.connect(jack(bell_gain, "out"), jack(rhythm, "in2"));
    let b = [
        1.0, 0.0, 0.0, 0.8, 0.0, 0.0, 1.0, 0.0, 0.0, 0.8, 0.0, 0.6, 0.0, 0.0, 1.0, 0.0,
    ];
    let chance = [(4, 70.0), (10, 70.0), (12, 50.0)];
    let p = pattern(&[0.0; 16], &b, &[], &chance, 3.0, 0.0);
    cable_params(&mut e, c, &p);
    ids.insert("bells", c);
    e.connect(jack(rhythm, "out"), jack(master, "in1"));
    // Pad, transposed with the chords.
    for osc in [pad_a, pad_b, pad_c] {
        e.connect(jack(chords, "pitch"), jack(osc, "pitch"));
    }
    e.connect(jack(pad_a, "out"), jack(pad_mix, "in1"));
    e.connect(jack(pad_b, "out"), jack(pad_mix, "in2"));
    e.connect(jack(pad_c, "out"), jack(pad_mix, "in3"));
    e.connect(jack(pad_mix, "out"), jack(pad_filter, "in"));
    let lfo = e.connect_route(jack(slow, "out"), pad_filter, "cutoff_hz");
    e.set_route_amount(lfo, 0.2, true);
    // Pad gate. A: held, breathing. B: a sixteen-step chop.
    let c = e.connect(jack(pad_filter, "out"), jack(master, "in2"));
    let a: Vec<f32> = (1..=16)
        .map(|k| if k % 4 == 1 { 0.85 } else { 0.7 })
        .collect();
    let on = [1, 0, 1, 1, 0, 1, 0, 1, 1, 0, 1, 0, 1, 1, 0, 1].map(|v| v as f32);
    let p = pattern(&a, &on, &[], &[], 45.0, 0.0);
    cable_params(&mut e, c, &p);
    ids.insert("pad", c);
    // The whole dry band (`master`: bass, bells, pad, arp) passes the bus cable; the echoes join
    // after it. A: two stabs a bar, the echoes ring between them. B: open.
    let c = e.connect(jack(master, "out"), jack(bus, "in1"));
    let mut a = [0.0; 16];
    a[0] = 1.0;
    a[8] = 0.5;
    let p = pattern(&a, &[1.0; 16], &[(9, 75.0)], &[], 5.0, 1.0);
    cable_params(&mut e, c, &p);
    ids.insert("bus", c);
    e.connect(jack(bus, "out"), jack(chorus, "in_l"));
    e.connect(jack(bus, "out"), jack(chorus, "in_r"));
    e.connect(jack(chorus, "left"), jack(verb, "in_l"));
    e.connect(jack(chorus, "right"), jack(verb, "in_r"));
    e.connect(jack(verb, "left"), jack(out, "left"));
    e.connect(jack(verb, "right"), jack(out, "right"));
    assert_eq!(ids.len(), CABLES.len());
    (e, ids)
}

/// The patch rendered through the runtime path, each morph moved by the timeline. Stereo,
/// interleaved.
fn render(state: &kabl_core::PatchState, ids: &HashMap<&'static str, CableId>) -> Vec<f32> {
    let mut c = compile(state, SR, 4).expect("piece compiles");
    let blocks = (BARS * BAR_S * SR) as usize / BLOCK;
    let mut out = Vec::with_capacity(blocks * BLOCK * 2);
    for b in 0..blocks {
        let bar = (b * BLOCK) as f32 / SR / BAR_S;
        for name in CABLES {
            assert!(c.set_runtime(
                kabl_engine::runtime::RuntimeTarget::Cable {
                    cable: ids[name],
                    slot: kabl_cables::MORPH as u8,
                },
                morph_at(name, bar),
                false,
            ));
        }
        c.process_block();
        for (l, r) in c.left().iter().zip(c.right()) {
            out.push(*l);
            out.push(*r);
        }
    }
    out
}

fn write_wav(path: &Path, samples: &[f32]) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for &s in samples {
        w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16)
            .unwrap();
    }
    w.finalize().unwrap();
}

/// The timeline for the drive-script generator: the cable ids, the bar length and the gestures.
fn timeline_json(ids: &HashMap<&'static str, CableId>) -> String {
    let cables: Vec<String> = CABLES
        .iter()
        .map(|n| {
            let kind = if *n == "filter" { "route" } else { "jack" };
            format!("    \"{n}\": {{\"id\": {}, \"kind\": \"{kind}\"}}", ids[n])
        })
        .collect();
    let gestures: Vec<String> = GESTURES
        .iter()
        .map(|t| {
            format!(
                "    {{\"cable\": \"{}\", \"from\": {}, \"to\": {}, \"start_bar\": {}, \"end_bar\": {}, \"what\": \"{}\"}}",
                t.cable, t.from, t.to, t.start, t.end, t.what
            )
        })
        .collect();
    format!(
        "{{\n  \"bpm\": {BPM},\n  \"bar_seconds\": {BAR_S},\n  \"bars\": {BARS},\n  \"cables\": {{\n{}\n  }},\n  \"gestures\": [\n{}\n  ]\n}}\n",
        cables.join(",\n"),
        gestures.join(",\n")
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (mut e, ids) = build();
    match args.get(1).map(String::as_str) {
        Some("write") => {
            kabl_core::save(Path::new(&args[2]), e.log()).expect("save");
            std::fs::write(&args[3], timeline_json(&ids)).expect("timeline");
            println!("wrote {} and {}", args[2], args[3]);
        }
        Some("render") => {
            let state = e.log().state().clone();
            let clip = render(&state, &ids);
            assert_eq!(clip, render(&state, &ids), "render is deterministic");
            write_wav(Path::new(&args[2]), &clip);
            let peak = clip.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            println!("peak {peak:.3}");
            // RMS per bar, to read the shape of the piece.
            let bar = (BAR_S * SR) as usize * 2;
            for (k, w) in clip.chunks(bar).enumerate() {
                let rms = (w.iter().map(|s| s * s).sum::<f32>() / w.len() as f32).sqrt();
                let peak = w.iter().fold(0.0f32, |m, s| m.max(s.abs()));
                println!("bar {k:2}: rms {rms:.3} peak {peak:.3}");
            }
        }
        Some("stems") => {
            // Each cable alone at B (the others at A, the bus open), four bars from bar 4 of the
            // chord cycle, against everything at A: its own contribution to the mix.
            let state = e.log().state().clone();
            let mut c = compile(&state, SR, 4).expect("piece compiles");
            let blocks = (8.0 * BAR_S * SR) as usize / BLOCK;
            let mut run = |b_on: &[&str]| {
                c.reset();
                let (mut sq, mut diff, mut prev, mut n) = (0.0f64, 0.0f64, 0.0f32, 0usize);
                for b in 0..blocks {
                    for name in CABLES {
                        let m = if b_on.contains(&name) || name == "bus" {
                            1.0
                        } else {
                            0.0
                        };
                        c.set_runtime(
                            kabl_engine::runtime::RuntimeTarget::Cable {
                                cable: ids[name],
                                slot: kabl_cables::MORPH as u8,
                            },
                            m,
                            false,
                        );
                    }
                    c.process_block();
                    if b * BLOCK > (4.0 * BAR_S * SR) as usize {
                        for (l, r) in c.left().iter().zip(c.right()) {
                            sq += (l * l + r * r) as f64 / 2.0;
                            diff += ((l - prev) * (l - prev)) as f64;
                            prev = *l;
                            n += 1;
                        }
                    }
                }
                ((sq / n as f64).sqrt(), (diff / n as f64).sqrt())
            };
            let show = |name: &str, (rms, bright): (f64, f64)| {
                println!("{name}: rms {rms:.3} brightness {bright:.3}")
            };
            show("all A", run(&[]));
            for name in CABLES.iter().filter(|n| **n != "bus") {
                show(&format!("{name} B"), run(&[name]));
            }
            show("bass B", run(&["bass"]));
            show("bass+filter B", run(&["bass", "filter"]));
            show("all B", run(&CABLES));
        }
        _ => eprintln!("usage: promo_demo write <patch dir> <timeline.json> | render <out.wav>"),
    }
}
