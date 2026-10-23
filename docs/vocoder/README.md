# Vocoder and audio input

A channel vocoder for the robot voice, and a way to bring a real voice into kabl. A vocoder imposes one sound's spectral shape on another: a voice (the modulator) is split into bands, each band's loudness follows the voice, and the same bands of a synth (the carrier) are opened by those levels. Play a saw chord, speak, and the chord speaks.

## The modules

**`vocoder`** (an effect). Inputs: `in_l`, `in_r` (the carrier, stereo), `mod` (the voice). Outputs: `left`, `right`.

| Control | Range | What it does |
|---|---|---|
| Bands | 8 to 32 (16) | Slices the voice is split into. More is clearer speech, fewer is more robotic. |
| Shift | -12 to 12 st (0) | Moves the synth's side of the bands against the voice's: up is a smaller mouth, down a larger one. |
| Attack, Release | 0.5 to 50 ms (4), 5 to 500 ms (40) | How fast a band opens and closes. Short release is crisp, long is smeared. |
| Hf (face label: Hiss) | 0 to 1 (0.5) | Unvoiced path: hiss that opens only on `s`, `t`, `f` (when the voice is mostly above 5 kHz), shaped by the voice there. 0 is the clean vocoder, exactly. Sounds only while the synth does. Default 0.5 puts the hiss about 13 dB under the vocoded sound on the stand-in voice's consonants and about 18 dB under it on vowels (it was 11 dB under on vowels before the gate: the "static"). |
| Drive | 0 to 36 dB (18) | Gain of the voice into the bands. The vocoder multiplies, so a quiet voice makes a quiet result. 18 rather than 24: 6 dB more headroom for a hot voice and 6 dB less microphone floor. The module's output peak with the -20 dBFS stand-in voice was 0.26 at 24, so nothing clipped; this is margin, not a fix for a fault. `live-voice` makes the 6 dB up in its output gain. |
| Mix | 0 to 1 (1) | The synth alone to the vocoded sound alone. |
| Spacing (advanced) | 0 to 1 (0) | 0: equal musical steps; 1: evenly spread, more bands at the top. |
| Low, High (advanced) | 60 to 400 Hz (120), 3 to 16 kHz (9) | The range the bands cover. |

Method, numbers and limits are in the header of `crates/modules/src/builtins/vocoder.rs`. In short: two cascaded TPT state-variable band-passes per band for the voice and for each carrier channel (a stereo carrier keeps its width), a rectifier and one-pole follower per band, the carrier band times the follower, summed. Parameters are read every 16 samples; the filters are redesigned only while a control moves. 32 bands cost 36 us per 64-sample block at 48 kHz (2.7 % of the 1333 us budget), 12.8 % of the block budget at 192 kHz (`cargo test --release -p kabl-modules --test vocoder cpu_at_32_bands -- --ignored --nocapture`).

**`audio.in`** (a source). Outputs `left`, `right`, one `gain_db` knob. The host's input; a mono source plays on both. Non-finite samples read as 0 and everything is clamped to +-16, so silence, clipping, NaN or an unplugged device never reach the patch as anything worse than a loud finite signal. Silent until a host feeds it.

## Where the voice comes from

Both, as asked:

1. **Internal modulator, no microphone.** The vocoder's `mod` takes any audio, so a robot phrase can be made from modules kabl already has: a buzz (`osc.va` saw) through `filter.formant` whose vowel steps through a random loop, gated by an envelope, plus a noise burst at each syllable start. That is `robot-voice`. Everyone hears it at once.
2. **Audio input.** `audio.in` carries the real voice.
   - *Standalone:* `kabl-ui --input` opens the default input device at the output's sample rate. It is opt-in so no microphone is opened unless asked. A missing device, a device without that rate, or one that disappears while playing leaves the patch hearing silence; the output keeps playing. The status line says what happened and Retry reopens the input with the output.
   - *Plugin:* one optional stereo input port. REAPER shows it as input pins 1 and 2 (`2 in+out`): put the voice on the kabl track itself or send a track to it. See below for what this changes in existing projects.

## The plugin port, and existing projects

The CLAP plugin used to have no inputs. It now has one auxiliary stereo input. What was checked in REAPER 7.75 (scripts in `scripts/host-*`):

- **A project made before the port existed loads and renders identically.** The same project (the factory `palette/keys` and `palette/pad` patches played by a MIDI chord, 10 s) rendered with the build from `master` and with this build is sample for sample the same (largest difference 0.0). The saved state needs no change and the schema stays at 7.
- **Audio already on the kabl track.** Before, an audio item or a send on an instrument track with no inputs passed through the plugin untouched. With an input port REAPER hands it to the plugin and takes the plugin's output instead, which would have silenced it. The plugin therefore adds its input to its output whenever the patch has no `audio.in` (every existing patch). Measured: the same project with a voice item on the kabl track has the same level and spectrum in every 0.5 s window before and after; the pass-through is 64 samples (1.3 ms) later, REAPER's delay compensation for the plugin's own latency.
- **A patch with `audio.in`** takes the input and passes nothing through. A voice sent from another track to the kabl track (channels 1 and 2) gives the same sound as the offline render of the same patch (envelope correlation 0.998); REAPER's pan law makes a mono send 3 dB quieter.
- The CLAP validator (rev `152b982`) result moved by one test: `process-audio-denormals` was skipped (no inputs) and now runs and passes. The baseline `docs/factory-performance-bank/evidence/validator.json` carries that one change.

Hosts differ in how they show a sidechain-style input. If another host maps the port to a different channel pair, the voice goes there; nothing else about the plugin changes. Option if a host dislikes the port: a second layout without it (nih-plug allows several); not built, no host asked for it.

