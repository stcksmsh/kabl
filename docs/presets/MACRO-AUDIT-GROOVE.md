# Macro audit: Groove

Pilot for the macro audit. Kosta judges by ear whether this format works before other categories are done. Audit only: no preset was changed. 14 of the 15 Groove presets are covered; Pinball is left out (it is being replaced). Loading Dock is a self-playing piece with cues: it is audited with no cue launched, so it plays its stored loop (the macros sit at their stored values) and the cues do not move the macros.

## Method

- Each preset plays its clip phrase (the keyboard part plus the running groove, free-running clock from 0.5 s) for 10 s, offline, at 48 kHz. One macro at a time moves, the other three stay at their stored values.
- Steps: the macro alone at 0, 1/7 ... 1 (8 renders). Sweep: the macro moves 0 to 1 to 0 over 8 s (1 s to 9 s) at runtime, like a hand on the knob; that is the clip, and the zipper check.
- Routes come from the patch: each macro's cables into module parameters. The value at macro 0, 0.5 and 1 is the stored base plus macro times the route amount, in knob travel (exponential knobs such as cutoff are exponential).
- `change` (dB) is the distance between two renders: the larger of the third-octave spectral distance and the 10 ms envelope distance. A phase-only difference is 0; it is shown separately as `waveform` (rms of the difference against the render). A step pair under 0.15 dB is a dead step; two or more dead steps while 0 against 1 moves is a dead zone; a pair more than 3 times the median pair is a jump.
- End checks at macro 0 and 1: peak above -1 dBFS, silence, level 12 dB under or 9 dB over the middle, CLICK (`audio_defects.py`). Zipper: a CLICK in the sweep that the end renders do not have.
- Overlap: two macros that change the same measures the same way (cosine over 0.9), or drive the same parameter. Treat it as a hint; the verdict says when it is only the measure.
- The measures hear spectrum, level, width, density and the waveform. They do not hear taste: every `ok` still wants Kosta's ear.
- Rerun for another category: `cargo build --release -p kabl-ui --example macro_audit`, write that generator's `phrases`, run `macro_audit <patches> <phrases.json> <renders> slug...`, then `docs/presets/scripts/macro_audit.py` (see its header). `--verdicts`, `--intro` and `--doc` write this kind of report.

## Findings

