//! The factory composite `kabl.analog-voice` v1 (catalogue C1, roadmap CB5): the subtractive
//! keyboard voice behind most Bass, Lead, Pad and Voice presets. Two `osc.va` into a mixer
//! into a drive into a ladder filter whose cutoff its own envelope sweeps, into a VCA its amp
//! envelope opens, into a level trim. Pitch feeds both oscillators and Gate both envelopes
//! (fan-in, G3), and Env Amount is the envelope's route into the cutoff: a cable parameter (G1).
//!
//! Two oscillators: the largest family of the catalogue's core (25 of 52 patches), and the
//! count every one- and three-oscillator patch is closest to. A one-oscillator patch is this card
//! with Mix 2 at 0. Defaults follow the catalogue's modal values. See
//! `docs/composites/ANALOG-VOICE.md`.
//!
//! `cargo run -p kabl-tools --example analog_voice_composite -- write composites` writes
//! `composites/analog-voice/v1.json`. Running it again writes the same bytes: the package holds
//! the patch state only, never the log's wall-clock timestamps.
//!
//! `cargo run --release -p kabl-tools --example analog_voice_composite -- wav DIR` renders the
//! same eight-second phrase offline through Cinder Block's voice built flat from
//! `patches/presets/cinder-block` (`DIR/cinder-block-voice.wav`) and through the card set to
//! Cinder Block's values (`DIR/analog-voice-cinder-block.wav`), and the card's defaults at C4
//! gated by a clock at 120 BPM (`DIR/analog-voice.wav`). It prints peak and RMS of each.
//!
//! The captures are scenes of `chrome_capture`: `analog-voice-rack`, `analog-voice-face`,
//! `analog-voice-design`.
//!
//! The test `composites_analog_voice` includes this file to compare `bytes()` with the
//! committed file, so only the generator is compiled there.
use kabl_core::panel::{Kind, Panel, Placement};
use kabl_core::{Definition, PortRef, Vec2};
use kabl_ui::{composites, PatchEditor};
use std::collections::BTreeSet;

#[path = "composite_gen/mod.rs"]
mod composite_gen;
use composite_gen::{expose, jack, param};

/// The Env Amount route's stored depth: the corpus's most common non-zero `fenv_amt`.
pub(crate) const ENV_AMOUNT: f32 = 0.2;

fn element(kind: Kind, x: f32, y: f32, width: f32, height: f32) -> Placement {
    Placement {
        kind,
        x,
        y,
        width,
        height,
        source: None,
        label: None,
        icons: Vec::new(),
    }
}

