//! Correctness for `kabl-standalone`'s hardware-independent logic: the ring buffer (proven RT-
//! safe via `assert_no_alloc`), MIDI-byte resolution, and voice-event application against a real
//! compiled patch. Nothing here needs an audio device or MIDI port — see `lib.rs`'s module doc
//! for why that split exists.

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use kabl_engine::compile::compile;
use kabl_engine::patch_engine::PatchEngine;
use kabl_engine::voice_allocator::VoiceAllocator;
use kabl_modules::builtins::MidiIn;
use kabl_modules::Module;
use kabl_standalone::{
    apply_voice_event, default_patch, resolve_midi_message, RingBuffer, VoiceEvent,
    DEFAULT_VOICE_COUNT, MIDI_IN_ID,
};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

// --- RingBuffer ---

#[test]
fn ring_buffer_round_trips_samples_in_order() {
    let mut rb = RingBuffer::new(8);
    rb.push_slice(&[1.0, 2.0, 3.0]);
    assert_eq!(rb.available(), 3);
    assert_eq!(rb.pop(), Some(1.0));
    assert_eq!(rb.pop(), Some(2.0));
    rb.push_slice(&[4.0, 5.0]);
    assert_eq!(rb.pop(), Some(3.0));
    assert_eq!(rb.pop(), Some(4.0));
    assert_eq!(rb.pop(), Some(5.0));
    assert_eq!(rb.pop(), None);
}

#[test]
fn ring_buffer_wraps_around_capacity() {
    let mut rb = RingBuffer::new(4);
    rb.push_slice(&[1.0, 2.0, 3.0]);
    rb.pop();
    rb.pop();
    // write pointer wraps past the end of the backing array here.
    rb.push_slice(&[4.0, 5.0, 6.0]);
    let mut out = Vec::new();
    while let Some(v) = rb.pop() {
        out.push(v);
    }
    assert_eq!(out, vec![3.0, 4.0, 5.0, 6.0]);
}

#[test]
#[should_panic(expected = "RingBuffer overflow")]
fn ring_buffer_overflow_panics_rather_than_silently_dropping() {
    let mut rb = RingBuffer::new(2);
    rb.push_slice(&[1.0, 2.0, 3.0]);
}

#[test]
fn ring_buffer_push_and_pop_do_not_allocate() {
    let mut rb = RingBuffer::new(1024);
    assert_no_alloc(|| {
        rb.push_slice(&[0.0; 64]);
        for _ in 0..64 {
            rb.pop();
        }
    });
}

// --- resolve_midi_message ---

#[test]
fn note_on_resolves_to_a_voice_and_correct_pitch() {
    let mut alloc = VoiceAllocator::new(4);
    let event = resolve_midi_message(&mut alloc, &[0x90, 60, 100]);
    assert_eq!(
        event,
        Some(VoiceEvent::NoteOn {
            voice: 0,
            semitones: 0.0,
            velocity: 100.0 / 127.0,
        })
    );
}

#[test]
fn note_on_pitch_is_relative_to_midi_note_60() {
    let mut alloc = VoiceAllocator::new(4);
    let event = resolve_midi_message(&mut alloc, &[0x90, 72, 127]);
    assert_eq!(
        event,
        Some(VoiceEvent::NoteOn {
            voice: 0,
            semitones: 12.0,
            velocity: 1.0,
        })
    );
}

#[test]
fn note_on_with_zero_velocity_is_a_note_off() {
    let mut alloc = VoiceAllocator::new(4);
    resolve_midi_message(&mut alloc, &[0x90, 60, 100]);
    let event = resolve_midi_message(&mut alloc, &[0x90, 60, 0]);
    assert_eq!(event, Some(VoiceEvent::NoteOff { voice: 0 }));
}

#[test]
fn explicit_note_off_status_resolves() {
    let mut alloc = VoiceAllocator::new(4);
    resolve_midi_message(&mut alloc, &[0x90, 60, 100]);
    let event = resolve_midi_message(&mut alloc, &[0x80, 60, 64]);
    assert_eq!(event, Some(VoiceEvent::NoteOff { voice: 0 }));
}

#[test]
fn note_off_for_an_unheld_note_resolves_to_none() {
    let mut alloc = VoiceAllocator::new(4);
    let event = resolve_midi_message(&mut alloc, &[0x80, 60, 64]);
    assert_eq!(event, None);
}

