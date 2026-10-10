# Sound engines: FM and wavetable

Three sound sources for the rack, built on the existing module contract: `osc.fm` (a sine
operator for phase-modulation FM), `osc.fm6` (a six-operator FM voice with eight algorithms and
an envelope per operator) and `osc.wt` (a band-limited wavetable oscillator). All are
voice-rate, take `pitch` like `osc.va`, and appear in the add-module palette and in the CLAP
plugin without further wiring. Status: engineering verified (below); **listening is
Kosta's call**, nothing here has been heard by a person.

The utility modules (quantizer, sample and hold, slew, logic, random and more) are in
`docs/sound-engines/utilities.md`.
The arpeggiator, clock swing and sequencer ratchets are in `docs/sound-engines/rhythm.md`.

Decisions and reasons: `docs/decisions.md`, "Sound engines: FM and wavetable" and "Sound
engines 2: six-operator FM, 4x, display data". Factory table
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

| carrier Hz | modulator Hz | index | feedback | plain rate | 2× (default) | 4× (opt-in) |
|---|---|---|---|---|---|---|
| 440 | 440 | 8 | 0 | −93 dB | −93 dB | −93 dB |
| 997 | 997 | 8 | 0 | −94 dB | −94 dB | −94 dB |
| 2114 | 2114 | 8 | 0 | −48 dB | −90 dB | −90 dB |
| 1471 | 2942 | 5 | 0 | −55 dB | −95 dB | −95 dB |
| 3331 | 3331 | 10 | 0 | −5 dB | −67 dB | −67 dB |
| 884 | 2651 | 12 | 0 | −3 dB | −36 dB | −36 dB |
| 2114 | 2114 | 16 | 0 | −2 dB | −47 dB | −47 dB |
| 440 | 440 | 0 | 1 | −94 dB | −94 dB | −88 dB |
| 1760 | 1760 | 0 | 0.5 | −73 dB | −81 dB | −74 dB |
| 1760 | 1760 | 0 | 1 | −24 dB | −50 dB | −58 dB |

−93 dB is the measurement floor. Without oversampling, an index of 8 on a 2 kHz carrier is
already audible (−48 dB) and index 10 on 3.3 kHz is noise. With it, ordinary FM is clean; what
remains is the extreme: the highest sideband is about (index + 1) × modulator + carrier, and
anything past about 36 kHz still folds back (rows 6 and 7, −36 to −47 dB). Feedback is capped
at 2 rad because 3 rad was already −37 dB at 440 Hz.

**4× is an opt-in (`oversample` = 4X) and helps less than expected.** In a modulator → carrier
pair the floor is not folding but the 6-point interpolation of the `pm` input, which arrives at
the plain rate whatever the internal rate: rows 6 and 7 are identical at 4×. Where the energy is
generated inside the operator it does help: full feedback at 1.76 kHz goes from −50 to −58 dB,
and operator 6 alone with full feedback at 3.1 kHz (osc.fm6) from −41 to −52 dB. The two
smaller feedback rows get slightly worse (+6 to +7 dB, still under −74 dB): the 4× filter is
sharper, and the feedback average spans a shorter time. 4× costs about twice the 2× path
(Cost per voice). Use it on feedback-heavy or very bright single voices, not as a default.

### Phase alignment

Oversampling delays a signal by 10 samples per operator (2 for the `pm` interpolation, 8 for the
filter). Left alone, a modulator and carrier would meet at a phase that depends on pitch, so the
spectrum of the most common stack, 1:1, would change across the keyboard and gain a DC offset (the
first tine-keys render had −0.07, which this found). A modulated operator therefore runs its own
phase 10 samples behind, and `fm.rs` checks that a 1:1 pair matches the ideal zero-latency
spectrum (harmonic *n* of sin(t + I sin t) is |J(n−1)(I) + (−1)ⁿ J(n+1)(I)|) at five pitches with
no DC. This is exact for one modulator into one carrier. Each further hop in a chain
(modulator → modulator → carrier) is off by about 8 samples; the effect is a slightly different
timbre at high pitches, not a defect in the sound's character. **For chains of three or more,
use `osc.fm6`**: all six operators run inside one module at the internal rate and meet with no
latency at all.

## osc.fm6: six operators in one module

