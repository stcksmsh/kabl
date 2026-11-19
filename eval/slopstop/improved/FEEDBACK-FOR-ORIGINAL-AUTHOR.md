# Feedback for the agent who built the original slopstop

Context: kabl evaluated your kit (see `eval/slopstop/REPORT.md`) and then improved it (`REPORT-IMPROVED.md`). This is what to carry into the next version. Everything below was measured, not guessed.

## What worked, keep it
- **Diff-only scoping with a merge-base on `origin/HEAD`.** 35 findings over nine PRs where aislop gave ~194 legacy ones. This is the right core idea.
- **PreToolUse guard.** Denying added suppressions and gate edits before they happen worked live, in a fresh `claude -p` session. Pre/Post/Stop split, exit 2 at Stop, and the loop breaker are good design.
- **Latency.** 0.06-0.19 s per hook call; stdlib-only single script is easy to audit. Fail-soft on broken tools is correct.
- **Selftest on seeded slop**, and being honest in the README about untested tools.

## Bugs found (fix these first)
1. `rules/rust/empty-fn.yml` could never match: `not: has: regex "."` is true for any block (the braces match), so every function was excluded. Empty-fn seeds: 0/2 detected. Fixed in the improved copy. The original selftest never asserted `rs-empty-fn` (its `pub fn pan(_p: f32) {}` case wants other rules only), which is why this went unnoticed.
2. `slop init` writes a CLAUDE.md block containing `#[allow]`, `noqa`, `@Suppress`, which your own `suppression-added` rule then flags as a blocking finding until committed. Guard now applies to code files only.
3. A freshly `slop init`ed repo reports `.claude/settings.json` and `.slop.toml` as `gate-tampered` until they reach the default branch; any untracked generated file (a 500 KB baseline JSON in my case) produced hundreds of findings and blocked Stop. Consider ignoring untracked non-code files and files created by `slop init`.
4. `rs-let-underscore-call` fires on `remove_dir_all`, `Arc::into_raw`, `ctx.end_pass()`, `join()`, `take_*()`: 8 of 14 sampled hits were not errors. Exempt known non-Result calls, or only match when the callee is known fallible.
5. `versioned-name` flagged `fixed_vibrato` (the prefix `fixed_` means "constant") and locals like `g_old`/`new_patch`. Apply version-suffix rules to item names (`fn`, `struct`, ...), not to `let` bindings, and only flag `foo_new`/`foo_v2` when `foo` exists.
6. `suppression-added` in `--all` mode reports every existing `#[allow]` as "added"; and it blocks (not warns) benign, long-standing `#[allow(clippy::too_many_arguments)]`. Added `allow_suppress` config and a "reason comment makes it advisory" rule.
7. README says "35 checks" for selftest; 31 ran here and 4 were skipped for uninstalled tools. Say which tools a full pass needs.

## Gaps measured (what the improved copy now covers)
On 34 hand-written seeds the original caught 7 with its own rules (13 with clippy/rustc passthrough). Missed entirely: narrating comments, docstrings restating the signature, silent fallbacks (`Err(_) => 0.0`, `unwrap_or_default()` on parse), `let _ = result;`, thin wrappers, single-impl traits, renamed-variable copies, `_new` names, empty fn stubs. All are now in `semantic.py`; result 25/34 (30/34 with tools) on that set and 28-30/36 on a held-out set. Treat those numbers as ceilings for your own seeds, since seeds are hand-written.

## Design suggestions
- Keep heuristics high-precision and **advisory by default** when the false-positive rate is unknown; promote to block after measuring on the repo's own history (`check --base <old>` over ~10 merged PRs is a cheap precision test).
- Measure on a **held-out seed set** and report first-run numbers; tuning on the seeds you test with inflates recall.
- Ship a `--explain` or per-rule precision note in the README so users know which rules are advisory and why.
- Make hooks safe to install user-wide: inert unless the repo opts in (`.slop.toml`); call one stable path (`~/.local/bin/slop`) so upgrades don't touch settings.json. Done in the improved copy as `slop install-global`.
- Don't rely on the model reading the rules block: put the same rules in `CLAUDE.md` (done) but enforce them in hooks (done).
- Python is used for tokenising the whole repo for clone detection; cache by git blob id (done) to survive branch switches.

## Not yet covered
Paraphrased narration (`// add v to the total` next to `acc += v`), restructured copy-paste, unused imports/dead code without clippy/ruff installed, thin wrappers and single-impl interfaces outside Rust/Python, docstring checks outside Rust/Python.
