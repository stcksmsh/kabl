# slopstop, improved: what changed and what it measures

Goal from Kosta: stop as much slop as possible. Your original kit is untouched. The improved copy is in `slopstop/` here (also `slopstop-improved.tar.gz`, and `slopstop-improved.patch` against the original). Changes are `semantic.py` (new, stdlib only), edits to `slop.py`, two ast-grep rules, 4 new selftest cases, README. Selftest: **35 passed, 0 failed, 4 skipped** (was 31/0/4 here).

## Where the original fell short, and the fix
| Gap measured in `REPORT.md` | Fix |
|---|---|
| Missed narrating comments, restated docstrings (0/7) | `narrating-comment`, `doc-restates-sig`: comment/doc whose words are all in the code or signature it sits on; WHY words, digits, backticks, ranges and units exempt it |
| Missed silent fallbacks, `let _ = result;` | `silent-fallback` (Rust `Err(_) => const`, `unwrap_or_default()` on fallible calls, `.ok().unwrap_or`, Python broad `except: pass/return None`), `let-underscore-var` |
| Missed thin wrappers, single-impl traits | `thin-wrapper` (forwards its own params, <=1 call site in its crate), `single-impl-trait` (not for extension traits on foreign types) |
| `_new` names missed, `fixed_vibrato` and `g_old` false hits | `variant-of-existing` (`foo_new`/`foo_v2` only when `foo` exists); `fixed_` prefix and `x_old` locals no longer flagged |
| Renamed copy-paste missed | `near-duplicate-code`: identifier/number/string-normalised 40-token windows, cached per git blob; needs >=8 lines and >=4 control-flow keywords so tables/builder runs are not clones |
| `rs-empty-fn` rule could never match (its `not has regex "."` always excluded the body) | rewritten; skips trait impls, trait defaults, `extern` fns. Empty fn stubs: 0/2 -> 2/2 |
| `slop init`'s own CLAUDE.md text tripped `suppression-added` | suppression check only on code files; a reason comment on or above the line makes it advisory; new `allow_suppress` config |
| `rs-let-underscore-call` FPs (cleanup, `into_raw`, `end_pass`, `take_*`, `join`) | those calls exempted (167 -> 134 legacy hits) |
| Agent rules text | `slop init`'s CLAUDE.md block now also tells agents not to narrate, restate, wrap, single-impl or clone-rename |

## Recall
Same hand-written 34 seeds as the aislop and original-slopstop evals (set A, which I used while developing, so it is a *development* set), plus a **held-out set B** of 36 seeds written afterwards in different phrasing with realistic originals present (`heldout-seed-files/`, `heldout-seeds.csv`). B was scored before tuning: improved **28/36** own rules (29 with the clone-window scoring fixed); I then fixed a compile error in one seed file and added stopwords "through/each/all/over", after which B is no longer clean. "Own" = slopstop/aislop rules; "+tools" adds clippy/rustc/ruff passthrough.

| set | original slopstop | improved slopstop | aislop |
|---|---|---|---|
| A (34, development) own / +tools | 7 (21%) / 13 (38%) | **25 (74%) / 30 (88%)** | 6 / 12 |
| B (36, held-out, first measure) own | 14 (39%) | **28 (78%)** | 6 (17%) |
| B final own / +tools | 14 / 18 | **30 (83%) / 34 (94%)** | 6 / 9 |

Still missed (own rules): narration that paraphrases with different names (`// add v to the total` beside `acc += v`); unused imports and dead code after `return` rely on clippy/rustc/ruff at Stop; one paraphrased docstring and one prod `unwrap()` (aislop's rule, deliberately not copied: it was 0% precise).

## Cost: false positives
- **Legacy repo, whole-repo fast audit, new rules only** (`check --all --fast`): `narrating-comment` 0, `thin-wrapper` 0, `variant-of-existing` 0, `single-impl-trait` 0, `doc-restates-sig` 2 (borderline true), `silent-fallback` 4 (2 are `decode_canonical(..).unwrap_or_default()`, plausibly real; 2 are legitimate fallbacks), `let-underscore-var` 13 (advisory; silencing unused variables), `rs-empty-fn` 1. About 20 hits across the whole existing repo. False hits I found by reading earlier versions' output (extension traits, `// OFF, ON.`, `extern` fns, `[lo, hi]` docs, `env::var` defaults, specific Python excepts) were fixed before this count.
- **Nine real PRs (#158-#165), new lines only:** 35 findings -> **34**: the two `fixed_vibrato` false hits are gone, one real copy added (`accesskit_capture.rs` screenshot loop vs `chrome_capture.rs`, 23 lines). New rules found nothing in these PRs; the code there has no slop of those kinds. Triage of the rest is unchanged from `triage-diffmode.csv` (1 TP before, 2 now).
- **With the kabl config** (`kabl.slop.toml`: allow `too_many_arguments`, exclude `docs/**`; reported separately, defaults above): **11 findings, 6 blocking** (5 jscpd clones in tests/examples, 1 near-duplicate TP), 5 advisory `file-too-large`. The 14 benign `suppression-added` are gone.

## Latency
Hook calls: PreToolUse 0.06-0.08 s, PostToolUse (now including semantic checks) **0.10-0.16 s** on the five files. Stop-mode `check` over the nine-PR diff: 33-38 s (was 56-77 s). Clippy and jscpd dominate.

## Not verified
- The improved hooks were exercised by selftest and manual hook calls, not in a fresh `claude -p` session (the original's live loop was verified in `REPORT.md`; the hook plumbing is unchanged).
- Recall sets are hand-written by one author; set B is clean only for its first measurement. Real agent slop will paraphrase more.
- Heuristics are Rust/Python-centric; other languages get only comment/doc/fallback checks that rely on `//`/`#` syntax.
- `near-duplicate-code` requires structurally identical copies. `single-impl-trait` and `let-underscore-var` stay advisory on purpose.
- No re-run of the 100-finding audit sample; the legacy-count above is hits, not a labelled precision.

## To use
Extract `slopstop-improved.tar.gz`, run its `install.sh` (or point `slop` at `slopstop/slop.py`), then `slop init` in kabl and copy `kabl.slop.toml` to `.slop.toml`. Commit `CLAUDE.md`, `.claude/settings.json` and `.slop.toml` on `master` first; until then slopstop treats them as new gate edits (first-run friction found earlier, not fixed).
