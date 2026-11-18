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
