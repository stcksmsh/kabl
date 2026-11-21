# REAPER code-module benchmark

Roadmap Phase 7: what a code module (`.kablmod` code, wasm run by wasmi) costs inside REAPER,
compared with a built-in module.

## How to run

```
cargo build --release -p kabl-clap
python3 docs/eng-v1/scripts/reaper_code_bench.py
```

The script uses its own Xvfb display, a scratch REAPER profile in `scratch/code-bench/` (it never
reads the user's REAPER config) and the silent `kabl_agents` audio sink. A run of the defaults takes
about 3 minutes. The numbers go to `docs/eng-v1/evidence/reaper-code-bench/result.json`. You can
change `KB_COUNTS` (default `1,2,3`), `KB_SECS` (default 30), `KB_REPEATS` (default 3) and
`KB_PLUGIN`.

## Method

- One kabl CLAP instance on one track holds an 8-note chord for 30 s. The patch is
  `midi.in -> osc.va -> X x N in series -> vca (env.adsr) -> out`.
- X is one of three: nothing (the baseline); the built-in `filter.svf`; or `demo.bitfilter`, the
  example code module (a stereo resonant low-pass with drive). Its blob is embedded in the plugin
  state the way a saved song carries it.
- REAPER renders offline with its own renderer, so render time is the plugin's CPU cost.
  - Each variant runs 3 times, each in a fresh REAPER, and the fastest run counts.
  - Cost per module = (variant − baseline) / N.
- `bench.lua` waits for kabl's fuel calibration before rendering. The calibration report comes
  from `KABL_CODE_CALIBRATION_REPORT`, so code modules run with the calibrated budget, not the
  start-up fallback.
- A code module over budget outputs silent blocks (`crates/modules/src/code/module.rs`). Every
  render's silent 256-frame blocks are therefore counted. A run with any silent block is marked
  `overrun` and given no cost.

## Results (2026-11-20, i7-13700H, REAPER 7.75, 48 kHz, device block 128)

| X × N | render (30 s audio) | realtime × | cost per module, ms per audio s | per 128-frame block |
|---|---|---|---|---|
| baseline | 0.92 s | 32.5 | – | – |
| filter.svf × 1 | 1.05 s | 28.5 | 4.3 | 11.6 µs |
| filter.svf × 3 | 1.29 s | 23.3 | 4.1 | 10.9 µs |
| demo.bitfilter × 1 | 2.02 s | 14.9 | 36.4 | 97 µs |
| demo.bitfilter × 2 | 3.11 s | 9.6 | 36.5 | 97 µs |
| demo.bitfilter × 3 | silent from the first block in all 3 runs (overrun) | | | |

The calibration report in REAPER: worst 927.2 fuel/µs (`call`; `spin` 1234, `fdiv` 1250,
`idiv` 1284, `mem` 4309). `period_us` was 1333.3, so kabl budgets against a 64-frame period, not
the 128 frames of REAPER's device block.

## What it shows

1. **Cost.** With 8 voices playing, one code filter costs about 36 ms of CPU per second of audio
   (3.6 % of one core). The built-in filter costs about 4.3 ms, so the code module costs about
   8.4× as much. Part of the gap is the work itself: the bitfilter is a 2-channel filter with drive,
   while `filter.svf` is a plain state-variable filter. The rest is wasmi interpretation. The cost
   is linear in N while the modules fit.
2. **Capacity.** At 8 voices only **two** of these code modules fit; a third silences every code
   module. Each module's budget is 50 % of the period divided among the code modules (25 % at
   most), converted to fuel at the *worst* calibration probe (927 fuel/µs). This filter's straight
   DSP burns fuel much faster per µs (near the `mem` probe, about 4300/µs). The numbers fit that
   explanation:
   - at N = 3 a module gets 1333 µs × 0.5 / 3 ≈ 222 µs ≈ 206k fuel per block for all its voices;
   - its real work is about 49 µs per 64 frames, about 209k fuel at the `mem` rate, just over
     the cap.

   So a typical DSP module gets roughly a quarter of the CPU time its share promises. The budget is
   safe but conservative. Raising code-module capacity is a budget-policy decision for Kosta (for
   example, calibrate on a DSP-like probe mix instead of the worst probe). This benchmark is the
   way to measure the effect.
3. Offline rendering stays at 9.6× realtime or faster with two code modules at 8 voices, so a code
   module does not threaten offline renders. The limit is the budget, not REAPER.
