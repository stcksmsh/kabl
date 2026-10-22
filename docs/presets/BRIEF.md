# Factory presets: sound-direction brief

Every factory preset author follows this page. The pilot (eight presets, `patches/presets/`) is the proof that the rules below can be built with the modules on master; the bank is about 100 presets in the same shape.

## Identity

kabl's factory bank is a **French-touch and 80s-synth instrument whose patches can also play themselves.** References: **Justice**, **Daft Punk**, **M83**. Take the sound, not the songs: no preset is a copy of a track, no name quotes one.

- From **Justice**: distorted, filtered, pumping bass and stabs; hard drive into a resonant filter; compression that breathes with the beat; phased and flanged chords; chorused saws.
- From **Daft Punk**: the disco and filter-house side: phased chord loops, filter sweeps and builds that open over bars, vocoder and vowel voices, robotic character, bright FM keys.
- From **M83**: the wide 80s side: supersaw pads, big gated and long reverbs, slow blooming filters, arpeggios under chords, lush chorus.

A preset is an instrument first: it plays from a MIDI keyboard the moment it loads. Internal sequencers, arpeggiators and cues are used where they suit the sound, not in every patch.

## Categories and target counts

The category goes in the preset's metadata and is the Sounds browser's category filter. `Groove` and `Voice` were added to the library vocabulary for the bank.

| Category (browser) | Count | Brief |
|---|---|---|
| Bass, Lead | 25 | Mono or legato. Distorted, filtered, pumping bass (drive before a resonant ladder, a filter envelope that snaps, a compressor after). Wide detuned leads with glide, echo and room. Some are played with the mod wheel opening the filter. |
| Keys | 20 | Poly. Disco stabs and phased chords on the off-beat, chorused electric pianos, FM keys with a bright tine over a mellow body. Short envelopes; effects sit on the chord, not on single notes. |
| Pad | 20 | Poly. Supersaws (stacked detuned saws), slow filter bloom, long attack and release, chorus, echo and a long room. The M83 side. |
| Groove | 15 | Arpeggiators and sequences that play with a held chord or on their own: arpeggios over a pumping chord, bass loops, rhythmic gates. Start on Run. |
| Piece | 10 | Self-playing pieces with cues: a build-up, a drop, a break. Cues move the four macros; the sequencers keep playing. A hand-played voice sits on top. |
| Voice | 10 | Vowel, robot, metal and glass voices: formant filter, bitcrusher, ring modulation, comb; a vocoder when it lands. |

## Macros: the same four everywhere

Every preset has exactly one `macro` module with these four, in this order, on CC 20-23, pinned in the Perform panel in this order (transport and cue pads first when the patch has them):

| Macro | CC | Means | Examples from the pilot |
|---|---|---|---|
| **Filter** | 20 | Where the spectrum sits: brightness or spectral position. 0 is dark or closed, 1 is open. | Ladder cutoff; FM operator index plus a lowpass; the vowel position of the formant filter. |
| **Drive** | 21 | Harmonic intensity: grit, weight, crush. 0 is clean. | Drive module dB; the bitcrusher mix on the robot voice; drive into the pad saws. |
| **Space** | 22 | The wet amount of the echo and the room. 0 is dry. | Delay mix and reverb mix together. |
| **Motion** | 23 | How much the sound moves by itself. 0 is still. | LFO depth on the filter (vibrato on a lead), phaser or chorus mix and rate, the pump depth of a chord, the vowel wander. |

The point: a sequencer, a cue or a hand can steer any preset the same way. A cue that says "Filter 0.2 to 0.9 over four bars" builds any preset in the bank.

Rules:

