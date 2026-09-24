//! The keyboard rules of docs/sound-palette-batch/keyboard.md, event by event: POLY voice
//! choice, stealing, the pedal, repeated notes; MONO/LEGATO priorities, returns to held notes,
//! envelope restarts, glide; All Notes Off, mode changes. Then in the engine: the gate dip and
//! glide as signals, keys held through live edits (overlapping too), Load, a deleted
//! `midi.in`, a sequencer chain untouched by the settings, no allocation.

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::Collector;
use kabl_core::{CableState, ModuleId, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::compile::compile;
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::{Action, KeyEvent, Keyboard};
use kabl_engine::patch_engine::PatchEngine;
use kabl_modules::builtins::KeySettings;

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;
const VOICES: usize = 8;

fn on(note: u8) -> KeyEvent {
    KeyEvent::On {
        note,
        velocity: 100,
    }
}
fn off(note: u8) -> KeyEvent {
    KeyEvent::Off { note }
}

/// Short form of an action: ('P', voice, note, glide, retrigger) or ('R', voice, 0, ..).
type A = (char, usize, i32, bool, bool);

fn run(kb: &mut Keyboard, events: &[KeyEvent]) -> Vec<A> {
    let mut v = Vec::new();
    for &e in events {
        kb.event(e, &mut |a| {
            v.push(match a {
                Action::Play {
                    voice,
                    pitch,
                    glide,
                    retrigger,
                    ..
                } => ('P', voice, pitch as i32 + 60, glide, retrigger),
                Action::Release { voice } => ('R', voice, 0, false, false),
            })
        });
    }
    v
}

fn kb(mode: u8, priority: u8, glide: u8) -> Keyboard {
    Keyboard::new(
        1,
        KeySettings {
            mode,
            priority,
            glide,
        },
        VOICES,
    )
}

const P: char = 'P';
const R: char = 'R';

// ---- POLY ----

#[test]
fn poly_takes_the_voice_released_longest_ago() {
    let mut k = kb(0, 0, 0);
    let a = run(
        &mut k,
        &[on(60), on(64), on(67), off(64), off(60), on(72), on(74)],
    );
    // 60→0, 64→1, 67→2; 64 then 60 released; next free voices: never-used 3.. have
    // released = 0, the oldest release; lowest index first.
    assert_eq!(
        a,
        vec![
            (P, 0, 60, false, true),
            (P, 1, 64, false, true),
            (P, 2, 67, false, true),
            (R, 1, 0, false, false),
            (R, 0, 0, false, false),
            (P, 3, 72, false, true),
            (P, 4, 74, false, true),
        ]
    );
    // One note at a time uses voice 0 every time (the old allocator's choice too)... after
    // the unused voices: then the one released longest ago.
    let mut k = kb(0, 0, 0);
    let a = run(&mut k, &[on(60), off(60), on(62), off(62)]);
    assert_eq!(a[2].1, 1, "a fresh voice before reusing a release tail");
}

#[test]
fn poly_steals_pedal_held_voices_before_keys_that_are_down() {
    let mut k = kb(0, 0, 0);
    let mut events = vec![KeyEvent::Sustain(true)];
    events.extend((60..68).map(on)); // all 8 voices
    events.push(off(62)); // voice 2 now held by the pedal only
    events.push(on(80));
    let a = run(&mut k, &events);
    assert_eq!(*a.last().unwrap(), (P, 2, 80, false, true));
    // No pedal-held voice: the oldest key down (60, voice 0).
    let mut k = kb(0, 0, 0);
    let mut events: Vec<KeyEvent> = (60..68).map(on).collect();
    events.push(on(81));
    let a = run(&mut k, &events);
    assert_eq!(*a.last().unwrap(), (P, 0, 81, false, true));
    // The stolen key's later release does nothing (its voice plays 81 now).
    assert!(run(&mut k, &[off(60)]).is_empty());
}

#[test]
fn poly_pedal_holds_released_keys_until_it_comes_up() {
    let mut k = kb(0, 0, 0);
    let a = run(
        &mut k,
        &[
            on(60),
            KeyEvent::Sustain(true),
            off(60),
            on(64),
            off(64),
            on(67),
        ],
    );
    assert!(
        a.iter().all(|x| x.0 == P),
        "nothing releases while the pedal is down"
    );
    assert_eq!(k.voices_sounding(), 3);
    let a = run(&mut k, &[KeyEvent::Sustain(false)]);
    assert_eq!(a, vec![(R, 0, 0, false, false), (R, 1, 0, false, false)]);
    assert_eq!(k.voices_sounding(), 1, "67 is still down");
    assert_eq!(run(&mut k, &[off(67)]), vec![(R, 2, 0, false, false)]);
}

#[test]
fn poly_repeated_note_restarts_on_its_own_voice() {
    let mut k = kb(0, 0, 0);
    let a = run(
        &mut k,
        &[
            on(60),
            on(60), // again without a release
            KeyEvent::Sustain(true),
            off(60),
            on(60), // again while the pedal holds it
        ],
    );
    assert_eq!(
        a,
        vec![
            (P, 0, 60, false, true),
            (P, 0, 60, false, true),
            (P, 0, 60, false, true)
        ]
    );
    assert_eq!(k.voices_sounding(), 1);
}

#[test]
fn poly_glide_always_follows_each_voice_from_its_last_note() {
    let mut k = kb(0, 0, 1);
    let a = run(&mut k, &[on(60), on(64), off(60), off(64), on(67)]);
    assert!(!a[0].3 && !a[1].3, "a voice's first note doesn't glide");
    // 67 lands on voice 2 (never used): no glide either; voice 0 next would glide.
    assert_eq!(a[4], (P, 2, 67, false, true));
    let a = run(&mut k, &[on(69), on(71), on(72)]);
    assert!(a.iter().filter(|x| x.1 < 2).all(|x| x.3), "{a:?}");
}

// ---- MONO / LEGATO ----

#[test]
fn mono_last_priority_returns_to_held_notes_and_retriggers() {
    let mut k = kb(1, 0, 0);
    let a = run(&mut k, &[on(60), on(64), on(67), off(67), off(60), off(64)]);
    assert_eq!(
        a,
        vec![
            (P, 0, 60, false, true),
            (P, 0, 64, false, true),
            (P, 0, 67, false, true),
            (P, 0, 64, false, true), // back to the most recent held key
            // 60 up: it wasn't sounding, nothing changes
            (R, 0, 0, false, false),
        ]
    );
}

#[test]
fn legato_restarts_only_a_new_phrase() {
    let mut k = kb(2, 0, 0);
    let a = run(&mut k, &[on(60), on(64), off(64), off(60), on(62)]);
    assert_eq!(
        a,
        vec![
            (P, 0, 60, false, true),  // new phrase (gate was low: no-op dip)
            (P, 0, 64, false, false), // overlapping: legato
            (P, 0, 60, false, false), // return to held: legato
            (R, 0, 0, false, false),
            (P, 0, 62, false, true), // new phrase
        ]
    );
}

#[test]
fn low_and_high_priority() {
    let mut k = kb(1, 1, 0); // LOW
    let a = run(&mut k, &[on(60), on(64), on(55), off(55), off(60)]);
    assert_eq!(
        a.iter().map(|x| x.2).collect::<Vec<_>>(),
        vec![60, 55, 60, 64],
        "64 doesn't sound while 60 is down; 55 does; then back up"
    );
    let mut k = kb(1, 2, 0); // HIGH
    let a = run(&mut k, &[on(60), on(55), on(64), off(64), off(60)]);
    assert_eq!(
        a.iter().map(|x| x.2).collect::<Vec<_>>(),
        vec![60, 64, 60, 55]
    );
}

#[test]
fn mono_glide_always_and_legato_only() {
    let mut k = kb(1, 0, 1); // ALWAYS
    let a = run(&mut k, &[on(60), off(60), on(67), on(72)]);
    assert_eq!(
        a.iter().map(|x| x.3).collect::<Vec<_>>(),
        vec![true, false, true, true],
        "the release has no glide flag; every note glides (MidiIn skips the very first)"
    );
    let mut k = kb(2, 0, 2); // LEGATO mode, glide LEGATO
    let a = run(&mut k, &[on(60), on(67), off(67), off(60), on(72)]);
    assert_eq!(
        a.iter().map(|x| (x.0, x.3)).collect::<Vec<_>>(),
        vec![(P, false), (P, true), (P, true), (R, false), (P, false)]
    );
}

#[test]
fn mono_pedal_keeps_the_last_note_and_the_next_key_is_a_new_phrase() {
    let mut k = kb(2, 0, 2);
    let a = run(
        &mut k,
        &[KeyEvent::Sustain(true), on(60), off(60), on(64), off(64)],
    );
    assert_eq!(
        a,
        vec![
            (P, 0, 60, false, true),
            (P, 0, 64, false, true), // no key was down: new phrase, restart, no legato glide
        ]
    );
    assert_eq!(k.voices_sounding(), 1);
    assert_eq!(
        run(&mut k, &[KeyEvent::Sustain(false)]),
        vec![(R, 0, 0, false, false)]
    );
}

#[test]
fn mono_repeated_key_is_release_then_press() {
    let mut k = kb(1, 0, 0);
    let a = run(&mut k, &[on(60), on(60)]);
    assert_eq!(
        a,
        vec![
            (P, 0, 60, false, true),
            (R, 0, 0, false, false),
            (P, 0, 60, false, true)
        ]
    );
}

#[test]
fn all_notes_off_and_mode_changes_release_and_forget() {
    let mut k = kb(0, 0, 0);
    run(&mut k, &[KeyEvent::Sustain(true), on(60), on(64), off(60)]);
    let a = run(&mut k, &[KeyEvent::AllOff]);
    assert_eq!(a.len(), 2);
    assert_eq!((k.keys_held(), k.voices_sounding()), (0, 0));
    assert!(
        run(&mut k, &[off(64)]).is_empty(),
        "a late key up is ignored"
    );
    // The pedal is forgotten too: a key released now releases.
    run(&mut k, &[on(62)]);
    assert_eq!(run(&mut k, &[off(62)]).len(), 1);

    let mut k = kb(0, 0, 0);
    run(&mut k, &[on(60), on(64)]);
    let mut a = Vec::new();
    k.set_settings(
        KeySettings {
            mode: 1,
            priority: 0,
            glide: 0,
        },
        &mut |x| a.push(x),
    );
    assert_eq!(a.len(), 2);
    assert_eq!((k.keys_held(), k.voices_sounding()), (0, 0));
    // Priority and glide changes release nothing.
    let mut a = Vec::new();
    run(&mut k, &[on(60)]);
    k.set_settings(
        KeySettings {
            mode: 1,
            priority: 2,
            glide: 1,
        },
        &mut |x| a.push(x),
    );
    assert!(a.is_empty());
}

#[test]
fn raw_midi_parses_to_key_events() {
    assert_eq!(KeyEvent::from_midi(&[0x91, 60, 100]), Some(on(60)));
    assert_eq!(KeyEvent::from_midi(&[0x90, 60, 0]), Some(off(60)));
    assert_eq!(KeyEvent::from_midi(&[0x80, 60, 30]), Some(off(60)));
    assert_eq!(
        KeyEvent::from_midi(&[0xB0, 64, 127]),
        Some(KeyEvent::Sustain(true))
    );
    assert_eq!(
        KeyEvent::from_midi(&[0xB0, 64, 63]),
        Some(KeyEvent::Sustain(false))
    );
    assert_eq!(KeyEvent::from_midi(&[0xB0, 123, 0]), Some(KeyEvent::AllOff));
    assert_eq!(KeyEvent::from_midi(&[0xB0, 120, 0]), Some(KeyEvent::AllOff));
    assert_eq!(KeyEvent::from_midi(&[0xB0, 1, 20]), None);
}

// ---- In the engine ----

const MIDI: ModuleId = 1;
const OUT: ModuleId = 2;
const CLOCK: ModuleId = 3;
const SEQ: ModuleId = 4;
const OSC: ModuleId = 5;

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

/// `midi.in` pitch on the left output and gate on the right, so a render shows both exactly
/// (the voice average of 8 lanes: one sounding voice reads 1/8 of its gate and pitch).
fn probe(params: &[(&str, f32)]) -> PatchState {
    let mut p = PatchState::new();
    p.modules.insert(MIDI, module("midi.in", params));
    p.modules.insert(OUT, module("out", &[]));
    cable(&mut p, port(MIDI, "pitch"), port(OUT, "left"));
    cable(&mut p, port(MIDI, "gate"), port(OUT, "right"));
    p
}

fn block(e: &mut PatchEngine) -> ([f32; BLOCK], [f32; BLOCK]) {
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    e.process_block(&mut l, &mut r);
    (l, r)
}

#[test]
fn legato_glide_is_a_straight_line_of_the_set_time_and_mono_dips_the_gate() {
    let collector = Collector::new();
    // LEGATO mode, glide LEGATO, 100 ms.
    let patch = probe(&[("mode", 2.0), ("glide", 2.0), ("glide_ms", 100.0)]);
    let mut e = PatchEngine::new(&collector.handle(), &patch, SR, VOICES).unwrap();
    e.key(on(60));
    block(&mut e);
    e.key(on(72)); // overlapping: glide 0 → 12 st over 4800 samples, no dip
    let mut pitch = Vec::new();
    let mut gate = Vec::new();
    for _ in 0..100 {
        let (l, r) = block(&mut e);
        pitch.extend(l.iter().map(|v| v * VOICES as f32));
        gate.extend(r.iter().map(|v| v * VOICES as f32));
    }
    assert!(
        gate.iter().all(|&g| g == 1.0),
        "legato: the gate never dips"
    );
    for (i, &p) in pitch.iter().enumerate().take(4800) {
        let want = 12.0 * (i + 1) as f32 / 4800.0;
        assert!((p - want).abs() < 1e-3, "sample {i}: {p} vs {want}");
    }
    assert!(pitch[4800..].iter().all(|&p| (p - 12.0).abs() < 1e-4));

    // MONO (retrigger): the next note dips the gate for exactly the first sample.
    let patch = probe(&[("mode", 1.0)]);
    let mut e = PatchEngine::new(&collector.handle(), &patch, SR, VOICES).unwrap();
    e.key(on(60));
    block(&mut e);
    e.key(on(62));
    let (l, r) = block(&mut e);
    assert_eq!(r[0], 0.0);
    assert!(r[1..].iter().all(|&g| g * VOICES as f32 == 1.0));
    assert_eq!(
        l[0] * VOICES as f32,
        2.0,
        "no glide: the pitch is there at once"
    );
}

#[test]
fn a_glide_time_change_applies_to_the_glide_in_progress() {
    let collector = Collector::new();
    let patch = probe(&[("mode", 1.0), ("glide", 1.0), ("glide_ms", 1000.0)]);
    let mut e = PatchEngine::new(&collector.handle(), &patch, SR, VOICES).unwrap();
    e.key(on(60));
    block(&mut e);
    e.key(on(72));
    for _ in 0..75 {
        block(&mut e); // 100 ms: 1.2 st of 12
    }
    let mut faster = probe(&[("mode", 1.0), ("glide", 1.0), ("glide_ms", 10.0)]);
    faster.modules.get_mut(&MIDI).unwrap();
    let g = e.build_swap(&collector.handle(), &faster).unwrap();
    e.receive_swap(g);
    for _ in 0..40 {
        block(&mut e);
    }
    let (l, _) = block(&mut e);
    assert!((l[0] * VOICES as f32 - 12.0).abs() < 1e-4, "arrived");
}

#[test]
fn keys_survive_live_edits_even_overlapping_ones_and_never_stick() {
    let mut collector = Collector::new();
    let handle = collector.handle();
    let patch = probe(&[("mode", 0.0)]);
    let mut e = PatchEngine::new(&handle, &patch, SR, VOICES).unwrap();
    // Pre-build graphs (allocates) before the audio-thread part.
    let swaps: Vec<_> = (0..12)
        .map(|_| e.build_swap(&handle, &patch).unwrap())
        .collect();
    let mut swaps = swaps.into_iter();
    let mut gates = Vec::with_capacity(64);
    assert_no_alloc(|| {
        e.key(on(60));
        e.key(on(64));
        e.key(KeyEvent::Sustain(true));
        for b in 0..60 {
            // Two graphs per swap point, one block apart: overlapping fades.
            if b % 5 == 0 || b % 5 == 1 {
                if let Some(g) = swaps.next() {
                    e.receive_swap(g);
                }
            }
            if b == 20 {
                e.key(off(60)); // held by the pedal
            }
            if b == 40 {
                e.key(KeyEvent::Sustain(false));
                e.key(off(64));
            }
            let (_, r) = block(&mut e);
            gates.push(r[BLOCK - 1] * VOICES as f32);
        }
    });
    collector.collect();
    assert!(
        gates[..40].iter().all(|&g| (g - 2.0).abs() < 1e-5),
        "{gates:?}"
    );
    assert!(gates[41..].iter().all(|&g| g == 0.0), "{gates:?}");
    assert_eq!(e.keys(), (0, 0));
}

#[test]
fn a_mode_change_releases_and_load_starts_fresh() {
    let collector = Collector::new();
    let handle = collector.handle();
    let mut e = PatchEngine::new(&handle, &probe(&[]), SR, VOICES).unwrap();
    e.key(on(60));
    e.key(on(64));
    block(&mut e);
    let g = e.build_swap(&handle, &probe(&[("mode", 2.0)])).unwrap();
    e.receive_swap(g);
    for _ in 0..30 {
        block(&mut e);
    }
    assert_eq!(e.keys(), (0, 0));
    assert_eq!(block(&mut e).1, [0.0; BLOCK]);
    // Keys still down play from their next press; their old releases are ignored.
    e.key(off(60));
    e.key(on(67));
    assert_eq!(e.keys(), (1, 1));

    // Load: a fresh graph, every keyboard starts over.
    let mut g = compile(&probe(&[("mode", 2.0)]), SR, VOICES).unwrap();
    g.fresh = true;
    e.finish_swap(&handle, g);
    for _ in 0..30 {
        block(&mut e);
    }
    assert_eq!(e.keys(), (0, 0));
    assert_eq!(block(&mut e).1, [0.0; BLOCK]);
}

#[test]
fn deleting_the_midi_in_releases_its_voices() {
    let collector = Collector::new();
    let handle = collector.handle();
    let mut e = PatchEngine::new(&handle, &probe(&[]), SR, VOICES).unwrap();
    e.key(on(60));
    block(&mut e);
    let mut empty = probe(&[]);
    empty.modules.remove(&MIDI);
    empty.cables.clear();
    let g = e.build_swap(&handle, &empty).unwrap();
    e.receive_swap(g);
    assert_eq!(e.keys(), (0, 0));
    // Put it back: it plays from the next press.
    let g = e.build_swap(&handle, &probe(&[])).unwrap();
    for _ in 0..30 {
        block(&mut e);
    }
    e.receive_swap(g);
    e.key(on(62));
    assert_eq!(e.keys(), (1, 1));
}

#[test]
fn keyboard_settings_never_touch_a_sequencer_chain() {
    // A clocked sequencer into an oscillator on the left; the MIDI probe's gate on the right.
    let build = |mode: f32| {
        let mut p = PatchState::new();
        p.modules
            .insert(MIDI, module("midi.in", &[("mode", mode), ("glide", 1.0)]));
        p.modules.insert(OUT, module("out", &[]));
        p.modules.insert(CLOCK, module("clock", &[("bpm", 140.0)]));
        p.modules
            .insert(SEQ, module("seq", &[("p2", 7.0), ("p3", -5.0)]));
        p.modules.insert(OSC, module("osc.va", &[]));
        cable(&mut p, port(CLOCK, "gate"), port(SEQ, "clock"));
        cable(&mut p, port(SEQ, "pitch"), port(OSC, "pitch"));
        cable(&mut p, port(OSC, "out"), port(OUT, "left"));
        cable(&mut p, port(MIDI, "gate"), port(OUT, "right"));
        p
    };
    let collector = Collector::new();
    let render = |mode: f32| {
        let mut e = PatchEngine::new(&collector.handle(), &build(mode), SR, VOICES).unwrap();
        let mut left = Vec::new();
        for b in 0..400 {
            if b % 50 == 0 {
                e.key(on(60 + (b / 50) as u8));
            }
            left.extend(block(&mut e).0);
        }
        left
    };
    assert_eq!(render(0.0), render(1.0));
    assert_eq!(render(0.0), render(2.0));
}
