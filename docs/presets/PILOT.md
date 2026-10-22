# Pilot presets

Eight presets that test the direction in `BRIEF.md` before the 100-preset batch. They use only modules on master (no module, engine, patch-format or schema change). They are written by `crates/ui/examples/presets_pilot.rs` to `patches/presets/<slug>/` and listed in the `FACTORY` list of `crates/ui/tests/library.rs`.

Nobody has listened to them yet: the author of this pilot can render and measure but not hear. Everything below about sound is what the signal chain is built to do and what the renders measure; Kosta's ears decide.

## The eight

Macros are always Filter (CC 20), Drive (21), Space (22), Motion (23). The table says what each one moves in that preset.

| Preset | Category | Plays | Filter | Drive | Space | Motion |
|---|---|---|---|---|---|---|
| Cherry Chainsaw | Bass | mono, legato glide, mod wheel opens the filter | ladder cutoff | drive stage before the filter | room mix | LFO depth on the cutoff |
| Neon Highway | Lead | mono, legato glide | ladder cutoff | drive stage | echo and room mix | vibrato depth |
| Roller Disco | Keys | poly stabs | ladder cutoff | drive stage | echo and room mix | phaser mix and rate |
| Violet Hour | Pad | poly, slow | ladder cutoff | drive stage | echo and room mix | slow LFO depth on the cutoff, chorus rate |
| Glass Elevator | Keys | poly FM, velocity | FM index and a lowpass | drive stage | ping-pong echo and room mix | chorus mix and rate |
| Chrome Chase | Groove | hold a chord, press Run: arpeggio over a pumping pad | ladder cutoff | drive stage | echo and room mix | pump depth of the held chord, slow cutoff sweep |
| Midnight Drop | Piece | press Run: bass and lead play; four cues; a hand-played voice on top | ladder cutoff | drive stage | echo and room mix | LFO depth on the cutoff |
| Android Choir | Voice | poly chords | vowel position (A to U) | bitcrusher mix | echo and room mix | vowel wander and vibrato depth |

Midnight Drop cues (CC 40-43, or the pads): Intro (Filter 0.15, Drive 0.2, Space 0.35, Motion 0.2), Build (0.9, 0.55, 0.5, 0.85, over 16 beats), Drop (0.7, 0.9, 0.2, 0.5, at once), Break (0.25, 0.1, 0.85, 0.3, over 4 beats). Cues set macros only; both sequencers play on.

## What the pilot says about the brief

- **The four macros hold.** Every preset has something honest for each of Filter, Drive, Space and Motion. The convention needed no change. The one refinement is to the rule, not the set: Motion is built as an LFO opened by the macro, so it means "how much it moves", and a macro must be wired as a route (not a plain cable) or the Perform panel says "moves nothing yet" for it.
- **Wet by default works.** With effect mixes at 0 on the modules and driven by Space, the stored Space value is the designed wet amount and Space at 0 gives a usable dry sound in all eight (measured: not silent, within a few dB of the designed level). Nothing here argues for baking the effects in deeper. Character effects (chorus, phaser) stay on the module or on Motion.
- **A limiter on the output is part of the recipe.** With macros up, three presets peaked over 0 dBFS (up to +5). Every preset ends in a `limiter` at -1.5 dB, so no macro setting clips; at the stored settings it does nothing the level rule would not already do.
- **Level by `gain` works as the level doc says.** All eight sit within 2.1 dB of -19 dBFS and under -1.4 dBFS peak. Presets with a sharp tine (Glass Elevator, Roller Disco) have 18 to 20 dB of crest, which is what limits how loud they can sit.

## Gaps

Things kabl lacks that a preset wanted. Each is written down, not built.

1. **No sidechain source.** The pump in Chrome Chase is five modules (`clock.div`, an envelope, a VCA for depth, an attenuverter, a VCA to duck). `compressor` has an `sc` input, but nothing yet makes an audio trigger to feed it. Real sidechain pumping waits for the drum voices (kick into `sc`).
2. **A macro cannot scale a route or an LFO depth.** Motion costs two modules per preset (`lfo` into a VCA whose gain the macro routes). A macro target on a route's amount, or a depth knob on `lfo`, would make Motion one cable.
3. **Unison is capped at four voices.** The supersaw pad stacks two oscillators for eight; a true supersaw (more voices, a mix control) would sound bigger for fewer modules. The M83 side will want it.
4. **No stereo-in echo.** `delay` takes one input. A stereo chain taps the chorus' left side into it and adds its stereo output at the room. It works; a stereo delay would be cleaner.
5. **The arpeggiator has no accent or pattern.** Chrome Chase moves by pump and filter, not by an accented line. A Groove category will want accent steps or an arp-through-sequencer combination.
6. **Drums and vocoder.** Midnight Drop, Chrome Chase and Cherry Chainsaw gain a kick (and the real sidechain); Android Choir gains the vocoder. Not used here.
7. **Cue pieces are leveled at their first cue.** The level test renders a piece at its stored macro values (the Intro); the Drop is about 5 dB louder than that.