fn package() -> composites::Package {
    let mut e = PatchEditor::new();
    let mut stage = |kind: &str, x: f32, y: f32, params: &[(&str, f32)]| {
        let id = e.add_module(kind, Vec2 { x, y });
        for &(name, v) in params {
            e.set_param(id, name, v);
        }
        id
    };
    let adsr = |[a, d, s, r]: [f32; 4]| {
        [
            ("attack_ms", a),
            ("decay_ms", d),
            ("sustain", s),
            ("release_ms", r),
        ]
    };
    // The catalogue's modal values, as the bass and lead builders (`Recipe` in
    // `presets_bass_lead.rs`) store them.
    let osc1 = stage(
        "osc.va",
        24.0,
        20.0,
        &[
            ("base_hz", 261.63),
            ("waveform", 2.0),
            ("unison", 1.0),
            ("detune", 15.0),
        ],
    );
    let osc2 = stage(
        "osc.va",
        24.0,
        420.0,
        &[("base_hz", 261.63), ("waveform", 2.0)],
    );
    let mix = stage("mixer", 214.0, 20.0, &[("level1", 1.0), ("level2", 0.3)]);
    let drive = stage("drive", 404.0, 20.0, &[("drive_db", 0.0)]);
    let lad = stage(
        "filter.ladder",
        594.0,
        20.0,
        &[("cutoff_hz", 3500.0), ("resonance", 0.1)],
    );
    let fenv = stage("env.adsr", 594.0, 420.0, &adsr([2.0, 300.0, 0.3, 100.0]));
    let aenv = stage("env.adsr", 784.0, 420.0, &adsr([2.0, 300.0, 0.6, 90.0]));
    let vca = stage("vca", 784.0, 20.0, &[("gain", 0.0)]);
    let level = stage("gain", 974.0, 20.0, &[("gain_db", -6.0)]);
    e.connect(jack(osc1, "out"), jack(mix, "in1"));
    e.connect(jack(osc2, "out"), jack(mix, "in2"));
    e.connect(jack(mix, "out"), jack(drive, "in"));
    e.connect(jack(drive, "out"), jack(lad, "in"));
    e.connect(jack(lad, "out"), jack(vca, "in"));
    e.connect(jack(aenv, "out"), jack(vca, "cv"));
    e.connect(jack(vca, "out"), jack(level, "in"));
    let fenv_cable = e.connect_route(jack(fenv, "out"), lad, "cutoff_hz");
    e.set_route_amount(fenv_cable, ENV_AMOUNT, true);
    let fan = |label: &str, a: PortRef, b: PortRef| {
        let mut x = expose(label, a);
        x.also.push(b);
        x
    };
    // Interface keys: ports 1..=8, then controls 9..=27, in the order listed.
    let ports = [
        fan("Pitch", jack(osc1, "pitch"), jack(osc2, "pitch")),
        fan("Gate", jack(fenv, "gate"), jack(aenv, "gate")),
        expose("Out", jack(level, "out")),
        expose(
            "Env Amount",
            PortRef::CableParam {
                cable: fenv_cable,
                param: "amount".into(),
            },
        ),
        expose("Cutoff mod", param(lad, "cutoff_hz")),
        expose("Level mod", param(level, "gain_db")),
        expose("Drive mod", param(drive, "drive_db")),
        expose("Detune mod", param(osc1, "detune")),
    ];
    let controls = [
        expose("Wave 1", param(osc1, "waveform")),
        expose("Octave 1", param(osc1, "base_hz")),
        expose("Unison", param(osc1, "unison")),
        expose("Detune", param(osc1, "detune")),
        expose("Wave 2", param(osc2, "waveform")),
        expose("Octave 2", param(osc2, "base_hz")),
        expose("Mix 2", param(mix, "level2")),
        expose("Drive", param(drive, "drive_db")),
        expose("Cutoff", param(lad, "cutoff_hz")),
        expose("Resonance", param(lad, "resonance")),
        expose("Filter Attack", param(fenv, "attack_ms")),
        expose("Filter Decay", param(fenv, "decay_ms")),
        expose("Filter Sustain", param(fenv, "sustain")),
        expose("Filter Release", param(fenv, "release_ms")),
        expose("Amp Attack", param(aenv, "attack_ms")),
        expose("Amp Decay", param(aenv, "decay_ms")),
        expose("Amp Sustain", param(aenv, "sustain")),
        expose("Amp Release", param(aenv, "release_ms")),
        expose("Level", param(level, "gain_db")),
    ];
    // Two rows of knobs in signal order. The top row is the oscillators (Wave 1 over Wave 2 as
    // glyphs, then Octave 1, Octave 2 and Mix 2), Drive, the filter (Cutoff, Resonance) and the
    // filter envelope; the bottom row is the jacks, Level, and the amp envelope right under the
    // filter's, so the two envelopes read as a pair. Unison and Detune are off the face, under
    // "More controls": Unison's sixteen steps need a selector wider than the face has room for,
    // and Detune only spreads unison voices. There is no room for displays at the face's fixed
    // height. Knobs, not sliders: a face slider follows the player's orientation setting. Knob
    // and jack widths fit their longest label, "Filter Release" and "IN Detune mod", at the
    // face's 12-point type.
    let mut panel = Panel {
        width: 840.0,
        ..Panel::default()
    };
    let mut put = |key: u64, kind: Kind, x: f32, y: f32, w: f32| {
        let h = match kind {
            Kind::Knob => 110.0,
            Kind::Jack => 58.0,
            _ => 55.0,
        };
        panel.placements.insert(key, element(kind, x, y, w, h));
    };
    put(9, Kind::Icons, 8.0, 64.0, 72.0);
    put(13, Kind::Icons, 8.0, 121.0, 72.0);
    // Octave 1 Octave 2 Mix 2 | Drive | Cutoff Resonance | Filter Attack .. Release.
    for (key, x) in [
        (10, 86.0),
        (14, 158.0),
        (15, 230.0),
        (16, 304.0),
        (17, 378.0),
        (18, 450.0),
    ] {
        put(key, Kind::Knob, x, 64.0, 70.0);
    }
    // Level, under Cutoff | Amp Attack .. Release.
    put(27, Kind::Knob, 378.0, 186.0, 70.0);
    for (y, first) in [(64.0, 19), (186.0, 23)] {
        for (i, (x, w)) in [(526.0, 70.0), (598.0, 70.0), (670.0, 78.0), (750.0, 80.0)]
            .into_iter()
            .enumerate()
        {
            put(first + i as u64, Kind::Knob, x, y, w);
        }
    }
    // Env Amount Cutoff mod Drive mod Detune mod, over Pitch Gate Level mod Out: the jacks a
    // keyboard and the output patch to along the face's bottom edge, so their cables leave
    // under the face instead of across the other jacks.
    for (i, key) in [4, 5, 7, 8, 1, 2, 6, 3].into_iter().enumerate() {
        let (col, row) = (i % 4, i / 4);
        put(
            key,
            Kind::Jack,
            8.0 + 92.0 * col as f32,
            186.0 + 62.0 * row as f32,
            88.0,
        );
    }
    let mut log = e.log().clone();
    let id = composites::encapsulate_with(
        &mut log,
        "Analog Voice",
        Definition {
            id: "kabl.analog-voice".into(),
            version: 1,
        },
        BTreeSet::from([osc1, osc2, mix, drive, lad, fenv, aenv, vca, level]),
        &ports,
        &controls,
        Some(panel),
    )
    .expect("Analog Voice wraps");
    let e = PatchEditor::from_log(log);
    composites::package(e.state(), id).expect("Analog Voice packages")
}

