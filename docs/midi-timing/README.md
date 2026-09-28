# D06 — expressive, correctly timed MIDI

**Status:** engineering submitted on branch `codex/d06-expressive-midi` (see [REPORT](REPORT.md)
for the exact commits, PR and verification); independent review in [REVIEW](REVIEW.md); owner
review **pending** ([CHECKLIST](CHECKLIST.md)). Design written before the code:
[design.md](design.md).

## What changed for a player

- **Pitch bend.** Every MIDI In has a **Bend** range (0–24 semitones, default ±2, saved with the
  sound, in its advanced area). The face's second line shows it (`bend ±2 st`). Center is
  exactly no bend; full up/down reach the range. Held, sustained and releasing notes follow;
  mono glide and bend add.
- **Modulation wheel.** MIDI In has a **Wheel** jack (0–1). Route it to any knob like any
  modulation. It is not a CC mapping: no pickup, no soft takeover. A knob MIDI-learned to CC 1
  keeps working exactly as before, with soft takeover.
- **Two keyboards played apart.** MIDI In's **Channel**: ALL (default, every keyboard hears every
  note as before) or 1–16. Notes held when you change it are still released by their own
  note-off; the pedal and notes-off reach whoever holds the notes.
- **Cleanup.** Sustain is per channel. CC 120/123 end that channel's notes; CC 121 resets bend,
  wheel and pedal; unplugging the controller ends only its notes and expression; the panel's
  All notes off, Load and Retry still end everything.
- **Timing.** Notes, releases, bends and wheel moves land on their own sample inside the
  engine's 64-sample blocks. Offline, the same performance renders bit-identically with any
  host buffer size; live, each MIDI message keeps its position within the callback period at a
  constant delay of one callback period + 64 frames (about 6.7 ms at 48 kHz / 256).
- **Demo sounds.** *Expressive Lead* (`patches/expressive/lead`: bend ±2, wheel → vibrato) and
  *Keys and Bass by Channel* (`patches/expressive/split`: channel 1 keys with wheel → filter,
  channel 2 legato bass with ±12 st bend).

Supported dialect and limits: design.md section 7. Not supported: MPE, aftertouch, RPN bend
range, CC 33 LSB, host note ids, split zones, several controllers at once.

## Evidence

Everything below is labelled by kind. None of it is physical-controller or listening evidence.

- **Offline renders** (production `Timeline`, `crates/engine/examples/render_midi.rs`): see
  REPORT "Listening files".
- **Live app recordings** (the app's own recorder, scripted virtual MIDI from
  `examples/midi_player`): REPORT "Laptop measurements" and the walkthrough.
- **Screenshots** of the real app: `img/`.

## Reproduce

    cargo test -p kabl-engine --test timeline --test expression   # partition invariance, expression
    cargo test --workspace
    cargo +1.98.1 clippy --workspace --all-targets -- -D warnings
    python3 docs/midi-timing/scripts/make_demos.py docs/midi-timing/scripts
    cargo run --release -p kabl-engine --example render_midi -- patches/expressive/lead \
        docs/midi-timing/scripts/lead.events lead.wav --frames irregular
    python3 docs/midi-timing/scripts/onsets.py TAKE.wav docs/midi-timing/scripts/pulse.events
    BASE_BIN=/path/to/baseline/kabl-ui OUT=/tmp/d06-measure DISPLAY=:97 \
        docs/midi-timing/scripts/measure-all.sh                     # laptop matrix (audible)

Patches are rebuilt by `cargo test -p kabl-ui --test expressive write_expressive_patches --
--ignored` and the gate probe by `... write_gate_probe -- --ignored`.
