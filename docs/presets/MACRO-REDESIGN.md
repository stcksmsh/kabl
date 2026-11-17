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

Not done: other presets keep their old macros; the Corner Store and Fault Line click is parked.

## Tours (offline renders, each macro swept 0 to 1 to 0 over 8 s on the held phrase, 11 s apart)

Git-ignored, on disk in `docs/presets/clips/macro-pilot-2/`: `old/<slug>-tour.mp3` and `new/<slug>-tour.mp3` per preset, one clip per macro beside them (`<slug>-mN-<name>.mp3`), and `tour.mp3` (both presets back to back, 44 s each). Start times inside each per-preset tour:

| Tour | Starts at 0 s | 11 s | 22 s | 33 s |
|---|---|---|---|---|
| copper-mile old | Filter | Phase | Pump | Room |
| copper-mile new | Sweep | Swirl | Pump | Room |
| brick-dust old | Filter | Dirt | Pump | Room |
| brick-dust new | Grind | Squash | Push | Slap |

Regenerate: `cargo build --release -p kabl-tools --example macro_audit --example presets_groove`, `presets_groove phrases <phrases.json>`, `macro_audit <patches> <phrases.json> <renders> copper-mile brick-dust`, then `docs/presets/scripts/macro_audit.py --category Groove --renders <renders> --patches <patches> --clips <dir> --out <report> copper-mile brick-dust` (the old side from `git archive origin/master patches`).
