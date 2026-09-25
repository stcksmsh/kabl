# D02 — Independent review

Reviewer: fresh reviewer subagent (read-only on product code; this file is its only edit).
Date: 2026-09-25.

- **Base:** `58e662290f5df69e7b66e5d8e48d6d8e5e583ae3`
- **Reviewed head:** `8e4d2fb4c05c1118f696aedc8a29c35519905d7a` (product code last changed at
  `e824aadfa9e2f3d613acf79bc0e696176e3663f2`; `8e4d2fb` adds docs and evidence only)
- **Integration state:** D01-R1 not yet merged into this branch. This review covers D02 alone.
  The combined result still needs a recheck.

## Coverage

What I read:

- The brief `docs/product-research/briefs/D02.md` and AGENT-WORKFLOW "Reviewer subagent before
  submission".
- `git diff 58e6622..8e4d2fb -- crates`. I read all of `crates/ui/src/explain.rs`,
  `crates/ui/src/help.rs` and `crates/ui/tests/explain.rs`, plus every touched hunk in
  `crates/ui/src/{lib,perform,routing,editor}.rs`.
- Context code:
  - `crates/engine/src/compile.rs`: `DEFAULT_ROUTE_AMOUNT`, `route_bypassed`, legacy param
    resolution, `full_scale`, and the block-rate modulation loop.
  - `crates/modules/src/info.rs`: `to_norm`/`from_norm`, `nominal_range`.
  - `registry::legacy_param` and `builtins/{macros,mixer,osc_va,filter_ladder,vca,env_adsr,midi_in,seq,delay,lfo,clock,clock_div,noise}.rs`.
  - `crates/engine/src/keyboard.rs`: mode, priority and glide rules.
  - `routing.rs`: `routes_into`, `route_span`, `combined_span`, `base_value`.
  - `lib.rs`: `UiState::validate`, off-face reveal, Escape chain.
  - `browser.rs`: `replace_patch`, `is_modified`.
  - `editor.rs`: id allocation, `from_log`, `seed_from`.
- Evidence: `README.md`, `CHECKLIST.md`, `coverage.md`, `evidence/*`, all eight screenshots
  (I viewed two), and `walkthrough.mp4`. I checked it with ffprobe: 77.1 s, 1440×900 h264, AAC
  48 kHz, mean −19.1 dB and max −2.9 dB (volumedetect). I also looked at frames at about 10 s,
  24 s, 32 s and 64 s.

What I ran, on this container at head `8e4d2fb`:

- `cargo test -p kabl-ui`: exit 0, all suites ok. Totals for `explain`: 12 passed, 1 ignored.
  Unit tests: 13 passed.
- `cargo test -p kabl-ui --test explain`: 12 passed, 1 ignored (the coverage writer).
- `cargo clippy -p kabl-ui --all-targets`: no warnings.
- `git diff --stat 58e6622..8e4d2fb -- crates/ui/src/browser.rs crates/ui/src/library.rs crates/ui/src/main.rs docs/find-play-save .ai`:
  empty. The ownership boundaries are respected and `.ai/` is untouched.

I did not rerun `cargo test --workspace` or workspace clippy. I relied on
`evidence/test-workspace.txt` (459 passed at e824aad) for the other crates.

## What checks out

- **Modulation math matches the engine.** The engine computes
  `base_norm + Σ source × amount / full_scale`, skips bypassed routes, and clamps once in
  `from_norm`. D02 handles this correctly:
  - `RouteView.src` uses the same `nominal_range / max(|lo|,|hi|)` as `compile.rs`'s
    `full_scale`.
  - The default amount is the same `DEFAULT_ROUTE_AMOUNT` (0.25).
  - The `bypass ≥ 0.5` test matches `route_bypassed`.
  - `route_span`/`combined_span` exclude bypassed routes and leave the clamp to `from_norm`.
  - Depth text is right: unipolar reads `+x % of travel at full`, bipolar reads `±x %` with
    `(inverted)`, and negative unipolar spans run downward from the base.
