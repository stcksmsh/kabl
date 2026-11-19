# Macro redesign: the pilot presets (S3-pilot)

All 8 presets of `tools/examples/presets_pilot.rs` redesigned with the method of `MACRO-REDESIGN.md`,
`MACRO-REDESIGN-PAD.md` and `MACRO-REDESIGN-BASS-LEAD.md`: each macro comes from what makes that patch
interesting, the stored sound is unchanged through `span` / `rise` unless a default change is listed below. The
builder got the same helpers the other builders have (`span`, `span_hi`, `span_lh`, `rise`, `routed`,
`depth_route`, `trim_by` for level compensation, and a `Space` choice for the echo and room: a macro, or fixed at
the old stored level). Tours (new macros only, 20 s each) are in `docs/presets/clips/macros-pilot/` with
`tours.txt` (git-ignored, sent to Kosta). Listen first: **Loading Dock**, **Salt Flat**, **Undertow**.

Every preset has 4 macros: none has 3 or 5+. Peaks: no step, sweep or combination render is over -1 dBFS and no
click appears in any step, sweep, end or combination render (`macro_audit` + `macro_audit.py --level-type
loading-dock/m4,salt-flat/m1,cinder-block/m3,glass-elevator/m2,boxwood/m2`). No limiter anywhere.

## Per preset

Positions are the stored values. "0 -> 1" is what you hear.

### Cinder Block (blunt mono bass)
| macro | stored | 0 -> 1 | moves |
|---|---|---|---|
| Bark | .52 | a dull thud opens to a bright bark, and the snap of every note digs in | ladder cutoff 150 Hz to 6 kHz; filter-envelope depth 0 to .38 |
| Hold | .50 | a short, tight thud to a held, sustained boom | amp decay 60 to ~670 ms, sustain .5 to 1, release 20 ms up |
| Clamp | .50 | a slow, punchy squeeze to a fast, flat clamp (level macro: the density grows, the trim follows part of the way) | compressor threshold -8 to -44 dB, ratio 2 to 14, attack 12 to ~.3 ms, makeup 3 to 15 dB; trim +11 dB over the travel |
| Crumble | 0 | clean, then a coarse digital break-up | bitcrusher mix 0 to 70 %, bits 10 to 6, downsample 1.5 to 5 |

The old Grit (the drive saturates already, it changed nothing you could hear) and Room are fixed at their old
stored values. The old Crush was dead (3 % at the stored position). Attacks 1 ms to 5 ms (amp) and 6 ms (filter):
the old patch clicked at the note starts.

### Parquet Loop (held chord as a filtered disco loop)
| macro | stored | 0 -> 1 | moves |
|---|---|---|---|
| Loop | .25 | the filter stands open, then the beat-locked saw closes it harder and harder once a beat | depth of the clock saw on the cutoff, 0 to 1.2 of the travel (.3 at the stored position, as before) |
| Phase | .50 | a plain chord, a slow shallow phaser, a fast deep ringing one | phaser mix 0 to 90 %, rate .15 to 1.7 Hz, depth 70 to 100 %, feedback 40 to 80 % |
| Squelch | .61 | a plain round close to a ringing, vocal filter | ladder resonance .08 to .85, trim -2 dB over the travel |
| Glide | .81 | chord changes step, then slide into each other (5 ms to 1.5 s) | MIDI glide time |

The old Cutoff (the loop hid it), Crunch (saturated) and Space are fixed at their old stored values. A first Open
macro on the cutoff undid the Loop (cosine -.91) and was replaced by the resonance. Patch gain -5.5 dB (peaks of the
extreme positions were over 0 dBFS): stored loudness -20.4 dB in the factory test.

