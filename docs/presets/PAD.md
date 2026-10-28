# Factory presets: pads

Third part of the bank (`BRIEF.md`): Pad to 14, with the pilot's Salt Flat counting. Generator: `crates/ui/examples/presets_pad.rs` (same recipe as `presets_bass_lead.rs`: one `Recipe` for the voice, one function per preset, a `Preset` entry with metadata and a clip phrase). Each preset is a folder under `patches/presets/` and an entry in the `FACTORY` list of `crates/ui/tests/library.rs`.

I can render and measure but not listen. Everything below about sound is what the chain is built to do plus what the renders show; Kosta's ears decide.

Stereo: the engine's step 1 is on master (`docs/stereo/DESIGN.md`): a stereo cable is spelled with the `left`/`right` and `in_l`/`in_r` pairs of chorus, phaser, reverb, delay and the other fx modules, and a mono module after a stereo signal runs once per channel. Pads use only that: a mono voice, then stereo fx stages (chorus, phaser, reverb), so the width comes from unison detune and the fx, not from a stereo voice path. No `PortInfo.width`, no `split`/`join`, no engine change. A true stereo voice (each unison voice panned) waits for step 2.

## Plan: 14 pads

Signature share: 4 of 14 (about a quarter). Space defaults follow the brief: ambient, pop and synthwave pads sit at 0.5 to 0.7, signature pads at 0.2 to 0.35. Every pad is poly, ends in the limiter at -1.5 dB, and names its four macros after what they do.

| Preset | Genre | Character | Macros (CC 20 up) | Signature | Group |
|---|---|---|---|---|---|
| Salt Flat | ambient | slow, clean, staticky layer on request | Bloom, Saturation, Space, Texture | no | pilot |
| Fog Bank | ambient | very slow saw and triangle cloud, long attack, 9 s room | Bloom, Drift, Depth, Space | no | 1 |
| Deep Field | cinematic | sub-heavy swell that opens over seconds, octave-up air | Swell, Sub, Shimmer, Space | no | 1 |
| Cotton Mill | pop, analog | pulse and saw with pulse-width motion and chorus | Cutoff, Motion, Chorus, Space | no | 1 |
| Picture Window | pop | bright seven-voice saw stack, clean and wide | Cutoff, Spread, Sparkle, Space | no | 1 |
| Drive-In | synthwave | lush detuned saws, slow vibrato, wide chorus, long echo | Cutoff, Wobble, Chorus, Space | no | 1 |
| Dry Cleaner | French touch | phased chords, compressed, close and dry, a DJ filter sweep | Sweep, Phase, Grit, Room | yes | 1 |
| Concrete Cloud | French touch | thick saturated pad: drive into a hard compressor, bitcrusher on request | Weight, Cutoff, Crush, Room | yes | 1 |
| Static Garden | evolving, textured | triangle bed with noise, ring mod and a random filter walk | Texture, Motion, Metal, Space | no | 1 |
| Glacier | ambient, cinematic | glassy FM bell pad, slow swell, long echo | Frost, Swell, Drift, Space | no | 2 |
| Pump House | French touch, 124 BPM | chord pad ducked by its own kick grid (Run starts it) | Cutoff, Drive, Pump, Room | yes | 2 |
| Tin Voice | French touch | robot vowel chords through a formant filter, dry and driven | Vowel, Grit, Shift, Room | yes | 2 |
| Rooftop | pop, analog | detuned saw pair, slow filter envelope, brassy swell | Cutoff, Swell, Chorus, Space | no | 2 |
| Tidepool | evolving | slow random movement in pitch and filter, glassy and wet | Drift, Motion, Tone, Space | no | 2 |

Slugs checked against `master`, `claude/presets-keys-pluck` and `claude/presets-keys-pluck-2`: none of the thirteen new slugs exists there. Words kept out of descriptions and macro names because the library tests search for them elsewhere: `warm`, `singing`.

## Group 1

Clips in `docs/presets/clips/pad/group1/` (`<slug>.flac` and `<slug>.mp3` at 320 kbps, `tour.flac`). Tour start times (one second of silence between clips): concrete-cloud 0, cotton-mill 15, deep-field 30, drive-in 57, dry-cleaner 84, fog-bank 94, picture-window 121, static-garden 136 seconds. Clip phrases are five-note chords at velocity 100 (the level test uses four notes at 90), so Fog Bank and Deep Field touch the limiter at -1.5 dB in the clip.