- **Navigation is view state only.** `go` and `back` touch only `UiState`. The off-face reveal
  uses the rack's existing `inspected` → `expanded.insert` path. `face.*` is never written, and
  the test checks that.
- **Stale references.** `PatchEditor::instance()` is taken from a process-wide counter in
  `new()`/`from_log()`. `seed_from` goes through `new()`. `replace_patch` is the only in-UI
  replacement path, and it builds a new editor, so opening another sound closes the
  explanation without touching `browser.rs`. Module and cable ids are monotonic within an
  editor (`next_module_id += 1`), so an undone-then-re-added module cannot alias an old
  subject id.
- **Bounded graph work.** `path_to_output` is a BFS with a visited map, capped at 512 modules,
  and it terminates on cycles (tested). One-hop destinations are deduplicated per via-module
  and capped at 24 rows.
- **Escape.** The chain runs drag → choose → learn → explanation. The explanation branch is
  skipped while a browser dialog is open or a text field has, or just had, focus. The
  `typing` latch is set at the end of `show()`, and `show()` has no early return. The test
  covers learn, rename and bare Escape.
- **Help text.** Sampled against the DSP and it is accurate:
  - `osc.va`: base_hz at pitch 0 = 261.63; unison level 1/√N; detune spread lowest→highest;
    pw only on the square.
  - `filter.ladder`: self-oscillation above about 91 %, k ≥ 4.
  - `vca`: `(gain+cv).clamp(0,1)`, and EXP squares it.
  - `env.adsr`: KEY latches A/D/R at the gate's rising edge.
  - `midi.in`: MONO retriggers; LEGATO retriggers only without an overlap; LAST/LOW/HIGH
    priority; glide LEGATO only on overlap.
  - `seq`: CLOCK/LENGTH gate modes, probability, startup bank.
  - `delay`: equal-power mix with exact endpoints; tone in the feedback path, so the first
    echo stays full.
  - `lfo`: sync counts in 16th pulses.
  - `clock`: four pulses per beat.

  All 21 `ModuleInfo.explain` strings I read are consistent. Bookkeeping keys (`pin.*`,
  `face.*`, `cc.*`) cannot appear as controls: `param_of` and the module control list read
  only registry `ParamInfo`.
- **Opening an explanation alone does not dirty the document.** Frame at about 10 s: the title
  reads "Evolving Pad" without ●. The screenshot `1280x800-dark-macro-shown.png`, taken after
  Show, also has no ●.

## Findings

