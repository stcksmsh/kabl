# D11 PR 6 handover (branch claude/d11-hardening, cut from claude/d11-wasmi-fixes)

Task: calibration hooked at app startup and device change; fault tests (trap, loop, mem over cap, NaN, oversize blob, forbidden import, ABI bump, each keeps synth playing and shows the right warning); bit-exact save/reopen with code; 16 voices x 4 modules zero-alloc check; perf gate unchanged; REAPER CLAP numbers (sink kabl_agents). Out of scope: Faust, D12, factory sounds, PR 5 ground (examples, code_notice.rs). Do not open PR until #131 merged, then rebase on origin/master. Do not merge. Report `DONE <PR#> <commit>` to Kabl overseer, cargo clean, stop.

Done: ABI check. `abi N` meta line, absent = host ABI (0); mismatch gives LoadError::AbiMismatch (`wasmi_engine.rs`). No test yet.

Findings:
- Engine-level tests already exist in crates/modules/tests/wasmi_engine.rs: trap, loop, memory cap, NaN, forbidden import, 16x4 alloc, budgets. Core has oversize-blob and version-change tests.
- Nothing wires code modules into the app. `Budget` and `Calibration::measure` are used only by tests. No loader turns `Embedded` into registered `CodeGroup`s. Unknown kind makes compile fail, so there is no bypass placeholder.

Remaining, in order:
1. `code/host.rs` in kabl-modules: CodeHost (engine, Budget, background calibrate, device_changed(sr, block), load(specs, voices) -> warnings). Call it from standalone main (startup, stream build) and clap activate.
2. Tests in crates/engine/tests (dev-dep `wat`): fault matrix on a real patch (built-in osc + test-only wasm fixture), ABI-bump test, bit-exact save/reopen via format::write_kabl_full/read_kabl_full, 16x4 alloc via PatchEngine + assert_no_alloc.
3. fmt, clippy, debug tests all targets, regenerate THIRD-PARTY-NOTICES if deps change.
4. REAPER CLAP run and numbers into PR body.
5. After #131 merges: rebase, open PR, wait for checks, read Codex comments.
