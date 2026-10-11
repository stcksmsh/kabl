# Effects and filters

Eight new modules and one new output for 1.0: `phaser`, `flanger`, `compressor`, `eq`, `limiter`, `bitcrusher`,
`filter.comb`, `filter.formant`, and a `notch` output on `filter.svf`. They follow the conventions of `chorus`,
`drive` and `filter.ladder`: stereo effects take `in_l`/`in_r` and give `out_l`/`out_r` (left alone, a mono source
fills both), every time-varying or cutoff-like parameter can be cabled at audio rate, knob edits glide (20 ms) so
nothing zippers, state rides across live edits, and nothing on the audio path allocates or locks. Every module is
numerically bounded at its extremes, from 44.1 to 192 kHz and block sizes from 1 up. No schema bump: saved patches
and the factory bank load and render unchanged.

The method of each module is written at the top of its source file in `crates/modules/src/builtins/`; this page is
the summary, the measurements and the clips. Decisions: `docs/decisions.md`, "Effects and filters". Shared helpers
(smoothing, time constants, delay reads) are in `fx.rs`.

## Modules

| Module | Ports | Parameters | Method |
|---|---|---|---|
| `phaser` | in_l, in_r, out_l, out_r | stages 2-12, rate 0.05-10 Hz, depth, feedback -95..95 %, mix | 2-12 TPT all-pass stages per side, sine LFO sweeping about 140 Hz to 4.5 kHz, right side a quarter turn behind; tanh in the feedback path |
| `flanger` | in_l, in_r, out_l, out_r | rate 0.02-5 Hz, depth, feedback -95..95 %, mix | 0.5-6.5 ms delay, Hermite interpolation, sine LFO, quarter-turn stereo; feedback of either sign through tanh |
| `compressor` | in_l, in_r, sc, out_l, out_r, gr | threshold -60..0 dB, ratio 1-20, attack, release, knee 0-24 dB, makeup | Stereo-linked feed-forward, quadratic soft knee, smoothing in dB; `sc` replaces the internal detector when patched; `gr` outputs the reduction in dB (0 or more) |
| `eq` | in_l, in_r, out_l, out_r | low shelf freq/gain, two bells freq/gain/Q, high shelf freq/gain (+-18 dB) | TPT state-variable bands (Simper), four bands in series |
| `limiter` | in_l, in_r, out_l, out_r | gain 0-24 dB, ceiling -24..0 dB, release | Instant attack, one-pole release, final clamp to the ceiling; no lookahead, no latency |
| `bitcrusher` | in_l, in_r, out_l, out_r | bits 1-16 (fractional), downsample 1-64x (fractional), mix | Mid-tread rounding and a sample-and-hold phase accumulator; no dither, no filters, so the aliasing is the effect |
| `filter.comb` | in, pitch, out | freq (played at pitch 0, C4 default), feedback -97..97 %, damp | Feedback comb at `1/f` with Hermite delay read and damping lag compensated; negative feedback sounds an octave lower (odd harmonics) |
| `filter.formant` | in, out | vowel 0-4 (a e i o u), shift -12..12 st, q 0.5-4 | Five parallel TPT band-passes from a tenor formant table, log-domain interpolation between vowels |
| `filter.svf` `notch` | in, cutoff CV, res CV; lp, bp, hp, notch | unchanged | Fourth output, low-pass plus high-pass |

## Measured

CPU, one instance, 48 kHz, block 256, default parameters, noise in, best of five runs
(`cargo test --release -p kabl-modules --test effects cpu_per_module -- --ignored --nocapture`):

| Module | ns per sample | % of one core |
|---|---|---|
| bitcrusher | 9.9 | 0.047 |
| limiter | 12.4 | 0.059 |
| formant | 14.3 | 0.069 |
| compressor | 14.9 | 0.071 |
| eq | 41.3 | 0.198 |
| comb | 44.4 | 0.213 |
| phaser | 82.5 | 0.396 |
| flanger | 123.9 | 0.595 |
| (for reference) filter.svf | 20.2 | 0.097 |
| (for reference) filter.ladder | 60.7 | 0.291 |
| (for reference) chorus | 64.1 | 0.308 |

Tests: `crates/modules/tests/effects.rs` (neutral settings, measured frequency responses, comb pitch to 0.00 cents,
compressor timing and sidechain, limiter ceiling, extreme-setting safety at 44.1-192 kHz and blocks 1 to large,
live-edit state carry, no allocation on the audio path in debug builds). Run with `-- --nocapture` for the numbers.

Host: `docs/effects-filters/evidence/host/result.json`. Nine self-playing tracks, one per demo, in REAPER (CLAP
plugin): all load, plugin states identical across save, quit, reopen and save, the reopened render bit-identical, the
render finite (peak 0.99, rms 0.30) and different from the dry version (relative difference 0.21). The older
factory-bank project loads with identical state and renders bit-identical to the build from before this batch.
Standalone: each demo opens in the editor, plays 10 s with the audio device open and real-time priority granted
(`evidence/standalone/`). Faces: `faces/<kind>/light.png` (light theme only), checked for legible controls.

Scripts: `scripts/make_demos.py` (builds the demo patches and renders the clips), `scripts/host_effects.py`,
`scripts/standalone.sh`, `scripts/capture_faces.sh`.

## Clips

Each clip is 7 s dry, then 7 s through the module, same music (a random loop held to D dorian on a saw voice, no
delay or reverb). Dry and wet are not loudness-matched; levels were set by peak.

| Clip | Listen for |
|---|---|
| `audio/phaser.m4a` | Six-stage notches sweeping slowly, moving left to right across the field. |
| `audio/flanger.m4a` | A jet-like sweep with strong positive feedback: ringing metallic peaks rising and falling. |
| `audio/compressor.m4a` | The loud attack of each note pulled down and the tail brought up (8:1 at -30 dB). |
| `audio/eq.m4a` | Bass lifted, highs cut, and a 3 Q bell around 700 Hz that a slow LFO moves up and down. |
| `audio/limiter.m4a` | 18 dB more drive against a -6 dB ceiling: louder and denser, peaks flattened, no overs. |
| `audio/bitcrusher.m4a` | 4 bits and 8x downsample: gritty, aliased, the pitch with fizz on top. |
| `audio/comb.m4a` | Noise bursts ringing as plucked strings at the sequence's pitches, darkening as they decay. |
| `audio/formant.m4a` | The saw voice speaking: a slow vowel sweep (half a vowel either way) around "i". |
| `audio/notch.m4a` | A saw with a notch sweeping through it: a hollow, phasing sound without resonant peaks. |

None of these has been heard by a person yet.

## Known gaps

- The limiter has no lookahead (see decisions), so a very hard-driven limiter distorts on the peaks it catches.
- Faces are basic and light-theme only; the dark-theme capture step of the shared script fails ("no target theme:dark").
- The bitcrusher's downsample value reads "4X" on the face (the unit string is upper-cased by the shared formatter).
- The clips are not loudness-matched, and nobody has listened to them.
