//! The factory composite scenes of `chrome_capture`: each card of the bank in a small rack.
use kabl_ui::PatchEditor;
use std::path::PathBuf;

/// A clock at 120 BPM into the factory composite `kabl.<name>` v1 (Drum Kit, a Sequenced
/// Part) into the output: the clock left of the card and the output under or beside it, so no
/// cable crosses a face. Glue Bus has no clock: the clock plays Drum Kit, and Glue Bus sits under
/// it with In and Duck In along its top, fed by Drum Kit's Out and Kick. Analog Voice is a voice:
/// a midi.in left of it plays Pitch and Gate, and the output sits under the midi.in, next to the
/// Out jack at the face's left edge.
pub(crate) fn factory_patch(name: &str) -> PatchEditor {
    use kabl_ui::composites;
    let mut e = PatchEditor::new();
    let voice = name == "analog-voice";
    let source = if voice {
        e.add_module("midi.in", kabl_core::Vec2 { x: 24.0, y: 20.0 })
    } else {
        let clock = e.add_module("clock", kabl_core::Vec2 { x: 24.0, y: 20.0 });
        e.set_param(clock, "bpm", 120.0);
        clock
    };
    // Sequenced Part's Out jack is above the output's face in the second row, so there the
    // output sits right of the card and its cable hangs under the card instead.
    let out_pos = match name {
        "drum-kit" => kabl_core::Vec2 { x: 210.0, y: 420.0 },
        "analog-voice" => kabl_core::Vec2 { x: 24.0, y: 420.0 },
        _ => kabl_core::Vec2 { x: 1060.0, y: 20.0 },
    };
    let out = e.add_module("out", out_pos);
    let insert = |e: &mut PatchEditor, name: &str, pos| {
        let bank = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../composites");
        let def = format!("kabl.{name}");
        let entry = composites::factory_entries(&bank)
            .into_iter()
            .find(|f| f.definition.id == def && f.definition.version == 1)
            .expect("the definition is in the bank");
        let pkg = composites::read_factory(&entry).expect("read the definition");
        let id = composites::insert(e, &pkg).expect("insert the composite");
        let mut c = e.state().composites[&id].clone();
        c.pos = pos;
        composites::set(e, id, c).expect("move the composite");
        id
    };
    let glue = name == "glue-bus";
    let card = if glue { "drum-kit" } else { name };
    let id = insert(&mut e, card, kabl_core::Vec2 { x: 210.0, y: 20.0 });
    let bus = glue.then(|| insert(&mut e, name, GLUE_BUS_POS));
    let port_of = |e: &PatchEditor, id, label: &str| {
        e.state().composites[&id]
            .ports
            .values()
            .find(|x| x.label == label)
            .expect("a port of the composite")
            .target
            .clone()
    };
    let port = |e: &PatchEditor, label: &str| port_of(e, id, label);
    let jack = |id, port: &str| kabl_core::PortRef::Module {
        id,
        port: port.into(),
    };
    let mut mix = port(&e, "Out");
    if voice {
        let (pitch, gate) = (port(&e, "Pitch"), port(&e, "Gate"));
        composites::connect(&mut e, &Default::default(), jack(source, "pitch"), pitch);
        composites::connect(&mut e, &Default::default(), jack(source, "gate"), gate);
    } else {
        let (gate, reset) = (port(&e, "Clock"), port(&e, "Reset"));
        e.connect(jack(source, "gate"), gate);
        e.connect(jack(source, "reset"), reset);
    }
    if let Some(bus) = bus {
        e.connect(mix, port_of(&e, bus, "In"));
        e.connect(port(&e, "Kick"), port_of(&e, bus, "Duck In"));
        mix = port_of(&e, bus, "Out");
    }
    e.connect(mix, jack(out, "in"));
    e
}

/// Glue Bus under Drum Kit, its In and Duck In jacks 90 points right of Drum Kit's Out and Kick:
/// a cable hanging straight down would cross the jack's label.
const GLUE_BUS_POS: kabl_core::Vec2 = kabl_core::Vec2 { x: 300.0, y: 420.0 };
