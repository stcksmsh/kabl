# slopstop

Token-free slop gate for AI coding agents. It's **glue, not a linter**: one stdlib-Python script that runs
existing OSS tools, keeps only findings on lines the agent added, adds checks no file linter can do
(diff-level), and wires it all into Claude Code hooks.

## What runs where

| Hook | When | Budget | What |
|---|---|---|---|
| `PreToolUse` | before Edit/Write/Bash | ~50 ms | **Blocks** (deny): edits to lint/CI/hook config, lint sections of `Cargo.toml`/`pyproject.toml`/`package.json`/gradle/`tsconfig`, newly added suppressions/skips, removed test assertions, `--no-verify` and friends |
| `PostToolUse` | after each edit | ~150 ms | Diff checks + ast-grep + fast linters on the edited file → fed back as context |
| `Stop` | agent tries to finish | 10 s – few min | Everything incl. clippy, type checkers, jscpd → **exit 2 keeps the agent working** until clean |
| `pre-commit` (opt.) | `git commit` | same as Stop | `slop check --staged` |

**Only new code counts.** The diff base is `auto`: merge-base with the default branch (or `HEAD` when you're
on it), plus uncommitted and untracked files. Legacy findings never show up unless the agent touches them.

**Loop breaker.** If Stop blocks on the same findings `stop_retries` (2) times in a row, it lets the agent stop
and shows you the list instead of looping forever. Advisory-only findings get a single look, then pass.

## Checks

**Diff-level (built in, every language):**
- `suppression-added`: `#[allow]`, `noqa`, `@Suppress`, `eslint-disable`, `@ts-ignore`, `#[ignore]`, `pytest.skip`, `it.skip`, …
- `test-weakened` / `test-deleted`: test files that lost assertions
- `gate-tampered`: lint/CI/hook config changed (also catches edits made through Bash)
- `stub-added` (`todo!()`, `unimplemented!()`, `TODO()`, "not implemented" throws; Python `NotImplementedError` = advisory), `todo-added`
- `chat-residue`: comments like "as requested", "I've updated", "here's the improved version", "in a real implementation"; `hedge-comment` (advisory): "for now", "should work", "placeholder"
- `versioned-name`: `foo_v2`, `parse_fixed`, `enhanced_x`, `loadUpdated`
- `debug-leftover` (`dbg!`, `breakpoint()`, `console.log`, `debugger`), `secret` (key patterns; gitleaks if installed), `file-too-large` (advisory)

**Semantic checks (`semantic.py`, stdlib only, added lines only, every `check`/post-edit run):**
- `narrating-comment`: `// increment the counter` above or beside `n += 1`. Only comments that *start* with a verb and whose content words all appear in the code they sit on; any "because/must/never/not/when/..." or digit/backtick exempts it.
- `doc-restates-sig`: one-line `///` or `"""` doc whose words are all in the fn name, params, return type or impl type (`/// Returns the level.` on `fn level(&self)`). Conditions, ranges and units (`[lo, hi]`, "panics", "if") exempt it.
- `silent-fallback`: `Err(_) => 0.0`, `.parse().unwrap_or_default()`, `.ok().unwrap_or(..)`, `unwrap_or_else(|_| default)`, Python `except Exception: pass/return None`. A trailing comment of 3+ words makes it advisory.
- `let-underscore-var`: `let _ = result;` (blocking for result/err/status-style names, advisory otherwise).
- `thin-wrapper`: Rust/Python fn that only forwards its own params to another fn and has at most one call site in its crate (`git grep`); trait impls exempt.
- `single-impl-trait`: new trait with <=1 impl, no `dyn` use, and not an extension trait on a foreign type (advisory).
- `variant-of-existing`: new `foo_new` / `foo_v2` / `foo_2` / `foo_alt` ... next to an existing `foo`.
- `near-duplicate-code` (Stop/`check` only): added block that matches existing code after renaming identifiers, numbers and strings (40-token windows, cached per git blob under `.git/slopstop/`). Needs >= 8 lines and >= 4 control-flow keywords, so declarative tables and builder-call runs are not clones.
- `rs-empty-fn` (ast-grep) now actually fires (the old rule could never match) and skips trait impls, trait defaults and `extern` fns.

**Tools (used if installed, skipped silently if not; `slop doctor` shows which):**

| Language | Fast (per edit) | Stop |
|---|---|---|
| all | ast-grep rule pack (`rules/`) | jscpd (new clones only), gitleaks |
| Rust | | `cargo clippy` (respects your `[lints]`) |
| Python | ruff (+ slop rule set) | basedpyright/pyright (errors only), vulture |
| Kotlin | | detekt (+ your `detekt.yml`) |
| Java | ast-grep | (compiler) |
| C/C++ | | cppcheck, clang-tidy (only with `compile_commands.json`) |
| TS/JS/Astro/Svelte/Vue/CSS | biome | tsc (if `tsconfig.json`) |
| Go | | golangci-lint v2, else `go vet` |
| Lua | selene | |
| Shell | shellcheck | |
| WGSL/GLSL | | naga (validation) |

