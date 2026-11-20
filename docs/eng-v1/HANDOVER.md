# ENG-V1 handover: E3a-2 REAPER code-module benchmark

## Task
Roadmap Phase 7 "REAPER code-module benchmark": measure what a code module (wasm) costs inside
REAPER compared with built-in modules. Deliver a re-runnable script plus its numbers in a doc, as one
draft PR on branch `claude/eng-reaper-bench` (from origin/master 87dcf836). Constraints: use a
scratch REAPER profile only and never touch the user's config; audio only on `kabl_agents`; offline
render; no format or ABI change; wait for all 9 checks; never merge. Out of scope: E3b, which waits
for the overseer's word.

## Done
Only discovery so far, with no code yet. This file is the only commit on the branch.

## Remaining, in order
1. Write `docs/eng-v1/scripts/reaper_code_bench.py`, modelled on `docs/presets/scripts/host.py`
   (Xvfb, a scratch profile under `scratch/code-bench/`, `pw-jack reaper -newinst`, offline render
   through a Lua script).
2. Patch variants: midi.in -> osc.va -> N x X -> out, where X is none, `demo.bitfilter` (code) or a
   comparable built-in filter. N is about 1, 8 and 32, with a held chord of about 8 notes so the
   voice-rate modules run per voice. Render about 30 s.
3. Lua: wait at least 5 s before rendering, so calibration finishes (it starts 2 s after
   device_changed) and the code budget is not the fallback. Time `Main_OnCommand(42230)` with
   `reaper.time_precise()`. Set `KABL_CODE_CALIBRATION_REPORT=<scratch>/calib-<tag>.txt`.
4. Report: render seconds, realtime factor, and per-instance cost = (t(N) - t(none)) / N for code
   and for the built-in. Check that the code render is non-silent and differs from the none
   variant, so a budget-cut or stand-in module cannot hide in the result.
5. Write the doc `docs/eng-v1/REAPER-CODE-BENCH.md` with the numbers and the command. Then run fmt,
   clippy and tests, open a draft PR (body via `gh api`), and send `DONE <PR#> <commit>`.

## Facts learned
- The CLAP state in a .rpp is `<STATE>` base64 of (u64 LE length + JSON SoundState).
  `docs/presets/scripts/host.py` has `chunk_of` and `states_of`.
- A SoundState with code: `version: 6`, plus `code` = lower-case hex of
  `kabl_core::embedded::Embedded::to_bytes()`. That needs `add_blob(kind, version, bytes)` and
  `bind(module_id, hash)` (see `crates/clap/src/code_tests.rs:90`). `sound_state` is private to
  kabl-clap, so produce the hex with a small Rust example that uses `kabl_core::embedded`
  (`crates/core/src/embedded.rs:319` to_bytes), or port the format to Python.
- The blob is `crates/modules/src/code/examples/demo_bitfilter.wasm`, kind `demo.bitfilter`:
  voice rate, in audio 2 ch, `cutoff_cv`, out audio 2 ch, params cutoff/resonance/drive. The CLAP
  does not register the examples itself, so the blob must be embedded in the state.
- `host.rs:310` writes the calibration report (worst_fuel_per_us, per-probe rates, period_us).
- Reusable render Lua: `docs/functional-cables/scripts/host.lua` (env KABL_SE_HOST/TAG/SECS;
  waits 2 s, saves, renders). It does not time the render, so copy it and add timing.
- reaper is `/usr/local/bin/reaper`. Build with `cargo build --release -p kabl-clap`.

## Standing rules
Report to "Kabl overseer [f98528]". Do not merge. Run `cargo clean` before stopping.
