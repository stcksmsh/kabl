# Handoff — for the next blank-context agent

Replaces the earlier handoff prompt. Read this, then `docs/STATUS.md` (top section) and the
last entries of `docs/decisions.md`. Trust code and git over older docs.

## Where things are

- Branch `master`, local only (not pushed). Modulation slice closed out at commit `7361270`
  plus the docs commit that adds this file (`git log -3`). Baseline before the slice: `e6db377`.
- Workflow: commit small working changes straight to `master`; no branches, PRs or pushes
  unless Kosta asks. Leave `.ai/` alone. AIW/Recall not in use.

Launch (real audio + MIDI; first non-"Midi Through" port):

    cargo run --release -p kabl-ui -- --patch patches/reference            # 1440×900
    cargo run --release -p kabl-ui -- --patch patches/reference --size 1280x800

## Accepted decisions (owner-confirmed)

Visual / layout (not built yet — see next scope): A-light / A-dark core modules; illustrated
custom skins with `labels_on_art` default false; user-selected primary controls, advanced
controls expand in place; Hidden cables use the stable rack; expansion pushes neighbours by
default, float is a setting; 1440×900 default, 1280×800 usable minimum; compact layout later
and optional.

Modulation (built):
- Any knob accepts modulation cables. Cable, ring, lane/dot, badge and drawer row are the same
  route. Each route: amount (signed = polarity), invert, bypass (keeps amount). Default +25 %
  on drop.
- Knob body = base. Ring = the **selected** route's amount only. Inspecting a knob shows one
  lane per route with a draggable dot (single source too); collapsed multi-source knobs show
  thin display-only rings, pressing them opens the lanes.
- Single source auto-selects only when the knob becomes inspected. Removing the selected
  source clears the selection even if one remains; never silently edit another route.
- Vertical drag everywhere (common default); Shift = ×0.1; Escape mid-drag cancels with no
  undo entry; a completed drag is one undo step.
- Math: taper-space sum of all active routes, one clamp, back to units; block rate (64
  samples); unipolar/bipolar/pitch sources scaled by nominal range.
- Envelope timing per envelope, saved: CONTINUOUS default (Surge/Vital behaviour), KEY-TRIGGER
  captures A/D/R at note-on and uses them through release; sustain always live. No per-route
  latching.

## Production vs prototype-only

Production (`crates/ui/src`, engine, core, file format schema v2): everything under
"Modulation" above; drag-to-knob; routing drawer (select, amount, invert, bypass, remove, base
entry, computed-range text); All/Focus/Hidden; stepped selectors (CONT/KEY, waveforms);
exact undo/redo incl. grouped repatch and module delete with cables; save/reload; live-edit
safety (MIDI through swaps, audio-thread state carry, lock-free swap queue, reclamation,
linear crossfade).

Prototype-only (`crates/ui/examples/rev2_proto/`, reference for the next scope): A/A-dark
themes, illustrated skins + contrast warning, primary/advanced controls and in-place
expansion (push/float), zoom and pan polish, toasts, `+40 %` text entry on the plug, x-ray
cable fading, rich LFO. The existing production canvas is still the old Eurorack/Patchbay
view with one demo skin (`osc.va`).

## Verification evidence

- `cargo test --workspace`: 174 pass (2 ignored = script generators). Clippy clean; fmt clean
  except the untouched `rev2_proto` files.
- Key suites: `crates/engine/tests/{live_edit,modulation,env_timing,legacy_sound}.rs`,
  `crates/core/tests/{legacy_format,replay_proptest,log_unit}.rs`,
  `crates/ui/tests/{interaction,editor_undo}.rs` (real egui input through `show()` at both
  sizes).
- Real binary, real X input: `docs/modulation-slice/{xdotool-walkthrough,closeout-real-x}.sh`
  — saved patches match expectations at 1440×900 and 1280×800.
- Real app audio + screen muxed (PipeWire null sink, MIDI via `aplaymidi`):
  `docs/modulation-slice/record-av.sh`. Offline renders: `reference_patch`, `live_edit_render`
  examples. Benchmark: `bench_reference` example (6 routes ≈ 17.8 µs/block vs ≈ 15.4
  routeless, 8 voices; details in `docs/benchmarks.md`).
- Kosta heard renders/recordings remotely: CONT/KEY difference clear, live edit clean.

## Awaiting Kosta (hands-on, cannot be automated)

1. Grab individual source dots, the ring and the knob body with a real hand: any mis-grabs?
2. Hold a MIDI chord (real controller) while editing base and depth.
3. Release keys during edits: no hanging or cut notes.
4. Shift fine drag; Escape mid-drag; undo/redo.
5. Save, restart with `--patch <dir>`, check routes/amounts/bypass/timing.

Fix concrete problems he reports within this slice before moving on.

## Unresolved / known limits

Block-rate modulation only; no hysteresis on stepped destinations; pitch full scale ±60 st;
mixer's four `level` params share a name (route reaches only the first); state carry is
O(modules²) per swap; jack inputs take one cable (replace on connect); engine output is quiet
(voice averaging); Pi 4 unmeasured; drawer sliders don't have Shift/Escape.

## Recommended next scope

Approved rack UI migration into production `kabl-ui`, reusing rev2_proto appearance but real
editor/engine state: A/A-dark themes, skins (`labels_on_art`), primary/advanced controls with
in-place expansion (push default, float setting), zoom. Keep the modulation controls in
`crates/ui/src/routing.rs` working through the migration. Not in scope: sequencers, effects,
another prototype round.

## Files

- Core: `crates/core/src/{op,state,log,format}.rs` (PortRef::Param, UnsetParam, Group, schema v2)
- Engine: `crates/engine/src/{compile,patch_engine}.rs` (routes, carry, MIDI, swap queue)
- Modules: `crates/modules/src/{info,module}.rs`, `builtins/env_adsr.rs` (timing), `StateBuf`
- UI: `crates/ui/src/{lib,routing,editor,main}.rs`
- Reference patch: `patches/reference/`; slice doc: `docs/modulation-slice/README.md`
- Design specs: `docs/design/revision-2/{INTERACTIONS,PROTOTYPE,REVIEW}.md`
