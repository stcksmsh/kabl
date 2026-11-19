# Handover: aislop evaluation, for the web planner

Written for an agent with no access to this checkout. Everything needed to decide is below. Details are in `eval/aislop/REPORT.md` on branch `eval/aislop` (draft PR #166, github.com/stcksmsh/kabl).

## The question
Should kabl (Rust/egui modular synth, standalone app + CLAP plugin, near v1.0) adopt `aislop` (github.com/scanaislop/aislop, MIT) as a PostToolUse hook for coding agents? This was an evaluation only. No kabl code was changed.

## Answer so far: REJECT at defaults
Tested version: aislop 0.22.1 (npm), default config, on origin/master 1bdb61e0.

| Measure | Result |
|---|---|
| Full scan | 43.8 s, score 74 "Needs Work", 194 findings (3 errors, 191 warnings) |
| Coverage | 321 production files scanned; 44 Lua files unscored; `tests/`, `examples/`, `benches/` dirs excluded by built-in policy |
| Precision | 11 real problems in a random sample of 100 findings (seed 42, manual triage, "not worth blocking" counted against it) |
| Recall on 34 seeded slop instances | 6 (18%) by aislop's own rules; 12 (35%) counting clippy/rustc warnings it passes through |
| Hook latency | `aislop hook claude` about 2 s; `aislop scan --include <file>` 23-39 s (clippy runs workspace-wide) |
| Hook behaviour | exit 0, advisory JSON `additionalContext` (schema `aislop.hook.v2`); never blocks |

Why it fails:
- `ai-slop/rust-non-test-unwrap` (83 findings, 0 real in sample) flags infallible `slice.try_into().unwrap()`, `Mutex::lock().unwrap()`, and unwraps right after a guard. It cannot tell them from a risky unwrap.
- `complexity/function-too-long`, `too-many-params`, `file-too-large` use generic thresholds and hit DSP `process()`, egui immediate-mode functions and test functions. Only the huge files were real (`ui/src/lib.rs` 6145 lines, `engine/src/compile.rs` 3185).
- `python-repetitive-dispatch` reports one `elif` ladder once per branch.
- Missed entirely by aislop's own rules: docstrings restating signatures, empty fn stubs, silent fallbacks (`unwrap_or_default`), `let _ = result;`, thin wrappers, single-impl traits, `_v2`/`enhanced_`/`_new` names, copy-paste blocks. It has no clone detector.
- The hook did not flag a deliberately sloppy edit I made; a manual run reported only pre-existing findings.
- The Rust-specific domain traps in the brief (`unsafe`, `#[inline]`, DSP magic numbers, lock-free) did not misfire, because no rule targets them.

## What is NOT verified (do not treat as known)
1. Live delivery of hook feedback is PARTLY confirmed. After install, an Edit of `panel.rs` showed no feedback, but later a Write of a markdown file did deliver the `aislop.hook.v2` JSON into the session (0 findings, no block). So the channel works mid-session; why the first Edit produced nothing is unexplained (possibly hooks had not loaded yet). Whether it would flag real slop live was not re-tested.
2. The `--quality-gate` Stop hook (blocks on score regression). Not tested.
3. A tuned run. Config was deliberately not changed. The only proposal (unmeasured): disable `rust-non-test-unwrap`, `python-repetitive-dispatch` and `complexity/*` except `file-too-large`; scope to `crates/`; keep `rust-todo-stub`, `trivial-comment`, `unused-import`. That mostly duplicates what clippy and rustc already do.
4. Python/C engines: `ruff`, `cppcheck`, `cargo-audit` were not installed.
5. Triage is one reviewer's judgement; seeds were hand-written, not organic agent output, so recall is likely flattering.

## Suggested alternative (an opinion, not tested)
A `cargo clippy -D warnings` hook for Rust, plus jscpd for copy-paste. jscpd (`--min-tokens 50`) caught the exact-copy seed but missed the renamed-variable copy, and reports 428 existing clones (5.3% of lines) in kabl, so its noise is untriaged.

## Decisions for the owner (Kosta)
- Accept REJECT, or ask for the tuned run and a live hook test in a fresh session first?
- Is a clippy + jscpd hook worth a separate task?
- Safe to ignore otherwise: the evaluation touched nothing in kabl.

## Artifacts and facts
- Branch `eval/aislop` (pushed), `eval/aislop/`: `REPORT.md`, `baseline.json` (full scan), `baseline.time`, `triage.csv` (100 rows), `seeds.csv` (34 rows).
- Branch `eval/aislop-seeded`: local only, never merge. Seeds are `crates/{core,modules,engine,ui,cables}/src/seed_*.rs`, wired in with `mod seed_x;`. `seeds.csv` line numbers refer to it.
- Hook config was generated in the worktree only (`.claude/settings.json`, `AISLOP.md`, `CLAUDE.md`) and is not committed. `aislop hook install` writes to the global config unless `--project` is passed.
- Worktree: `.claude/worktrees/eval-aislop`. `cargo clean` was run.

## Standing rules
Report to the session named "Kabl overseer". Do not merge PR #166. Merged, engineering-complete and owner-accepted are separate statuses; only Kosta grants the last.
