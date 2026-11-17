# Macro redesign pilot 2: Copper Mile and Brick Dust

Why: the old macros doubled up on the same sound change (a filter, a distortion, a duck, a room, by habit). Each macro now comes from what makes that patch interesting, and the four are meant to be four different ways to change the sound. Clean by default, texture on a macro, no hiss by default, peaks under -1 dBFS at every macro position. No fixed role list. The stored (default) sound of both presets stays as before. This re-applies the #69 pilot on current master (schema 8, builders in `tools/examples/presets_groove.rs`) and uses the audit tool of #66 (`tools/examples/macro_audit.rs`, `docs/presets/scripts/macro_audit.py`, already on master).

Written from the patch recipe and the audit renders. Nobody has listened yet; the "what you hear" lines are what the parameters and measures say. The old-vs-new tour is the real test.

## Copper Mile (disco loop: octave saw bass, saw stabs, phaser, kick-ducked)

| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Sweep | 0.90 | A closed, wah-like filter over the bass and the stabs opens until it is wide open | ladder cutoff 449 Hz to 20 kHz; resonance 0.60 to 0.21 |
| Swirl | 0.50 | The stabs go from a plain saw chord to a slow, shallow phaser to a fast, deep, ringing one that is also wider | phaser mix 0 to 90 %, rate 0.15 to 1.07 Hz, depth 62 to 98 %, feedback 10 to 90 %; stab unison detune 2 to 26 ct |
| Pump | 0.60 | The kick ducks nothing, then more and more of the loop on every kick | duck threshold 0 to -57 dB at a fixed ratio (was the ratio, where the first step did 80 %) |
| Room | 0.25 | Unchanged: a tempo-synced echo and a tight room | echo and room mix |

## Brick Dust (blunt stomp: driven saw and square bass, heavy compressor)

| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Grind | 0.00 | The approved bass, then an aliased, crushed grit builds on it | new bitcrusher on the bass bus: mix 0 to 100 %, bits 8 to 4, hold 3 to 14 samples; drive 29.7 to 36 dB with the trim 6 dB lower |
| Squash | 0.60 | The mix is clamped harder and gets flatter and denser; the drums lose their gaps | the music compressor: threshold -6 to -39 dB, makeup 0 to 6.7 dB (ratio stays 6) |
| Push | 0.50 | The stomp pushes faster or drags (112 to 140 bpm, 126 at the stored position) | clock tempo |
| Slap | 0.067 | A slapback and a tight room come in, three times as wide as before | echo mix max 45 %, room mix max 30 % (the stored position is a third of the old one, so the sound is the same) |

## Checks (`macro_audit`, 8 steps per macro, others at their stored value)

Old macros against new, same phrase, same tool:

| Preset | Old flags | New flags |
|---|---|---|
| Copper Mile | Filter dead; Pump dead + jump; Room weak | Room weak (unchanged macro); Sweep, Swirl, Pump clean |
| Brick Dust | Filter weak; Dirt duplicate; Pump dead + jump; Room weak + duplicate | Grind jump (the last step); Squash, Push, Slap clean |

- Peaks: Copper Mile worst -2.2 dBFS at any step and in any sweep (same as the old macros); Brick Dust worst -3.7 (old -2.9). No click in any step or sweep render (`audio_defects` clicks, no `problems` entry).
- Change 0 against 1 (dB of the audit measure): Sweep 1.4, Swirl 2.8, Pump 11.7, Room 1.0; Grind 1.9, Squash 6.6, Push 9.6, Slap 2.0. Push measures large because tempo moves every onset; the level and spectrum do not change.
- Weak spots, honestly: Brick Dust Grind is back-loaded, the first five steps move 0.1 to 0.3 dB and the last step 1.6 dB (flag `jump`): a crusher on a bass is mostly in the low harmonics, and a harder or milder setting made it worse in the first pilot. Copper Mile Sweep is front-loaded. Swirl is mostly phase, so it measures only 2.8 dB. Listen to Grind and Sweep first.

## Default sound against the old patch (16 s clip, float, same phrase, `presets_groove clips`)

