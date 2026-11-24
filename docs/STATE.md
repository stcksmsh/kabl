# State 2026-11-24

## Goal
Make kabl easy to start, learn, understand and reuse, and bring it to Windows through VST3, without losing musical depth. Plan: `docs/ROADMAP.md` (canonical). Release 1.0 on Linux first.

## Done
- 11-23: roadmap #207; release-mode test fix #205; golden corpus C0 #206; acceptance checklist R1 #208; factory composite ids CI1 #210; state #211.
- 11-24: L1 lesson engine #209; CI2 `encapsulate_with` #212; W1a RT handle Send #213; CI3 cable-param exposure #215 (schema v9 only when used); CI4 exposure fan-in #216; W2 platform paths #217; L2 computer keys and key strip #218; W3 file browser on Windows #219; W4 Windows-safe save #220; L4 macro reveal #221.
- `v1.0.0-rc.2` tagged on `cd9578b6`; predates #208, so use master's `docs/ACCEPTANCE-V1.md`.

## In flight
- #214 Space factory composite (`claude/space-composite`, draft, review clean at 814904d6, behind master): waits on Kosta's OD-C. Before merge: update-branch, verify, review again.
- Composite face labels (`claude/composite-face-labels`): builder running. Fixes "IN In"/"OUT Out" and doubled control names on closed composite faces.
- R3 rc.2 clean-profile install (`claude/rc2-install-test`, no commits): blocked, agents denied `gh release`.

## Next
1. After OD-C: merge #214, then CB2-CB4 (more factory composites) and CM1 (factory patches on composites).
2. L3 first-run flow after D1, D3, D7.
3. R3 once `gh release` is allowed, then R4 Kosta's acceptance, R5 fixes, tag v1.0.0.
4. Windows: W5 clap-validator (agent run of the downloaded tool was refused; needs Kosta's OK), then W6 VST3, W7-W10. All need the Windows CI job in `docs/windows/CI-JOB.md` pasted into `.github/workflows/` by Kosta.
5. Follow-ups: CLAP host automation of a macro does not reach the editor (no macro reveal, rack knob does not follow; needs crates/clap); route `crates/modules/src/code/install.rs:118` rename through `kabl_core::fsutil`; arp and `midi.note` ignore note source (controller and computer key C4 share a voice); CLAP `sound_state` schema signalling for v9; Windows factory-content location (W9); print child stderr in `composites_factory.rs:93`; `kabl_engine::golden` could be `#[doc(hidden)]`.

## Waiting on owner
- `auto_clean = false` in master `.nonslop.toml`: finishing builders wipe the shared target mid-build.
- `.claude/settings.json` denies `gh release:*`; replace with create/delete/edit/upload denies to allow download/view.
- Whether agents may download and run clap-validator (W5).
- Paste the Windows CI job from `docs/windows/CI-JOB.md`.
- Decisions: 2 D1, 3 D3, 4 D7, 5 OD-1, 6 OD-C (Space, #214), 7 OD-W1, 8 OD-W2, 9 OD-W3.

## Decisions
- Kosta 11-23: coordinator merges and tags itself once verify, review and CI are green (pre-production, no users). Hooks still bind.
- Kosta 11-23: lessons are for enjoyment, not school: Next always skips; a step done earlier or already met on arrival shows done.
- Coordinator on delegation: D03-R A, D2 A, D4 A, D5 A, D6 A, OD-3 A, OD-C2 A, OD-C3 A (v9 only when needed), OD-W4..W9 A.
- L2: while computer keys are on, A plays a note and never opens Add module; UI notes use `Source::Host` in the standalone.
- L4: reveal only on a hand or controller move (drag, wheel, keys, CC, undo), never on cue morphs; hold 1.2 s, 4 rows then "+k more".
- W3: Windows drive list from `GetLogicalDrives` (no media probe). W4: retry only on Windows errors 5/32, ~0.95 s; log rotation uses `move_file` (keeps source on failure).
- VST3 route stays clap-wrapper; W6 adds a CLAP vs VST3 CPU check.

## Hard-won facts
- Builds serial through shared target `/secondary/Programming/Github/kabl-target`; every cargo or render under `systemd-run --user --scope -p MemoryMax=10G -p MemorySwapMax=2G`. Never `cargo clean` or `CARGO_TARGET_DIR`. `touch crates/ui/build.rs` when the shared OUT_DIR goes stale.
- Windows cross-check: stable `cargo check --target x86_64-pc-windows-gnu -p kabl-ui -p kabl-standalone -p kabl-clap` with `ZSTD_SYS_USE_PKG_CONFIG=1 PKG_CONFIG_ALLOW_CROSS=1`; 1.99.0 lacks the target. MSVC untested.
- Hooks: agents cannot edit `.nonslop.toml` or `.claude/settings.json`; deny rules beat allow. The merge hook wants verify at the current head, so re-verify after every update-branch.
- GitHub: `gh pr edit` fails, use `gh api -X PATCH repos/stcksmsh/kabl/pulls/N -F body=@file`; update branch `gh api -X PUT .../pulls/N/update-branch`; tags `gh api repos/stcksmsh/kabl/git/refs -f ref=refs/tags/<t> -f sha=<c>`; draft PRs need `gh pr ready`.
- release.yml runs `cargo test --workspace --release`; PR CI is debug only.
- Flaky: `code_examples::module_settings_survive_save_and_reopen_through_the_real_path` (save timing); passes on rerun.
- Root checkout `master` is stale; use `origin/master`. Old `claude/*` branches with no PR are history, not dead builders.
