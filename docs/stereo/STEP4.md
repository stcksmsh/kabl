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

Demo: `evidence/osc-width-demo.{flac,mp3}` (width 0, 50, 100 %, 3 s each; `audio_defects.py`: clean). Source: `cargo run --release -p kabl-engine --example stereo_osc_demo -- out.wav`.
