# Voice presets (P2-voice): ten voices and FX

Ten presets of the `Voice` role (BRIEF.md, Roles table): vowel, formant, talkbox and robot voices; metal, glass and noise; a riser and an impact. Generator: `tools/examples/presets_voice.rs` (helpers copied from `presets_pad.rs`; no other generator, module or preset changed). Method as in `MACRO-REDESIGN.md`: each macro is a different kind of change, clean by default, texture on a macro, no limiter, no drum module, peaks at or under -1 dBFS at every macro position.

Written from the patch recipes and the audit renders (`macro_audit`, 8 steps per macro and the combination renders). Nobody has listened yet except through the meters; the "what you hear" columns are what the parameters and the measures say. The tours are the test (see Clips).

## The list

| Preset | Genre | What it is | Loudness / peak, dBFS (level test: all macros at the stored value) | Space macro |
|---|---|---|---|---|
| Sock Puppet | French touch (signature) | mono saw lead through a vocoder; the "voice" is a formant mouth built into the patch | -21.2 / -6.9 | none: close and dry |
| Automaton | French touch (signature) | poly robot choir: a vocoder whose voice is a monotone buzz speaking a looping sentence | -20.4 / -6.7 | none: close and dry |
| Vox Chop | house | chord stabs through a formant filter, vowel hops on the host clock, clock chop | -22.3 / -8.1 | Space |
| Oracle | ambient and cinematic | wordless choir, clear vowels, slow bloom, long room | -20.7 / -6.2 | Space |
| Temple Bowl | experimental and ambient | struck inharmonic FM bowl with a ring modulated partner | -21.2 / -3.7 | Space |
| Chandelier | pop and ambient | bowed crystal: sine, high FM partial, rubbed rim | -20.7 / -5.3 | Space |
| Whiteout | experimental | noise through a key-tracked comb: wind that sings the notes | -21.7 / -4.5 | Space |
| Updraft | techno and house | riser: hold a key and a closed filter opens over seconds | -20.2 / -5.4 | Space |
| Depth Charge | hip-hop and cinematic | one-shot impact: falling sine, noise crack, hall | -21.2 / -4.9 | Space |
| Maw | drum and bass | mono growl bass voice, a formant mouth moved by a free LFO | -22.4 / -11.8 | none: dry and heavy |

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
| Crush | 0.00 | Clean to a coarse digital grind (5 bits, downsample 6) | crusher mix 0 to 80 % |

Detail first moved the vocoder's band count; changing it while playing clicked (flag `zipper` in the audit), so the band count is fixed at 16 and Detail moves the follower time and the consonant noise.

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
| Breath | 0.00 | Pure choir, then a whisper through the same vowels comes in | noise into the formant, mixer level 0 to 1 (noise at 0 dB) |
| Choir | 0.23 | One voice to a crowd: the saw stack spreads and the chorus widens | saw detune 3 to 34 ct, chorus mix 10 to 28 %, depth 35 to 45 % |
| Space | 0.65 | Dry to a long echo and a big room | echo mix 0 to 30 %, room (8 s) mix 0 to 60 % |

### Temple Bowl (poly)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Strike | 0.28 | A soft mallet to a hard clang | carrier FM index 0.3 to 3.5 (the strike envelope rides on top) |
| Metal | 0.61 | A tuned bowl to an inharmonic clang | modulator ratio 1 to 3.9, ring modulated partner level 0 to 0.2 |
| Ring | 0.66 | A short tap to a seven second ring | amp decay 350 ms to 7 s, release 300 ms to 1.2 s; a trim keeps the level |
| Space | 0.60 | Dry to a long echo and a hall | echo and room mix |

### Chandelier (poly)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Rub | 0.00 | Clean glass, then a wet-finger rim hum comes in with a very small vibrato (a few cents: the motion is the friction, not the pitch) | rim noise band (1.1 kHz) level 0 to 0.45, vibrato depth 0 to 4 ct |
| Overtone | 0.35 | The glassy FM partial comes forward | partial level 0 to 0.29 |
| Bow | 0.63 | A struck ping to a slow bow-in (level-type: a slow attack is quieter over a short note) | amp attack 8 ms to 2.6 s |
| Space | 0.60 | Dry to a long echo and a hall | echo and room mix |

### Whiteout (poly)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Pitch | 0.75 | Plain wind to a note that rings: the comb follows the key | comb feedback 0 to 94 %, comb level 0.3 to 0.75, raw noise 0.75 to 0.3 |
| Gust | 0.35 | A still wash to a filter that breathes, deeper and faster | LFO into the ladder cutoff, depth 0 to 0.3 of the range, rate 0.1 to 2.3 Hz |
| Grain | 0.00 | Clean wind to a gritty, crushed one (hiss is the sound of this preset; the grit is on the macro) | crusher mix 0 to 80 % (5 bits, downsample 6) |
| Space | 0.50 | Dry to a long echo and a big room | echo and room mix |

### Updraft (poly; hold a note for the whole build)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Build | 0.64 | How long the rise takes: 1.5 s to 10 s | filter envelope attack 1.5 to 10 s |
| Lift | 0.25 | How far the filter sweeps open (and how much it rings), with only a very small pitch rise (80 cents at most, so it reads as a riser, not a whistle slide) | envelope depth on the cutoff 0 to 0.9, resonance 0.1 to 0.5, envelope depth on fine tune 0 to 0.3 |
| Heat | 0.30 | Clean to saturated, with more noise | drive 0 to 20 dB (trim -3 to -10), noise level 0.15 to 0.25 |
| Space | 0.40 | Dry to an echo and a room | echo and room mix |

