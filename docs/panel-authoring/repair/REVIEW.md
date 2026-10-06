# Independent repair review

## Final sizing, wheel and insertion review — 2026-10-06

A fresh independent reviewer, /root/d10_final_review, inspected the affected sizing/event/paint paths and rechecked production head **35eed79956dd702e0a62cd80be534d5c34dc9632**. Independent execution passed all 15 rack_editor tests and the nested-wheel test (16/16). The review confirms bounded UI-only changes; identities, persistence, chronological Undo and audio contracts are unchanged. No actionable source finding remains.

The reviewer flagged an early proposed all-sided scrollbar padding expansion because it could steal parent gestures outside the child. This was resolved before the product commit by using the scroll container's exact allocated scope rectangle with clip/layer checks. The reviewer also required genuine overflow evidence rather than treating wheel movement over already fitting oscillator content as scrolling proof. A tall native sequencer regression and actual private fixture now distinguish these cases. No approval is inferred from positive feedback about other UI parts.

Final binary capture/package artifact review is recorded in final/evidence/artifact-review.json after checks. Historical review sections below remain associated with their original heads.

Fresh independent reviewer `/root/d10_repair_review` reviewed the working repair against d702174b41c54cb5361643582f92c118cd4f1232, then rechecked the committed product at **5f7622a1aa9f44746e1f9d7a33a3c3ec3f9f8730**. The reviewer made no source edits and did not automate host sessions.

Four P2 findings were resolved:

1. Front insertion preview could place a neighbour at x264 but scene snapping redrew it at x270. Scene and committed coordinates now agree; the regression inserts before the first occupied item.
2. Add calculated row end from native modules only. It now includes composite faces; the composite-only Add regression preserves the original face position.
3. Inspect bound module on a parent face could select an invisible nested descendant. It now enters the leaf's direct owning scope.
4. More filtered away existing parameter leads and jack cables because it used the collapsed scene's old floating rules. Scoped cable drawing now retains both; a real-widget regression checks them.

Final source disposition: **pass, no unresolved actionable findings**. The reviewer rechecked shared native/default/authored gestures and ghosts, cancellation/Undo, scoped navigation, visibility/order metadata, modal controls, theme application and launchers. Independent checks passed: 11 repair unit tests, 3 focused interaction tests and Python parsing of four implementation scripts.

The review explicitly separates source correctness from the later real capture, release packaging, host recall and owner acceptance. Artifact and source-free results are recorded in REPORT.md and evidence/verification.json. Owner disposition remains changes requested; no merge or D11 authorization follows from review.

The same independent reviewer completed a read-only artifact recheck at documentation head 88ca98e61bd022fc01fff502e2ed01a57f0c6d74. All 204 packaged manifest hashes matched, release UI/plugin hashes matched final builds, normal and unavailable launches showed 19 and zero factory Sounds respectively, and both archived and relocated host recalls matched complete expected f32 state. Mapped bundled plugin and closed-editor native replay passed at 2, 6 and 13 seconds within 0.025. No actionable artifact defect remained. The pending package-status documentation was corrected and bundled documentation/manifest refreshed after that review; production binaries and projects were unchanged.
