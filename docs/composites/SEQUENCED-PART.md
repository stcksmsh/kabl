# Sequenced Part (`kabl.sequenced-part` v1, `kabl.sequenced-part-sub` v1)

Catalogue section C3 in `docs/pivot/composite-catalogue.md`, roadmap row CB3. A self-playing
line: a step sequencer into a voice into a level into a tone filter. 32 browser patches build
it by hand, 80 times.

**Status: prototype.** The interface below is the catalogue's proposal, built so it can be
judged on captures and renders. A definition is immutable once it ships (D09), so names,
defaults and the face may still change before these versions are released.

## Two definitions

The catalogue's families A and B differ in wiring, so each has its own definition. One
definition for both would carry the sub-oscillator and mixer of B in every A line, where it
plays nothing, and B's sub needs no control of its own (its level is part of the sound, as in
every groove). Family C (the learning pieces) stays flat, as the catalogue says.

| definition | family | inside | defaults from |
|---|---|---|---|
| `kabl.sequenced-part` "Sequenced Part" | A, piece line | seq → osc → vca (env) → gain → svf | Laundromat's first stab |
| `kabl.sequenced-part-sub` "Sequenced Part + Sub" | B, groove part | seq → osc + sub osc → mixer → vca (env) → gain → svf | Corner Store's bass |

### Sequenced Part (family A)

As `add_line` in `tools/examples/presets_piece.rs` builds a line with a tone filter and no sub:

- `seq`: 16 steps, gate 50 %, timed gates (`gate_mode` 1); the pattern
  `- - 9 - - 9 - - - - 9 - 12 - - -` (semitones, `-` a rest), velocity 70 on every note;
- `osc.va`: square, pulse width 35 %, 2 unison voices, detune 14, an octave below C4 (130.8 Hz);
- `env.adsr` 1 / 200 / 0.7 / 40 into the `vca` (`gain` 0, so the envelope opens it);
- `gain` −6 dB;
- `filter.svf` low-pass at 3000 Hz, resonance 0.1, its `lp` is Out.

These are Laundromat's (`patches/presets/laundromat`) first stab. The catalogue's modal values
(Level −8 dB, Tone 1400 Hz) are not any one patch's line; Laundromat's is the one the project
calls its sound.

### Sequenced Part + Sub (family B)

As `seq_part` in `tools/examples/presets_groove.rs` builds a part with a low-pass, no glide and
no formant:

- `seq`: 16 steps, gate 55 %; the pattern `0 - 12 0 - 3 - 5 7 - 5 - 3 - 0 -`;
- `osc.va`: saw, 1 voice, detune 6, two octaves below C4 (65.4 Hz);
- a second `osc.va`, a square an octave below that, on the same pitch;
- `mixer`: the oscillator at 1.0, the sub at 0.3;
- `env.adsr` 1 / 140 / 0.4 / 40 into the `vca`;
- `gain` −3.1 dB (level 0.7);
- `filter.svf` low-pass at 8000 Hz, resonance 0, its `lp` is Out.

These are Corner Store's (`patches/presets/corner-store`) bass. Grooves without a low-pass get
the filter here too: the catalogue's Tone control needs it, and fully open it passes the part.

In both, banks B to D of the sequencer hold the module's defaults. Other settings, such as the
pulse width or the sub's level, stay at those values and are reachable through Inspect
internals.

## Interface

The same in both definitions. Ports (interface keys 1–8):

| port | kind | inside |
|---|---|---|
| Clock | gate in | `seq.clock`: one step per pulse (a 16th from `clock`) |
| Reset | gate in | `seq.reset`: the next step is step 1 |
| Out | audio out | the tone filter's `lp` |
| Pitch out | pitch out | `seq.pitch`, for chaining another voice |
| Gate out | gate out | `seq.gate` |
| Gate mod | modulation in | route into `seq.gate_len` |
| Tone mod | modulation in | route into the tone filter's `cutoff_hz` |
| Level mod | modulation in | route into `gain.gain_db` |

Controls (interface keys 9–20), with family A's default first and B's second:

