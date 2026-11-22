# Voice presets (P2-voice): ten voices and FX

Ten presets of the `Voice` role (BRIEF.md, Roles table): vowel, formant, talkbox and robot voices; metal, glass and noise; a riser and an impact. Generator: `tools/examples/presets_voice.rs` (helpers copied from `presets_pad.rs`; no other generator, module or preset changed). Method as in `MACRO-REDESIGN.md`: each macro is a different kind of change, clean by default, texture on a macro, no limiter, no drum module, peaks at or under -1 dBFS at every macro position.

Split by category: the five voices (Sock Puppet, Automaton, Vox Chop, Oracle, Maw) are `Voice`; the five FX (Temple Bowl, Chandelier, Whiteout, Updraft, Depth Charge) are `FX`, a new category that shows under the Voice shelf in the Sounds scene. Sound unchanged; only the category in each `sound.toml` moved.

Written from the patch recipes and the audit renders (`macro_audit`, 8 steps per macro and the combination renders). Nobody has listened yet except through the meters; the "what you hear" columns are what the parameters and the measures say. The tours are the test (see Clips).

## The list

| Preset (category) | Genre | What it is | Loudness / peak, dBFS (level test: all macros at the stored value) | Space macro |
|---|---|---|---|---|
| Sock Puppet (Voice) | French touch (signature) | mono saw lead through a vocoder; the "voice" is a formant mouth built into the patch | -21.2 / -6.9 | none: close and dry |
| Automaton (Voice) | French touch (signature) | poly robot choir: a vocoder whose voice is a monotone buzz speaking a looping sentence | -20.4 / -6.7 | none: close and dry |
| Vox Chop (Voice) | house | chord stabs through a formant filter, vowel hops on the host clock, clock chop | -22.3 / -8.1 | Space |
| Oracle (Voice) | ambient and cinematic | wordless choir, clear vowels, slow bloom, long room | -20.7 / -6.2 | Space |
| Temple Bowl (FX) | experimental and ambient | struck inharmonic FM bowl with a ring modulated partner | -21.2 / -3.7 | Space |
| Chandelier (FX) | pop and ambient | bowed crystal: sine, high FM partial, rubbed rim | -19.3 / -4.5 | Space |
| Whiteout (FX) | experimental | noise through a key-tracked comb: wind that sings the notes | -21.7 / -4.5 | Space |
| Updraft (FX) | techno and house | riser: hold a key and a closed filter opens over seconds | -20.5 / -8.0 | Space |
| Depth Charge (FX) | hip-hop and cinematic | one-shot impact: falling sine, noise crack, hall | -21.2 / -4.9 | Space |
| Maw (Voice) | drum and bass | mono growl bass voice, a formant mouth moved by a free LFO | -22.4 / -11.8 | none: dry and heavy |

Two signature presets (Sock Puppet, Automaton; the brief allows three). Sock Puppet, Automaton and Maw have no space macro on purpose: their sound is close and dry (signature rule, and a bass), so they carry no echo or room at all and their four macros are all about the voice. Sock Puppet and Vox Chop need Run for their clock-driven macros (Chatter, and Vowels and Chop): both descriptions say so; without Run they are plain sounds and the no-MIDI check renders near silence (nothing sounds without a note; the clock only moves a formant and a duck).

The vocoder needs a modulator, and a factory preset cannot rely on audio input: Sock Puppet and Automaton build theirs inside the patch (`mouth` in the generator: a 98 or 110 Hz saw buzz and a little noise through a `filter.formant` with `hold` at 1, so every vowel is clear). The played notes are the carrier. The clock is the only thing Sock Puppet's Chatter needs; Automaton's sentence is a free LFO and needs nothing.

## Macros

Stored = the macro value in the patch (the designed sound). Level compensation: every macro that would change the loudness has its own `gain` stage that follows it (so no two macros share a parameter; the stored sound is unchanged by it), so the level test and the audit see roughly constant loudness. Macros marked "level-type" move the level on purpose.

