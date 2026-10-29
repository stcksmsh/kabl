# Stereo step 4: osc.va stereo output, stereo pan

- `osc.va`: `out_r` replaced by `stereo` (width 2, io slots 1 and 2). `out` is unchanged (the whole stack at `width` 0, the left channel otherwise), so a patch that only wires `out` is bit-identical. `stereo` at `width` 0 is two copies of `out`; a stereo cable into a mono input averages, which is exact for L = R. `out_r` was used by no shipped patch.
- `pan`: `in` is width 2. A mono source is broadcast, giving the old pan sample for sample; a stereo source is balanced (left times cos, right times sin).
- Skipped: `widener` (optional; add when a patch needs width past the oscillator).
- Not wired by default: `stereo` makes every follow module after it a two-instance pair (design section 1), so wire it only when width is wanted. Mono patches pay nothing.

## Numbers (unison 8, detune 30 ct, saw, 220 Hz; test `width_keeps_the_mono_fold_alive`)

| width | L/R correlation | mono fold ((L+R)/2) vs width 0 |
|---|---|---|
| 0 | 1.000 | 0.00 dB |
| 50 | 0.847 | -0.35 dB |
| 100 | 0.459 | -1.34 dB |

No collapse in mono. CPU, `bench_patches` on `patches/crowded`, `taskset -c 2`, master vs this branch, two runs each: 8 voices 38.8 / 38.9 us, 16 voices 76.2-76.4 / 76.5-76.9 us (no change; no shipped patch uses `stereo`).

`width` is smoothed per sample: the module ramps each oscillator's left/right gain across the block from where the last block ended (the engine's own 10 ms smoothing only moves the value once per block). Test `a_fast_width_move_does_not_step` moves width 0 -> 100 % in one block and asserts sample 0 stays within one ramp step of the old sound, the block ends on the new one, and no sample-to-sample jump exceeds the saw's own edges. Width 0 to 0 takes the old mono path, so mono stays bit-identical.

Demo: `evidence/osc-width-demo.{flac,mp3}`, one continuous 20 s render of a four-chord phrase (1.5 s per chord, envelope and VCA after the `stereo` port). 0-12 s: `width` swept 0 -> 100 -> 0 % by a 12 s LFO (L/R correlation per second: 1.00, 0.99, 0.95, 0.80, 0.68, 0.62, 0.51, 0.69, 0.82, 0.91, 0.99, 1.00). 12-20 s: mono / width 100 % A/B in 2 s sections with 20 ms crossfades (corr 1.00, 0.58, 1.00, 0.58). `audio_defects.py`: clean. Source: `cargo run --release -p kabl-engine --example stereo_osc_demo -- out.wav`.
