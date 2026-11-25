# State 2026-11-24 (night)

## Goal
Make kabl easy to start, learn, understand and reuse, and bring it to Windows through VST3, without losing musical depth. Plan: `docs/ROADMAP.md` (canonical). Release 1.0 on Linux first. Composite faces plan: `docs/composites/FACES.md`.

## Done (11-24)
L1 #209, CI2 #212, W1a #213, CI3 #215, CI4 #216, W2 #217, L2 #218, W3 #219, W4 #220, L4 #221, face labels #223, face design v1 #224, W5a clap-validator #225, R3 rc.2 install #227, Windows test fixes #229 #232 #234, Acid Line #230, Space composite #214, state #233, Pluck line macros (L3a) #231, face decisions D1-D8 #228, LF line endings #235, golden hashes Linux-only #237, macro trim + Perform header tools #236, L3b first run as lesson 00 #240, Windows test fixes #239 (corpus names "/") #241 (record test paths) #242 (imported-morph wav within one 16-bit step off Linux), owner config #226, L3c Sound-check strip #244, Sounds ▶ preview #243, plugin Start card #245.

## In flight
- Composite faces slice 1, stored form v10: `claude/faces-stored-form` (builder). Scope widened to editor.rs (prune in `PatchEditor::append`) and struct-literal fixes in code_examples.rs, clap sound_state.rs, engine tests/code_scope.rs, ui tests/composite_face.rs, lib.rs test literal.
- #203 promo v2: parked.

## Next
1. Composite faces: slices 2+ from FACES.md §9 after slice 1 merges (slider ships vertical + horizontal for Kosta to pick).
2. Docs: `docs/learning/LESSON-FORMAT.md` lacks `sound_check` (#244) and `plugin_text` (#245) step fields.
4. CB2-CB4 (Drum Kit, Sequenced Part, Glue Bus): Kosta judges each on screenshots.
5. Windows: green `windows` job, then W6 VST3 (clap-wrapper), W7-W10. Kosta tests on a VM later.
6. Layout fixes from R3 at 1280x800: status bar clipped, hint card over top-right text; README: REAPER scans `~/.clap`.
7. Follow-ups: L3c minors (strip output pick remembered before the stream opens; `Choice::Unset` = never chosen and "System default"; plugin first-run claim is read-then-write, not atomic; stale "Preview when selected" text in docs/pivot/firstrun-learning-design.md:75); L3b minors (lessons.rs `satisfy` arms for AnyParamMoved/SoundsOpen never run; `any_param_moved.by` untested; tour comments in kit/g4.rs:48, keys.rs:215, library.rs:159; `Sev::Info` unused; duplicate Next/Skip when Learn drawer open over Start card); `perform_view.rs:671-685` duplicates `kit::icon_menu`; `file_browser.rs:390` fallback `/` vs `C:\`; engine golden hashes Linux-only once engine tests run on Windows; `first_run.rs:188` cutoff check counts only env cables; CLAP host macro automation never reaches the editor; `code/install.rs:118` rename via `kabl_core::fsutil`; arp/`midi.note` ignore note source; CLAP sound_state v9 signalling; print child stderr in `composites_factory.rs:93`.

## Waiting on owner
- Taste: anchor ring covers Cutoff label/readout and clips "Perform" (#240 captures); Perform Macros "? …" placement; slider look; CB2-CB4 screenshots.

## Decisions (11-24, Kosta)
- L3c: no-output card wording, Sound-check strip look, plugin step 1 text accepted; Play hidden on the plugin Start card (11-24).
- D1 start in the simple patch; D3 ▶ previews and opens; D7 Pluck line Brightness + Space macros resting at 0; OD-1 Acid Line accepted; OD-C Space approved, composites judged on screenshots; OD-W1 VM later; OD-W2 Live trial later; OD-W3 unsigned first.
- Composite faces F1-F8 as recorded in FACES.md.
- Perform Macros "? …" moved to header, framed, ink colour (#236).
- Bit-identical hashes are a Linux reference; other platforms assert a clean render, or within one 16-bit step for generated wavetables (#237, #242).
- Coordinator merges itself once verify, review (current head) and CI are green (`[merge] coordinator = "auto"`). Gate-file PRs go to Kosta.
- Agents may edit CI and download tools; gate-file edits still need Kosta (hook); the contents API route is used only on his explicit OK.
- Communication: say "N decisions pending"; give them in full when asked.

## Hard-won facts
- Builds serial through shared `/secondary/Programming/Github/kabl-target`, every cargo under `systemd-run --user --scope -p MemoryMax=10G -p MemorySwapMax=2G`. Parallel builders share target: a doctest can link another worktree's kabl_core and fail locally (seen 11-24 on #245); CI builds clean. Rely on CI for full tests.
- Every merge moves master: update-branch, re-verify, re-review (verdicts are per sha) on every other open PR.
- `nonslop slice verify` here has no gates: it checks slop, scope and master merged only. CI is the test gate.
- Brief `Scope In` globs like `crates/ui/tests/**` block later slices; `--force` only after checking the in-flight branch leaves the file alone.
- `gh pr checks --watch` right after a push can exit early: wait until a check is pending first.
- Windows job runs `cargo test --locked -p kabl-ui -p kabl-clap -p kabl-standalone` on windows-latest; it stops at the first failing test binary.
- Windows cross-check: stable `cargo check --target x86_64-pc-windows-gnu -p kabl-ui --tests` with `ZSTD_SYS_USE_PKG_CONFIG=1 PKG_CONFIG_ALLOW_CROSS=1`.
- GitHub: PR body via `gh api -X PATCH .../pulls/N -F body=@file`; update branch `gh api -X PUT .../pulls/N/update-branch`; drafts need `gh pr ready`.
- Subagent reports can be lost after one SubagentHandback: ask it to answer via SendMessage. A builder must never drive xdotool without `--window`. Root checkout `master` is stale.
