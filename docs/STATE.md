# State 2026-11-27

## Goal
Make kabl easy to start, learn, understand and reuse, and bring it to Windows through VST3, without losing musical depth. Plan: `docs/ROADMAP.md` (canonical). 1.0 = Linux + Windows, CLAP + VST3 + standalone, with faces (FACES.md slices 5-11), composites CB2-CB5 and lessons L5-L8; macOS 1.1 (Kosta 11-25/11-26). Composite faces plan: `docs/composites/FACES.md`.

## Done
- 11-24: L1 #209, CI2 #212, W1a #213, CI3 #215, CI4 #216, W2 #217, L2 #218, W3 #219, W4 #220, L4 #221, face labels #223, face design v1 #224, W5a clap-validator #225, R3 rc.2 install #227, Windows test fixes #229 #232 #234 #239 #241 #242, Acid Line #230, Space composite #214, L3a #231, face decisions #228, LF endings #235, golden hashes Linux-only #237, Perform header tools #236, L3b #240, owner config #226, L3c: Sound-check strip #244, Sounds ▶ preview #243, plugin Start card #245.
- 11-25: state #246, faces slice 1 stored form v10 #247, R3 status-bar layout + REAPER README note #248, faces slice 2 composite design page #249, LESSON-FORMAT docs #250, state #251, faces palette drag #252, state #253, faces slider #254 (vertical default per Kosta), faces toggle #255, Windows settings-save race fix #257 (real data-loss bug: save could drop a running code module's settings).
- 11-26: settings note quiet #258, faces follow-ups #256, 1.0 decisions #260, faces slice 5 icon selector #261, faces slice 6 displays #262 (70349261), state #263, gate unlock #265 (Kosta).
- 11-27 (overnight, Kosta: merge everything, review later): W6 VST3 #264, faces slice 7 buttons #266, W7 VST3 params/MIDI #267 (VST3 played no notes before: fixed), CB2 Drum Kit #268, L5 lessons 1-3 #269, LESSON-FORMAT docs #270, CB3 Sequenced Part #271.

## In flight
- CB4 Glue Bus (`claude/cb4-glue-bus`), W8 Windows editor (`claude/w8-editor`): builders running; briefs in root .claude/tmp/coord1126/.
- #203 promo v2: parked.

## Next
1. Kosta runs docs/ACCEPTANCE-V1.md on rc.2 (his pick, 11-25); fix findings (check each against master first).
2. Windows/VST3 for 1.0 (OD-W4..W9 decided): W8 editor DPI/focus (main risk), W9 Inno Setup installer, W10 release job (+ VST logo guidelines check), WA Kosta VM pass. ASIO: first check the ASIO SDK licence against GPL-3.0-or-later (unverified), then W11a (standalone ASIO, required for 1.0).
3. Faces slices 8-11 (FACES §9) in order; 8 and L6 touch crates/clap (after W8). Briefs ready: faces 8 (brief-faces-morph.md).
4. CB5 Analog Voice, then L6, then L7+L8 as one slice (D5), all required for 1.0. W11a ASIO (licence OK: GPLv3 option; Windows binary effectively GPLv3-only).
5. Before tagging v1.0.0: rc.3 from master + short re-check of first run / Perform / design page (proposed, Kosta to confirm).
6. Follow-ups: #262 minors (face_display.rs comment above the OSC tap assert describes the Output meter; `displays::has_display` kind list not tied to the `draw` dispatch by a test; Displays list buttons 160 vs 236 wide, ragged); #252 minors (Esc cancelling an element move/resize has no test; palette drag clears only when the face draws again); #249 minors (no snapshot list UI yet: slice 9; removed-composite canvas state has no capture; docs/design/redesign/g4/SWEEP.md mentions dev-tools scene); #248 minors (status_bar test copies frame.rs toolbar sizes; geometry asserts not shown failing on master; left side not elided below 1180 px); #247 Icons minimum flat 36x55 until the Icons slice (FACES §5.1: 18/option); L3c minors (strip pick remembered before stream opens; `Choice::Unset` = never chosen and "System default"; plugin first-run claim read-then-write); L3b minors (lessons.rs `satisfy` arms AnyParamMoved/SoundsOpen never run; `any_param_moved.by` untested; tour comments kit/g4.rs:48, keys.rs:215, library.rs:159; `Sev::Info` unused; duplicate Next/Skip with Learn drawer over Start card); `perform_view.rs:671-685` duplicates `kit::icon_menu`; `file_browser.rs:390` `/` vs `C:\`; `first_run.rs:188` cutoff check counts only env cables; CLAP host macro automation never reaches the editor; `code/install.rs:118` rename via `kabl_core::fsutil`; arp/`midi.note` ignore note source; CLAP sound_state v9 signalling; child stderr in `composites_factory.rs:93`.

