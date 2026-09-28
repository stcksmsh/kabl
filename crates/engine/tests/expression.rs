//! D06 expressive MIDI (docs/midi-timing/design.md, section 7), through `PatchEngine`: bend
//! center/endpoints/range and held-note response, the wheel as a modulation output, channel
//! routing between two keyboards, releases by ownership after a routing change, deletion and
//! replacement, source and channel scoped cleanup, reset controllers, per-channel sustain,
//! identities across channels and sources, stealing, and the midi.in change bound.

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::Collector;
use kabl_core::{CableState, ModuleId, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::{bend_norm, KeyEvent, MidiEvent, Source};
use kabl_engine::patch_engine::PatchEngine;
use kabl_modules::builtins::{Change, MidiIn};
use kabl_modules::{Module, ProcessIo, Signal};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;

fn port(id: ModuleId, p: &str) -> PortRef {
    PortRef::Module { id, port: p.into() }
}

fn module(kind: &str, params: &[(&str, f32)]) -> ModuleState {
    ModuleState {
        kind: kind.into(),
        pos: Vec2::default(),
        params: params.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
    }
}

fn cable(p: &mut PatchState, from: PortRef, to: PortRef) {
    let id = p.cables.len() as u64 + 1;
    p.cables.insert(
        id,
        CableState {
            from,
            to,
            params: Default::default(),
            steps: vec![],
        },
    );
}

/// Keyboard 1: `out1` to the left output; keyboard 2 (when given): `out2` to the right.
fn two(a: &[(&str, f32)], out1: &str, b: Option<&[(&str, f32)]>, out2: &str) -> PatchState {
    let mut p = PatchState::new();
    p.modules.insert(1, module("midi.in", a));
    p.modules.insert(9, module("out", &[]));
    cable(&mut p, port(1, out1), port(9, "left"));
    if let Some(b) = b {
        p.modules.insert(2, module("midi.in", b));
        cable(&mut p, port(2, out2), port(9, "right"));
    }
    p
}

fn engine(c: &Collector, p: &PatchState, voices: usize) -> PatchEngine {
    PatchEngine::new(&c.handle(), p, SR, voices).unwrap()
}

fn block(e: &mut PatchEngine) -> ([f32; BLOCK], [f32; BLOCK]) {
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    e.process_block(&mut l, &mut r);
    (l, r)
}

fn at(e: &mut PatchEngine, ch: u8, ev: KeyEvent, offset: usize) {
    e.key_at(MidiEvent::new(Source::Controller, ch, ev), offset);
}

fn on(note: u8) -> KeyEvent {
    KeyEvent::On {
        note,
        velocity: 100,
    }
}

#[test]
fn bend_center_and_endpoints_are_exact() {
    assert_eq!(bend_norm(8192), 0.0);
    assert_eq!(bend_norm(0), -1.0);
    assert_eq!(bend_norm(16383), 1.0);
    assert!(bend_norm(8191) < 0.0 && bend_norm(8193) > 0.0);
    // Parsing: LSB first, 14 bit; velocity 0 is a note off; CC 1 the wheel.
    let p = |d: &[u8]| MidiEvent::parse(Source::Controller, d).map(|e| (e.channel, e.event));
    assert_eq!(p(&[0xE3, 0x00, 0x40]), Some((3, KeyEvent::Bend(8192))));
    assert_eq!(p(&[0xE0, 0x7F, 0x7F]), Some((0, KeyEvent::Bend(16383))));
    assert_eq!(p(&[0x95, 60, 0]), Some((5, KeyEvent::Off { note: 60 })));
    assert_eq!(p(&[0xB1, 1, 64]), Some((1, KeyEvent::Wheel(64))));
    assert_eq!(p(&[0xB0, 121, 0]), Some((0, KeyEvent::ResetControllers)));
    assert_eq!(p(&[0xB0, 123, 0]), Some((0, KeyEvent::NotesOff)));
    assert_eq!(
        p(&[0xB0, 7, 100]),
        None,
        "other CCs stay with learn and mappings"
    );
    assert_eq!(KeyEvent::from_midi(&[0xB0, 123, 0]), Some(KeyEvent::AllOff));
}

#[test]
fn bend_moves_held_notes_by_the_range_from_its_own_sample() {
    let c = Collector::new();
    let mut e = engine(&c, &two(&[("bend", 12.0)], "pitch", None, ""), 1);
    at(&mut e, 0, on(60), 0);
    block(&mut e);
    at(&mut e, 0, KeyEvent::Bend(16383), 10);
    let (l, _) = block(&mut e);
    assert_eq!(l[9], 0.0);
    assert_eq!(l[10], 12.0, "full bend up = +range, at its sample");
    at(&mut e, 0, KeyEvent::Bend(0), 0);
    assert_eq!(block(&mut e).0[0], -12.0);
    at(&mut e, 0, KeyEvent::Bend(8192), 0);
    assert_eq!(block(&mut e).0[0], 0.0, "center is exactly no bend");
    // Default range: 2 semitones. A released (tail) voice follows the bend too.
    let mut e = engine(&c, &two(&[], "pitch", None, ""), 1);
    at(&mut e, 0, on(67), 0);
    at(&mut e, 0, KeyEvent::Off { note: 67 }, 1);
    at(&mut e, 0, KeyEvent::Bend(16383), 2);
    let (l, _) = block(&mut e);
    assert_eq!(l[2], 9.0);
}

#[test]
fn bend_adds_after_a_mono_glide_and_the_glide_ignores_it() {
    let c = Collector::new();
    let p = [
        ("mode", 2.0),
        ("glide", 1.0),
        ("glide_ms", 10.0),
        ("bend", 2.0),
    ];
    let mut e = engine(&c, &two(&p, "pitch", None, ""), 1);
    at(&mut e, 0, on(60), 0);
    at(&mut e, 0, KeyEvent::Bend(16383), 0);
    block(&mut e);
    at(&mut e, 0, on(72), 0);
    for _ in 0..20 {
        block(&mut e);
    }
    assert_eq!(block(&mut e).0[BLOCK - 1], 14.0);
    assert_eq!(e.expression(1), Some((1.0, 0.0)));
}

#[test]
fn the_wheel_is_one_value_on_every_voice_even_with_no_note() {
    let c = Collector::new();
    let mut e = engine(&c, &two(&[], "wheel", None, ""), 8);
    at(&mut e, 0, KeyEvent::Wheel(127), 20);
    let (l, _) = block(&mut e);
    assert_eq!(l[19], 0.0);
    assert_eq!(l[20], 1.0, "the voice average is the wheel, not 1/8 of it");
    at(&mut e, 0, KeyEvent::Wheel(0), 0);
    assert_eq!(block(&mut e).0[0], 0.0);
}

#[test]
fn the_wheel_routed_to_a_cutoff_changes_the_sound() {
    let c = Collector::new();
    let mut p = PatchState::new();
    p.modules.insert(1, module("midi.in", &[]));
    p.modules.insert(2, module("osc.va", &[("waveform", 2.0)]));
    p.modules
        .insert(3, module("filter.svf", &[("cutoff_hz", 200.0)]));
    p.modules.insert(4, module("out", &[]));
    cable(&mut p, port(1, "pitch"), port(2, "pitch"));
    cable(&mut p, port(2, "out"), port(3, "in"));
    cable(&mut p, port(3, "lp"), port(4, "left"));
    let id = p.cables.len() as u64 + 1;
    p.cables.insert(
        id,
        CableState {
            from: port(1, "wheel"),
            to: PortRef::Param {
                id: 3,
                param: "cutoff_hz".into(),
            },
            params: [("amount".to_string(), 1.0)].into(),
            steps: vec![],
        },
    );
    let energy = |wheel: u8| {
        let mut e = engine(&c, &p, 1);
        at(&mut e, 0, KeyEvent::Wheel(wheel), 0);
        at(&mut e, 0, on(48), 0);
        let mut hf = 0.0;
        let mut prev = 0.0;
        for _ in 0..200 {
            for s in block(&mut e).0 {
                hf += ((s - prev) as f64).powi(2);
                prev = s;
            }
        }
        hf
    };
    assert!(energy(127) > 4.0 * energy(0), "a wheel up opens the filter");
}

#[test]
fn two_keyboards_on_channels_play_apart_and_all_layers_by_default() {
    let c = Collector::new();
    let split = two(
        &[("channel", 1.0)],
        "gate",
        Some(&[("channel", 2.0)]),
        "gate",
    );
    let mut e = engine(&c, &split, 1);
    at(&mut e, 0, on(60), 0);
    let (l, r) = block(&mut e);
    assert_eq!((l[0], r[0]), (1.0, 0.0), "channel 1 plays keyboard 1 only");
    at(&mut e, 1, on(40), 0);
    at(&mut e, 1, KeyEvent::Bend(0), 0);
    let (l, r) = block(&mut e);
    assert_eq!((l[0], r[0]), (1.0, 1.0));
    assert_eq!(
        e.expression(1),
        Some((0.0, 0.0)),
        "channel 2's bend stays on keyboard 2"
    );
    assert_eq!(e.expression(2), Some((-1.0, 0.0)));
    at(&mut e, 0, KeyEvent::Off { note: 60 }, 0);
    let (l, r) = block(&mut e);
    assert_eq!((l[0], r[0]), (0.0, 1.0));

    // Default ALL on both: the existing layering, every keyboard hears every note.
    let layered = two(&[], "gate", Some(&[]), "gate");
    let mut e = engine(&c, &layered, 1);
    at(&mut e, 5, on(60), 0);
    let (l, r) = block(&mut e);
    assert_eq!((l[0], r[0]), (1.0, 1.0));
}

#[test]
fn a_note_held_across_a_routing_change_is_released_by_its_own_off() {
    let c = Collector::new();
    let h = c.handle();
    let all = two(&[("bend", 12.0)], "gate", None, "");
    let mut e = engine(&c, &all, 1);
    at(&mut e, 0, on(60), 0);
    at(&mut e, 0, KeyEvent::Sustain(true), 0);
    at(&mut e, 0, KeyEvent::Bend(16383), 0);
    block(&mut e);
    // Keyboard 1 now listens to channel 2 only.
    let moved = two(&[("bend", 12.0), ("channel", 2.0)], "gate", None, "");
    let g = e.build_swap(&h, &moved).unwrap();
    e.receive_swap(g);
    for _ in 0..40 {
        assert_eq!(
            block(&mut e).0[BLOCK - 1],
            1.0,
            "the held note keeps sounding"
        );
    }
    assert_eq!(
        e.expression(1),
        Some((0.0, 0.0)),
        "a channel change resets bend"
    );
    at(&mut e, 0, on(62), 0);
    assert_eq!(e.keys(), (1, 1), "a new channel-1 note is not taken");
    at(&mut e, 0, KeyEvent::Off { note: 60 }, 0);
    assert_eq!(block(&mut e).0[0], 1.0, "channel 1's pedal still holds it");
    at(&mut e, 0, KeyEvent::Sustain(false), 3);
    let (l, _) = block(&mut e);
    assert_eq!(
        (l[2], l[3]),
        (1.0, 0.0),
        "pedal up releases it at its sample"
    );
    assert_eq!(e.keys(), (0, 0));
}

#[test]
fn deleting_or_replacing_a_keyboard_leaves_no_note_or_bend_behind() {
    let c = Collector::new();
    let h = c.handle();
    let p = two(&[], "pitch", Some(&[]), "gate");
    let mut e = engine(&c, &p, 1);
    at(&mut e, 0, on(64), 0);
    at(&mut e, 0, KeyEvent::Bend(16383), 0);
    at(&mut e, 0, KeyEvent::Wheel(90), 0);
    block(&mut e);
    let mut gone = p.clone();
    gone.modules.remove(&2);
    gone.cables.retain(|_, c| c.from.module_id() != 2);
    let g = e.build_swap(&h, &gone).unwrap();
    e.receive_swap(g);
    assert_eq!(e.expression(2), None);
    // Replace it (same id comes back): a fresh keyboard at rest, the old key not replayed.
    let g = e.build_swap(&h, &p).unwrap();
    for _ in 0..40 {
        block(&mut e);
    }
    e.receive_swap(g);
    for _ in 0..40 {
        block(&mut e);
    }
    assert_eq!(e.expression(2), Some((0.0, 0.0)));
    assert_eq!(block(&mut e).1[0], 0.0, "no replayed note");
    assert_eq!(
        e.expression(1),
        Some((1.0, 90.0 / 127.0)),
        "keyboard 1 kept its state"
    );
}

#[test]
fn notes_off_and_a_lost_source_end_only_their_own_notes() {
    let c = Collector::new();
    let mut e = engine(&c, &two(&[], "gate", None, ""), 8);
    at(&mut e, 0, on(60), 0);
    at(&mut e, 1, on(62), 0);
    at(&mut e, 0, KeyEvent::Bend(0), 0);
    e.key_at(MidiEvent::new(Source::Preview, 0, on(70)), 0);
    e.key_at(MidiEvent::new(Source::Host, 0, on(72)), 0);
    assert_eq!(e.keys().1, 4);
    at(&mut e, 1, KeyEvent::NotesOff, 0);
    assert_eq!(e.keys().1, 3, "notes off: channel 2 of the controller only");
    at(&mut e, 0, KeyEvent::SourceLost, 0);
    assert_eq!(e.keys().1, 2, "the preview and the host keep playing");
    assert_eq!(e.expression(1), Some((0.0, 0.0)), "its bend went with it");
    e.key_at(MidiEvent::new(Source::Host, 0, KeyEvent::AllOff), 0);
    assert_eq!(e.keys(), (0, 0), "panic ends everything");
}

#[test]
fn reset_controllers_centers_bend_zeroes_the_wheel_and_lifts_the_pedal() {
    let c = Collector::new();
    let mut e = engine(&c, &two(&[], "gate", None, ""), 1);
    at(&mut e, 0, KeyEvent::Bend(100), 0);
    at(&mut e, 0, KeyEvent::Wheel(127), 0);
    at(&mut e, 0, KeyEvent::Sustain(true), 0);
    at(&mut e, 0, on(60), 0);
    at(&mut e, 0, KeyEvent::Off { note: 60 }, 0);
    assert_eq!(e.keys(), (0, 1));
    at(&mut e, 3, KeyEvent::ResetControllers, 0);
    assert_ne!(
        e.expression(1),
        Some((0.0, 0.0)),
        "another channel's reset changes nothing"
    );
    at(&mut e, 0, KeyEvent::ResetControllers, 0);
    assert_eq!(e.expression(1), Some((0.0, 0.0)));
    assert_eq!(e.keys(), (0, 0));
}

#[test]
fn sustain_holds_only_its_own_channel_and_same_keys_on_two_channels_are_two_voices() {
    let c = Collector::new();
    let mut e = engine(&c, &two(&[], "gate", None, ""), 8);
    at(&mut e, 0, KeyEvent::Sustain(true), 0);
    at(&mut e, 0, on(60), 0);
    at(&mut e, 1, on(60), 0);
    assert_eq!(
        e.keys(),
        (2, 2),
        "one key, two channels: two identities, two voices"
    );
    at(&mut e, 1, KeyEvent::Off { note: 60 }, 0);
    assert_eq!(
        e.keys().1,
        1,
        "channel 2 is not sustained by channel 1's pedal"
    );
    at(&mut e, 0, KeyEvent::Off { note: 60 }, 0);
    assert_eq!(e.keys().1, 1, "channel 1's pedal holds its own note");
    at(&mut e, 0, KeyEvent::Sustain(false), 0);
    assert_eq!(e.keys(), (0, 0));
}

#[test]
fn a_stolen_note_s_release_never_ends_the_note_that_took_its_voice() {
    let c = Collector::new();
    let mut e = engine(&c, &two(&[], "gate", None, ""), 2);
    at(&mut e, 0, on(60), 0);
    at(&mut e, 0, on(62), 0);
    at(&mut e, 0, on(64), 0); // steals 60's voice
    at(&mut e, 0, KeyEvent::Off { note: 60 }, 0);
    assert_eq!(e.keys().1, 2, "60's release finds nothing to end");
    // Velocity 0 is a release.
    e.key_at(
        MidiEvent::parse(Source::Controller, &[0x90, 62, 0]).unwrap(),
        0,
    );
    assert_eq!(e.keys().1, 1);
}

#[test]
fn a_voice_takes_at_most_its_bound_of_changes_and_a_release_always_lands() {
    let mut m = MidiIn::new();
    m.prepare(
        SR,
        BLOCK,
        &kabl_modules::QualityConfig {
            tier: kabl_modules::QualityTier::Live,
        },
    );
    for i in 0..200 {
        m.schedule(
            i % BLOCK,
            Change::Play {
                pitch: i as f32,
                velocity: 1.0,
                glide: false,
                retrigger: true,
            },
        );
    }
    m.schedule(BLOCK - 1, Change::Release);
    let params = [Signal::Scalar(0.0); 6];
    let mut bufs = [[0f32; BLOCK]; 4];
    let [a, b, cc, d] = &mut bufs;
    let mut outs: [&mut [f32]; 4] = [a, b, cc, d];
    let mut io = ProcessIo::new(&[], &mut outs, &params, BLOCK);
    m.process(&mut io);
    assert!(!m.gate(), "the release replaced the last change");
}

#[test]
fn expression_and_routing_do_not_allocate() {
    let c = Collector::new();
    let split = two(
        &[("channel", 1.0)],
        "gate",
        Some(&[("channel", 2.0)]),
        "wheel",
    );
    let mut e = engine(&c, &split, 8);
    block(&mut e);
    assert_no_alloc(|| {
        for i in 0..20u8 {
            at(&mut e, i % 3, on(40 + i), i as usize);
            at(
                &mut e,
                i % 2,
                KeyEvent::Bend(i as u16 * 800),
                i as usize + 1,
            );
            at(&mut e, 1, KeyEvent::Wheel(i * 6), i as usize + 2);
            block(&mut e);
        }
        at(&mut e, 0, KeyEvent::NotesOff, 0);
        at(&mut e, 1, KeyEvent::ResetControllers, 0);
        at(&mut e, 0, KeyEvent::SourceLost, 0);
        block(&mut e);
    });
}
