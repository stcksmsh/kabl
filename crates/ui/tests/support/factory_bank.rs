use kabl_core::{ModuleId, PortRef, Vec2};
use kabl_modules::builtins::seq::bank_param;
use kabl_ui::{banks, library::Meta, PatchEditor};
use std::path::Path;

pub const INVENTORY: &[(&str, &str, &str, &str)] = &[
    ("palette/keyboard-bass", "bass", "mono dark keyboard", "Play C2-C3, single notes. Tone opens the filter; Weight adds resonance; Shape lengthens decay; Room adds a short space. Bend +/-2 semitones. CC20-23 control macros."),
    ("palette/lead", "lead", "mono legato expressive", "Overlap E4-G4-B4 for legato glide. Tone brightens, Vibrato adds motion, Bite adds drive, Echo adds repeats. Bend +/-2 semitones; CC20-23 control macros."),
    ("palette/keys", "pluck keys", "poly bright keyboard", "Play E3-B3-E4-G4, then release. Tone brightens; Weight emphasizes the attack; Shape lengthens decay; Room adds ambience. Velocity shapes volume; bend +/-2 semitones. CC20-23 control macros."),
    ("palette/pad", "pad", "poly slow atmospheric", "Hold E3-B3-E4-G4 for at least four seconds. Warmth opens the filter, Evolve deepens motion, Air adds breath, Space adds echo and room. Leave time for the release; CC20-23 control macros."),
    ("palette/strings", "strings", "poly warm ensemble", "Hold E3-G3-B3-E4; overlap the next chord. Bright opens the tone, Ensemble widens detuning, Bow softens attack, Space adds room. Velocity shapes volume; CC20-23 control macros."),
    ("palette/breath", "texture", "poly airy noise", "Hold one low note or a wide chord. Breath adds noise, Gust sets motion, Color changes the breath band, Space adds room. Use sparse notes and let the tail decay; CC20-23 control macros."),
    ("palette/bass", "bass pulse", "112bpm e minor resonant", "Open stopped. Run starts the E-minor pulse at 112 BPM. Bass banks: A Line, B Lift, C Hold, D Rest; launch on the next bar. Cutoff, Resonance, Accent and Drive shape the pulse. Stop releases the voice."),
    ("interlocking", "interlocking arpeggio", "116bpm e minor polyrhythm", "Run starts 7-step bass against 5-step arp at 116 BPM in E minor. Launch Pulse, Lift, Sparse or Rest on either bank card. Glow opens tone, Motion speeds the drift, Length sustains notes, Air sharpens the arp."),
    ("echo", "probabilistic motion", "116bpm e minor probability echo", "Run starts probabilistic E-minor motion at 116 BPM. Pulse and Lift keep the first step certain; Sparse uses 45% ghost notes; Rest mutes. Launch both bank cards for sections. Glow, Motion, Length and Echo shape variation."),
    ("palette/progression", "atmospheric progression", "72bpm e minor slow evolving", "Run starts slow minor harmony at 72 BPM, one chord every two beats. Banks: Home, Lift, Descent, Rest; launch on the next bar. Glow opens tone, Motion speeds drift, Bloom lengthens release, Space adds room. Stop leaves a soft tail."),
    ("composition", "performance", "112bpm e minor evolving sections", "Open stopped. Run, then launch Intro, Main, Variation, Breakdown or Return cues on the next bar. Energy, Motion, Space and Glow shape the three layers. Play a single E4-B4 lead over them. CC40-44 launch cues; CC46 toggles Run."),
    ("sound-palette", "performance", "104bpm e minor layered sections", "Open stopped in Rest at 104 BPM. Run, then launch Pulse, Groove or Lift for bass and percussion; Air and Break are sparse. Play E3-B3-E4-G4 for keyboard layers; raise Strings, Pad or Lead faders to add voices. Energy, Motion, Bright and Space vary sections. Stop releases sequences; All notes off clears held keys."),
];

