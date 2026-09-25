# D01 — structured report

```text
Batch ID / outcome: D01 — Find, play and save a sound. Open kabl, find a sound in a
  Factory / Your Sounds browser, audition it without a controller, change its existing
  controls, Save As a personal sound and find it again.
Starting commit: 30b9abd (origin/master at start). Contains the expected planning commit
  c9aac0ba0dc80272ca4628d8a8c6bbd6fd138c96; the two intervening commits (09d4950, 30b9abd)
  only add .claude/skills/{caveman,ponytail} and their licenses — no product/doc change.
Submitted head commit: the commit that adds this REPORT.md on the branch below (see the PR
  for its SHA). Tests/clippy ran at 7030b74; commits after it change REPORT.md/README.md only.
  Product code unchanged since f3bebd9 (real-app evidence build) except crates/ui/tests.
Branch / pushed remote / PR if required: claude/install-caveman-ponytail-bhigco on origin
  (the cloud container required this branch). PR to master for Kosta to merge: https://github.com/stcksmsh/kabl/pull/2.
  Not merged by the agent.
Engineering status: submitted — built, awaiting Kosta's review.
Owner-review status: pending. (Composition + Motion and Sound Palette reviews: still pending,
  explicitly deferred by Kosta for D01, not approved. D00 not complete.)
```

## What now works (user-facing)

- Launch without `--patch` → **Sounds** panel: Factory + Your Sounds, search (all words over
  name/category/tags/description), category menu, ★ favorites, Recent (missing entries shown
  with Forget). Rows say **Keys / Sequence / Seq + Keys**.
- 15 factory sounds incl. a new **Init Keyboard**, the six palette voices, the Composition /
  Sound Palette / Performance / Interlocking / Echo / Sequence pieces, two rack studies.
- Browsing is silent. **Open** loads (parse + compile first; failures leave the rack alone).
- Audition: note (−8/+8 octaves), Note/Major/Minor, velocity, length (≤ 4 s UI, 10 s engine
  cap), Play/Stop, with a line saying exactly what plays. Pieces open **stopped**; Start/Stop.
- Existing rack and Perform controls edit the sound. Toolbar: name with ● when unsaved,
  **Save** (Save As for factory/new), **Save As** (name, category, tags), Ctrl+S; **Rename**;
  **New** (from Init Keyboard). Open/New/quit ask Save / Don't save / Cancel.
- Relocatable Linux package; user data in `~/.local/share/kabl/`.

## Acceptance matrix

