# Glue Bus (`kabl.glue-bus` v1)

Catalogue section C5 in `docs/pivot/composite-catalogue.md`, roadmap row CB4. The master
colour and dynamics stage of the grooves and pieces: drive into a ladder tone filter into a
trim into a compressor into a second compressor that the kick ducks through its sidechain. 16
browser patches build it by hand.

**Status: prototype.** The interface below is the catalogue's proposal, built so it can be
judged on captures and renders. A definition is immutable once it ships (D09), so names,
defaults and the face may still change before this version is released.

## One definition

The catalogue has two variants: with the ducked second compressor (10 patches) and with a
single compressor (6). One definition covers both. With Duck In unpatched, the second
compressor passes the music sample for sample, so the single-compressor patches are this card
with Duck In left open (test `duck_in_ducks_and_unpatched_it_is_the_single_compressor_chain`).
Duck is not a dead control: it sets the duck's threshold whenever something is patched to Duck
In, as a sidechain threshold does on any compressor.

Duck In feeds the second compressor's `sc` through a `gain` at 0 dB. A compressor whose `sc` is
unpatched falls back to its own detector, so wiring Duck In straight to `sc` would compress the
music a second time, at the Duck threshold, whenever nothing is patched. The gain's output is
always a signal, silent when nothing reaches Duck In, and a silent sidechain turns nothing down.

## Inside

As `groove` in `tools/examples/presets_groove.rs` builds the bus, with Corner Store's settings
(`patches/presets/corner-store`):

- `drive`: 4 dB, its own trim −2 dB;
- `filter.ladder`: cutoff 1600 Hz, resonance 0.2, drive 2 dB;
- `gain` (Trim): −2 dB;
- `compressor`: threshold −24 dB, 4:1, attack 4 ms, release 110 ms, knee 4 dB, makeup 4 dB;
- `gain` (Duck In's feed): 0 dB, into the second compressor's `sc`;
- `compressor` (the duck): threshold −34 dB, 4:1, attack 1 ms, release 180 ms, knee 3 dB, no
  makeup. Its `out` is Out.

The grooves store the duck's ratio at 1:1 and let their Pump macro raise it; here it is 4:1, so
the card ducks with no macro. The drive's trim, the ladder's drive, the knee and the makeup stay
at these values and are reachable through Inspect internals.

## Interface

Ports (interface keys 1–3):

| port | kind | inside |
|---|---|---|
| In | audio in | `drive.in` |
| Duck In | audio in | the feed `gain.in`, into the duck's `sc` (the kick) |
| Out | audio out | the duck compressor's `out` (stereo) |

Controls (interface keys 4–12):

| control | default | range | unit | inside |
|---|---|---|---|---|
| Drive | 4 | 0–36 | dB | `drive.drive_db` |
| Tone | 1600 | 20–20000 (exponential) | Hz | `filter.ladder.cutoff_hz` |
| Resonance | 0.2 | 0–1 | | `filter.ladder.resonance` (off the face) |
| Trim | −2 | −60–24 | dB | `gain.gain_db` |
| Threshold | −24 | −60–0 | dB | first `compressor.threshold_db` |
| Ratio | 4 | 1–20 | :1 | first `compressor.ratio` |
| Attack | 4 | 0.1–200 (exponential) | ms | first `compressor.attack_ms` |
| Release | 110 | 5–2000 (exponential) | ms | first `compressor.release_ms` |
| Duck | −34 | −60–0 | dB | the duck `compressor.threshold_db` |

Every control uses its parameter's own range and curve, so no control needs a mapping (G2). A
stored value outside a control's range loads as stored, unclamped (test
`values_outside_a_control_range_are_stored_unclamped`, with Trim at −70 dB and the makeup at
−2 dB).

## Face

720 × 340 points:

- **Jacks**, along the top: In and Duck In at 164 and 242 points, clear of the card's name and
  as far apart as Drum Kit's Out and Kick, so the cables from a Drum Kit above run parallel;
  Out at the right. In the rack capture Glue Bus sits 90 points right of Drum Kit: a cable
  that hangs straight down crosses the label over the jack it ends in.
- **Displays**, in the middle band: the ladder's response over Drive and Tone, the first
  compressor's curve and gain reduction over Threshold to Release, the duck's over Duck.
- **Knobs**, in one row along the bottom: Drive, Tone | Trim | Threshold, Ratio, Attack,
  Release | Duck. Knobs, not sliders: a face slider follows the player's slider orientation
  setting and collapses in a short band.

Resonance is the catalogue's advanced control, so it is off the face: it is in the design
page's list and in Inspect internals. No artwork.

## Differences from the catalogue proposal

- **One definition for both variants** (see above), with a unity `gain` in front of the duck's
  `sc`.
- **Duck's ratio is fixed at 4:1.** The catalogue exposes the duck's threshold only; the grooves
  vary its ratio through the Pump macro. Migrating them (CM1) may need a Duck Ratio control.
- **The jacks are along the top**, where the other factory cards have them along the bottom: a
  bus takes its input from the cards above it.
- **No drive bypass.** The catalogue writes the drive as optional; here it is always in the
  chain, and Drive at 0 dB is its lightest setting. Whether that is close enough for the
  drive-less patches is a question for CM1.

## Regenerating

`cargo run -p kabl-tools --example glue_bus_composite -- write composites` writes
`composites/glue-bus/v1.json`; two runs write the same bytes. `wav DIR` renders eight seconds of
a clock at 120 BPM into Drum Kit into Glue Bus into the output: `glue-bus.wav` with Drum Kit's
Kick on Duck In, `glue-bus-no-duck.wav` without, and the dry loop as `drum-kit.wav`. The
captures are the `chrome_capture` scenes `glue-bus-rack`, `glue-bus-face` and
`glue-bus-design`: Drum Kit with Glue Bus stacked under it.

## Tests

`crates/ui/tests/composites_glue_bus.rs`:

- `the_committed_glue_bus_is_what_the_generator_writes`: the generator's bytes equal the file.
- `glue_bus_has_the_catalogue_interface`: ports with their keys and inner ends, controls with
  their parameter, range and default, as listed above; Duck In's gain feeds the duck's `sc`;
  Resonance alone is off the face.
- `glue_bus_passes_a_signal_and_keeps_silence_silent`: a saw into In comes out; with nothing on
  In, the output RMS is below 1e-6.
- `duck_in_ducks_and_unpatched_it_is_the_single_compressor_chain`: a full-scale sine on Duck In
  turns Out down by more than 6 dB (25 dB measured); unpatched, Out matches drive → ladder →
  trim → compressor built flat with the same settings within 1e-6.
- `glue_bus_plays_inside_a_parent_composite`: the same patch wrapped in a parent composite
  renders the same samples.
- `values_outside_a_control_range_are_stored_unclamped`: G2, as above.

`composites_factory.rs` `every_shipped_factory_definition_inserts` inserts it from the bank.
