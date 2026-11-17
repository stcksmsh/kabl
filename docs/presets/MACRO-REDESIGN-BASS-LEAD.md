# Macro redesign: bass and lead presets, first half (12 of 24)

Same method as `MACRO-REDESIGN.md` (pilot): each preset gets four macros from what makes that patch interesting, four different kinds of change, clean by default, stored (default) sound unchanged. Builders are in `tools/examples/presets_bass_lead.rs` (`span`, `span_hi`, `Voice::snap`, `Voice::depth_keep`; `routed` recomputes the old stored value). Presets 13 to 24 (copper-wire .. short-circuit) keep their old macros: a second slice.

Written from the patch recipes and the audit renders. Nobody has listened yet; "what you hear" is what the parameters and measures say. The A/B tours are the real test.

Where the old space macro (Room) did not matter for the patch, the echo and room are now a fixed wet amount at the old stored mix, and the macro slot went to a gesture of the patch. Presets with a long tail as part of the sound (Acid Rain, Spun Sugar, Hot Wire, Lantern, Low Tide) keep Space.

## Presets (stored position in brackets)

### Steel Toe (French touch bass, ducked on the host clock)
| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Bite [.45] | filter opens from a dull thud to a bright bark, each note snaps harder | ladder cutoff 507 Hz to 5 kHz; filter-envelope depth .08 to .30 |
| Weight [.60] | the sub grows and the saw steps back: a thin mid bass to a sub-heavy one | mixer: square sub .15 to .73, saw .75 to .42 |
| Squash [.50] | the compressor clamps harder, flatter and denser | threshold -10 to -42 dB, makeup 6 to 20 dB |
| Pump [.30] | the beat ducks the bass, deeper and for longer | duck depth 0 to -.60, duck decay 90 ms up |

### Tar Pit (808, sine with pitch snap, slides)
| Macro | What you hear | What it moves |
|---|---|---|
| Thump [.50] | the kick at the front of each note: harder and longer | pitch-snap depth .25 to .75, snap decay 15 to 50 ms |
| Body [.50] | a short boom to a long ringing tail, the triangle fills it | amp decay 600 to ~5000 ms, release 120 ms up, triangle level 0 to .30 |
| Dirt [.35] | the sine gains harmonics and speaks on small speakers | drive 6 to 20 dB |
| Slide [.80] | glide between overlapped notes: a quick step to a long swoop | glide time 40 ms to 2.4 s |

### Acid Rain (legato acid bass)
| Macro | What you hear | What it moves |
|---|---|---|
| Squelch [.50] | the filter opens from a dark growl to a bright snarl and the resonance rises with it | cutoff 271 Hz to 4 kHz, resonance .20 to .61 |
| Accent [.60] | envelope shape: short clipped notes to long held ones | amp decay 90 to ~600 ms, sustain .25 to 1, filter-envelope decay 80 to ~600 ms |
| Slide [.40] | glide time between notes | 15 to ~380 ms |
| Space [.25] | dotted echo and room | echo mix, room mix |
(The old Drive is fixed at its old stored value.)

### Oatmeal (clean round pop bass)
| Macro | What you hear | What it moves |
|---|---|---|
| Warmth [.20] | a soft saturation adds harmonics to the clean note (harmonics, not filtering: a triangle and a sine have little to filter) | drive 3 to ~21 dB |
| Pluck [.50] | a filter snap at the front of each note and a shorter, clipped note | envelope depth 0 to .70, filter-envelope decay 60 to ~540 ms, amp decay 900 to ~140 ms |
| Throb [0.00] | a slow tremolo fades in (motion; off by default) | LFO 4.5 Hz on the output level, depth 0 to .08 of the gain range |
| Squeeze [.40] | the light compressor clamps harder | threshold -8 to -43 dB, makeup 0 to 7.5 dB |
(The old Tone stays fixed at its old stored cutoff, the old Body at its stored levels.)

### Spun Sugar (vocal pop lead)
| Macro | What you hear | What it moves |
|---|---|---|
| Tone [.80] | soft vowel to bright singing edge | cutoff 2.8 kHz to 12 kHz |
| Vibrato [.35] | a vibrato fades in and speeds up as it deepens | depth 0 to .10, LFO rate 3.5 to ~7 Hz |
| Shimmer [.40] | a lone voice to a choir | chorus mix 8 to 63 %, depth 20 to 90 %, saw-stack detune 4 to 19 ct |
| Space [.50] | echo and room | echo mix, room mix |