### Loading Dock (filter-house piece with cues; built with cues, the macros are launched by them)
| macro | stored (Intro) | 0 -> 1 | moves |
|---|---|---|---|
| Sweep | .12 | the filter opens from a closed thump to wide open; the level follows so the opening never clips | ladder cutoff 170 Hz to 15 kHz; trim -12 dB and the music compressor's makeup -5 dB over the travel |
| Stabs | .61 | the off-beat stab line sits low in the mix at the stored position (-18 dB) and the macro brings it forward, smoothly: out at 0, a straight line in dB to full level at 1, and a longer ring (20 ms to ~280 ms decay) | stab gain stage -40 dB to -4 dB (a straight line in dB, no step); stab decay on an exponential line |
| Wash | 0 | dry, then a noise riser, a straight quarter-note echo and a dark room wash in | riser level, echo mix 0 to 30 %, room mix 0 to 30 % |
| Pump | .70 | the kick ducks nothing, then more and more of the loop, hard (level macro) | duck threshold 0 to -60 dB at a fixed 20:1, release 260 ms; trim +16 dB and the music compressor's makeup 7 to 13.5 dB (stored) and on, over the travel |

Cues retargeted to the new macros (Intro / Build / Gap / Drop / Break): Sweep .12 .90 .08 .78 .20; Stabs .61 .88 0 .85
.50; Wash 0 .65 .85 .10 .60; Pump .70 .85 1 1 .50. The Sweep clip above 0 dBFS is fixed: the level after the filter now
falls 12 dB (was 9) over the Sweep travel, and the compressor makeup follows it. Cue launches still move all four
macros (`presets_pilot macros` prints peak and loudness at every cue state). Tour (bass, stabs and drums, no held
chord): RMS -22.0 dB at the stored position (-21.6 at Sweep 0), peaks -8.1 dBFS at Sweep, -7.0 dBFS at Stabs and Wash
and -3.9 dBFS at worst (Pump 0, the unducked end). The all-at-1 combination is what holds the level there: +3 dB of
makeup took it over -1 dBFS.

**Kosta's verdicts on the listen-first tour and what changed:**
- "Stabs and Pump not hard enough": the stab line went through the closed filter and sat ~10 dB under the mix. It now
  bypasses the filter (own drive, joins after it); Stabs measure 10.5 dB 0 to 1 (was 1.6). Pump is a 20:1 duck with a
  260 ms release (was 10:1 / 220 ms): 11 dB of change, level -5 dB. With the stored Sweep fully open (the experiment
  `PP_SWEEP0=0.75`) Pump is identical and Stabs no larger: the stored Sweep did not mask Pump.
- "Stabs too loud by default, and they jump": the stored stab level is -18 dB under full; the Stabs gain is a straight
  line in dB (the old level was a linear amplitude, whose first step is an audible onset), `macro_audit` shows no `jump`.
- "Wash too dissonant with stabs at max": the Wash echo was a dotted eighth against an off-beat eighth stab line, so
  the repeats landed between the stabs, on other notes of the line (E, G, B, A, D), and rang into the next stab; the
  reverb on top carried the cluster. Now the echo is a straight quarter note (repeats land on the next stab, a chord tone
  of the same Em line), 30 % feedback (was 40) and a 2.2 kHz tone (was 4 kHz); the room is damped at 2.2 kHz (was 3.5 kHz).
  The riser and the gesture are unchanged. `loading-dock-wash-with-stabs-at-1.mp3` is the Wash sweep with Stabs held at 1.
- Sweep reads 4 dB on the audit measure (was ~7 with the stabs inside the filter): drums and the ducked bass dominate
  the long-term spectrum; the tour is its test.

**Default changes:** the old Drive (a saturated drive and a crusher that did nothing audible) is gone: Intro no
longer has an 18 % crusher mix, the drive is fixed at its old 26.5 dB; the stab line is a macro, unfiltered, with its
own drive; the old Pump was a ratio that did 80 % of its work in the first notch, it is a threshold now; the duck is
deeper (stored -42 dB at 20:1, was -34 at 10:1); the drums sit 3 dB higher and the hand-played voice 7 dB lower; the
music compressor's makeup is 13.5 dB (was 6; the trim before a compressor with an 8:1 ratio does little, the makeup is
where the level lives). The factory test measures the piece with a held chord on the hand-played voice, so its
loudness differs from the tour's.

