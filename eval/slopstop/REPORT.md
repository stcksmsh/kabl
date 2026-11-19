# slopstop evaluation on kabl

Tested the owner-supplied `~/Downloads/slopstop.tar.gz` (stdlib-Python script `slop.py`, 1114 lines, plus ast-grep rule pack; README, selftest, `install.sh`). No version string in the kit; evaluated as extracted. Same plan as `eval/aislop/REPORT.md`; aislop numbers are quoted for comparison.

**Setup.** Extracted to an empty scratch dir and read before running; `slop.py` makes no network calls and has no destructive operations. I did NOT run `install.sh` (global `pip --break-system-packages`, `npm -g`, symlink in `~/.local/bin`). Instead: a venv with `ast-grep-cli` + `ruff`, an npm prefix with `jscpd@4`, and a wrapper (`sh slop.sh`) that sets PATH. Clippy ran from the system cargo. Not installed, so skipped silently: `basedpyright`, `vulture`, `cppcheck`, `clang-tidy`, `selene`, `shellcheck`, `gitleaks`. Selftest: 31 passed, 0 failed, 4 skipped (README says "35 checks"; the 4 skips are tools I didn't install).

## Baseline audit (`check --all`, `baseline.json`)
206.5 s wall, **2205 findings** (1065 duplicate-code, 167 `rs-let-underscore-call`, 153 `suppression-added`, ~700 ruff). 1059 of them under `docs/`. This is not slopstop's intended mode (README: "one-off audit, not as a gate"): `--all` treats every line as new, so every existing `#[allow]` is "added".

Precision of that audit: random sample of 100, seed 42 (`triage.csv`). **3 TP / 100.**

| rule | all | sampled | TP | FP | MEH |
|---|---|---|---|---|---|
| duplicate-code | 1065 | 47 | 2 | 12 | 33 |
| ruff:* (docs scripts) | ~700 | 26 | 1 | 12 | 13 |
| rs-let-underscore-call | 167 | 14 | 0 | 8 | 6 |
| suppression-added | 153 | 7 | 0 | 0 | 7 |
| sh-swallow-stderr | 12 | 3 | 0 | 2 | 1 |
| rs-test-no-assert / sh-or-true / file-too-large | 13/16/15 | 3 | 0 | 2 | 1 |

## Precision in its real mode: new lines only (`diffmode.json`, `triage-diffmode.csv`)
`check --base acce234c` on nine recent agent-written PRs (#158-#165, 227 files, +27.7k lines), 76.7 s. Only **35 findings**, so all were triaged: **1 TP, 3 FP, 31 MEH.** Clippy produced nothing new. By severity, the 22 *blocking* findings were 1 TP, 2 FP, 19 MEH.

| rule | n | TP | FP | MEH | note |
|---|---|---|---|---|---|
| suppression-added (block) | 14 | 0 | 0 | 14 | new `#[allow(clippy::too_many_arguments)]` on preset-builder helpers, a house pattern; correct match, wrong severity to block on |
| duplicate-code (block) | 6 | 1 | 0 | 5 | one 26-line copy ui test vs `tools/examples/support/mockup.rs` |
| file-too-large (warn) | 5 | 0 | 0 | 5 | `ui/src/lib.rs` 6144 lines etc.; pre-existing |
| ruff:* (docs scripts) | 8 | 0 | 3 | 5 | shebang, `print` in CLI scripts |
| versioned-name (block) | 2 | 0 | 2 | 0 | `fixed_vibrato`: "fixed" means constant |

The big difference from aislop: the diff-only filter removes the legacy noise (aislop: 194 findings; slopstop on the same repo: 35 over nine PRs). It does not make the remaining findings precise.

## Recall on the 34 seeds (`seeds.csv`)
Same seeded branch as the aislop eval (`eval/aislop-seeded`, seeds wired into the module tree), `check --base HEAD~1`, 54 s. "Own rules" excludes clippy/rustc passthrough. Clippy `dead_code` findings on the seed files are ignored as artefacts of how the seeds were placed.

| category | seeded | slopstop rules | with clippy |
|---|---|---|---|
| narrating comment | 4 | 0 | 0 |
| docstring restates signature | 3 | 0 | 0 |
| todo!/unimplemented! | 2 | 2 | 2 |
| empty fn stub | 2 | 0 | 0 |
| silent fallback | 4 | 0 | 1 |
| `let _ = result;` swallowed | 2 | 1 (call form only; `let _ = result;` missed) | 1 |
| thin forwarding wrapper | 1 | 0 | 0 |
| single-impl trait | 1 | 0 | 0 |
| `_v2`/`enhanced_`/`_new` | 3 | 2 (`_new` missed) | 2 |
| copy-pasted block | 2 | 1 (exact copy; renamed copy missed) | 1 |
| unused import | 4 | 0 | 4 (rustc) |
| dead code after return | 2 | 0 | 1 |
| TODO / meta comment | 3 | 1 | 1 |
| bonus: prod `unwrap()` | 1 | 0 | 0 |
| **total** | **34** | **7 (21%)** | **13 (38%)** |

aislop on the same seeds: 6 (18%) own, 12 (35%) with clippy. Different gaps: slopstop catches `_v2`/`enhanced_` names and jscpd clones, which aislop did not; aislop caught narrating comments (2) and `unwrap`, which slopstop did not. Both miss docstrings, empty stubs, silent fallbacks, thin wrappers and single-impl traits (slopstop's README says as much). `// placeholder` (an empty-stub seed's comment) was caught as an advisory hedge-comment, but the empty fn itself was not.

In diff mode the seeded branch produced no findings outside the seed files except one `file-too-large` on `crates/ui/src/lib.rs` (caused by the `mod seed_d;` line I appended): the diff filter works.

## Hook viability
- **Latency (manual hook calls, payload on stdin, file edited with an injected `todo!`/`let _ =` first):**
  | file | PreToolUse | PostToolUse |
  |---|---|---|
  | small util `core/src/panel.rs` | 0.10 s | 0.15 s |
  | big DSP `modules/.../drums.rs` | 0.12 s | 0.17 s |
  | egui UI `ui/src/pattern.rs` | 0.10 s | 0.16 s |
  | CLAP glue `clap/src/lib.rs` | 0.15 s | 0.19 s |
  | test file `core/tests/kabl_format.rs` | 0.11 s | 0.15 s |

  All under the 1 s target (aislop scan: 23-39 s; aislop hook: ~2 s). **Stop hook** (clippy + jscpd) took 56 s for a one-line change; 77 s for the nine-PR diff.
- **Diff mode:** yes, it is the default (merge-base with `origin/HEAD`, plus uncommitted and untracked; `--base`, `--staged`, `--all` available).
- **Hook config** from `slop init`, per-worktree (`.claude/settings.json`; I replaced the command path with `sh .../slop.sh hook <ev>` only so the venv tools are on PATH):
  `PreToolUse` matcher `Edit|Write|MultiEdit|NotebookEdit|Bash` -> `slop.py hook pre` (timeout 15); `PostToolUse` matcher `Edit|Write|MultiEdit|NotebookEdit` -> `hook post` (60); `Stop` -> `hook stop` (900). It also appends a rules block to `CLAUDE.md` and writes `.slop.toml`.
- **Exit codes:** Pre denies via JSON `permissionDecision: deny` (exit 0); Post prints `additionalContext` (exit 0); **Stop exits 2** on findings (keeps the agent working), loop breaker after `stop_retries` = 2.
- **Did the loop work live? Yes, in a fresh `claude -p` session** (stream captured in `live-hook-run2.txt`): after an Edit inserting `todo!();`, the agent received `slopstop: 1 issue(s) in your edit of crates/core/src/panel.rs ... [stub-added] stub left in code: todo!()`; an Edit adding `#[allow(dead_code)]` was **refused before running**: `PreToolUse:Edit hook error: slopstop: adding a suppression/skip ... needs user approval`. In an earlier fresh run (`live-hook-run1.txt`, same session setup) the Stop hook blocked the agent repeatedly (477 findings in my untracked eval files) and the agent stopped, asking me. Editing hooks mid-session in my own session did not take effect (the previously installed aislop hook kept firing), so a fresh session is needed after `slop init`.
- **Friction found:**
  1. `slop init`'s own `CLAUDE.md` rules text trips slopstop's blocking `suppression-added` rule (it contains "`#[allow]`"). A freshly initialised repo blocks Stop until `CLAUDE.md` is committed, and `.claude/settings.json`/`.slop.toml` show as `gate-tampered` until they are on the default branch (base is the merge-base with `origin/HEAD`). For the live test I committed them on a throwaway branch and set `base = "HEAD"`.
  2. Untracked files count as new: any generated JSON/CSV under `eval/` produced hundreds of findings and blocked Stop.

## Verdict: ADOPT WITH CONFIG, as an advisory/blocking-lite hook; not as a hard gate yet
Better than aislop on the points that matter for a hook: sub-0.2 s per edit, real diff-only scoping (35 findings over nine PRs instead of ~200 legacy), a Pre hook that blocks suppressions and gate edits, and a Stop hook with a loop breaker, all confirmed live. Its recall (21%/38%) is only slightly above aislop's, and precision on new code is low (1/35).

Config I would apply (not tested as a run, so unmeasured; listed from the triage):
- `warn = ["suppression-added"]` or add `allow_suppressions = true` for `tools/examples/**`: all 14 diff-mode hits were benign `#[allow(clippy::too_many_arguments)]`.
- `disable = ["versioned-name"]` or accept some FPs: `fixed_*` is a legitimate name; its true hits (`_v2`, `enhanced_`) are what we want.
- `exclude = ["docs/**", "tools/examples/**"]`: all 8 ruff findings were docs-script nits, and most duplicate-code/ruff noise lives there.
- `disable = ["rs-let-underscore-call"]` (8/14 FP in the audit sample: `end_pass()`, `Arc::into_raw`, `remove_dir_all` cleanup are not errors), or downgrade to advisory (it already is `warn`).
- Keep: `stub-added`, `todo-added`, `duplicate-code` (min tokens 50 caught the exact-copy seed), `gate-tampered`, the Pre-hook denials.
- Add the clippy lints it suggests (`todo`, `unimplemented`, `dbg_macro`, `let_underscore_must_use` = deny) to `Cargo.toml` workspace lints yourself, and consider `-D warnings`: clippy/rustc supplied most of the extra recall (unused imports 4/4).
- Fix before adopting: the `CLAUDE.md` self-trigger and the first-run `gate-tampered`/base behaviour above.

## Not verified / caveats
- Stop hook looping behaviour end to end after real findings: observed blocking in run 1 but the agent stopped by asking; I did not verify the 2-strike loop breaker live (selftest covers it).
- `gitleaks`, `vulture`, `basedpyright`, `cppcheck`, `selene`, `shellcheck`, `clang-tidy` not installed, so their slice of the 2205/35 findings is absent.
- Config recommendations above are from manual triage and were not re-run.
- Triage is one reviewer's judgment, MEH counted against precision; the diff-mode sample is a single repo range of nine PRs (all macro-preset work, heavy on `tools/examples`), not representative of UI/DSP edits.
- The seeds are my hand-written ones (recall likely flattering for pattern-matching rules).
- `install.sh` was not run; its behaviour was only read.
