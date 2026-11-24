# State 2026-11-24 (late)

## Goal
Make kabl easy to start, learn, understand and reuse, and bring it to Windows through VST3, without losing musical depth. Plan: `docs/ROADMAP.md` (canonical). Release 1.0 on Linux first. Composite faces plan: `docs/composites/FACES.md`.

## Done (11-24)
L1 #209, CI2 #212, W1a #213, CI3 #215, CI4 #216, W2 #217, L2 #218, W3 #219, W4 #220, L4 #221, face labels #223, face design v1 #224, W5a clap-validator #225, R3 rc.2 install #227, Windows test fixes #229 #232 #234, Acid Line #230, Space composite #214, state #233, Pluck line macros (L3a) #231, face decisions D1-D8 #228, LF line endings `.gitattributes` #235, golden hashes Linux-only #237, macro module trim + Perform header tools + capture tempo #236.

## In flight
- L3b first run (`claude/l3b-first-run`, opus builder, started ~15:00): lesson 00 on the lesson engine, start in Rack with Pluck line playing, Start card (no scrim), pass-through anchor ring, delete `tour.rs`. Brief: `.claude/tmp/4045b405/brief-l3b.md`. On DONE: verify, review, captures to Kosta, merge.
- #226 owner config (`auto_clean = false`, Windows CI job): gate files, **Kosta merges**. CI rerun after #235/#237. If `windows` still fails, read the log and slice the next test fix (pattern: Unix-only assumptions, CRLF, absolute float hashes).
- #203 promo v2: parked.

## Next
1. L3c: Sound-check strip (remembers output device + MIDI input, `style::Prefs`), card on first plugin instance (D2 A, never auto-plays), browser row ▶ previews and opens the sound's card while clicking a row stays silent (D3, Kosta's "play button which opens and previews"; confirm on captures), `docs/ACCEPTANCE-V1.md` §1. After L3b merges (both touch lib.rs).
2. Composite faces: build slices from FACES.md §9 (slider ships vertical + horizontal for Kosta to pick).
3. CB2-CB4 (Drum Kit, Sequenced Part, Glue Bus): Kosta judges each on screenshots.
4. Windows: green `windows` job, then W6 VST3 (clap-wrapper), W7-W10. Kosta tests on a VM later, all at once.
5. Layout fixes from R3 at 1280x800: status bar clipped, hint card over top-right text, "Preview when selected" truncated; README: REAPER scans `~/.clap`.
6. Follow-ups: `perform_view.rs:671-685` duplicates `kit::icon_menu`; chrome_capture `KABL_CAPTURE_START=1` still labels the patch "Acid Line"; `file_browser.rs:390` fallback `/` vs Windows `C:\`; engine `golden_renders.rs`/`legacy_patches.rs` absolute hashes (Linux-only rule, needed once engine tests run on Windows); `first_run.rs:188` cutoff check counts only env cables; CLAP host macro automation never reaches the editor; `crates/modules/src/code/install.rs:118` rename via `kabl_core::fsutil`; arp/`midi.note` ignore note source; CLAP sound_state v9 signalling; print child stderr in `composites_factory.rs:93`.

## Waiting on owner
- Merge #226 when green.
- Taste: Perform Macros header has one "? …" pair per macro (framed 28 px buttons, #236 captures); keep, or one pair per card next to "MIDI CC"? Slider look; L3b captures; CB2-CB4 screenshots.

## Decisions (11-24, Kosta)
- D1 start in the simple patch; D3 ▶ previews and opens; D7 Pluck line Brightness + Space macros resting at 0; OD-1 Acid Line accepted; OD-C Space approved, composites judged on screenshots; OD-W1 VM later; OD-W2 Live trial later; OD-W3 unsigned first.
- Composite faces F1-F8 as recorded in FACES.md.
- Perform Macros "? …": "ugly there, should be on top and more legible" -> header, framed, ink colour (#236).
- Bit-identical render hashes are a Linux reference; other platforms assert a clean render (finite, peak, not silent) instead (#237).
- Coordinator merges itself once verify, review (current head) and CI are green. Gate-file PRs go to Kosta.
- Agents may edit CI and download tools; gate-file edits still need Kosta (hook).
- Communication: say "N decisions pending"; give them in full when asked.

## Hard-won facts
- Builds serial through shared `/secondary/Programming/Github/kabl-target`, every cargo under `systemd-run --user --scope -p MemoryMax=10G -p MemorySwapMax=2G`. Until #226 merges, other sessions' Stop hooks `cargo clean` it mid-build: rerun on missing-file errors; rely on CI for full tests.
- Every merge moves master: update-branch, re-verify, re-review (verdicts are per sha) on every other open PR. Merge the one that unblocks most first.
- `gh pr checks --watch` started right after a push can exit early: wait ~30 s first.
- Windows job (on #226) runs `cargo test --locked -p kabl-ui -p kabl-clap -p kabl-standalone` on windows-latest; `$HOME` unset there, autocrlf on (fixed by `.gitattributes`).
- Windows cross-check: stable `cargo check --target x86_64-pc-windows-gnu -p kabl-ui --tests` with `ZSTD_SYS_USE_PKG_CONFIG=1 PKG_CONFIG_ALLOW_CROSS=1`.
- GitHub: PR body via `gh api -X PATCH .../pulls/N -F body=@file`; update branch `gh api -X PUT .../pulls/N/update-branch`; drafts need `gh pr ready`.
- Subagent reports can be lost after one SubagentHandback: ask it to answer via SendMessage.
- A builder must never drive xdotool without `--window`. Root checkout `master` is stale; use `origin/master`.
