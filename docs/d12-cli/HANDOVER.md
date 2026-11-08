# D12 PR 2 handover: `cargo-kabl` CLI (new/build/validate) plus template

Branch `claude/d12-cli`, worktree `.worktrees/g4-kit`. Report to the session "Kabl overseer"; do not merge; stop when done.

## Task (condensed)
Crate `crates/cargo-kabl` (GPL-3.0-or-later, binary `cargo-kabl`) with `new <id> [--audio|--control]`, `build`, `validate`, plus a working `sdk/template` (0BSD, one test, README, LICENSE, rust-toolchain pin). Move `kabl.notices` from the SDK macro to `cargo kabl build`. CI: two clean template builds byte-identical. Reuse the host's `WasmiEngine`/`Layout::validate`. Do not edit `crates/modules`; in `sdk/` touch only the notices removal and the template. Out of scope: `cargo kabl test` (PR 3), packaging (PR 4).
Finish: fmt, clippy, debug tests counting every target, `cargo deny`, regenerate `THIRD-PARTY-NOTICES.md` if deps change (`python3 -I packaging/gen-notices.py`), open PR, wait for ALL checks, read Codex comments, fix real findings, send `DONE <PR#> <commit>`, `cargo clean`.

## Done (pushed)
- `309f98da` CLI, template, notices moved to build, CI job `cargo-kabl` in `.github/workflows/ci.yml`.
- `edb7eddd` finite-only smoke inputs.
- `971466f6` two-tier smoke render (current head before this handover).
- fmt, clippy (`-p cargo-kabl --all-targets`), `cargo deny check` pass; THIRD-PARTY-NOTICES unchanged (serde_json and wat were already in the lock). 22 cargo-kabl unit tests pass.
- Template built in two different directories gave identical sha256.

## Remaining, in order
1. `git fetch && git rebase origin/master` (PR 0 = #138, merged as 1bdb61e0). Expect conflicts only in `sdk/` (agent 1 added `migrate` to the SDK) and possibly `Cargo.lock`.
2. Switch the unknown-key warning to the new parser. `validate.rs` has its own `KNOWN_KEYS` list and scan (runs before the host parse); check whether PR 0 exposes a known-key list or a warning API in `parse_meta` and reuse it instead. Keep `an_unknown_key_warns` test; the host now loads unknown keys, so it should also assert `ok()`.
3. Check that PR 0's `abi_minor` gate lets the SDK blobs load; no `LOCALHACK` is needed any more. Confirm the template, `demo.logic`, `demo.bitfilter` pass: `cargo build -p cargo-kabl`, then `target/debug/cargo-kabl kabl build --manifest-path sdk/template/Cargo.toml` (examples need `--allow-reserved`). PR 0 swaps the SDK example blobs in the host; check `sdk/` examples still match.
4. Full workspace debug tests (`cargo test --workspace`, count every target), clippy `--workspace --all-targets -D warnings`, `cargo fmt --all --check`, `cargo deny check`, notices diff.
5. Open the PR (end the description with the attribution line from the session reminder), wait for ALL checks including the new `cargo-kabl` job, read Codex threads, fix real findings, send `DONE <PR#> <commit>`, then `cargo clean`.

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