### Depth Charge (poly; every key is a one-shot)
| Macro | Stored | What you hear 0 to 1 | What it moves |
|---|---|---|---|
| Boom | 0.54 | A short thud to a four second boom (level-type: a longer tail is more energy) | amp decay 250 ms to 4.5 s, release 150 to 600 ms |
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

## Checks (`macro_audit`, 8 steps per macro, combination renders)

Peaks: every step, sweep and combination render of the audit is at or under -1 dBFS. The factory level test (`factory_levels_hold_the_target`) passes for all ten with no `LOUD_EXCEPTIONS` entry. It sets every macro to 0.65, then 0.15, mid-note, and each macro alone to 1, and the generator's `testlevel` command replays it so a preset can be levelled without the four-minute test. Worst peak over those renders (stored value and macro sweeps), dBFS: Sock Puppet -6.9, Automaton -6.4, Vox Chop -7.5, Oracle -2.9, Temple Bowl -2.0, Chandelier -2.6, Whiteout -1.5, Updraft -4.8, Depth Charge -3.2, Maw -11.0. Loudness is -20.2 to -22.4 dB (the brief's window is -19 +- 5.5). The first level pass was wrong for Oracle, Temple Bowl and Whiteout (peaks of 0 to +3 dBFS with every macro at 0.65): the audit renders put the other macros at their stored values, the level test moves them all at once. No click in any render. No limiter anywhere. The phrase clips pass `audio_defects.py` (clean).

The audit table below is from before the last gain trims (Oracle -6 dB, Temple Bowl -3.5 dB, Chandelier -1 dB, Whiteout -1.3 dB, Maw +2.5 dB in the Grit trim, Vox Chop +1.5 dB, Whiteout Gust compensation): gain only, so the flags are unchanged.

| Preset | All at 0 (peak / loudness) | All at 1 | Flags that are left, honestly |
|---|---|---|---|
| Sock Puppet | -7.9 / -20.6 | -9.3 / -22.1 | Weight and Bite `jump` (a new layer and the crusher come in from nothing) |
| Temple Bowl | -2.6 / -25.2 | -5.8 / -23.1 | Strike and Ring read as opposite (cosine -0.93: Strike brightens, Ring darkens; different gestures: hardness against length). Metal `jump` and level +2.6 dB (the ring modulated partner comes in from nothing) |
| Depth Charge | -11.5 / -28.2 | -8.6 / -23.1 | Boom level-type (+3.4 dB); Space `jump` (the dry-to-hall first step) |
| Oracle | -8.5 / -20.3 | -3.4 / -18.3 | Vowel level swing 9.6 dB: the vowels differ in energy (the formant filter passes each vowel at its own level, and a single comp stage cannot undo a swing that is not linear in the macro); level-type |
| Automaton | -7.5 / -20.5 | -10.8 / -21.3 | Speech and Crush cosine 0.97, Detail and Crush opposite -0.95 (they all move the top bands; by ear one is a sentence, one is grit); Speech and Crush `jump` |
| Vox Chop | -10.7 / -23.9 | -8.9 / -23.4 | clean |
| Chandelier | -8.9 / -21.4 | -9.4 / -25.2 | Bow level-type (-4.7 dB); Overtone and Bow `jump` |
| Whiteout | -8.0 / -22.7 | -7.4 / -23.3 | Pitch `jump` (the comb starts ringing) |
| Updraft | -13.6 / -21.3 | -8.0 / -22.2 | clean |
| Maw | -13.7 / -29.6 | -15.4 / -30.4 | clean |

## Gain staging notes

- A `gain` stage stops at +24 dB: Whiteout's trim was clamped there, so its level was raised at the noise source instead.
- The compensation stage of a macro is a `gain` module after the voice (`comp_gain` in the generator); it is why no two macros of a preset share a parameter.
- `level_trim` in the generator is 0: the preset trims live in each recipe's `trim_db`.

## Clips

Git-ignored, on disk in `docs/presets/clips/voice/`: `<slug>.mp3` per preset (the phrase, 8 to 24 s, played as meant) and `<slug>-tour.mp3` (macro 1 to 4 back to back, each 20 s: 6 s at 0, 4 s up, 6 s at 1, 4 s down, 1 s of silence between, 84 s per file), and `clips.txt` with the start second of every tour clip and what to listen for. Gap guard on (no silence over 1 s under -45 dBFS in a clip after its first second); Depth Charge needs `--max-gap 3` (its Space at 0 is a dry one-shot with a gap between hits, as Tar Pit's Body at 0). Regenerate: `presets_voice write patches`, `presets_voice phrases <phrases.json>`, `MACRO_TOUR_BPM=124 macro_audit patches <phrases.json> <renders> <slug>`, `docs/presets/scripts/macro_tour_ab.py --patches patches --renders <renders> --out <dir> <slug>`.

Listen first: Sock Puppet Mouth and Chatter (the vocoder is new ground: do the vowels read as vowels), Temple Bowl Metal (the top rises 28 dB at 1: a clang, or harsh?), Whiteout Pitch (does the wind sing the key), Updraft Lift (a riser, not a whistle).
