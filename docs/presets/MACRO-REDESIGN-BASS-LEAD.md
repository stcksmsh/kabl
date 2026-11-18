# Macro redesign: bass and lead presets, first half (12 of 24)

Same method as `MACRO-REDESIGN.md` (pilot): each preset gets four macros from what makes that patch interesting, four different kinds of change, clean by default, stored (default) sound unchanged. Builders are in `tools/examples/presets_bass_lead.rs` (`span`, `span_hi`, `Voice::snap`, `Voice::depth_keep`; `routed` recomputes the old stored value). The second half (copper-wire .. short-circuit, 11 presets) is below.

Written from the patch recipes and the audit renders. Nobody has listened yet; "what you hear" is what the parameters and measures say. The A/B tours are the real test.

Where the old space macro (Room) did not matter for the patch, the echo and room are now a fixed wet amount at the old stored mix, and the macro slot went to a gesture of the patch. Presets with a long tail as part of the sound (Acid Rain, Spun Sugar, Hot Wire, Lantern, Low Tide) keep Space.

## Presets (stored position in brackets)

### Steel Toe (French touch bass, ducked on the host clock)
| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Bite [.45] | filter opens from a dull thud to a bright bark, each note snaps harder | ladder cutoff 507 Hz to 5 kHz; filter-envelope depth .08 to .30 |
| Weight [.60] | the sub grows and the saw steps back: a thin mid bass to a sub-heavy one | mixer: square sub .15 to .73, saw .75 to .42 |
| Squash [.50] | the compressor goes from a slow, punchy squeeze to a fast, flat clamp (density more than a level drop) | threshold -18 to -34 dB, ratio 2 to 10, attack 25 to ~1 ms, makeup 6 to 20 dB |
| Pump [.30] | the beat ducks the bass and it takes the whole beat to swell back. The duck is a one-sixteenth gate; the audible breathing is its release, which used to be 50 ms (so the dip was over before the off-beat stabs, and the old decay did nothing) | duck depth 0 to -.60 of the gain range, duck release ~30 to 170 ms |

### Tar Pit (808, sine with pitch snap, slides)
| Macro | What you hear | What it moves |
|---|---|---|
| Thump [.30] | the front of each note: a soft swell at 0, a click and a pitch kick at 1 (the click is a choice now, not the default) | pitch-snap depth .35 to .80, snap decay 15 to ~55 ms, amp attack ~7 to ~3 ms |
| Body [.50] | a short boom to a long ringing tail | amp decay 550 to ~5000 ms, release 120 ms up |
| Dirt [.40] | a clean, warm sine to a gritty one that speaks on small speakers | drive 1.3 to 24 dB (12 dB stored) |
| Slide [.80] | glide between overlapped notes: a quick step to a long swoop | glide time 40 ms to 2.4 s |

### Acid Rain (legato acid bass)
| Macro | What you hear | What it moves |
|---|---|---|
| Squelch [.50] | the filter opens from a dark growl to a bright snarl and the resonance rises with it | cutoff 271 Hz to 4 kHz, resonance .20 to .61 |
| Accent [.60] | envelope shape: short clipped notes to long held ones | amp decay 90 to ~600 ms, sustain .25 to 1, filter-envelope decay 80 to ~600 ms |
| Slide [.40] | glide time between notes | 15 to ~380 ms |
| Space [.70] | the dotted echo repeats more and more (one repeat to a long wash); echo and room mix stay at the old stored level | echo feedback 0 to 64 % |
(The old Drive is fixed at its old stored value.)

### Oatmeal (clean round pop bass)
| Macro | What you hear | What it moves |
|---|---|---|
| Warmth [.20] | a soft saturation adds harmonics to the clean note (harmonics, not filtering: a triangle and a sine have little to filter) | drive 3 to ~21 dB |
| Soften [0.00] | the pop of each note becomes a slower swell and a longer ring-out (articulation; the old Pluck snap stays fixed) | amp attack 3 to 40 ms, release 110 to 600 ms |
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
| Slap [.40] | resonance on the filter snap of each note: a plain pop to a wah (the snap depth stays fixed) | ladder resonance .05 to .43 |
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
| Crush [.25] | the crusher bites into the saturated drive, coarser and coarser (digital grit; the old Grit drive stays fixed, it was saturated) | crusher mix 0 to 40 %, bits 9 to 4, downsample 1.3 to ~6 |
| Octave [.30] | a buzzing pulse an octave up joins | pulse level 0 to .60 |
| Squash [.50] | compressor clamps harder | threshold -12 to -44 dB, ratio 4 to 12, makeup 6 to 22 dB |