/// `composites/analog-voice/v1.json` exactly as committed.
pub(crate) fn bytes() -> Vec<u8> {
    serde_json::to_vec_pretty(&package()).expect("serialise")
}

/// Cinder Block's voice two ways, each into an output: `flat` is the preset
/// (`patches/presets/cinder-block` under `patches`) with its effects after the trim removed,
/// `card` is `pkg` (this definition) with every leaf set to the preset's values and the same
/// `midi.in`, macros and routes patched to its ports. Hold's three routes into the amp
/// envelope are left out of both: the catalogue gives the card no amp envelope input.
pub(crate) struct CinderBlock {
    pub(crate) flat: PatchEditor,
    pub(crate) card: PatchEditor,
}

pub(crate) fn cinder_block(pkg: &composites::Package, patches: &std::path::Path) -> CinderBlock {
    use kabl_core::ModuleId;
    use std::collections::BTreeMap;
    let log = kabl_core::load(&patches.join("presets/cinder-block")).expect("cinder-block");
    let mut flat = PatchEditor::from_log(log);
    let kind_of = |e: &PatchEditor, id: ModuleId| e.state().modules[&id].kind.clone();
    let of_kind = |e: &PatchEditor, kind: &str| -> Vec<ModuleId> {
        let p = e.state();
        p.modules
            .iter()
            .filter(|(_, m)| m.kind == kind)
            .map(|(&id, _)| id)
            .collect()
    };
    for kind in ["bitcrusher", "compressor", "delay", "reverb", "split"] {
        for id in of_kind(&flat, kind) {
            flat.remove_module(id);
        }
    }
    let [macros] = of_kind(&flat, "macro")[..] else {
        panic!("one macro module")
    };
    let hold: Vec<_> = flat
        .state()
        .cables
        .iter()
        .filter(|(_, c)| c.from == jack(macros, "m2"))
        .map(|(&id, _)| id)
        .collect();
    for id in hold {
        flat.disconnect(id);
    }
    let [trim] = of_kind(&flat, "gain")[..] else {
        panic!("one gain")
    };
    let [out] = of_kind(&flat, "out")[..] else {
        panic!("one output")
    };
    flat.connect(jack(trim, "out"), jack(out, "in"));

    let mut card = PatchEditor::new();
    let id = composites::insert(&mut card, pkg).expect("insert Analog Voice");
    let c = card.state().composites[&id].clone();
    let targets = |label: &str| -> Vec<PortRef> {
        c.ports
            .values()
            .find(|x| x.label == label)
            .unwrap_or_else(|| panic!("no port {label}"))
            .targets()
            .cloned()
            .collect()
    };
    let leaf = |t: &PortRef| t.module_id().expect("a module end");
    let (pitch, gate) = (targets("Pitch"), targets("Gate"));
    let PortRef::CableParam {
        cable: env_route, ..
    } = targets("Env Amount")[0]
    else {
        panic!("Env Amount is a cable parameter")
    };
    // The preset's module ids, by role, to the card's leaves and the new outside modules.
    let p = flat.state();
    let feeding = |port: &PortRef| {
        p.cables
            .values()
            .find(|x| &x.to == port)
            .and_then(|x| x.from.module_id())
            .expect("a cable into the port")
    };
    let [mix] = of_kind(&flat, "mixer")[..] else {
        panic!("one mixer")
    };
    let [vca] = of_kind(&flat, "vca")[..] else {
        panic!("one vca")
    };
    let aenv = feeding(&jack(vca, "cv"));
    let fenv = *of_kind(&flat, "env.adsr")
        .iter()
        .find(|&&m| m != aenv)
        .expect("a filter envelope");
    let card_leaf = |kind: &str| {
        *c.members
            .iter()
            .find(|&&m| card.state().modules[&m].kind == kind)
            .expect(kind)
    };
    let mut map: BTreeMap<ModuleId, ModuleId> = BTreeMap::from([
        (feeding(&jack(mix, "in1")), leaf(&pitch[0])),
        (feeding(&jack(mix, "in2")), leaf(&pitch[1])),
        (fenv, leaf(&gate[0])),
        (aenv, leaf(&gate[1])),
    ]);
    for kind in ["mixer", "drive", "filter.ladder", "vca", "gain"] {
        map.insert(of_kind(&flat, kind)[0], card_leaf(kind));
    }
    for (&from, &to) in &map {
        assert_eq!(kind_of(&flat, from), kind_of(&card, to));
        for (name, &v) in &p.modules[&from].params {
            card.set_param(to, name, v);
        }
    }
    let inner = map.clone();
    for kind in ["midi.in", "macro", "out"] {
        let [old] = of_kind(&flat, kind)[..] else {
            panic!("one {kind}")
        };
        let new = card.add_module(kind, p.modules[&old].pos);
        for (name, &v) in &p.modules[&old].params {
            card.set_param(new, name, v);
        }
        map.insert(old, new);
    }
    // Every cable that enters or leaves the voice, onto the card's leaves; the envelope's
    // route keeps the preset's depth.
    let mut env_cable = false;
    for x in p.cables.values() {
        let end = |r: &PortRef| match r {
            PortRef::Module { id, port } => jack(map[id], port),
            PortRef::Param { id, param: n } => param(map[id], n),
            PortRef::CableParam { param: n, .. } => PortRef::CableParam {
                cable: env_route,
                param: n.clone(),
            },
        };
        let inside =
            |r: &PortRef| kabl_core::composite::owner(p, r).is_some_and(|m| inner.contains_key(&m));
        let amount = x.params.get("amount").copied();
        if inside(&x.from) && inside(&x.to) {
            if matches!(x.to, PortRef::Param { .. }) {
                env_cable = true;
                card.set_route_amount(env_route, amount.unwrap_or(0.0), true);
            }
            continue;
        }
        let new = card.connect(end(&x.from), end(&x.to));
        if let Some(a) = amount {
            card.set_route_amount(new, a, true);
        }
    }
    assert!(env_cable, "the preset has an envelope route");
    CinderBlock { flat, card }
}