Severity: diff checks and high-precision rules (swallowed errors, empty catch, compiler/type errors, unused
imports, blind except, clones) **block**. Everything else is **advisory**: shown, gets one look at Stop, never loops.

## Install

```sh
git clone <your repo> ~/.local/share/slopstop
~/.local/share/slopstop/install.sh                 # everything
~/.local/share/slopstop/install.sh rust python     # or only some: rust python kotlin web lua shell cpp go shaders
slop selftest                                      # 35 checks on seeded slop across languages
```

User-wide (every Claude Code session on the machine), with an upgrade-proof path:

```sh
slop install-global --dry-run     # show what it would write
slop install-global               # copy the kit to ~/.local/share/slopstop/versions/<stamp>, flip `current`,
                                  # link ~/.local/bin/slop, merge hooks into $CLAUDE_CONFIG_DIR (or ~/.claude)/settings.json
slop install-global --uninstall   # remove only slop's hooks
```

Hooks always call `~/.local/bin/slop hook ...`. Upgrading = unpack the new kit and run its `slop install-global`
again: the `current` symlink flips atomically and settings.json is rewritten identically (last 3 versions kept).
User-wide hooks are inert in any repo without a `.slop.toml` (or `SLOP_ALL=1`), so `slop init` is the per-repo opt-in.
Hooks run with your normal PATH, so install `ast-grep`, `ruff`, `jscpd` etc. where that PATH finds them (pipx/npm -g).

Per repo:

```sh
cd ~/code/kabl
slop init            # .claude/settings.json hooks, CLAUDE.md rules block, .slop.toml
slop init --git-hook # also a git pre-commit hook
slop doctor          # which linters are active for this repo's languages
slop check           # what the agent would be told right now (vs merge-base)
slop check --all     # whole repo, legacy included (use for a one-off audit, not as a gate)
```

Commit `.claude/settings.json`, `CLAUDE.md` and `.slop.toml` yourself (agents are blocked from editing the first and last).

Recommended extra for Rust repos (gates `todo!` etc. at compile time too). Add to `Cargo.toml` yourself:

```toml
[workspace.lints.clippy]   # or [lints.clippy] for a single crate
todo = "deny"
unimplemented = "deny"
dbg_macro = "deny"
let_underscore_must_use = "deny"
# not unwrap_used: it misfires on infallible unwraps (lock, try_into on fixed slices)
```

## Configure (`.slop.toml`)

```toml
[slop]
disable = ["rs-let-underscore-call", "ruff:C901"]  # rule ids; "ruff:*" prefixes work
warn = ["duplicate-code"]                          # downgrade to advisory
block = ["rs-test-no-assert"]                      # upgrade to blocking
exclude = ["assets/**"]                            # extends built-in excludes (target, node_modules, ...)
protected = ["docs/api-contract.md"]               # extra files agents may not edit
allow_suppressions = false
allow_suppress = ["clippy::too_many_arguments"]    # substrings of suppressions that stay allowed
clone_min_lines = 8                                # near-duplicate-code: min lines of an added block
stop_retries = 2
jscpd_min_tokens = 50
base = "auto"                                      # or "HEAD" / a ref
```

Project-specific ast-grep rules: put an `sgconfig.yml` + rules under `.slop/` in the repo; they run alongside the kit's.

## Measuring it

kabl already has a benchmark: the 34 seeds on `eval/aislop-seeded`.

```sh
git checkout eval/aislop-seeded
slop check --base master --json > slop-seeds.json   # compare with seeds.csv; aislop got 6/34
```

## Known gaps / untested

- **Tested here** (selftest or output-format probes): ast-grep, ruff, basedpyright, clippy, detekt, biome, cppcheck, selene, shellcheck, jscpd, `go vet` format.
- **Wired but untested** (no network access to them during development): gitleaks (`gitleaks dir` flags),
  golangci-lint v2 JSON flags, naga, clang-tidy with a real compile DB, tsc. If one misbehaves it fails soft
  (logged, skipped), never blocks the agent.
- Semantic checks are heuristics for Rust and Python (comments/docs/fallbacks also run on other `//`/`#` languages;
  wrappers, traits and variants are Rust/Python only). Narration that paraphrases the code with different
  names (`// add v to the total` beside `acc += v`) is missed.
- `near-duplicate-code` needs structurally identical code; a copy that was restructured is not found.
- Not detected: docstrings on non-Rust/Python languages, thin wrappers in Kotlin/TS, single-impl interfaces outside Rust.
- Hooks use `cwd` from the hook input, so they work inside agent git worktrees.
