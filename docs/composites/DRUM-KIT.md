# Drum Kit (`kabl.drum-kit` v1)

Catalogue section C4 in `docs/pivot/composite-catalogue.md`, roadmap row CB2. A drum machine:
`drum.grid` playing kick, snare, clap, hats (closed and open) and tom into a submix. 23 browser
patches build this kit by hand.

**Status: prototype.** The interface below is the catalogue's proposal, built so it can be
judged on captures and renders. A definition is immutable once it ships (D09), so names,
defaults and the face may still change before this version is released.

## Source

The inner graph is the kit of the factory pieces (`add_kit` in `tools/examples/presets_piece.rs`,
the catalogue's four-stage variant), because it is the only variant with a bus for the
catalogue's Bus Tone and Hats/Bus:

- `drum.grid` rows kick, snare, clap, chat, ohat, tom gate `drum.kick`, `drum.snare`,
  `drum.clap`, `drum.hat` `closed` and `open` (so the open hat chokes), and `drum.tom`;
- the kick into its own mixer (level 0.7);
- clap, hats and snare into the bus mixer (in1 clap 0.5, in2 hats 0.3, in3 snare 0.5), through a
  `filter.ladder` at 20 kHz, resonance 0.2: open, as every piece stores it until a macro closes it;
- the kick mixer, the tom (0.3) and the ladder into the kit mixer, which is Out.

Voice settings are Laundromat's kit (`patches/presets/laundromat`), except that Laundromat mutes
the snare in the bus (level 0) and this kit plays it at 0.5. Pattern A of the grid is a
four-on-the-floor beat with a backbeat snare and clap, eighth closed hats, offbeat open hats and
one tom hit; patterns B to D are empty. Rows play 16 steps. Other settings, such as the kick
level or tom decay, stay at those values and are reachable through Inspect internals.

## Interface

Ports (interface keys 1–10):

| port | kind | inside |
|---|---|---|
| Clock | gate in | `drum.grid.clock`: one step per pulse (a 16th from `clock`) |
| Reset | gate in | `drum.grid.reset`: the next step is step 1 |
| Out | audio out | the kit mixer's `out` |
| Kick | audio out | `drum.kick.out`, for a sidechain |
| Hats/Bus | audio out | the ladder's `out`: clap, hats and snare, for a space send |
| Kick mod | modulation in | route into the kit mixer's `level1` (the kick's share) |
| Clap mod | modulation in | route into the bus mixer's `level1` |
| Hat mod | modulation in | route into the bus mixer's `level2` |
| Tom mod | modulation in | route into the kit mixer's `level2` |
| Tone mod | modulation in | route into the ladder's `cutoff_hz` |

The five modulation inputs are the levels and the cutoff that the macros of the pieces and
grooves drive (`rise(.., kit.out, "level1")`, `kit.top` `level1`/`level2`, the hat level of the
groove kits, the bus ladder in five pieces).

Controls (interface keys 11–20):

| control | default | range | unit | inside |
|---|---|---|---|---|
| Kick Tune | 55 | 30–120 (exponential) | Hz | `drum.kick.tune_hz` |
| Kick Decay | 250 | 60–2000 (exponential) | ms | `drum.kick.decay_ms` |
| Kick Drive | 8 | 0–30 | dB | `drum.kick.drive_db` |
| Snare Decay | 200 | 60–1200 (exponential) | ms | `drum.snare.decay_ms` |
| Snare Level | −8 | −24–6 | dB | `drum.snare.level_db` |
| Clap Level | −10 | −24–6 | dB | `drum.clap.level_db` |
| Hat Level | −16 | −24–6 | dB | `drum.hat.level_db` |
| Tom Tune | 120 | 60–500 (exponential) | Hz | `drum.tom.tune_hz` |
| Tom Level | −10 | −24–6 | dB | `drum.tom.level_db` |
| Bus Tone | 20000 | 20–20000 (exponential) | Hz | `filter.ladder.cutoff_hz` on the bus |

Every control uses its parameter's own range and curve, so no control needs a mapping (G2).

## Face

840 × 340 points, the widest face a panel allows. One column per voice, its display on top
(the hit's envelope from the voice's own parameters, with a playhead while audio runs) and its
knobs below: Kick (Tune, Decay, Drive), Snare (Decay, Level), Clap · Hats (Clap Level, Hat
Level, under the hat's closed/open display), Tom (Tune, Level), then Bus Tone. The ten jacks
run along the bottom in interface order. Displays are 60 points high, the height that fits
three rows in the face. Knobs are 70 wide and jacks 78, except Snare Decay (74) and Hats/Bus
(84), which are widened so their labels print whole. Clap has no display of its own because a
display is at least 120 points wide and the five columns fill the width. No artwork.

## Differences from the catalogue proposal

- **The ladder is always there.** The catalogue says Bus Tone is the ladder "where present";
  the 2-stage groove kits have none. Here it is part of the kit at 20 kHz, open.
- **Modulation ports are named "Kick mod", "Clap mod", "Hat mod", "Tom mod", "Tone mod".** The
  catalogue only says "the level ports that macros drive". The level ones land on mixer levels,
  not on the voices' `level_db` that the Level controls set: those are what the macros drive.
- **No pattern editing on the face** (gap G6): the grid is edited through Inspect internals.

## Regenerating

`cargo run -p kabl-tools --example drum_kit_composite -- write composites` writes
`composites/drum-kit/v1.json`; two runs write the same bytes. `wav DIR` renders eight seconds
of a clock at 120 BPM into Drum Kit into the output. The captures are the `chrome_capture`
scenes `drum-kit-rack`, `drum-kit-face` and `drum-kit-design`.

## Tests

`crates/ui/tests/composites_drum_kit.rs`:

- `the_committed_drum_kit_is_what_the_generator_writes`: the generator's bytes equal the file.
- `drum_kit_has_the_catalogue_interface`: ports and their inner ends, controls with their
  parameter, range and default, as listed above.
- `drum_kit_alone_plays_when_clocked`: clock → Drum Kit → out sounds; without the clock it is
  silent.
- `every_drum_kit_voice_triggers`: with only one grid row's trigger cable left, for each of
  kick, snare, clap, closed hat, open hat and tom, the kit still sounds.
- `drum_kit_plays_inside_a_parent_composite`: the same patch wrapped in a parent composite
  renders the same samples.

`composites_factory.rs` `every_shipped_factory_definition_inserts` inserts it from the bank.
