# Macro redesign: keys and plucks, first half (12 of 23)

Same method as `MACRO-REDESIGN.md` and `MACRO-REDESIGN-BASS-LEAD.md`: each preset gets four macros from what makes that patch interesting, four different kinds of change, clean by default, the stored (default) timbre unchanged. Builders: `tools/examples/presets_keys_pluck.rs` (shared stages, `Rack::span` / `span_hi` / `rise` / `level`, `routed`), `presets_keys_pluck/bank.rs` and `group2.rs`. This PR does the first twelve (Velvet Tine to Corduroy); the other eleven (Matinee, Snowmelt, Scaffold, Brass Filament, Spinet, Willow, Dry Creek, Gumdrop, Cold Cut, Rivet Gun, Tuning Fork) keep their old macros and their patch files are untouched (bit-identical renders).

Written from the patch recipes, the audit renders and Kosta's first listen (Spark Plug, Hand Bell, Checkerboard); the other nine are not heard yet. "What you hear" is what the parameters and measures say, the tours are the test.

How it is built: every keys preset had the same habit set (m1 cutoff and FM index, m2 drive, m3 space, m4 one extra). `Post { fixed: true, old: [..] }` stops those routes; the cutoff, drive, echo, room and chorus or phaser mix stand at the value the old route left at the old stored position (`routed`), and each preset routes its own macros with `span` (value at 0, stored value at the stored position, on in the same line to 1), `span_hi` (the value at 1 given), `rise` (a macro stored at 0) and `level` (below). The stored position of a macro is where its line passes through the old stored sound, so it is no longer the old number.

## Two rules from the listen (Kosta, applied to all twelve)

