# Pilot presets

Eight presets that test the direction in `BRIEF.md` before the 100-preset batch. They use only modules on master (no module, engine, patch-format or schema change). They are written by `crates/ui/examples/presets_pilot.rs` to `patches/presets/<slug>/` and listed in the `FACTORY` list of `crates/ui/tests/library.rs`.

Nobody has listened to them yet: the author of this pilot can render and measure but not hear. Everything below about sound is what the signal chain is built to do and what the renders measure; Kosta's ears decide.

## The eight, spread across genres and roles

Three carry the French-touch signature (tagged `signature`), five cover other genres and roles. Macros are always Filter (CC 20), Drive (21), Space (22), Motion (23); the table says what each one moves.

| Preset | Role | Genre | Plays | Filter | Drive | Space (default) | Motion |
|---|---|---|---|---|---|---|---|
| Gravel Pit | Bass | French touch, signature | mono, short off-beat notes, mod wheel opens the filter | ladder cutoff | drive stage and bitcrusher mix, on top of a dirty base | slapback and tight room (0.25) | filter wobble depth |
| Parquet Loop | Keys | disco, signature, 122 BPM swing | hold a chord, press Run: the filter closes on every beat | ladder cutoff | drive stage | dotted echo and tight room (0.25) | phaser mix and rate |
| Basement Drop | Piece | filter house, signature, 124 BPM swing | press Run; four cues; a hand-played voice on top | ladder cutoff (a resonant DJ sweep) | drive stage and bitcrusher mix | synced echo and room (0.15) | LFO depth on the cutoff |
| Glass Elevator | Keys | pop | poly FM electric piano, velocity | FM index and a lowpass | drive stage | ping-pong echo and room (0.60) | chorus mix and rate |
| Undertow | Bass | hip-hop, drum and bass | mono, legato glide, long low notes | ladder cutoff | drive stage | short room (0.30) | beating of the two saws and a cutoff sweep |
| Salt Flat | Pad | ambient, cinematic | poly, slow, open chords | ladder cutoff | a little saturation | long echo and a 9 s room (0.65) | slow cutoff drift and chorus rate |
| Boxwood | Pluck | mallet | poly FM mallet, velocity | FM index and a lowpass | warmth | echo and a medium room (0.45) | chorus mix and rate |
| Neon Highway | Lead | synthwave | mono, legato glide | ladder cutoff | drive stage | dotted echo and room (0.60) | vibrato depth |

Basement Drop cues (CC 40-43, or the pads): Intro (Filter 0.12, Drive 0.25, Space 0.15, Motion 0.2), Build (0.85, 0.6, 0.3, 0.8, over 16 beats), Drop (0.7, 0.9, 0.15, 0.5, at once), Break (0.2, 0.2, 0.5, 0.3, over 4 beats). Cues set macros only; both sequencers play on.

## What changed from the first pilot

The first pilot (eight presets that read as synthwave) was Cherry Chainsaw, Neon Highway, Roller Disco, Violet Hour, Glass Elevator, Chrome Chase, Midnight Drop and Android Choir. The direction then changed twice: first to a harder French-touch character, then to a general-purpose bank with the French touch as one corner. This is the result.

| Now | Was | What changed |
|---|---|---|
| Gravel Pit | Cherry Chainsaw | Same slot (distorted mono bass) made blunter: 18 dB of drive (was 8), a bitcrusher, a compressor at 8:1, no glide, a snappier envelope, and a slapback and a 0.45 s room instead of a 0.9 s room. |
| Parquet Loop | Roller Disco | Stabs became a held chord that is a filter loop: a clock-locked LFO closes a resonant filter on every beat; four-stage phaser, crunch and compressor; 122 BPM with swing; a dotted echo and a tight room instead of a wash. |
| Basement Drop | Midnight Drop | Filter-house at 124 BPM with swing: a syncopated bass and off-beat stabs that pump together, a resonant DJ-style sweep from a closed filter, harder drive and crush, much drier (Intro Space 0.15), cues rebalanced. |
| Glass Elevator | Glass Elevator | Kept as it was; it is now the clean pop keys. Only the tags changed (pop instead of french touch). |
| Neon Highway | Neon Highway | Kept as it was; it is now the synthwave representative. Only the tags changed (synthwave added). |
| Undertow | new | A reese sub bass for hip-hop and drum and bass. |
| Salt Flat | new | An ambient and cinematic pad (replaces the supersaw Violet Hour: slower, softer, much longer tails). |
| Boxwood | new | An FM mallet pluck. |

