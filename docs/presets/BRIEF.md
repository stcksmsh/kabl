# Factory presets: sound-direction brief

Every factory preset author follows this page. The pilot (eight presets, `patches/presets/`) shows that the rules below can be built with the modules on master; the bank is about 100 presets in the same shape.

## Identity

kabl is a **general-purpose instrument**. The factory bank has to fit whatever a player is making, so it shows range across genres and roles, not one style.

What is distinctive about kabl is not a genre. It is what the instrument can do that most synths cannot: patches that **play themselves** (sequencers, arpeggiators, clocks), **patterned cables** (a step pattern on a cable), **cue morphs** (a cue moves the macros over bars), and a **rack** you can open and rewire. The bank should show those, and should be a good playable bank before it shows anything else.

Within that, the bank has a **signature corner**: a French-touch direction (Justice, Daft Punk, M83 as references), roughly a fifth to a quarter of the bank (about 22 presets), tagged `signature`. The rest of the bank is not French touch and does not have to sound like it. Synthwave and the 80s are one genre of many and keep a few presets; they were over-represented in the first pilot.

Take sound, not songs: no preset is a copy of a track, no name quotes one.

## Roles and target counts

Categories are by role. The role is the preset's `category` in the Sounds browser.

| Role (browser category) | Count | Brief |
|---|---|---|
| Bass | 14 | Mono or legato. Sub and reese (hip-hop, drum and bass), round and plucked (pop, funk), distorted and compressed (the signature). Mod wheel opens the filter where it suits. |
| Lead | 12 | Mono, legato or hard-retriggered. Smooth and singing (pop, synthwave), hard and bitcrushed (signature), squelchy and acid, pure and modular. |
| Keys (keys and chords) | 16 | Poly. Clean electric pianos and organs (pop, house), disco stabs and phased chord loops (signature), FM keys, rhodes-like keys, house piano stabs. |
| Pad | 14 | Poly. Ambient and cinematic (slow, wide, long tails), supersaw and analog pads (synthwave, pop), thick and saturated pads (signature), evolving and textured pads. |
| Pluck (plucks and mallets) | 10 | Poly. FM and sampled-feel mallets, marimbas, kalimbas, harp and guitar-like plucks, short synth plucks for pop and trap. |
| Groove (grooves and arps) | 14 | Arpeggiators and sequences that play with a held chord or on their own, at a stated tempo: house and techno loops, pumping arps (signature), trap and breaks patterns, modular bleeps. Start on Run. |
| Piece | 10 | Self-playing pieces with cues: build-ups and drops (filter house, techno), ambient evolutions, cinematic swells, a pop loop with sections. Cues move the four macros; the sequencers keep playing. |
| Voice (voices and FX) | 10 | Vowel, formant, talkbox and robot voices, metal, glass and noise, risers and impacts. Vocoder presets when the module lands. `FX` becomes a category with the first FX preset. |

`Groove`, `Voice` and `Pluck` were added to the category vocabulary (`CATEGORIES` in `crates/ui/src/library.rs`) for the bank.

## Genre spread (working proposal)

The first column is the share of the bank (about 100 presets); the pilot picks one or more from most rows. Adjust if the bank turns out unbalanced; the reason for each row is the use it serves.

| Genre | About | Why it is there |
|---|---|---|
| House and techno | 14 | The most common thing players make; four-on-the-floor grooves, stabs, acid bass, builds and drops. |
| French touch and electro | 16 | The signature corner: filtered disco loops, dirty bass, pumping, talkbox. With a few house presets that follow the signature rules it makes the 22 `signature` presets. |
| Synthwave and 80s | 10 | A distinct, popular sound kabl does well: lush leads and pads, gated and arpeggiated; kept to ten so it does not read as the whole bank. |
| Pop | 12 | Clean, wide, playable keys, plucks, leads, pads and bass; the "just works" sounds. |
| Hip-hop and trap | 10 | Sub and 808-style bass, dark keys and plucks, bells, short stabs. |
| Drum and bass and breaks | 8 | Reese and neuro bass, rolling arps, pads, stabs. |
| Ambient and cinematic | 14 | Slow pads, evolving textures, drones, swells, mallets with long tails; kabl's self-playing patches suit this well. |
| Experimental and modular | 10 | Patterned cables, polymeter, probability, sample-and-hold, FM bells and metal: the rack as an instrument. |
| Funk, disco, world and other | 6 | Clavs, funky bass, strings and plucks that fit no row above. |

