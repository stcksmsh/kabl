# State 2026-11-27 (4)

## Goal
Make kabl easy to start, learn, understand and reuse, and bring it to Windows through VST3, without losing musical depth. Plan: `docs/ROADMAP.md` (canonical). 1.0 = Linux + Windows, CLAP + VST3 + standalone, with faces (FACES.md slices 5-11), composites CB2-CB5 and lessons L5-L8; macOS 1.1. Composite faces plan: `docs/composites/FACES.md` (§9 = slice plan).

## Done
- Up to 11-27 morning: see git history (state #272, #278). Highlights: W6 VST3 #264, W7 #267, W8 editor #274, CB2-CB5 #268 #271 #273 #275, L5 #269, faces slices 1-7.
- 11-27 day: state #278, cable-param controls #277 (Env Amt knob on Analog Voice), W9 Windows installer #276 (Inno Setup + zip; CI install/launch/upgrade/uninstall; kabl.exe not shipped), faces slice 8 engine morph #279 (ca337fe5), W9b #280 (4dba6cba: windows_subsystem; notices union linux+windows-msvc), state #281, W9c icon #282 (c5b18da0: embed-resource, kabl.ico in exe + installer), faces slice 9 snapshots #283 (2d87517b; captures + recall WAV sent to Kosta, copies in .claude/tmp/cap283/).

## In flight (builders die on restart: resume from the pushed branch, never redispatch from scratch)
- #284 faces slice 10 whole blocks + Pin to Perform (`claude/faces-blocks`, pushed 3d6b8cbc; worktree .claude/worktrees/agent-a64ed450749f6ae38). Builder was killed by the restart before reporting: check the worktree (`git status`, unpushed), then send a builder with brief .claude/tmp/coord1126/brief-faces-blocks.md + "continue from the pushed branch claude/faces-blocks" to finish Done means (captures, PR body), then verify, review, CI, merge.
- #203 promo v2: parked.
- Every merge moves master: update-branch the other open PRs, re-verify, delta re-review (patch-id check), CI.

## Next
1. Finish #284 (above).
2. Faces slice 11 (compact face): brief to write from FACES §9 item 11 (scope includes crates/core panel.rs/format.rs; add design_page.rs, snapshots.rs: slice 9 needed them).
3. Windows: W8b (host resize on DPI change + Live keyboard passthrough: public baseview method + vendor/nice-plug egui change), W10 release job (+ VST logo guidelines check), W11a standalone ASIO (required for 1.0), WA Kosta VM pass (list in PR #276 body / packaging/windows/README.md).
4. L6 (lessons 6, 8; clap automation hook), then L7+L8 as one slice.
5. Before v1.0.0: rc.3 from master + short re-check (proposed, Kosta to confirm).
6. Follow-ups (minors): #277 route knob: own drag code (panels.rs:322) without Escape cancel and losing pre-threshold pixels, no macro pulse ring / double-click reset on authored face (lib.rs:6086); core format does not refuse two exposures on one target (only encapsulate_with/add_exposure do). #276: launch check stops kabl-ui before the window is up (test-install.ps1:221); Inno fallback pins 6.4.3 vs runner 6.7.1. #283: macro held-cue test checks `morph_from` directly, not through `recall`. Older minors: git show c224539d:docs/STATE.md (Next 6).

## Waiting on owner
- Morning review list: .claude/tmp/coord1126/kosta-review.md (captures, composite interface questions, VST3 id notation, validator allowlist, legacy input passthrough, lesson deviations).
- #277 knob label "Env Amt" (labels must be unique vs port "Env Amount"): question in ANALOG-VOICE.md. Captures: .claude/tmp/coord1126/cap277/.
- R4 acceptance pass on rc.2; rc.3 re-check before v1.0.0 (proposed).

## Decisions (Kosta)
- 11-27: Windows icon option A: add build-only `embed-resource`, kabl.ico in exe/installer/Start menu. Keep Inno Setup (licence permits commercial use; authors ask commercial users to buy): Kosta buys a licence if kabl revenue passes 5k.
- 11-27 coordinator: faces briefs must include design_page.rs + snapshots.rs (list UI lives there); refusal-test inversion approved when a slice lifts "not supported yet". Shared target dir cleared 11-27 (11 GB free -> 144 GB).
- 11-27 coordinator: W9 does not ship kabl.exe (Linux doesn't; its self-test path is build-time); recall behind a fresh pending Load lands (FACES §3.3 "a recall morph always lands"), cue macros there are dropped.
- 11-26 night: D5 lessons 4+5 around composites; composites merge without waiting; ASIO GPLv3-only Windows binary OK; gate-tampered disabled until production (#265). VST3 bundle layout; validator allows one failure (Process Format 1234567.8 Hz).
- 11-26: OD-W4 Inno Setup + zip; OD-W5 VST3 id bda9c239b42389589c969403a7b9d23e; OD-W6 VST compatible logo; OD-W7 ship kabl.clap; OD-W8 ASIO in first Windows release; OD-W9 native MSVC; macOS 1.1; faces, composites, lessons all in 1.0. Merge policy `[merge] coordinator = "auto"`.
- 11-25: 1.0 includes Windows + VST3. Vertical face slider default. ONE builder at a time (shared target dir). Acceptance on rc.2.
- Earlier: git show c224539d:docs/STATE.md.
- Coordinator merges itself once verify, review (current head) and CI are green. Gate-file PRs (.github, .nonslop.toml) go to Kosta ("merge" from Kosta lets the coordinator merge them).
- Communication: say "N decisions pending"; give them in full when asked.

## Hard-won facts
- Builds through shared `/secondary/Programming/Github/kabl-target`, every cargo under `systemd-run --user --scope -p MemoryMax=10G -p MemorySwapMax=2G`, tests per crate -j 2. `cargo kabl test` does not exist; use `cargo +1.99.0 test -p <crate>`. Stale kabl_core errors = another worktree's artifacts: `touch crates/core/src/lib.rs`.
- `nonslop slice verify` has no gates (slop, scope, master merged). CI is the test gate. slopstop blocks test names with v8/v9/v10.
- `nonslop status` lists ~200 legacy `claude/*` branches with "no PR": ignore; filter with `grep -E 'record (open|released|blocked)|PR #[0-9]+ open|DIRTY|UNPUSHED'`.
- `nonslop slice new` reads the stale root checkout ("paths do not exist": harmless). Declared-only overlap: check the real diff, then `--force`.
- `gh pr checks --watch` right after a push exits early: wait until checks exist. GitHub: PR body `gh api -X PATCH .../pulls/N -F body=@file`; update branch `gh api -X PUT .../pulls/N/update-branch`; drafts need `gh pr ready`.
- Reviewers have no SendMessage: their report comes as handback; a resumed reviewer can re-review a new head (cheap).
- Captures: `xvfb-run -a -s "-screen 0 1440x900x24" cargo run --release -p kabl-tools --example chrome_capture -- OUT scene`. Silent.
- Subagent reports can be lost after one SubagentHandback: ask it to answer via SendMessage. A session end kills running builders: worktree stays DIRTY; resume the same agent, not a fresh dispatch. Root checkout `master` is stale.
