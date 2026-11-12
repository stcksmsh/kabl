# Macro redesign pilot: Copper Mile and Brick Dust

Why: the old macros doubled up on the same sound change (a filter, a distortion, a duck, a room, by habit). Each macro now comes from what makes that patch interesting, and the four are meant to be four different ways to change the sound. The stored (default) sound of both presets is unchanged. The ear decides; the numbers below are a sanity check from `macro_audit` (8 positions per macro, others at their stored value).

Written from the patch recipe and the audit renders. I could not listen, so the "what you hear" lines are what the parameters and measures say; Kosta's old-vs-new tour is the real test.

## Copper Mile (disco loop: octave saw bass, saw stabs, phaser, kick-ducked)

| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Sweep | 0.90 | A closed, wah-like filter over the bass and the stabs opens until it is wide open (the loop starts muffled and honking, ends bright) | ladder cutoff 450 Hz to 20 kHz; resonance 0.60 to 0.21 (a resonant peak at the start, none at the end) |
| Swirl | 0.50 | The stabs go from a plain saw chord to a slow, shallow phaser to a fast, deep, ringing one that is also wider | phaser mix 0 to 90 %, rate 0.15 to 1.1 Hz, depth 62 to 98 %, feedback 10 to 90 %; stab unison detune 2 to 26 ct |
| Pump | 0.60 | The kick ducks nothing, then more and more of the loop on every kick | duck threshold 0 to -57 dB at a fixed ratio (was: the ratio, where the first step did 80 %) |
| Room | 0.25 | Unchanged: a tempo-synced echo and a tight room | echo and room mix |

## Brick Dust (blunt stomp: driven saw and square bass, heavy compressor)

| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Grind | 0.00 | The approved bass, then an aliased, crushed grit builds on it | new bitcrusher on the bass bus: mix 0 to 100 %, bits 8 to 4, hold 3 to 14 samples; drive 29.7 to 36 dB with the trim 6 dB lower |
| Squash | 0.60 | The mix is clamped harder and gets flatter and denser; the drums lose their gaps | the music compressor: threshold -6 to -39 dB, makeup 0 to 6.7 dB (ratio stays 6) |
| Push | 0.50 | The stomp pushes faster or drags (112 to 140 bpm, 126 at the stored position) | clock tempo |
| Slap | 0.067 | A slapback and a tight room come in, three times as wide as before | echo mix max 45 %, room mix max 30 % (the stored position is a third of the old one, so the sound is the same) |

## Checks (`macro_audit`, 8 steps)

- Peaks at every step: Copper Mile worst -2.0 dBFS (Pump 0), Brick Dust worst -3.6. No click, no `jump`, no `dead` flag. Sweep clips: none flagged.
- Weak spots, honestly: Copper Mile Sweep is front-loaded (the first step does 1.2 dB, the last 0.2) and measures only 1.1 dB overall because the drums are not in the filter; Swirl is mostly phase, so only 3 dB measured; Brick Dust Grind measures 2.1 dB (the last two steps 0.7 and 1.4, the first three 0.1 to 0.2): a crusher on a bass is mostly in the low harmonics. A harder Grind (bits 3, hold 20) jumped 18 dB in the last step, and a milder one (mix 80 %, bits 4, hold 12) measured `dead`, so this is the middle. Listen to Grind and Sweep first.
- Similarity of the difference signatures (cosine of the change vector; above 0.9 or below -0.9 is a flag):

Copper Mile

| | Sweep | Swirl | Pump | Room |
|---|---|---|---|---|
| Sweep | 1 | 0.90 | -0.69 | 0.31 |
| Swirl | 0.90 | 1 | -0.51 | 0.06 |
| Pump | -0.69 | -0.51 | 1 | -0.85 |
| Room | 0.31 | 0.06 | -0.85 | 1 |

Brick Dust

| | Grind | Squash | Push | Slap |
|---|---|---|---|---|
| Grind | 1 | 0.37 | 0.33 | -0.38 |
| Squash | 0.37 | 1 | 0.58 | -0.99 |
| Push | 0.33 | 0.58 | 1 | -0.67 |
| Slap | -0.38 | -0.99 | -0.67 | 1 |

Flagged pairs: Copper Mile Sweep-Swirl (0.90, both small changes) and Brick Dust Squash-Slap (-0.99). The measure is dominated by the stereo-width term: Squash narrows the sides, Slap widens them. They are different gestures to the ear (dynamics against space); the measure only sees the shared axis. Copper Mile Pump-Room (-0.85) is under the 0.9 flag.

## Default sound against the old patch (16 s clip, float, same phrase)

| Preset | Loudness | Peak | Difference render |
|---|---|---|---|
| Copper Mile | 0.00 dB | -6.18 / -6.18 dBFS | bit-identical (-240 dB) |
| Brick Dust | 0.00 dB | -3.88 old / -3.79 new dBFS | -38.8 dB; without the new crusher module it is -127 dB (float noise). The extra module shifts the module ids and the noise of the snare and hats is drawn differently; the sound is the same |

## How

`crates/ui/examples/presets_groove.rs`: `Spec::custom` lets a preset route macros 1 to 3 itself (`copper_plan`, `brick_plan`); the fourth stays the space macro. `span` and `rise` set a param so that the stored macro position gives exactly the old value (the old routes' effect is recomputed with `routed`), and the macro runs on in the same line to 1.

Not done: other presets keep their old macros; the Corner Store and Fault Line click is parked (see `MACRO-DEFECTS-2-HANDOVER.md` on `claude/groove-macro-defects-2`).

## Tours (old macros 0 to 1 to 0, then the new ones; each macro 8 s, 11 s apart)

`docs/presets/clips/macro-redesign/old/<slug>-tour.mp3` and `.../new/<slug>-tour.mp3`. Start times in each tour: old Filter 0 s, Phase (Dirt) 11 s, Pump 22 s, Room 33 s; new Sweep (Grind) 0 s, Swirl (Squash) 11 s, Pump (Push) 22 s, Room (Slap) 33 s. One clip per macro and an all-presets `tour.mp3` sit beside them (`tour.mp3` there holds only these two).

Old audit flags for comparison: Copper Mile Filter weak, Pump dead+jump; Brick Dust Filter weak, Dirt duplicate, Pump dead+jump, Room weak+duplicate. New: no flags on either preset.
