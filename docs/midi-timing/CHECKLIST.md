# D06 owner review (pending)

Use the branch/PR named in [REPORT](REPORT.md); do not merge until satisfied. Record pass/fail,
controller model and anything odd beside each item. Launch from the release bundle (`START-HERE.txt`)
or the repository:

    cargo run --release -p kabl-ui -- --patch patches/expressive/lead --perform --rate 48000 --frames 256

Choose your controller under Perform > MIDI in.

- [ ] **Bend range and center.** Expressive Lead: hold a note, push the bender fully up and down:
  exactly a whole tone (±2 st) each way; let it spring back: the pitch returns exactly to the
  note (no residual detune). Open the MIDI In's advanced area (**+4**), set **Bend** to 12:
  the same move is an octave; the face line reads `bend ±12 st`. Undo returns it to ±2.
  Result: ______
- [ ] **Bend on held and sustained notes.** With the sustain pedal down, play and release a
  few notes, then bend: every sounding note (and its release) moves. Result: ______
- [ ] **Wheel versus CC pickup.** Expressive Lead: the modulation wheel adds vibrato from 0
  and removes it at 0, immediately, with no pickup. Then pin any knob to Perform and MIDI-learn
  it to the wheel (CC 1): that knob still needs a pickup crossing (soft takeover) while the
  vibrato follows the wheel at once. Both move together; neither silently replaces the other.
  Result: ______
- [ ] **Sustain, repeated notes, release.** Keys and Bass by Channel on channel 1: pedalled
  chords hold after the keys go up and stop at pedal up; the same key repeated under the pedal
  re-attacks without piling up voices; fast repeated notes without the pedal each end. No
  stuck notes after All notes off. Result: ______
- [ ] **Independent destinations.** Keys and Bass by Channel: channel 1 plays only the keys,
  channel 2 only the bass (legato glide, ±12 st bend). Set the bass MIDI In's **Channel** to
  ALL: both layer again. Hold a channel-1 chord, change a keyboard's channel while holding,
  release: nothing sticks. Result: ______
- [ ] **Save and reopen.** Change Bend and Channel, Save As, quit, reopen the saved sound:
  the values and the wheel routes are back and play the same. Result: ______
- [ ] **Ordinary playing at 48 kHz / 256.** Play for a few minutes on both demo sounds and a
  factory patch; note any lag, flams, stuck notes, crackles or xruns (status bar and
  **Log**). The app now plays MIDI at a constant one-callback-plus-64-frames delay
  (about 6.7 ms at 256); say whether it feels different from before. Result: ______
- [ ] **Disconnect cleanup.** Hold notes with the pedal down and the wheel up, unplug the
  controller: its notes end, the vibrato returns to rest; replug and play on. Result: ______
- [ ] **D05 recovery with MIDI.** While holding notes, use **Retry audio** (or change the
  output): notes stop, nothing replays, playing again works at once. Record a take while
  playing bends; check the WAV. Result: ______
- [ ] **Listening files.** Play the three WAVs from the bundle (`listening/`): the lead's
  bends and vibrato, the keys' pedal/release tails and the ratchets; compare
  `timing-pre-d06-emulation.wav` with `timing.wav`. Result: ______

These are Kosta's hands-on dispositions. Scripted virtual-MIDI runs and offline renders are not
a substitute.