## Macros: the same four everywhere

Every preset has exactly one `macro` module with these four, in this order, on CC 20-23, pinned in the Perform panel in this order (transport and cue pads first when the patch has them):

| Macro | CC | Means | Examples from the pilot |
|---|---|---|---|
| **Filter** | 20 | Where the spectrum sits: brightness or spectral position. 0 is dark or closed, 1 is open. | Ladder cutoff; FM operator index plus a lowpass. |
| **Drive** | 21 | Harmonic intensity: grit, weight, warmth. A preset that is dirty by design stays dirty at 0 and gets dirtier. | Drive module dB; the bitcrusher mix on a distorted bass; a touch of warmth on a pad. |
| **Space** | 22 | The wet amount of the echo and the room. 0 is dry. | Delay mix and reverb mix together. |
| **Motion** | 23 | How much the sound moves by itself. 0 is still. | LFO depth on the filter, phaser or chorus mix and rate, the beating of a reese, the pump depth of a chord. |

The point: a sequencer, a cue or a hand can steer any preset the same way. A cue that says "Filter 0.2 to 0.9 over four bars" builds any preset in the bank.

Rules:

- Names are exactly `Filter`, `Drive`, `Space`, `Motion` (labels `name.m1` to `name.m4`).
- Every macro moves something audible over its whole range. A macro with nothing obvious to do still gets a small, honest effect on the nearest sound; do not leave a dead macro.
- The macro value stored in the patch is the sound as designed. Cues and automation move it from there.
- Motion depth is an LFO through a VCA that the macro opens (`lfo` to `vca.in`, a route from macro `m4` into the VCA's `gain`, the VCA output as the route source), because a macro cannot scale a route's amount directly. Use a route, not a plain cable, from the macro: the Perform panel lists what a macro moves from its routes and says "moves nothing yet" for a cable.

## Wet and dry: effects baked in, reachable on Space

Effects are part of the sound. A preset ships with its echo and room already on, at a level that suits its genre, and the player can dial them down:

- **Echo and room mix are 0 on the module and driven by the Space macro** (the route amount is the largest mix wanted). The stored Space value is the baked-in wet amount; Space at 0 is the dry sound, at 1 the wettest.
- **Default Space is moderate and genre-appropriate, not low everywhere.** Ambient and cinematic, synthwave and pop pads and leads sit high (Space 0.5 to 0.7, long rooms, long echoes). Pop keys and plucks sit in the middle. Hip-hop, bass and signature presets sit low (0.2 to 0.35: slapbacks, tight rooms, tempo-synced echoes used as an effect, not a wash).
- **Character effects** (chorus, phaser, bitcrusher) that define a sound sit at their working mix on the module, or, when they should be dialled out, are driven by Motion or Drive and are 0 on the module.
- Never bake an effect in so deep that no macro can take it out.
- Every preset ends in a `limiter` (ceiling -1.5 dB, 150 ms release) so no macro setting can clip the output.

## The signature subset

The signature presets (about 22, tagged `signature`) carry the French-touch direction. They follow these extra rules, in addition to everything above. The rules are guidance for them only, not for the whole bank.

Do:

- **Dirt and punch over polish.** Overdriven, saturated, compressed until it pumps; a bitcrusher or hard drive on the mid-range; blunt, short bass stabs. A `compressor` after the voice (fast attack, ratio 4 to 8).
- **Groove and filter movement.** Short, syncopated notes; house tempo (120 to 126 BPM) with swing (25 to 30 %) on the clock; a resonant filter as a performance control (a DJ-style sweep, a filter that closes on every beat); phaser on chords; a pump (a beat-synced envelope ducking the chords or bass).
- **Close, dry rooms.** Space defaults low (0.2 to 0.35): slapbacks, tight rooms, a tempo-synced echo as an effect.
- **Voices that talk.** Formant filter, talkbox and robot voices.

Not this (signature only): long lush reverb and echo on everything; clean wide chorused saws; gated or washed-out space; slow nostalgic arps and pads; smooth vibrato leads; names that say it ("Neon", "Chrome", "Midnight", "Highway", "Sunset", "Retro", "VHS"). Those belong to the synthwave presets and are fine there.

References for the signature: Justice (distortion, compression, hard cut-up rhythm), Daft Punk (Homework, Discovery: filtered disco loops, phaser chords, sidechain pump, talkbox voice, house tempo), and M83 only for the occasional big pad or piece, saturated rather than glossy.

## Levels

The level rule is `docs/factory-levels/README.md`: no sample above -1 dBFS, loudness -19 dBFS RMS within +-5.5 dB (aim for +-3), rendered the way the preset demonstrates itself (a held chord for keys, the clock run for sequences). Level a preset with its `gain` module, or the makeup of its compressor when a heavy compressor makes the gain stage insensitive; do not change the sound to fix the level. `factory_levels_hold_the_target` renders every patch under `patches/`, so a new preset is checked as soon as it is saved. Presets with cues are leveled at their stored macro values (the first cue); say in the description if the drop is louder.

## Names

- Evocative, usually two words, in the style of "Iron Bell" and "Rust Bass": an image, a material or a place, not a spec.
- Names **vary by genre** and should not all sound like one world: an ambient pad can be a landscape (Salt Flat), a dirty bass a quarry (Gravel Pit), a mallet a wood (Boxwood), a piece a place (Basement Drop). Do not use one vocabulary for the whole bank.
- No artist, album or track names, no trademarks, no "Pad 1". Unique across the bank; at most 60 characters.
- The folder is the name in lower case with hyphens: `patches/presets/gravel-pit/`.

## Metadata: how a preset declares itself

`sound.toml` in the preset folder (the Sounds browser reads it; `Meta` in `crates/ui/src/library.rs` describes it):

```toml
name = "Gravel Pit"
category = "Bass"                      # one of CATEGORIES in library.rs
tags = ["mono", "distorted", "compressed", "french touch", "signature"]
description = "..."                    # what it is, then what each macro does
keys = true                            # derived: the patch has a midi.in
sequence = false                       # derived: the patch has a clock
```

- `keys` and `sequence` are derived from the patch (`Meta::play_of`); write them from the patch, never by hand. Every preset has `keys = true`, even a self-playing one (a held chord or a hand-played line on top).
- Tags, in this order: how it plays (`mono`, `poly`, `legato`, `arp`, `sequenced`), what it is made of or does (`supersaw`, `fm`, `phased`, `distorted`, `reese`, `cues`), the genre (`french touch`, `drum and bass`, `ambient`, `pop`, `synthwave`), for a clocked preset its tempo (`122bpm`), and `signature` for the signature subset. Lower case, three to seven tags.
- The description is two or three sentences: what it is, how to play it, then one clause per macro in Filter, Drive, Space, Motion order. All four macro names appear in it, so the browser search finds it.

## How a preset is made and added

1. Build it from existing modules in code (the pilot's generator is `crates/ui/examples/presets_pilot.rs`: one function per preset, a `Preset` entry with its metadata, a clip phrase) and write it to `patches/presets/<slug>/`.
2. Add one entry to the `FACTORY` list in `crates/ui/tests/library.rs`: the browser test asserts that nothing is missing or stray.
3. Level it (`level_table`).
4. Run the checklist below.

Patch format and schema are unchanged. A preset only uses modules that exist on master.

## Checklist: a preset passes only if

1. Plays from MIDI on load (`midi.in`, or an `arp` fed by held keys with a `midi.in` beside it), with a 2-semitone bend range; mono or legato for bass and lead.
2. A self-playing preset starts on Run (the transport is pinned first) and says so in its description.
3. Exactly the four macros, named, on CC 20-23, pinned in order, each audibly doing its job from 0 to 1 (`every_preset_keeps_to_the_brief` checks names, CCs, pins, flags, category, tags and that the description names all four).
4. Space at 0 is dry; the stored Space value is the designed wet amount for its genre.
5. A category from the vocabulary; a name that follows the rules; three to seven tags including a genre; a description with one clause per macro.
6. Level: peak under -1 dBFS and -19 +-5.5 dBFS loudness (`factory_levels_hold_the_target`); no clipping with all four macros up (the limiter).
7. Loads, compiles and renders without a note in the Sounds browser (`every_factory_sound_is_discoverable_categorized_and_loads`).
8. No module added or changed; no dead macro.
9. Checked in the standalone and in REAPER.
10. A clip of the preset played as meant (a short MIDI phrase, or the self-playing preset running) with one line on what to listen for.
11. A signature preset also follows the signature rules above.

## What the pilot found

See `docs/presets/PILOT.md` for the pilot list, the evidence and the gaps.