| ID | Severity | File / function | Failure scenario | Evidence | Requested correction |
|---|---|---|---|---|---|
| R1 | minor (affects the "Honest explanation" acceptance row) | `explain.rs` `macro_now`, used by `dest_detail` and `base_and_modulation` | Evolving Pad, Warmth → Ladder #7 Cutoff: the row reads "at its stored 40 %: 1.13 kHz · 3 other routes also move it". The figure is `from_norm(base_n + amount·pos)`, which is the base plus this one route. The other active routes are left out: Ring Mod #13, MIDI pitch, and ADSR #8 (unipolar +6 %, so it never averages out). A musician reads it as "with Warmth at 40 % the cutoff is 1.13 kHz", but the filter never sits there while a note plays. CHECKLIST step 2 asks the owner to confirm that "the 'at its stored …' values follow", which reinforces that reading. | `explain.rs` lines 449–467, 562–572; screenshot `img/1280x800-dark-macro-shown.png`; video frame at about 32 s | When other non-bypassed routes reach the same parameter, either suppress the figure or label it as partial, e.g. "base + this route alone at its stored 40 %: 1.13 kHz; the other routes add on top". Add a test assertion on the pad Cutoff row. |
| R2 | minor | `explain.rs` `outgoing`/`dest_detail`/`base_and_modulation` together with `routing::routes_into` | An old patch has a modulation route to mixer param `level`, the pre-rename name. The engine applies it to channel 1 (`compile.rs:591`), and `param_of` also resolves it to `level1`. But `routes_into(state, id, "level1")` matches the param string exactly and misses the cable, so three things go wrong: (a) the macro's destination row "Mixer #N · Level 1" shows no depth, sign or bypass (`dest_detail` returns ""); (b) explaining Level 1 says "Modulation: none. The control stays at its base.", which is false; (c) Show sets `selected_route` to that cable, and `UiState::validate` clears it at once because it compares param names exactly. No factory patch uses `level` (grep `patches/`), so only old user patches are affected. | Code reading: `routing.rs:59-66`, `explain.rs:223-231, 287-296, 551-556`, `lib.rs:339-347` | Resolve legacy names consistently. For example, have `routes_into` (or an explain-local variant) also match a cable whose param is `registry::legacy_param(kind, name)`, and add a small test that builds a `level` route by hand. At minimum, do not print "Modulation: none" when an unmatched legacy route exists. |
| R3 | minor (evidence gap) | `tests/explain.rs` `a_direct_pin_reaches_its_exact_parameter`; screenshots `*-dark-direct-pin.png` | The acceptance row "Direct pin → parameter … including a currently hidden/off-face control" is shown only with the on-face ADSR Attack. The test never checks that the control was hidden first. The off-face reveal is proven only for a macro destination (Ladder Drive). The factory set has a real off-face direct pin, `palette/lead` "Glide time" (`midi.in` `glide_ms`, which is in `advanced`), and it is not exercised. The code path is shared (`go` → `inspected` → reveal), so the behaviour is likely right, but this is unverified for the row as written. | `tests/explain.rs` lines 196–211; `coverage.md` "palette/lead \| Glide time" | Add a test on `palette/lead`: assert `knob:<midi>.glide_ms` is absent, click `pexplain:…glide_ms` then `explain-show:…`, and assert it is present and inspected with the patch unchanged. Optionally add one real-app screenshot. |
| R4 | minor (PLAUSIBLE; behaviour change not documented or tested) | `perform.rs` `card`, the rename `TextEdit` | Before, `request_focus()` ran right after `ui.add(TextEdit)`, which re-took any focus the field gave up to a click elsewhere, so `lost_focus()` (live in egui 0.35, `Memory::lost_focus`) stayed false. Clicking away left the rename open until Enter or Escape. Now `request_focus` runs after the `lost_focus()` check, so clicking any other widget or the rack commits the rename. That is probably the originally intended "commit on blur", but it is a user-visible change the README does not mention (it describes only the focus that stayed after Enter/Escape), and no test covers click-away. | Code reading of the diff at `perform.rs:795-817`; egui `response.rs:365`, `memory/mod.rs:851` | Confirm that commit-on-click-away is intended. Mention it in the README Focus section and add an interaction test: double-click to rename, type, click elsewhere, then assert the label is committed as one undo step and no focus is left. |
| R5 | nit | `explain.rs` `path_to_output` / `path_line` | With more than 512 reachable modules, the search returns `None` at the cap and the panel states the graph fact "no cable path from X to an Output", which may be false. | Lines 376–378, 797–804 | Return a distinct "search limit reached" outcome and word it as unknown. |
| R6 | nit | `explain.rs` `special` (TRANSPORT) and `help::special_pin_help` | The help says the transport acts on "this clock and every sequencer it drives", but "Acts on" lists only the modules on the clock's `gate` cable. In `echo`/`interlocking` that means `clock.div` #2 and seq #3; seq #4 behind the divider, and the `reset` output's targets, are not listed. LFO/delay clock inputs would be listed as "acted on" even though the transport does not start or stop them. | Lines 907–911; `patches/echo` cables 1–6 | Either follow gate cables through `clock.div` (bounded) to `seq`, or reword to "Modules on this clock's gate output". |
| R7 | nit | `explain.rs` `back`, `module` controls list | (a) Showing a sequencer control in another bank switches `edit_bank` (`lib.rs:453-463`). `SavedView` does not save it, so Back leaves the face on the other bank. (b) Clicking a control in a module explanation's Controls list assigns `subject` directly, so a previous `target` or `card` from another subject is kept, whereas `open()` would reset `target`. | Lines 82–91, 202–219, 1018–1023 | Save and restore `edit_bank` for the shown module in `SavedView`, and route the Controls-list click through `open(…, None)` or reset `target`. |
| R8 | nit (test quality) | `tests/explain.rs` `H::unchanged_since`, `edits_from_perform_and_rack_are_one_state_while_explained` | `browser::is_modified` is always false in the harness because `ui.doc` is `None`, so the "navigation dirtied the document" assertion is vacuous. The full-state equality on the line before does cover it. The "one state" test writes through `editor.set_param`, not the Perform slider or rack widget its comment names. | Lines 108–117, 419–441; `browser.rs:207` | Set `ui.doc` to a saved snapshot in `H::new`, or drop the vacuous assert. Drive the value once through `pslider:3.cutoff_hz` and once through the rack/drawer control. |
| R9 | nit (unverified) | `lib.rs` Escape chain | Escape pressed while an egui popup or context menu is open (Sounds/Add combo, a module's right-click menu, the card `…` menu) likely closes the popup and the explanation together. The chain checks browser dialogs and text focus, not open popups. | `lib.rs:486-499` | Also skip closing when a popup is open (`egui::Popup::is_any_open(ctx)` or equivalent in 0.35), or accept this and note it. |
| R10 | nit | `help.rs` | (a) `("midi.in","glide")` says LEGATO glides "between overlapping notes", but in POLY a LEGATO glide never happens (`keyboard.rs:235`). (b) The `module_title` doc comment shows `(filter.ladder)`, which the function does not output. | `help.rs:188-190, 263-267` | Add "(MONO/LEGATO modes)" to the glide help and fix the doc comment. |

No blocker or major findings. I found no path by which explanation or navigation edits the
patch, adds an undo step, sets `take_dirty`, queues a launch or transport command, or writes
`face.*`.

## Acceptance evidence as reviewed

| Criterion | Reviewer view |
|---|---|
| Direct pin → parameter | The exact parameter is verified (test, screenshot). An off-face direct pin is **unverified** (R3). |
| Macro → multiple destinations | Verified: test for route edit, bypass, remove and undo; real-app walkthrough with bypass and Ctrl+Z; Show on the off-face Drive. |
| Honest explanation | Empty state and duplicate labels are verified by test. "Not fabricated" is weakened by R1 (partial value label), and R2 applies to legacy patches only. |
| One state | State equality, save/reload and undo are verified by test through the editor API, not the UI (R8). "Help alone does not dirty" is verified in the real-app frame and screenshot. |
| Base/modulation distinction | Verified by test (units, signed unipolar/bipolar, bypass, CC ≠ modulation) and by screenshot. |
| Factory help coverage | The test asserts that no factory card falls back and every factory macro has a destination. I did not regenerate `coverage.md` (that would write outside REVIEW.md), so whether it is current is unverified. |
| Navigation/focus | Back, Escape and text-field Escape are verified in the test and the walkthrough. Patch replacement and module deletion are verified in the test only (synthetic egui input through `show()`), not in the real app. |
| Layout | Screenshots exist at both sizes and themes. The walkthrough is audible. The browser panel is not shown open in any D02 screenshot, although the brief asks for "browser/Perform/routing states exercised". |
| Regression | kabl-ui tests and clippy were rerun green at this head. Workspace totals come from the implementer's log at e824aad. |
| Final integration | **Not done.** D01-R1 is not merged. `REPORT.md` does not exist yet, although the README links it. |

## Limits of this review

- This review covers D02 alone, before D01-R1 integration. The combined result must be
  rechecked.
- The review is static apart from the kabl-ui test and clippy runs. I did not run the real
  app, audio, MIDI hardware or a laptop. Real-app behaviour is judged from the committed
  screenshots and video, which are scripted xdotool/Xvfb runs with a fifo MIDI stand-in.
- R4 and R9 rely on reasoning about egui 0.35 focus and popup internals. They are labelled
  PLAUSIBLE/unverified and were not reproduced.
- Help accuracy was sampled (the modules listed above), not proven for every parameter.
- Reviewer approval is not owner (Kosta) approval. Owner checks remain pending.

## Implementer responses

The fixes are in 0987db5 and e60300c. At e60300c, `cargo test --workspace` gives 465 passed,
0 failed, 15 ignored, and `cargo clippy --workspace --all-targets` is clean.

| ID | Resolution |
|---|---|
| R1 | **Fixed.** `macro_now` now checks for other active routes into the same control. When there are some, the row reads "base + this route alone at its stored 40 %: … (the other routes move it further)". Test: `base_and_modulation_are_told_apart_with_units` asserts this on the pad's Cutoff. CHECKLIST step 2 wording still applies. |
| R2 | **Fixed.** New `routing::reaches(state, id, stored, param)` follows the compiler: the same name, or a legacy name that resolves to the first param it names. `routes_into` and `UiState::validate` both use it. Test: `a_legacy_mixer_level_route_is_explained_on_channel_1`. |
| R3 | **Fixed (test).** `a_direct_pin_to_an_off_face_control_reveals_it` covers `palette/lead`'s "Glide time" (MIDI In #2 `glide_ms`) at 1280×800. It checks the control is off the face first, then the reveal, the patch left unchanged, and that Back collapses it again. |
| R4 | **Kept, now documented and tested.** Committing when you click elsewhere is what the existing `lost_focus` branch meant to do. Test: `a_card_rename_commits_when_you_click_elsewhere` checks the label, one undo step and that no focus is left. The README Focus section mentions it. |
| R5 | **Fixed.** `path_to_output` returns `Result<Option<_>, PathLimit>`. The panel says the path was "not worked out" when the search hits its limit. Test: a 600-module chain returns `Err(PathLimit)`. |
| R6 | **Fixed.** New `explain::clocked` follows clock cables through `clock.div` (bounded) and lists only sequencers. The transport starts and stops those; a synced LFO or delay keeps its last rate. Reset-only cables do not count. Test: `transport_acts_on_sequencers_behind_a_divider` (interlocking → [3, 4]). |
| R7 | **Fixed.** `SavedView` stores `edit_bank`, and Back restores it. A click in the Controls list goes through `open()`, which resets the target and keeps the origin card. |
| R8 | **(a) Disputed. (b) Fixed.** (a) `browser::frame_input` sets `ui.doc` on the first frame, and `H::new` now asserts `ui.doc.is_some() && !is_modified`. So the check does test something, and a real edit makes it true (asserted in the new test). (b) `edits_through_the_card_and_the_rack_agree_while_explained` drags the Perform card slider (`pslider:3.cutoff_hz`) and the drawer control (`dparam:3.cutoff_hz`). Undo returns the document to clean. |
| R9 | **Fixed.** When a popup is open now, or was open at the end of the last frame, Escape belongs to the popup. Test: `escape_with_a_menu_open_closes_only_the_menu` (a card ⋯ menu). |
| R10 | **Fixed.** The glide help now says "in MONO and LEGATO modes", and the `module_title` doc comment is corrected. |

Other gaps: `coverage.md` was regenerated at 0987db5 and has not changed. The screenshots and
walkthrough were re-recorded at 0987db5. I did not add a real-app screenshot with the browser
panel open; the browser is D01-R1's area and D02 does not change it.

## Recheck
