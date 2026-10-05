# Independent D09 review

Reviewer: fresh subagent `/root/d09_review`. Base: f870b3c4316b9ea819d35344de09f6b7d24c59ca. Initial product: af684837642348d51debe69fe763e152e6fc3e8f. Final source recheck: `a337f2c1eae867671f9deabadfa10f681de2bb26` (earlier rechecks recorded below).

The reviewer read the direct product diff, brief, repaired D08 REPORT/REVIEW, callback audit and vendor strict profile. Coverage included independent duplication/state, nested ownership and recursion, stable aliases, unchanged compilation/scheduling/voice/feedback, embedded persistence, atomic package rejection, pins/cue/clock remapping, native target identity, project Undo and real UI correctness. Independent focused tests passed: initial 8, final 9.

| Finding | Scenario | Correction | Final disposition |
|---|---|---|---|
| P2 card overlap | Raw stored leaf positions may coincide; rack packing moves leaves while card remained raw and was obscured by an unrelated leaf | Pack visible cards against actual rendered leaf/card rectangles; include cards in Fit bounds; preserve saved leaf positions | Resolved; packed-position regression passes |
| P2 copy label target | Default label named original #2 although copied alias targeted a fresh leaf | Regenerate default binding labels on remap, preserve user labels | Resolved; copy binding-label regression passes |
| P2 duplicate label keys | Inner label BTreeMap accepted last duplicate value despite malformed-package contract | Unique nested-label deserializer and atomic decode regression | Resolved |

Earlier reviewer disposition: **source recheck PASS at 22b8b8bf6b5c712a8461d34d149aa7e106c0b64f**. No remaining actionable source finding within covered paths; D08 callback ownership paths unchanged. This is bounded source review, not exhaustive DSP/OS/machine-code proof.

The reviewer explicitly excluded owner-history approval and unfinished final app/host/package evidence from the source pass. Kosta's history choice remains pending. Final artifact/evidence recheck is recorded below when complete; neither source review nor tests establish owner approval, physical controller/listening/feel/latency or production-beta acceptance.

Final source increment recheck: **PASS at e5646b8609b196aad7ce4fd98f6421f422671cf2**, independent 10/0 focused tests. The added 16KiB host-envelope reserve is conservative; dependency test proves external cue/clock preservation, explicit inclusion and fresh internal remapping. Reviewer read final README/design/checklist and raw workflow, recall and closed-editor playback. No new actionable source finding. App/host artifact provenance remains explicitly at 22b8b8b where unaffected; final binary/package verification indexed separately.

Persistence source recheck: **PASS at a337f2c1eae867671f9deabadfa10f681de2bb26**, independent 11/0 focused tests. Core load rejects invalid final ownership before returning a document; seeded ownership uses one invertible Group, so nested Undo cannot orphan children. No new actionable source finding within reviewed scope. Earlier artifact pass covered e564 binaries; latest binary/package records are refreshed separately.

Final artifact recheck: **PASS at product a337f2c1eae867671f9deabadfa10f681de2bb26**. Reviewer independently verified 2,394 owner manifest files with zero mismatches, evidence and packaged binary hashes, exact two-instance f32 recall, decoded renders 8/9 with PCM SHA256 c535792b…e2cb7b, and validator35 successes/9 skips. Earlier capture/playback heads, failed probes, unrefreshed Write/extended-Free checks and owner limits remain explicit. No remaining actionable finding within reviewed scope. Required history choice still prevents engineering completion and PR submission.