| control | default | range | unit | inside |
|---|---|---|---|---|
| Steps | 16 / 16 | 1–32 (stepped) | steps | `seq.length` (bank A) |
| Gate | 50 / 55 | 1–100 | % | `seq.gate_len` |
| Transpose | 0 / 0 | −24–24 | st | `seq.transpose` |
| Wave | square / saw | sine, triangle, saw, square | | `osc.va.waveform` |
| Unison | 2 / 1 | 1–16 (stepped) | voices | `osc.va.unison` |
| Detune | 14 / 6 | 0–100 | | `osc.va.detune` |
| Attack | 1 / 1 | 0.1–10000 (exponential) | ms | `env.adsr.attack_ms` |
| Decay | 200 / 140 | 0.1–10000 (exponential) | ms | `env.adsr.decay_ms` |
| Sustain | 0.7 / 0.4 | 0–1 | | `env.adsr.sustain` |
| Release | 40 / 40 | 0.1–10000 (exponential) | ms | `env.adsr.release_ms` |
| Level | −6 / −3.1 | −60–24 | dB | `gain.gain_db` |
| Tone | 3000 / 8000 | 20–20000 (exponential) | Hz | `filter.svf.cutoff_hz` |

Every control uses its parameter's own range and curve, so no control needs a mapping (G2). In
the sub part, Wave, Unison and Detune are the main oscillator's; Transpose moves both
oscillators, because it shifts the pitch they share.

**Cues.** A cue addresses a sequencer by its module id (`cueN.seq.<id>`), and the sequencer
inside the card keeps its leaf id, so a cue launches its banks with the card closed. Insert and
duplicate remap the reference.

## Face

840 × 340 points, the widest face a panel allows:

- **Sequencer**, along the top: Steps as a selector of its 32 lengths (576 points, the 18 per
  choice a selector needs), with Gate and Transpose as knobs at the right.
- **Voice**, in the middle: at the left, Wave's four waveform glyphs beside the oscillator's
  display (its waveform, with a playhead while audio runs) and Unison's 16 choices under them;
  then Detune and the envelope knobs, Attack, Decay, Sustain, Release.
- **Output**: Level and Tone at the bottom right, under Gate and Transpose.
- **Jacks**, along the bottom left, in interface order, 80 points wide each.

Steps and Unison are stepped parameters, which take selectors, not knobs; together they need
864 points, more than a row holds, so they sit in different bands. That leaves no room for an
envelope or filter display. Gate and Transpose are knobs, not sliders: a face slider follows
the player's slider orientation setting, and a vertical slider needs more height than the top
band has. No artwork.

## Differences from the catalogue proposal

- **Octave is Transpose** (`seq.transpose`, ±24 semitones). The catalogue lists "Octave (osc)",
  but the oscillator has only `base_hz` (20–20000 Hz) and a control has one target with no
  mapping (G2), so a knob there would move one oscillator of the sub part away from the other.
  The sequencer's transpose moves both and the Pitch out with them.
- **The Pitch and Gate outputs are there**, named "Pitch out" and "Gate out": a port and a
  control may not share a label, and the control is the catalogue's "Gate".
- **Modulation ports are named "Gate mod", "Tone mod", "Level mod"**, as Drum Kit names its
  own.
- **Family B always has the tone filter** (the catalogue says "± svf").
- **No clock divider.** Family A's clock-divided pad puts a `clock.div` at the input; here the
  divider stays outside the card, in front of Clock.
- **No pattern editing on the face** (gap G6): the steps are edited through Inspect internals.

## Regenerating

`cargo run -p kabl-tools --example sequenced_part_composite -- write composites` writes
`composites/sequenced-part/v1.json` and `composites/sequenced-part-sub/v1.json`; two runs
write the same bytes. `wav DIR` renders eight seconds of a clock at 120 BPM into each
definition into the output. The captures are the `chrome_capture` scenes
`sequenced-part-rack`, `sequenced-part-face`, `sequenced-part-design` and the same three for
`sequenced-part-sub`.

## Tests

`crates/ui/tests/composites_sequenced_part.rs`, each over both definitions:

- `the_committed_sequenced_parts_are_what_the_generator_writes`: the generator's bytes equal
  the files.
- `sequenced_parts_have_the_catalogue_interface`: ports with their keys and inner ends, controls
  with their parameter, range and default, as listed above.
- `sequenced_parts_alone_play_when_clocked`: clock → part → out sounds; without the clock it is
  silent.
- `both_oscillators_of_the_sub_part_sound`: with only one of the two cables into the sub
  part's mixer left, for each, the part still sounds.
- `sequenced_parts_play_inside_a_parent_composite`: the same patch wrapped in a parent
  composite renders the same samples.
- `a_cue_launches_the_inner_pattern_with_the_part_closed`: a cue targeting the inner sequencer
  switches it to bank B (rests) on the next bar while the card is closed.

`composites_factory.rs` `every_shipped_factory_definition_inserts` inserts them from the bank.
