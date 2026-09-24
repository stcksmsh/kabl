//! Voice allocator (brief section 7's "voice allocator" — routes MIDI notes to instances of a
//! voice-rate subgraph). Until now, every test/demo that plays more than one note at once
//! (`patch_demo.rs`'s chord, `compile.rs`'s `chord_patch` tests) picked voice indices by hand —
//! fine for a fixed test chord, not how a real keyboard works: a person presses keys in whatever
//! order, releases them in whatever order, and may hold more notes than there are voice slots.
//!
//! Deliberately decoupled from `CompiledPatch`/`midi.in`: this is pure bookkeeping (which voice
//! index does note `N` belong to right now), not audio or graph code, and doesn't need to know
//! about pitch semantics (semitones, MIDI note numbers, or anything else) — the caller supplies
//! whatever `note_id` it likes (a `u32`) and is responsible for turning an assigned voice index
//! into an actual `MidiIn::note_on` call. Keeping it decoupled makes it independently testable
//! (no `CompiledPatch`/`compile()` needed to test allocation policy) and reusable regardless of
//! how pitch ends up represented.
//!
//! Policy: assign the lowest-index free voice; when all voices are busy, steal the *oldest*
//! still-held voice (the one whose `note_on` happened longest ago) — a standard, simple voice-
//! stealing policy (same "last-note priority" spirit `midi_in.rs`'s doc comment already
//! mentions for a single instance, extended across many). Not velocity- or envelope-stage-aware
//! (stealing the quietest/most-released voice is a real, better-sounding refinement that needs
//! to inspect each voice's envelope state — deferred, flagged, not attempted blind).

use std::collections::HashMap;

pub struct VoiceAllocator {
    /// Per-voice: `Some((note_id, age))` if held, `None` if free. `age` is this allocator's own
    /// monotonic counter, not a wall-clock time — only relative order matters for "oldest."
    voices: Vec<Option<(u32, u64)>>,
    /// note_id -> voice index, kept in sync with `voices` for O(1) `note_off` instead of a scan.
    note_to_voice: HashMap<u32, usize>,
    next_age: u64,
}

impl VoiceAllocator {
    /// `voice_count` must be at least 1 — a 0-voice allocator has nothing to assign and
    /// `note_on` would panic trying to steal from an empty pool.
    pub fn new(voice_count: usize) -> Self {
        VoiceAllocator {
            voices: vec![None; voice_count],
            note_to_voice: HashMap::new(),
            next_age: 0,
        }
    }

    pub fn voice_count(&self) -> usize {
        self.voices.len()
    }

    /// Voices holding a note (on, not yet off).
    pub fn held(&self) -> usize {
        self.voices.iter().filter(|v| v.is_some()).count()
    }

    /// Assigns `note_id` a voice: the lowest-index free voice if one exists, otherwise the
    /// oldest currently-held voice (stolen — whatever note it was playing loses its voice
    /// without a `note_off` of its own; the caller decides whether/how to signal that, e.g. by
    /// releasing the stolen note's envelope some other way, or just letting the new note's
    /// `note_on` retrigger the shared voice-rate chain).
    ///
    /// Re-triggering an already-held `note_id` (a duplicate `note_on` without an intervening
    /// `note_off`) reuses that note's existing voice and refreshes its age, rather than stealing
    /// a second voice for the same note — a real MIDI source can send a repeated note-on (e.g. a
    /// stuck key's driver, or legato retrigger), and holding two voices for one logical note
    /// would leak a slot until both happened to get stolen.
    pub fn note_on(&mut self, note_id: u32) -> usize {
        let age = self.next_age;
        self.next_age += 1;

        if let Some(&voice) = self.note_to_voice.get(&note_id) {
            self.voices[voice] = Some((note_id, age));
            return voice;
        }

        let voice = self
            .voices
            .iter()
            .position(|v| v.is_none())
            .unwrap_or_else(|| self.oldest_voice());

        if let Some((old_note, _)) = self.voices[voice] {
            self.note_to_voice.remove(&old_note);
        }
        self.voices[voice] = Some((note_id, age));
        self.note_to_voice.insert(note_id, voice);
        voice
    }

    /// Releases `note_id`, returning its voice index — `None` if this note isn't currently
    /// resident (already released, or its voice was stolen by a later `note_on`). A caller
    /// should only call `MidiIn::note_off` on the returned voice, never unconditionally on
    /// whatever voice it expected: if this note was stolen, releasing that voice would incorrectly
    /// cut off the *new* note that stole it.
    pub fn note_off(&mut self, note_id: u32) -> Option<usize> {
        let voice = self.note_to_voice.remove(&note_id)?;
        self.voices[voice] = None;
        Some(voice)
    }

    /// Voice index currently holding `note_id`, if any — for a caller that wants to query
    /// without mutating (e.g. to update a note already in flight rather than sending a fresh
    /// `note_on`).
    pub fn voice_for(&self, note_id: u32) -> Option<usize> {
        self.note_to_voice.get(&note_id).copied()
    }

    fn oldest_voice(&self) -> usize {
        self.voices
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.map(|(_, age)| (i, age)))
            .min_by_key(|&(_, age)| age)
            .map(|(i, _)| i)
            .expect("oldest_voice is only called when every voice is occupied")
    }
}
