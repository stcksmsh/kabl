# D02 — structured report

```text
Batch ID / outcome: D02 — Musical controls that explain themselves. A musician presses ? on a
  Perform card and sees what it really changes, taken from the current patch graph. Show jumps
  to each target in the rack, including controls that are off the module face. Back returns
  to the card. Nothing about the sound, the saved patch or the undo history changes.
Starting commit: 58e662290f5df69e7b66e5d8e48d6d8e5e583ae3 (origin/master when the batch began).
Integration: D01-R1 merged to master (PR #4, 326bbdc); master merged into D02 with 4fa0402.
Submitted head commit: combined product head 4b478fd (D02's product code last changed at
  e60300c). The docs head is the latest commit on the branch. The combined-head recheck is
  in REVIEW.md.
Branch / pushed remote / PR if environment-required: claude/d02-musical-controls-knjmgu. The
  environment requires this name; it stands in for work/d02-controls. Pushed to origin. Draft
  PR https://github.com/stcksmsh/kabl/pull/3
Engineering status: integrated with D01-R1 and re-verified on the combined head. Merge-ready
  only once REVIEW.md's combined-head recheck finds no blocker.
Owner-review status: pending.
```

## What now works (user-facing)

- **? on every Perform card.**
  - Direct pins show their label next to what they really are
    (`SVF Filter #3 · Cutoff (filter.svf "cutoff_hz")`), plus help, range and units, base vs
    modulation, a note that a MIDI CC is not modulation, and the signal path.
  - Macros list every cable that actually leaves that output: signed depth, the range it
    covers, bypass state, the value at the macro's stored position (marked "this route
    alone" when other routes add on top), and one bounded hop through a signal jack.
  - Transport, bank and cue cards say what they act on.
- **Show / Back.** Show reuses the rack's inspect, reveal and flash, marks the target, and
  scrolls the drawer to its route rows. Back restores the previous view and flashes the card.
- **Explain from the rack.** In the drawer: Explain on a control or a module. In the module
  context menu: "Explain this module". There is also opt-in ? Help, with inline help for
  each module and control.
- **Keyboard and focus.** Escape gives way to drags, choose mode, MIDI learn, text fields,
  dialogs and popups. An ended card rename no longer keeps the keyboard; this was a
  pre-existing bug that blocked undo and Escape.

## Acceptance matrix

| Criterion | Result | Evidence |
|---|---|---|
| Direct pin → parameter, incl. hidden/off-face | pass (software) | tests `a_direct_pin_reaches_its_exact_parameter`, `a_direct_pin_to_an_off_face_control_reveals_it` (lead "Glide time"); `img/*-dark-direct-pin.png`; walkthrough 0:36–0:47 |
| Macro → multiple destinations; signed/bypassed; reveal each; change/remove/undo | pass (software) | tests `a_macro_card_lists_…`, `the_explanation_follows_route_edits_and_undo`, `signal_jack_destinations_list_one_bounded_hop`; `img/*-macro-shown.png`; walkthrough (bypass ≈0:56, undo ≈1:01) |
| Honest explanation (empty state, duplicate labels, no fabricated live values) | pass | tests `a_macro_without_routes_…`, `base_and_modulation_are_told_apart_with_units`; ranges are labelled "configured … not measured"; per-route value marked "this route alone" (R1) |
| One state; undo/save/reload; help alone doesn't dirty | pass (software) | tests `edits_through_the_card_and_the_rack_agree_while_explained`, `edits_from_perform_and_rack_are_one_state_while_explained` (save/reload), `inline_help_is_opt_in_and_changes_nothing`; `H::unchanged_since` checks the patch, undo depth, document, rebuild flag and queues |
| Base/modulation distinction with units | pass | test `base_and_modulation_…`; screenshots |
| Factory help coverage | pass | `coverage.md` (every factory card); tests `every_factory_card_gets_specific_help`, `every_builtin_param_has_help` |
| Navigation/focus: return, patch replacement, deletion, text entry, Escape | pass (software); replacement and deletion tested only, not recorded in the real app | tests `module_removal_and_patch_replacement_…`, `escape_and_text_fields_…`, `escape_with_a_menu_open_…`, `a_card_rename_commits_…`; walkthrough (Back, Escape) |
| Layout: both sizes and both themes; audible walkthrough | pass (cloud, scripted) | `img/` (10 shots, including Save As over an explanation); `walkthrough.mp4` 77 s with audio (mean −18.9 dB), recorded from the combined head 4b478fd |
| Regression: workspace tests and clippy at the reported head | pass | `evidence/test-workspace.txt` (combined head 4b478fd: 479 passed, 0 failed, 15 ignored), `evidence/clippy.txt` (clean) |
| Final integration with D01-R1 plus reviewer recheck of the combined result | pass (merge 4fa0402; recheck in REVIEW.md) | test `a_save_dialog_over_an_explanation_keeps_escape_and_the_document`; `img/*-dark-save-as-over-explain.png`; REVIEW.md "Combined-head recheck" |
| Novice task (brighter/slower pad, explain it) | unverified; owner observation | CHECKLIST step 10 |

## Changes

- New `crates/ui/src/explain.rs`: the graph model, the drawer section, Show/Back and the target
  marker.