| Preset | Macros (CC 20 up) | Listen for |
|---|---|---|
| Fog Bank | Bloom, Drift, Depth, Space | Chords that take about two seconds to arrive and several to fade; Drift up for a slow shimmer in pitch and filter, Depth for an octave of weight underneath. |
| Deep Field | Swell, Sub, Shimmer, Space | The filter opening over several seconds on every held chord; Swell at 0 stays dark, Sub and Shimmer add an octave below and above. |
| Cotton Mill | Cutoff, Motion, Chorus, Space | The pulse width breathing inside held chords; Chorus at 0 is dry and narrow. |
| Picture Window | Cutoff, Spread, Sparkle, Space | A clean, bright stack that widens with Spread; Sparkle adds airy octave-up saws. |
| Drive-In | Cutoff, Wobble, Chorus, Space | Wide chorused saws with a slow pitch sway; Chorus at 0 is a plain stack. |
| Dry Cleaner | Sweep, Phase, Grit, Room | Phaser sweeping through short, compressed chord stabs; Phase at 0 is a plain saw chord, Sweep works like a DJ filter. |
| Concrete Cloud | Weight, Cutoff, Crush, Room | A heavy, squashed chord that stays dense while held; Crush up for lo-fi grit, Weight up for more saturation. |
| Static Garden | Texture, Motion, Metal, Space | Hiss and grain around the chord (Texture), bell-like inharmonic overtones (Metal), a filter that wanders by itself (Motion). |

### Checks (group 1)

- `every_preset_keeps_to_the_brief`, the library tests and `factory_levels_hold_the_target` pass.
- Levels as `factory_bank::level_table` measures them (peak / loudness dBFS): Fog Bank -7.1 / -21.0, Deep Field -7.3 / -22.1, Cotton Mill -4.0 / -19.2, Picture Window -3.6 / -19.9, Drive-In -4.6 / -19.9, Dry Cleaner -3.9 / -20.7, Concrete Cloud -8.3 / -20.7, Static Garden -4.2 / -20.1. The generator's own `levels` command plays the same chord at the same times but reads a few dB differently for the slow pads; the test's number is the one that counts.
- Macros (`presets_pad macros patches 1`): every macro moves the render. Weakest: Static Garden Texture (-13 dB under the render, after raising the noise level and the route; the first version was -29 dB), Concrete Cloud Crush (-17 dB). Several, such as Deep Field Swell or Fog Bank Space, move the render as much as the signal itself.
- Clicks: the first scan (energy above 3 kHz in 5 ms windows) found nothing, and was wrong for dark material, see Round two. With the second-difference test below, all eight clips have no flagged window (Concrete Cloud's one window at 0.25 s is the note starting from silence). Highest high-frequency share of any window: Dry Cleaner -3 dB (its short stabs), Concrete Cloud -9 dB; the rest under -6.5 dB.
- clips: 320 kbps mp3 against the float render, all eight pass (SNR 36 to 73 dB below 16 kHz, peaks within 0.1 dB).
- REAPER (`scripts/host.py`, evidence in `evidence/host-pad/`): all eight saved, quit, reopened with identical state and identical render; nothing plays without MIDI.
- Standalone (`scripts/standalone.py`, evidence in `evidence/standalone-pad/`): all eight open, take a chord from a virtual MIDI controller and stay running, audio on the silent sink.

### Notes

- Stereo is only the engine's step 1: every pad is a mono voice into stereo chorus, phaser, delay and reverb stages. Unison detune and the fx make the width; a stereo voice path (each unison voice panned, `osc.va` `out_r`) waits for step 2.
- Dry Cleaner has no clock and no pump: it plays chords, the phaser and compressor supply the movement. The kick-ducked pad is Pump House (group 2).
- Fog Bank and Deep Field use long attacks, so the level test's three-second holds sit on the swell; their loudness once a chord has fully opened is a little higher than the number above.

## Round two (Kosta's first listen)

Deep Field crackled from about 5 s. The overseer's test: per 50 ms window, peak of the second difference over its RMS above 12 (single-sample kinks, which a high-frequency-energy scan misses on a dark, smooth signal). Before: Deep Field 32 flagged windows in the 26 s clip (first at 5.95 s), every other pad 0 except Concrete Cloud's onset window. Found by rendering the chain with one stage changed (`PP_SET`): the dry signal (Space at 0) and the delay alone are clean; the crackle came with the room, and about half of it with the limiter.

- The limiter (`limiter.rs`) has instant attack and no lookahead and says so itself: on a signal that keeps pushing it, the gain moves within one sample, which is a kink. The five-note clip chords pinned Deep Field and Fog Bank at the -1.5 dB ceiling. With the ceiling raised so the limiter never acts, 32 windows became 14.
- The room: a ten second reverb on a sub-heavy signal. Reverb only (delay and room off otherwise), ceiling raised: 24 windows at 10 s decay, 123 at 30 s, 1 at 3 s, 3 at 4 s, 22 at 8 s. The kinks scale with decay (the tank level), they are single samples about 1 % of the signal. Not changed here: the reverb module's modulated allpass is the likely source; it is an engine matter, flagged, not touched.

Fix, in the patches only: Deep Field's room from 10 s to 6.5 s, and a gentle compressor (threshold -16 dB, 3:1, 15 ms attack, 250 ms release) between the room and the limiter, on Deep Field and Fog Bank (`Voice::guard`), so the limiter is only a safety. Trim raised to keep the level (Deep Field +3 dB, Fog Bank +1.5 dB). Character kept: the slow swell, the sub and the long tail; the tail is a little shorter. After: 0 flagged windows in both, 0 in the other six (apart from the onset window), levels -22.1 and -21.0 by the factory test, clip peaks -4.7 and -5.1 dBFS. REAPER and standalone checks rerun for both. Tour start times unchanged.