| Criterion | Result | Evidence |
|---|---|---|
| Discover voices and pieces (every factory entry discoverable, categorized, loadable; search/favorite/recent persistence) | pass (engineering) | `crates/ui/tests/library.rs` `every_factory_sound_is_discoverable_categorized_and_loads`, `search_matches…`, `favorites_and_recents_persist…`; `img/walkthrough/w01,w02,w10,w11`; owner "find a category without help": **unverified** |
| First sound (start/stop preview and piece transport visible; no sound from focus navigation) | pass (scripted) | `walkthrough.mp4` 0–17 s silent while browsing; `evidence/walkthrough-audio-timeline.txt`; `browser.rs` `browsing_never_makes_sound_or_loads`, `a_piece_opens_stopped…`; 60-second owner task: **unverified** (formative, Kosta's) |
| Keep edits (modify control, Save As, reopen, factory unchanged) | pass | walkthrough 43–66 s, 99–111 s; `w04,w06,w09`; `browser.rs` `edit_save_as_reopen_and_the_factory_file_stays`; `library.rs` `save_as_reopen_rename_and_the_factory_file_is_unchanged` (byte compare) |
| Unsaved protection (load/new/quit, cancel, save failure, undo to saved) | pass (engineering) | `browser.rs` `unsaved_edits_ask_and_cancel_keeps_them_exactly`, `save_from_the_question_then_continues`, `quit_with_unsaved_changes_asks`, `a_failed_save_keeps_the_work_and_the_question`; `w05`, `img/*-light-unsaved-question.png`; real window-close path exercised only through `request(Quit)` in tests, not by closing the Xvfb window: **quit via window manager unverified** |
| Preview ownership (repeated previews, overlapping controller, sustain, load, device change, no stuck notes) | pass (engine + scripted stand-in) | `crates/engine/tests/preview.rs` (10 tests); walkthrough 119–128 s overlap RMS in `evidence/walkthrough-audio-timeline.txt`; `browser.rs` `losing_window_focus_stops_a_preview`; audio-device change: not applicable (no device switching exists); physical controller feel: **unverified** |
| Failure recovery (malformed, uncompilable, read-only, duplicate name, missing recent) | pass | `library.rs` `malformed_and_uncompilable…`, `a_read_only_user_directory…`, `a_failed_save_leaves_the_old_sound…`, `an_interrupted_save_is_repaired…`, `names_and_directories_never_collide_silently`; `browser.rs` `missing_recents_and_bad_patches_are_explained`, `save_as_onto_a_taken_name_offers_replace` |
| Portability (installed package outside repo; factory discovery and user save) | pass (cloud) | `packaging/linux/package.sh`; `scripts/portability.sh`; `evidence/portability-run.txt`; `img/p01,p02` |
| Layout (screenshots + walkthrough at 1440×900, 1280×800, A-light/A-dark) | pass (scripted) | `img/1440x900-{light,dark}.png`, `img/1280x800-{light,dark}.png`, dialogs; `browser.rs` `every_control_is_reachable_at_both_sizes`; walkthrough is 1440×900 only |
| Legacy behaviour (save/load/undo/keyboard regressions, suite, clippy, old patches) | pass | `evidence/test-workspace.txt` (444/0/14), `evidence/clippy.txt`; `library.rs` `old_patch_directories_without_metadata_are_listed`; all 14 existing patch dirs unchanged except an added `sound.toml` |

## Changes

- `crates/engine/src/patch_engine.rs` — `Command::Preview`/`PreviewStop`, preview block
  countdown, controller/preview key masks; reset on All Notes Off and fresh graphs.
- `crates/ui/src/library.rs` (new) — factory/user discovery, `sound.toml` `Meta`, identity,
  staged save + repair, rename, favorites/recents, factory-path check.
- `crates/ui/src/browser.rs` (new) — Sounds panel, `Doc` + unsaved state, audition,
  Start/Stop, toolbar Save/Save As, dialogs, New.
- `crates/ui/src/lib.rs` — `UiState` fields, panel/dialog wiring, toolbar (path field moved
  into the browser), undo shortcuts off while a dialog is open.
- `crates/ui/src/main.rs` — opens the library, doc for `--patch`, pieces open stopped,
  quit guard, location log line, sample rate for previews.
- `crates/ui/Cargo.toml` — `serde`, `serde_json`, `toml` (already in the lockfile).
- Tests: `crates/engine/tests/preview.rs`, `crates/ui/tests/{library,browser}.rs`; updated
  `interaction.rs` (folder load answers the unsaved question).
- `patches/init-keyboard/` (new), `patches/**/sound.toml` (new metadata).
- `packaging/linux/package.sh`; `docs/rack-migration/drive.py` (`KABL_BIN`, `-`, `fill`,
  folder save via the browser); `docs/find-play-save/**`.
- Docs: `docs/decisions.md`, `HANDOFF.md`, `STATUS.md`, `PRODUCT-PLAN.md` ledger, D01 brief
  status line, root `README.md` (21 module kinds, running/packaging).

## Decisions

`docs/decisions.md`, "2026-09-25 — D01 Find, play and save a sound": sidecar `sound.toml`
(rejected: database/index, tags in the log); directory identity (rejected: name identity);
fixed category vocabulary, keys/sequence derived from the patch; XDG data locations and
exe-relative factory lookup (rejected: `~/.config`, embedded patches); staged save with
read-back and rename swap, no fsync (documented); parse+compile before load; unsaved =
state inequality (rejected: log-entry counters, broken by coalescing); audio-thread bounded
preview with per-source ownership (rejected: UI-timed note-offs, separate preview voice);
pieces open stopped from the browser only.

## Verification

- `cargo test --workspace` at 7030b74 (Ubuntu 24.04.4 container, 4 vCPU, rustc/cargo
  1.94.1): exit 0, **444 passed, 0 failed, 14 ignored** — `evidence/test-workspace.txt`.
- `cargo clippy --workspace --all-targets` at 7030b74: exit 0, 0 warnings — `evidence/clippy.txt`.
- Also at f3bebd9 (evidence build): 443 passed / 0 failed / 14 ignored, clippy clean
  (7030b74 adds one test).
- Formatting: `rustfmt` on changed files only (`library.rs`, `browser.rs`, `main.rs`,
  `patch_engine.rs`, the new/changed tests). `lib.rs` was not clean under rustfmt at the
  start, so it was edited by hand and not reformatted.

## Real-app evidence

Scripted (xdotool on Xvfb), installed package built at f3bebd9, PipeWire null sink recorded
from its monitor, controller = `examples/midi_player` via `KABL_MIDI_PIPE` fifo (stand-in).
`walkthrough.mp4` (2:18, audible, 1440×900, 48 kHz/256); `img/walkthrough/w01…w11`;
`img/{1440x900,1280x800}-{light,dark}.png` + dialog shots; `img/p01,p02`;
`evidence/walkthrough-{drive.log,t0.txt,audio-timeline.txt,app.log,callback-stats.txt}`,
`evidence/portability-run.txt`. Reproduction commands: README "Real-app evidence".

## Measurements

48 kHz, 256 frames asked and delivered, walkthrough take (~140 s, 26 272 callbacks), cloud VM,
audio thread SCHED_FIFO 80 via `chrt` (the app's rtkit request was refused: no system bus).
Execution: worst 8 525 µs / 5 333 µs budget, 18 over half, 2 late. Arrival: worst 49 060 µs,
49 late. Xruns: 5. No stream stall. Not laptop evidence; the Composition spike and Sound
Palette stall questions remain open.

## Compatibility

Old patches: format unchanged, `sound.toml` optional; every existing patch loads (library
test + suite); `--patch` launches unchanged (clocks run). Undo: an ordinary op log; favorites,
recents, previews, Start/Stop never enter it; undo back to the saved state clears ●. Sound:
no module or patch-parameter change; factory log files byte-identical. Keyboard: previews use
the same keyboards; POLY/MONO/LEGATO, glide, sustain, All Notes Off, MIDI reconnect unchanged
(existing `keyboard.rs` tests pass). Transport: Start/Stop = existing Run/Stop commands.
Plugin automation: not applicable (no plugin yet).

## Limits / deviations

- All real-app evidence is scripted in a cloud container with a fifo MIDI stand-in; no
  hardware, laptop, speakers or beginner observation. The 60-second task is formative and
  Kosta's.
- Walkthrough only at 1440×900; 1280×800 covered by screenshots and the reachability test.
- Quitting through the window manager's close was not driven in the real app (logic tested
  through `request(Quit)` and `quit_now`).
- No fsync; no multi-instance write protection. No delete/duplicate/import/export of user
  sounds. The sustain pedal holds released preview notes.
- Commits went to the environment-mandated branch, not straight to master.
- Factory names/categories/descriptions are the agent's wording.

## Owner checklist

`docs/find-play-save/CHECKLIST.md` (package build, install to `~/opt`, run outside the
checkout at 48 kHz/256; all boxes unchecked).

## Follow-ups (not implemented)

- The window-close path (`close_requested` → CancelClose) should be exercised once on the
  laptop's window manager (Wayland/X11) — small, checklist item.
- Delete/duplicate for Your Sounds — likely needed once people save many sounds; small.
- At 1280×800 with browser + drawer + Perform open, the rack is ~620×300 px; a "collapse
  others when browsing" behaviour could help — UI polish, D02-adjacent.
- Init Keyboard's envelope cable crosses the envelope's title in the rack — cosmetic
  placement in `write_factory_library`.
- The engine's 10 s preview cap is a constant; revisit if long-release pads need longer.

## Repo status

Clean working tree at submission; all work pushed. `.ai/` does not exist in this checkout
and was not created or touched. Unrelated files untouched (the two skill commits predate
this batch; historical batch evidence unchanged; `drive.py` changed only as listed).
