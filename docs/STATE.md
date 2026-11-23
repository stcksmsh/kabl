# State 2026-11-23

## Goal
Make kabl easy to start, learn, understand and reuse, and bring it to Windows through VST3, without losing musical depth. The plan is `docs/ROADMAP.md` (canonical since 2026-11-23). Release 1.0 on Linux comes first.

## Done
- Release 1.0.0 prep: #196 merged at `89d9c1be`.
- Triangle polyBLAMP: #204 merged.
- Perform scenes fix: #199 merged.
- SDK port limits: #198 merged.
- Pivot investigations (read-only): corpus manifest, composite catalogue, first-run and learning design, Windows/VST3 feasibility. Copied to `docs/pivot/`.
- Canonical roadmap: branch `claude/roadmap-pivot` (this PR).

## In flight
- Promo v2 captions pass on `claude/promo-v2` (#203, draft). Not approved; no video until E2.
- #205, the release-mode test build fix (`assert_no_alloc` disable_release). Green and reviewed clean; awaiting Kosta's merge, then rc.2.
- #206, golden render hashes for the 129 browser patches (C0). In review.
- L1, the lesson engine core with the recipes migrated. In flight (builder running, branch `claude/lesson-engine`).

## Next
1. After Kosta merges #205: tag `v1.0.0-rc.2` and watch the release run (R2).
2. R1: align `docs/ACCEPTANCE-V1.md` §1/§2 with 1.0 and drop the stale [PR] markers. Docs only.
3. R3: install the rc.2 tarball on a clean profile. Then Kosta runs the acceptance checklist (R4).
4. L2 (computer-key notes and key strip) and L4 (macro reveal), alongside L1.
5. When OD-C2 is answered: CI1 (factory composite ids and bank), then CI2.
6. When OD-W9 is answered: W1 (RT handle `Send` fix plus a `windows-latest` CI job).

## Waiting on owner
- Merge #205.
- Acceptance run on rc.2.
- Promo v2 questions: are the half-bar cuts too fast; the Loading Dock pulse; a Pad flash.
- First run and learning: D1-D7 and D03-R (ROADMAP "Owner decisions").
- Acid Line mix values 0.16/0.08 on 0-100 % params (OD-1); the `crowded` fixture in the browser (OD-3).
- Composites: first-batch interface approval (OD-C), id scheme (OD-C2), format impact of G2/G3 (OD-C3).
- Windows: test machine, Live licence, signing, installer, class id freeze, VST logo, Windows CLAP, ASIO, toolchain (OD-W1 to OD-W9).

## Hard-won facts
- **Cargo setup**
  - Shared cargo target: `/secondary/Programming/Github/kabl-target`.
  - Untracked `kabl/.cargo/config.toml` sets it, with `jobs = 8`.
  - Run every build under `systemd-run --user --scope -p MemoryMax=10G -p MemorySwapMax=2G`. An OOM on 11-23 came from unbounded builds.
  - At most 3 building agents at once.
- **auto_clean.** `.nonslop.toml` on master has `[done] auto_clean = true`, which ran `cargo clean` and wiped the shared target. It must become `false`. Agents cannot edit that file; Kosta changes it.
- **GitHub CLI**
  - `gh pr edit` fails on this machine. Set PR bodies and bases with `gh api -X PATCH repos/stcksmsh/kabl/pulls/N -F body=@file`.
  - Update a PR branch with `gh api -X PUT repos/stcksmsh/kabl/pulls/N/update-branch`.
  - Create tags with `gh api` on `repos/stcksmsh/kabl/git/refs` (`ref=refs/tags/<tag>`, `sha=<commit>`).
- **Release tests.** `release.yml` runs `cargo test --workspace --release`, but PR CI builds debug only, so release-only compile errors surface only on a tag. That is what broke rc.1.
- **Golden renders.** `crates/engine/tests/golden_renders.rs` walks `patches/` to depth 3. Before #206 it covered 26 patches and none of the presets.
- **Corpus facts**
  - No shipped patch uses a composite.
  - 54 browser patches have 121 cable-param routes, the G1 blocker.
  - The Sounds browser scans two directory levels: 129 listed, 19 engine demos hidden.
- **Windows.** The workspace cross-compiles for `x86_64-pc-windows-gnu` with one real error: the RT priority handle is not `Send` (`crates/ui/src/app/callback.rs`).
- **Branch hygiene.** The root checkout's `master` is stale; use `origin/master`. `ccr-1adff24f-zzq23d` (`docs/v2/PLAN.md`) is unreviewed and must not be merged.