- New `crates/ui/src/help.rs`: authored help text and range text.
- `editor.rs`: `PatchEditor::instance`.
- `lib.rs`: minimal hunks, listed in README "Files".
- `perform.rs`: the card's ? button, its outline, the rename focus fix.
- `routing.rs`: Explain on the drawer heading, inline help, the scroll to routes, and legacy
  route resolution (`reaches`).
- New `crates/ui/tests/explain.rs`: 18 tests plus the coverage writer.
- `docs/musical-controls/*`.
- D01-R1-owned files were not touched: `browser.rs`, `library.rs`, `main.rs`,
  `docs/find-play-save`.

## Decisions

Recorded in the README "Decisions" table:

- the drawer holds a separately scrolling explanation;
- reveal is transient, through `inspected` and `expanded`;
- Back restores a view snapshot and is not an undo step;
- a stale explanation is detected by the editor instance, not by editing `replace_patch`;
- help is keyed by kind and parameter;
- the macro value is a stored fact, labelled partial when other routes add on top;
- signal-jack tracing stops after one hop.

`docs/decisions.md`, HANDOFF and STATUS are deliberately left unedited until integration, as
the brief requires.

## Verification

- `cargo test --workspace` at the combined head 4b478fd: exit 0, **479 passed, 0 failed, 15 ignored**.
  Before integration, e60300c: 465 passed.
- `cargo clippy --workspace --all-targets` at 4b478fd: exit 0, no warnings.
- Focused: `cargo test -p kabl-ui --test explain`: 19 passed, 1 ignored (the writer).
- Environment: Ubuntu 24.04.4 cloud VM, 4 vCPU, rustc 1.94.1. System packages installed
  for the build: libasound2-dev, libudev-dev, libdbus-1-dev, xdotool, ffmpeg, pipewire and
  wireplumber.

## Subagent review

- A fresh general-purpose reviewer subagent reviewed 58e6622..8e4d2fb and found 10
  minor/nit findings, no blocker or major.
- All were fixed except R8(a), which is disputed with evidence. R4 was kept as intended
  behaviour, now documented and tested.
- The recheck against e60300c/2516be4 is recorded in `REVIEW.md` → "Recheck". A fresh reviewer then rechecked the combined head: see "Combined-head recheck".

## Real-app evidence

- Screenshots `img/{1440x900,1280x800}-{light-module-help,light-macro-shown,dark-macro-shown,dark-direct-pin}.png`.
- `walkthrough.mp4`, from `record-walkthrough.sh` running `scripts/walkthrough.txt`.
- Screenshot script `scripts/shots.txt`.
- All scripted with xdotool on Xvfb, using the release build of the combined head 4b478fd.
- Audio at 48 kHz, 256 frames, through PipeWire into a null sink. MIDI came from the
  fifo stand-in (`examples/midi_player`), **not hardware**.

## Measurements

Walkthrough run on the VM: 14833 callbacks. Execution and arrival are reported separately:

- execution: 37 late, worst 16.7 ms against a 5.3 ms budget;
- arrival: 878 late;
- 12 xruns;
- RT priority refused.

D02 adds no audio-thread code. None of this is laptop evidence.

## Compatibility

- No change to the patch format, ops, sound, DSP or factory patches.
- Old patches with a mixer `level` route are now explained on Level 1, where the engine
  applies it.
- Undo and the op log are untouched by navigation. No plugin or automation exists yet.

## Limits and deviations

- The novice-ease observation is pending with the owner.
- Values are configured, not measured.
- Replacing the patch and deleting a module while an explanation is open are tested through
  egui input only, not in the real app.
- Outline under a floating expansion: see README Limits.
- Integrated with D01-R1 in the cloud only. The owner checks for D01, D01-R1 and D02 are all
  pending.

## Owner checklist

`CHECKLIST.md`: 11 steps, all unchecked. Launch:
`cargo run --release -p kabl-ui -- --patch patches/palette/pad --perform --rate 48000 --frames 256`
(plus `--size 1280x800`). Do this after integration.

## Follow-ups (not implemented)

- Rack cable highlighting (`draw_cables`, the inspected-strong check) still compares route
  parameter names exactly. A legacy `level` route is drawn as intended, but it is not
  emphasised while Level 1 is inspected. The impact is cosmetic and only affects old patches.
  Suggested for a later rack touch-up.
- The "Bpm" label for the clock tempo parameter (`routing::param_label`) could read "Tempo".
  That is a rack-wide label change, so it was left for a separate decision.

## Integration state

Done, following the brief.

1. D01-R1 merged to master by Kosta (PR #4).
2. `git merge origin/master` into this branch: merge 4fa0402, no rebase, no force-push. There
   were no conflicts; D01-R1 changed only `browser.rs`, `library.rs`, `main.rs`, their tests
   and docs.
3. Added the integration test (4b478fd). With an explanation open, D01-R1's Save As dialog
   takes Escape first, and the document stays clean.
4. Reran the full suite: `cargo test --workspace` gives 479 passed, 0 failed, 15 ignored, and
   clippy is clean. Re-recorded the screenshots at both sizes (including Save As over an
   explanation) and the audible walkthrough, from the combined build.
5. Wrote the `docs/decisions.md` D02 entry and updated HANDOFF and STATUS, after D01-R1's
   entries. D01-R1 is marked merged.
6. The reviewer recheck of the combined head is in REVIEW.md.

## Repo status

Clean. `.ai/` and unrelated files are untouched. Workspace-wide formatting was never run;
rustfmt ran only on changed files.
