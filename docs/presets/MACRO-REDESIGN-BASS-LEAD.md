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
| Cutoff [.50] | dark growl to bright snarl | cutoff 271 Hz to 4 kHz |
| Squelch [.35] | resonance and the snap of each note rise, the filter starts to drive | resonance .25 to .69, ladder drive 0 to 5.7 dB, envelope depth .09 to .29 |
| Slide [.40] | glide time between notes | 15 to ~380 ms |
| Space [.25] | dotted echo and room | echo mix, room mix |
(The old Drive is fixed at its old stored value.)

### Oatmeal (clean round pop bass)
| Macro | What you hear | What it moves |
|---|---|---|
| Tone [.80] | muffled to bright low-pass | cutoff 687 Hz to 4 kHz |
| Pluck [.50] | a snap at the front of each note, longer as it deepens | envelope depth 0 to .70, decay 60 to 540 ms |
| Body [.50] | hollow triangle to a full sine an octave down | sine .30 to .90, triangle .70 to .30 |
| Squeeze [.40] | the light compressor clamps harder | threshold -8 to -43 dB, makeup 0 to 7.5 dB |

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
| Crush [.50] | a plain hard lead to a coarse digital grind | crusher mix 0 to 70 %, bits 10 to 4 |
| Length [.60] | a stab to a held blare | sustain .50 to 1, release 20 ms up |

### Hot Wire (legato acid lead)
| Macro | What you hear | What it moves |
|---|---|---|
| Cutoff [.50] | closed growl to scream | cutoff 634 Hz to 9 kHz |
| Squelch [.50] | resonance, drive and snap rise | resonance .20 to .71, ladder drive 0 to 4 dB, envelope depth .05 to .25 |
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
| Tone [.50] | thud to bright pop | cutoff 350 Hz to 4 kHz |
| Slap [.55] | the filter snap of each note: deeper and longer | envelope depth 0 to .60, decay 40 to 175 ms |
| Stab [.50] | short and dry to long and ringing | amp decay 80 to 845 ms, sustain .10 to .60 |
| Body [.50] | the saw an octave down comes forward of the pulse | saw .15 to .75, pulse .60 to .30 |

### Slate Roof (neuro bass)
| Macro | What you hear | What it moves |
|---|---|---|
| Cutoff [.45] | closed rumble to bright snarl | cutoff ~395 Hz to 4 kHz |
| Growl [.35] | the filter rolls faster and deeper | LFO depth 0 to .25, rate 1.2 to ~20 Hz |
| Stack [.50] | a tight stack to a wall | stack detune 8 to 52 ct, second saw fine 2 to 14 ct |
| Crush [.20] | the digital edge comes in | crusher mix 0 to 90 % |

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

Peaks: no step, sweep or combination render is over -1 dBFS (worst case in the combination tables: Low Tide stored -3.6 dBFS, Steel Toe pairs -4.0). No click appears in the sweeps of the redesigned macros except the ones listed below.

Flags that are left, honestly:

| Preset | Flag | Note |
|---|---|---|
| Steel Toe | click at 6.5 s in the end renders of Bite and Squash and in the pairs Weight+Squash, Squash+Pump | The old Room macro had the same click at 6.5 s (the clock-duck phrase). Not fixed here |
| Rubber Band | clicks at 0.2, 1.7, 3.1 s in the end renders | Already in the old patch (old Tone, Slap, Grit, Room all had them): they come from the patch's own retrigger at macro 0, not from a macro. Parked |
| Slate Roof | click at 7.3 s in pairs Cutoff+Growl and Growl+Crush | Growl is the LFO on the cutoff; the old Crush jump is also still there |
| Pig Iron | Bite and Crush `jump` (last step); pair Crush+Length click at 7.7 s; Crush+Length `cancel` | Crush is back-loaded (the crusher is mostly in the highs) |
| Low Tide | Tide and Swell `jump` | Slow LFO and slow attack: the 10 s render window holds few cycles |
| Oatmeal | Tone `weak`, Pluck `weak`, similar (cos .92) | The patch is a triangle and a sine: above 2 kHz there is little to filter. Honest limit of the sound |
| Acid Rain, Hot Wire, Rubber Band | Cutoff and Squelch (or Tone and Slap) `similar` (.97 to .99) | Two filter gestures on one saw. Different in kind (cutoff against resonance and snap) but the measure sees the same brightening |
| Slate Roof | Growl and Crush `similar` (1.0) | Both brighten the top; one is motion, one is a digital edge |
| Hot Wire | all four at 1 `cancel` (4.2 dB against 6.1 dB for the largest single) | Cutoff opens and Squelch resonance rises: they trade level |

Default render against the old patch (16 s clip, float, `presets_bass_lead clips`): Oatmeal and Spun Sugar are bit-identical; every other preset differs by -125 dB or better (float rounding of the macro line); Gutter by -90.5 dB (the crusher's quantiser amplifies a rounding error). The other 12 presets are untouched (their patch logs were reverted: only timestamps differ).

## Tours

`docs/presets/clips/macros-bass-lead/` (git-ignored): `<slug>-ab.mp3` per preset, old and new interleaved per macro slot, 6 s at 0, 4 s up, 6 s at 1, 4 s down, 1 s of silence between; `tours.txt` lists the start seconds. Rendered at 124 bpm (the phrases' grid). Listen first: **Steel Toe** (the clock duck, Weight and Squash change the pump), **Tar Pit** (Thump, Body and Slide are all new gestures on a sine), **Spun Sugar** (Shimmer and Vibrato).

## Not done

Presets 13 to 24 (second slice). The Steel Toe and Rubber Band clicks. Oatmeal's weak Tone and Pluck.
