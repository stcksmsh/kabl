# Independent review record

Initial independent reviewer subagent inspected base `f1e2c9ee204f96e348bf4c874159775d6ba41321`, head `f896c4e1dff647fd7cda2e3f0f3a31d1e01988d1`, code and validator/host evidence before workspace maintenance pruned that checkout. The reviewer made no changes. Findings:

- P0 known: pinned nice-plug CLAP loader trusts unbounded state length and aborts on random invalid state; 3 cutoff state reproducibility tests fail without host rescan. Host state acceptance blocked.
- P1 new: a rejected editor compile was promoted to serializable `Control.patch` on next repaint after `take_dirty()`; fixed by always retrying `submit_editor_snapshot` and retaining valid patch on failure, with focused test.
- P1 new: host cutoff affected DSP but rack display and patch field could remain stale; synchronized rack from host parameter on editor frame/reopen, with focused test. Closed-editor patch field can still differ until reopen; canonical host parameter persists independently.
- P2: collector retirement while editor closed needed a drain; added drain on host state load. Final retirement remains until another control/lifecycle opportunity.
- P2: 256 swaps per block produced a CPU burst; bounded to one per block. Wildcard choke handling added; pitch-only semantics remain limited.
- Further lifecycle finding: wrapper applies host parameters before `PersistentField::set`, which returns no rejection status; a bad decoded patch can partially mutate sound while state load reports success. Documented as unresolved framework-level blocker.

The reviewer confirmed file boundaries, real engine/editor path and absence of a plugin-owned device in the reviewed snapshot. **Final recheck of reconstructed/fixed head is pending.** The pruned original commit was not published; this record does not claim the reviewer inspected the reconstruction yet.