1. **Loudness.** A macro that is not about level (tone, motion, space, texture) keeps the loudness about constant over its travel: the drive trim follows the drive down (`drive.trim_db` on the same macro) or `Rack::level` moves the gain stage the other way by the measured amount (the stored level stays). Level-type macros may move it, on purpose: Turnstile Pump (the duck), Checkerboard Squash and Linoleum Squeeze (density), Velvet Tine Ring (a longer note holds more energy). `macro_audit` now measures this: the loudness at 1 against 0 and the swing over the 8 steps, flag `level` over 3 dB (`LEVEL_DB`, `--level-type slug/mK` for the level-type ones; PR #163, also here).
2. **Bolder.** Ranges are pushed to the ends of the parameters where the patch takes it (chorus depth 100 %, phaser feedback to 88 %, partial detunes out to 190 ct, Zap to 200 ct, attack 250 ms, release up to seconds), since the compensation and the combination check keep it safe.

Two facts the old presets hid, from the audit of the old macros:
- Cutoff was dead on Checkerboard, Spark Plug, Turnstile and weak on Corduroy: the old route left the ladder at 8 to 13 kHz at the stored position, wide open. The new "Bite", "Snap" and "Quack" start from a closed filter and end at the stored (open) position, stored at 1.0.
- The old Length (Checkerboard) and Decay (Spark Plug) routes put the decay at 4 to 7 seconds on stabs a quarter of a second long: dead. The decay stays at its stored length (same sound) and the slot went to a gesture that works.
- Spark Plug's and Corduroy's second oscillator is a **triangle** (waveform 1), so its pulse width does nothing; the old recipe comments say "pulse".

## Presets (stored position in brackets; the numbers are from the macro routes of the final patches: value at 0 to value at 1)

### Velvet Tine (soft Rhodes, FM)

| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Tine [0.40] | no metal, a plain soft body, to a bright tine that rings out of each note | index 0.3 to 1.93; level4 0 to 0.75 |
| Ring [0.50] | a short, damped note to a long note (level-type: a longer note holds more energy) | decay1 510 to 9.5e+03; gain_db 10.1 to 8.1; decay5 606 to 9.5e+03 |
| Detune [0.00] | a pure tine to a beating, chorused one as the partner tine drifts out of tune (off by default) | fine5 4 to 90; gain_db 10.1 to 10.9 |
| Phase [0.25] | a still, thin phaser to a fast, deep swirl | mix 0 to 70; rate_hz 0.171 to 3; depth 47.3 to 98; feedback 4 to 88 |

### Checkerboard (house piano stab, driven and squashed)

| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Bite [1.00] | a closed, thudding stab whose filter snaps open on every note, to the open bark | cutoff_hz 400 to 8.44e+03; env.adsr on filter.ladder.cutoff_hz; decay_ms 40 to 180 |
| Hollow [0.35] | a timbre change at constant loudness, nothing like Squash: the stab goes from a bright saw stack to a hollow, woody one as the triangle (odd harmonics, no top) takes over from the saws and the body stays; the gain follows so the level does not move (0.0 dB at 1) | level2 0 to 1; level1 0.846 to 0 (the saws are gone at 1: a pure triangle); gain_db -1.0 dB at 1 (the compressor after it takes back part of any gain, so the compensation is small and negative) |
| Squash [0.70] | a light compressor to a hard clamp (density: the level may move; the makeup follows only in part) | threshold_db -8 to -28; ratio 2 to 7.71; makeup_db 2 to 6.29; gain_db 2.2 to 6.2 |
| Spread [0.00] | the saw stack widens and drifts apart, mono to wide (off by default) | detune 12 to 65; width 0 to 100; fine 5 to 30 |

### Linoleum (phased disco electric piano)

| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Swirl [0.80] | a thin, slow phaser to a deep, fast, ringing one | rate_hz 0.1 to 1.49; mix 5 to 67.5; feedback 0 to 75; depth 57 to 92 |
| Chime [0.60] | a plain electric piano to one with a bell partial | index 0.3 to 1.3; level4 0 to 0.667 |
| Hold [0.60] | a damped tick to a long ring | decay1 220 to 2.3e+03; decay3 220 to 2.3e+03; decay5 220 to 2.3e+03 |
| Squeeze [0.90] | a light compressor to a hard clamp (density: the level may move) | threshold_db -10 to -25.6; ratio 2 to 5.33; makeup_db 2 to 5.33; gain_db -0.6 to 3.4 |

### Cellar Door (dark hip-hop electric piano)

| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Dark [0.75] | a muffled felt tone to the open stored tone (the stored cutoff was 9.6 kHz) | cutoff_hz 450 to 1.98e+04; index 0.12 to 0.893 |
| Felt [0.00] | a hammer that hits at once to one that pushes the note in slowly (off by default) | attack1 2 to 250; attack2 2 to 250; attack3 2 to 250; attack5 2 to 250; attack6 2 to 250 |
| Wobble [0.00] | a slow tape pitch drift that fades in and speeds up (off by default) | lfo on osc.fm6.fine; rate_hz 0.4 to 3 |
| Smear [0.30] | a dry key to a deep, slow chorus | mix 0 to 80; depth 58 to 98; rate_hz 0.151 to 2.5 |

### Pulpit (drawbar organ through a rotor)

| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Bars [0.55] | a thin, flute-like organ to a full, deep one | level1 0.12 to 0.993; gain_db 2.1 to 0.1; level3 0.556 to 1 |
| Perc [0.00] | each key strikes with a bright 3rd, 4th and 6th harmonic that fades to a softer organ (off by default: the bars hold) | sustain4 1 to 0; decay4 50 to 150; level4 0.5 to 0.9; sustain5 1 to 0; decay5 50 to 150; level5 0.4 to 0.9; sustain6 1 to 0; decay6 50 to 150; level6 0.25 to 0.9 |
| Rotor [0.75] | a slow, thin rotor to a fast, swirling one | rate_hz 2 to 7.71; depth 6 to 27.3; mix 4 to 35.3 |
| Overdrive [0.30] | a clean organ to a hot, tubey one, the trim following the drive down | drive_db 0 to 26.5; trim_db -0.5 to -5.5 |

### Rosewood (marimba)

| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Mallet [0.80] | a felt mallet to a hard mallet | index 0.6 to 1.16; level4 0 to 0.375 |
| Ring [0.50] | a damped bar to a bar that rings on after the key is up (the old Ring moved decays of 2.6 to 10 s, longer than any note; they stay, the macro works on the release) | release1 36.6 to 700; release3 20 to 500 |
| Beat [0.08] | the two fourth partials drift apart and beat | fine3 0 to 150; level3 0.365 to 0.8 |
| Roll [0.00] | a tremolo roll of the bars, up to about 4 dB either way, fades in (off by default) | lfo on gain.gain_db; rate_hz 5 to 13 |

### Rain Gutter (kalimba with a long, wet tail)

| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Tine [0.65] | a soft thumb piano to a bright, pinging one | index 0.3 to 1.38; level2 0 to 0.769; level4 0 to 0.462 |
| Ring [0.50] | a damped tick to a tine that rings for seconds | decay1 300 to 9.63e+03; decay3 400 to 1e+04; gain_db 12.2 to 5.2 |
| Echo [0.60] | a short echo and a small room to a long, ringing echo and room (the wet level stays as it was; this is the one macro that owns the space) | decay_s 0.6 to 14.2; feedback 0 to 70 |
| Shimmer [0.30] | a dry tone to a deep chorus | mix 0 to 73.3; depth 14.3 to 100; rate_hz 0.169 to 3 |

### Spark Plug (blunt, driven, compressed pluck)

| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Snap [1.00] | a dull thud whose filter snaps open on every note, to a crack | cutoff_hz 300 to 7.31e+03; env.adsr on filter.ladder.cutoff_hz; decay_ms 30 to 110 |
| Grit [0.70] | a clean blunt pluck to a crushed one, the trim following the drive down | drive_db 6 to 32.4; trim_db 5 to -12.1 |
| Ring [0.60] | a staccato tick to a pluck that rings on after the key is up (replaces Squash, which overlapped Snap and Grit) | release_ms 12 to 175; gain_db 8 to 3 |
| Zap [0.00] | each pluck starts sharp, up to 200 ct, and falls into tune (off by default) | env.adsr on osc.va.fine |

### Tin Roof (Wurlitzer-like electric piano)

| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Reed [0.50] | a plain electric piano to a buzzing, reedy one | level5 0.1 to 0.9; fine5 0 to 14; level4 0 to 0.7 |
| Sustain [0.50] | a short, dry tine to a long, held note | decay1 200 to 7.2e+03; decay5 200 to 7.2e+03; gain_db 6.5 to 3.5 |
| Bark [0.60] | a clean tone to an overdriven one, the trim following the drive down | drive_db 0 to 24.6; trim_db 3.5 to -7.33 |
| Chorus [0.30] | a dry tone to a deep, fast chorus | mix 0 to 66.7; depth 7.14 to 100; rate_hz 0.432 to 5 |

### Turnstile (driven, phased saw chord pumping on the host clock, 123 bpm)

| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Pump [0.45] | the chord does not duck, then ducks harder and longer on every beat (level-type; press Run) | env.adsr on gain.gain_db; decay_ms 140 to 422 |
| Swirl [0.60] | a thin, slow phaser to a deep, fast, ringing one | mix 0 to 66.7; rate_hz 0.1 to 1.01; feedback 5 to 80; depth 53 to 98; gain_db 0.7 to 0.7 |
| Grit [0.60] | a clean saw chord to a crushed one, the trim following the drive down | drive_db 2 to 32.5; trim_db 2.5 to -10 |
| Spread [0.35] | a tight chord to a wide one, the triangle drifting off the saws | detune 2 to 36.3; fine -30 to 38.6 |

### Hand Bell (inharmonic FM bell; echo and room are constant)

| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Strike [0.80] | a dull, muted bell to a hard clapper with bright metal | index 0.2 to 1.14 |
| Ring [0.50] | a damped bell to one that rings on after the strike (the one macro that touches the space) | release1 60 to 6e+03; release3 60 to 4.17e+03; release5 60 to 2.67e+03; gain_db 7.75 to 5.25 |
| Shimmer [0.25] | a dry bell to a deep, fast chorus (replaces Tail, which made the bell sound holy) | mix 0 to 80; depth 26.7 to 100; rate_hz 0.139 to 3 |
| Clang [0.50] | a tuned bell to a clangorous one, the partials pulling apart (fine tunes out to about 190 ct) | fine2 0 to 534; fine4 -150 to 178; fine6 150 to -188 |

### Corduroy (funk clavinet)

| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Quack [1.00] | a closed, dull clav whose filter snaps open on every note, to the open one | cutoff_hz 500 to 8.44e+03; env.adsr on filter.ladder.cutoff_hz; decay_ms 30 to 120 |
| Clack [0.00] | each pluck starts a little sharp, up to 120 ct, and falls into tune, the clav's clack (off by default) | env.adsr on osc.va.fine |
| Damp [0.50] | a choked string to one that rings out | decay_ms 20 to 1.44e+03; release_ms 6 to 417; gain_db 14 to 5 |
| Phase [0.30] | a dry clav to a deep, fast phaser | mix 0 to 55; rate_hz 0.242 to 5; depth 58 to 98; feedback 12.3 to 88 |

## Count

Every preset has 4. No 3 and no 5+ in this half: where the old design had a dead or doubled macro, the slot went to a distinct gesture (Checkerboard Hollow, Spark Plug Ring, Cellar Door Felt, Corduroy Clack, Velvet Tine Detune).

## Kosta's notes, and what was done

- **Spark Plug:** Squash "may overlap Snap/Grit" and was replaced by **Ring** (the release of the amp and filter envelopes: a staccato tick to a pluck that rings on); Zap is louder (route 0.7 to 1.0, up to 200 ct). The compressor stays at its stored setting.
- **Hand Bell:** Strike and Ring kept (Ring is bolder). **Tail** made the bell "holy" and overlapped with Ring and the echo: it is gone, echo and room are a constant wet amount at the old stored level, and the slot is **Shimmer** (the chorus). Only Ring touches the space. Clang is wider.
- **Checkerboard:** Bite and Squash kept, Spread bolder (detune 65 ct, width 100 %), **Hollow** is a level-compensated timbre change (Kosta: the first version "made it quieter, too much like Squash"): the triangle comes up to 1.0 while the saws back off to 0, a pure triangle at 1 (Kosta: "more", then "remove the saws completely"; the top is 13 dB down at 1), loudness within 0.2 dB over the travel, stored sound unchanged.
- **Rain Gutter:** Tail is renamed **Echo** and is the one macro that owns the space (reverb decay, echo feedback; the wet level stays).

## Old flags against the final audit

| Preset | Old flags | Final flags |
|---|---|---|
| Velvet Tine | Drive peak 0.7 dBFS at 1, Phase peak at 0, Tine/Drive cos .82; three pairs over -1 dBFS Phase `level` (swing 4.2 dB, the phaser's notches); Tine/Ring opposite |
| Checkerboard | Cutoff dead, Length dead | clean; Bite/Hollow opposite (cos -.98 or beyond: one brightens, the other hollows; the audit's `duplicate` on the shared trim gain with Squash is the level compensation, not a doubled move); every pair peaks at -3.5 dBFS or lower |
| Linoleum | Bright/Crunch cos .81 | clean |
| Cellar Door | Dirt pileups | clean |
| Pulpit | Brightness dead + jump, Room jump | Perc/Overdrive opposite (cos -.98, Perc darkens and Overdrive brightens) |
| Rosewood | Room jump, Ring jump, Warmth/Ring cos .84 | Mallet/Beat cos .84 |
| Rain Gutter | Space jump | Tine/Ring opposite (cos -.99) |
| Spark Plug | Cutoff dead, Decay dead | clean |
| Tin Roof | Room jump | clean |
| Turnstile | Cutoff dead | Swirl and Grit `level` (swing 4.0 and 4.4 dB, not monotonic: the phaser and the drive) |
| Hand Bell | Space jump | clean |
| Corduroy | Cutoff weak, Drive+Room peak -0.3 | Quack `jump` (the filter opens in the first steps) |

Peaks: every step, sweep and combination render is at or under -1 dBFS. No click in any end or sweep render. The factory level test (all macros at 0.15, 0.65, hot) passes.

### Open flags (honest list)

- **Velvet Tine** is in `LOUD_EXCEPTIONS` (`crates/ui/tests/factory_bank.rs`, with Deep Field and Salt Flat): its chord strike peaks within a dB of the ceiling with Ring and Phase up, so its gain (-3.9 dB, was -2.7) is set by the -1 dBFS rule on the audit clip; the loudness of the level test sits 1.4 dB under the window (-26 at all macros at 0.15) and the stored sound is 1.2 dB quieter than before. I took the exception (under 2 dB lost) rather than soften the strike in the patch.
- The "opposite" pairs are different gestures that happen to push the measure in opposite directions (bright against dark); I listened to none of them.
- Weak by the meters, pitch or phase moves the measure does not see: Cellar Door Wobble and Smear, Pulpit Rotor and Perc, Rosewood Beat, Spark Plug Zap, Hand Bell Shimmer and Clang, Corduroy Clack. The tours are the test.

## Default sound against master (stored position)

The stored timbre of all twelve is the old one (a 16 s clip against master, -76 dB or better below the signal, i.e. float noise, before the level changes below). The patch gain changed on purpose for headroom and the level rule (`preset_trims.inc`), loudness of the chord measurement (`presets_keys_pluck levels`, dBFS) old to new:

| Preset | Trim | Loudness |
|---|---|---|
| Velvet Tine | -2.7 to -3.9 dB | -23.1 to -24.3 (an exception in the level test, below) |
| Checkerboard | -0.5 to -4.0 dB | -18.9 to -19.8 |
| Linoleum | -1.6 to -7.0 dB | -22.0 to -23.5 |
| Rosewood | -2.2 to -2.0 dB | -20.4 to -20.2 |
| Rain Gutter | -3.3 to -2.3 dB | -22.8 to -21.9 |
| Spark Plug | -0.6 to -5.0 dB | -19.6 to -20.6 |
| Turnstile | -4.8 to -8.3 dB | -17.1 to -20.5 |
| Hand Bell | -4.0 to -2.5 dB | -22.8 to -21.3 |
| Corduroy | -0.2 to -1.5 dB | -20.2 to -21.5 |

Cellar Door, Pulpit and Tin Roof are at the old level. Linoleum (-23.5) is the quietest and could not go up: its Swirl and Squeeze together reach the ceiling. Hand Bell lost Space (see above).

## What did not work

- `Rack::level` as a fix for peaks: it moves the level at 0 by the same amount, and peaks follow the transients, not the loudness. It is now used for the level rule only, with measured amounts.
- Tremolo on Velvet Tine and Roll on Rosewood as negative-only gain wobbles (an LFO on a gain is bipolar; held under the stored peak it loses about 1.6 to 4 dB of loudness, which the level test and the level rule do not allow): Velvet Tine's is replaced by Detune, Rosewood's Roll is a plain ±4 dB wobble under the headroom of that patch.
- Cellar Door "Hammer" (a thud operator): same change as "Dark" (cos .96), replaced by "Felt" (attack). Corduroy "Pickup": same as "Quack" (cos .99), replaced by "Clack". Checkerboard "Stab" (length): dead above 0.4 s on a quarter-second stab, replaced by "Hollow". Pulpit "Bars" as the upper drawbars: same as Overdrive (cos .95), now the sub and the second bar.

## Tours

`docs/presets/clips/macros-keys-pluck/` (git-ignored): `<slug>-tour.mp3` per preset, new macros 1 to 4 back to back (20 s each: 6 s at 0, 4 s up, 6 s at 1, 4 s down; 1 s between; 84 s per file) and `tours.txt` with the start seconds. Gap guard on, all pass. Tempi: Velvet Tine 84, Checkerboard 124, Linoleum 122, Cellar Door 80, Pulpit 120, Rosewood 120, Rain Gutter 120, Spark Plug 124, Tin Roof 100, Turnstile 123, Hand Bell 120, Corduroy 104 (`.kabl-agent`-free regeneration: `MACRO_TOUR_BPM=<bpm> macro_audit patches phrases renders slug`, then `macro_tour_ab.py`). Turnstile's Pump needs the clock; the tour mode starts it. Listened by Kosta: Spark Plug, Hand Bell, Checkerboard (first versions, notes above). Listen next: Velvet Tine (Detune and the open peak flag), Rosewood Ring and Roll, Pulpit Perc, Rain Gutter Echo.

# Second half: Matinee to Tuning Fork (11 of 23)

Same method and builders as above (`presets_keys_pluck/group2.rs`: `Post { fixed: true, old: [..] }`, `span` / `span_hi` / `rise` / `level`). The old habit set (m1 cutoff or index, m2 drive, m3 space, m4 one extra) is gone: every macro is a gesture of that patch and the stored (default) timbre is the old one. A macro is stored where its line passes through the old sound, so several sit at 1.0 or 0.0 because the old routes left the filter or the decay at its far end (Matinee Glass, Spinet Jack, Willow Ring, Cold Cut Ring). Every preset has **4 macros; no 3 and no 5+**. Space is the constant old stored wet amount, except the **Hall** macros of Matinee, Snowmelt and Willow, which own the space: reverb decay and echo feedback move, the wet level stays.

Written from the patch recipes and the audit renders (steps, sweeps, combinations); nobody has listened yet. "What you hear" is what the parameters and meters say; the tours are the test.

Two findings of the audit of the old macros, as in the first half: the old cutoff routes left the ladder wide open at the stored position (Matinee 19 kHz, Spinet and Dry Creek fully open, Gumdrop 10 kHz), and the old decay routes of Scaffold, Gumdrop, Willow and Cold Cut sat at 1.4 to 10 s on notes a fraction of a second long. Those slots became gestures that work (Matinee Square, Scaffold Tail on the release, Gumdrop Hold, Cold Cut Click).

## Presets (stored position in brackets)

### Matinee (seven detuned saws, soft filter, big room)

| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Swell [0.00] | each chord is struck at once to one that pushes in over 1.4 s (level-type, off by default) | attack of amp and filter envelope 8 to 1400 ms |
| Glass [1.00] | a dark, closed pad to the stored bright, open one | cutoff 450 Hz to 19 kHz |
| Square [0.50] | the bright saw stack to a hollow, woody square pad at the same loudness | square level 0 to 0.6, saw level 0.75 to 0.25, gain -3 dB at 1 |
| Hall [0.60] | a small room and one echo to a huge, ringing hall (the wet level stays) | reverb decay 0.4 to 14 s, echo feedback 8 to 58 % |

### Snowmelt (soft, slow electric piano, six-second room)

| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Tine [0.40] | a soft, round key to a glassy one | index 0.1 to 1.8, hammer partial 0 to 0.375 |
| Bloom [0.00] | each key struck at once to a chord that grows in over 1.5 s (level-type, off; the hammer thud stays) | attack of five operators 12 to 1500 ms |
| Drift [0.50] | a still tone to a beating partner tine under a deep, slow chorus | chorus mix 0 to 60, depth 10 to 100, rate 0.07 to 0.9 Hz; partner tine fine 3 to 28 ct |
| Hall [0.65] | the room grows from small to huge, ten seconds and more (the wet level stays) | reverb decay 0.6 to 20 s, echo feedback 8 to 65 %, gain -2.5 dB at 1 |

### Scaffold (dark, wide drum-and-bass stab)

| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Shut [0.78] | a shut, boxed-in stab to one whose filter snaps open and closes on every hit | cutoff 200 Hz to 16 kHz, filter decay 50 to 414 ms |
| Stack [0.50] | a tight pair of stacks to a vast, ripping wall | detune 3 to 49 ct, second stack detune 2 to 34, fine -2 to -22 |
| Swirl [0.50] | a still stab to a deep, ringing phaser | mix 0 to 70, rate 0.1 to 0.4 Hz, depth 52 to 98, feedback 5 to 88 |
| Tail [0.60] | a clipped tick to a stab that rings on after the key is up | release 12 ms to 0.5 s, gain -3 dB at 1 |

### Brass Filament (sustained metallic FM, algorithm 5)

| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Blow [0.00] | a hard strike to a tone bowed in over 450 ms (level-type, off) | attack of all six operators 3 to 450 ms |
| Alloy [0.50] | a tuned tone to a clangorous metal, the partials pulled out of tune | fine of partials 2, 4 and 5: 0 to 280, 0 to -180, 0 to 60 ct |
| Hold [0.00] | a tone that fades away to one that holds at full (level-type, off) | sustain of all operators to 1.0 |
| Warp [0.50] | smooth to a rough, tearing metal that closes in as it tears | feedback 0.02 to 1.0, cutoff 20 kHz to 4.5 kHz |

### Spinet (harpsichord-like pluck)

| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Jack [1.00] | a dull, boxed key to the stored bright pluck whose filter snaps on every note | cutoff 700 Hz to 20 kHz, filter decay 40 to 300 ms |
| Honky [0.15] | two strings in tune to a second one pulled out of tune, a honky-tonk beating | second string fine 2 to 48 ct |
| Sustain [0.50] | a damped tick to a string that rings on after the key is up | release 6 to 600 ms, gain -3 dB at 1 |
| Double [0.35] | one string to two, a deep chorus doubling it | chorus mix 0 to 50, depth 10 to 67, rate 0.35 to 2.5 Hz |

### Willow (soft harp)

| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Nail [0.50] | a soft fingertip to a bright, clicking nail | index 0.1 to 1.7, two fast partials 0 to 0.6 and 0.4 |
| Ring [1.00] | a damped, quickly dying string to one that sings for ten seconds | decay of two operators to 10 s, release 60 to 500 ms, gain -6 dB at 1 |
| Halo [0.35] | a plain string to a shimmering one, the octave partial drifting out of tune | fine of partial 3, 4 to 40 ct |
| Hall [0.60] | a small room to a huge hall with a long echo (the wet level stays) | reverb decay 0.7 to 16 s, echo feedback 8 to 61 % |

### Dry Creek (nylon-string guitar)

| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Pick [0.50] | a soft fingertip to a hard, clicking pick | two fast click partials 0 to 0.6 and 0.4 |
| Nylon [0.00] | the bright stored string to a dark, round nylon one (off by default) | cutoff 20 kHz to 700 Hz, drive to 0, gain +3 dB at 1 |
| Ring [0.50] | a muted, palm-damped note to a string let ring | decay of two operators 150 ms to 5.4 s, release 20 to 220 ms, gain -9 dB at 1 |
| Double [0.30] | one string to a doubled, chorused pair | chorus mix 0 to 50, depth 0 to 100, rate 0.25 to 2.5 Hz; partner string fine 0 to 10 ct |

### Gumdrop (bouncy pop pluck)

| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Bounce [0.85] | a flat, dull pluck to a squelching, resonant bounce on every note | cutoff 300 Hz to 10 kHz, resonance 0.1 to 0.4 |
| Hold [0.80] | a clipped dot to a long, bouncing note | decay 130 ms to 8 s, release 80 to 120 ms, gain -5 dB at 1 |
| Boing [0.00] | each note drops in from above, up to 120 ct, and falls into tune (off) | filter envelope on both oscillators' fine tune |
| Chorus [0.30] | a dry pluck to a deep, fast chorus | mix 0 to 83, depth 10 to 93, rate 0.6 to 3 Hz |

### Cold Cut (dark bell-pluck for trap)

| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Click [0.50] | a hollow knock to a hard, metallic click on top | the metal partial (ratio 3, 267 ct) 0 to 1.0 |
| Ring [1.00] | a dead tick to a bell that hangs in the air | decay of two operators 120 ms to 6.5 s, release 30 to 150 ms, gain -7 dB at 1 |
| Grit [0.40] | a clean, hollow bell to a crushed one, the trim following the drive down | drive 2 to 20 dB, trim 0 to -7 dB |
| Dark [0.17] | the open stored tone to a muffled, closed bell (the gain follows the filter down) | cutoff 20 kHz to 1.2 kHz, gain +8 dB at 1 |

### Rivet Gun (bright, phased, compressed staccato pluck)

| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Gate [0.50] | a clipped tick to a rivet that holds and rings on | decay 30 to 120 ms (and on), release 8 to 40 ms, gain -7 dB at 1 |
| Phase [0.45] | a dry rivet to a deep, fast, ringing phaser | mix 0 to 45, rate 0.5 to 4 Hz, feedback 40 to 90, gain +4 dB at 1 |
| Snap [1.00] | a closed, dull rivet to the stored one whose filter snaps open on every hit | cutoff 450 Hz to 15 kHz |
| Stack [0.40] | a tight pulse to a wide, ratty stack | detune 2 to 32 ct, second oscillator fine 0 to 17 ct |

### Tuning Fork (tuned metal bars)

| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Overtones [0.50] | a pure fork to a clangorous bar | partial levels 2, 3, 4 up to twice the stored value |
| Ring [0.85] | a damped bar to one that sings for seconds | decay of three operators to 6 s, release 60 to 400 ms, gain -4 dB at 1 |
| Beat [0.15] | the doubled partials drift apart from unison to a slow, wide beating | fine 5: 0 to 33 ct, fine 6: 0 to 53 ct |
| Bowed [0.00] | each bar struck to one that is bowed in over 250 ms (level-type, off) | attack of all operators 0.5 to 250 ms |

Numbers are from the macro routes; the stored sound is the old one (see "Default sound").

## Flags: final audit

Level rule as in the first half: Swell, Bloom, Blow, Hold and Bowed are level-type (`--level-type`); every other macro keeps the loudness within about 3 dB over its travel (the audit measures with the phrase; compensation with `Rack::level`).

| Preset | Flags left |
|---|---|
| Matinee | Glass `dead` and `jump` (the stored filter is wide open: on a log axis the top third does little; same as Bite and Quack in the first half), Swell `jump` (an attack time on a log axis); Glass/Square cos -0.93 (one brightens, the other hollows) |
| Snowmelt | Bloom `jump` (attack on a log axis) |
| Scaffold | Shut `jump` (the filter opens in the first steps) |
| Brass Filament | Blow/Warp cos 0.83 (both small moves of the audit measure; one changes the onset, the other the roughness) |
| Spinet | Jack and Sustain `jump` |
| Willow | Nail/Halo cos -0.95 (Nail brightens, the beating Halo takes it away) |
| Dry Creek | Nylon `jump`; Pick/Nylon opposite (cos -0.95); Nylon/Ring cos 0.87 (both darken the sustained part) |
| Gumdrop | Bounce and Hold `jump` |
| Cold Cut | Ring `jump`; Click/Grit cos 0.82; Click/Dark opposite (one adds top, the other closes it); `duplicate` on the shared trim gain (the level compensation, not a doubled move) |
| Rivet Gun | `duplicate` on the shared trim gain only |
| Tuning Fork | `duplicate` on the shared trim gain only |

Peaks: every step, sweep and combination render is at or under -1 dBFS; no combination flag in any preset; no click in any end or sweep render. The factory level test (all macros at 0.15, 0.65 and 1, and each alone at 1) passes.

## Default sound against master (stored position)

The stored timbre is the old one: a 16 s clip against master with the overall gain divided out is -130 dB (float noise) for Matinee, Snowmelt, Brass Filament, Spinet, Willow, Dry Creek, Gumdrop and Tuning Fork. Scaffold, Cold Cut and Rivet Gun read -37, -28 and -23 dB after the gain is divided out: their patch gain changed on purpose and the compressor behind it takes part of any gain back (same as Checkerboard in the first half). The patch gain (`preset_trims.inc`) changed for headroom, presence and the compensation rule:

| Preset | Trim | Why |
|---|---|---|
| Snowmelt | -0.6 to -2.6 dB | the all-1 combination peaked at -0.5 dBFS |
| Scaffold | 0 to -1.0 dB | headroom for Swirl |
| Brass Filament | -2.0 to -0.5 dB | presence |
| Spinet | -3.0 to -0.5 dB | presence |
| Willow | -5.0 to -3.0 dB | presence |
| Dry Creek | -3.0 to -4.5 dB | the Ring compensation raises Ring at 0 |
| Gumdrop | -3.0 to -2.0 dB | the Hold compensation |
| Cold Cut | -3.0 to -8.0 dB | the Ring and Dark compensations raise the peak at 0 |
| Rivet Gun | -0.9 to -4.5 dB | the stored chord peaked at +0.2 dBFS |
| Tuning Fork | -5.0 to -3.5 dB | presence |

The preset descriptions and the library test entries name the new macros.

## Notes on the design

- **Matinee:** the old Width (the detune) became **Square**, a timbre change at constant loudness (Checkerboard Hollow's trick), because Stack and Hall both moved the side level (cos 0.93).
- **Brass Filament:** the first version had four brightening macros (cos 0.82 to 0.90). Strike became **Blow** (the attack), Alloy works on the tuning of the partials, Warp closes the filter while it tears.
- **Spinet:** Buzz (a louder second saw) overlapped Jack (cos 0.98), so it is **Honky**, the detune of the second string alone. The decay route was dead (the notes are 0.16 s) and is gone; Sustain works on the release.
- **Willow:** Halo is the detune of an octave partial, not a level (the level version overlapped Nail, cos 0.94).
- **Dry Creek:** Steel (cutoff and drive up) overlapped Pick (cos 0.91), so it is **Nylon**, the same move the other way, stored at 0.
- **Rivet Gun:** Crunch (the drive) overlapped Gate (cos 0.82); the slot is **Snap**, the filter, stored at 1.0.
- **Cold Cut:** Dark runs from the open stored tone down (stored 0.17); the first version, closed to open, was cos 0.85 with Click.

## Open flags (honest list)

- Chord-held loudness (the factory test) and phrase loudness (the audit) disagree for the long-ring macros (Willow Ring, Gumdrop Hold, Cold Cut Ring, Dry Creek Ring): the compensation follows the audit phrase, so a three-second held chord is quieter at 0 than at 1. The peak rule holds either way.
- Willow Nail/Halo and Matinee Glass/Square are opposite by the meters (different gestures that push the top in opposite directions); nobody has listened.
- Over the 0.8 line by the meters, left on purpose until someone has listened: Brass Filament Blow/Warp (0.83), Cold Cut Click/Grit (0.82), Dry Creek Nylon/Ring (0.87).
- Weak by the meters, pitch or phase moves they do not see: Spinet Honky, Willow Halo, Tuning Fork Beat, Gumdrop Boing, Snowmelt Drift. The tours are the test.

## Tours

`docs/presets/clips/macros-keys-pluck-2/` (git-ignored): `<slug>-tour.mp3` per preset (new macros 1 to 4, 20 s each: 6 s at 0, 4 s up, 6 s at 1, 4 s down, 1 s between) and `tours.txt` with the start seconds. Gap guard on. Tempi: Matinee 90, Snowmelt 70, Scaffold 174, Brass Filament 60, Spinet 120, Willow 100, Dry Creek 100, Gumdrop 110, Cold Cut 140, Rivet Gun 125, Tuning Fork 120. Regenerate: `MACRO_TOUR_BPM=<bpm> macro_audit patches phrases renders slug`, then `macro_tour_ab.py`. **Listen first:** Brass Filament (Blow, Alloy, Warp are new gestures), Cold Cut (Dark runs the other way, Ring on a sparse line), Tuning Fork (Beat and Bowed).
