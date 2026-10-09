# Sound engines: FM and wavetable

Two sound sources for the rack, built on the existing module contract: `osc.fm` (a sine
operator for phase-modulation FM) and `osc.wt` (a band-limited wavetable oscillator). Both are
voice-rate, take `pitch` like `osc.va`, and appear in the add-module palette and in the CLAP
plugin without further wiring. Status: engineering verified (below); **listening is
Kosta's call**, nothing here has been heard by a person.

Decisions and reasons: `docs/decisions.md`, "Sound engines: FM and wavetable". Factory table
sources and licences: `crates/modules/assets/wavetables/PROVENANCE.md`.

## osc.fm: the operator

`out = level * sin(2π (phase + (index * pm + feedback) / 2π))`. Wire one operator's `out` to
another's `pm` and the second is a carrier whose phase the first bends. Operators branch, stack
and loop as the patch needs; there is no fixed algorithm list.

| | |
|---|---|
| ports | `pitch` in (semitones, as `osc.va`), `pm` in (audio), `out` |
| `ratio` | 0 = ×0.5, then ×1 to ×32 (stepped) |
| `fine` | ±600 cents on the ratio: ×4 − 231 ct = ×3.5, so inharmonic ratios are one knob from an integer |
| `level` | output amplitude. On a modulator it sets how far the carrier is bent: put an envelope here |
| `index` | radians of phase deviation per unit of `pm` (0 to 16) |
| `feedback` | the operator's last two outputs back into its phase, up to 2 rad |
| `base_hz` | frequency at pitch 0, C4 = 261.63 Hz (advanced) |

`level`, `index` and `feedback` are smoothed per sample across each block, so a block-rate cable
(an envelope on `level`) never zippers.

### Aliasing, measured

High-index FM puts energy far past Nyquist, and it folds back as inharmonic hash. The operator
therefore runs at twice the sample rate internally: the `pm` input is interpolated
(6-point Lagrange) and the output passes a 33-tap Kaiser low-pass FIR (17.5 kHz passband, over
70 dB down from 30.5 kHz at 48 kHz). Measured as the power of everything that is not a harmonic
of the carrier below 20 kHz, relative to the harmonic power (`print_alias_table`, 48 kHz):

| carrier Hz | modulator Hz | index | feedback | plain rate | 2× (shipped) |
|---|---|---|---|---|---|
| 440 | 440 | 8 | 0 | −93 dB | −93 dB |
| 997 | 997 | 8 | 0 | −94 dB | −94 dB |
| 2114 | 2114 | 8 | 0 | −48 dB | −90 dB |
| 1471 | 2942 | 5 | 0 | −55 dB | −95 dB |
| 3331 | 3331 | 10 | 0 | −5 dB | −67 dB |
| 884 | 2651 | 12 | 0 | −3 dB | −36 dB |
| 2114 | 2114 | 16 | 0 | −2 dB | −47 dB |
| 440 | 440 | 0 | 1 | −94 dB | −94 dB |
| 1760 | 1760 | 0 | 0.5 | −73 dB | −81 dB |
| 1760 | 1760 | 0 | 1 | −24 dB | −50 dB |

−93 dB is the measurement floor. Without oversampling, an index of 8 on a 2 kHz carrier is
already audible (−48 dB) and index 10 on 3.3 kHz is noise. With it, ordinary FM is clean; what
remains is the extreme: the highest sideband is about (index + 1) × modulator + carrier, and
anything past about 36 kHz still folds back (rows 6 and 7, −36 to −47 dB). Four times the rate
would cover those and costs another doubling; it is not done. Feedback is capped at 2 rad
because 3 rad was already −37 dB at 440 Hz.

### Phase alignment

Oversampling delays a signal by 10 samples per operator (2 for the `pm` interpolation, 8 for the
filter). Left alone, a modulator and carrier would meet at a phase that depends on pitch, so the
spectrum of the most common stack, 1:1, would change across the keyboard and gain a DC offset (the
first tine-keys render had −0.07, which this found). A modulated operator therefore runs its own
phase 10 samples behind, and `fm.rs` checks that a 1:1 pair matches the ideal zero-latency
spectrum (harmonic *n* of sin(t + I sin t) is |J(n−1)(I) + (−1)ⁿ J(n+1)(I)|) at five pitches with
no DC. This is exact for one modulator into one carrier. Each further hop in a chain
(modulator → modulator → carrier) is off by about 8 samples; the effect is a slightly different
timbre at high pitches, not a defect in the sound's character.