## Waiting on owner
- Morning review list: root .claude/tmp/coord1126/kosta-review.md (captures, composite interface questions, VST3 id notation, validator allowlist, legacy input passthrough, lesson deviations).
- R4: acceptance pass on rc.2.
- rc.3 for a short re-check before v1.0.0 (proposed).

## Decisions (Kosta)
- 11-26 night: D5 = lessons 4+5 built around composites and show they are optional (L7 joins L8 after CB3+CB5); composites merge without waiting (OD-C judged after); ASIO GPLv3-only Windows binary OK; gate-tampered disabled until production (#265). Coordinator (delegated): VST3 bundle layout; validator allows one failure (Process Format 1234567.8 Hz, CLAP refuses >768 kHz).
- 11-26: OD-W4 Inno Setup + zip; OD-W5 freeze VST3 id bda9c239b42389589c969403a7b9d23e; OD-W6 use the VST compatible logo (W10 checks Steinberg guidelines); OD-W7 ship kabl.clap; OD-W8 ASIO in the first Windows release (standalone; W11a required); OD-W9 native MSVC; macOS 1.1; faces, composites, lessons all in 1.0.
- 11-26: merge policy is `[merge] coordinator = "auto"` in .nonslop.toml.
- 11-25: 1.0 includes Windows + VST3 (Ableton). Vertical face slider is the default (horizontal stays in Settings). Settings note: option A (quiet when no audio / on open, #258). Acceptance on rc.2, not rc.3.
- 11-25: ONE builder at a time (shared target dir; parallel worktree builds swap kabl_core artifacts and give false failures). Use the time: run non-building work alongside (reviews, briefs, docs-only slices with "no cargo", research). Renders compile, so they queue behind the builder.
- 11-25: design page look OK; add drag-from-palette; anchor ring not clipping, drop it; Perform Macros "?"/"…" placement OK.
- 11-24: L3c no-output wording, Sound-check strip look, plugin step 1 text accepted; Play hidden on plugin Start card. D1 start in the simple patch; D3 ▶ previews and opens; D7 Pluck line Brightness + Space macros at 0; OD-1 Acid Line; OD-C Space, composites judged on screenshots; OD-W1 VM later; OD-W2 Live trial later; OD-W3 unsigned first. Faces F1-F8 in FACES.md.
- Bit-identical hashes are a Linux reference; other platforms assert a clean render, or within one 16-bit step for generated wavetables (#237, #242).
- Coordinator merges itself once verify, review (current head) and CI are green. Gate-file PRs (CI, .nonslop.toml) go to Kosta; the merge hook refuses them for the coordinator.
- Communication: say "N decisions pending"; give them in full when asked.

## Hard-won facts
- 11-25: shared target dir cleared by coordinator at 7 GB free (152 GB freed); builds cold after that. Workspace tests -j 4 OOM under the 10G cap: builders test per crate -j 2. `gh run rerun --failed` returned HTTP 500 repeatedly 11-25: update-branch instead to get a fresh run.
- Builds through shared `/secondary/Programming/Github/kabl-target`, every cargo under `systemd-run --user --scope -p MemoryMax=10G -p MemorySwapMax=2G`, tests `-j 4` (-j 8 workspace test OOM-killed 11-25). Stale kabl_core errors ("no field snapshots") = another worktree's artifacts: `touch crates/core/src/lib.rs` and rebuild. `cargo kabl test` does not exist; builders use `cargo +1.99.0 test --workspace`.
- Every merge moves master: update-branch, re-verify, re-review (verdicts are per sha) on every other open PR.
- `nonslop slice verify` has no gates: slop, scope, master merged only. CI is the test gate. slopstop blocks test names with v8/v9/v10 ([versioned-name]).
- `nonslop slice new` warns "paths do not exist" because it reads the stale root checkout; harmless. Widen a running slice's scope by editing the brief and `slice new --force`.
- `gh pr checks --watch` right after a push can exit early: wait until a check is pending first. Old watchers on superseded heads fire stale notices; check the current head.
- Windows job: `cargo test --locked -p kabl-ui -p kabl-clap -p kabl-standalone` on windows-latest; stops at the first failing test binary.
- GitHub: PR body via `gh api -X PATCH .../pulls/N -F body=@file`; update branch `gh api -X PUT .../pulls/N/update-branch`; drafts need `gh pr ready` before merge.
- Captures: `tools/examples/chrome_capture.rs` scenes incl. first-run*, perform, sounds-preview, design-page, design-page-red-drop; `xvfb-run -a -s "-screen 0 1440x900x24" cargo run --release -p kabl-tools --example chrome_capture -- OUT scene`. Silent.
- Subagent reports can be lost after one SubagentHandback: ask it to answer via SendMessage. A session end kills running builders: their worktree stays DIRTY; resume the same agent, not a fresh dispatch. Builders never drive xdotool without `--window`. Root checkout `master` is stale.
