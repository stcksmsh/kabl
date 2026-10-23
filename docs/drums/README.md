# Drum voices

Five synthesised drum voices and a MIDI-note trigger. Analog-style (808 and 909 territory), made for four-on-the-floor grooves and for pumping a pad against the kick through the compressor's sidechain.

| Kind | Name | Inputs | Output |
|---|---|---|---|
| `drum.kick` | Kick | `gate`, `accent`, `velocity` | `out` |
| `drum.snare` | Snare | `gate`, `accent`, `velocity` | `out` |
| `drum.clap` | Clap | `gate`, `accent`, `velocity` | `out` |
| `drum.hat` | Hi-Hat | `closed`, `open`, `accent`, `velocity` | `out` |
| `drum.tom` | Tom | `gate`, `accent`, `velocity` | `out` |
| `midi.note` | MIDI Note | none | `gate`, `velocity` |

All voices are global-rate modules (one instance, not one per MIDI voice). They stay silent and cost a few nanoseconds per sample until a gate arrives.

## Structure decision: one module per voice

A rack is a set of patch cables between modules with their own faces. A drum is a voice you want to mix, filter, compress and tap on its own: the kick goes to the compressor's `sc` jack, the hat to a delay, the snare through a reverb. One module per voice gives each its own jacks (so a sequencer lane, an arpeggiator or a MIDI note drives it with one cable), its own face and display, and its own cost: an unused voice is a few comparisons per block, and a module that is not in the patch is not there at all. A kit module would also hit the engine's four-outputs-per-module limit (`MAX_OUTPUTS`) or need a mixer inside it that the rack already has.

The hat is one module with two gate inputs, `closed` and `open`. Both play the same six-oscillator source through one envelope, so a closed hit that arrives while the open hat rings retakes the envelope with the short decay: the choke, as on the 808 pedal.

## Triggering

- A rising edge through 0.5 on `gate` (`closed` / `open` on the hat) starts a hit on that sample. Falling edges are ignored (one-shots).
- `velocity` and `accent` are read on the trigger sample only. Hit gain is `(1 - vel_sens * (1 - velocity)) * 10^(accent_db * accent / 20)`. An unpatched `velocity` (a held 0) counts as full velocity, so a bare gate plays at full level. `vel_sens` and `accent_db` are advanced params.
- Works with `seq` (gate + velocity), `arp`, `clock` (16th pulses), `clock.div`, `logic`, `comparator`, and `midi.note`.
- **Retrigger without a click.** A hit that starts while the voice still sounds moves its first sample onto the last sample of the old output and fades the difference out over 1 ms (`Declick`). A hit from silence is left alone, so its attack stays sharp.

## MIDI notes: `midi.note`

`midi.in` cannot route single notes to separate triggers, and a `midi.in` gate reaching a global module is averaged over the voice lanes. The smallest working addition is a `midi.note` module: one MIDI note number (`note`) and channel (`channel`, 0 = all) in, `gate` and `velocity` out. The engine hands it every note-on in the same place it hands notes to the arpeggiator (`PatchEngine::send_key`), with the sample offset the host or controller gave, so the gate rises on the note-on's sample. The gate is a fixed 10 ms pulse (a drum reacts to the rising edge only); a repeated note drops the gate for one sample first. `velocity` holds the last velocity, 0 to 1.

General MIDI drum map, one `midi.note` per drum, patched to the voice's `gate` and `velocity`:

| Drum | Note | Voice input |
|---|---|---|
| Kick | 36 | `drum.kick` `gate` |
| Snare | 38 | `drum.snare` `gate` |
| Clap | 39 | `drum.clap` `gate` |
| Closed hat | 42 | `drum.hat` `closed` |
| Open hat | 46 | `drum.hat` `open` |
| Tom | 45 | `drum.tom` `gate` |

Set `channel` to 10 for a DAW drum track on the GM channel. Not the default (ALL), so a keyboard works too.