One change outside the presets: the library vocabulary (`CATEGORIES` in `crates/ui/src/library.rs`) gains **Groove** and **Voice**. It is a list of names for the browser and Save As; the patch format is unchanged.

## For Kosta: clips and capture

Clips are in `docs/presets/clips/`, one per preset, played the way it is meant to be used (a short MIDI phrase through the real engine; the self-playing preset running). What to listen for:

| Clip | Listen for |
|---|---|
| `cherry-chainsaw.mp3` | The snap of the filter on each note and the growl as Drive goes up; the glide on the overlapped notes. |
| `neon-highway.mp3` | The glide between overlapped notes and the echo trailing each phrase. |
| `roller-disco.mp3` | The phaser sweep moving through each stab; turn Motion to zero and it goes still. |
| `violet-hour.mp3` | The slow bloom of the filter on each chord and the long tail after release. |
| `glass-elevator.mp3` | The tine on the attack against the mellow tail, and the way Filter opens the whole spectrum. |
| `chrome-chase.mp3` | The pad breathing under the arpeggio on each beat; raise Motion and it pumps harder. |
| `midnight-drop.mp3` | The filter opening through the Build, the Drop landing hard, and the Break draining into echo. |
| `android-choir.mp3` | The vowel wandering on a held chord; raise Drive and the choir turns to a robot. |

The clips play the stored macro values: no macro is moved during a clip except by the cues in Midnight Drop. `img/sounds-pilot.png` is the Sounds browser filtered to the pilot (search `Filter Drive Space Motion`), 1280x800, light theme (a fresh profile); `img/sounds-pilot-selected.png` has Chrome Chase selected. `evidence/standalone/` has the standalone Rack and Perform views of every preset with a chord held.

## Evidence

Regenerate with `presets_pilot` (`write`, `clips`, `macros`, `phrases`), `scripts/standalone.py` and `scripts/host.py`.

- **Levels** (`level_table`, 24 s render of each preset's own demonstration): peak / loudness in dBFS: Android Choir -5.1 / -19.2, Cherry Chainsaw -11.9 / -19.7, Chrome Chase -5.5 / -18.8, Glass Elevator -1.8 / -21.0, Midnight Drop -4.2 / -18.8, Neon Highway -8.0 / -19.5, Roller Disco -1.9 / -21.1, Violet Hour -1.5 / -19.9. `factory_levels_hold_the_target` passes.
- **Macros at the extremes** (`presets_pilot macros`, cues not launched): with every macro at 0 or at 1 no preset exceeds -1.5 dBFS and all renders are finite; Space at 0 alone leaves every preset audible (loudness -18 to -22 dBFS).
- **Standalone** (`scripts/standalone.py`, release build, real audio device, real-time priority granted): all eight open, take a chord from a virtual MIDI controller and stay running; Rack and Perform captures in `evidence/standalone/`. The Perform panel shows the four macros named, on CC 20-23, each with what it moves ("Ladder . Cutoff", "Drive . Drive", "Delay . Mix . Reverb . Mix", "VCA . Gain"); Midnight Drop shows Transport and the four cue pads. Motion reads "VCA . Gain" because that is the module the route reaches; a macro-friendly label would be nicer (not changed here).
- **REAPER 7** (`scripts/host.py`, CLAP plugin, `evidence/host/result.json`): per preset a project with the plugin and a MIDI item playing the clip phrase (the piece also gets its cue buttons). Saved, fully quit, reopened and saved: the plugin state is identical in all eight; the render after reopen equals the first (relative difference 0.0); every render is finite, peak 0.07 to 0.22, not silent. Without the MIDI item the seven keyboard presets render silent (RMS 0.0), so the MIDI is what plays them; Midnight Drop plays by itself with the transport running in the host.
- **Tests**: `every_preset_keeps_to_the_brief` (new, `crates/ui/tests/presets.rs`: one macro module, the four names, CC 20-23, pins, play flags, category, tags, a description naming all four macros), `every_factory_sound_is_discoverable_categorized_and_loads` and `factory_levels_hold_the_target` pass. Strict clippy 1.99 (`cargo +1.99 clippy --workspace --all-targets -- -D warnings`) is clean.
- **Not done**: nobody has listened (see top); other hosts; the CLAP validator and cargo-deny run in CI only.
