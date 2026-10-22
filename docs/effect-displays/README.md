# Effect and filter displays

Live displays on the faces of the nine modules added with the effects batch (PR #25), in the layout of the older modules: display above the knobs, jacks lower. They are drawn from the module's parameters (right with audio off) and moved by the engine's face taps while audio runs and the face is on screen.

| Module | Display | Moves with |
|---|---|---|
| `eq` | Combined response of the four bands (shelves and bells) on a 20 Hz to 20 kHz log axis, +-14 dB, a ring on each band | Modulated frequency, gain and Q (routes with a live source). The curve uses the module's own coefficients (`band_response_db`). |
| `compressor` | Transfer curve (input to output dB, threshold line, soft knee, makeup) and a gain-reduction bar with the dB value | The `gr` output (largest reduction of the window), modulated threshold, ratio, knee, makeup |
| `limiter` | Transfer curve (gain, then the ceiling) and a gain-reduction bar | The module's gain reduction (`Module::view`, lowest gain over the last block) |
| `phaser` | Response of the all-pass chain with feedback and mix at the LFO's position, the two ends of the sweep faint behind | LFO phase (`Module::view`), modulated depth, feedback, mix |
| `flanger` | The comb at the current delay, the two ends of the sweep faint behind | LFO phase (`Module::view`), modulated depth, feedback, mix |
| `filter.comb` | Response (peaks or notches by feedback sign, damping), marker at the fundamental | Modulated parameters and the cable into `pitch` |
| `filter.formant` | Sum of the five formant band-passes, vowel letter, first two formants marked | Modulated vowel, shift, q |
| `bitcrusher` | One wave as the module would hand it on: rounded to the bit depth, held for the downsample ratio, the original faint | Parameters only (no live data; the picture is the effect) |
| `filter.svf` | Existing response display now draws the notch curve (low-pass plus high-pass) and treats a patched `notch` as the bright one | As before |

Every module got a display. Where a picture is approximate: the phaser, flanger and comb curves are the small-signal transfer functions (the `tanh` in the feedback paths and the comb's damping phase are left out); the bitcrusher shows a fixed 48-sample sine, so the downsample ratio is relative to that, not to the sample rate; a dense comb (low pitch) is drawn as the envelope of its peaks, not every peak.

Low zoom follows the existing displays: frame only below 45 %, plain trace to 70 %, full from 70 % (ghost curves, rings, captions).

## Engine changes

- `Module::view` on `phaser` and `flanger` (LFO phase) and `limiter` (gain reduction in dB): each module keeps one `f32` it already had or one added field, nothing allocates.
- `compressor` is read on its `gr` output (output 2). The engine reads one output per module instance, so the compressor's own display takes that slot.
- Exports so faces draw the module's own maths: `band_response_db` (eq), `formant_bands` (formant; the module's design now uses it), `compressor_reduction` (the gain computer). No sound changed.
- Tests: `crates/modules/tests/view.rs` (limiter reduction, LFO phase, EQ curve against a rendered sine, formant table), `crates/engine/tests/facetap.rs` (reports for the new views, identical audio with taps on and off, no allocation).

The compressor never reduced gain in a running patch before PR #34 (unpatched inputs reached it as zero buffers); its display needs that fix.

## Check it yourself

```
cargo build --release -p kabl-ui --bin kabl-ui
python3 docs/effect-displays/scripts/make_patch.py scratch/ed-patch    # all nine, self-playing
target/release/kabl-ui --patch scratch/ed-patch
```
`scripts/capture.py` takes the theme A and B, light and dark matrix under Xvfb; `scripts/combine.py` appends the nine to `patches/composition` for the benches.

## Numbers

Frame time (`bench_chrome 300 live`, rack only, audio on, fake live data every frame, 1440x900 light; this machine is noisy, a second run moves medians by 0.2 ms):

| Patch | median | p95 | max |
|---|---|---|---|
| Composition alone (reference), zoom 1.0 | 6.5 ms | 7.2 | 10.5 |
| Nine effects (24 modules), zoom 0.5, all nine visible | 2.9 | 3.2 | 3.5 |
| Nine effects, zoom 1.0 | 3.1 | 3.4 | 4.6 |
| Composition plus the nine (combined), zoom 0.45 | 8.5 to 8.7 | 9.3 | 10.8 to 11.9 |

The combined patch is about twice the module and cable count of Composition; with the nine displays switched off the same patch measures 8.4 to 8.5 ms, so the displays cost about 0.1 to 0.2 ms of it. Max stays under 16 ms in every run but one dark run at zoom 0.7 (max 72 ms, p95 17 ms) that I could not reproduce and attribute to the machine (see the report).

Audio callback (`bench_composition` on the combined patch, steady state, 16 face taps including the nine effects): median 335 us with taps against 315 to 340 us without (budget 5333 us); p99 770 against 900; p99.9 about 840 against 1060 to 1300. The tap cost is inside the noise.

Captures (`media/`): theme A (Satin) and B (Lacquer), light and dark, all nine at 50 % and three close-ups at 111 to 166 %; `displays-moving.mp4` (24 s, the displays animating). The plugin in REAPER is in `media/plugin-*` when present.
