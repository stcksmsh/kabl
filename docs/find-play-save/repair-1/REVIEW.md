# D01-R1 — reviewer subagent record

Reviewer: a fresh general-purpose Claude subagent spawned by the implementing agent
at the end of implementation (per AGENT-WORKFLOW "Reviewer subagent before submission"). It
received the brief, workflow, D01 README/brief, exact commits and evidence paths, read the
code directly, ran tests and scratch probe programs outside the repo, and wrote no product
changes. Reviewer approval is not Kosta's approval.

| Round | Base | Reviewed head | Result |
|---|---|---|---|
| 1 | 58e6622 | product 131b13b, evidence b5ec0a5 | 0 blocker/major, 3 minor, 1 doc, 4 nits |
| 2 (recheck) | 131b13b | 94230ea (+ README 035b713) | round-1 items fixed; 1 new **major**, 1 minor |
| 3 (final recheck) | 94230ea | **b579fb3** (final product head) | both fixed; no findings blocking submission; 2 optional nits |

Tests the reviewer ran: round 1 workspace 452/0/14; round 2 455/0/14; round 3 457/0/14;
clippy clean each time; focused `--test library --test browser` each round.

## Findings and resolutions

| # | Severity | Where | Failure scenario (reviewer) | Resolution |
|---|---|---|---|---|
| 1.1 | minor | `library.rs` `commit_dir` / `Library::save` | After an `Interrupted` user save left `.x.old` as the only copy, a retry in the same session cleared `.old` before swapping; a second failure lost both copies while saying "Nothing was overwritten" (probe: `sounds/` empty). | 94230ea: `Library::save` settles interrupted swaps first; `commit_dir` puts a lone `.old` back instead of clearing it. Test `a_retry_after_an_interrupted_user_save_keeps_the_only_copy`. Confirmed round 2. |
| 1.2 | minor | `repair_folder` | Crash rollback left a new `checkpoint.json` where the folder had none, and a new-folder crash left meta+checkpoint without a log. | 94230ea: marker phases; `roll_back` removes files the save moved in where none existed. Test `an_interrupted_folder_save_is_repaired_on_the_next_open` (no-checkpoint, new folder, writing phase). Confirmed round 2. |
| 1.3 | minor | `repair_folder` via `read_patch`/`save_folder` | A user's own `.kabl-save/` was deleted recursively on any open; repair notes were discarded. | 94230ea: act only with kabl's `kabl-staging` marker; remove only kabl's files then non-recursive `remove_dir`; a foreign `.kabl-save` blocks saving; the open message shows the repair note. Test `a_kabl_save_directory_kabl_didnt_make_is_left_alone`. Confirmed round 2. |
| 1.4 | minor (docs) | `docs/find-play-save/README.md` | Folder-save guarantees, reserved `.kabl-save`, repair, `KABL_SAVE_FAIL` undocumented. | 035b713 README "Guarantees and failure behaviour". Confirmed round 2. |
| 1.5 | nit | `fail_point` | `KABL_SAVE_FAIL` read in release builds. | Kept, documented as a scripted-run hook (the real-app failure evidence needs it), like `KABL_RECORD_FAIL_AFTER`. Accepted round 2. |
| 1.6 | nit | `save_folder` `Interrupted` text | Promised restoration the repair rule wouldn't do in one injected-only case. | 94230ea: text no longer promises; says the next open/save settles it. Confirmed. |
| 1.7 | nit | `browser::save` | Success message used a possibly stale `doc.name`. | 94230ea: library entry's name, doc name refreshed. Confirmed. |
| 1.8 | nit (tests) | tests | No UI test that Cancel after a collision leaves A; repair test only complete folders; browser R2 test uses injection instead of the base's directory fixture. | Added `cancel_after_a_collision_keeps_the_other_sound`; repair cases added (1.2); the fixture change is explained in the test and REPORT (the staged save moves a directory named `checkpoint.json` aside, so that fixture no longer fails). |
| 2.1 | **major** (new in 94230ea) | `remove_staging` + `repair_folder` | A kill during cleanup after a *successful* save (marker still "replacing", `.old` files already removed) made the next open roll back and delete all three patch files (probe: folder empty). | b579fb3: marker "done" before cleanup ("undone" after an in-process rollback); committed = staged log gone and live log present (the log moves last). Test `a_save_killed_during_cleanup_stands` (existing and new folder, "replacing"/"done"). Confirmed round 3. |
| 2.2 | minor | `save_folder` | An empty leftover `.kabl-save` or kabl's own unrecoverable staging blocked saving with "kabl didn't make it". | b579fb3: empty dir removed (non-recursive); marker present → `Interrupted` "could not be undone: the previous files are kept in …". Test `leftover_staging_is_cleared_or_explained`. Confirmed round 3. |
| 2.3 | nit | README | Link to `repair-1/REPORT.md` dangling until closeout. | Resolved: REPORT.md added with this record. |
| 3.1 | nit (optional) | README | Says a foreign `.kabl-save` is "never touched"; an *empty* one is now removed. | Wording fixed in the closeout docs commit. |
| 3.2 | nit (optional) | `save_folder` | Injected `replace:log.jsonl` failure followed by a crash mid-rollback can leave the old log with a new meta/checkpoint; load replays the log only, so the sound is right. Not reachable without the test hook. | Left as is; recorded here. |

## Review limits (reviewer's statement)

Crash states were simulated by hand-built on-disk states, not by killing a process; no real
out-of-space/permission faults (root container); Xvfb recordings not re-run and the mp4 not
watched; not all 16 dialog screenshots inspected; D02 and preview behaviour not reviewed.
REPORT/HANDOFF/STATUS were written after the final recheck and were not reviewed.