1. **Every Filter macro has a dead top.** Cutoff macros (Copper Mile, Brick Dust, Ore Cart, Stoop Sale, Fault Line, Corner Store, Glass Coast, Rope Walk, Lamp Oil, Dune Buggy; Loading Dock's Sweep is even but too hot, see 8) run from a base of 0.9 to 2.5 kHz to 20 kHz. The sound stops changing between macro 0.55 and 0.75 because the filter is already past the content. Effect: about the top third of the knob does nothing. Fix idea: shorter route amount (about 0.3 to 0.4 of travel) or a higher base.
2. **Pump (six presets) is a switch, not a knob.** Copper Mile, Brick Dust, Ore Cart, Rope Walk, Scrap Metal and Loading Dock route macro 3 to the kick-duck compressor ratio, 1 to 19. The first step (1/7) already does 80% of the change (5.7 to 6.8 dB; Loading Dock 6.5 dB) and nothing moves above 0.3. Fix idea: route it to threshold or to a ratio range of about 1 to 6, or to the duck's release.
3. **Clicks while moving or at an end:** Fault Line Filter (5.8 s), Corner Store Filter (3.7 s while moving, 7.6 s at the end render), Rope Walk Pump (8.9 s at the end render; also +0.1 dBFS peak at macro 0).
4. **Slab City Tone does nothing audible** (it opens the filter of a sine 808). **Slab City Glide** is silent below 0.8 (log scale).
5. **Fault Line Hats** has no effect (0.04 dB), **Glass Coast Hats** barely (0.4 dB).
6. **Room/Space macros are all alike** (mono to stereo, a few dB). They work, even steps. Brick Dust Room is weak (0.5 dB, 15%/10% mix at most).
7. **Overlap:** Lamp Oil Chord and Dub, and Ore Cart Rumble and Space, share a measured signature, but they drive different layers; not a duplicate.
8. **Loading Dock:** Sweep clips: the peak is above 0 dBFS from macro 0.57 up (+1.4 dBFS). Pump peaks at -0.6 dBFS at macro 0. Drive is weak (0.75 dB). Wash jumps 5 dB in its first step.


## Flagged macros (28 of 56)

- copper-mile m1 Filter: **weak**, cutoff 1.4k to 20k moves only +2.4 dB near 4.5 kHz, and nothing above macro 0.6 (the filter is already open)
- copper-mile m3 Pump: **dead zone**, kick-duck ratio 1 to 19: the first step (1/7) does 80% of the change, 0.3 to 1 is flat; also a jump of 5.7 dB at the bottom
- brick-dust m1 Filter: **weak**, +2.5 dB near 1.8 kHz over the whole travel, flat above 0.7
- brick-dust m2 Dirt: **weak**, drive 20 to 36 dB changes the stereo side level (+3 dB) but not level or brightness; the drive is already saturated at 20 dB. Overlap with Room is by the measure only
- brick-dust m3 Pump: **dead zone**, kick-duck ratio: 6.8 dB jump in the first step, flat from 0.3 up
- brick-dust m4 Room: **weak**, mix up to 15%/10% moves the render by only 0.5 dB
- ore-cart m1 Filter: **weak**, 0.6 dB, nothing you can name from the measures; flat above 0.6
- ore-cart m2 Rumble: **weak**, level of the rumble layer 0.40 to 0.76 changes the render by 1.6 dB and only as stereo width, no low-end change; same signature as Space
- ore-cart m3 Pump: **dead zone**, kick-duck ratio: 6.8 dB jump in the first step, flat from 0.3 up
- stoop-sale m3 Filter: **weak**, 0.4 dB, flat in the top 30%
- fault-line m2 Filter: **zipper**, click while the cutoff moves at 5.8 s; steps flat above 0.6 (cutoff already beyond the content)
- fault-line m3 Hats: **dead**, hat level 0 to 0.4 changes the render by 0.04 dB: not audible
- corner-store m2 Filter: **bad end**, CLICK in the macro-1 render at 7.6 s and one while moving at 3.7 s; steps flat above 0.6
- glass-coast m1 Filter: **dead zone**, cutoff 2.5k to 20k: +3.6 dB top over the first half, zero change from 0.57 up
- glass-coast m2 Hats: **weak**, hats level: 0.4 dB, +2 dB near 11 kHz
- still-water m1 Bloom: **dead zone**, Bloom: +9 dB near 2.2 kHz in the first half, zero change from 0.71 up
- slab-city m1 Tone: **dead**, Tone drives the 808's ladder only: 1 kHz to 20 kHz on a sine changes its phase (waveform -16 dB), not the sound; no audible effect, spectral change 0.2 dB
- slab-city m2 Glide: **dead zone**, Glide: rise/fall 0.1 to 100 ms on a log scale, so nothing moves until macro 0.8; waveform differs -2 dB, only at note changes
- rope-walk m1 Filter: **dead zone**, cutoff: +4 dB top in the first half, zero change from 0.57 up
- rope-walk m3 Pump: **bad end**, kick-duck ratio: peak +0.1 dBFS at macro 0 (ratio 1, no duck) and a CLICK in the macro-1 render at 8.9 s; 5.9 dB jump in the first step, flat from 0.3
- lamp-oil m3 Filter: **dead zone**, cutoff: +4.6 dB near 1.8 kHz in the first third, zero change from 0.57 up
- scrap-metal m1 Vowel: **weak**, vowel 0 to 4 changes the long-term spectrum by only 0.7 dB (the formant sits on a quiet stab); the steps are uneven (0.1 to 0.7 dB)
- scrap-metal m3 Pump: **dead zone**, kick-duck ratio: 5.7 dB jump in the first step, flat from 0.3 up
- dune-buggy m2 Filter: **dead zone**, cutoff: +4.4 dB near 2.2 kHz in the first third, zero change from 0.71 up
- loading-dock m1 Sweep: **bad end**, cutoff 170 Hz to 15 kHz: even steps (1.4 to 3.6 dB), but the peak passes 0 dBFS from macro 0.57 (+1.4 dBFS) and is -0.1 dBFS at 1; top +20 dB, centroid 127 Hz to 2.2 kHz
- loading-dock m2 Drive: **weak**, drive 20 to 36 dB plus crusher 0 to 60% changes the render by 0.75 dB, nothing above 0.7; the waveform moves (-9 dB)
- loading-dock m3 Wash: **jump**, riser, delay and reverb: the first step is a 5 dB jump (mono to stereo, the riser opens), then 0.4 dB per step
- loading-dock m4 Pump: **bad end**, kick-duck ratio: peak -0.6 dBFS at macro 0 (ratio 1, no duck), 6.5 dB jump in the first step, flat from 0.3 up

## Tables

### copper-mile

| macro | routes: value at macro 0 / 0.5 / 1 | effect (change vs macro 0: at 0.5 / at 1; waveform 0 vs 1) | measured flags | verdict |
|---|---|---|---|---|
| m1 Filter | filter.ladder#28.cutoff_hz 1400 / 1.112e+04 / 2e+04 Hz (range 20-2e+04, exponential; base 1400, macro x 0.60 of travel) | +2.4 dB near 4490 Hz. 0.70 / 0.67 dB; waveform -18.4 dB | weak | **weak**: cutoff 1.4k to 20k moves only +2.4 dB near 4.5 kHz, and nothing above macro 0.6 (the filter is already open) |
| m2 Phase | phaser#30.mix 0 / 45 / 90 % (range 0-100, linear; base 0, macro x 0.90 of travel) | no change in level, spectrum or density; the waveform differs by -9.3 dB (pitch path, timing or phase). 1.59 / 1.52 dB; waveform -9.3 dB | - | **ok**: phase offset: levels and spectrum stay, the waveform moves (-9 dB); subtle, listen |
| m3 Pump | compressor#32.ratio 1 / 10.02 / 19.05 :1 (range 1-20, linear; base 1, macro x 0.95 of travel) | quieter by 2.4 dB; darker (top -6.7 dB, low -2.2 dB); goes from stereo to mono; -7.9 dB near 4490 Hz. 6.74 / 7.00 dB; waveform -5.1 dB | dead, jump | **dead zone**: kick-duck ratio 1 to 19: the first step (1/7) does 80% of the change, 0.3 to 1 is flat; also a jump of 5.7 dB at the bottom |
| m4 Room | delay#41.mix 0 / 10 / 20 % (range 0-100, linear; base 0, macro x 0.20 of travel)<br>reverb#42.mix 0 / 7.5 / 15 % (range 0-100, linear; base 0, macro x 0.15 of travel) | goes from mono to stereo. 0.60 / 1.27 dB; waveform -22.1 dB | - | **ok**: delay and reverb mix up to 20%/15%, mono to stereo, steady |

### brick-dust

| macro | routes: value at macro 0 / 0.5 / 1 | effect (change vs macro 0: at 0.5 / at 1; waveform 0 vs 1) | measured flags | verdict |
|---|---|---|---|---|
| m1 Filter | filter.ladder#21.cutoff_hz 700 / 5560 / 2e+04 Hz (range 20-2e+04, exponential; base 700, macro x 0.60 of travel) | +2.5 dB near 1782 Hz. 0.59 / 0.64 dB; waveform -17.2 dB | weak | **weak**: +2.5 dB near 1.8 kHz over the whole travel, flat above 0.7 |
| m2 Dirt | drive#20.drive_db 20 / 28.1 / 36 dB (range 0-36, linear; base 20, macro x 0.45 of travel) | wider (+3.1 dB side). 1.37 / 2.42 dB; waveform -19.0 dB | duplicate; overlaps Room (m4): same change, cosine 0.96 | **weak**: drive 20 to 36 dB changes the stereo side level (+3 dB) but not level or brightness; the drive is already saturated at 20 dB. Overlap with Room is by the measure only |
| m3 Pump | compressor#24.ratio 1 / 10.02 / 19.05 :1 (range 1-20, linear; base 1, macro x 0.95 of travel) | quieter by 2.0 dB; narrower (-10.4 dB side); -7.6 dB near 1782 Hz. 8.04 / 8.35 dB; waveform -6.0 dB | dead, jump | **dead zone**: kick-duck ratio: 6.8 dB jump in the first step, flat from 0.3 up |
| m4 Room | delay#33.mix 0 / 7.5 / 15 % (range 0-100, linear; base 0, macro x 0.15 of travel)<br>reverb#34.mix 0 / 5 / 10 % (range 0-100, linear; base 0, macro x 0.10 of travel) | goes from mono to stereo. 0.22 / 0.49 dB; waveform -26.6 dB | weak, duplicate; overlaps Dirt (m2): same change, cosine 0.96 | **weak**: mix up to 15%/10% moves the render by only 0.5 dB |

### ore-cart

| macro | routes: value at macro 0 / 0.5 / 1 | effect (change vs macro 0: at 0.5 / at 1; waveform 0 vs 1) | measured flags | verdict |
|---|---|---|---|---|
| m1 Filter | filter.ladder#28.cutoff_hz 500 / 3342 / 2e+04 Hz (range 20-2e+04, exponential; base 500, macro x 0.55 of travel) | no change you can name from the measures. 0.57 / 0.58 dB; waveform -18.3 dB | weak | **weak**: 0.6 dB, nothing you can name from the measures; flat above 0.6 |
| m2 Rumble | mixer#26.level1 0.4 / 0.58 / 0.76  (range 0-1, linear; base 0.4, macro x 0.36 of travel) | goes from mono to stereo. 0.88 / 1.57 dB; waveform -26.1 dB | duplicate; overlaps Space (m4): same change, cosine 1.00 | **weak**: level of the rumble layer 0.40 to 0.76 changes the render by 1.6 dB and only as stereo width, no low-end change; same signature as Space |
| m3 Pump | compressor#31.ratio 1 / 10.02 / 19.05 :1 (range 1-20, linear; base 1, macro x 0.95 of travel) | quieter by 2.6 dB; darker (top -5.5 dB, low -2.3 dB); goes from stereo to mono; -10.2 dB near 1782 Hz. 8.18 / 8.55 dB; waveform -4.8 dB | dead, jump | **dead zone**: kick-duck ratio: 6.8 dB jump in the first step, flat from 0.3 up |
| m4 Space | delay#40.mix 0 / 15 / 30 % (range 0-100, linear; base 0, macro x 0.30 of travel)<br>reverb#41.mix 0 / 10 / 20 % (range 0-100, linear; base 0, macro x 0.20 of travel) | goes from mono to stereo. 0.85 / 1.95 dB; waveform -20.9 dB | duplicate; overlaps Rumble (m2): same change, cosine 1.00 | **ok**: delay 30% and reverb 20%, mono to stereo |

### stoop-sale

| macro | routes: value at macro 0 / 0.5 / 1 | effect (change vs macro 0: at 0.5 / at 1; waveform 0 vs 1) | measured flags | verdict |
|---|---|---|---|---|
| m1 Swing | clock#11.swing 38 / 63 / 88 % (range 0-100, linear; base 38, macro x 0.50 of travel) | no change in level, spectrum or density; the waveform differs by -13.6 dB (pitch path, timing or phase). 0.29 / 0.58 dB; waveform -13.6 dB | - | **ok**: swing 38 to 88% moves the timing only (no level or spectrum change); 88% is an extreme shuffle, listen to the top |
| m2 Dust | mixer#26.level4 0 / 0.15 / 0.3  (range 0-1, linear; base 0, macro x 0.30 of travel) | +7.3 dB near 1782 Hz. 5.86 / 7.71 dB; waveform -17.7 dB | - | **ok**: dust layer: +7.3 dB near 1.8 kHz, even steps |
| m3 Filter | filter.ladder#29.cutoff_hz 1800 / 1.012e+04 / 2e+04 Hz (range 20-2e+04, exponential; base 1800, macro x 0.50 of travel) | no change you can name from the measures. 0.25 / 0.39 dB; waveform -21.0 dB | weak | **weak**: 0.4 dB, flat in the top 30% |
| m4 Space | delay#41.mix 0 / 15 / 30 % (range 0-100, linear; base 0, macro x 0.30 of travel)<br>reverb#42.mix 0 / 10 / 20 % (range 0-100, linear; base 0, macro x 0.20 of travel) | goes from mono to stereo. 1.16 / 2.30 dB; waveform -11.4 dB | - | **ok**: space, mono to stereo, even steps |

### fault-line

| macro | routes: value at macro 0 / 0.5 / 1 | effect (change vs macro 0: at 0.5 / at 1; waveform 0 vs 1) | measured flags | verdict |
|---|---|---|---|---|
| m1 Reese | drive#20.drive_db 8 / 17 / 26 dB (range 0-36, linear; base 8, macro x 0.50 of travel) | louder by 3.0 dB; low end +3.2 dB; wider (+4.8 dB side); +3.5 dB near 88 Hz; sparser (5.1 to 2.3 onsets/s). 4.69 / 7.15 dB; waveform -2.7 dB | - | **ok**: Reese layer: +3 dB loud, +3 dB low end, wider; busier-looking onsets 5.1 to 2.3 per second (the layer masks the hats), listen |
| m2 Filter | filter.ladder#21.cutoff_hz 900 / 6015 / 2e+04 Hz (range 20-2e+04, exponential; base 900, macro x 0.55 of travel) | +10.2 dB near 1414 Hz. 2.14 / 2.15 dB; waveform -8.1 dB | zipper; click while moving at 5.8 s | **zipper**: click while the cutoff moves at 5.8 s; steps flat above 0.6 (cutoff already beyond the content) |
| m3 Hats | mixer#31.level4 0 / 0.2 / 0.4  (range 0-1, linear; base 0, macro x 0.40 of travel) | no change you can name from the measures. 0.01 / 0.04 dB; waveform -30.4 dB | dead | **dead**: hat level 0 to 0.4 changes the render by 0.04 dB: not audible |
| m4 Space | delay#33.mix 0 / 12.5 / 25 % (range 0-100, linear; base 0, macro x 0.25 of travel)<br>reverb#34.mix 0 / 10 / 20 % (range 0-100, linear; base 0, macro x 0.20 of travel) | goes from mono to stereo. 0.85 / 1.61 dB; waveform -9.3 dB | - | **ok**: space, mono to stereo |

### corner-store

| macro | routes: value at macro 0 / 0.5 / 1 | effect (change vs macro 0: at 0.5 / at 1; waveform 0 vs 1) | measured flags | verdict |
|---|---|---|---|---|
| m1 Swing | clock#11.swing 40 / 65 / 90 % (range 0-100, linear; base 40, macro x 0.50 of travel) | no change in level, spectrum or density; the waveform differs by 2.4 dB (pitch path, timing or phase). 3.18 / 4.31 dB; waveform 2.4 dB | - | **ok**: swing 40 to 90% moves the timing (waveform +2 dB), levels and spectrum stay; 90% is extreme |
| m2 Filter | filter.ladder#29.cutoff_hz 1600 / 8997 / 2e+04 Hz (range 20-2e+04, exponential; base 1600, macro x 0.50 of travel) | +8.7 dB near 2245 Hz. 1.71 / 1.76 dB; waveform -8.0 dB | dead, bad end, zipper; CLICK in the end renders at 7.6 s; click while moving at 3.7 s | **bad end**: CLICK in the macro-1 render at 7.6 s and one while moving at 3.7 s; steps flat above 0.6 |
| m3 Drive | drive#28.drive_db 4 / 11.2 / 18.4 dB (range 0-36, linear; base 4, macro x 0.40 of travel) | louder by 2.7 dB; wider (+3.8 dB side); +3.1 dB near 1782 Hz; sparser (5.5 to 3.7 onsets/s). 2.82 / 5.54 dB; waveform -4.3 dB | - | **ok**: drive: +2.7 dB louder, +3 dB near 1.8 kHz, wider; even steps |
| m4 Room | delay#41.mix 0 / 7.5 / 15 % (range 0-100, linear; base 0, macro x 0.15 of travel)<br>reverb#42.mix 0 / 6 / 12 % (range 0-100, linear; base 0, macro x 0.12 of travel) | goes from mono to stereo. 0.60 / 1.26 dB; waveform -14.4 dB | - | **ok**: room, mono to stereo |

### glass-coast

| macro | routes: value at macro 0 / 0.5 / 1 | effect (change vs macro 0: at 0.5 / at 1; waveform 0 vs 1) | measured flags | verdict |
|---|---|---|---|---|
| m1 Filter | filter.ladder#29.cutoff_hz 2500 / 1.671e+04 / 2e+04 Hz (range 20-2e+04, exponential; base 2500, macro x 0.55 of travel) | brighter (top +3.6 dB, low +0.0 dB); +7.7 dB near 3564 Hz. 1.62 / 1.65 dB; waveform -7.7 dB | dead | **dead zone**: cutoff 2.5k to 20k: +3.6 dB top over the first half, zero change from 0.57 up |
| m2 Hats | mixer#40.level4 0 / 0.5 / 1  (range 0-1, linear; base 0, macro x 1.00 of travel) | +2.3 dB near 11314 Hz. 0.12 / 0.40 dB; waveform -25.5 dB | weak | **weak**: hats level: 0.4 dB, +2 dB near 11 kHz |
| m3 Chorus | chorus#31.mix 0 / 45 / 90 % (range 0-100, linear; base 0, macro x 0.90 of travel) | no change in level, spectrum or density; the waveform differs by 2.0 dB (pitch path, timing or phase). 1.70 / 2.21 dB; waveform 2.0 dB | - | **ok**: chorus: levels stay, the waveform moves (+2 dB) |
| m4 Space | delay#42.mix 0 / 27.5 / 55 % (range 0-100, linear; base 0, macro x 0.55 of travel)<br>reverb#43.mix 0 / 21 / 42 % (range 0-100, linear; base 0, macro x 0.42 of travel) | goes from mono to stereo. 1.13 / 2.07 dB; waveform -0.5 dB | - | **ok**: space, mono to stereo |

### still-water

| macro | routes: value at macro 0 / 0.5 / 1 | effect (change vs macro 0: at 0.5 / at 1; waveform 0 vs 1) | measured flags | verdict |
|---|---|---|---|---|
| m1 Bloom | filter.ladder#28.cutoff_hz 1800 / 1.012e+04 / 2e+04 Hz (range 20-2e+04, exponential; base 1800, macro x 0.50 of travel) | +8.9 dB near 2245 Hz. 1.76 / 1.90 dB; waveform -2.9 dB | dead | **dead zone**: Bloom: +9 dB near 2.2 kHz in the first half, zero change from 0.71 up |
| m2 Pulse | mixer#38.level4 0 / 0.4 / 0.8  (range 0-1, linear; base 0, macro x 0.80 of travel) | brighter (top +14.2 dB, low -0.0 dB); +34.2 dB near 11314 Hz. 4.57 / 6.23 dB; waveform -16.7 dB | jump | **ok**: Pulse: top +14 dB (11 kHz +34 dB of a quiet band), first step is 2.4 dB then 0.4 to 1.1; strong, listen for hiss at 1 |
| m3 Weight | mixer#26.level1 0.4 / 0.61 / 0.82  (range 0-1, linear; base 0.4, macro x 0.42 of travel) | darker (top +0.1 dB, low +4.3 dB); +4.2 dB near 70 Hz. 0.90 / 1.64 dB; waveform -8.2 dB | - | **ok**: Weight: low end +4 dB near 70 Hz, even steps |
| m4 Space | delay#40.mix 0 / 25 / 50 % (range 0-100, linear; base 0, macro x 0.50 of travel)<br>reverb#41.mix 0 / 22.5 / 45 % (range 0-100, linear; base 0, macro x 0.45 of travel) | goes from mono to stereo; +9.5 dB near 354 Hz. 1.53 / 2.82 dB; waveform 1.2 dB | - | **ok**: space, mono to stereo, +9 dB near 350 Hz |

### slab-city

| macro | routes: value at macro 0 / 0.5 / 1 | effect (change vs macro 0: at 0.5 / at 1; waveform 0 vs 1) | measured flags | verdict |
|---|---|---|---|---|
| m1 Tone | filter.ladder#23.cutoff_hz 1000 / 5623 / 2e+04 Hz (range 20-2e+04, exponential; base 1000, macro x 0.50 of travel) | no change you can name from the measures. 0.20 / 0.23 dB; waveform -16.1 dB | weak | **dead**: Tone drives the 808's ladder only: 1 kHz to 20 kHz on a sine changes its phase (waveform -16 dB), not the sound; no audible effect, spectral change 0.2 dB |
| m2 Glide | slew#20.rise_ms 0.1 / 3.162 / 100 ms (range 0.1-1e+04, exponential; base 0.1, macro x 0.60 of travel)<br>slew#20.fall_ms 0.1 / 3.162 / 100 ms (range 0.1-1e+04, exponential; base 0.1, macro x 0.60 of travel) | no change in level, spectrum or density; the waveform differs by -2.2 dB (pitch path, timing or phase). 0.03 / 0.54 dB; waveform -2.2 dB | dead | **dead zone**: Glide: rise/fall 0.1 to 100 ms on a log scale, so nothing moves until macro 0.8; waveform differs -2 dB, only at note changes |
| m3 Hats | mixer#33.level4 0 / 0.5 / 1  (range 0-1, linear; base 0, macro x 1.00 of travel) | brighter (top +9.1 dB, low -0.0 dB); +16.8 dB near 11314 Hz. 2.04 / 3.53 dB; waveform -19.5 dB | - | **ok**: hats: +9 dB top, even steps |
| m4 Room | delay#35.mix 0 / 7.5 / 15 % (range 0-100, linear; base 0, macro x 0.15 of travel)<br>reverb#36.mix 0 / 7.5 / 15 % (range 0-100, linear; base 0, macro x 0.15 of travel) | goes from mono to stereo. 1.04 / 2.00 dB; waveform -10.4 dB | - | **ok**: room, mono to stereo |

### rope-walk

| macro | routes: value at macro 0 / 0.5 / 1 | effect (change vs macro 0: at 0.5 / at 1; waveform 0 vs 1) | measured flags | verdict |
|---|---|---|---|---|
| m1 Filter | filter.ladder#29.cutoff_hz 1500 / 1.191e+04 / 2e+04 Hz (range 20-2e+04, exponential; base 1500, macro x 0.60 of travel) | brighter (top +4.0 dB, low +0.0 dB); +5.3 dB near 5657 Hz. 1.59 / 1.63 dB; waveform -13.4 dB | dead | **dead zone**: cutoff: +4 dB top in the first half, zero change from 0.57 up |
| m2 Phase | phaser#31.mix 0 / 45 / 90 % (range 0-100, linear; base 0, macro x 0.90 of travel) | no change in level, spectrum or density; the waveform differs by -3.5 dB (pitch path, timing or phase). 1.69 / 1.90 dB; waveform -3.5 dB | - | **ok**: phase offset: levels stay, waveform moves (-3.5 dB) |
| m3 Pump | compressor#33.ratio 1 / 10.02 / 19.05 :1 (range 1-20, linear; base 1, macro x 0.95 of travel) | quieter by 3.7 dB; top -5.9 dB; narrower (-8.4 dB side); -6.4 dB near 7127 Hz; sparser (6.5 to 3.6 onsets/s). 7.08 / 7.34 dB; waveform -4.5 dB | dead, jump, bad end; peak 0.1 dBFS at 0; CLICK in the end renders at 8.9 s | **bad end**: kick-duck ratio: peak +0.1 dBFS at macro 0 (ratio 1, no duck) and a CLICK in the macro-1 render at 8.9 s; 5.9 dB jump in the first step, flat from 0.3 |
| m4 Room | delay#42.mix 0 / 10 / 20 % (range 0-100, linear; base 0, macro x 0.20 of travel)<br>reverb#43.mix 0 / 7.5 / 15 % (range 0-100, linear; base 0, macro x 0.15 of travel) | goes from mono to stereo. 0.54 / 1.26 dB; waveform -17.5 dB | - | **ok**: room, mono to stereo |

### lamp-oil

| macro | routes: value at macro 0 / 0.5 / 1 | effect (change vs macro 0: at 0.5 / at 1; waveform 0 vs 1) | measured flags | verdict |
|---|---|---|---|---|
| m1 Chord | mixer#27.level2 0.3 / 0.65 / 1  (range 0-1, linear; base 0.3, macro x 0.70 of travel) | mid +3.3 dB; wider (+3.4 dB side); +6.3 dB near 707 Hz. 1.61 / 2.63 dB; waveform -8.8 dB | duplicate; overlaps Dub (m2): same change, cosine 0.98 | **ok**: chord layer 0.3 to 1.0: +3 dB mid, wider. Same measured change as Dub (by the measure only, they are different layers) |
| m2 Dub | delay#28.mix 0 / 35 / 70 % (range 0-100, linear; base 0, macro x 0.70 of travel) | +2.8 dB near 707 Hz. 2.57 / 3.42 dB; waveform -7.0 dB | duplicate; overlaps Chord (m1): same change, cosine 0.98 | **ok**: delay mix up to 70%: +3 dB near 700 Hz. Same measured change as Chord (by the measure only) |
| m3 Filter | filter.ladder#30.cutoff_hz 1400 / 1.112e+04 / 2e+04 Hz (range 20-2e+04, exponential; base 1400, macro x 0.60 of travel) | +4.6 dB near 1782 Hz. 0.77 / 0.75 dB; waveform -13.3 dB | dead | **dead zone**: cutoff: +4.6 dB near 1.8 kHz in the first third, zero change from 0.57 up |
| m4 Space | delay#42.mix 0 / 22.5 / 45 % (range 0-100, linear; base 0, macro x 0.45 of travel)<br>reverb#43.mix 0 / 16 / 32 % (range 0-100, linear; base 0, macro x 0.32 of travel) | goes from mono to stereo. 1.66 / 3.18 dB; waveform -4.1 dB | - | **ok**: space, mono to stereo |

### scrap-metal

| macro | routes: value at macro 0 / 0.5 / 1 | effect (change vs macro 0: at 0.5 / at 1; waveform 0 vs 1) | measured flags | verdict |
|---|---|---|---|---|
| m1 Vowel | filter.formant#26.vowel 0 / 2 / 4  (range 0-4, linear; base 0, macro x 1.00 of travel) | no change you can name from the measures. 0.67 / 0.69 dB; waveform -17.9 dB | weak | **weak**: vowel 0 to 4 changes the long-term spectrum by only 0.7 dB (the formant sits on a quiet stab); the steps are uneven (0.1 to 0.7 dB) |
| m2 Crush | bitcrusher#32.mix 0 / 40 / 80 % (range 0-100, linear; base 0, macro x 0.80 of travel) | no change you can name from the measures. 0.93 / 2.08 dB; waveform -19.1 dB | - | **ok**: crush mix up to 80%: 2 dB change, no level or brightness shift in the measures; listen |
| m3 Pump | compressor#34.ratio 1 / 10.02 / 19.05 :1 (range 1-20, linear; base 1, macro x 0.95 of travel) | quieter by 2.7 dB; narrower (-8.3 dB side); -5.0 dB near 707 Hz; sparser (5.9 to 4.3 onsets/s). 6.84 / 7.15 dB; waveform -5.2 dB | dead, jump | **dead zone**: kick-duck ratio: 5.7 dB jump in the first step, flat from 0.3 up |
| m4 Room | delay#43.mix 0 / 15 / 30 % (range 0-100, linear; base 0, macro x 0.30 of travel)<br>reverb#44.mix 0 / 10 / 20 % (range 0-100, linear; base 0, macro x 0.20 of travel) | goes from mono to stereo. 0.74 / 1.65 dB; waveform -16.2 dB | - | **ok**: room, mono to stereo |

### dune-buggy

| macro | routes: value at macro 0 / 0.5 / 1 | effect (change vs macro 0: at 0.5 / at 1; waveform 0 vs 1) | measured flags | verdict |
|---|---|---|---|---|
| m1 Swing | clock#11.swing 33 / 58 / 83 % (range 0-100, linear; base 33, macro x 0.50 of travel) | no change in level, spectrum or density; the waveform differs by -3.7 dB (pitch path, timing or phase). 0.99 / 1.40 dB; waveform -3.7 dB | - | **ok**: swing moves the timing only (waveform -3.7 dB), even steps |
| m2 Filter | filter.ladder#29.cutoff_hz 1700 / 1.136e+04 / 2e+04 Hz (range 20-2e+04, exponential; base 1700, macro x 0.55 of travel) | +4.4 dB near 2245 Hz. 0.75 / 0.74 dB; waveform -12.2 dB | dead | **dead zone**: cutoff: +4.4 dB near 2.2 kHz in the first third, zero change from 0.71 up |
| m3 Chop | seq#19.gate_len 10.4 / 50 / 89.6 % (range 1-100, linear; base 10.4, macro x 0.80 of travel) | +4.0 dB near 1414 Hz. 2.90 / 5.49 dB; waveform -8.6 dB | - | **ok**: chop: +4 dB near 1.4 kHz, even steps |
| m4 Space | delay#41.mix 0 / 15 / 30 % (range 0-100, linear; base 0, macro x 0.30 of travel)<br>reverb#42.mix 0 / 10 / 20 % (range 0-100, linear; base 0, macro x 0.20 of travel) | goes from mono to stereo. 1.90 / 3.52 dB; waveform -9.2 dB | - | **ok**: space, mono to stereo |

### loading-dock

| macro | routes: value at macro 0 / 0.5 / 1 | effect (change vs macro 0: at 0.5 / at 1; waveform 0 vs 1) | measured flags | verdict |
|---|---|---|---|---|
| m1 Sweep | filter.ladder#31.cutoff_hz 170 / 1605 / 1.515e+04 Hz (range 20-2e+04, exponential; base 170, macro x 0.65 of travel) | brighter (top +20.4 dB, low -1.6 dB); +20.7 dB near 11314 Hz; -2.6 dB near 140 Hz; centroid 127 to 2235 Hz. 5.67 / 10.32 dB; waveform 0.4 dB | bad end; peak -0.1 dBFS at 1 | **bad end**: cutoff 170 Hz to 15 kHz: even steps (1.4 to 3.6 dB), but the peak passes 0 dBFS from macro 0.57 (+1.4 dBFS) and is -0.1 dBFS at 1; top +20 dB, centroid 127 Hz to 2.2 kHz |
| m2 Drive | drive#30.drive_db 20 / 30.8 / 36 dB (range 0-36, linear; base 20, macro x 0.60 of travel)<br>bitcrusher#33.mix 0 / 30 / 60 % (range 0-100, linear; base 0, macro x 0.60 of travel) | no change in level, spectrum or density; the waveform differs by -9.4 dB (pitch path, timing or phase). 0.66 / 0.75 dB; waveform -9.4 dB | dead | **weak**: drive 20 to 36 dB plus crusher 0 to 60% changes the render by 0.75 dB, nothing above 0.7; the waveform moves (-9 dB) |
| m3 Wash | delay#35.mix 0 / 15 / 30 % (range 0-100, linear; base 0, macro x 0.30 of travel)<br>reverb#36.mix 0 / 15 / 30 % (range 0-100, linear; base 0, macro x 0.30 of travel)<br>vca#18.gain 0 / 0.45 / 0.9  (range 0-1, linear; base 0, macro x 0.90 of travel) | goes from mono to stereo. 5.68 / 6.58 dB; waveform -5.6 dB | jump | **jump**: riser, delay and reverb: the first step is a 5 dB jump (mono to stereo, the riser opens), then 0.4 dB per step |
| m4 Pump | compressor#27.ratio 1 / 10.02 / 19.05 :1 (range 1-20, linear; base 1, macro x 0.95 of travel) | quieter by 6.5 dB; brighter (top -0.1 dB, low -6.2 dB); mid -7.8 dB; -8.1 dB near 223 Hz. 7.77 / 8.09 dB; waveform -3.3 dB | dead, jump, bad end; peak -0.6 dBFS at 0 | **bad end**: kick-duck ratio: peak -0.6 dBFS at macro 0 (ratio 1, no duck), 6.5 dB jump in the first step, flat from 0.3 up |

## Clips

Every clip: the groove runs, the macro moves 0 to 1 to 0 over 8 s (1 s to 9 s of a 10 s render), the other macros at their stored values. Start times are in seconds from the start of the file.

- `docs/presets/clips/macro-audit/groove/tour.mp3`: all 56 sweeps, 1 s apart, 20 ms fades:
  0.0 copper-mile Filter; 11.0 copper-mile Phase; 22.0 copper-mile Pump; 33.0 copper-mile Room; 44.0 brick-dust Filter; 55.0 brick-dust Dirt; 66.0 brick-dust Pump; 77.0 brick-dust Room; 88.0 ore-cart Filter; 99.0 ore-cart Rumble; 110.0 ore-cart Pump; 121.0 ore-cart Space; 132.0 stoop-sale Swing; 143.0 stoop-sale Dust; 154.0 stoop-sale Filter; 165.0 stoop-sale Space; 176.0 fault-line Reese; 187.0 fault-line Filter; 198.0 fault-line Hats; 209.0 fault-line Space; 220.0 corner-store Swing; 231.0 corner-store Filter; 242.0 corner-store Drive; 253.0 corner-store Room; 264.0 glass-coast Filter; 275.0 glass-coast Hats; 286.0 glass-coast Chorus; 297.0 glass-coast Space; 308.0 still-water Bloom; 319.0 still-water Pulse; 330.0 still-water Weight; 341.0 still-water Space; 352.0 slab-city Tone; 363.0 slab-city Glide; 374.0 slab-city Hats; 385.0 slab-city Room; 396.0 rope-walk Filter; 407.0 rope-walk Phase; 418.0 rope-walk Pump; 429.0 rope-walk Room; 440.0 lamp-oil Chord; 451.0 lamp-oil Dub; 462.0 lamp-oil Filter; 473.0 lamp-oil Space; 484.0 scrap-metal Vowel; 495.0 scrap-metal Crush; 506.0 scrap-metal Pump; 517.0 scrap-metal Room; 528.0 dune-buggy Swing; 539.0 dune-buggy Filter; 550.0 dune-buggy Chop; 561.0 dune-buggy Space; 572.0 loading-dock Sweep; 583.0 loading-dock Drive; 594.0 loading-dock Wash; 605.0 loading-dock Pump
- `docs/presets/clips/macro-audit/groove/copper-mile-tour.mp3` (this preset's macros): 0.0 Filter; 11.0 Phase; 22.0 Pump; 33.0 Room
- `docs/presets/clips/macro-audit/groove/brick-dust-tour.mp3` (this preset's macros): 0.0 Filter; 11.0 Dirt; 22.0 Pump; 33.0 Room
- `docs/presets/clips/macro-audit/groove/ore-cart-tour.mp3` (this preset's macros): 0.0 Filter; 11.0 Rumble; 22.0 Pump; 33.0 Space
- `docs/presets/clips/macro-audit/groove/stoop-sale-tour.mp3` (this preset's macros): 0.0 Swing; 11.0 Dust; 22.0 Filter; 33.0 Space
- `docs/presets/clips/macro-audit/groove/fault-line-tour.mp3` (this preset's macros): 0.0 Reese; 11.0 Filter; 22.0 Hats; 33.0 Space
- `docs/presets/clips/macro-audit/groove/corner-store-tour.mp3` (this preset's macros): 0.0 Swing; 11.0 Filter; 22.0 Drive; 33.0 Room
- `docs/presets/clips/macro-audit/groove/glass-coast-tour.mp3` (this preset's macros): 0.0 Filter; 11.0 Hats; 22.0 Chorus; 33.0 Space
- `docs/presets/clips/macro-audit/groove/still-water-tour.mp3` (this preset's macros): 0.0 Bloom; 11.0 Pulse; 22.0 Weight; 33.0 Space
- `docs/presets/clips/macro-audit/groove/slab-city-tour.mp3` (this preset's macros): 0.0 Tone; 11.0 Glide; 22.0 Hats; 33.0 Room
- `docs/presets/clips/macro-audit/groove/rope-walk-tour.mp3` (this preset's macros): 0.0 Filter; 11.0 Phase; 22.0 Pump; 33.0 Room
- `docs/presets/clips/macro-audit/groove/lamp-oil-tour.mp3` (this preset's macros): 0.0 Chord; 11.0 Dub; 22.0 Filter; 33.0 Space
- `docs/presets/clips/macro-audit/groove/scrap-metal-tour.mp3` (this preset's macros): 0.0 Vowel; 11.0 Crush; 22.0 Pump; 33.0 Room
- `docs/presets/clips/macro-audit/groove/dune-buggy-tour.mp3` (this preset's macros): 0.0 Swing; 11.0 Filter; 22.0 Chop; 33.0 Space
- `docs/presets/clips/macro-audit/groove/loading-dock-tour.mp3` (this preset's macros): 0.0 Sweep; 11.0 Drive; 22.0 Wash; 33.0 Pump
- one file per macro: `docs/presets/clips/macro-audit/groove/<slug>-m<k>-<name>.mp3`
