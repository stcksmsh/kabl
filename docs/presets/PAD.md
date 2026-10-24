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

(filled in below once built)
