# State 2026-11-23 (night)

## Goal
Make kabl easy to start, learn, understand and reuse, and bring it to Windows through VST3, without losing musical depth. Plan: `docs/ROADMAP.md` (canonical). Release 1.0 on Linux first.

## Done
- Roadmap and pivot investigations #207; release-mode test fix #205; golden corpus hashes C0 #206 (plus shared `kabl_engine::golden` helper); acceptance checklist matches 1.0 (R1) #208; factory composite ids and read-only bank (CI1) #210.
- `v1.0.0-rc.2` tagged on `cd9578b6` (#205 merge); release run passed. Note: the tag predates #208, so use master's `docs/ACCEPTANCE-V1.md`.

## In flight
- #209 L1 lesson engine (`claude/lesson-engine`, `f3c8c3ff`): verify ok, review clean, CI 9/9. Merge waits only on Kosta's D03-R.
- CI2 `encapsulate_with` (`claude/encapsulate-with`): builder running.
- R3 rc.2 clean-profile install (`claude/rc2-install-test`, no commits): blocked, agents denied `gh release download`.

## Next
1. D03-R answered A: merge #209. Then L2 (computer-key notes and key strip) and L4 (macro reveal); both touch `crates/ui/src/lib.rs`, so after #209.
2. R3 once `gh release` is allowed (or the tarball is in `.claude/tmp/r3-a1046/dl/`), then R4 Kosta's acceptance, R5 fixes, tag v1.0.0.
3. After CI2: CI3/CI4 (G1 cable params, G3 fan-in; builder first reports whether a schema bump is needed, OD-C3), then a Space prototype for Kosta's OD-C judgement, then CB1-CB4, CM1.
4. W1 (RT handle Send fix plus windows-latest MSVC CI).
5. Follow-ups: fold `composites::find_factory_bank` into `library::find_factory_dir`; print child stderr in `composites_factory.rs:93`; `kabl_engine::golden` could be `#[doc(hidden)]`.

## Waiting on owner
- `auto_clean = false` in master `.nonslop.toml` (hook lets only Kosta edit it). Finishing builders wipe the shared target; it broke three builds on 11-23.
- `.claude/settings.json` denies `gh release:*`; Kosta allowed download, but only he can edit the file.
- Decisions from the 11-23 list: 1 D03-R, 2 D1, 3 D3, 4 D7, 5 OD-1, 6 OD-C, 7 OD-W1, 8 OD-W2, 9 OD-W3.

## Decisions
- Kosta 11-23: coordinator merges and tags itself once verify, review and CI are green (pre-production, no users). Hooks still bind.
- Kosta 11-23: lessons are for enjoyment, not school: Next always skips; a step done earlier or already met on arrival shows done and asks nothing.
- Taken by coordinator on recommendation (Kosta delegated routine calls): D2 A, D4 A, D5 A, D6 A, OD-3 A (after 1.0), OD-C2 A (`kabl.<name>` vN), OD-C3 builder reports first, OD-W4 A Inno Setup, OD-W5 A freeze id, OD-W6 A no logo, OD-W7 A ship CLAP, OD-W8 A WASAPI only, OD-W9 A MSVC.
- VST3 route stays clap-wrapper: thin in-process translation, no extra copy or thread; W1 adds a CLAP vs VST3 CPU check in REAPER.
- Factory bank installs to `share/kabl/factory-composites` (not `composites`, which is the user library under ~/.local).

## Hard-won facts
- Builds serial through shared target `/secondary/Programming/Github/kabl-target`; every cargo or render under `systemd-run --user --scope -p MemoryMax=10G -p MemorySwapMax=2G`. Never `cargo clean` or `CARGO_TARGET_DIR`. kabl-ui build.rs OUT_DIR is shared across checkouts: `touch crates/ui/build.rs` when `lessons.rs` or similar vanishes.
- Hooks: agents cannot edit `.nonslop.toml` or `.claude/settings.json`; deny rules beat allow.
- GitHub: `gh pr edit` fails, use `gh api -X PATCH repos/stcksmsh/kabl/pulls/N -F body=@file`; update branch `gh api -X PUT .../pulls/N/update-branch`; tags `gh api repos/stcksmsh/kabl/git/refs -f ref=refs/tags/<t> -f sha=<c>`; draft PRs need `gh pr ready`; the merge hook refuses while any check runs.
- release.yml runs `cargo test --workspace --release`; PR CI is debug only, so release-only alloc failures show first on a tag.
- Flaky: `code_examples::module_settings_survive_save_and_reopen_through_the_real_path` (timing) failed once on #209 CI, passed on rerun.
- Corpus: no shipped patch uses a composite yet; G1 blocks 40 patches incl. Lichen; 129 browser patches, 4 with cues.
- Root checkout `master` is stale; use `origin/master`. Old `claude/*` branches with no PR are history, not dead builders.