## Demo patches

| Patch | What it is |
|---|---|
| `sound-engines/demos/robot-voice` | Hold chords (E3 B3 E4 G4). A supersaw chord speaks a phrase that plays itself, a new vowel and a click every tick. 20 bands. |
| `sound-engines/demos/live-voice` | The same chord vocoded by whatever arrives on Audio in. Silent until something does. |

Both follow the factory level rule (`docs/factory-levels/README.md`) and pass `factory_levels_hold_the_target`. The factory render has no microphone, so `audio.in` patches are rendered with a stand-in voice: `voice-espeak-16k.wav`, scaled to -20 dBFS RMS over its sounding samples. The patches are generated by `scripts/make_vocoder_demos.py`; `cargo run -p kabl-engine --example resave -- DIR` writes the `checkpoint.json` the factory bank needs.

## Listen

Clips in `audio/` (AAC), made by `scripts/make_clips.py` with the offline renderer (`render_midi`, which now takes `--audio-in FILE.wav`):

The clips were made and checked by measurement (levels, spectra, envelope correlation), not by ear: I cannot listen. What to listen for is what the design should give; judge it and tell me where it is wrong.

- `robot-phrase.m4a`: 8 s of the phrase alone (a buzz through the vowel filter with a soft noise puff on every step), a pause, then the same 8 s spoken by the chord. Listen for the same rhythm and vowel changes in the chord's pitches. The puffs are the consonants of the patch's design: white noise at -15 dB, 25 ms attack, 90 ms decay (it was a 0 ms attack burst, then 10 ms; both still read as clicks in the dry modulator, which has no other discontinuity: with the puffs muted its largest second difference is 0.5x its 20 ms level, with them 3.3x before this change and 2.3x after). Robot-voice Hf is 0.8 to keep the quieter puffs audible after the vocoder.
- `band-sweep.m4a`: the same 4 s at 8, 12, 16, 24 and 32 bands, joined by 60 ms crossfades (five separate renders: the module is not swept live in this clip; `moving_any_parameter...` tests that). Listen for the vowel colour at 8 bands and how much of each syllable's shape appears as bands are added. Whether 24 and 32 are better or only different is the thing to judge; the default is 16 (20 in `robot-voice`).
- `live-voice.m4a`: a voice alone, a pause, then through the live-voice patch with two chords (E minor, then C). Listen for whether the words stay readable under the chord change and whether the `s` and `t` sounds are too hissy (the Hf knob) or too weak.

Envelope correlation between the voice and the vocoded clip over twelve bands from 150 Hz to 7 kHz averages 0.64 (log magnitude; speech and its pauses included).

Captures in `media/`: `A|B-light|dark-all50.webp` (the self-playing patch at 50 %), `*-vocoder.webp` (the vocoder close up, live bars), and `plugin-reaper-vocoder.webp` (the plugin editor in REAPER). Face: bar per band on a log frequency axis as tall as the voice is loud there (the module's own follower, via `Module::view`), a tick under each bar where the synth's side of the band sits after Shift. With audio off the bars rest low so the picture still shows where the bands are.

## Tests

- `crates/modules/tests/vocoder.rs` (12 run, 1 ignored CPU print): a 1 kHz voice puts the carrier's 1 kHz harmonic 45 dB over its 3 kHz one; Shift moves the peak an octave; output follows the voice level (6 dB for 6 dB; bands 8 to 32 within 5 dB); no voice is silence and Mix 0 is the carrier exactly; the unvoiced path adds hiss only for a hissing voice, and only with a synth under it; block sizes 1, 7 and 33 give identical output to 64; extreme settings and NaN, infinity and 1e30 input stay finite at 44.1, 48, 96 and 192 kHz and recover; no allocation in `process`; every parameter swept there and back while a saw is held under a steady broadband voice never drops a 512-sample block under 25 % of the running level (lowest measured 0.21, Drive; band count 0.65) and never steps harder than the steady run does; a vowel with a little top end gets no hiss from Hf while a hissing voice does (Hf 0 is exactly the clean vocoder); `audio.in` passes what it is fed and sanitises the rest.
- `crates/engine/tests/audio_in.rs`: the host's block reaches the output, silence before a host feeds it.
- `crates/standalone/src/input.rs`: a short or empty queue pads with silence, a backlog is trimmed to the newest frames, a closed input feeds silence.
- `crates/ui/tests/factory_bank.rs`: the stand-in voice is -20 dBFS; the factory level test covers both demos.

## Known gaps

- The standalone's missing-device and vanishing-device paths are covered by the silence and queue tests and by reading the error callback; an input device was not unplugged during a live run.
- The unvoiced path is one band (5 to 12 kHz) with a fixed 5 kHz voice corner. A talkbox model, pitch correction and a sampler are out of scope.
- `audio.in` takes the host's block as it arrives: no resampling, so the standalone needs an input at the output's sample rate.
- The vocoder is a global (not per-voice) effect, so a patch with two vocoders runs two banks; there is no input metering on `audio.in` yet.
- Local clippy is rustc 1.93; CI's newer stable may flag more.

## Reproduce

```
cargo build --release -p kabl-engine --example render_midi -p kabl-ui --bin kabl-ui
python3 docs/vocoder/scripts/make_clips.py
Xvfb :153 -screen 0 1600x1000x24 &
DISPLAY=:153 python3 docs/vocoder/scripts/capture.py OUTDIR
python3 docs/vocoder/scripts/host-make-project.py scratch/p.rpp patches/palette/keys
python3 docs/vocoder/scripts/host-run.py scratch/p.rpp target/release/libkabl_clap.so NAME   # then host-compare.py
```
