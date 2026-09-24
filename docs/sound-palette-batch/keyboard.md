# Keyboard expression: the rules

Written before the code (sound palette batch, stage B). The code is
`crates/engine/src/keyboard.rs` (the state machine), `crates/modules/src/builtins/midi_in.rs`
(glide and the retrigger dip) and `crates/ui/src/main.rs` (MIDI to the audio thread). Tests:
`crates/engine/tests/keyboard.rs`.

## Where the settings live

The voice settings belong to the **MIDI In module** (`midi.in`), as four ordinary params saved
with the patch and undone like any knob:

| param | options | default |
|---|---|---|
| `mode` | POLY, MONO (retrigger), LEGATO | POLY (the old behaviour) |
| `priority` | LAST, LOW, HIGH (mono modes) | LAST |
| `glide` | OFF, ALWAYS, LEGATO (only between overlapping notes) | OFF |
| `glide_ms` | 5 ms – 3 s, exponential | 120 ms |

Only what a `midi.in` drives follows them. A sequencer chain has no `midi.in`, so mono,
priority and glide never touch it. With several `midi.in` modules, each has its own settings
and its own voice assignment; every one still hears every key (as before).

## Keys, voices, the pedal

- The keyboard state lives on the audio thread (`PatchEngine`), one per `midi.in`, outside the
  graphs: MIDI arrives as key events (note on/off, sustain pedal CC 64, All Notes Off CC 123 /
  All Sound Off CC 120), and the keyboard turns them into per-voice play/release actions for
  every running graph. So a live edit (a graph swap) never loses or replays a key.
- **Held** = the key is physically down. **Sounding** = a voice's gate is high. The sustain
  pedal (CC 64 ≥ 64 = down) keeps released keys sounding until it comes up.

### POLY (default)

- A new key takes a free voice: the one released longest ago (so a release tail is reused
  last); with several never used, the lowest. The old allocator took the lowest free voice;
  playing one note at a time, both give voice 0 (checked: the existing tests are unchanged).
- All voices busy: steal the oldest voice whose key is up but held by the pedal; if none, the
  oldest held voice. A stolen voice restarts its envelope (gate dip, below).
- Key up: the voice releases, unless the pedal is down: then it keeps sounding, marked
  "sustained". Pedal up releases every sustained voice.
- The same key again while it still sounds (held twice, or sustained): it stays on its voice
  and the envelope restarts.
- Glide ALWAYS: each voice glides from the last pitch *it* played (a voice's first note does
  not glide). Glide LEGATO has no meaning in POLY and does nothing.

### MONO and LEGATO

One voice (voice 0). The keyboard keeps the list of held keys, each with its velocity.

- **Which key sounds**: among held keys, the priority pick: LAST = the most recently pressed,
  LOW = the lowest, HIGH = the highest.
- **Key down**: if the new key becomes the pick, it sounds: pitch and velocity change. If it
  doesn't (e.g. LOW priority and the key is above the one sounding), nothing audible changes;
  the key is remembered.
- **Key up of the sounding key**: if other keys are still held, the pick among them sounds
  again at its own velocity ("return to held note"). If none is held and the pedal is down, the
  last note keeps sounding until pedal up. Otherwise the voice releases.
- Key up of a key that isn't sounding: it is forgotten, nothing audible changes.
- **Envelope**: MONO restarts the envelope on every change of sounding note, including a
  return to a held note. LEGATO restarts it only when no other key was held at the moment of
  the change (a new phrase), so overlapping notes glide/step without re-attacking. A note
  sustained only by the pedal is not "held": the next key starts a new phrase.
- The same key pressed again without a release counts as release + press.
- **Glide**: ALWAYS glides on every change of sounding note, including after a release (from
  the last pitch played; the very first note does not glide). LEGATO glides only when another
  key was held (overlapping playing, including returns to a held note).

### Glide

From the pitch the voice is at right now (mid-glide included) to the new note, in a straight
line in semitones, taking exactly `glide_ms` whatever the interval (constant time). A glide
time change applies to the glide in progress from the next block.

### Envelope restart ("gate dip")

A restart holds the voice's gate low for the first sample of the next block, then high: an
envelope sees a release edge and a new attack from its current level (no click, no reset to
zero). A voice that isn't sounding starts normally.

## Other events

- **All Notes Off** (Perform panel button, CC 123, CC 120), **MIDI disconnect**, and **Load**:
  every voice of every keyboard releases (release tails ring), held keys and the pedal state
  are forgotten; note-offs for keys that were down are then ignored. A pedal still down comes
  back with its next CC 64 message.
- **Changing `mode`** (an edit): that keyboard releases every voice and forgets its keys (as
  All Notes Off for that module). Keys still down play again from their next press. Changing
  `priority`, `glide` or `glide_ms` applies from the next key event and releases nothing.
- **MIDI reconnect**: nothing is held after the disconnect, so nothing can stick.
- **Live edits** (graph swaps): keyboards live outside the graphs; the `midi.in` state (gates,
  pitches, a glide in progress, a pending dip) is carried into the new graph. Every key event
  reaches every running graph.
- **Deleting a `midi.in`**: its keyboard goes with it. Adding one: it hears keys from the next
  press.
- **Transport** (Run/Stop/Restart, launches): keyboards don't take part.

## Not in scope

MPE, pitch bend, aftertouch, MIDI clock, arpeggiator, split/layer zones, per-channel routing.
