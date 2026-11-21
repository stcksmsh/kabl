# ENG-V1 handover: E3a-2 REAPER code-module benchmark

## Task
Roadmap Phase 7 "REAPER code-module benchmark": a re-runnable script plus its numbers in a doc, one
draft PR on `claude/eng-reaper-bench`. Scratch REAPER profile only, `kabl_agents` sink only,
offline render, no format or ABI change. Out of scope: E3b (main.rs/standalone), which went to
another builder.

## Done
- `docs/eng-v1/scripts/reaper_code_bench.py` + `bench.lua`: the benchmark.
- `docs/eng-v1/REAPER-CODE-BENCH.md`: method, numbers, findings.
- `docs/eng-v1/evidence/reaper-code-bench/result.json`: the raw run.
- Findings: a code filter costs about 8.4× the built-in SVF (36 vs 4.3 ms per audio s, 8 voices).
  Only 2 fit at 8 voices, because fuel is budgeted at the worst calibration probe.

## Remaining
Draft PR, wait for the 9 checks, send DONE to the overseer, `cargo clean`.

## Facts learned
- CLAP state `code` = hex of `Embedded::to_bytes` (ported to Python in the script).
- kabl budgets against `period_us` 1333 (64 frames) in REAPER even though REAPER's BSIZE is 128.
- An overrun code module renders silent blocks; the script counts them.

## Standing rules
Report to "Kabl overseer [f98528]". Do not merge. Run `cargo clean` before stopping.
