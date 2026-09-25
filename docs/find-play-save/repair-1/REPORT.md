# D01-R1 — report

```text
Batch ID / outcome: D01-R1 — saving safety and closeout evidence. Both concerns
  reproduced and fixed; real window-manager close verified in the cloud.
Starting commit: 58e662290f5df69e7b66e5d8e48d6d8e5e583ae3 (origin/master)
Product head (tested, reviewed): b579fb3
Submitted head commit: the docs commit adding this file (PR below has the SHA); after
  b579fb3 only docs/** change.
Branch / pushed remote / PR: claude/install-caveman-ponytail-bhigco (environment-required,
  restarted from 58e6622 after PR #2 merged) → https://github.com/stcksmsh/kabl/pull/4. Not merged by the agent.
Engineering status: submitted
Owner-review status: pending (D01 hands-on checklist not done; Composition + Motion and
  Sound Palette reviews pending)
```

**What now works:** Replace it applies only to the name it was offered for. Folder saves
(Save on a folder document, Save to folder) are staged and roll back, keep the folder's
other files, and say "Nothing was overwritten" only when true. Interrupted saves are
settled on the next open/save. Closing with unsaved work asks, via the real WM path too.

**Acceptance**

| Item | Result | Evidence |
|---|---|---|
| R1 concern | confirmed at base, fixed | `evidence/repro-before.txt`; `crates/ui/tests/browser.rs` `replace_never_targets_a_sound_after_the_name_changed`, `cancel_after_a_collision_keeps_the_other_sound`, `save_as_onto_a_taken_name_offers_replace`; `library.rs` `a_replacement_must_name_the_sound_it_replaces`; `img/*-replace-offered.png`, `img/*-name-changed-offer-gone.png` |
| R2 concern | confirmed at base, fixed | `evidence/repro-before.txt` (log changed 3996→4216 bytes under "Nothing was overwritten"); `browser.rs` `a_failed_folder_save_leaves_the_saved_folder_unchanged`, `save_to_folder_is_staged_too`; `library.rs` folder/user-save failure-at-every-stage, crash-repair, cleanup-crash, foreign-staging tests |
| R3 real WM close | pass (Xvfb + openbox, `wmctrl -c`) | `close-walkthrough.mp4`; `evidence/close-drive.log` (`= running` / `= exited 0` asserts), `close-summary.txt`, `closed-pad.{before,after}.sha256`; `img/c1-*`, `c2-*`, `c3-*` |
| Dialog shots, both sizes/themes | pass | `img/{1440x900,1280x800}-{light,dark}-{replace-offered,name-changed-offer-gone,close-question,save-failed}.png` |
| Preview semantics | unchanged (not in scope) | no preview/engine code touched |

Neither concern was disproven. The base's R2 repro fixture (a directory named
`checkpoint.json`) no longer fails because the staged save moves it aside, so the
after-fix tests inject failures with `library::fail_saves_at` / `KABL_SAVE_FAIL`.

**Changes:** `crates/ui/src/library.rs` (`save_folder`, `repair_folder`, staging marker,
`ReplaceMismatch`, `Interrupted`, fault hook, settle-before-save); `crates/ui/src/browser.rs`
(SaveAs offer tied to its name, folder paths via `save_folder`, truthful failure text, repair
note on open, in-place Save keeps library name); `crates/ui/src/main.rs` (close while a dialog
is open: message); tests in `crates/ui/tests/{browser,library}.rs`;
`docs/rack-migration/drive.py` (`wmclose`, `running`, `exited`); `docs/find-play-save/repair-1/`.
`lib.rs`, perform/routing/rack untouched.

**Decisions:** `docs/decisions.md` "D01-R1: saving safety repairs".

**Verification** (Ubuntu 24.04.4 cloud container, rustc 1.94.1, at b579fb3):
`cargo test --workspace` exit 0, **457 passed, 0 failed, 14 ignored**
(`evidence/test-workspace.txt`); `cargo clippy --workspace --all-targets` exit 0, 0 warnings
(`evidence/clippy.txt`); focused `evidence/focused-tests.txt` (browser 17, library 23 + 1
ignored writer).

**Subagent review:** fresh reviewer subagent, base 58e6622; rounds at 131b13b, 94230ea,
final recheck at **b579fb3**: no findings blocking submission. One major found in round 2
(cleanup-crash rollback deleting a completed save, introduced by a round-1 fix) and fixed in
b579fb3. Record: `REVIEW.md`.

**Real-app evidence** (scripted, release build of b579fb3, Xvfb 1600×1000 + openbox
undecorated at 0,0, `wmctrl -c`, PipeWire null sink, no audio recorded):
`record-close.sh` → `close-walkthrough.mp4` (77 s, 4 parts: Cancel then Save via Save As
then quit; injected save failure keeps app/question/files, close during the question
refused, Don't save quits; close during Save As refused; unedited quits at once).
`record-dialogs.sh` → the 16 dialog shots. D01's audio walkthrough is unchanged and does
not measure this code.

**Measurements:** none new (no audio-path change).

**Compatibility:** patch format unchanged; folder saves write the same three files
(`.kabl-save/` is reserved and transient); old folders and library sounds load as before;
undo/unsaved state unchanged; failed saves keep the edit, undo history and ● and stop
Save-then-Open/New/Quit.

**Limits:** crash cases simulated by hand-built states (tests) and injected failures, not
real kills or disk-full; X11/openbox only (no Wayland, no laptop); no fsync or multi-writer
protection (unchanged). One injected-only mixed state is recorded in `REVIEW.md` 3.2.

**Owner checklist:** `docs/find-play-save/CHECKLIST.md` (unchanged items; its close check now
has cloud evidence but stays unchecked for the laptop's window manager).

**Follow-ups:** none new beyond D01's list; picker/Stop semantics still await Kosta's choice.

**Repo status:** clean after push; `.ai/` absent and untouched; D02 files not touched.