### Pig Iron (hard crushed lead)
| Macro | What you hear | What it moves |
|---|---|---|
| Bite [.50] | filter opens, the note snaps harder | cutoff 2.1 to 12 kHz, envelope depth .05 to .25 |
| Weld [.50] | the saw drifts against the pulse, the pulse thins | second saw fine 0 to 14 ct, pulse width 50 to 20 % |
| Crush [.50] | a plain hard lead to a coarse digital grind | crusher mix 0 to 70 %, bits 9 to 5, downsample 1.5 to ~5 |
| Length [.60] | a stab to a held blare | sustain .50 to 1, release 20 ms up |

### Hot Wire (legato acid lead)
| Macro | What you hear | What it moves |
|---|---|---|
| Squelch [.50] | closed growl to scream, the resonance rises with the cutoff | cutoff 634 Hz to 9 kHz, resonance .20 to .71 |
| Slide [.40] | glide time between overlapped notes | 8 to ~200 ms |
| Wire [.30] | the pulse oscillator gets a faster pulse-width sweep and thickens | PWM depth 0 to .35, rate .3 to ~1.4 Hz, level .15 to .65 |
| Space [.40] | dotted echo and room | echo mix, room mix |

### Lantern (ambient swell lead)
| Macro | What you hear | What it moves |
|---|---|---|
| Glow [.50] | filter opens, the octave sine comes forward | cutoff 1.8 to 12 kHz, sine level .05 to .35 |
| Swell [.40] | notes bloom slower and fade longer | attack ~320 ms to 5 s, release 500 ms up |
| Drift [.30] | a slow vibrato fades in, the chorus swims wider and slower | vibrato depth 0 to .27, rate 3.2 Hz up, chorus depth 15 up, rate .1 Hz up |
| Space [.65] | long echo and big room | echo mix, room mix |

### Rubber Band (funk pluck bass)
| Macro | What you hear | What it moves |
|---|---|---|
| Room [.20] | a short slapback and room around the pluck (space) | echo mix, room mix |
| Slap [.73] | the filter snap of each note: deeper and longer | envelope depth 0 to .45, decay 40 to ~120 ms |
| Stab [.50] | short and dry to long and ringing | amp decay 80 to 845 ms, sustain .10 to .60 |
| Body [.50] | the saw an octave down comes forward of the pulse | saw .15 to .75, pulse .60 to .30 |

### Slate Roof (neuro bass)
| Macro | What you hear | What it moves |
|---|---|---|
| Cutoff [.45] | closed rumble to bright snarl | cutoff ~395 Hz to 4 kHz |
| Growl [.44] | the filter rolls faster and deeper | LFO depth 0 to .20, rate 1.2 to ~20 Hz |
| Stack [.50] | a tight stack to a wall | stack detune 8 to 52 ct, second saw fine 2 to 14 ct |
| Room [.20] | a tight room and slapback (space) | echo mix, room mix |
(The crusher stays at the mix the old Crush was stored at, 18 %.)

### Gutter (dirty French touch bass)
| Macro | What you hear | What it moves |
|---|---|---|
| Bite [.45] | filter opens, the chug snaps harder | cutoff 430 Hz to 5 kHz, envelope depth .18 to .33 |
| Grit [.65] | the drive climbs and the crusher bites into it | drive 21.8 to 36 dB, crusher mix 0 to 15 % |
| Octave [.30] | a buzzing pulse an octave up joins | pulse level 0 to .60 |
| Squash [.50] | compressor clamps harder | threshold -12 to -44 dB, ratio 4 to 12, makeup 6 to 22 dB |

### Low Tide (ambient sub)
| Macro | What you hear | What it moves |
|---|---|---|
| Tone [.45] | filter opens, the triangle an octave down comes forward | cutoff ~540 Hz to 5 kHz, triangle .10 to .43 |
| Tide [.35] | a slow filter tide fades in and speeds up | LFO depth 0 to .30, rate .05 to ~1.2 Hz |
| Swell [.30] | notes bloom slower and fade longer | attack 65 ms to 5 s, release 600 ms up |
| Space [.60] | long echo and big room | echo mix, room mix |

## Checks (macro_audit with the combination check, #159; 8 steps per macro)

Peaks: no step, sweep or combination render is over -1 dBFS (worst case in the combination tables: Low Tide all four at 1 at -2.6 dBFS, stored -3.6). No click appears in a step, sweep, end or combination render.

Flags that are left, honestly:

