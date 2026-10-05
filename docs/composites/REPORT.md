# D09 implementation checkpoint

## Disposition

Engineering: implementing; common composite workflow and independent source recheck pass. Engineering completion and draft PR submission are held for Kosta's required history choice. Merged: no. Owner review/approval: pending. No merge, D10 work or messages to other chats occurred.

The dependency gate passed before product edits. Live PR #11 was merged at f870b3c4316b9ea819d35344de09f6b7d24c59ca; fetched master equalled that commit, with no intervening changes. Updated repaired D08 contracts replaced historical pre-repair references. D08 tested/reviewed/packaged f7157619836075d0d7704e59be5d66e6224890b1; submitted eb22ce7db9be63831f54e97d7308dbff42b3238f. Strict-profile R6 is closed within its declared assumptions; earlier hardware and production-beta gaps remain.

## Exact heads

| Role | Head / disposition |
|---|---|
| Starting merged master | f870b3c4316b9ea819d35344de09f6b7d24c59ca |
| Brief/common-scope record before product edits | 36b7a36f32056d13bafa0d5d5fe7011bb4a4d385 |
| Initial implementation | af684837642348d51debe69fe763e152e6fc3e8f |
| Packed UI/identity review fixes | 22b8b8bf6b5c712a8461d34d149aa7e106c0b64f |
| Current product / workspace tested / independently reviewed | a337f2c1eae867671f9deabadfa10f681de2bb26 |
| App captures, real widget workflow, first final host recall/playback | 22b8b8bf6b5c712a8461d34d149aa7e106c0b64f; later source changes lower package byte cap, validate project ownership on load and seed nested ownership atomically; DSP/graphics unchanged |
| Packaged product | See evidence/verification.json and source-free owner manifest; final release is built from a337f2c1eae867671f9deabadfa10f681de2bb26 |
| Submitted PR head | None: required history disposition pending; pushed checkpoint SHA is recorded in task CHECKPOINT.implementation.md after commit |

An artifact-only checkpoint commit may follow the product head. It does not change tested/reviewed/built product source. Final per-file hashes, binary identities and command status are in `evidence/verification.json`; no artifact is relabeled as tested at a later source head without provenance.

## Delivered behavior

Typed embedded ownership and stable public aliases preserve original leaf/cable IDs and all existing DSP scheduling. Grouping is metadata only. Encapsulate, open real modules, expose/edit, independently duplicate, chronological Undo/Redo, save/reload and immutable publication/import are implemented. Copies receive fresh internal identities and pin positions; external cables, CC/buttons and native lane assignments are deliberately omitted and explained. Cue/clock remapping preserves same-song external references and requires closed dependencies for portable export/import. Library changes never alter existing songs.

No composite runtime, wrapper block or recursive engine exists. Existing graph, state, runtime controls, perform widgets, routing and user library directory remain authoritative. Strict callback ownership, bounded handoffs, complete-state publication, legacy host formats and native identity paths remain unchanged apart from accepting composite metadata/version 3.

## Verification

Current workspace: **624 passed, 0 failed, 22 ignored**. Strict workspace Clippy and release build pass, with the unchanged vendor dependency warnings retained separately. Focused composites: **11 passed**; plugin suite: **26 passed** within the workspace. Validator and source-free packaged checks are indexed with their actual binary/head in verification.json. Prior final validator at 22b8b8b: **35 successes, 9 skips**.

Exact PCM: original/grouped voice, stereo chorus/reverb and a delayed-feedback boundary route match bit-for-bit with guarded allocation-free process blocks. Actual REAPER uses two plugin instances, original native lanes, scripted MIDI and tempo data. Accepted 36-second stereo float32 renders use the declared initialized/non-anticipative/device-closed/full-warm-up serial policy; decoded PCM SHA256 equals `c535792b70919e23b5f8b1f25dd92333f397b6bcbb5e9c3ccb2e5bb1c7e2cb7b`, matching repaired D08. Raw warm-ups are retained and excluded from accepted equality. Complete embedded states match at f32 precision after fresh REAPER save/recall with the source library unavailable.

Real X11 widgets selected four leaves, encapsulated, duplicated, edited the original public alias, performed project Undo/Redo, opened both instances and saved. Saved cutoffs were 130 Hz and 3000 Hz with disjoint leaf sets. Exact 1440×900/1280×800 client screenshots cover both themes and internals. Silent walkthrough and audible scripted engine/actual REAPER rendered examples are included; none is physical input or owner listening proof.

Closed-editor real REAPER playback checks current arrangement envelopes at 2, 6 and 13 seconds within .025 normalized units under the declared non-anticipative profile. Native bases show the expected processing-ahead offset. The first reused D08 replay harness targeted its recorded project rather than this arrangement and failed; anticipative playback exceeded the chosen tolerance; extended-range attempts failed to complete, including a corrected nil-item harness error. Those probes remain in the raw archive and are not acceptance passes. No newly verified Free transition beyond song end or native exposed-control Write recording is claimed. D08 strict-profile transport/native gesture evidence is reused only because those paths remain unchanged; provenance is `docs/host-production/evidence/strict-profile/index.json`, product f7157619836075d0d7704e59be5d66e6224890b1. Current workspace protects Host/Free/automation/state contracts.

## Independent review

Fresh reviewer `/root/d09_review` found three P2 issues: raw-position card overlap, copied default labels showing original target IDs and inner duplicate label-key acceptance. All were fixed with focused regressions. The reviewer rechecked exact source head a337f2c1eae867671f9deabadfa10f681de2bb26 and independently ran all 11 focused tests; no remaining actionable source finding within the reviewed scope. REVIEW records coverage and evidence limitations. Review does not settle owner history or hardware approval.

## Remaining required decision and limits

Kosta must choose chronological project Undo, which reverses the latest project action across instances, or separate per-instance Undo, which must preserve newer other-instance edits and define conflicts for shared routes. Current implementation demonstrates the former. BRIEF/A03 deliberately remain unresolved; independent state is not mislabeled independent histories. No final engineering-complete or submission status is asserted until this disposition is supplied.

Owner hardware/controller, listening/feel, physical latency/xruns and production-beta checks remain pending. D08 ordinary-render nondeterminism, 64-frame plugin latency, dense2open backend ERR2 and process maximum5491.27µs exceeding period5333.33µs are preserved. Current screenshots' callback indicators are incidental standalone observations, not marginal timing or device evidence. Strict-profile off-audio backlog and valid-host assumptions remain documented.

Next action after Kosta's history choice: finalize BRIEF/history acceptance, implement/retest/review any necessary history behavior, refresh only affected evidence, commit/push and create/attach D09 draft PR. No additional merge permission is sought or required because no merge is authorized.