Dropped: Violet Hour, Chrome Chase and Android Choir. Harder reworks of the arpeggio groove (a swung 124 BPM arpeggio over a pumped chord) and the voice (a talkbox) were tried during the rework and left out of the eight to make room for other genres; they are the first candidates for the bank.

## What the pilot says about the brief

- **The four macros hold across genres.** A dirty bass, a disco chord loop, a reese, an ambient pad, a mallet and a synthwave lead all have something honest for Filter, Drive, Space and Motion. No change to the set. Drive is the loosest: on a dirty preset it adds to grit that is already there, on a pad it is a little warmth. The brief now says so.
- **Wet by default works, with a genre-appropriate default.** Mixes are 0 on the modules and driven by Space, so the stored value is the designed wet amount and Space at 0 is a usable dry sound in all eight. Defaults run from 0.15 (Basement Drop) to 0.65 (Salt Flat), and the brief now asks for that spread instead of low everywhere.
- **A limiter on the output is part of the recipe.** With macros up, three presets peaked over 0 dBFS in the first pilot. Every preset ends in a `limiter` at -1.5 dB.
- **Heavy compression makes the gain stage insensitive.** Gravel Pit gained only 1.2 dB from +6.5 dB of gain because a 8:1 compressor sits after it; it was leveled with the compressor's makeup instead. The brief says so.
- **Modulation amounts add in the knob's travel and can saturate.** In the first Gravel Pit the filter envelope (0.45) plus the Filter macro (0.25 at its default) pushed the cutoff to the top of its range on every note, so the bass measured 7.5 kHz wide; lowering the amounts brought it to about 4.5 kHz. Keep the sum of a cutoff's routes under 1 at their peaks.
- **The signature rules are buildable.** Dirt (drive plus crusher plus compressor), pump (five modules), swing (the clock's own `swing`), a clock-locked filter loop (`lfo` with `sync`) and a build-up on cues all work with existing modules.

## Gaps

Things kabl lacks that a preset wanted. Each is written down, not built.

1. **No sidechain source.** The pump in Basement Drop is five modules (`clock.div`, an envelope, a VCA for depth, an attenuverter, a VCA to duck). `compressor` has an `sc` input, but nothing yet makes an audio trigger to feed it. Real sidechain pumping waits for the drum voices (kick into `sc`).
2. **A macro cannot scale a route or an LFO depth.** Motion costs two modules per preset (`lfo` into a VCA whose gain the macro routes). A macro target on a route's amount, or a depth knob on `lfo`, would make Motion one cable. The Perform panel labels such a Motion "VCA . Gain" because that is the module the route reaches.
3. **Unison is capped at four voices.** A true supersaw (more voices, a mix control) would sound bigger for fewer modules; the supersaw and synthwave pads will want it.
4. **No stereo-in echo.** `delay` takes one input; the stereo chain taps the left side into it and adds its stereo output at the room. A stereo delay would be cleaner.
5. **The arpeggiator has no accent or pattern.** A Groove category will want accent steps or an arp-through-sequencer combination.
6. **No genre field.** Genre lives in the tags; a preset's genre cannot be filtered on its own. If the browser should offer a genre filter, the metadata needs a field (a schema change, so not done here).
7. **Drums and vocoder.** Gravel Pit, Parquet Loop and Basement Drop gain a kick and the real sidechain; the voice presets gain the vocoder. Not used here.
8. **Cue pieces are leveled at their first cue.** The level test renders a piece at its stored macro values (the Intro); the Drop is a few dB louder than that.

Changes outside the presets: `CATEGORIES` in `crates/ui/src/library.rs` gains **Groove**, **Voice** and **Pluck**. It is a list of names for the browser and Save As; the patch format is unchanged.

## For Kosta: clips and capture

Clips are in `docs/presets/clips/`, one per preset, played the way it is meant to be used (a short MIDI phrase through the real engine; the self-playing preset running). The clips play the stored macro values: no macro is moved during a clip except by the cues in Basement Drop. What to listen for:

| Clip | Listen for |
|---|---|
| `gravel-pit.mp3` | The blunt, flattened attack and the snap of the filter on each short note. |
| `parquet-loop.mp3` | The filter closing on the beat under a held chord; the phaser comes in as Motion goes up. |
| `basement-drop.mp3` | The filter opening through the Build (about 4 to 12 s), the Drop landing hard, and the Break pulling back to a dark loop (from about 20 s). |
| `glass-elevator.mp3` | The tine on the attack against the mellow tail. |
| `undertow.mp3` | The slow beating and the weight of the sub; the glide on the overlapped notes. |
| `salt-flat.mp3` | The slow bloom of each chord and the long tail that carries into the next. |
| `boxwood.mp3` | The click and the wooden ring of each note. |
| `neon-highway.mp3` | The glide between overlapped notes and the echo trailing each phrase. |

`img/sounds-pilot.png` is the Sounds browser filtered to the pilot (search `Filter Drive Space Motion`), 1400x940 (the list needs the height to show all eight); `img/sounds-pilot-selected.png` has one preset selected. `evidence/standalone/` has the standalone Rack and Perform views of every preset with a chord held.

## Evidence

Regenerate with `presets_pilot` (`write`, `clips`, `macros`, `phrases`), `scripts/standalone.py` and `scripts/host.py`.

- **Levels** (`level_table`, 24 s render of each preset's own demonstration), peak / loudness in dBFS: Basement Drop -1.8 / -19.0, Boxwood -2.2 / -20.3, Glass Elevator -1.8 / -21.0, Gravel Pit -7.4 / -19.6, Neon Highway -8.0 / -19.5, Parquet Loop -1.5 / -19.2, Salt Flat -1.8 / -18.4, Undertow -13.1 / -20.3. All within 2.1 dB of -19; `factory_levels_hold_the_target` passes.
- **Macros at the extremes** (`presets_pilot macros`, cues not launched): with every macro at 0 or at 1 no preset exceeds -1.5 dBFS and all renders are finite; Space at 0 alone leaves every preset audible (loudness -17 to -25 dBFS).
- **Standalone** (`scripts/standalone.py`, release build, real audio device, real-time priority granted): all eight open, take MIDI from a virtual controller and stay running; Rack and Perform captures in `evidence/standalone/`.
- **REAPER 7** (`scripts/host.py`, CLAP plugin, `evidence/host/result.json`): per preset a project with the plugin and a MIDI item playing the clip phrase (the piece also gets its cue buttons). Saved, fully quit, reopened and saved: the plugin state is identical in all eight; the render after reopen equals the first (relative difference 0.0); every render is finite, peak 0.07 to 0.22, not silent. Without the MIDI item the seven keyboard presets render silent (RMS 0.0), so the MIDI is what plays them; Basement Drop plays by itself with the transport running in the host.
- **Tests**: `every_preset_keeps_to_the_brief` (`crates/ui/tests/presets.rs`: one macro module, the four names, CC 20-23, pins, play flags, category, tags, a description naming all four macros), `every_factory_sound_is_discoverable_categorized_and_loads` and `factory_levels_hold_the_target` pass. Strict clippy 1.99 (`cargo +1.99 clippy --workspace --all-targets -- -D warnings`) is clean.
- **Measured, not heard**: through Basement Drop's clip the spectral centroid rises from about 250 Hz to 6.6 kHz over the Build, stays near 3.4 kHz through the Drop and falls to 330 Hz in the Break.
- **Not done**: nobody has listened; other hosts; the CLAP validator and cargo-deny run in CI only.
