# Independent Factory Bank Review

## Scope and Provenance

This review was performed by a fresh reviewer subagent, separately from implementation. Starting base: `23075cd713884903a8985c88ab43bd1817477308`. Initial reviewed product/content head: `f5dd27af82f08474700a7c45a5a3ef033a13003c`; documentation checkpoint: `6d5903291c190432f0f265e9fbaca0b54877a617`. Runtime binary source: `160b3c2b20b8eb1437756f9dd7f090b8acacc0f2`.

I inspected the runtime browser/library/Perform diff, authored builders and bindings, saved factory definitions, focused test schedules, audio metrics and preview inventory, actual application frames, reproduction scripts, REAPER recall/native-replay receipts and the relocated review-candidate bundle receipts. This is code/content and evidence review, not owner listening or physical-controller testing. I did not independently rerun the entire workspace or claim physical audio observations.

## Initial Findings

1. **[P2] The long performance does not demonstrate the muted keyboard entries.** At the initial head, `crates/ui/tests/factory_bank.rs:273` schedules cue changes and `:286` macro changes, but never changes Sound Palette's layer mixer faders. `crates/ui/tests/sound_palette.rs:1090` initializes Strings, Pad and Lead to zero; cues at `:1188` change sequencer banks only. Therefore the 72-second take contains sequenced bass/percussion and Breath, but its embedded strings/pad/lead cannot become audible. Add declared timed layer-fader gestures, retain real runtime controls, refresh the long preview and headroom/tail metrics, and align the guide/report with the actual demonstration.

2. **[P2] Sound Palette's guide and tempo tag contradict its starting graph.** `crates/ui/tests/support/factory_bank.rs:18` says Run starts bass and percussion and tags the piece `112bpm`. The actual builder has a 104 BPM clock (`crates/ui/tests/sound_palette.rs:1065`) and all four sequencers begin on Rest (`:1080`). Run alone does not start audible sequence playback. Correct the guide to request an appropriate cue after Run, preserve the atmospheric startup, and replace the stale tempo tag in both metadata and embedded guidance where applicable.

Both findings are content/verification issues, not native parameter identity or callback-ownership regressions. They were resolved and independently rechecked below.

## Other Review Results

The curated inventory is six sounds, four sequences and two performances; nine existing IDs are retained and three additions fill missing roles. Existing factory entries remain accessible. Broad purpose matching coexists with old role categories, and complete graph-derived keyboard/sequence flags protect metadata lacking the new conventions. Browser replacement clears preview/key ownership through the existing path and infers stopped sequence loads from the actual graph. New prepared controls use existing routes, pins, CC bindings, banks and cues without new DSP/runtime dependencies or renumbering original modules.

Normal and all-macros-one audio metrics cover all twelve entries, with finite samples, declared stop/tail windows and measured headroom. The long release measurements are not proof of physical stuck-note freedom. Reviewed 1280x800 and 1440x900 browser/Perform frames provide readable representative light/dark workflows. The GUI receipt records 24 browser loads and changed-macro edit/save/full restart. Existing Taller/scroll behavior is retained for larger control sets.

The candidate portable receipt records manifest-verified bundled binaries/definitions, normal bundled discovery and a separate unavailable-factory complete-state opening. REAPER receipts record complete f32 state equality on initial/full restart and closed-editor native parameter replay; mapped-plugin paths identify the bundled candidate plugin. These are relocation and recall/replay checks, not deterministic-render or physical-latency evidence. The final owner bundle must be recreated after findings are fixed and checked against its own manifest.

Workspace/Clippy/release/validator pass claims are supported by retained logs, not reviewer reruns: 653 workspace passes, zero failures, 24 ignored utilities; validator 35 successes and nine capability skips. Release binaries remain at the declared runtime head; subsequent source changes concern tests/content rather than runtime code.

## Acceptance Limits

Owner listening, musical usefulness, controller feel, physical latency/xruns and production-beta approval remain pending. Ordinary host-render nondeterminism, the declared full warm-up render policy, dense-editor/backend performance limits and strict lifecycle/profile assumptions remain inherited. This review neither resolves nor relabels those limits. Draft submission is not merge or owner approval.

## Final Recheck

Final reviewed product/content head: `07c723fd4983366696a99b1b9d4e7069c636434b`. The first correction landed at `79d71beaeaebe3054adcdfb4cd75e7da7fe61342`; its documentation/artifact checkpoint was `0f78c512218462b048a28ae91bc936b14f1202fe`. Runtime source/binaries remain at `160b3c2b20b8eb1437756f9dd7f090b8acacc0f2`; the intervening changes affect authored content, tests and evidence, not runtime implementation.

Finding 1 is resolved. The long render now applies 25 real runtime layer/send parameter changes at 2/12/24/36/48 seconds. Strings, Pad and Lead receive positive mixer levels overlapping subsequent keyboard phrases; cues still operate actual editable sequencer banks. The schedule is declared in README.md. Refreshed performance metrics record 25 layer changes, 24 macro changes, peak -5.9308 dBFS, RMS -23.9706 dBFS and final-second RMS -95.6128 dBFS. The refreshed preview and editable project accompany the evidence. This establishes the missing gesture coverage, not listening approval.

Finding 2 is resolved. The saved guide accurately describes Rest startup, Run followed by cue selection, keyboard-layer faders and 104 BPM. Metadata now contains only the authored 104 BPM tag, not 112 BPM. The writer removes obsolete BPM tags before applying the authored tempo; the focused regression test asserts positive 104 BPM and negative 112 BPM search results. The initial repair had left the old tag alongside the new one; the final head explicitly corrects that incomplete repair.

Final owner bundle reviewed: `/home/stcksmsh/.codex/visualizations/2026/10/08/01a11be1-c6f3-7fe2-b4e6-f33210e9ea62/factory-bank-owner-test`. At recheck its manifest product/packaged head is `07c723fd4983366696a99b1b9d4e7069c636434b`; its initial packaged checkpoint is retained separately. I independently hashed its 196 immutable payload files against the manifest: zero mismatches. Mutable host/profile/log directories are excluded from immutable payload hashes, as owner launches legitimately change them.

The final portable receipt identifies that actual owner bundle, outside-checkout working directory and successful normal populated browse/open plus separate unavailable-factory complete-state opening/save. REAPER initial/full-restart mapped-plugin receipts identify this owner bundle's `plugins/kabl.clap`; complete f32 state equality and closed-editor native replay remain successful. Final workspace/Clippy logs and verification receipt identify the corrected content head: 653 passes, zero failures, 24 ignored utilities and completed Clippy. The review is based on inspection of those retained execution receipts, not a second reviewer-run full test suite.

No unresolved actionable findings remain within the reviewed engineering/content scope. Final submission may update documentation/receipt heads and copy this review into the same new owner bundle; runtime binaries, factory definitions and audio payload must remain identical to the reviewed content. Listening, hardware and inherited limitations above remain pending or unchanged. This is independent engineering review clearance for draft submission, not merge or owner approval.
