# State 2026-11-24 (evening)

## Goal
Make kabl easy to start, learn, understand and reuse, and bring it to Windows through VST3, without losing musical depth. Plan: `docs/ROADMAP.md` (canonical). Release 1.0 on Linux first. Composite faces plan: `docs/composites/FACES.md`.

## Done (11-24)
L1 #209, CI2 #212, W1a #213, CI3 #215, CI4 #216, W2 #217, L2 computer keys #218, W3 file browser #219, W4 safe save #220, L4 macro reveal #221, state #222, face labels #223, face design v1 #224, clap-validator W5a #225 (0 failures; Windows validator job in `docs/windows/CI-JOB.md`), R3 rc.2 clean install #227 (all 8 steps pass), Windows test fixes #229 #232, Acid Line 16 %/8 % (OD-1) #230, Space composite #214.

## In flight (open PRs)
- #226 owner config (`auto_clean = false`, Windows CI job): gate files, so **Kosta merges it**. Windows job re-running after #232; if it fails, fix the next test (pattern so far: Unix-only tests, `\` vs `/` in names), then update-branch #226.
- #228 face design decisions: review round 3 found 3 must-fix (morphing-map lifetime, land-table overflow, prune at `PatchEditor::append`); builder fixing, also adding Kosta's ask: compact composite face with an expandable "+N" part like module faces. Then re-review, merge.
- #231 Pluck line macros (L3a): builder reworking per Kosta: macros rest at 0 (knobs keep real values), trim macro module to the 2 used slots. Re-verify, re-review, merge. Check: capture showed 124 BPM in status bar vs clock 112.
- #203 promo v2: parked (workstream E).

## Next
1. L3 first run (lesson 00, start in Rack with Pluck line playing, Start card, sound-check strip, plugin card, delete tour.rs). D1 = start in the simple patch. D3 = row ▶ previews and opens the sound's card; clicking a row stays silent (my reading of Kosta's "play button which opens and previews"; confirm on captures).
2. Composite faces: build slices from FACES.md section 9 once #228 merges (slider ships vertical + horizontal for Kosta to pick).
3. CB2-CB4 (Drum Kit, Sequenced Part, Glue Bus): Kosta judges each on screenshots like Space.
4. Windows: green Windows job, then W6 VST3 (clap-wrapper), W7-W10. Kosta tests on a VM later, all at once.
5. Layout fixes from R3 at 1280x800: status bar clipped/overlapping, hint card over top-right text, "Preview when selected" truncated; README should mention REAPER scans `~/.clap`.
6. Follow-ups: CLAP host automation of macros never reaches the editor (no reveal); `crates/ui/tests/explain.rs:564` `\` in names on Windows; Windows-native stalled-sink test for applog; route `crates/modules/src/code/install.rs:118` rename through `kabl_core::fsutil`; arp/`midi.note` ignore note source; CLAP sound_state v9 signalling; print child stderr in `crates/ui/tests/composites_factory.rs:93`.

## Waiting on owner
- Merge #226 when its CI is green (only Kosta can merge gate files).
- Taste: slider look (on captures), L3 first-run captures, CB2-CB4 screenshots.

## Decisions (11-24, Kosta)
- Start in a simple patch (D1). Browser: ▶ previews and opens (D3). Pluck line gets Brightness + Space macros, resting at 0, module trimmed (D7). Acid Line fixed and accepted (OD-1). Space approved; composites judged on screenshots (OD-C). Windows: VM only, tested later all together (OD-W1); Live trial later (OD-W2); unsigned first, cheapest signing later: SignPath free, Certum ~69 EUR (OD-W3).
- Composite faces F1-F8: snapshots with jump/morph (B); slider judged on captures; built-in + PNG icons (B); one simple format step, no users yet; recursive targets into nested composites; Pin to Perform (B); whole blocks (B); a composite design page.
- Agents may edit CI and download tools (clap-validator); gate-file edits still need Kosta (hook).
- Communication: say "N decisions pending" only; give them in full, plain language, when Kosta asks.
- Coordinator merges itself once verify, review and CI are green. Hooks still bind.

## Hard-won facts
- Builds serial through shared `/secondary/Programming/Github/kabl-target`, every cargo under `systemd-run --user --scope -p MemoryMax=10G -p MemorySwapMax=2G`. Until #226 merges, finishing builders still `cargo clean` it; rely on CI for full workspace tests.
- Merge hook wants verify at the current head and the branch containing master: after every update-branch, re-verify (and re-review: verdicts are per sha).
- `gh pr checks --watch` started right after a push can exit before checks register: re-run it.
- Windows cross-check: stable `cargo check --target x86_64-pc-windows-gnu -p kabl-ui -p kabl-standalone -p kabl-clap` with `ZSTD_SYS_USE_PKG_CONFIG=1 PKG_CONFIG_ALLOW_CROSS=1`.
- GitHub: PR body via `gh api -X PATCH .../pulls/N -F body=@file`; update branch `gh api -X PUT .../pulls/N/update-branch`; drafts need `gh pr ready`.
- Flaky: `code_examples::module_settings_survive_save_and_reopen_through_the_real_path`.
- A builder must never drive xdotool without `--window`: one typed into Kosta's terminal on 11-24.
- Root checkout `master` is stale; use `origin/master`.
