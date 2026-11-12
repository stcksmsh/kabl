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

## Remaining
1. `.github/scripts/clippy.sh` and `cargo test --workspace`: not finished (run was stopped; all results seen so far passed). Also the allocation check.
2. Known gap: manifest `abi_minor` is written as 0 (CodeMeta has no minor); `resources` empty.
3. No UI test for module_install; no cargo-kabl unit test for pack (needs wasm target).
4. Open the draft PR, wait for checks, send DONE.

Next command: `cargo test --workspace 2>&1 | grep -E "^test result|FAILED"` from this worktree.
