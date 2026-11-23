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
  Saw and square keep master's exact sync expression: an earlier reordering of the float sum
  changed the synced square in `palette/lead` and `expressive/lead` bit for bit.
- `crates/modules/tests/osc_palette.rs`: `triangle_aliases_far_less_than_naive_at_the_same_level`
  and `synced_triangle_is_band_limited`.
- Golden hash updated for `palette/keys` only (the one triangle patch in the curated list).

## Measured

- Aliasing at 4186 Hz (C8; 4000 Hz folds onto its own harmonics at 48 kHz): worst -33.9 ->
  -46.0 dB, total -31.5 -> -45.9 dB. 3520 Hz: total -35.3 -> -52.8 dB. 1760 Hz: -43.9 -> -61.1.
- Sync triangle 880x3.3: total -17.5 -> -30.6 dB.
- Level vs naive: 110 Hz -0.0002 dB, 440 Hz -0.003 dB, 880 Hz -0.012 dB. Sample peak 440 Hz
  -0.11 dB, 880 Hz -0.21 dB (corner rounding). Samples away from corners are identical.
- Factory `level_table` (148 patches): only `oak-stave`, `salt-flat`, `static-garden` move,
  each by 0.01 dB. Max peak -1.04 dBFS. 31 patches use the triangle (find with
  `.claude/tmp`-style script parsing `log.jsonl` for `osc.va` waveform = 1).

## Golden check (all 148 patches, master vs branch)

`KABL_WRITE_GOLDEN=1 cargo test --release -p kabl-engine --test golden_renders` (chord hash)
and `KABL_DIGEST=<file> cargo test --release -p kabl-ui --test factory_bank render_digest --
--ignored` (24 s demo render) on both builds, then diff. Both change exactly the triangle
patches and nothing else; `presets/chandelier` uses the triangle and does not change.

## Passing

fmt, clippy `--workspace --all-targets -D warnings`, `cargo test --workspace`.

## Remaining

None. Draft PR open; never merge. Listening pairs in the root checkout's
`.claude/tmp/listen/triangle/`: `triangle-dry-{before,after}.wav` (bare `osc.va` triangle,
melody C6-C8 then a C5-C8 sweep) and `triangle-spark-plug-{before,after}.wav`.

Inharmonic energy vs harmonic, dry clip, before -> after: melody mean -38.1 -> -46.9 dB, C8
-31.4 -> -44.9 dB, sweep -38.3 -> -45.8 dB. Factory patches mask it: the largest
before/after difference among the 30 that change is Spark Plug at -32.3 dB of the signal
(Soft Sine, filtered at 1.4 kHz, is far below that).
