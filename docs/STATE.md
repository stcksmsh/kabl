# State 2026-11-24 (evening)

## Goal
Make kabl easy to start, learn, understand and reuse, and bring it to Windows through VST3, without losing musical depth. Plan: `docs/ROADMAP.md` (canonical). Release 1.0 on Linux first. Composite faces plan: `docs/composites/FACES.md`.

## Done (11-24)
L1 #209, CI2 #212, W1a #213, CI3 #215, CI4 #216, W2 #217, L2 #218, W3 #219, W4 #220, L4 #221, face labels #223, face design v1 #224, W5a clap-validator #225, R3 rc.2 install #227, Windows test fixes #229 #232 #234, Acid Line #230, Space composite #214, state #233, Pluck line macros (L3a) #231, face decisions D1-D8 #228, LF line endings #235, golden hashes Linux-only #237, macro trim + Perform header tools #236, L3b first run as lesson 00 #240, Windows test fixes #239 (corpus names "/") #241 (record test paths) #242 (imported-morph wav within one 16-bit step off Linux).

## In flight
- #226 owner config: `auto_clean = false`, Windows CI job, `[merge] coordinator = "auto"`, retired Caveman/Ponytail note dropped. Gate files: **Kosta merges**. CI rerun after #242. If `windows` fails again: read the full job log (`gh api repos/stcksmsh/kabl/actions/jobs/<job>/logs`, `--log-failed` truncates), slice the next test fix (pattern: Unix-only paths, platform libm floats, path separators).
- #203 promo v2: parked.

## Next
1. After #226 merges: render L3b `first-run-no-output` capture (chrome_capture scene exists) for Kosta to confirm the card wording.
2. L3c: Sound-check strip (remembers output device + MIDI input, `style::Prefs`), card on first plugin instance (D2 A, never auto-plays), browser row ▶ previews and opens the sound's card while clicking a row stays silent (D3), `docs/ACCEPTANCE-V1.md` §1.
3. Composite faces: build slices from FACES.md §9 (slider ships vertical + horizontal for Kosta to pick).
4. CB2-CB4 (Drum Kit, Sequenced Part, Glue Bus): Kosta judges each on screenshots.
5. Windows: green `windows` job, then W6 VST3 (clap-wrapper), W7-W10. Kosta tests on a VM later.
6. Layout fixes from R3 at 1280x800: status bar clipped, hint card over top-right text, "Preview when selected" truncated; README: REAPER scans `~/.clap`.
7. Follow-ups: L3b minors (lessons.rs `satisfy` arms for AnyParamMoved/SoundsOpen never run; `any_param_moved.by` untested; tour comments in kit/g4.rs:48, keys.rs:215, library.rs:159; `Sev::Info` unused; duplicate Next/Skip when Learn drawer open over Start card); `perform_view.rs:671-685` duplicates `kit::icon_menu`; `file_browser.rs:390` fallback `/` vs `C:\`; engine golden hashes Linux-only once engine tests run on Windows; `first_run.rs:188` cutoff check counts only env cables; CLAP host macro automation never reaches the editor; `code/install.rs:118` rename via `kabl_core::fsutil`; arp/`midi.note` ignore note source; CLAP sound_state v9 signalling; print child stderr in `composites_factory.rs:93`.

## Waiting on owner
- Merge #226 when green.
- Taste: anchor ring covers Cutoff label/readout and clips "Perform" (#240 captures); Perform Macros "? …" placement; slider look; CB2-CB4 screenshots.

## Decisions (11-24, Kosta)
- D1 start in the simple patch; D3 ▶ previews and opens; D7 Pluck line Brightness + Space macros resting at 0; OD-1 Acid Line accepted; OD-C Space approved, composites judged on screenshots; OD-W1 VM later; OD-W2 Live trial later; OD-W3 unsigned first.
- Composite faces F1-F8 as recorded in FACES.md.
- Perform Macros "? …" moved to header, framed, ink colour (#236).
- Bit-identical hashes are a Linux reference; other platforms assert a clean render, or within one 16-bit step for generated wavetables (#237, #242).
- Coordinator merges itself once verify, review (current head) and CI are green (`[merge] coordinator = "auto"`). Gate-file PRs go to Kosta.
- Agents may edit CI and download tools; gate-file edits still need Kosta (hook); the contents API route is used only on his explicit OK.
- Communication: say "N decisions pending"; give them in full when asked.

## Hard-won facts
- Builds serial through shared `/secondary/Programming/Github/kabl-target`, every cargo under `systemd-run --user --scope -p MemoryMax=10G -p MemorySwapMax=2G`. Until #226 merges, master's `.nonslop.toml` has `auto_clean = true`: every builder's Stop hook (worktrees read master's copy, not the root's untracked one) runs `cargo clean` on the shared target and breaks other builds. Rely on CI for full tests.
- Every merge moves master: update-branch, re-verify, re-review (verdicts are per sha) on every other open PR.
- `nonslop slice verify` here has no gates: it checks slop, scope and master merged only. CI is the test gate.
- Brief `Scope In` globs like `crates/ui/tests/**` block later slices; `--force` only after checking the in-flight branch leaves the file alone.
- `gh pr checks --watch` right after a push can exit early: wait until a check is pending first.
- Windows job runs `cargo test --locked -p kabl-ui -p kabl-clap -p kabl-standalone` on windows-latest; it stops at the first failing test binary.
- Windows cross-check: stable `cargo check --target x86_64-pc-windows-gnu -p kabl-ui --tests` with `ZSTD_SYS_USE_PKG_CONFIG=1 PKG_CONFIG_ALLOW_CROSS=1`.
- GitHub: PR body via `gh api -X PATCH .../pulls/N -F body=@file`; update branch `gh api -X PUT .../pulls/N/update-branch`; drafts need `gh pr ready`.
- Subagent reports can be lost after one SubagentHandback: ask it to answer via SendMessage. A builder must never drive xdotool without `--window`. Root checkout `master` is stale.
