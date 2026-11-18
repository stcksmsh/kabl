# Macro redesign: pad presets (S3-pad)

All 13 pad presets of `tools/examples/presets_pad.rs` redesigned with the method of
`MACRO-REDESIGN.md` and `MACRO-REDESIGN-BASS-LEAD.md`. Every preset has 4 macros. The stored sound is
unchanged unless listed under "Default changes". Tours (new macros only, 20 s each) are in
`docs/presets/clips/macros-pad/` with `tours.txt` (git-ignored, sent to Kosta). Listen-first: Tin Voice,
Drive-In, Rooftop.

## Rules applied after Kosta's verdict

- **Level**: a macro that is not about level keeps the loudness within 3 dB over its travel (audit `level`
  flag, `macro_audit.py --level-type` for the deliberate ones). Compensation is `comp`: a slope on the
  voice's gain stage that leaves the stored sound as it was. Tin Voice Shift has no gain stage, so it
  rides the compressor makeup.
- **Bolder** means bigger tonal and space moves, not more pitch wobble (Kosta: deep pitch modulation on
  saw stacks reads as creepy, warm pads stay warm). Chorus macros cap at depth 65 to 80 and rate 0.6 to
  0.9 Hz on every pad; Drive-In Sway is back at its old pitch depth with a slow filter breath beside it.
- No limiter anywhere (gain staging only). Peaks are at -1.2 dBFS or lower at every step and in every
  combination (highest: Rooftop Chorus, -1.0 at a step; combinations -1.7 or lower).

## Level-type macros (may move the loudness on purpose)

Fog Bank Arrival, Drive-In Pulse (gate), Glacier Swell, Concrete Cloud Squash (a compressor: the density
falls as it clamps). Rooftop Swell and Pump House Pump move level 2 to 3 dB, inside the limit.

## Per preset

Positions are the stored values. "0 -> 1" is what you hear.

### Fog Bank
| macro | stored | 0 -> 1 | moves |
|---|---|---|---|
| Arrival | .77 | each chord is struck quickly -> a slow bloom of 4 s or more (level macro) | amp and filter attack and release |
| Haze | .61 | close and dark -> bright, hazy, wide | filter cutoff, chorus mix |
| Drift | .08 | still -> the pitch sways slowly (pitch only; the filter walk is fixed at its old depth) | slow pitch LFOs on every oscillator |
| Space | .65 | dry mono -> echo and a long room | echo and room mix |

### Deep Field
| macro | stored | 0 -> 1 | moves |
|---|---|---|---|
| Swell | .74 | opens out of the dark quickly -> a very slow, wide opening | filter envelope attack and depth |
| Sub | .50 | nothing below -> a deep octave under | octave-down sine level, gain comp |
| Shimmer | .35 | plain -> an octave of air above with a deeper chorus | octave-up level, chorus depth |
| Space | .60 | dry -> long echo and a huge room | echo and room mix |

### Cotton Mill
| macro | stored | 0 -> 1 | moves |
|---|---|---|---|
| Motion | .25 | still pulse -> pulse width sweeps fast and wide | PWM depth and rate, gain comp, a small cutoff pull so the thinning pulse is not just "brighter" |
| Chorus | .50 | nothing -> a deep, fast wash | chorus mix, depth, rate |
| Warmth | .80 | dark and round -> bright and glassy | filter cutoff |
| Beat | .40 | two saws almost locked -> a slow beating | fine tune of the two saws (start at +-1.5 ct: at exactly 0 they cancel and the level jumped 4 dB) |

### Picture Window
| macro | stored | 0 -> 1 | moves |
|---|---|---|---|
| Spread | .53 | tight stack -> wide, beating | detune of both stacks |
| Sparkle | .40 | plain -> an airy octave above | octave-up level, gain comp |
| Chorus | .45 | dry -> deep wash | chorus mix, depth, rate |
| Air | .50 | soft and close -> open and glassy | new high-shelf EQ at flat |

### Drive-In
| macro | stored | 0 -> 1 | moves |
|---|---|---|---|
| Sway | .20 | steady -> a slow, gentle breathing: a small pitch sway plus a slow filter movement (back at the old pitch depth, no wobble) | slow pitch LFO (old depth), slow filter LFO |
| Chorus (was Wash) | .60 | plain stack -> a wide, lush chorus (width, not wobble: depth 80, rate 0.9 Hz at most) | chorus mix, depth, rate |
| Weight | .20 | thin -> the square an octave down comes forward, clearly (level to .75) | octave-down level, gain comp |
| Pulse | 0 | steady -> hard sixteenth chops, echo and room ring through the gaps (level macro) | a trance gate on the host clock; press Run |

Rename proposed to Kosta: Drive-In -> "Sunset Strip" (not done; waits for his answer).

### Dry Cleaner
| macro | stored | 0 -> 1 | moves |
|---|---|---|---|
| Sweep | .63 | muffled and resonant -> bright | cutoff, resonance |
| Phase | .50 | plain -> fast, deep, ringing phaser | phaser mix, rate, depth, feedback |
| Grit | .40 | clean -> driven hard | drive, trim, gain comp |
| Snap | .50 | a short tick -> a held, ringing chord | sustain, release, gain comp |