### Low Tide (ambient sub)
| Macro | What you hear | What it moves |
|---|---|---|
| Tone [.45] | filter opens, the triangle an octave down comes forward | cutoff ~540 Hz to 5 kHz, triangle .10 to .43 |
| Tide [.40] | a slow filter tide fades in, speeds up and resonates | LFO depth 0 to .26, rate ~.08 to .4 Hz, resonance .02 to .20 |
| Swell [.30] | notes bloom slower and fade longer | attack 65 ms to 1.5 s, release 600 ms up |
| Space [.60] | long echo and big room | echo mix, room mix |

## Checks (macro_audit with the combination check, #159; 8 steps per macro)

Peaks: no step, sweep or combination render is over -1 dBFS (worst case in the combination tables: Low Tide all four at 1 at -2.6 dBFS, stored -3.6). No click appears in a step, sweep, end or combination render.

Flags that are left, honestly:

| Preset | Flag | Note |
|---|---|---|
| Oatmeal | Warmth and Squeeze `cancel` as a pair (7.2 dB against 11.0 dB for Warmth alone) | Both are strong alone (Warmth 12.5 dB, Squeeze 9.8 dB): Warmth makes the note louder and brighter, Squeeze quieter. Left |
| Steel Toe | Pump `jump` (last steps 1.2 and 2.3 dB against a median .4), Squash and Pump `similar` (cos .86) | The duck deepens and lengthens together, so the breathing is mostly in the last third (5.2 dB at 1; measured per beat with a held note: about 10 dB dip at the old stored depth, deeper at 1). Both macros lower the level. Left: Pump is the audible gesture Kosta asked for |
| Steel Toe | pair Squash+Pump `cancel` (7.5 dB against 12.1 dB for the larger single) | Both clamp the level. Left |
| Tar Pit | tour: Body at 0 has a 1.25 s silent gap | The macro itself: a 600 ms tail between sparse 808 notes. Guard run with the default 1 s fails for this clip only |

Every other per-macro flag (`jump`, `dead`, `weak`, `similar`, `bad end`, `zipper`) is cleared in the last audit of all 12 presets, and the combination tables have no `click`, `peak` or `pileup` flag. How the last ones were evened out: Acid Rain Space moves the echo feedback (mix stays at the old level) instead of the dry-to-wet mix; Rubber Band Slap is the resonance on the snap, Stab is decay, sustain and release together; Gutter's old Grit drive was saturated, so the macro is now the crusher (mix, bits, downsample); Low Tide Tide adds a resonance rise and a gentler LFO range and Swell stops at a 1.5 s attack; Oatmeal's Pluck is replaced by Soften (attack and release).

### Clicks (the old Steel Toe 6.5 s and Rubber Band end-render clicks)

Cause in the patches, not the macros: a note that retriggers while the previous note is still ringing restarts the amp (and filter) envelope from its current level with a 1 to 3 ms attack, and the level steps. The Rubber Band click at 0.25 s and 3.15 s was in the stored sound too (second difference -14 dB, isolated); an 8 ms attack on both envelopes removes it (tested with `PP_SET`). Fixed in the patches: amp attack 2 to 6 ms (Steel Toe, Rubber Band, Slate Roof), 3 to 5 ms (Pig Iron), filter-envelope attack 3 to 8 ms (Rubber Band), 4 to 8 ms (Slate Roof), 1 to 6 ms (Pig Iron). This is a change to the default sound of these four presets (a softer first few milliseconds of each note): see below. Slap on Rubber Band and Growl on Slate Roof were also narrowed (Slap to .45 of the knob at 1, Growl to .20) so their click at 1 is gone. (`PP_SET` in `presets_bass_lead` now splits the module kind at the last dot, so dotted kinds such as `env.adsr` work.)

### Level of Tar Pit

The old stored sound was peak -13.8 dBFS, loudness -22.2 dB: in line with the bank by loudness (Steel Toe -16.6, Acid Rain -20.4, Oatmeal -20.9, Spun Sugar -22.6, Pig Iron -22.5, Hot Wire -21.7, Rubber Band -21.4, Gutter -17.7, Low Tide -21.7) but it sounded timid and clicked at every note start. Kosta heard it, so **the default of Tar Pit changed on purpose**: amp attack 1 to 6 ms and snap-envelope attack .5 to 3 ms (no click by default), triangle sub .15 to .30 and drive 11 to 12 dB (more body and presence), patch gain +5 dB. Now peak -12.2 dBFS, loudness -18.5 dB (`presets_bass_lead levels`), mid-bank. The click and punch moved to the Thump macro.

