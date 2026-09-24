# Sound palette: hands-on checklist

For Kosta's listening and keyboard review. Everything here was driven by scripts in a cloud
container (virtual display, fifo controller, silent sink); none of it was heard on speakers
or played on a hardware keyboard. Settings: 48 kHz / 256 frames.

    cargo run --release -p kabl-ui -- --patch patches/palette/lead --perform --rate 48000 --frames 256
    cargo run --release -p kabl-ui -- --patch patches/sound-palette --perform --rate 48000 --frames 256

Patch guide (controls, CCs): [PATCHES.md](PATCHES.md). Macros are CC 20–23 on channel 1.

## Sound (one patch at a time, `patches/palette/<name>`)

- [ ] **strings**: slow chords sound like a moving ensemble, not a static saw; Ensemble (CC 21)
  widens and thickens; Bow (CC 22) slows the attack; no beating that sounds out of tune.
- [ ] **pad**: warm, evolving, the PWM audible as slow movement; Evolve (CC 21) deepens the
  breathing; Air (CC 22) adds noise without hiss dominating; long release rings cleanly.
- [ ] **lead**: sings; the synced pulse gives a vowel on each attack; Vibrato (CC 21) is
  musical at mid travel; Bite (CC 22) adds grit without harshness at high notes (drive is
  ADAA only, no oversampling: listen above C6).
- [ ] **bass**: resonant, pitched, punchy on accents; Cutoff/Resonance (CC 20/21) sweep
  without whistling out of control; Drive (CC 23) thickens without mush.
- [ ] **breath**: sounds like air/wind with a pitch, not like a whistle; chords blow as
  separate voices (each voice has its own noise); Gust (CC 21) moves like wind.
- [ ] **perc**: kick, snare, hat distinct; Decay/Tone/Crunch/Room (CC 20–23) each useful.
- [ ] Patches differ by their synthesis, not only by their reverb (try Space/Echo/Room at 0).
- [ ] High notes: saws above ~C6 (PolyBLEP) — any audible aliasing you mind?
- [ ] Loudness: poly patches are quieter per note than the lead; chords stay clean (no clip
  light) on six-note chords. The meter is peak-only; there is no limiter.

## Keyboard feel (rules: [keyboard.md](keyboard.md))

- [ ] **POLY** (strings/pad/breath): chords, re-striking a sounding note, the sustain pedal
  (CC 64) holding released keys; more than 8 notes steals the oldest.
- [ ] **LEGATO** (lead): overlapping notes glide and don't re-attack; detached notes re-attack
  and don't glide; releasing the top note of a held pair returns to the other.
- [ ] Switch the lead to **MONO** on its face: every note re-attacks; **LOW** and **HIGH**
  priority (MIDI In advanced area, `+2`) pick the lowest/highest held key.
- [ ] Glide **ALWAYS** vs **LEGATO**; Glide time (CC 24 in the lead patch) from snappy to slow.
- [ ] The MIDI In face line (e.g. `LEGATO · LAST · glide 90 ms`) always says what the keys do.
- [ ] No stuck notes: unplug the controller with keys held, plug back; All notes off in the
  Perform panel; Load with keys held; live edits and transport with keys held. The status
  hover shows notes held / voices sounding.
- [ ] Sustain pedal, CC 120/123 never get MIDI-learned (try Learn and press the pedal).

## Controls and UI (1440×900 and `--size 1280x800`, A-light and A-dark)

- [ ] MIDI In face: keys picture, the settings line, Mode and Glide selectors readable.
- [ ] VA Oscillator advanced area: Pulse width, Fine, Unison 1–4, Detune.
- [ ] Ladder (Drive in its advanced area), Chorus (Width advanced), Drive (Mix advanced),
  Noise (Color, Level).
- [ ] Macros: named, pinned, learnable; routes from them show on the destinations' rings.
- [ ] Perform panel of the piece: cues, four macros, four layer faders, echo, glide time fit
  and read clearly.

## The piece (`patches/sound-palette`)

- [ ] Listen to `audio/sound-palette-take.m4a` (scripted, ~9½ min).
- [ ] Play it yourself: cue buttons CC 40–44 (Air, Pulse, Groove, Lift, Break), layer
  faders CC 24–27, lead echo CC 28. Every layer hears every key; raise the one you want.
- [ ] Watch the status bar for late callbacks / xruns while playing dense chords with all
  layers up and turning knobs (the piece is the heaviest patch so far; see README).
