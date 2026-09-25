# D02 — Musical controls that explain themselves

**Built and integrated with D01-R1 (merged master, re-verified on the combined head).** Owner review: pending.
Brief: [`product-research/briefs/D02.md`](../product-research/briefs/D02.md). Checklist:
[`CHECKLIST.md`](CHECKLIST.md). Report: [`REPORT.md`](REPORT.md). Review: [`REVIEW.md`](REVIEW.md).
Coverage table: [`coverage.md`](coverage.md). Draft PR: https://github.com/stcksmsh/kabl/pull/3
(branch `claude/d02-musical-controls-knjmgu`; the environment requires this name, it stands in
for `work/d02-controls`).

Pick a Perform card, press its **?**, and kabl shows what that card actually changes in
this patch. **Show** takes you to the control in the rack, and **Back** returns you to the card.
None of this changes the sound, the saved patch or the undo history.

## What it does

**Explain (drawer, top section).** You can open it three ways:

- **?** on any Perform card. The card stays outlined while its explanation is open.
- **Explain** next to the inspected control's heading in the Routing drawer.
- **Explain** next to the module name in the drawer, or **Explain this module** in a module's
  right-click menu.

What the explanation shows:

- **Direct pin** (e.g. Init Keyboard "Brightness"):
  - the card's label (the author's name for it) and what it really is: `Perform card → SVF
    Filter #3 · Cutoff (filter.svf "cutoff_hz")`;
  - authored help, plus range, unit, taper, default and stepped choices;
  - **Base vs modulation**, described below;
  - the signal path from its module to an Output, followed through cables and bounded.
- **Macro card** (e.g. Evolving Pad "Warmth"), under **What it changes**, lists every cable
  that leaves that macro output, read from the current graph each frame:
  - the module title with its `#id` and the parameter, e.g. `Ladder Filter #7 · Cutoff`;
  - signed depth: `+20 % of travel at full` for unipolar sources, `±12 %` for bipolar ones,
    with `(inverted)` when the amount is negative;
  - the configured values it spans: `→ 650 Hz – 2.59 kHz`;
  - the value at the macro's stored position, when the macro knob has no routes of its own.
    When other active routes also move that control, it reads "base + this route alone …
    (the other routes move it further)" rather than claiming a total;
  - bypass, shown as strikethrough plus "bypassed, adds nothing now";
  - other routes into the same control, and whether the destination has a Perform card.

  A signal-jack destination (e.g. Sound Palette "Motion" → Ring Modulator `b`) also shows
  what that module then moves: one hop, at most 24 rows. With no destinations the
  explanation says the macro changes nothing and how to give it one.
- **Transport, bank and cue cards**: what the card does, the clock or sequencer it belongs to,
  and what it acts on (the sequencers on a clock's gate, through dividers too, and the sequencers a cue
  switches).
- **Module**: its `ModuleInfo.explain` text (checked against the DSP code, see Decisions), a
  note where one helps, the signal path, inputs and outputs from the cables, and a
  collapsible list of controls, each with one-line help and its own Explain.

**Show / Back.**

- **Show** inspects the target with the rack's existing mechanisms:
  - it selects the module, sets the inspected knob and its route (so the ring, lanes and
    drawer row highlight it), and pans the control into view;
  - an off-face control, such as the ladder's Drive, is expanded transiently (view state
    only, no `face.*` is written) and flashes;
  - a `Shown` pill and outline stay on the target, and the button reads **● Shown**;
  - the drawer scrolls to that control's routes, where depth, invert, bypass and remove
    work as before.
- **Back to "Warmth"** restores the view from before the first Show: pan, zoom, inspected
  knob, selection, expanded modules and whether the Perform panel was open. It also
  scrolls to the originating card and flashes it, and puts back the sequencer bank a Show
  switched the face to.
- **✕** or **Escape** closes the explanation without moving the view.

**Base vs modulation.**

- **Base (stored).** The rack knob, the drawer's Base field, its Perform card and its MIDI CC
  all write this value, and Save keeps it. When a CC is mapped, the explanation says the CC
  writes the base with pickup and is not a modulation source.