Known limits: `note` and `channel` are read in `process`, so a changed `note` takes effect one block (1.3 ms) later (a note in the first block of a new module is kept and checked against the params then). 32 notes per module per block; more are dropped.

## The voices

Decay times are the time to fall 60 dB. Each envelope is one multiply per sample. A voice stops processing once it has fallen below -80 dB (relative to its drive) and writes zeros.

- **Kick.** A sine whose frequency falls from `tune * 2^(sweep_st/12)` to `tune` with a time of `sweep_ms` (frequency decays exponentially, as the 808's bridged-T does), times an exponential body (`decay_ms`). `click` adds a 2.5 ms noise burst at the start (the beater), `tone` is a one-pole low-pass (350 Hz to 17 kHz), `drive_db` a `tanh` stage normalised to unit peak (weight and harmonics, same peak).
- **Tom.** The kick's core with tom ranges and defaults (60 to 500 Hz, small sweep).
- **Snare.** Two sines (shell tone and an inharmonic 1.78x partial) with a short pitch drop, plus white noise high-passed at `tone` (1.2 to 7.2 kHz) and rolled off at three times that. `snappy` crossfades equal-power from the drum (0) to the noise (1). `decay_ms` is the rattle; the body decays in 40 % of it. `drive_db` as for the kick.
- **Clap.** White noise through a band-pass (`tone`, 600 Hz to 3 kHz, Q about 1.7). The envelope is `bursts - 1` short bursts, each falling to 5 % in `spread_ms`, then a tail of `decay_ms`. Bounded with `tanh`.
- **Hat.** Six square waves at the 808's frequencies (205.3, 304.4, 369.6, 522.7, 540 and 800 Hz, shifted by `tune_st`), summed and high-passed at `tone` (4.5 to 11.7 kHz). *Why squares and not FM or filtered noise:* the six detuned squares give the inharmonic, metallic lattice a real 808 hat has, and one voice is six phase accumulators; FM needs tuned ratio pairs to sound as dense, and noise through filters is a shaker. The squares use PolyBLEP, so they do not fold back at 44.1 kHz. `closed_ms` / `open_ms` set the two decays.

## Faces and displays

Faces use the current layout: display above the knobs, jacks lower. The display is the amplitude envelope of a hit on a time axis (about half of the 60 dB time, so the audible part), drawn from the module's own parameters, and right with audio off:

- kick, tom: the body envelope filled, the pitch curve dashed;
- snare: the rattle filled, the drum tone as a line (their balance follows `snappy`);
- clap: the bursts and the tail;
- hat: the closed and the open decay, the bright one being the hit that last played.

While audio runs a playhead rides the curve at the time since the last hit (`Module::view().position`, from the face taps). Low zoom follows the other displays: frame only below 45 %, plain trace to 70 %, full from 70 %. `midi.note` has no display (a note-name readout on its `note` knob).

## Demos

- `sound-engines/demos/four-floor` (Four On The Floor): five sequencers on one clock at 124 bpm. Kick on every beat, snare and clap on the second beat, open hats on the off-beats cut off by closed hats. Bank B adds a snare ghost note and a tom fill (the factory render launches bank B, so both banks are level-checked).
- `sound-engines/demos/pump` (Kick Pump): a saw pad (two oscillators, a fifth apart, three unison voices each) through a ladder filter into the compressor; the kick feeds the compressor's `sc` jack. Threshold -34 dB, ratio 12, attack 1 ms, release 220 ms. Lower the threshold or raise the ratio for harder pumping.

Both follow the factory level rule: four-floor -18.7 dBFS loudness, pump -18.8 (peak -4.4 and -6.0). `crates/ui/tests/drum_demos.rs` builds both through `PatchEditor`; `write_patches` rewrites them.

## Tests

- `crates/modules/tests/drums.rs`: level and exact silence at the end of a hit, starts on the sample of the edge (edge at 1, 37, 63, 64, 100), block size 1, 7, 64 and 128 render the same, retrigger steps no more than a fresh hit, velocity and accent, unpatched velocity, hat choke, every param at min and max at 44.1, 48, 96 and 192 kHz and blocks 1, 64 and 333 stay finite and bounded, NaN and huge inputs, no allocation on the audio path; `midi.note` edge, velocity, channel, retrigger gap, pulse length, queue limits.
- `crates/engine/tests/drums.rs`: MIDI notes through the real engine start the right voice on the sample, velocity travels with the note, a `midi.in` patch is undisturbed, the kick ducks a pad through the sidechain, no allocation, deterministic render.
- `crates/ui/tests/drum_demos.rs`: committed demos equal the builders, the pump ducks the pad by at least 8 dB more than the same patch with ratio 1, the groove has a hit on every beat.

## Cost

`cargo run --release -p kabl-modules --example bench_drums` (one voice, 48 kHz, median of 15 runs of 2 s):

| Voice | idle | retriggered every 62 ms (never stops) |
|---|---|---|
| Kick | 3.3 ns/sample (0.016 % core) | 90.6 (0.44 %) |
| Snare | 3.7 (0.018 %) | 46.1 (0.22 %) |
| Clap | 3.3 (0.016 %) | 15.6 (0.075 %) |
| Hat | 4.6 (0.022 %) | 47.2 (0.23 %) |
| Tom | 3.8 (0.018 %) | 85.8 (0.41 %) |

The kick and tom pay for a `sin` and a `tanh` per sample.

## Clips and captures (`media/`)

Clips (`*.mp3`, each hit loud then soft; made by `cargo test -p kabl-engine --test drum_clips -- --ignored`, played through `midi.note`):

- `kick.mp3`, six settings: default; 38 Hz long boom; 62 Hz tight with a fast fall; bright click and tone; 24 dB drive (fat, buzzy); no sweep, click or drive (a plain sine thump). Listen for the pitch drop at the front of each hit.
- `snare.mp3`, five: default; Snappy 0 (drum only); Snappy 1 with a bright Tone (hiss only); low 140 Hz with a 600 ms rattle; high 260 Hz, short, 20 dB drive. Listen for the balance of drum and rattle.
- `clap.mp3`, five: default four bursts; two bursts 5 ms apart (one snap); six bursts 14 ms apart (a crowd); a long dark tail; a short bright one. Listen for the flutter before the tail.
- `tom.mp3`, four tunings from 90 to 330 Hz. Listen for the small pitch drop.
- `hat.mp3`: closed (loud, soft); open; open cut off by a closed hit 0.35 s later (the choke); tuned down and dark, open then closed; tuned up and bright, three short closed hits.
- `groove-four-floor.mp3`: the groove demo, bank A, 16 s. `pumping-kick-sidechain.mp3`: the pump demo; listen for the pad dipping on every kick and swelling back between.

Captures: `A-` (Satin) and `B-` (Lacquer), `light` and `dark`: `*-all50.webp` is the groove patch at 50 %, then a close-up per voice. Made by `scripts/capture.py`. The REAPER check is `scripts/host_drums.py`; its result is in `evidence/host/result.json`: in REAPER 7.75, a MIDI item on a track holding the plugin starts every drum on exactly the sample of its note (first sample offset 0 for all eight hits, kick velocity 127 against 30 peaks 0.166 against 0.090), the saved project reloads to an identical state, and its render is bit-identical.

## Check it yourself

```
cargo test -p kabl-modules --test drums
cargo test -p kabl-engine --test drums
cargo test -p kabl-ui --test drum_demos
cargo test -p kabl-engine --test drum_clips -- --ignored    # target/drum-clips/*.wav
cargo test -p kabl-ui --test drum_demos write_renders -- --ignored   # target/drum-demos/*.wav
cargo build --release -p kabl-ui --bin kabl-ui && target/release/kabl-ui --patch patches/sound-engines/demos/four-floor
```