#[test]
fn non_note_messages_resolve_to_none() {
    let mut alloc = VoiceAllocator::new(4);
    // Control change (0xB0), pitch bend (0xE0), and a too-short message all resolve to nothing.
    assert_eq!(resolve_midi_message(&mut alloc, &[0xB0, 7, 100]), None);
    assert_eq!(resolve_midi_message(&mut alloc, &[0xE0, 0, 64]), None);
    assert_eq!(resolve_midi_message(&mut alloc, &[0x90, 60]), None);
}

// --- apply_voice_event, against a real compiled patch ---

fn pitch_of(compiled: &mut kabl_engine::compile::CompiledPatch, voice: usize) -> Option<f32> {
    let midi = compiled
        .module_mut(MIDI_IN_ID, Some(voice))?
        .as_any_mut()
        .downcast_mut::<MidiIn>()?;
    let mut state = std::collections::HashMap::new();
    struct Probe<'a>(&'a mut std::collections::HashMap<String, f32>);
    impl kabl_modules::StateWriter for Probe<'_> {
        fn write_f32(&mut self, key: &str, value: f32) {
            self.0.insert(key.to_string(), value);
        }
    }
    midi.save_state(&mut Probe(&mut state));
    state
        .get("gate")
        .filter(|&&g| g > 0.5)
        .map(|_| state["pitch"])
}

#[test]
fn apply_voice_event_reaches_the_correct_compiled_voice() {
    let patch = default_patch();
    let handle_collector = basedrop::Collector::new();
    let handle = handle_collector.handle();
    let mut engine = PatchEngine::new(&handle, &patch, 48000.0, DEFAULT_VOICE_COUNT)
        .expect("default_patch should compile");

    apply_voice_event(
        &mut engine,
        MIDI_IN_ID,
        VoiceEvent::NoteOn {
            voice: 2,
            semitones: 7.0,
            velocity: 0.5,
        },
    );

    assert_eq!(pitch_of(engine.active_mut(), 2), Some(7.0));
    assert_eq!(
        pitch_of(engine.active_mut(), 0),
        None,
        "other voices untouched"
    );

    apply_voice_event(&mut engine, MIDI_IN_ID, VoiceEvent::NoteOff { voice: 2 });
    assert_eq!(
        pitch_of(engine.active_mut(), 2),
        None,
        "gate should drop on note off"
    );
}

#[test]
fn apply_voice_event_does_not_allocate() {
    let patch = default_patch();
    let collector = basedrop::Collector::new();
    let handle = collector.handle();
    let mut engine = PatchEngine::new(&handle, &patch, 48000.0, DEFAULT_VOICE_COUNT)
        .expect("default_patch should compile");

    assert_no_alloc(|| {
        apply_voice_event(
            &mut engine,
            MIDI_IN_ID,
            VoiceEvent::NoteOn {
                voice: 0,
                semitones: 0.0,
                velocity: 0.8,
            },
        );
        apply_voice_event(&mut engine, MIDI_IN_ID, VoiceEvent::NoteOff { voice: 0 });
    });
}

// --- default_patch() ---

#[test]
fn default_patch_compiles_and_plays() {
    let patch = default_patch();
    let mut compiled =
        compile(&patch, 48000.0, DEFAULT_VOICE_COUNT).expect("default_patch should compile");

    let mut alloc = VoiceAllocator::new(DEFAULT_VOICE_COUNT);
    let voice = alloc.note_on(60);
    if let Some(m) = compiled.module_mut(MIDI_IN_ID, Some(voice)) {
        if let Some(midi) = m.as_any_mut().downcast_mut::<MidiIn>() {
            midi.note_on(0.0, 0.9);
        }
    }

    let mut sum_sq = 0.0f64;
    let mut n = 0usize;
    for _ in 0..100 {
        compiled.process_block();
        for &s in compiled.left().iter() {
            assert!(s.is_finite());
            sum_sq += (s as f64) * (s as f64);
            n += 1;
        }
    }
    let rms = (sum_sq / n as f64).sqrt();
    assert!(
        rms > 0.001,
        "default patch should produce audible output, got rms={rms}"
    );
}