- **Modulation.** Each route adds in knob travel, then the sum is clamped once. The
  explanation gives the combined reach and labels it "configured from the routes, not
  measured".
- **What is not shown.** kabl has no telemetry, so the explanation never shows the
  instantaneous effective value. There is no host automation yet, so it does not describe
  any.

**Inline help (opt-in).** **? Help** in the drawer header adds each module's description and
each control's one-line help and range to the drawer. It starts off, and every control works
with it closed.

## Invariants and how they are kept

- **One state.** Every explanation reads `PatchEditor::state()` each frame. Nothing is cached,
  and the only authored lists are help text keyed by module kind and parameter name.
- **Navigation is view state only.** `explain::go` and `back` only touch `UiState`: pan,
  zoom, inspected knob, selection, expanded modules, the Perform panel and the drawer. They
  send nothing to the audio thread and add no undo entry. Tests check that the patch, the
  undo depth, the document's unsaved state, the audio-rebuild flag, and the launch and
  transport queues are all unchanged.
- **Stale references.**
  - A removed target is dropped and the panel says so; undo brings it back.
  - A removed subject module says "no longer in the patch".
  - Opening another sound closes the explanation. It is tied to the editor instance: a new
    `PatchEditor::instance()` is taken per load, so `browser.rs` needed no change.
  - Selections the rack already validated are handled as before.
- **Focus.** Escape is claimed in this order: a drag, choose-primary mode, MIDI learn, a text
  field or open dialog, and only then the explanation.
  - Escape in a text field belongs to that field. egui drops a field's focus on Escape before
    the frame runs, so kabl remembers whether a text field had focus at the end of the
    previous frame.
  - A pre-existing bug surfaced here and is fixed: after Enter or Escape, a finished Perform
    card rename kept keyboard focus. That also blocked the Ctrl+Z and Escape shortcuts.
  - An Escape while a menu or popup is open (a card's ⋯ menu, a combo box) closes that popup
    only.
  - A card rename now commits when you click elsewhere (one undo step), as the existing
    `lost_focus` code intended; before, the field grabbed focus back every frame.
  - Help and navigation never start a preview, the transport, or a parameter edit.
- **Bounded graph work.** Destinations go one hop and stop at 24 rows. The signal-path
  search is breadth-first over at most 512 modules, keeps a visited set so cycles end, and
  follows signal cables only. An explanation says "no cable path" when there is none; it
  never guesses why a patch is silent. Over the 512-module limit it says the path was not
  worked out, rather than "no path".
- **Old patches.** A route stored on a mixer's pre-rename `level` is shown on Level 1, as the
  engine applies it (`routing::reaches`).

## Decisions

| Decision | Why | Rejected |
|---|---|---|
| Explanations live in the existing Routing drawer, in their own scroll area (at most ~55 % of the drawer) | The brief asks to reuse the drawer. The route rows of a shown control stay visible at 1280×800 | A new permanent panel; putting the explanation in the same scroll area, which pushed the route rows out of view |
| Reveal reuses `inspected` plus the transient expansion | This is how the rack already reveals an off-face route target, and it never writes `face.*` | Temporarily changing the face |
| Back restores a saved view snapshot, not an undo step | Navigation is not a musical edit (U18) | Recording navigation in the op log |
| Stale explanations are detected with `PatchEditor::instance()` | Keeps D01-R1's `browser.rs::replace_patch` untouched | Adding a reset line to `replace_patch`, a D01-owned file |
| Help is keyed by module kind and parameter (slot name for sequencer banks) | One text serves every patch instance. A card label is never used to infer what a control does | Prose per patch |
| Macro "value now" is shown only from the macro's stored position, and only when the macro knob has no routes of its own | That value is a stored fact. Anything else would claim an instantaneous DSP value | Showing a modulated effective value |
| Signal-jack destinations go one hop | Makes "Motion → ring mod b → cutoffs" visible while staying bounded | A full graph walk or a graph editor |

`ModuleInfo.explain` was checked against the module source. All 21 texts describe what the
code does, and none were changed. Two needed context, which is authored in `help.rs`
instead:

- **Ring modulator.** The factory sounds use it as a depth control (macro × LFO).
- **Ladder resonance.** Above about 91 % it self-oscillates.

The `docs/decisions.md` entry ("D02: musical controls that explain themselves") and the
HANDOFF and STATUS updates were written after the D01-R1 integration, as the brief asks.

## Files

- `crates/ui/src/explain.rs` (new): the explanation model built from the graph
  (`outgoing`, `destinations`, `path_to_output`, `base_and_modulation`) and the drawer
  section, plus `go`, `back` and the target marker.
- `crates/ui/src/help.rs` (new): authored help per module, parameter and jack; `range_text`;
  help for special cards.
- `crates/ui/src/editor.rs`: `PatchEditor::instance()`.
- `crates/ui/src/lib.rs`, all minimal hunks, listed here because D01-R1 may also touch this
  file:
  - `mod explain` and `mod help`;
  - the `UiState::explain` field;
  - validation in `show()` and Escape priority;
  - the drawer header's **? Help** button and the explanation's scroll area;
  - the module panel's Explain button and inline help;
  - the **Explain this module** menu entry;
  - the rack target marker;
  - recording text-field focus and open popups at the end of the frame;
  - `validate` resolves legacy route names through `routing::reaches`.
- `crates/ui/src/perform.rs`:
  - the card's **?** button, its origin outline and Back flash;
  - the rename-focus fix;
  - `mixer_source` made `pub(crate)`.
- `crates/ui/src/routing.rs`: the drawer heading's Explain button, inline help, the one-shot
  scroll to a shown control's routes, and `reaches` (legacy route names, also used by
  `UiState::validate`).
