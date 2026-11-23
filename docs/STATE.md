# State 2026-11-23 (late)

## Goal
Make kabl easy to start, learn, understand and reuse, and bring it to Windows through VST3, without losing musical depth. The plan is `docs/ROADMAP.md` (canonical since 2026-11-23; owner brief in `docs/pivot/BRIEF-2026-11-23.md`). Release 1.0 on Linux comes first.

## Done
- Release 1.0.0 prep #196 (`89d9c1be`), triangle polyBLAMP #204 (`c294d702`), Perform scenes #199 (`8c42c40f`), promo cut 1 #197 (`55b317bf`), SDK port limits #198.
- `v1.0.0-rc.1` tagged on `89d9c1be`: release run failed at `cargo test --workspace --release` (assert_no_alloc). Nothing published.
- Pivot investigations (read-only) in `docs/pivot/`: corpus manifest, composite catalogue, first-run and learning design, Windows/VST3 feasibility.

## In flight
- #205 release-mode test fix (`e81bd2c3`): 9/9 green, reviewer clean. Waits for Kosta "merge 205 and tag rc.2".
- #207 this roadmap + state (`claude/roadmap-pivot`): docs only, reviewer minor findings all fixed. Waits for Kosta "merge 207".
- #206 C0 golden hashes for the 129 browser patches (`claude/golden-corpus`, `f17d2194`): reworked to 8 s renders for all + each cue launched once (4 patches have cues); test moved to `crates/ui/tests/golden_corpus.rs`. Re-review and CI running.
- L1 lesson engine (`claude/lesson-engine`, opus builder): engine + TOML format + runner + progress in library.json, recipes ported unchanged, captures to `.claude/tmp/listen/lessons/`. No PR yet when this was written. If the builder died with the session, check the branch on origin and re-dispatch from the ROADMAP L1 slice.
- #203 promo v2 (`11c9acd3`, draft, CI green on earlier head): captioned A video at `.claude/tmp/listen/promo/promo-v2-A-captions.mp4`, zoom done in image space (no label popping), unsaved dot fixed. PARKED per pivot (E after composites). Known fix before any use: caption "More than 100 factory sounds, each played from a few named macros" is false (11 of 129 have no macros); wide racks still cut off (needs UI fit-to-view).

## Next
1. After "merge 205": merge, tag `v1.0.0-rc.2` via gh api on the merge commit, watch release run (R2); then R3 clean-profile install (builder), then Kosta's acceptance (R4). R1 (ACCEPTANCE-V1 §1/§2 docs fix) before R4.
2. #206: re-review after the rework push; offer merge when green.
3. L1: review PR when it lands; then L2 (computer-key notes + key strip) and L4 (macro reveal).
4. When OD-C2 answered: CI1 (factory composite ids + bank), then CI2 (generators emit composites), then CB1-CB4 (Space, Drum Kit, Sequenced Part, Glue Bus).
5. When OD-W9 answered: W1 (RT handle Send fix + windows-latest CI).

## Waiting on owner
- `auto_clean = false` committed to master `.nonslop.toml` (urgent: every finishing builder wipes the shared target). Local copy already false.
- "merge 205 and tag rc.2"; "merge 207".
- D1-D7 + D03 reversal (learning), OD-1 Acid Line mix 0.16/0.08, OD-3 `crowded` fixture, OD-C/C2/C3 (composites), OD-W1..W9 (Windows: test machine, Live trial, signing, installer, class id, logo, Windows CLAP, ASIO, MSVC).
- Promo: parked unless Kosta asks; old v2 variant questions superseded (A is the base).

## Hard-won facts
- **Builds are serial.** Shared target `/secondary/Programming/Github/kabl-target` via untracked `kabl/.cargo/config.toml` (`jobs = 8`); cargo's lock serialises agents. Every build/render under `systemd-run --user --scope -p MemoryMax=10G -p MemorySwapMax=2G`. Never `cargo clean`, never `CARGO_TARGET_DIR` (parallel only with Kosta's OK). OOM 11-23: two uncapped builders + Kosta's Java build on a 30 GB box.
- **Shared target pitfall.** A build script from another branch can go stale (kabl-ui build.rs panicked on L1's `lessons/` dir); fix by touching `crates/ui/build.rs`. Tell builders.
- **Coordinator merges only on Kosta's "merge <n>"** in the conversation; tags likewise.
- **GitHub CLI.** `gh pr edit` fails: use `gh api -X PATCH repos/stcksmsh/kabl/pulls/N -F body=@file`. Update branch: `gh api -X PUT .../pulls/N/update-branch`. Tags: `gh api repos/stcksmsh/kabl/git/refs -f ref=refs/tags/<t> -f sha=<c>` (triggers release.yml). `gh pr checks --watch` can exit before checks register: re-check. Draft PRs need `gh pr ready` before merge.
- **Release tests.** release.yml runs `cargo test --workspace --release`; PR CI is debug only.
- **Golden renders.** Standard render = 3-note chord, 400 blocks of 64 = 0.53 s; too short for pieces (see #206 rework).
- **Corpus.** No shipped patch uses a composite; G1 (macro routes on cable amounts can't cross a composite boundary) blocks 40 patches incl. Lichen; browser scans 2 levels: 129 listed, 19 engine demos hidden.
- **Windows.** Cross-compiles for x86_64-pc-windows-gnu with one real error (RT priority handle not Send, `crates/ui/src/app/callback.rs:64`). Route: clap-wrapper around the CLAP.
- **Agents and the worktree guard.** Report files must be written inside the agent's worktree or handed back; investigators could not write to `.claude/tmp/coordinator/reports/`.
- **Branch hygiene.** Root checkout `master` is stale; use `origin/master`. `ccr-1adff24f-zzq23d` (docs/v2/PLAN.md) unreviewed, never merge.
