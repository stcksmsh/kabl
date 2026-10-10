# Handover: functional cables, routes into cable parameters (PR #15)

Branch `claude/functional-cables`, worktree `/secondary/Programming/Github/kabl/.worktrees/functional-cables`. Report to the session named "Kabl overseer". Do not merge. Style: caveman and ponytail at ultra in chat, normal English in code, commits, docs, PR text. Context rule and checkpoint protocol: `/secondary/Programming/Github/kabl/CLAUDE.md`.

## Task as received (condensed)

Original: make a cable's own parameters (morph, probability, glide) modulation destinations; see `docs/decisions.md`, "Routes into a cable's own parameters". Kosta accepted the macro-morph clip, Morph Suite, the nesting rule (patterned and chained routes allowed, loops and self-targets rejected), the 0.25 default amount and refusing routes across a group boundary.

Current task: make #15 mergeable after master moved to bb70204 (FM, wavetable, tables at schema 5). Merge master, renumber schema steps, reconcile host state, prove nothing moved, totals, validator, REAPER, update handover and PR body, push, report to the overseer, do not merge, stop.

## Done

- Master merged by merge commit `4ff476c` (30 branch commits keep their hashes; no force-push). Conflicts: `crates/core/src/format.rs`, `docs/benchmarks.md`.
- Schema: 5 = tables (master), 6 = patterns, 7 = routes; `CURRENT_SCHEMA_VERSION` 7. Host state version untouched by this branch (master's 4).
- Re-stamped in the repo: morph-arc, echo-throws, five-against-eight, `docs/promo-demo/patch/morph-suite` to 6; macro-morph to 7. Logs untouched.
- Tests: `crates/engine/tests/tables_and_cables.rs` (wavetable plus patterned cable; master schema-5 table file; table refused below 5). Totals: `cargo test --workspace --no-fail-fast` 783 passed, 0 failed, 28 ignored. Strict clippy: no error outside `crates/ui/examples/app_target`.
- Proof: `render_hash` over 27 patches (incl. all five branch patches and Morph Suite) identical before and after the merge; macro-morph clip wav sha256 identical (994c9ec8...); master's `golden_renders` (26 patches, incl. oscillator demos) passes unchanged.
- Host: validator 35/9 equal to baseline; REAPER save, quit, reopen with macro under automation: identical (diff 0.0), see `HOST.md`.

## Remaining

Nothing but the push and report if they were denied: `git push origin claude/functional-cables`, `gh pr edit 15 --body-file docs/functional-cables/PR-BODY.md`.

## Facts that cost effort

- Heredocs and compound commands can trip the worktree guard: write scripts with Write into `$CLAUDE_JOB_DIR/tmp`. Git with other-worktree paths in the same command is refused.
- `cargo fmt` follows mods: run `rustfmt --edition 2021 <file>` on touched files only.
- Host run: `FC_REF` must be an f32 wav: `cable_routes_demo render-host x.wav`, then `ffmpeg -i x.wav -c:a pcm_f32le y.wav`. The clap-validator binary is at `.worktrees/factory-performance-bank/scratch/factory-bank/tools/clap-validator` (copy it to the job tmp dir first).
- A route's schema number is only a stamp; nothing reads it to play patterns or routes.

## Assumptions and open decisions for Kosta

- Files saved outside the repo by earlier builds of this branch keep stamps 5 or 6; they play correctly here, but a master build would play a patterns file stamped 5 as plain cables. Open and save once to restamp to 7.
- Host state version not bumped for patterns or routes (an older host build reads them as plain cable params). Revisit if that must be refused.
- Promo recut with one macro after the redesign; host measurement of pulse-sample accuracy not done.
