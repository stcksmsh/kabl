# D12 PR 2 handover: `cargo-kabl` CLI (new/build/validate) plus template

Branch `claude/d12-cli`, worktree `.worktrees/g4-kit`. Report to the session "Kabl overseer"; do not merge; stop when done.

## Task (condensed)
Crate `crates/cargo-kabl` (GPL-3.0-or-later, binary `cargo-kabl`) with `new <id> [--audio|--control]`, `build`, `validate`, plus a working `sdk/template` (0BSD, one test, README, LICENSE, rust-toolchain pin). Move `kabl.notices` from the SDK macro to `cargo kabl build`. CI: two clean template builds byte-identical. Reuse the host's `WasmiEngine`/`Layout::validate`. Do not edit `crates/modules`; in `sdk/` touch only the notices removal and the template. Out of scope: `cargo kabl test` (PR 3), packaging (PR 4).
Finish: fmt, clippy, debug tests counting every target, `cargo deny`, regenerate `THIRD-PARTY-NOTICES.md` if deps change (`python3 -I packaging/gen-notices.py`), open PR, wait for ALL checks, read Codex comments, fix real findings, send `DONE <PR#> <commit>`, `cargo clean`.

## Done (pushed; PR #139, draft)
- Original work: CLI, template, notices at build, CI job `cargo-kabl`, finite-only smoke, two-tier smoke render.
- Rebased on origin/master (PR 0 #138); one conflict, in the SDK `lib.rs` doc comment.
- `79919a87`: unknown-key warning uses the host's `wasmi_engine::unknown_keys` (no local list; the warning has no line number now).
- `4787410f`: `as_chunks` in the smoke render. CI clippy (rust 1.99) flags `chunks_exact_to_as_chunks`; local rustc 1.93 does not.
- Local: workspace tests `--no-fail-fast` 175 targets, 1253 passed, 0 failed; clippy `-D warnings`, fmt, `cargo deny check` clean. Template, `demo.logic`, `demo.bitfilter` build and validate. THIRD-PARTY-NOTICES unchanged.
- Flake seen once: `kabl-ui` `code_examples::module_settings_survive_save_and_reopen_through_the_real_path` ("no warning on a normal save"), a PR 0 timing test; passed on rerun.

## Remaining, in order
1. Check CI is green on the head commit: `gh pr checks 139`.
2. No Codex threads existed while the PR was a draft. Kosta marks it ready (`gh pr ready` is not available to agents); then read Codex threads (count only those with no reply) and fix real findings.
3. Untested: the default `cargo kabl new` SDK dependency (git on this repo, no SDK tag yet); only `--sdk-path` was exercised.
4. Do not merge. Report to "Kabl overseer". Work is paused until the weekly reset.

## Rules decided with the overseer
- Smoke render, tier 1 (error if output is non-finite): silence, impulse, full-scale square, ±1e4, `f32::MIN_POSITIVE`, denormals; params at default/min/max. Warning if output exceeds ±1e6 on inputs of ±1 or less.
- Tier 2 (warning only): ladder ±1e6 ... ±1e30; first magnitude that goes non-finite or faults is reported ("kabl stops this module if an input reaches about ±X"). `f32::MAX` is deliberately not used (any scaling module overflows).
- PR 0 makes the host zero non-finite inputs, so the validator never feeds NaN/inf.
- Reserved ids (`kabl.`, `demo.`, `test.`) are errors; `--allow-reserved` exists for the SDK examples.

## Blob sizes (before notices removal -> raw build -> via `cargo kabl build`)
`demo.logic` 10,964 -> 2,494 -> 6,320 B; `demo.bitfilter` 12,857 -> 4,387 -> 8,223 B; template 1,970 -> about 5,824 B. The difference to raw is the single notice copy (about 3.8 KB net after stripping name/producers sections).

## Facts that cost effort
- `cargo kabl` flags: `CARGO_ENCODED_RUSTFLAGS` (stack 8192, `--no-entry`, debuginfo=0, three `--remap-path-prefix`); output at `<target>/wasm32-unknown-unknown/release/kabl/<id>.wasm`.
- Template Cargo.toml has its own `[workspace]`; `cargo kabl new` substitutes `yourhandle.gain`, `yourhandle-gain`, the SDK dependency line (git by default, `--sdk-path` for local), the year and the holder. Default SDK dep is `git = "https://github.com/stcksmsh/kabl"`, untested for lack of a pushed SDK tag.
- In this background worktree compound shell commands (heredocs, `&&` chains with cargo-kabl, python heredocs) are often refused by the guard: use single plain commands and the Write/Edit tools.
- The CLI embeds `sdk/kabl-module-sdk/NOTICE.txt` and the template files via `include_str!`.
