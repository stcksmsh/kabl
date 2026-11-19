# aislop evaluation on kabl

**Version:** aislop 0.22.1 (npm), defaults, no config file. Repo at `origin/master` 1bdb61e0. Clippy and rustfmt ran (cargo, system); `ruff`, `cppcheck`, `cargo-audit` not installed, so Python lint/format, C lint and dependency audit did not run.

## Baseline (`baseline.json`)
- Full scan: **43.8 s wall**, score **74 / "Needs Work"**, **194 findings** (3 errors, 191 warnings, 21 "fixable"). 85 of 194 sit in `docs/` scripts. 129 `.rs`, 63 `.py`, 2 `.c`. rustfmt and clippy: 0 findings.
- Coverage: 321 production files scanned, 44 Lua files unsupported/unscored. Everything under `tests/`, `examples/`, `benches/` is excluded by the built-in directory policy (e.g. `crates/core/tests/*.rs`), so integration tests are not scanned. 362 `.rs` files in `crates/`; 484 in repo. The "dominant language" came out as python/unknown, not Rust.
- aislop's own `rust-*` rules are only two: `rust-non-test-unwrap`, `rust-todo-stub`. Everything else for Rust is clippy passthrough.

## Precision (`triage.csv`)
194 > 150, so random sample of **100, seed 42** (python `random.Random(42)` over findings sorted by file/line/rule). Manual labels. Precision = TP/(n sampled); MEH counts against it, as specified. Overall: **11 TP / 100**.

| rule | findings (all) | sampled | TP | FP | MEH | precision |
|---|---|---|---|---|---|---|
| ai-slop/rust-non-test-unwrap | 83 | 37 | 0 | 13 | 24 | 0% |
| ai-slop/python-repetitive-dispatch | 29 | 16 | 0 | 13 | 3 | 0% |
| complexity/function-too-long | 20 | 13 | 1 | 1 | 11 | 8% |
| complexity/file-too-large | 19 | 10 | 6 | 0 | 4 | 60% |
| complexity/too-many-params | 16 | 10 | 0 | 0 | 10 | 0% |
| ai-slop/narrative-comment | 12 | 5 | 0 | 2 | 3 | 0% |
| ai-slop/unused-import (py) | 9 | 4 | 4 | 0 | 0 | 100% |
| complexity/deep-nesting | 2 | 2 | 0 | 0 | 2 | 0% |
| ai-slop/swallowed-exception | 2 | 1 | 0 | 1 | 0 | 0% |
| ai-slop/python-chained-dict-get | 1 | 1 | 0 | 1 | 0 | 0% |
| security/python-exec | 1 | 1 | 0 | 0 | 1 | 0% |

Small-n rows (n<5) are anecdotal. No finding was a confirmed defect (aislop's own `confirmed-defect` count: 0).

## Recall on seeded slop (`seeds.csv`, branch `eval/aislop-seeded`, local only)
34 instances in 5 new files (`crates/{core,modules,engine,ui,cables}/src/seed_*.rs`). First scan with the files outside the module tree: only 6 hits, clippy blind. Files were then wired in with `mod seed_x;` and rescanned; table is that run. "aislop rules" = aislop's own `ai-slop/*` rules; "with clippy" adds clippy lints surfaced by aislop (compiler, not aislop's detection).

| category | seeded | aislop rules | with clippy |
|---|---|---|---|
| narrating comment | 4 | 2 (trivial-comment) | 2 |
| docstring restates signature | 3 | 0 | 0 |
| todo!/unimplemented! | 2 | 2 | 2 |
| empty fn stub | 2 | 0 | 0 |
| silent fallback (`unwrap_or_default`, `Err(_)=>const`, ...) | 4 | 0 | 1 (clippy manual_unwrap_or) |
| `let _ = result;` swallowed | 2 | 0 | 0 |
| thin forwarding wrapper | 1 | 0 | 0 |
| single-impl trait | 1 | 0 | 0 |
| `_v2` / `enhanced_` / `_new` naming | 3 | 0 | 0 |
| copy-pasted ~30-line block | 2 | 0 | 0 |
| unused import | 4 | 0 | 4 (rustc) |
| dead code after return | 2 | 0 | 1 (rustc) |
| TODO / meta comment | 3 | 1 | 1 |
| bonus: prod `unwrap()` | 1 | 1 | 1 |
| **total** | **34** | **6 (18%)** | **12 (35%)** |

jscpd (`--min-tokens 50`, rust, `crates/`) **caught** the exact-copy block (seed_a:34-62 vs seed_c:33-61, 29 lines) but **missed** the renamed-variables copy in seed_d. aislop has no clone detector (its dedup rules are duplicate-import / duplicate-type-declaration), so it missed both. jscpd also reports 428 pre-existing clones (5.3% of lines) in kabl; its noise was not triaged.