- `crates/ui/tests/explain.rs` (new): 19 tests plus the ignored coverage writer.

D01-R1-owned files were not touched: `browser.rs`, `library.rs`, `main.rs` and
`docs/find-play-save`.

## Verification

Cloud container: Ubuntu 24.04.4, 4 vCPU VM, rustc 1.94.1. No sound card, no ALSA sequencer,
no rtkit. Combined head tested: **4b478fd**. That is D02 (product last changed at e60300c)
merged with master containing D01-R1 (merge 4fa0402), plus one integration test. Later
commits change docs and evidence only. Every screenshot and the walkthrough were re-rendered
from the release build of 4b478fd. Eight of the ten PNGs came out byte-identical to the
0987db5 renders, so git shows no change for them; the two Save As shots and the video are
new.

- `cargo test --workspace` at 4b478fd: **479 passed, 0 failed, 15 ignored**, exit 0
  ([`evidence/test-workspace.txt`](evidence/test-workspace.txt)). New:
  - `crates/ui/tests/explain.rs` (19 tests, one of them `a_save_dialog_over_an_explanation_keeps_escape_and_the_document`,
    which covers D01-R1's Save As; real egui input through `show()` at 1280×800 and
    1440×900);
  - `help.rs` unit tests (3), including "every built-in parameter has help".
- `cargo clippy --workspace --all-targets` at 4b478fd: no warnings, exit 0
  ([`evidence/clippy.txt`](evidence/clippy.txt)).
- Focused: `cargo test -p kabl-ui --test explain`. To regenerate the coverage table:
  `cargo test -p kabl-ui --test explain write_help_coverage -- --ignored`.

## Real-app evidence

All real-app evidence is **scripted**. `docs/rack-migration/drive.py` uses xdotool on Xvfb
(1600×1000) and aims at the targets the app reports. The app is the release build of 4b478fd (combined head).

