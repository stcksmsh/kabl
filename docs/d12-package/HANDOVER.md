# D12 PR 4: `.kablmod` package and host install (handover)

Task: D12 brief section 4.1 and table row 4. Branch `claude/d12-package` from origin/master 00fb5ec9.
Rules: fmt, clippy, debug tests counting every target, audio only on kabl_agents; draft PR, wait for all
checks, send `DONE <PR#> <commit>`; after Kosta marks ready, Codex round; do not merge. Report to "Kabl overseer".

## Done (all in the working tree, committed with this handover)
- `crates/modules/src/code/package.rs`: Manifest, Package, stored-zip reader/writer, copyleft rule, `WasmiEngine::notices`.
- `crates/modules/src/code/install.rs`: user module dir, scan, load_all, install with refuse rules, missing report.
- `crates/modules/tests/package.rs`: 8 tests, pass.
- `crates/cargo-kabl/src/pack.rs` + one call in `build.rs`: writes `target/kabl/<id>-<version>.kablmod`. Verified end to end with `unzip -t`.
- Template `sdk/template/Cargo.toml` gains `authors` and `description` (manifest needs an author).
- UI: `crates/ui/src/module_install.rs` (load at Add-module popover, drop a .kablmod to install, hover info with notices, missing-version line on patch open); hooks in lib.rs, tables.rs, browser.rs.
- Deps: serde, serde_json, crc32fast added to kabl-modules (all in lock; THIRD-PARTY-NOTICES unchanged, diff clean).
- cargo fmt, clippy workspace (-D warnings): clean.

## Update (builder session 2)
- `abi_minor` is now the blob's own (`WasmiEngine::read_abi_minor`); `resources` stays empty.
- cargo-kabl pack unit test added (temp 0BSD crate + WAT blob; no wasm target needed).
- fmt, clippy (--workspace --all-targets -D warnings, local 1.99) clean; `cargo test --workspace --no-fail-fast` all ok
  (incl. the assert_no_alloc check in modules/tests/effects.rs). `kabl-ui code_examples::bitfilter...` failed once in a
  parallel run and passed alone and in the rerun: flaky, not caused by this PR.

## Remaining
1. Draft PR, wait for checks (CI lint uses Rust 1.100.0), send DONE to "Kabl overseer [47d561]".
2. No UI test for module_install.
3. Rebase only on overseer `REBASE` (#141, #142 touch cargo-kabl build.rs/main.rs and ui examples).