## Top false-positive patterns
1. **`rust-non-test-unwrap` (83 findings, 0 TP in sample).** Flags `slice.try_into().unwrap()` on fixed-size slices (infallible, `core/src/embedded.rs:260`, `panel.rs:72`), `Mutex::lock().unwrap()` (idiomatic poisoning, `clap/src/lib.rs:518`), and unwrap right after an `is_some_and` guard (`engine/src/patch_engine.rs:668`). It also flags unwraps in scratch/probe code under `docs/`. It does not distinguish these from a real unchecked unwrap.
2. **`python-repetitive-dispatch`**: one `elif` ladder in a docs driver (`docs/rack-migration/drive.py`) is reported once per branch, giving 12 duplicate findings from one smell.
3. **`function-too-long` / `too-many-params` / `file-too-large`** use generic thresholds: flags DSP `process()` (`osc_fm6.rs:279`, 173 lines), egui immediate-mode fns, and 468-line test fns. Only genuinely huge files (`ui/src/lib.rs` 6145, `engine/compile.rs` 3185) were TP.

The domain traps in the brief (`unsafe`, `#[inline]`, DSP magic numbers, lock-free) did **not** misfire: no rule targets them. The DSP-specific misfire is the length rule.

## Hook
- **Latency:** the hook command (`aislop hook claude`) took **~2 s** per edit in a manual run. `aislop scan --include <file>` takes **23-39 s** (small util 34.9 s, big DSP `drums.rs` 35.0 s, egui `pattern.rs` 22.7 s, CLAP `clap/src/lib.rs` 38.7 s) because clippy runs workspace-wide (~18-32 s); the test file was not scanned at all (excluded dir, 2.6 s). Target <1 s not met by `scan`; hook ~2 s misses it too.
- **Diff mode:** yes, `--changes [--base <ref>]` and `--staged`. With no changes, `--changes` ran in 2.7 s. Format and lint were skipped in that run.
- **Install:** `aislop hook install --claude` defaults to **global** scope; `--project` is required. Generated `.claude/settings.json` (worktree only, not committed): `PostToolUse`, matcher `Edit|Write|MultiEdit`, command `aislop hook claude`; plus `FileChanged` on `.aislop/config.yml|.aislop/rules.yml|package.json` running `aislop hook claude --on-file-changed`. It also writes `.claude/AISLOP.md` and `.claude/CLAUDE.md` (`@AISLOP.md`), telling the agent to treat findings as blocking.
- **Exit code / channel:** exit **0**, stdout JSON `{"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"<aislop.hook.v2 JSON>"}}`, i.e. advisory context, never blocks. `--quality-gate` adds a Stop hook that blocks on regression below a baseline; **not tested**.
- **Did the feedback loop work? Partly.** My sloppy Edit (narrating comment + `let _ = b.len();`) produced no feedback in the session. Later, after the report was written, a Write of a markdown file did deliver the hook's `aislop.hook.v2` JSON into the session (0 findings, no block), so the channel works mid-session. Why the first Edit showed nothing is unexplained (hooks may not have loaded yet); the live Edit case was not re-run. Simulating the hook by hand with a PostToolUse payload gave valid JSON, but it listed only the two *pre-existing* unwraps in the file and did **not** flag the two sloppy lines I had added (`newFindingCount` 2 = pre-existing). Reverted afterwards.

## Verdict: REJECT (as a kabl hook), with a note
At defaults it fails both ways: 11% precision on existing code (mostly noise from one unwrap rule and size thresholds) and 18% recall on obvious seeded slop, with no clone detection; the hook did not flag the slop I injected. It is cheap to run (~2 s hook) and fast on `--changes`, so it is not a latency problem, but there is little signal to gate on. Things that would be worth a **separate tuned run** (not done here, so not measured): disabling `ai-slop/rust-non-test-unwrap`, `ai-slop/python-repetitive-dispatch`, `complexity/*` except `file-too-large`; restricting scope to `crates/`; leaving only `rust-todo-stub`, `trivial-comment`, `unused-import` and clippy. That leaves rules clippy and rustc already cover. For Rust slop, a `clippy -D warnings` hook plus jscpd for copy-paste looks like better value.

## Not verified / caveats
- Whether the live hook flags real slop on an Edit (delivery itself was seen once, on a Write); `--quality-gate` Stop hook.
- Triage is one reviewer's judgment on a 100/194 sample, with MEH counted against precision. Seeds are author-written (34, more than the ~20 asked), not organic agent output; recall is probably flattering for rules that match textual patterns.
- Python/C lint engines (ruff, cppcheck) and `cargo audit` were unavailable.
- A tuned run was not performed.