`out = Σ over carriers of o_k / √(carriers)`, `o_k = amp_k · sin(2π (phase_k + (index · 8 ·
Σ o_j of its modulators + feedback_k) / 2π))`, `amp_k = level_k · env_k · velocity factor`.
Operator numbers run 1 to 6; a modulator always has a higher number than its target. Operator 6
can feed back on itself. A rising `gate` restarts every phase at 0 (key sync, as a DX does), so
a note always starts with the same spectrum.

| | |
|---|---|
| ports | `pitch`, `gate`, `velocity` in; `out` |
| `algorithm` | 0 to 7, who modulates whom (table below) |
| `ratioN`, `fineN` | per operator, as `osc.fm`: ×0.5, ×1 to ×32 stepped, ±600 cents |
| `levelN` | operator amplitude; on a modulator, how far it bends its targets |
| `attackN`, `decayN`, `sustainN`, `releaseN` | per-operator ADSR (0.1 ms to 10 s); no envelope modules needed |
| `velN` | how much the `velocity` input scales this operator (0 ignores it) |
| `index` | global modulation depth (0 to 2; 1 = 8 rad for a full-level modulator) |
| `feedback` | operator 6 into itself, up to 2 rad |
| `oversample` | 2X (default) or 4X |
| `base_hz`, `fine` | tuning (advanced) |

| algorithm | routing | heard |
|---|---|---|
| 0 | 6>5>4>3>2>1 | 1 |
| 1 | 2>1, 4>3, 6>5 | 1, 3, 5 |
| 2 | 3>2>1, 6>5>4 | 1, 4 |
| 3 | 2>1, 6>5>4>3 | 1, 3 |
| 4 | 2, 3, 4, 5, 6 all into 1 | 1 |
| 5 | 6 into each of 1 to 5 | 1 to 5 |
| 6 | 6>4>2>1, 5>3>1 | 1 |
| 7 | none (six sines added) | 1 to 6 |

`level` and `index` and `feedback` are smoothed per sample across each block, like `osc.fm`.
`algorithm` and `oversample` change the structure and recompile. A finished voice costs nothing
(an operator below −120 dB skips its sine; after 80 silent samples the voice outputs exact zero).

### Why one module, and what was measured

Six chained `osc.fm` modules keep a 10-sample delay per hop (above), so a chain of three or
more has a pitch-dependent timbre error. Inside `osc.fm6` there is no hop: the six operators
share one internal clock and are filtered once. `tests/fm6.rs` evaluates all eight algorithms
independently in f64 (the equations above, written from the algorithm labels) and compares the
module's output at 130.8, 523.3 and 1046.5 Hz, at 2× and 4×, with key sync, aligned by the
filter's known delay (7.5 output samples at 2×, 7.25 at 4×): the worst difference over 4096
samples is 0.003 of a full-scale sine (limit asserted 0.005), including depth-6 chains. This
is the analytic-spectrum test for chains deeper than two operators.

Non-harmonic power below 20 kHz (as above) for chains of equal ratio-1 operators, level 1
(`print_alias_table` in `tests/fm6.rs`):

| chain depth | Hz | index | 2× | 4× |
|---|---|---|---|---|
| 2 | 440 | 0.25 | −93 dB | −93 dB |
| 2 | 2093 | 1.0 | −86 dB | −86 dB |
| 3 | 1047 | 0.35 | −89 dB | −89 dB |
| 4 | 523 | 0.3 | −89 dB | −89 dB |
| 4 | 1047 | 0.3 | −60 dB | −60 dB |
| 6 | 523 | 0.2 | −77 dB | −77 dB |
| 6 | 1047 | 0.2 | −55 dB | −63 dB |
| 1 (op 6 alone, feedback 1) | 440 | 0 | −94 dB | −88 dB |
| 1 (op 6 alone, feedback 0.5) | 1760 | 0 | −81 dB | −74 dB |
| 1 (op 6 alone, feedback 1) | 1760 | 0 | −50 dB | −58 dB |
| 1 (op 6 alone, feedback 1) | 3136 | 0 | −41 dB | −52 dB |

Where 2× and 4× agree the floor is the 24 kHz output filter's transition band, not folding at
the internal rate; 4× pays off only for energy past 48 kHz: deep chains at high pitch and
feedback. The default stays 2×.

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

## Display data for the interface