| Preset | Peak old / new | Difference render |
|---|---|---|
| Copper Mile | -6.21 / -6.21 dBFS | bit-identical |
| Brick Dust | -3.85 / -3.80 dBFS | -18.3 dB below the old signal. Built without the new crusher module the render differs by -89.5 dB (float noise), so the difference is only the extra module: it shifts the ids of the drum modules, and the snare and hat noise is drawn differently. Same patch, other noise samples |

Other presets: renders bit-identical (Ore Cart, Scrap Metal checked; the patch files of every other preset are untouched).

## How

`tools/examples/presets_groove.rs`: `Spec::custom` lets a preset route macros 1 to 3 itself (`copper_plan`, `brick_plan`); the fourth stays the space macro. `span` and `rise` set a param so that the stored macro position gives exactly the old value (the old routes' effect is recomputed with `routed`), and the macro runs on in the same line to 1. Patches were regenerated with `presets_groove write patches`; only Copper Mile and Brick Dust change.

Not done in the pilot: other presets kept their old macros; the Corner Store and Fault Line click was parked. Both are handled in the next section.

# The rest of Groove (S2): twelve presets

Same method as the pilot, now for Ore Cart, Stoop Sale, Fault Line, Corner Store, Glass Coast, Still Water, Slab City, Rope Walk, Lamp Oil, Acid Line, Scrap Metal and Dune Buggy (Loading Dock lives in `presets_pilot.rs` and is left to the pilot-family slice). Per preset the audit of the old macros (dead, weak, jump, duplicate, clicks, peaks) decided what stayed and what was replaced; the fourth macro stays the space macro (echo and room), which the audit found fine, and Copper Mile's Room, the one weak space macro, now reaches two and a half times as far. Nobody has listened yet: the "what you hear" columns are what the parameters and the audit measures say.

How it is built: `main_line` picks the position where the macro's main param sits on its old stored value and stores the macro there (so the stored position of a macro is no longer the old one, the stored sound is); the macro's other params follow the same line with `span` / `span_hi`. Plans: `ore_plan`, `stoop_plan`, `fault_plan`, `corner_plan`, `glass_plan`, `still_plan`, `slab_plan`, `rope_plan`, `lamp_plan`, `scrap_plan`, `dune_plan`; Acid Line is edited in its own function. `Handles` now reaches the bass and chord parts, the bus, the beat, the kit and the dub echo.

**Gate length.** The sequencer's `gate_len` only counts in `gate_mode` 1 (length); in the default mode the gate follows the clock pulse (50 % duty) and the part's `gate` field (55, 65, 78 in the old recipes) never sounded. Gate macros therefore set mode 1 and store 50 % (the same sound at zero swing; with swing the pulse stretches, so Rope Walk and the swung presets have no gate macro).

**The factory level test** (`crates/ui/tests/factory_bank.rs`, `factory_levels_hold_the_target`) drives every macro to 0.65 and then to 0.15 (and to 1 in the hot renders), whatever the stored value, and wants every peak at -1 dBFS or lower and the loudness within -19 ± 5.5 dBFS. It caught what the clip phrase did not: Corner Store's demo peak (the 8 kHz top rings, see below) and Still Water, whose dark low end made the 0.15 passages too quiet (Bloom now starts at 1.2 kHz, Pulse and Weight stop at 0.8 and 0.85). All presets pass.

## Old flags against new flags

| Preset | Old flags | New flags |
|---|---|---|
| Ore Cart | Filter weak; Rumble and Space duplicate; Pump dead + jump | Rumble and Space duplicate (see below) |
| Stoop Sale | Filter weak | clean |
| Fault Line | Hats dead; Filter click with Reese (parked) | clean |
| Corner Store | Filter dead + clicks (parked) | clean |
| Glass Coast | Filter dead; Hats weak | clean |
| Still Water | Bloom dead; Pulse jump | clean |
| Slab City | Tone weak; Glide dead | clean |
| Rope Walk | Filter dead; Pump dead + jump | clean |
| Lamp Oil | Chord and Dub duplicate; Filter dead | clean |
| Acid Line | Cutoff and Accent duplicate; Slide weak | Cutoff and Accent similar (cosine 0.89, under the duplicate line 0.9) |
| Scrap Metal | Vowel weak; Pump dead + jump | Crush weak (1.0 dB) |
| Dune Buggy | Filter dead | clean |
| Copper Mile | Room weak | clean |
| Brick Dust | Grind jump; Grind and Slap click (combination) | unchanged: see "Open flags" |

Peaks: every position of every macro of every preset is at or under -1 dBFS (worst: Ore Cart Pump -1.4, Fault Line Reese -1.3, Scrap Metal Pump -1.2, Corner Store Swing -1.1). No click in any end render or sweep render. Loudest macro moves, 0 against 1 (dB of the audit measure): Pump 10 to 16 on the presets that have one, Swing/Skip 9 to 13, Rumble 10, Bloom 9.

## Default sound against master (16 s clip, float, same phrase)

| Preset | Difference render |
|---|---|
| Copper Mile, Brick Dust, Acid Line, Glass Coast, Rope Walk, Dune Buggy | bit-identical (-217 to -221 dB, float noise) |
| Ore Cart, Still Water, Slab City, Lamp Oil, Scrap Metal | -81 to -124 dB below the old signal: float noise from the sum of the new routes |
| **Stoop Sale** | **-29.5 dB: the default hiss and the pitch wobble are gone.** The old Dust macro stood at 0.30, which put a hiss in the default sound (the hard rule is no hiss by default); Dust now starts at 0 and the hiss is its far end |
| **Fault Line** | **-22.4 dB: an 8 kHz low-pass on the bass part (the click fix, below) and 1 dB less level (trim -3 instead of -2, for the Reese macro's peak)** |
| **Corner Store** | **-20.2 dB: the same 8 kHz low-pass on the bass part and 2 dB less level. The low-pass makes the demo peak 2 dB higher (it rings at the saw edges), and the factory level test (peak -1 dBFS at every macro position) needed the 2 dB back** |

## The parked clicks

The Corner Store and Fault Line click is the reset edge of the bass saw at a note start, which a scanner window sees as an isolated spike when the filter lets it through. Muting the bass removed it; a pitch slew, a longer attack or another unison did not. What works is a soft top on the bass itself: `Part::lp(8000.0)` on both bass parts (the sound changes by the -22 and -23 dB above, a bit less top on the bass). The filter macros keep their range (Wah up to 11.8 kHz, Filter to 20 kHz) and no click remains in any step, sweep or combination render of the two presets.

## Open flags (honest list)

- **Brick Dust Grind**: still back-loaded (flag `jump`, the last step 1.6 dB against 0.1 to 0.4 before). Grind plus Slap at 1 clicks at 0.6 s (the first bass note of the run, 90 ms before its echo repeats): the echo exposes a crusher spike that sits among loud drums in the dry render. The Grind mix now reaches 100 % at two thirds of the travel so that the last third hardens bits and hold instead; that moved the pairs a little but did not fix the click. Needs listening.
- **Ore Cart Rumble and Space** measure as duplicates (cosine 0.96): the coarse measures (loudness, four bands, width, density) see both as "wider", Rumble because the held, driven bass widens the mix, Space because of the echo. The sound is different (Rumble is in the sub bands and the note length, Space is the tail); the measure cannot tell.
- **Acid Line Cutoff and Accent** point 0.89 the same way (both brighten); Accent also shortens the zap and the notes.
- **Scrap Metal Crush** moves only 1.0 dB: a wider range (mix 60 %, hold 14) puts a click in the all-at-1 render, so the macro stays milder.
- **Squash and Slap (Brick Dust)** are opposite in the similarity matrix (cosine -0.98): more squash means less tail. Left as is.

## The twelve designs

Stored = where the macro stands in the default sound (the old stored sound).

### Ore Cart (techno loop, 132 bpm)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Rumble | 0.39 | The bass goes from clean and clipped short to driven hard and held long | bus drive 0 to 10 dB (stored) to 26 dB, trim -1 to -5 dB; bass gate 8 % to 50 % (stored) to a full step |
| Tunnel | 0.72 | The music opens from a dark tunnel (only the sub) to the stored sound and a little beyond | ladder cutoff 180 Hz to 3.4 kHz (stored) to 8 kHz; resonance 0.45 to 0.30 |
| Pump | 0.55 | No duck, then the kick takes more and more of the loop | duck threshold 0 to -34 dB (stored) to -60 dB; ratio fixed |
| Space | 0.30 | unchanged | echo and room mix |

### Stoop Sale (boom bap, 86 bpm)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Swing | 0.58 | The off-beats get later and the whole loop drags | swing 20 % to 60 % (stored) to 90 %; tempo 78 to 86 (stored) to 92 bpm |
| Dust | 0.00 | A clean loop, then vinyl hiss and a slow wobble of the pitch of bass and chords come in | noise on the bus 0 to 30 %; a 0.55 Hz LFO on the oscillators' fine tune |
| Tape | 0.85 | A muffled, hot, saturated sound opens into a clean, open one | ladder cutoff 250 Hz to 8.7 kHz (stored) to 20 kHz; bus drive 19 dB to 3 dB (stored) to 0 |
| Space | 0.30 | unchanged | echo and room mix |

### Fault Line (two-step DnB, 174 bpm)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Reese | 0.97 | The reese grows teeth: more detune, more drive (the stored sound is the full effect; the macro only takes it away) | bass detune 6 to 22 ct (stored); drive 6 to 17 dB (stored) to 17.3 dB. The drive saturates, so the trim stays at -4 dB |
| Filter | 0.86 | A closed, resonant bass opens | ladder cutoff 250 Hz to 6 kHz (stored) to 12 kHz; resonance 0.45 to 0.20 |
| Break | 0.44 | A sparse kit gets hats and a louder snare | hat level 0 to 0.22 (stored) to 0.5; snare 0.2 to 0.45 (stored) to 0.77 |
| Space | 0.25 | unchanged | echo and room mix |

### Corner Store (funk, 108 bpm)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Swing | 0.63 | From nearly straight 16ths to a heavy shuffle | swing 15 % to 65 % (stored) to 95 %; tempo 100 to 108 (stored) to 113 bpm |
| Wah | 0.97 | A closed, resonant funk filter wah opens to the stored (nearly fully open) sound | ladder cutoff 350 Hz to 10.6 kHz (stored) to 11.8 kHz; resonance 0.45 to 0.20 |
| Bite | 0.49 | Clean to hot | bus drive 2 dB to 8.3 dB (stored) to 15 dB; trim to keep the level |
| Room | 0.25 | unchanged | echo and room mix |

### Glass Coast (synthwave, 100 bpm)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Arp | 0.41 | The arpeggio goes from clipped plucks to full-length notes and gets thicker | arp gate 15 % to 50 % (stored) to 100 %; arp detune 4 to 18 ct (stored) and on |
| Pulse | 0.50 | The bass pulses tighten or run into each other, and the hats rise | bass gate 15 % to 50 % (stored) to 85 %; hat level 0.1 to 0.55 (stored) to 1 |
| Chorus | 0.50 | The chorus goes from nothing to a deep, fast, wide one | mix 0 to 45 % (stored) to 90 %; rate 0.1 to 0.3 (stored) to 0.5 Hz; depth 20 to 55 % (stored) to 90 % |
| Space | 0.55 | unchanged | echo and room mix |

The Filter macro of the old Glass Coast is gone (its top two thirds were dead); the cutoff stays at what it left at the stored position (16.7 kHz).

### Still Water (ambient, 68 bpm)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Bloom | 0.88 | The music opens from dark to bright | ladder cutoff 1.2 kHz to 10.1 kHz (stored) to 20 kHz; resonance 0.45 to 0.05 |
| Pulse | 0.28 | More and more hats and toms | hat level 0.1 to 0.32 (stored) to 0.8; tom level 0.1 to 0.35 (stored) to 0.8 |
| Weight | 0.63 | The bass grows | bass level 0.2 to 0.61 (stored) to 0.85 |
| Space | 0.60 | unchanged | echo and room mix |

### Slab City (trap, 140 bpm)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Glide | 0.15 | The 808 goes from stepping to long slides, and gets hotter | slide 0.1 ms to 3.2 ms (stored) to 100 ms (both directions); drive 4.5 to 6 dB (stored) and on, trim lower to keep the level |
| Hustle | 0.33 | Sparse, then busy: hats and clap come up | hat level 0.4 to 0.6 (stored) to 1; clap 0.2 to 0.4 (stored) to 0.8 |
| Tone | 0.89 | The pad opens from a muffled saw to the stored sound | ladder cutoff 400 Hz to 8.7 kHz (stored) to 9.5 kHz |
| Room | 0.25 | unchanged | echo and room mix |

### Rope Walk (disco, 118 bpm)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Knob | 0.93 | A DJ-knob filter: closed and resonant to open | cutoff 300 Hz to 11 kHz (stored) to 20 kHz; resonance 0.45 to 0.25; bass drive 0 to 5 dB |
| Swirl | 0.55 | A plain stab, a slow phaser, a fast deep ringing one | phaser mix 0 to 50 % (stored) to 90 %, rate, depth and feedback; stab detune |
| Pump | 0.60 | No duck, then the kick takes the loop; the music is 2.2 dB quieter at 0 | duck threshold 0 to -34 dB to -57 dB; music level trim |
| Room | 0.25 | unchanged | echo and room mix |

### Lamp Oil (dub techno, 124 bpm)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Chord | 0.43 | The stab goes from a short, dark, thin click to a long, bright, thick chord | chord tone filter 300 Hz to 1 kHz (stored) to 5 kHz; gate 10 % to 50 % (stored) to a full step; detune 4 to 12 ct (stored) and on |
| Dub | 0.58 | The stab's own echo grows, with more repeats and a darker tail | dub mix 0 to 35 % (stored) to 60 %; feedback 30 to 62 % (stored) and on; tone 3 kHz to 1.8 kHz (stored) to 300 Hz |
| Deep | 0.87 | The music opens from a dark filter | ladder cutoff 250 Hz to 11 kHz (stored) to 20 kHz; resonance 0.45 to 0.15 |
| Space | 0.50 | unchanged | echo and room mix |

### Acid Line (303, 124 bpm; no space macro)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Cutoff | 0.40 | The bass filter opens | ladder cutoff (unchanged route) |
| Resonance | 0.55 | The squeal, and the filter is driven harder | resonance 0.25 to 0.59 (unchanged route); ladder drive 0 to 6 dB (stored) and on |
| Accent | 0.60 | Accented notes zap, then wow: more opening, a longer envelope, longer notes | accent depth (unchanged route); filter envelope decay 10 to 220 ms (stored) and on; gate 30 % to 62 % (stored) |
| Slide | 0.49 | Marked notes go from a step to a long portamento | slide time 0.1 to 28 ms (stored) to 100 ms |

### Scrap Metal (electro stomp, 128 bpm)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Vowel | 0.50 | The stab talks from a to u, sharper at the top, and the music's filter follows | vowel 0 to 2 (stored) to 4; formant Q 2 to 10; stab detune 3 to 12 ct; ladder cutoff 450 Hz to 1.2 kHz (stored) and on |
| Crush | 0.53 | Clean, then a mild bitcrush on the whole music | crusher mix 0 to 24 % (stored) to 45 %; hold 7 to 10 (stored) to 12 |
| Pump | 0.60 | No duck, then the kick takes the loop | duck threshold 0 to -34 dB (stored) to -57 dB |
| Room | 0.30 | unchanged | echo and room mix |

### Dune Buggy (UK garage, 130 bpm)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Skip | 0.59 | From nearly straight to a heavy skip | swing 12 % to 58 % (stored) to 90 %; tempo 124 to 130 (stored) to 134 bpm |
| Warmth | 0.91 | The music opens from dark | ladder cutoff 300 Hz to 11 kHz (stored) to 20 kHz; resonance 0.45 to 0.15 |
| Chop | 0.46 | The chord stabs go from thin clipped ticks to long, fat chords | chord gate 8 % to 50 % (stored) to a full step; detune 2 to 10 ct (stored) and on |
| Space | 0.30 | unchanged | echo and room mix |

## Listen first (the riskiest designs)

1. **Stoop Sale**: the default hiss and the wobble are gone (Dust starts at 0), Tape is a new kind of move (dark and hot to open and clean).
2. **Fault Line**: the default sound changed by -23 dB (a soft top on the bass), Reese and Break are new.
3. **Corner Store**: the default sound changed (the same soft top, and 2 dB quieter), Wah now reaches the click level only at the very top.

Also worth an ear: Ore Cart Rumble (new gate-length gesture), Lamp Oil Chord (the tone filter and the gate), Slab City Glide (the 808 slide with drive).

## Tours of the twelve (offline renders)

Git-ignored, on disk in `docs/presets/clips/macros-groove/`: `<slug>-ab.mp3` per preset and `tours.txt` with the start second of every clip. Each clip is 20 s: the macro held at 0 for 6 s, up over 4 s, held at 1 for 6 s, down over 4 s, the phrase looping on the grid of the preset's tempo; old macro 1, new macro 1, old macro 2, new macro 2, and so on, 1 s of silence between clips. Regenerate: `MACRO_TOUR_BPM=<bpm> macro_audit <patches> <phrases.json> <renders> <slug>` once for the old patches (`git archive origin/master patches`) and once for the new, then `docs/presets/scripts/macro_tour_ab.py --old-patches P --old-renders R --new-patches P --new-renders R --out DIR <slugs>`.

## Combination (do the four macros mix well together?)

`macro_audit` writes, besides the per-macro steps and sweeps, `combo/<slug>-base|all0|all1|mA-mB.wav`: the stored sound, every macro at 0, every macro at 1, and every pair at 1 with the other two at their stored values (45 renders of 10 s per preset in all; about 40 s per preset). `macro_audit.py` reads them and writes a "Combination" table and a 4x4 similarity matrix per preset into `tables.md` (`audit.json` under `_combo`). Flags (thresholds are constants at the top of the script):

| Flag | Meaning | Threshold |
|---|---|---|
| `peak` | any combination render over -1 dBFS | `PEAK_DB` -1 |
| `click` | a click (`audio_defects.clicks`) the stored sound does not have | |
| `cancel` | a pair at 1 changes the sound (same distance as `change`, against the stored sound) by less than 0.7 of the larger single macro's change; only when that single moves 1 dB or more | `CANCEL_RATIO`, `CANCEL_MIN_DB` |
| `pileup` | a pair or all four at 1 is 3 dB louder than the louder single, or 6 dB louder than the stored sound, or its 2 kHz-up bands (mean of upper and air) are 4 dB brighter than the brighter single's | `PILE_LOUD_DB`, `PILE_BASE_DB`, `PILE_TOP_DB` |
| `silent` | all at 0 is under -45 dB or 18 dB under the stored sound | |
| `similar` | in the matrix, cosine of two macros' 0-to-1 change vectors over 0.8 (both moving more than 1.5 dB) is listed as too similar; under -0.9 as opposite (one undoes the other). Each component of a vector is limited to ±10 dB first, so one huge move (a room from nothing to +40 dB of width) cannot decide the cosine. The same clipped cosine drives the old `duplicate` flag (over 0.9) | `SIMILAR_COS`, `CLIP_DB` |

A flag is a draft verdict; a person reads the numbers. Run it with the same commands as the audit (the combination renders come with the `macro_audit` run).

## Tours (offline renders, each macro swept 0 to 1 to 0 over 8 s on the held phrase, 11 s apart)

Git-ignored, on disk in `docs/presets/clips/macro-pilot-2/`: `old/<slug>-tour.mp3` and `new/<slug>-tour.mp3` per preset, one clip per macro beside them (`<slug>-mN-<name>.mp3`), and `tour.mp3` (both presets back to back, 44 s each). Start times inside each per-preset tour:

| Tour | Starts at 0 s | 11 s | 22 s | 33 s |
|---|---|---|---|---|
| copper-mile old | Filter | Phase | Pump | Room |
| copper-mile new | Sweep | Swirl | Pump | Room |
| brick-dust old | Filter | Dirt | Pump | Room |
| brick-dust new | Grind | Squash | Push | Slap |

Regenerate: `cargo build --release -p kabl-tools --example macro_audit --example presets_groove`, `presets_groove phrases <phrases.json>`, `macro_audit <patches> <phrases.json> <renders> copper-mile brick-dust`, then `docs/presets/scripts/macro_audit.py --category Groove --renders <renders> --patches <patches> --clips <dir> --out <report> copper-mile brick-dust` (the old side from `git archive origin/master patches`).