## osc.wt: the wavetable oscillator

| | |
|---|---|
| ports | `pitch` in, `pos` in (CV, audio rate), `out` |
| `table` | which of the 8 factory tables |
| `position` | where in the table, 0 to 1, linear between frames |
| `pos_mod` | how far the `pos` input moves `position` (1 = whole table) |
| `user` | 0 = the factory `table`; 1 to 8 = the patch's embedded table in that slot |
| `fine`, `base_hz` | as `osc.va` |

A table is 1 to 64 frames of one cycle (2048 samples, 512 harmonics at most). Each frame becomes
ten mipmaps, `512 >> k` harmonics, stored at 16 samples per top harmonic. The oscillator reads
the richest level whose top harmonic stays under Nyquist at the block's highest pitch, so a
table cannot alias by being too bright. A full-bandwidth saw (512 harmonics) measured at 29
notes from MIDI 24 to 108: worst non-harmonic power −66.8 dB (523 Hz). Block-rate cables into
`position` ramp across the block; the `pos` input is per sample.

Factory tables (8, 16 to 32 frames): Saw to Square, Harmonic Sweep, Vowels, Glass Bell, Digital
Hollow, Electric Piano, Organ, Choir. Five are computed by `build.py`; three are sampled from
Adventure Kid Waveforms (CC0). See PROVENANCE.md.

### Importing a table

Everything below runs on a control thread, never the audio thread.

- Any .wav (PCM 8/16/24/32, float 32/64, mono or stereo, extensible): a length that is a
  multiple of 2048 samples is that many frames (up to 64); any other length is one cycle,
  resampled; a Serum-style `clm ` chunk names another frame length. The result is band-limited,
  DC-free and peak-normalised, stored as a 16-bit mono .wav.
- From a script: `cargo run --release -p kabl-engine --example wavetable_import -- PATCH_DIR 1 my.wav`
  embeds it in slot 1 of a saved patch; a `osc.wt` with `user` = 1 plays it.
- From code: `PatchEditor::import_table(slot, name, bytes)`, one undo step. **There is no
  button for it in the interface yet**; that belongs to the interface redesign, with the table
  editor and display.
- Errors name the cause: not a .wav, cut short, unsupported format, fewer than 16 samples, more
  than 64 frames, silent, NaN or infinite samples, larger than 32 MB, or a sound that would
  exceed the 2 MiB save limit.

### Tables travel with the patch

A user table is stored in the patch itself (`PatchState.tables`, `Op::SetTable`, file schema 5),
as base64 of its canonical .wav, using the same embed-bytes-in-the-document approach as the
panel artwork (`kabl_core::table` copies `Artwork`'s name rule and size limit; names are never
paths). Save/load, undo, comparison and the CLAP state all carry it; the host state is version 4
when a table is present, so older builds refuse the state instead of silently dropping the
table. Deleting the original .wav changes nothing. An empty or corrupt slot plays the factory
table instead of silence.

## Demo patches and clips

`patches/sound-engines/demos/*`, built by `crates/ui/tests/sound_engines.rs`; audio in
`docs/sound-engines/audio/` (AAC, rendered offline by the same engine the app runs, 8 voices).
The demos sit one level deeper than the factory browser scans (`patches/sound-engines/demos/`), so
the curated factory bank and its tests are unchanged; making them factory sounds needs `FACTORY`
and `INVENTORY` entries with guides, which is Kosta's decision. All four need a keyboard. Launch with
`cargo run --release -p kabl-ui -- --patch patches/sound-engines/demos/NAME`.

| patch | clip | what it shows | old palette could not |
|---|---|---|---|
| `tine-keys` | tine-keys.m4a (10 s) | FM electric piano: body (×1) and tine (×14) operators with their own envelopes bend one carrier; velocity brightens | a tine attack that decays faster than the body |
| `glass-bells` | glass-bells.m4a (13 s) | two inharmonic FM pairs, 1 : 3.5 and 2 : 5.04, ringing out on their own | inharmonic partials with their own decays |
| `vowel-drift` | vowel-drift.m4a (16 s) | Vowels and Choir tables detuned ±7 cents, each swept by its own slow LFO at audio rate | morphing between recorded cycles |
| `imported-morph` | imported-morph.m4a (10 s) | a .wav imported into the patch (a resonance climbing 24 frames), position swept by the envelope and velocity, over an FM sub with feedback | a user-supplied timbre that moves per note |

