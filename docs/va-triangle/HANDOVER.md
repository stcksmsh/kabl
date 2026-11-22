# osc.va band-limited triangle: handover

## Task

Replace the naive `osc.va` triangle with a band-limited one before 1.0. Keep level, peaks and
zero crossings; real-time safe, saw-like cost; holds under sync and FM. Measure aliasing
before/after, check factory levels and goldens, keep peaks <= -1 dBFS. Commit, push, open a PR
(never merge). Report to "Kabl overseer".

## Done (branch `claude/va-triangle-blamp`)

- `crates/modules/src/dsp.rs`: `poly_blamp` (polyBLEP integrated once, residual
  `(1-|x|)^3/6`) and `triangle_blamp` (corners at 0.25 / 0.75, slope steps of -8 / +8 per
  phase, scaled by `dt`). `FullOsc` uses it too.
- `crates/modules/src/builtins/osc_va.rs`: triangle uses `triangle_blamp`; the sync edge adds a
  BLAMP term for the triangle's slope change (`kink`), next to the existing BLEP jump term.
- `crates/modules/tests/osc_palette.rs`: `triangle_aliases_far_less_than_naive_at_the_same_level`
  and `synced_triangle_is_band_limited`.
- Golden hashes updated for `expressive/lead`, `palette/lead`, `palette/keys` only.

## Measured

- Aliasing at 4186 Hz (C8; 4000 Hz folds onto its own harmonics at 48 kHz): worst -33.9 ->
  -46.0 dB, total -31.5 -> -45.9 dB. 3520 Hz: total -35.3 -> -52.8 dB. 1760 Hz: -43.9 -> -61.1.
- Sync triangle 880x3.3: total -17.5 -> -30.6 dB.
- Level vs naive: 110 Hz -0.0002 dB, 440 Hz -0.003 dB, 880 Hz -0.012 dB. Sample peak 440 Hz
  -0.11 dB, 880 Hz -0.21 dB (corner rounding). Samples away from corners are identical.
- Factory `level_table` (148 patches): only `oak-stave`, `salt-flat`, `static-garden` move,
  each by 0.01 dB. Max peak -1.04 dBFS. 31 patches use the triangle (find with
  `.claude/tmp`-style script parsing `log.jsonl` for `osc.va` waveform = 1).

## Passing

fmt, clippy `--workspace --all-targets -D warnings`, `osc_palette`, `osc_va`, `golden_renders`
(regenerated before the hash edit; re-run to confirm).

## Remaining

1. `cargo test --workspace` (stopped while compiling), incl. `-p kabl-ui --test factory_bank`.
2. If green, open the PR with the numbers above. Never merge.
