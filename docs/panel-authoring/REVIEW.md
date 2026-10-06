# Independent D10 review

Current owner disposition: **changes requested** after hands-on use. The [hands-on repair package](repair/README.md) and [repair report](repair/REPORT.md) supersede interaction/theme and normal-launch claims below. Earlier evidence and owner bundle remain preserved.

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

## Final source and artifact recheck

Reviewed artifact head: `495c1ba8d4646d44bf2e4a87c754da8d5baf7943`. Product/test head: `25f20ae8547bc367403414467040a8475cffa34f`. Built production source: `ca8b504a7d35e1aeee7398ea622feda4bf747b95`. The reviewer directly compared source changes after the source repair pass: only the empty-rack test fixture initialization changes; production behavior and binaries are unchanged. D10-R1 remains resolved.

Independent artifact checks confirmed all **63 owner-bundle manifest files**, with **zero hash mismatches**, and both release binary hashes match verification.json. The bundle manifest names the actual reviewed artifact head. Raw workspace log totals independently sum to **634 passed, 0 failed, 22 ignored**. Clippy and release logs finish successfully; validator JSON records **35 successes and 9 declared skips**, with no failure. These final broad checks are implementer-run evidence, inspected independently; they are not relabeled as reviewer reruns.

The reviewer inspected representative real application authoring, both theme/size, dense rack, portable release, native 1.5-scale and fallback screenshots. Public labels, values, selector choices, help and fallback controls remain readable within the captured views. Reproduction scripts use private X11 displays and real production widgets; screenshots are application output, not the generated material reference. The private native scale capture has the recorded physical 1920×1200 size for logical 1280×800.

Both raw host recall files (normal and relocated-bundle runs) independently decode to two complete f32-normalized states equal to the bundled arrangement, including two embedded panels. Each native replay records nine observations; maximum absolute errors are **0.00453125** and **0.00453126**, below the declared **0.025** tolerance. The relocated host mapping explicitly identifies the plugin inside the portable bundle. Launch scripts derive bundled paths from their own location; no source checkout/artwork/library lookup is needed for the embedded projects.

### D10-R2 — P2: required walkthrough video missing

At artifact head 495c1ba, the media directory contains selected screenshots but no walkthrough video. `docs/product-research/AGENT-WORKFLOW.md`, “Batch evidence package,” requires “A walkthrough video and selected screenshots.” The existing scripts and screenshots support the underlying operations, but do not satisfy that explicit artifact requirement.

**Requested correction:** Add a curated video recorded from the real task-owned application workflow, identify scripted input and originating production head, and include it in the owner bundle/manifest. Do not substitute a slideshow or generated mockup for a real application capture.

**Disposition:** Resolved in artifact repair `12f593256e06f4eb3be093ffd0deba1c5131b74a`; the missing-video finding was open at the 495c1ba checkpoint. Final repair verification is recorded below. Owner approval and merge remain separate. Hardware/listening/controller/physical latency/feel, ordinary-render nondeterminism and dense backend/performance limitations remain explicitly pending in REPORT/CHECKLIST.

## Final video repair and bundle recheck

**PASS at artifact head `12f593256e06f4eb3be093ffd0deba1c5131b74a`.** Direct diff from 495c1ba contains the walkthrough video, its script/provenance and document updates; production/test source is unchanged from `25f20ae8547bc367403414467040a8475cffa34f`. D10-R1 and D10-R2 are both resolved. No remaining actionable finding within the reviewed scope.

Independent ffprobe verification confirms a complete H.264 MP4, **1440×900**, **509 frames**, **21.208333 seconds**. The reviewer extracted and inspected actual frames at 10, 13, 16 and 20 seconds: the real Design window contains the edited label Tone, the applied face shows Tone, chronological Undo restores Brightness, and Public controls displays the original exposed controls and ports. The saved checkpoint independently contains Brightness. The script records the task-owned real release application via X11; evidence identifies scripted input, no recorded audio, actual production head and release binary hash. This is UI workflow evidence, not listening or physical-input proof.

The refreshed bundle manifest names 12f5932 and independently validates **64 files with zero hash mismatches**, including the same walkthrough MP4. The prior release binary hashes, complete host recall/native replay, owner limits and source dispositions remain unchanged. After committing this review, the implementer must refresh the bundled review/manifest and record the final submitted documentation head; that receipt must preserve the distinct built/tested/reviewed heads.

Engineering review is complete for the agreed D10 scope. Owner material/listening/controller/physical latency/feel and production-beta acceptance remain pending; ordinary-render nondeterminism and dense backend/performance limits remain open. No merge or owner approval is implied.