Clip levels peak at 0.73 to 0.85. Spectral checks only (no listening): no DC, no NaN, no sample step over 0.43 of full scale; tine-keys averages 1.5 to 2 kHz spectral centroid on chords and
about 4 kHz on its top-register melody; glass-bells starts around 6 kHz and decays to under 1
kHz. The mix of gains, chorus and reverb is a starting point, not a judgement.

## Cost per voice

48 kHz, 64-sample blocks, i7-13700H pinned with `taskset -c 2`, release build
(`bench_sources`, `bench_patches`; also in `docs/benchmarks.md`).

| source, one voice | ns per sample | share of one core |
|---|---|---|
| `osc.va` saw (reference) | 12.9 | 0.06 % |
| `osc.wt` | 13.5 to 14.4 | 0.07 % |
| `osc.fm` at the plain rate | 22.3 | 0.11 % |
| `osc.fm` shipped (2×) | 43.0 | 0.21 % |
| two-operator stack | 82.4 | 0.40 % |
| four-operator chain | 160.9 | 0.77 % |

Whole demo patches with eight voices sounding, median per 64-sample block (budget 1333 µs):
tine-keys 115 µs (8.6 %), glass-bells 235 µs (17.7 %, a 7 s reverb and four operators per voice),
vowel-drift 113 µs, imported-morph 72 µs; the palette's strings 126 µs and pad 135 µs for
comparison.

## Verified in the CLAP plugin and a real host (2026-10-09)

Release plugin from PR head (sha256 `5d1e2a24…58e6`), CLAP validator 0.4.1, REAPER 7.75 on a
private Xvfb display and profile, PipeWire. Scripts: `docs/sound-engines/scripts/host.py` and
`host.lua` (the method of `docs/factory-performance-bank/scripts/host.py` and
`docs/host-production/scripts/render.lua`), `validator_diff.py`. Evidence:
`docs/sound-engines/evidence/` (`validator.json`, `host/result.json`, the saved projects, the
mapped-plugin paths, REAPER's FX listing).

- **Validator:** 35 success, 9 skipped, no failures. The same 44 tests as the recorded baseline
  (`docs/reaper-instrument/evidence/validator-final.json`), no status changed.
- **New project, save, full quit, reopen:** a two-track project, track 1 the imported-morph demo
  (`osc.wt` on a user table + `osc.fm`), track 2 tine-keys (`osc.fm`). The table was imported from
  a real .wav with the `wavetable_import` example and the .wav deleted before REAPER started. The
  plugin loaded on both tracks. After REAPER saved, quit completely and reopened what it had
  saved, the plugin states were equal (float32-normalised JSON) at all three points: written,
  saved, saved after reopen. State 1 is version 4 and carries the table `host-sweep.wav` in slot 1;
  state 2 is version 2.
- **Renders:** 24 s offline master-mix renders by REAPER: finite, active every second, peak
  0.258, RMS 0.073. The render after the reopen is bit-identical to the one before it. The same
  project with the table removed from the state differs from it by 66 % of RMS (spectrum cosine
  0.90), so the imported table is what plays in the host.
- **Older project:** the two-track project the factory-bank batch saved with the previous build
  (version 2 states, automation envelopes). Opened by this build, its saved plugin state equals
  the original, and its 24 s render is bit-identical to the render by the baseline plugin (the
  binary recorded in `docs/factory-performance-bank/evidence/host/recall.json`, sha256 `80933e3d…2b57`,
  `crates/` identical to origin/master) and to a second baseline render.

## What is not verified

- How anything sounds. Every number above is a measurement; none is a judgement. The renders
  are not listening.
- Physical latency, xruns, a controller, a host other than REAPER, the editor (the host runs
  had it closed; there is no table UI to try).
- Three-operator chains and deeper keep a small pitch-dependent phase error (above).
- Hardware (Pi 4) cost.