### Default sound against the old patch (16 s clip, float, `presets_bass_lead clips`)

Oatmeal and Spun Sugar are bit-identical; Tar Pit, Acid Rain, Hot Wire, Lantern, Low Tide differ by -125 dB or better (float rounding of the macro line); Gutter by -90.5 dB (the crusher's quantiser amplifies a rounding error). Changed on purpose by the click fix (attack times): Steel Toe -23 dB, Pig Iron -19 dB, Rubber Band -20 dB, Slate Roof -30 dB below the old signal (peaks about 0.2 to 2.3 dB lower), plus the Slate Roof crusher now fixed at 18 %. The other 12 presets are untouched (their patch logs were reverted: only timestamps differ).

## Tours

`docs/presets/clips/macros-bass-lead/` (git-ignored): `<slug>-tour.mp3` per preset: new macro 1 to 4 back to back, each 20 s (6 s at 0, 4 s up, 6 s at 1, 4 s down), 1 s of silence between, 84 s; `tours.txt` lists the start seconds. Rendered at 124 bpm. Tour mode loops the phrase at its own length (first to last hit, rounded up to whole bars) so the sound is continuous (before, it looped every 8 bars and most phrases left about 7 s of silence per clip). `macro_tour_ab.py` fails when a clip has a gap over 1 s under -45 dBFS after its first second (`--max-gap` to relax); with `--old-patches`/`--old-renders` it still writes old/new A/B tours. Listen first: **Steel Toe** (Weight and Squash change the pump), **Tar Pit** (Thump, Body and Slide are new gestures on a sine), **Spun Sugar** (Shimmer and Vibrato).

## Second half (copper-wire .. short-circuit, 11 presets)

Same method. The stored sound of every preset is unchanged (16 s clip against the old patch, difference -99 dB or better) except Loud Mouth and Short Circuit, whose patch gain went up on purpose (below). New helpers: `kick` (a fast envelope off the gate that kicks the pitch of the oscillators at the front of each note; depth on a macro, 0 at the stored position) and `fixed_vibrato`. Old Tone/Cutoff/Drive/Room macros that did not fit a gesture of the patch are fixed at their old stored value; Space stays where the echo is the sound (Copper Wire, Tail Light, Rail Yard, Oak Stave, Glass Needle, Afterglow, Fire Escape).

### Copper Wire (sine ring-modulated by a triangle a fifth up)
| Macro | What you hear 0 to 1 | What it moves |
|---|---|---|
| Metal [.45] | a plain sine bass to bell tones around it | ring modulator level 0 to .7 |
| Weight [.50] | the clean sine under the bells grows from thin to a heavy sub | sine level .25 to .8 |
| Hold [.50] | a short plucked sine to a long ringing drone | amp decay 90 to ~900 ms, sustain .6 to 1, release 25 ms up |
| Space [.25] | echo and room keep the bells ringing | echo mix, room mix |

### Tail Light (synthwave bass)
| Macro | What you hear | What it moves |
|---|---|---|
| Pluck [.50] | a tight pluck to a slow filter open on every note | filter-envelope depth .08 to .32, decay 60 ms to 1 s |
| Width [.40] | the chorus comes in and swims wider, the saw stack spreads | chorus mix 0 to 50 %, depth 15 to 78 %, stack detune 3 to 20 ct |
| Weight [.50] | the square sub grows, the second saw steps back | sub .05 to .35, second saw .55 to .25 |
| Space [.40] | dotted echo and room | echo mix, room mix |

### Rail Yard (house bass ducked on the host clock)
| Macro | What you hear | What it moves |
|---|---|---|
| Pump [.275, was .55 of a half-depth route: same sound] | the beat ducks deeper and the bass takes longer to swell back | duck depth 0 to -.60, duck release ~30 to 110 ms |
| Snap [.50] | the filter bites each note harder and longer | envelope depth .08 to .32, decay 50 to 500 ms |
| Thick [.30] | the saw stack spreads and the drive thickens it | stack detune 0 to 20 ct, drive 3 to 34 dB (8 dB stored) |
| Room [.25] | synced echo and room | echo mix, room mix |

### Oak Stave (upright pluck)
| Macro | What you hear | What it moves |
|---|---|---|
| Pluck [.50] | how hard the filter snaps and how long it takes to close | envelope depth .50 to 1.0, decay 90 to 640 ms |
| Thump [.30] | none to a plucked-string pitch kick at the front of the note | pitch-snap depth 0 to 100 ct |
| Body [.40] | the sine under the note comes forward and rings longer | sine level .15 to .88, amp decay 250 ms to 3.3 s |
| Room [.30] | short room | echo mix, room mix |

### Glass Needle (sine lead, ping-pong echo)
| Macro | What you hear | What it moves |
|---|---|---|
| Overtone [.30] | a thin sine to a glassy ring (the old Overtone, from a faint .03) | octave sine .03 to .43 |
| Ping [.00] | a needle-tap pitch kick on every note (off by default) | pitch kick 0 to 100 ct over 200 ms |
| Vibrato [.10] | a vibrato fades in and speeds up | depth 0 to 30 ct, rate 4.8 to 7.5 Hz |
| Space [.55] | ping-pong echo and room | echo mix, room mix |

### Loud Mouth (formant talking lead; reworked after listening)
Kosta's ear: the Vowel sweep got muddled in the middle (a plain morph passes through blends of two vowels), Talk sounded like an underwater ripple, Shout did almost nothing.

| Macro | What you hear | What it moves |
|---|---|---|
| Vowel [.35] | a clear vowel at every position: a, e, i, o, u with short crossings between (no blends); stored is a clear "e" | formant vowel 0 to 4, with the new `hold` param at 1 |
| Talk [.35] | a talkbox: on the host clock the mouth jumps to a new vowel every eighth note, from still to chattering (replaces the sine drift, which was the ripple). Press Run | tempo-synced sample-and-hold LFO (1/8) on the vowel, depth 0 to 2 vowels |
| Shift [.50] | smaller to larger mouth | formant shift -12 to +12 st |
| Shout [.50] | a mumble to a shout: the drive pushes, the formants sharpen, the level comes up | drive 9 to 32 dB, formant Q .7 to 2.9, compressor makeup 0 to 12 dB (10 dB on the audit measure) |

New in the formant filter: `hold` (0 to 1, default 0 = the old smooth morph, so no other preset changes). At 1 the formants sit on the pure vowel over most of the way and cross to the next within a fifth of it (smoothstep). Loud Mouth sets it to 1. **Default change:** the stored sound is now a steady, clear "e" with a moderate talkbox (before: a blend with a sine drift); level -21.8 dB loudness, peak -12.7 dBFS. Four macros: Vowel, Talk, Shift and Shout are four different gestures (place, rhythm, size, force).

### Velvet Sine (was Dust Devil: reworked after listening)
Kosta's ear: "the name doesn't fit, the slide makes it a kid whistle". The old preset was a sine with a 1.1 s slide, so it whistled. It is now a soft, round lead: the same sine and hint of triangle, a short legato glide, vibrato, the dotted echo. The name says what it is. Whistle (resonance) and Scoop (pitch dive) are gone.

| Macro | What you hear | What it moves |
|---|---|---|
| Glide [.50] | a step to a short, smooth slide between overlapped notes (never a whistle) | glide 5 ms to ~320 ms (40 ms stored) |
| Body [.35] | a pure sine to a rounder, hollow tone | triangle 0 to .57, cutoff 700 Hz to ~5 kHz |
| Bloom [.00] | each note swells in and fades out slowly: a soft pluck to a pad-like lead (off by default) | amp attack 6 ms to 500 ms, release 220 ms to 1.1 s |
| Vibrato [.40] | a vibrato fades in and speeds up | depth 0 to 16 ct, rate 5.6 to 8.5 Hz |

**Default change:** the stored glide is 40 ms (was 1.1 s); everything else at its old stored value; patch gain +3 dB (loudness -20.0 dB). The echo and room are fixed at the old stored level. Slug `velvet-sine` (the patch folder `dust-devil` is removed). Four macros: glide, timbre, envelope and vibrato are four different gestures.

### Afterglow (synthwave lead)
| Macro | What you hear | What it moves |
|---|---|---|
| Spread [.50] | both saw stacks from tight to huge | detune 6 to 72 ct (main), 6 to 30 ct (octave) |
| Rise [.00] | the filter starts closed and opens slowly into each note (off by default) | cutoff 1.8 kHz to 500 Hz base, envelope depth .10 to .70, attack 10 ms to 1.2 s |
| Octave [.20] | the stack an octave up comes forward | level 0 to .6 |
| Space [.60] | long echo and big room | echo mix, room mix |
| Chop [.00] | a trance gate on the host clock, off at 0, up to hard sixteenth-note chops at 1; the echo and room ring through the gaps. Edges 3 ms / 12 ms: no clicks. Press Run | gate envelope on the clock into the output gain, depth 0 to -42 dB |

Five macros (approved by Kosta): Chop is a gesture none of the four others has (rhythm), and it needs its own full-range control. Stored sound unchanged (bit-identical to the old patch, with the echo kept free-running).

### Horn Section (synth brass stab)
| Macro | What you hear | What it moves |
|---|---|---|
| Blat [.50] | the swell at the front of each stab gets deeper and slower | envelope depth .10 to .60, attack 20 to ~180 ms |
| Section [.40] | one tight horn to a loose section | stack detune 2 to 27 ct, second saw -1 to -16 ct |
| Blare [.30] | the brassy drive | drive 0 to 20 dB |
| Vibrato [.10] | a vibrato fades in and speeds up | depth 0 to 28 ct, rate 5 to 7 Hz |

### Fire Escape (house stab)
| Macro | What you hear | What it moves |
|---|---|---|
| Dive [.50] | how far each note falls into its pitch, and for how long | dive depth 0 to 120 ct, envelope decay 40 to ~800 ms |
| Stack [.50] | a tight chord to a wall | detune 6 to 66 ct and 4 to 40 ct |
| Length [.50] | a short stab to a held one | amp decay 60 to ~1.1 s, release 25 to ~600 ms |
| Space [.40] | dotted echo and room | echo mix, room mix |

### Short Circuit (ring-mod lead)
| Macro | What you hear | What it moves |
|---|---|---|
| Metal [.50] | the ring modulator comes in on the saw | level 0 to .7 |
| Crush [.35] | the bitcrusher mixes in and gets coarser | mix 0 to 70 %, bits to 5, downsample to 5 |
| Snap [.50] | a soft click to a zap on every note | envelope depth .03 to .28, decay 40 to ~360 ms |
| Siren [.00] | the ringed square wobbles in pitch, a drift to a siren (off by default) | LFO on its fine tune 0 to 100 ct, rate .5 to 9 Hz |

## Checks (second half, before the listening fixes; Loud Mouth, Afterglow and Velvet Sine were audited again, see above; `macro_audit` with the combination check, 8 steps per macro)

Peaks: no step, sweep or combination render is over -1 dBFS (worst combination: Oak Stave Body+Pluck at -5.5 dBFS). No click in any render. No `peak`, `click` or `pileup` flag in any combination; no pair of macros over cosine .8.

Flags that are left, honestly:

| Preset | Flag | Note |
|---|---|---|
| Velvet Sine | pair Glide+Body `cancel` (3.6 dB against 5.2) | Left |
| Afterglow | Rise `jump` | A time gesture on a slow lead (a 1.2 s filter attack): the meters see it only at the end. Listen |
| Horn Section | Blat `jump` | The swell deepens and slows together, so the last steps move most |
| Oak Stave | pair Body+Room `cancel` (2.3 dB against 3.5) | Room hides the longer ring of Body. Left |
| several | small meter change (Vibrato .4 to 1.5 dB, Rise 1.2 dB) | These move pitch or time, which the spectral measure does not see; the tours are the test |

How the redesign started from the audit: Copper Wire's first attempts (Interval, Strike) were the same sound as Metal (cosine .94 to .99: everything went through the ring modulator); Weight and Hold are different kinds of change. A Knock macro (filter snap) was dead on a pure sine and was dropped.

### Default sound against the old patch (16 s clip, float)
Bit-identical or -99 dB or better below the old signal for eight of the eleven (float rounding of the macro line). Changed on purpose: **Loud Mouth** patch gain +4 dB (loudness -25.8 to -21.2 dB, the quietest preset of the bank), **Short Circuit** +2 dB (-23.9 to -21.1 dB) and **Velvet Sine** +3 dB (-20.0 dB loudness, after the overseer asked for more presence). Levels of the eleven (`presets_bass_lead levels`): loudness -18.6 to -21.9 dB, peak -5.6 to -14.1 dBFS. The old Rail Yard Pump route is now the same depth with a duck release (the old duck release was fixed at 50 ms; stored sound the same).

## Tours (second half)

`docs/presets/clips/macros-bass-lead-2/` (git-ignored): `<slug>-tour.mp3` and `tours.txt`, same format as the first half (new macros only, 20 s each, 124 bpm, gap guard on: all pass). Listen first (re-rendered after the fixes): **Loud Mouth**, **Velvet Sine**, **Afterglow** (Chop is the fifth clip, at 84 s).