### Concrete Cloud
| macro | stored | 0 -> 1 | moves |
|---|---|---|---|
| Weight | .71 | light -> saturated and heavy with the octave down | drive, octave level, gain comp |
| Squash | .50 | loose hold -> flat slab (level macro) | compressor threshold, ratio, makeup, gain comp |
| Crush | .25 | clean -> coarse digital grind | bitcrusher mix, bits, downsample |
| Tail | .39 | struck and cut -> a slow swell that hangs | attack and release |

### Static Garden
| macro | stored | 0 -> 1 | moves |
|---|---|---|---|
| Static | .50 | clean -> a fizzing noise layer | noise level |
| Wander | 0 | still -> pitch drifts (the filter walk is fixed) | two pitch LFOs |
| Metal | .50 | plain -> bell-like overtones, partner detunes | overtone oscillator level and tune, gain comp |
| Space | .55 | dry -> echo and room | echo and room mix |

### Glacier
| macro | stored | 0 -> 1 | moves |
|---|---|---|---|
| Frost | .45 | dark and flat -> bright, icy | upper-octave level, gain comp |
| Width | .70 | narrow pulse -> wide | pulse width |
| Swell | .60 | quick -> a very slow bloom (level macro) | attack and release |
| Space | .70 | dry -> echo (max .30) and room | echo and room mix |

### Pump House
| macro | stored | 0 -> 1 | moves |
|---|---|---|---|
| Pump | .35 | steady -> a deep, long duck on every beat (level macro-ish, within 3 dB) | duck depth and decay, host clock |
| Weight | .44 | thin -> the octave down takes over | oscillator levels, gain comp |
| Wide | .30 | narrow -> wide chorus | chorus mix, depth, rate |
| Open | .83 | dark -> fully open (Heat dropped, drive fixed) | filter cutoff |

### Tin Voice
| macro | stored | 0 -> 1 | moves |
|---|---|---|---|
| Vowel | .35 | one vowel through a sweep of vowels | formant vowel |
| Chatter | 0 | still -> the mouth jumps to a new vowel at random, faster and wider | S&H vowel depth and rate |
| Shift | .58 | a small mouth -> a large one | formant shift, compressor makeup (level comp) |
| Grit | .46 | clean -> driven, sharp and crushed | drive, formant Q, new bitcrusher mix, bits, downsample |

### Rooftop
| macro | stored | 0 -> 1 | moves |
|---|---|---|---|
| Swell | .49 | each chord leans slowly into the filter (level macro-ish, within 3 dB) | attack, filter envelope depth |
| Section | .23 | one tight horn -> a harder, bigger section: wider detune (to 24 ct) and a bite of drive (level held by the drive trim) | oscillator detune and fine, drive and drive trim |
| Octave | 0 | brass -> a second section an octave up joins | third oscillator level, gain comp |
| Chorus | .50 | dry -> a wide, lush wash (depth 70, rate 0.8 Hz at most) | chorus mix, depth, rate |

### Tidepool
| macro | stored | 0 -> 1 | moves |
|---|---|---|---|
| Drift | .45 | still -> slow pitch drift | pitch LFOs |
| Steps | 0 | still -> random pitch steps, a burble | S&H pitch depth and rate |
| Tone | .73 | dark and plain (floor 650 Hz) -> bright with the octave sine | cutoff, octave level, gain comp |
| Space | .60 | dry -> echo and room | echo and room mix |

## Audit results

Full 13-preset audit with the `level` flag (`macro_audit.py --category Pad --level-type
fog-bank/m1,drive-in/m4,concrete-cloud/m2,glacier/m3`): no `level` flag left, no pair with similarity
over 0.8, no clicks, every peak at -1.2 dBFS or lower and every combination (pairs, all 0, all 1) at
-1.7 dBFS or lower. Loudness is -16 to -21 dB over all steps (bank middle -15 .. -22).

Remaining flags, all understood:
- `duplicate` on Deep Field Sub and Shimmer, Drive-In Weight and Pulse, Cotton Mill Motion and Warmth,
  Dry Cleaner Grit and Snap, Concrete Cloud Weight and Squash: each pair shares a parameter (the gain stage
  through `comp`, or the cutoff). The audible changes differ, and the similarity check (cosine over 0.8) passes
  for every pair.
- `jump` on Drive-In Pulse (a gate: hard chops are the point), Drive-In Weight (the octave-down entry)
  and Tidepool Tone (the dark floor of the filter: the top band is nearly silent at 0).

## Count justifications

All 13 presets have 4 macros: none has 3 or 5+.

## Default changes (state in the PR)

- Tin Voice: formant `hold` 1 (the vowel stays clear at every position), trim 14 to 26, small crush at
  the stored Grit.
- Level trims: Deep Field 11 to 14, Picture Window 10 to 11, Rooftop 9 to 11.5, Glacier 9 to 12, Tidepool
  9 to 11, Dry Cleaner 8 to 10, Static Garden 7.5 to 6.
- Cotton Mill Motion pulls the cutoff a little (stored sound moves about 3 % in cutoff).
- Cotton Mill / Picture Window / Drive-In / Dry Cleaner / Concrete Cloud / Rooftop / Glacier / Pump House
  / Static Garden lost their old Cutoff / Room / Space macros (fixed at the old stored value). Fog Bank
  Drift no longer moves the filter (walk fixed at its old depth).
- Drive-In: Wash renamed Chorus; Sway keeps its old stored pitch depth and adds a faint slow filter movement.
- Rooftop: trim 9 to 11.5; a few dB of drive at the stored Section.