#[cfg(not(test))]
mod tool {
    use super::*;
    use std::path::Path;

    fn write(root: &Path) {
        let dir = root.join("analog-voice");
        std::fs::create_dir_all(&dir).expect("bank folder");
        let bytes = bytes();
        composites::decode(&bytes).expect("the package decodes");
        let path = dir.join("v1.json");
        std::fs::write(&path, bytes).expect("write");
        println!("wrote {}", path.display());
    }

    /// A two-bar bass line in eighths at 125 BPM, played twice, in eight seconds.
    fn phrase(patch: &kabl_core::PatchState, path: &Path) {
        const LINE: [f32; 16] = [
            -12.0, -12.0, 0.0, -12.0, -9.0, -12.0, -5.0, -7.0, -12.0, -12.0, 0.0, -12.0, -5.0,
            -7.0, -9.0, -10.0,
        ];
        // 0.24 s per eighth, gated for 0.18 s, at 48 kHz; the engine's 64-frame blocks divide
        // both.
        const STEP: usize = 11520;
        const HELD: usize = 8640;
        composite_gen::render_with(patch, path, |c, frame| {
            let (step, at) = (frame / STEP, frame % STEP);
            if at == 0 {
                c.note_on(0, LINE[step % LINE.len()], 0.8);
            } else if at == HELD {
                c.note_off(0);
            }
        });
    }

    pub(super) fn main() {
        let a: Vec<String> = std::env::args().skip(1).collect();
        let dir = std::path::PathBuf::from(a.get(1).cloned().unwrap_or_else(|| ".".into()));
        match a.first().map(String::as_str) {
            Some("write") => write(&dir),
            Some("wav") => {
                let patches = Path::new(env!("CARGO_MANIFEST_DIR")).join("../patches");
                // The defaults, C4 on the clock's gate, for comparison.
                composite_gen::wav(&dir, "analog-voice");
                let cinder = cinder_block(&package(), &patches);
                phrase(cinder.flat.state(), &dir.join("cinder-block-voice.wav"));
                phrase(
                    cinder.card.state(),
                    &dir.join("analog-voice-cinder-block.wav"),
                );
            }
            _ => eprintln!("usage: analog_voice_composite write|wav DIR"),
        }
    }
}

#[cfg(not(test))]
fn main() {
    tool::main();
}