`Module::view(&self, &mut ModuleView)` (crates/modules/src/view.rs) lets an oscillator say what
an interface can draw. `osc.wt`, `osc.fm` and `osc.fm6` implement it from a 4096-sample ring of
their own output; every other module leaves it at the default (`valid = false`).

| field | meaning |
|---|---|
| `valid` | the voice has played (a gated `osc.fm6` is invalid until its first note) |
| `position` | `osc.wt`: where in the table the voice reads, 0 to 1 (`position` plus the `pos` input); 0 for the others |
| `cycle` | 128 points: one period of the output, from the newest rising zero crossing that leaves a whole period (else the last period), linearly interpolated |

The engine fills it into `ProbeReport.view` (the signal-inspection report) from the voice that
was loudest in the measured window, once per report. It is a copy of fixed-size data on the
audio thread: no allocation, no lock (`probe.rs` checks it inside `assert_no_alloc`). The
interface needs nothing else for a table display: `wavetable::factory_frames(i)` returns the
raw frames of factory table `i` (control thread, it decodes), and a user table's frames come from
`wavetable::decode_canonical(&table.wav)`. Draw the frame pair around `position`, and `cycle`
for the live shape.

## Demo patches and clips

`patches/sound-engines/demos/*`, built by `crates/ui/tests/sound_engines.rs`; audio in
`docs/sound-engines/audio/` (AAC, rendered offline by the same engine the app runs, 8 voices).
The demos sit one level deeper than the factory browser scans (`patches/sound-engines/demos/`), so
the curated factory bank and its tests are unchanged; making them factory sounds needs `FACTORY`
and `INVENTORY` entries with guides, which is Kosta's decision. All eight need a keyboard. Launch with
`cargo run --release -p kabl-ui -- --patch patches/sound-engines/demos/NAME`.

| patch | clip | what it shows | old palette could not |
|---|---|---|---|
| `tine-keys` | tine-keys.m4a (10 s) | FM electric piano: body (×1) and tine (×14) operators with their own envelopes bend one carrier; velocity brightens | a tine attack that decays faster than the body |
| `glass-bells` | glass-bells.m4a (13 s) | two inharmonic FM pairs, 1 : 3.5 and 2 : 5.04, ringing out on their own | inharmonic partials with their own decays |
| `vowel-drift` | vowel-drift.m4a (16 s) | Vowels and Choir tables detuned ±7 cents, each swept by its own slow LFO at audio rate | morphing between recorded cycles |
| `imported-morph` | imported-morph.m4a (10 s) | a .wav imported into the patch (a resonance climbing 24 frames), position swept by the envelope and velocity, over an FM sub with feedback | a user-supplied timbre that moves per note |
| `dx-keys` | dx-keys.m4a (10 s) | `osc.fm6` electric piano, algorithm 1: a mellow ×1 body pair, a ×14 tine pair gone in 90 ms, a detuned shimmer pair; per-operator envelope and velocity, no envelope modules; same phrase as tine-keys | six envelopes in one voice without twelve modules |
| `iron-bell` | iron-bell.m4a (13 s) | `osc.fm6` bell, algorithm 1: three inharmonic pairs (1 : 3.5, 2 : 5.04, 3.1 : 8.9) that ring 4 to 7 s and darken as the modulators fade; same phrase as glass-bells | a six-partial bell from one module |
| `rising-pad` | rising-pad.m4a (16 s) | Glass Bell and Digital Hollow tables, positions swept open by the note's own envelope (3.2 s attack), plus a filter that opens with it; same chords as vowel-drift | a chord that opens from a dull tone to a bright one by itself |
| `rust-bass` | rust-bass.m4a (10 s) | `osc.fm6` bass, algorithm 3: a punchy ×1 pair (bend gone in 200 ms) beside a four-operator chain with feedback on operator 6; velocity adds growl; same phrase as imported-morph | a bass that attacks clean then growls |