- **Screenshots**, 1440×900 and 1280×800, in A-light and A-dark. Files are
  `img/<size>-<name>.png`:
  - `light-module-help`: module explanation with inline help;
  - `light-macro-shown` and `dark-macro-shown`: the "Warmth" macro with the off-face ladder
    Drive revealed;
  - `dark-direct-pin`: the "Attack" pin shown in the rack;
  - `dark-save-as-over-explain`: D01-R1's Save As dialog over an open explanation (Escape
    closes only the dialog).

  Script: [`scripts/shots.txt`](scripts/shots.txt). Command:
  `DISPLAY=:97 KABL_ARGS=--perform python3 docs/rack-migration/drive.py docs/musical-controls/scripts/shots.txt 1440x900 OUT patches/palette/pad`.
- **Walkthrough with audio**: [`walkthrough.mp4`](walkthrough.mp4), 77 s, Evolving Pad at
  1440×900, recorded by [`record-walkthrough.sh`](record-walkthrough.sh) from
  [`scripts/walkthrough.txt`](scripts/walkthrough.txt). The steps (video time in seconds from `evidence/walkthrough-drive.log` minus `walkthrough-t0.txt`):
  1. 0:06 Hold a chord and explain "Warmth".
  2. 0:11 and 0:27 Turn Warmth up and down through its mapping, MIDI CC 20. The sound brightens and darkens,
     and the card shows `● live`.
  3. 0:17 / 0:22 Show Cutoff, then the off-face Drive.
  4. 0:32 Back to the card.
  5. 0:36 / 0:40 Explain and Show the direct "Attack" pin.
  6. 0:56 Bypass the Warmth → Cutoff route. The explanation row is struck through at once.
  7. 1:01 Undo with Ctrl+Z. The row returns.
  8. 1:06 / 1:10 Toggle Help, then close with Escape.

  Audio is 48 kHz with 256-frame buffers. The app plays through ALSA → PipeWire 1.0.5 into a
  silent null sink and is recorded from its monitor. The track's mean is −18.9 dB, peak −3.2 dB.
- **Stand-ins.** The "controller" is `examples/midi_player` writing to the `KABL_MIDI_PIPE`
  fifo, not hardware.
- **Timing on the VM**, from [`evidence/walkthrough-stats.txt`](evidence/walkthrough-stats.txt),
  reported separately:
  - execution: 37 late callbacks, worst 16.7 ms against a 5.3 ms budget;
  - arrival: 878 late;
  - 12 xruns;
  - RT priority refused (no rtkit).

  This is not laptop evidence. D02 adds no audio-thread code.

  **Audible dropouts, not clipping.** The track peaks at 0.69 of full scale (−3.2 dBFS) and no
  sample reaches 0.99, so nothing clips. It does contain six silent gaps, where every sample
  in a run of at least 64 is below 1e-6. These are buffer underruns on the VM, and they are
  heard as clicks:

  | Time | Gap length |
  |---|---|
  | 0:12.9 | 3 ms |
  | 0:23.7 | 8 ms |
  | 0:24.6 | 13 ms |
  | 0:24.9 | 3 ms |
  | 0:25.3 | 51 ms |
  | 0:44.8 | 8 ms |

  The cluster around 0:25 falls while Warmth is near full: it drives the ladder filter and
  opens the cutoff while a four-note chord plays. The worst callback execution, 16.7 ms
  against a 5.3 ms budget, was logged at about the same moment. The laptop checklist should
  listen for this at 256 frames.

## Limits

- Scripted clicks are software evidence. Whether a beginner finds this easy, and how the
  novice brighter/slower task goes, are owner observations still pending.
- Explanations show configured ranges, not measured values. Signal telemetry and why-no-sound
  diagnostics belong to D03.
- Host automation doesn't exist yet (D08), so explanations don't describe any.
- A shown target on a module whose advanced area floats ("Float over neighbours") gets the
  flash, but the outline is drawn under the floating area.
- The route rows sit in the drawer's lower scroll area. With many routes at 1280×800, the last
  rows can need a scroll.
- Patch replacement and module deletion with an explanation open are verified by the
  interaction tests (real egui input through `show()`), not in the recorded real-app run.
- **Integration state:** D01-R1 is merged in, and the combined head is verified and
  rechecked (see REVIEW.md).