### Sock Puppet (mono, 124 bpm clock, Run for Chatter)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Mouth | 0.30 | The talkbox mouth steps a, e, i, o, u, clear at every position (no blends); the mod wheel moves it too | formant vowel 0 to 4 on the modulator, `hold` 1 |
| Chatter | 0.35 | The mouth is still, then hops to a new vowel on every eighth of the host clock, up to chattering | tempo-synced sample-and-hold LFO (1/8) into the vowel, depth 0 to 0.75 |
| Weight | 0.40 | A saw an octave down joins under the lead | mixer level 0 to 0.5, a trim keeps the level |
| Bite | 0.21 | Clean and round to driven, with the consonant hiss and a crusher coming in | drive 0 to 28 dB (trim -0.5 to -12), vocoder `hf` 0.2 to 0.9, crusher mix 0 to 60 % |

Swapped against the plan: the planned Size (vocoder formant shift) measured as the same gesture as Mouth (cosine 0.86), so it became Weight.

### Automaton (poly, close and dry)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Speech | 0.50 | One held vowel, then a sentence that walks a, e, i, o, u and back every four seconds | free saw LFO (0.25 Hz) into the modulator's vowel, depth 0 to 0.5 of the range |
| Detail | 0.45 | A smeared, vowel-like robot to a crisp one with consonants | vocoder follower release 160 to 12 ms, `hf` 0 to about 0.55 |
| Size | 0.50 | A small, bright robot to a large, dark one | vocoder formant shift +6 to -6 st |
| Weight | 0.40 | A saw an octave down joins under the robot: a low-band change, unlike Speech (which moves the top) | mixer level 0 to 0.5, a trim keeps the level |

Detail first moved the vocoder's band count; changing it while playing clicked (flag `zipper` in the audit), so the band count is fixed at 16 and Detail moves the follower time and the consonant noise. Crush (a bitcrusher) was replaced by Weight in the second round: Speech and Crush both moved the top bands (cosine 0.97, over the 0.8 rule), and the octave down is a different part of the spectrum.

### Vox Chop (poly, 124 bpm clock, Run)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Chop | 0.50 | A steady chord to a deep pulse on every eighth: a silent envelope ducks the level, no drum | `clock.div` (1/8) gate into a short envelope into a gain stage, depth 0 to 42 dB; a trim keeps the level |
| Vowels | 0.50 | One vowel, then a mouth that jumps to a new vowel on every sixteenth | sample-and-hold LFO (1/16) into the formant vowel, depth 0 to 0.5 |
| Shift | 0.50 | A small, bright mouth to a large, dark one | formant shift +7 to -7 st |
| Space | 0.30 | Dry to a tempo-free slapback and a tight room | echo mix 0 to 20 %, room mix 0 to 30 % |

### Oracle (poly)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Vowel | 0.10 | a, e, i, o, u in clear steps (a stored "a" to start); the mod wheel moves it | formant vowel 0 to 4, `hold` 1 |
| Breath | 0.00 | Pure choir, then a whisper through the same vowels comes in | noise into the formant, mixer level 0 to 0.4 (noise at 0 dB) |
| Choir | 0.23 | One voice to a crowd: the saw stack spreads and the chorus widens | saw detune 3 to 34 ct, chorus mix 10 to 28 %, depth 35 to 45 % (a trim keeps the level) |
| Space | 0.65 | Dry to a long echo and a big room | echo mix 0 to 30 %, room (8 s) mix 0 to 60 % |

### Temple Bowl (poly)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Strike | 0.28 | A soft, long mallet contact to a hard, short one: the strike gets a sharper edge (the tone itself stays about the same, so it is a different gesture from Metal) | carrier FM index 0.8 to 1.8, mallet envelope decay 900 ms to about 60 ms |
| Metal | 0.61 | A tuned bowl to an inharmonic clang | modulator ratio 1 to 3.9, ring modulated partner level 0 to 0.2 |
| Ring | 0.66 | A short tap to a seven second ring (level-type: a ring that lasts seven seconds is more sound than a tap, +4 dB on the tour measure after a 10 dB trim) | amp decay 350 ms to 7 s, release 300 ms to 1.2 s |
| Space | 0.60 | Dry to a long echo and a hall | echo and room mix |