- Names are exactly `Filter`, `Drive`, `Space`, `Motion` (labels `name.m1` to `name.m4`).
- Every macro moves something audible over its whole range. A macro that has nothing to do in a preset still gets a small, honest effect on the nearest sound (a pad's Drive thickens the saws); do not leave a dead macro.
- The macro value stored in the patch is the sound as designed. Cues and automation move it from there.
- Motion depth is the LFO through a VCA that the macro opens (`lfo` to `vca.in`, macro `m4` to `vca.cv`, the VCA output as the route source), because a macro cannot scale a route's amount directly.

## Wet and dry: effects baked in, reachable on Space

Effects are part of the sound: a preset ships with its chorus, phaser, echo and room already on. The rule that keeps them reachable:

- **Echo and room mix are 0 on the module and driven by the Space macro** (route amount = the largest mix wanted). The macro's stored value is the baked-in wet amount; dragging Space to 0 gives the dry sound, to 1 the wettest.
- **Character effects** (chorus, phaser, bitcrusher) that define the sound sit at their working mix on the module, or, when they should be dialled out, are driven by Motion or Drive and are 0 on the module.
- Never bake an effect in so deep that no macro can take it out. The pilot supports this working assumption: Space at 0 gave usable dry sounds in all eight; nothing argued against it.

## Levels

The level rule is `docs/factory-levels/README.md`: no sample above -1 dBFS, loudness -19 dBFS RMS within +-5.5 dB (aim for +-3), rendered the way the preset demonstrates itself (a held chord for keys, the clock run for sequences). Level a preset with its `gain` module (or a mixer level) only; do not change the sound to fix the level. `factory_levels_hold_the_target` renders every patch under `patches/`, so a new preset is checked as soon as it is saved.

Presets with cues are leveled at their stored macro values (the first cue); say in the description if the drop is louder.

## Names

- Evocative, two words, in the style of "Iron Bell" and "Rust Bass": an image or a place, not a spec. Pilot: Cherry Chainsaw, Neon Highway, Roller Disco, Violet Hour, Glass Elevator, Chrome Chase, Midnight Drop, Android Choir.
- No artist, album or track names, no trademarks, no "Pad 1". Unique across the bank; at most 60 characters.
- The folder is the name in lower case with hyphens: `patches/presets/cherry-chainsaw/`.

## Metadata: how a preset declares itself

`sound.toml` in the preset folder (the Sounds browser reads it; the same file `Meta` in `crates/ui/src/library.rs` describes):

```toml
name = "Cherry Chainsaw"
category = "Bass"                      # one of CATEGORIES in library.rs
tags = ["mono", "distorted", "filtered", "pumping", "french touch"]
description = "..."                    # what it is, then what each macro does
keys = true                            # derived: the patch has a midi.in
sequence = false                       # derived: the patch has a clock
```

- `keys` and `sequence` are derived from the patch (`Meta::play_of`); write them from the patch, never by hand. Every preset has `keys = true`, even a self-playing one (a held chord or a hand-played line on top).
- Tags, in this order: how it plays (`mono`, `poly`, `legato`, `arp`, `sequenced`), what it is made of or does (`supersaw`, `fm`, `phased`, `distorted`, `vowel`, `cues`), an era or style (`80s`, `french touch`, `disco`) and, for a clocked preset, its tempo (`118bpm`). Lower case, a few words each, three to six tags.
- The description is two or three sentences: what it is, how to play it, then one clause per macro in Filter, Drive, Space, Motion order.

## How a preset is made and added

1. Build it from existing modules in code (the pilot's generator is `crates/ui/examples/presets_pilot.rs`: one function per preset, a `Preset` entry with its metadata, a clip phrase) and write it to `patches/presets/<slug>/`.
2. Add one line to the `FACTORY` list in `crates/ui/tests/library.rs`: the browser test asserts that nothing is missing or stray.
3. Level it (`level_table`, then the `gain` module).
4. Run the checklist below.

Patch format and schema are unchanged. A preset only uses modules that exist on master.

## Checklist: a preset passes only if

1. Plays from MIDI on load (`midi.in`, or an `arp` fed by held keys with a `midi.in` beside it), with sensible bend range (2 semitones) and mono or legato for bass and lead.
2. A self-playing preset starts on Run (the transport is pinned first) and says so in its description.
3. Exactly the four macros, named, on CC 20-23, pinned in order, each audibly doing its job from 0 to 1 (`every_preset_keeps_to_the_brief` checks the names, CCs, pins, flags, category, tags and description).
4. Space at 0 is dry; the stored Space value is the designed wet amount.
5. Category from the vocabulary; a name that follows the rules; 3 to 6 tags; a description with one clause per macro.
6. Level: peak under -1 dBFS and -19 +-5.5 dBFS loudness (`factory_levels_hold_the_target`).
7. Loads, compiles and renders without a note in the Sounds browser (`every_factory_sound_is_discoverable_categorized_and_loads`).
8. No module added or changed; no dead macro; no clipping with all four macros up.
9. Checked in the standalone and in REAPER.
10. A clip of the preset played as meant (a short MIDI phrase, or the self-playing preset running) with one line on what to listen for.

## What the pilot found

See `docs/presets/PILOT.md` for the pilot list, the evidence and the gaps.
