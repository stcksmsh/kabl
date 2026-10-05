# D09 acceptance dispositions

Every source/app pass below is bounded evidence, not owner approval. Product and per-artifact heads are in evidence/verification.json. Kosta resolved A03 as chronological project Undo with independent instance state; separate per-instance histories are not required. Engineering and final reviewer dispositions are recorded in REPORT/REVIEW. Neither merge nor owner approval is claimed.

| ID | Criterion | Disposition | Evidence / limit |
|---|---|---|---|
| A01 | Encapsulate/open/expose/edit/duplicate/Undo/save/reload | Source + actual widgets pass | 12 focused tests; workflow.json; standalone applied-log roundtrip |
| A02 | Flat/composite sound | Pass | Exact float PCM including poly voice, stereo effect and boundary feedback; REAPER accepted hash matches repaired D08 |
| A03 | Independent instance state and chronological project Undo | Source/app pass; owner hands-on pending | Kosta selected project Undo. A/B/A edits remain isolated; save/reload then Undo/Redo reverses complete states in chronological order. Separate per-instance histories are outside the agreed requirement |
| A04 | External routes retain identity | Source pass; app subset pass | Exact CableIds/endpoints/settings preserved; public route plugs resolve original leaf targets |
| A05 | Cues/pins/native identity | Source + host playback/recall pass; physical checks pending | Cue/clock inclusion/remap test, controller/pin omission test, unchanged native lanes; closed-editor real playback. Exposed-control native Write not freshly recorded; repaired D08 native gesture path reused |
| A06 | No added grouping latency | Source/PCM pass; physical latency unmeasured | No DSP wrapper/adaptor or metadata graph rebuild; existing plugin 64-frame latency unchanged. Device latency is not inferred |
| A07 | Nesting/recursion | Source pass; owner error walk pending | 8 levels accepted, 9 rejected, recursive parent/definition and malformed tree rejected before transaction |
| A08 | Voice/cycle rules | Source/PCM pass; owner listening pending | Existing compiler receives unchanged leaf graph. Exact poly/event/tail/stereo/feedback comparison plus workspace regressions |
| A09 | Embedded recall without library | Source + actual REAPER/source-free recall pass | Complete states equal at f32 precision after fresh-process save; packaged plugin mapped with unavailable factory |
| A10 | Standalone applied history | Source + actual widget save pass | Log replay rejects malformed ownership before install; atomic seeded-tree Undo and applied project Undo restore original graph and metadata. Host restart retains sound state, not full Undo history |
| A11 | Atomic malformed-state rejection | Source pass; owner UI error walk pending | Duplicate semantic/inner-label IDs, bad kind/binding/tree/depth/size/dependency fail before mutation; log/counters unchanged |
| A12 | Instance edit vs immutable publication | Source pass; owner publish walk pending | Versions never overwritten; edited song independent of missing/updated library; import uses fresh identities |
| A13 | D08 Host/Free + strict runtime contracts | Regression/source pass; host subset refreshed | Framework/native delivery untouched. Current workspace/plugin/Clippy/release/validator; final recall + closed-editor playback. Free-transition extended harness did not complete; reuse D08 transport evidence with exact provenance |
| A14 | Existing UI usable both themes/sizes | Actual captures + widget tests pass; owner feel pending | 1440×900 and 1280×800 client captures; compact cards scroll, Open reveals native modules, no D10 panel work |
| A15 | Portable package + fresh review | Source/artifact recheck pass; draft submission for owner review | Manifest/head/evidence index and independent reviewer; chronological history disposition finalized; runtime binaries and unaffected evidence reused with exact provenance |

Earlier D08 hardware/controller/listening/feel/physical latency/xruns/production-beta checks remain pending. Ordinary-render nondeterminism and dense-open backend failures are preserved. Failed D09 harness probes and corrected commands remain in the task-owned raw archive; none are counted as passes. Kosta selected history semantics; reviewer verification does not constitute hands-on owner approval or physical evidence.