### Glass Elevator (FM electric piano)
| macro | stored | 0 -> 1 | moves |
|---|---|---|---|
| Tine | .60 | a mellow body to a glassy tine that rings | FM index .4 to 1.4; tine operator level 0 to .83 and decay 25 to 210 ms |
| Ring | .50 | a short, dry piano to a held, ringing one (level macro: a longer tail is more sound) | every body decay 2.5 times shorter to 2.5 times longer, releases too |
| Tremolo | 0 | clean, then a suitcase-piano tremolo fades in and speeds up | tremolo depth 0 to 5 dB, rate 3.5 to 6.5 Hz; trim -3.5 dB at the top |
| Space | .60 | dry to ping-pong echo and a room | echo mix, room mix |

The old Grit (drive; it clipped at 1) is fixed at its old stored value; the chorus is fixed at its old stored
30 %; the filter at its old stored value (nearly open).

### Undertow (reese sub bass)
| macro | stored | 0 -> 1 | moves |
|---|---|---|---|
| Open | .95 | a closed rumble opens to the stored tone | ladder cutoff 130 Hz to 2.3 kHz |
| Beat | .40 | the two saws pull apart, the beating goes from a slow throb to a fast shimmer | fine tune of the two saws +-5 to +-37 ct |
| Slide | .40 | a step to a long swoop between overlapped notes | glide 5 ms to 900 ms |
| Wobble | .40 | a slow roll in the filter becomes a wub | LFO depth, LFO rate .2 to 3.5 Hz, sweep depth .3 |