| Preset | Flag | Note |
|---|---|---|
| Acid Rain | Space `jump` | The echo and room come in over a dry bass: the first step adds most of the change (unchanged space macro) |
| Oatmeal | Pluck `weak` (0.4 dB); Warmth and Squeeze `cancel` as a pair | A triangle and a sine have almost nothing for a filter snap to bite: Pluck is mainly the shorter note. Warmth makes the note louder, Squeeze quieter |
| Rubber Band | Slap `jump`, Stab `dead` | Slap and Stab move the note shape in a few steps; Stab changes the decay and sustain of notes that are short already |
| Gutter | Grit `weak` | The drive is at 22 to 36 dB: the sound is saturated and the crusher adds little. Left |
| Low Tide | Tide and Swell `jump` | Slow LFO and slow attack: the 10 s render window holds few cycles |
| Tar Pit | tour: Body at 0 has a 1.25 s silent gap | The macro itself: a 600 ms tail between sparse 808 notes. Guard run with the default 1 s fails for this clip only |

No `similar` pair is left (the matrix shows none over 0.8 in the 12). The combination table has no `click`, `peak` or `pileup` flag.

### Clicks (the old Steel Toe 6.5 s and Rubber Band end-render clicks)

Cause in the patches, not the macros: a note that retriggers while the previous note is still ringing restarts the amp (and filter) envelope from its current level with a 1 to 3 ms attack, and the level steps. The Rubber Band click at 0.25 s and 3.15 s was in the stored sound too (second difference -14 dB, isolated); an 8 ms attack on both envelopes removes it (tested with `PP_SET`). Fixed in the patches: amp attack 2 to 6 ms (Steel Toe, Rubber Band, Slate Roof), 3 to 5 ms (Pig Iron), filter-envelope attack 3 to 8 ms (Rubber Band), 4 to 8 ms (Slate Roof), 1 to 6 ms (Pig Iron). This is a change to the default sound of these four presets (a softer first few milliseconds of each note): see below. Slap on Rubber Band and Growl on Slate Roof were also narrowed (Slap to .45 of the knob at 1, Growl to .20) so their click at 1 is gone. (`PP_SET` in `presets_bass_lead` now splits the module kind at the last dot, so dotted kinds such as `env.adsr` work.)

### Level of Tar Pit

Stored sound: peak -13.8 dBFS, loudness -22.2 dB. Loudness is in line with the bank (Steel Toe -16.6, Acid Rain -20.4, Oatmeal -20.9, Spun Sugar -22.6, Pig Iron -22.5, Hot Wire -21.7, Rubber Band -21.4, Gutter -17.7, Low Tide -21.7). Only the peak is low: a sine has a low crest, so no change.

### Default sound against the old patch (16 s clip, float, `presets_bass_lead clips`)

Oatmeal and Spun Sugar are bit-identical; Tar Pit, Acid Rain, Hot Wire, Lantern, Low Tide differ by -125 dB or better (float rounding of the macro line); Gutter by -90.5 dB (the crusher's quantiser amplifies a rounding error). Changed on purpose by the click fix (attack times): Steel Toe -23 dB, Pig Iron -19 dB, Rubber Band -20 dB, Slate Roof -30 dB below the old signal (peaks about 0.2 to 2.3 dB lower), plus the Slate Roof crusher now fixed at 18 %. The other 12 presets are untouched (their patch logs were reverted: only timestamps differ).

## Tours

`docs/presets/clips/macros-bass-lead/` (git-ignored): `<slug>-tour.mp3` per preset: new macro 1 to 4 back to back, each 20 s (6 s at 0, 4 s up, 6 s at 1, 4 s down), 1 s of silence between, 84 s; `tours.txt` lists the start seconds. Rendered at 124 bpm. Tour mode loops the phrase at its own length (first to last hit, rounded up to whole bars) so the sound is continuous (before, it looped every 8 bars and most phrases left about 7 s of silence per clip). `macro_tour_ab.py` fails when a clip has a gap over 1 s under -45 dBFS after its first second (`--max-gap` to relax); with `--old-patches`/`--old-renders` it still writes old/new A/B tours. Listen first: **Steel Toe** (Weight and Squash change the pump), **Tar Pit** (Thump, Body and Slide are new gestures on a sine), **Spun Sugar** (Shimmer and Vibrato).

## Not done

Presets 13 to 24 (second slice). Acid Rain Space, Rubber Band Slap and Stab, Gutter Grit, Low Tide Tide and Swell, Oatmeal Pluck (see the table).