fn at(x: f32, row: usize) -> Vec2 {
    Vec2 {
        x,
        y: kabl_ui::rack::row_y(row),
    }
}
fn port(id: ModuleId, name: &str) -> PortRef {
    PortRef::Module {
        id,
        port: name.into(),
    }
}
fn set(e: &mut PatchEditor, id: ModuleId, values: &[(&str, f32)]) {
    for (key, value) in values {
        e.set_param(id, key, *value);
    }
}
fn pin(e: &mut PatchEditor, id: ModuleId, key: &str, order: f32, label: &str) {
    e.set_presentation(&[(id, format!("pin.{key}"), Some(order))]);
    e.set_label(id, &format!("pin.{key}"), Some(label.into()));
}
fn route(e: &mut PatchEditor, src: ModuleId, key: &str, dest: ModuleId, param: &str, amount: f32) {
    let cable = e.connect_route(port(src, key), dest, param);
    e.set_route_amount(cable, amount, true);
}
fn macros(e: &mut PatchEditor, names: [&str; 4], row: usize) -> ModuleId {
    let m = e.add_module("macro", at(24.0, row));
    for (i, name) in names.iter().enumerate() {
        let key = format!("m{}", i + 1);
        e.set_param(m, &key, 0.3);
        e.set_label(m, &format!("name.{key}"), Some((*name).into()));
        pin(e, m, &key, (i + 1) as f32, name);
        e.set_presentation(&[(m, format!("cc.{key}"), Some((20 + i) as f32))]);
    }
    m
}

pub fn keyboard(bass: bool) -> PatchEditor {
    let mut e = kabl_ui::recipes::pluck_to_pad();
    let old_pins: Vec<_> = kabl_ui::perform::pins(e.state())
        .into_iter()
        .map(|p| (p.id, format!("pin.{}", p.key), None))
        .collect();
    e.set_presentation(&old_pins);
    set(
        &mut e,
        1,
        &[("mode", if bass { 1.0 } else { 0.0 }), ("bend", 2.0)],
    );
    set(&mut e, 2, &[("waveform", if bass { 3.0 } else { 1.0 })]);
    set(
        &mut e,
        3,
        &[
            ("cutoff_hz", if bass { 480.0 } else { 3200.0 }),
            ("resonance", 0.18),
        ],
    );
    set(
        &mut e,
        6,
        &[
            ("attack_ms", 3.0),
            ("decay_ms", if bass { 200.0 } else { 360.0 }),
            ("sustain", if bass { 0.55 } else { 0.0 }),
            ("release_ms", if bass { 90.0 } else { 240.0 }),
        ],
    );
    let vel = e.add_module("ringmod", at(24.0, 2));
    let reverb = e.add_module("reverb", at(200.0, 2));
    let gain = e.add_module("gain", at(450.0, 2));
    set(
        &mut e,
        reverb,
        &[("decay_s", if bass { 0.6 } else { 1.5 }), ("mix", 5.0)],
    );
    set(&mut e, gain, &[("gain_db", if bass { 7.0 } else { 3.0 })]);
    let remove: Vec<_> = e
        .state()
        .cables
        .iter()
        .filter(|(_, c)| {
            c.to == port(4, "cv") || c.to == port(5, "left") || c.to == port(5, "right")
        })
        .map(|(&id, _)| id)
        .collect();
    for id in remove {
        e.disconnect(id);
    }
    for (a, ap, b, bp) in [
        (6, "out", vel, "a"),
        (1, "velocity", vel, "b"),
        (vel, "out", 4, "cv"),
        (4, "out", gain, "in"),
        (gain, "out", reverb, "in_l"),
        (gain, "out", reverb, "in_r"),
        (reverb, "left", 5, "left"),
        (reverb, "right", 5, "right"),
    ] {
        e.connect(port(a, ap), port(b, bp));
    }
    let m = macros(&mut e, ["Tone", "Weight", "Shape", "Room"], 3);
    route(&mut e, m, "m1", 3, "cutoff_hz", 0.22);
    route(&mut e, m, "m2", 3, "resonance", 0.2);
    route(&mut e, m, "m3", 6, "decay_ms", 0.1);
    route(&mut e, m, "m4", reverb, "mix", 0.2);
    pin(&mut e, 6, "release_ms", 5.0, "Release");
    pin(&mut e, gain, "gain_db", 6.0, "Level");
    e
}

