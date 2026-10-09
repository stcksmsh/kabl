# Handover: functional cables to engineering-complete (PR #15)

Branch `claude/functional-cables`, worktree `/secondary/Programming/Github/kabl/.worktrees/functional-cables`. Report to the session named "Kabl overseer". Do not merge. Style: caveman and ponytail at ultra in chat, normal English in code, commits, docs, PR text.

## Task as received (condensed)

Finish functional cables to engineering-complete, in order: (1) push unpushed commits; (2) record morph as kept-as-built pending Kosta; (3) bring morph and glide to the pattern/probability standard (tests for timing, restart reproducibility, runtime edits without rebuild, undo, save/reload, legacy bit-identical; no allocation; cost in `docs/benchmarks.md`); (4) full workspace tests and strict clippy at the final head, record totals; (5) verify in the real plugin and host: CLAP validator against the recorded baseline (35 successes, 9 skips) and a REAPER save, full quit, reopen with state and render comparison, for a project using pattern, probability and morph; adapt the scripts on `origin/claude/sound-engines` under `docs/sound-engines/scripts/` (`host.py`, `host.lua`, `validator_diff.py`); (6) update the PR #15 description. Out of scope: rebasing onto the FM/wavetable branches, restyling the cable editor (redesign is on another branch), merging.

## Done

- Promo demo (earlier, same branch): `docs/promo-demo/`, reviewed (`REVIEW.md`).
- `cf4e449`: morph exact at both ends (`a*(1-m) + b*m`), 7 new tests in `crates/engine/tests/functional_cables.rs` (ends bit-identical, replay after Restart/Stop/Run, runtime edits and compile boundary, glide timing, undo/save/reload of morph+glide+B, no-alloc with morph running); `cable_cost` gets `KABL_MORPH`; cost in `docs/benchmarks.md` (no measurable difference); `docs/decisions.md` and `docs/functional-cables/README.md` say morph is kept as built, pending Kosta. `cargo test -p kabl-engine --test functional_cables`: 19 passed; `legacy_patches`: 1 passed.

## Remaining, in order

1. Host check (step 5). Fetch the scripts: `git show origin/claude/sound-engines:docs/sound-engines/scripts/host.py` (and `host.lua`, `validator_diff.py`) into `docs/functional-cables/scripts/`, adapt, do not rewrite. Need a patch with pattern, probability and morph (use `patches/functional-cables/morph-arc` or `docs/promo-demo/patch/morph-suite`). Build the plugin as the scripts expect, run the validator and diff against the baseline (35 successes, 9 skips), then REAPER save, full quit, reopen, compare state and render. Write results to `docs/functional-cables/HOST.md` with exact commands, versions, and what was not run.
2. Full `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` at the final head; record totals in the PR and in `docs/functional-cables/README.md` or REVIEW.md. Last known: 683 passed before the 7 new tests (expect 690).
3. Update the PR #15 description (draft body: use `gh pr edit 15 --body-file`); say the morph choice is an assumption awaiting Kosta; include test totals, host results, the promo as a separate follow-up commit set on this branch. Needs Kosta's go-ahead per his rule to ask before posting; the overseer relayed it, so confirm with the overseer if unsure.
4. One report to "Kabl overseer": what changed, totals, host results, unverified, decisions for Kosta. Then stop.

## Facts that cost effort

- Writes are confined to the session's own worktree; `git worktree add` elsewhere cannot be written to. Use branches in this worktree.
- Compound shell commands, `$VARS` and `| head` pipes trip the guard or panic Rust stdout; write to a file, use plain commands, Write/Edit for scripts. `pkill -f <name>` matches your own shell: use `pkill <exact name>`.
- Scratch dir: `/home/stcksmsh/.claude/jobs/484d43b6/tmp`.
- Regenerating the promo patch rewrites `log.jsonl` timestamps.
- rustfmt only touched files; check `git status` after.

## Assumptions and open decisions for Kosta

- Morph = A/B blend plus glide (assumed, not confirmed). Open: morph as a cable property or a shared source; cable params mappable to macro/CC (promo moved seven morphs one at a time).
- Promo recut after design and FM/wavetable land (`docs/promo-demo/README.md`).
