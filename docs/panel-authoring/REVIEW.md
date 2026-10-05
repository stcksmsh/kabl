# Independent D10 review

Reviewer: fresh subagent `/root/d10_review`. Base: `db33f6a2a9c0274e272d702142ad5d63d39271a6`. Initial reviewed product head: `ffb9929bb5b23b9fa629c48f31888e8b3a38db3f`. The working tree also contains the implementer's uncommitted empty-rack Fit repair in `crates/ui/src/lib.rs`; that repair was read and its focused regression passed. This is an initial source review, not final engineering acceptance. Final product-head and evidence recheck remain required.

## Findings

### D10-R1 — P1: deleting a placed leaf produces a project that cannot reload

**Location:** `crates/core/src/state.rs`, `PatchState::apply`, `Op::RemoveModule` branch; called by `crates/ui/src/editor.rs`, `PatchEditor::remove_module`.

**Scenario:** Create a composite, expose a leaf parameter, place its control on an authored panel, then inspect internals and delete that leaf. Removal prunes `controls` and `ports` but retains the corresponding `panel.placements` entry. The current document now violates the panel binding invariant. Ordinary save succeeds, but subsequent project loading fails; compile, package export and complete host-state validation also reject this state. This affects the normal in-scope internal editing workflow and can make a saved project unavailable after restart.

**Independent reproduction:** A temporary Rust harness compiled against the current local product libraries created a composite containing the default SVF filter, exposed `cutoff_hz`, applied a valid knob placement, then called `remove_module`. Actual output:

```text
removed controls=0 placements=1 validation=Err("Panel binding 3: missing exposed ID or wrong control kind")
reload=Some(Composite("Panel binding 3: missing exposed ID or wrong control kind"))
undo exact=true
```

Harness: `/tmp/d10-review-remove.rs`; saved reproduction: `/tmp/d10-review-save`. Undo restores the entire original state, but does not make the intervening invalid save acceptable.

**Requested correction:** Prune placements whose exposed IDs disappear when removing a module, in the shared core operation alongside exposure pruning. Preserve the existing exact inverse metadata restoration. Add a regression covering placed control and jack removal, validation, save/reload and exact chronological Undo/Redo.

**Disposition:** Resolved at `ca8b504a7d35e1aeee7398ea622feda4bf747b95` (functional fix in preceding `ffc912f` commit). The shared core removal prunes placements after exposure pruning, and the existing inverse restores exact composite metadata. Independent source recheck and executable verification passed; final artifact acceptance remains pending.

## Initial coverage and verification

The reviewer read the direct base-to-head product diff, D10 brief/design/README, workflow, D09 design/review contracts, repaired D08 callback audit and vendor strict profile. Source coverage includes typed metadata, geometry/overlap bounds, semantic binding checks, embedded PNG limits and document decode budget, bounded package/artwork reads, export refusal of existing files, atomic composite transactions, instance copies, stable interface/leaf/native targets, authoring draft/apply/reload/default flows, authored face/fallback rendering, focus geometry, texture preparation and retirement, sectional rendering and fonts.

No vendor or audio-processing path is changed in the reviewed diff. Complete-state validation adds PNG decoding on the control side. Image bytes remain presentation metadata and do not enter compiled DSP nodes. This preserves the reviewed D08 ownership boundaries within this diff; it is not a repeated exhaustive framework or machine-code audit.

Independent commands run in the D10 checkout:

- `cargo test -p kabl-ui --test panels`: 5 passed. Includes exact panel history/package round trips, atomic asset/binding/overlap rejection, missing-art/unplaced fallback validity, aggregate budget rejection and 500 guarded PCM-equivalence blocks.
- `cargo test -p kabl-ui --lib authored_geometry`: 1 passed.
- `cargo test -p kabl-ui --lib empty_rack_fit`: 1 passed, with the implementer's uncommitted repair.
- `cargo test -p kabl-ui --test composites`: 12 passed, including D09 chronology, copies, closed packages, real egui widget aliases and guarded PCM equivalence.
- `cargo test -p kabl-clap panel_tests`: 1 passed; complete panel recall retains native target identity. Existing vendor warnings were emitted; the command exited successfully.

The original focused tests miss leaf deletion after panel placement. The independent reproduction above exposes that gap.

## Limits and pending evidence

Final real GUI/native scaling/host/bundle evidence is being refreshed and has not been accepted by this review. Initial source/test results must not be represented as final artifact acceptance. Owner listening, controller, physical latency/feel and production-beta checks remain pending. Ordinary-render nondeterminism and dense-editor backend/performance limits remain open. No merge or owner approval is implied.

## Source repair recheck

**PASS at `ca8b504a7d35e1aeee7398ea622feda4bf747b95`.** The reviewer read the full `ffb9929..ca8b504` source diff: shared `RemoveModule` pruning, control-and-jack deletion regression, empty-rack Fit guard/regression, and formatting-only cache retirement. No new actionable source finding within covered paths. Product sources were clean at recheck; untracked evidence and documentation remain in progress.

Independent commands at this head: `cargo test -p kabl-ui --test panels` (6 passed), `cargo test -p kabl-ui --lib empty_rack_fit` (1 passed), and `cargo test -p kabl-ui --test composites` (12 passed). The original temporary reproduction was rebuilt against compatible current local libraries and now reports:

```text
removed controls=0 placements=0 validation=Ok(())
reload=None
undo exact=true
```

The added regression exercises placed knob and jack metadata, validates the deleted document, saves/reloads it, then checks complete-state equality after Undo and Redo. The source recheck resolves D10-R1; it does not accept unfinished GUI/native scaling/host/bundle evidence or owner checks.