Clip levels peak at 0.70 to 0.91 (dx-keys 0.91, iron-bell 0.81, rising-pad 0.70, rust-bass 0.55). Spectral checks only (no listening): no DC, no NaN, no sample step over 0.43 of full scale; tine-keys averages 1.5 to 2 kHz spectral centroid on chords and
about 4 kHz on its top-register melody; glass-bells starts around 6 kHz and decays to under 1
kHz. The mix of gains, chorus and reverb is a starting point, not a judgement. For the four newer
demos only spectra and levels were checked: rising-pad's spectral centroid climbs from 0.8 kHz
(first second) to 2.4 kHz (fifth) as its envelope opens the tables; rust-bass's strongest line
is at 65.0 Hz for the 65.4 Hz note (MIDI 36) and its centroid stays near 70 Hz; iron-bell's level
is 0.16 → 0.11 → 0.15 → 0.08 over the first four seconds (notes overlap) and its centroid
falls from 1.7 to 0.9 kHz between notes.

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

A second run for the multi-operator work (same machine, but a different time of day: the
`osc.va` reference measured 9.2 ns, not 12.9, so compare rows inside one table only; the machine
was under heavy load from other jobs, so the table is the minimum of three runs pinned to
different cores, two of which agreed within 4 %):

| source, one voice | ns per sample | share of one core |
|---|---|---|
| `osc.va` saw (reference) | 9.2 | 0.04 % |
| `osc.wt` | 10.8 to 11.8 | 0.05 % |
| `osc.fm` plain rate | 17.2 | 0.08 % |
| `osc.fm` 2× (default) | 33.0 | 0.16 % |
| `osc.fm` 4× | 69.7 | 0.33 % |
| four-operator chain of `osc.fm`, 2× | 132.3 | 0.64 % |
| `osc.fm6`, 1 operator sounding | 50.1 | 0.24 % |
| `osc.fm6`, 2 operators (default patch) | 63.0 | 0.30 % |
| `osc.fm6`, 4-operator chain | 95.6 | 0.46 % |
| `osc.fm6`, 6-operator chain, 2× | 135.7 | 0.65 % |
| `osc.fm6`, 6-operator chain, 4× | 259.1 | 1.24 % |

`osc.fm6` has a fixed cost (the filter and the envelopes, about 40 ns) plus about 17 ns per
sounding operator; silent operators cost nothing. Six chained `osc.fm` modules would cost about
200 ns and be wrong beyond two hops; one `osc.fm6` costs 136 ns and is not. At 8 voices a 6-operator
2× voice is about 5 % of one core, 4× about 10 %.

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

### The multi-operator work (2026-10-09, second pass)

Release plugin from this branch (sha256 `be738d7e…d59b`), the same validator, REAPER and method.
Script `docs/sound-engines/scripts/host_fm6.py`; evidence `docs/sound-engines/evidence/host-fm6/`
and `evidence/validator-fm6.json`.

- **Validator:** 35 success, 9 skipped, no failures; the same 44 tests as PR #17's recorded run,
  no status changed (`validator_diff.py`).
- **Three-track project, save, full quit, reopen:** track 1 dx-keys (`osc.fm6`, 2×), track 2
  rust-bass with `oversample` set to 4× (`osc.fm6`), track 3 rising-pad (`osc.wt` with an envelope
  on its position). The plugin loaded on all three tracks. After REAPER saved, quit completely and
  reopened what it had saved, the plugin states were equal (float32-normalised JSON) at all three
  points. The 24 s master render is finite, active every second, peak 0.265, RMS 0.061 (mean
  0.0017), and the render after the reopen is bit-identical to the one before. The same project
  with the three states replaced by the pad differs from it by 104 % of RMS (spectrum cosine
  0.50), so the `osc.fm6` tracks are what plays in the host.
- **Older project:** the factory-bank project again: its saved plugin state equals the original,
  and the 24 s render by this build is bit-identical to the baseline plugin's (sha256
  `80933e3d…2b57`) and to a second baseline render. PR #17's own patches are also pinned by
  `golden_renders.rs`, which hashes the offline renders of all 26 committed patches against
  hashes recorded with PR #17's build.

## What is not verified

- How anything sounds. Every number above is a measurement; none is a judgement. The renders
  are not listening.
- Physical latency, xruns, a controller, a host other than REAPER, the editor (the host runs
  had it closed; there is no table UI to try).
- Chains of `osc.fm` modules deeper than two keep their small pitch-dependent phase error
  (above); use `osc.fm6`.
- The new demos have been rendered and measured, not heard; the host run played the `osc.fm6`
  and sweep tracks but is not listening.
- The new help text and selector labels in the editor were checked by the unit tests only, not
  by opening the editor (nothing draws the table or cycle yet).
- Hardware (Pi 4) cost.