### Chandelier (poly)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Rub | 0.00 | Clean glass, then a low wet-finger hum comes in (noise low-passed at 700 Hz, under the glass) with a very small vibrato (a few cents: the motion is the friction, not the pitch). Level-type: it adds a layer of sound, and compensating it made it a plain level change that the audit read as the same gesture as Bow | rim noise level 0 to 1, vibrato depth 0 to 4 ct |
| Overtone | 0.35 | The glassy FM partial comes forward and bends from the octave to the inharmonic glass mode (2.76 times the pitch): timbre, not level | partial level 0 to 0.14, partial tune 0 to +557 ct, a trim keeps the level |
| Bow | 0.63 | A struck ping that dies away to a slow bow-in that holds: the envelope (level-type: a slow attack is quieter over a short note) | amp attack 8 ms to 2.6 s, sustain 0.5 to 0.98 |
| Space | 0.60 | Dry to a long echo and a hall | echo mix 0 to 25 %, room mix 0 to 45 %; a trim keeps the level |

### Whiteout (poly)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Pitch | 0.75 | Plain wind to a note that rings: the comb follows the key | comb feedback 0 to 94 %, comb level 0.3 to 0.75, raw noise 0.75 to 0.3 |
| Gust | 0.35 | A still wash to a filter that breathes, deeper and faster | LFO into the ladder cutoff, depth 0 to 0.3 of the range, rate 0.1 to 2.3 Hz |
| Grain | 0.00 | Clean wind to a gritty, crushed one (hiss is the sound of this preset; the grit is on the macro) | crusher mix 0 to 80 % (5 bits, downsample 6) |
| Space | 0.50 | Dry to a long echo and a big room | echo and room mix |

### Updraft (poly; hold a note for the whole build)
The filter opens the same way at every macro position (filter envelope on the cutoff, fixed 0.25). Each macro has its own dimension: time, pitch, texture, tail.

| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Build | 0.45 | How long the rise takes: 1.5 s to 10 s | filter envelope attack 1.5 to 10 s |
| Lift | 0.10 | The pitch climbs with the rise: none to an octave (about a whole tone stored) | filter envelope depth on the oscillators' base pitch, 0 to 0.1 of its travel (an octave) |
| Air | 0.30 | The saws give way to a noise wash (level held by a trim) | noise mixer level 0.05 to 0.72 and noise source -14 to 0 dB, saw levels 0.41 / 0.16 to 0.05 / 0.02, trim -4.8 to +11.2 dB |
| Space | 0.40 | Dry to an echo and a room | echo and room mix |

Distinctness (owner: "the macros might be similar"). Change vectors from the audit's steps 0 against 7 (third-octave levels over bands above the floor plus 40 dB, stereo width, the level and centroid curves over the held note, flatness, centroid rise time, pitch). Before, Lift (filter depth, resonance) was Build (time) backwards: cosine -0.84, and Heat (drive, noise) moved little. Now no pair is over 0.45: Build/Lift -0.32, Build/Air -0.42 (a ten second build is darker over the held note, the noise brighter), Build/Space +0.10, Lift/Air +0.20, Lift/Space -0.10, Air/Space +0.00.

### Depth Charge (poly; every key is a one-shot)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Boom | 0.54 | A short thud to a four second boom (a trim keeps the level within about 1 dB) | amp decay 250 ms to 4.5 s, release 150 to 600 ms |
| Crack | 0.39 | A dull thud to a hard noise crack on the front | noise burst level 0 to 0.9 |
| Drop | 0.50 | The pitch falls less, then falls a long way into the thump | fall envelope depth on both oscillators, 0 to 0.18 of the base-pitch range |
| Space | 0.60 | Dry to a long echo and a seven second hall | echo mix 0 to 15 %, room mix 0 to 50 % |