pub fn progression() -> PatchEditor {
    let mut e = PatchEditor::new();
    let clock = e.add_module("clock", at(24.0, 0));
    let div = e.add_module("clock.div", at(210.0, 0));
    let seq = e.add_module("seq", at(500.0, 0));
    let mix = e.add_module("mixer", at(24.0, 2));
    let filt = e.add_module("filter.ladder", at(260.0, 2));
    let env = e.add_module("env.adsr", at(500.0, 2));
    let vca = e.add_module("vca", at(760.0, 2));
    let reverb = e.add_module("reverb", at(24.0, 3));
    let out = e.add_module("out", at(260.0, 3));
    let lfo = e.add_module("lfo", at(450.0, 3));
    set(&mut e, clock, &[("bpm", 72.0)]);
    set(&mut e, div, &[("div", 8.0)]);
    set(
        &mut e,
        mix,
        &[("level1", 0.13), ("level2", 0.11), ("level3", 0.1)],
    );
    set(&mut e, filt, &[("cutoff_hz", 1000.0), ("resonance", 0.15)]);
    set(
        &mut e,
        env,
        &[
            ("attack_ms", 900.0),
            ("decay_ms", 1400.0),
            ("sustain", 0.7),
            ("release_ms", 2000.0),
        ],
    );
    set(&mut e, vca, &[("gain", 0.0)]);
    set(
        &mut e,
        reverb,
        &[("mix", 22.0), ("decay_s", 3.5), ("damp_hz", 4500.0)],
    );
    set(&mut e, lfo, &[("rate_hz", 0.07), ("waveform", 1.0)]);
    for (i, hz) in [130.81, 155.56, 196.0].iter().enumerate() {
        let osc = e.add_module("osc.va", at(24.0 + i as f32 * 230.0, 1));
        set(
            &mut e,
            osc,
            &[
                ("base_hz", *hz),
                ("waveform", 2.0),
                ("unison", 2.0),
                ("detune", 12.0),
            ],
        );
        e.connect(port(seq, "pitch"), port(osc, "pitch"));
        e.connect(port(osc, "out"), port(mix, &format!("in{}", i + 1)));
    }
    for (a, ap, b, bp) in [
        (clock, "gate", div, "clock"),
        (clock, "reset", div, "reset"),
        (clock, "reset", seq, "reset"),
        (div, "gate", seq, "clock"),
        (seq, "gate", env, "gate"),
        (mix, "out", filt, "in"),
        (filt, "out", vca, "in"),
        (env, "out", vca, "cv"),
        (vca, "out", reverb, "in_l"),
        (vca, "out", reverb, "in_r"),
        (reverb, "left", out, "left"),
        (reverb, "right", out, "right"),
    ] {
        e.connect(port(a, ap), port(b, bp));
    }
    for (b, pitches) in [
        [4.0, 0.0, 7.0, 2.0],
        [4.0, 7.0, 11.0, 9.0],
        [4.0, 2.0, 0.0, -3.0],
        [0.0; 4],
    ]
    .iter()
    .enumerate()
    {
        banks::rename(&mut e, seq, b, ["Home", "Lift", "Descent", "Rest"][b]);
        e.set_param(seq, bank_param(b, 16), 4.0);
        e.set_param(seq, bank_param(b, 26), 1.0);
        e.set_param(seq, bank_param(b, 25), 80.0);
        for (k, pitch) in pitches.iter().enumerate() {
            e.set_param(seq, bank_param(b, k), *pitch);
            e.set_param(seq, bank_param(b, 8 + k), if b == 3 { 0.0 } else { 1.0 });
        }
    }
    let m = macros(&mut e, ["Glow", "Motion", "Bloom", "Space"], 4);
    route(&mut e, m, "m1", filt, "cutoff_hz", 0.22);
    route(&mut e, m, "m2", lfo, "rate_hz", 0.01);
    route(&mut e, lfo, "out", filt, "cutoff_hz", 0.08);
    route(&mut e, m, "m3", env, "release_ms", 0.08);
    route(&mut e, m, "m4", reverb, "mix", 0.25);
    pin(&mut e, clock, "transport", 0.0, "Transport");
    pin(&mut e, seq, "banks", 5.0, "Harmony");
    pin(&mut e, clock, "bpm", 6.0, "Tempo");
    e
}

pub fn write_metadata(root: &Path) {
    for (dir, role, tags, guide) in INVENTORY {
        let path = root.join(dir).join("sound.toml");
        let mut meta: Meta = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        meta.description = (*guide).into();
        if let Some(tempo) = tags.split_whitespace().find(|tag| tag.ends_with("bpm")) {
            meta.tags.retain(|tag| !tag.ends_with("bpm") || tag == tempo);
        }
        for tag in std::iter::once("curated")
            .chain(std::iter::once(*role))
            .chain(tags.split_whitespace())
        {
            if !meta.tags.iter().any(|t| t == tag) {
                meta.tags.push(tag.into());
            }
        }
        std::fs::write(path, toml::to_string_pretty(&meta).unwrap()).unwrap();
    }
}

pub fn attach_guide(e: &mut PatchEditor, dir: &str) {
    if let Some((_, _, _, guide)) = INVENTORY.iter().find(|entry| entry.0 == dir) {
        let id = e
            .state()
            .modules
            .iter()
            .find(|(_, m)| m.kind == "macro")
            .map(|(&id, _)| id)
            .unwrap();
        e.set_label(id, "guide", Some((*guide).into()));
    }
}