The old Weight (drive, 2 dB louder) is fixed at its old stored value, so is the room. **Default change (Kosta: "a bit
quiet generally"):** patch gain +7 dB (the compressor takes part of it back): audit loudness -20.3 to -18.3 dB, factory
test -20.7 to about -18 dB, peak -11.3 dBFS. **Click:** the old Cutoff and
Beating clicked at 5.4 s with a bright filter (saw edges of a 65 Hz reese through an open filter, as in the parked
Corner Store click). Open now stops at 2.3 kHz (the stored tone is 2.0 kHz), so the stored sound and every
position are click-free; the pitch-only macros (Beat, Slide, Wobble) measure small on the spectral measure and are
the tour's test.

### Salt Flat (slow ambient pad)
| macro | stored | 0 -> 1 | moves |
|---|---|---|---|
| Bloom | .65 | each chord leans in within a fraction of a second, then blooms slowly (6.5 s) with a longer tail (level macro) | amp and filter attack 300 ms to 6.5 s, release 1.5 to ~9.6 s |
| Depth | 0 | plain, then a triangle an octave below fills in the floor | octave-down level 0 to .6; trim -8 dB over the travel |
| Glare | .82 | a dark, close haze opens to a bright, open pad and on to a glassy shine | ladder cutoff 300 Hz to 16 kHz (7.9 kHz stored) |
| Space | .65 | dry to a long echo and a huge room (level held: the trim rides down 2 dB over its travel) | echo mix, room mix |

The old Saturation (drive 2.9 dB) is fixed; the slow filter drift keeps its old depth. Old Texture (a shimmer
layer, a wider chorus and a looser unison at once) and old Saturation were the same measure (cosine .89) and the
all-at-1 combination clipped at +1.2 dBFS: both are gone. An octave-up Shimmer and a Breath (filter breathing,
then a level breathing) were tried and rejected (cosine .96 with Glare, .93 with Bloom). Patch gain +4 dB (the old
stored sound was -24.2 dB, now -20.2).

### Boxwood (FM mallet)
| macro | stored | 0 -> 1 | moves |
|---|---|---|---|
| Mallet | .50 | a soft felt mallet (a dull knock) to a hard wooden one with a long overtone and a sharp click | overtone operator level .2 to 1 and decay 30 to ~270 ms, attack 2.5 to .1 ms; click level 0 to .8, decay 12 to 75 ms; trim -1 dB |
| Ring | .50 | a dry knock to a long vibraphone ring (level macro: a longer tail is more sound) | every body decay 2.9 times shorter to 2.9 times longer, releases too |
| Metal | 0 | a wooden bar, then the overtones slide off the harmonics into a struck metal bar | overtone fine tune +600 ct (op 2 and 4), +600 ct (op 5) |
| Space | .45 | dry to an echo and a medium room | echo mix, room mix |

The old Brightness (FM index and filter), Warmth (drive 2.5 dB, +6 dB louder at 1) and Chorus are fixed at their
old stored values. Patch gain +2.5 dB (old stored -25.9 dB, now -23.4: peaky mallet attacks, the peak decides).

### Neon Highway (legato synthwave lead)
| macro | stored | 0 -> 1 | moves |
|---|---|---|---|
| Open | .91 | muffled and dark to fully open (velocity still brightens it) | ladder cutoff 350 Hz to 20 kHz |
| Pulse | .30 | the square pulse under the saws comes forward and thins to a nasal pulse | pulse level .16 to .8, pulse width 50 to 17 %, trim +1 dB |
| Vibrato | .35 | a vibrato fades in, deepens to 20 ct and speeds up (warm, not seasick) | LFO depth 0 to .10 of the travel, rate 4.5 to 6.8 Hz |
| Space | .60 | dotted echo and room | echo mix, room mix |

The old Bite (drive, +5 dB louder at 1) is fixed at its old stored value; glide stays at 70 ms. **Default change:**
the stored filter is 14 kHz (was fully open, 20 kHz): the old Cutoff macro was dead across most of its travel;
the stored sound is -33 dB below the old patch, a hair less top.

## Old flags against new flags

| Preset | Old flags | New flags |
|---|---|---|
| Cinder Block | all four bad end with note-start clicks; Crush dead | clean (Clamp is a level macro by design) |
| Parquet Loop | Cutoff dead | clean |
| Loading Dock | Sweep and Pump duplicate (both drive the trim); Drive dead; Wash jump; Pump dead + jump + level; Sweep clipped | Sweep and Pump duplicate (the trim after the filter is how the level holds; the sounds are not alike: cosine -.18), Wash jump (the riser noise) |
| Glass Elevator | Grit level + peak 0.5 dBFS; all-at-1 over 0 dBFS (+1.3) | clean (Ring is a level macro by design) |
| Undertow | Cutoff jump + click; Beating zipper click | clean |
| Salt Flat | Saturation level; Texture duplicate of Saturation (.89); all-at-1 +1.2 dBFS | Depth and Glare `jump` (the first steps of a layer that starts at nothing / a dark filter floor) |
| Boxwood | Warmth level (+6.2 dB) | clean (Ring is a level macro by design) |
| Neon Highway | Cutoff dead + jump; Bite level (+5.2 dB) | clean |

Honest weak spots (the tours are the test): Parquet Loop Loop and Glide, Undertow Beat/Slide/Wobble and Neon
Highway Vibrato move pitch, time or phase more than the spectrum, so the meters read 1 to 3 dB; Boxwood Metal is
2.3 dB; Loading Dock Stabs 1.6 dB on a bass-heavy mix (the stab line is a few sparse notes).

## Count justifications

No preset has 3 or 5+ macros.

## Default changes (state in the PR)

- Cinder Block: amp attack 1 to 5 ms and filter-envelope attack 1 to 6 ms (note-start click); stored crusher mix
  3 % to 0 (inaudible).
- Parquet Loop: patch gain -5.5 dB (headroom for Phase and Squelch at their ends).
- Loading Dock: see above (no Intro crusher, unfiltered stabs as a macro, a harder pump as a threshold, drums +1.7 dB).
- Undertow: patch gain +7 dB.
- Glass Elevator: patch gain +1 dB (factory loudness -24.7 was under the window).
- Neon Highway: stored cutoff 20 kHz to 14 kHz.
- Salt Flat: patch gain +4 dB; Boxwood +2.5 dB.
- Glass Elevator, Undertow: stored sound unchanged (-217 and -131 dB below the old patch).