### Maw (mono, dry)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Vowel | 0.30 | The mouth in the bass goes a to u | formant vowel 0 to 4 (smooth morph: `hold` 0, a growl wants the blends) |
| Growl | 0.40 | A still mouth to a fast snarl | free LFO into the vowel, depth 0 to 0.3, rate 0.4 to 6 Hz |
| Grit | 0.31 | Warm to biting: drive and a crusher | drive 4 to 30 dB (trim 0 to -10), crusher mix 0 to 50 % |
| Weight | 0.40 | A sine sub comes in under the voice | mixer level 0 to 0.75, level held by a trim |

Growl and Vowel move the mouth, not the pitch (nothing here modulates the pitch of Maw, Chandelier's vibrato is a few cents).

## Checks (`macro_audit` with the combination renders, the level test, the tours)

Peaks: every step, sweep and combination render of the audit is at or under -1 dBFS. The factory level test (`factory_levels_hold_the_target`) passes for all ten with no `LOUD_EXCEPTIONS` entry. It sets every macro to 0.65, then 0.15, mid-note, and each macro alone to 1, and the generator's `testlevel` command replays it so a preset can be levelled without the four-minute test (`PP_SEGMENTS=1` prints the peak per stretch, to see which macro setting peaks). Worst peak over those renders, dBFS: Sock Puppet -6.9, Automaton -6.1, Vox Chop -4.0, Oracle -1.9, Temple Bowl -2.7, Chandelier -3.5, Whiteout -4.0, Updraft -6.4, Depth Charge -1.5, Maw -6.1. No click in any render. No limiter anywhere. The phrase clips pass `audio_defects.py` (clean).

**Presence (second round).** The phrase means (ffmpeg `mean_volume` of the committed clips) were -23 to -27 dB for five presets. The ceiling was the test's all-macros-at-0.65-then-0.15 case, not the phrases (phrase peaks sat at -8 to -11), so: the macro settings that peaked were compensated instead of the whole preset, a gentle compressor (threshold -22 to -26, ratio 3 to 4, no limiter) went after the voice of Oracle, Temple Bowl and Whiteout to take the crest down, Maw and Vox Chop got compressor makeup, Chandelier and Depth Charge were retrimmed, and the Depth Charge and Temple Bowl phrases got more hits (a one-shot phrase has silence between hits, which the mean counts). Phrase means now: Maw -19.2, Chandelier -19.2, Vox Chop -19.4, Automaton -20.4, Whiteout -20.9, Sock Puppet -21.6, Oracle -21.1, Updraft -20.9, Depth Charge -22.0, Temple Bowl -22.5. The last two sit a little under the -22 line: their level test peaks are at -1.5 and -2.7 dBFS and more gain would break the peak rule.

**Level drift of the non-level macros (second round).** Measured on the tours (RMS of 6 s at macro 0 against 6 s at macro 1, the others at their stored values): Sock Puppet within 0.5 dB, Automaton 1.1, Updraft 1.7 (Space; Build, Lift and Air are within 0.6), Whiteout 1.5 (Space), Maw 1.0, Vox Chop 1.1, Oracle 1.4 (Space; Vowel, Breath and Choir are within 0.7), Depth Charge 0.7. Each macro that moved the level has its own `gain` stage (`comp_gain`). Level-type, with the reason: **Temple Bowl Ring** (+4 dB: a seven second ring is more sound than a tap), **Chandelier Rub** (+4.5 dB: it adds a layer of hum; a compensating trim made it a plain level change that the audit read as the same gesture as Bow), **Chandelier Bow** (about 0 on the tour, -6 dB in the 10 s audit window: a 2.6 s attack is quiet at the start of a short note). The audit's first-10-seconds window and the tour's looped 20 s do not agree for the slow macros (Bow, Rub, Ring); the tour numbers are the ones above.

| Preset | All at 0 (peak / loudness) | All at 1 | Flags that are left, honestly |
|---|---|---|---|
| Sock Puppet (Voice) | -7.9 / -20.6 | -9.3 / -22.1 | Weight and Bite `jump` (a new layer and the crusher come in from nothing) |
| Temple Bowl (FX) | -3.6 / -27.5 | -7.8 / -22.8 | Metal `jump` (the ring modulated partner comes in from nothing); Ring level-type (above). No pair over 0.8 (Strike and Metal read 0.92 until Strike became an envelope change) |
| Depth Charge (FX) | -8.4 / -24.4 | -11.9 / -25.5 | Boom and Crack read as opposite (cosine -0.90: a longer boom is darker, the crack is the bright top; two different gestures, hardness against length) |
| Oracle (Voice) | -10.6 / -21.0 | -3.2 / -17.6 | clean |
| Automaton (Voice) | -7.2 / -20.3 | -9.9 / -21.1 | Speech `jump`; no pair over 0.8 (Speech and Crush read 0.97 until Crush became Weight) |
| Vox Chop (Voice) | -5.8 / -18.5 | -4.5 / -18.1 | Vowels `jump` |
| Chandelier (FX) | -5.8 / -19.6 | -14.4 / -29.7 | Overtone and Bow `jump`; Bow and Rub level-type (above). All at 1 is quiet because Bow at 1 is a 2.6 s attack. Third round (owner: "the macros might be a tad similar"): each macro now has its own dimension, Rub texture (noise), Overtone timbre (the partial bends inharmonic), Bow envelope (attack and sustain), Space room (width). Change-vector cosines (third-octave bands within 40 dB of the top, width, onset, attack, sustain, tail, flatness; macro alone 0 against 1) at most 0.32, was 0.39 (Bow and Space) and -0.49 (Overtone and Bow) with Bow barely moving |
| Whiteout (FX) | -6.7 / -20.1 | -10.7 / -26.0 | Pitch `jump` (the comb starts ringing) |
| Updraft (FX) | -13.7 / -22.9 | -9.3 / -23.2 | clean |
| Maw (Voice) | -7.4 / -22.3 | -8.5 / -22.6 | clean |

## Gain staging notes

- A `gain` stage stops at +24 dB: Whiteout's trim was clamped there, so its level was raised at the noise source instead.
- The compensation stage of a macro is a `gain` module after the voice (`comp_gain` in the generator); it is why no two macros of a preset share a parameter.
- `level_trim` in the generator is 0: the preset trims live in each recipe's `trim_db`.

## Clips

In `docs/presets/clips/voice/`: `<slug>.mp3` per preset (the phrase, 10 to 20 s, played as meant; committed) and `<slug>-tour.mp3` (git-ignored, on disk only: macro 1 to 4 back to back, each 20 s: 6 s at 0, 4 s up, 6 s at 1, 4 s down, 1 s of silence between, 84 s per file), and `clips.txt` with the start second of every tour clip and what to listen for. Gap guard on (no silence over 1 s under -45 dBFS in a clip after its first second); Depth Charge needs `--max-gap 3` (its Space at 0 is a dry one-shot with a gap between hits, as Tar Pit's Body at 0). Regenerate: `presets_voice write patches`, `presets_voice phrases <phrases.json>`, `MACRO_TOUR_BPM=124 macro_audit patches <phrases.json> <renders> <slug>`, `docs/presets/scripts/macro_tour_ab.py --patches patches --renders <renders> --out <dir> <slug>`.

Listen first: Sock Puppet Mouth and Chatter (the vocoder is new ground: do the vowels read as vowels), Temple Bowl Metal (the top rises 28 dB at 1: a clang, or harsh?), Whiteout Pitch (does the wind sing the key), Updraft Lift (an octave climb at 1: a riser, not a whistle).
