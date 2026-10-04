# Independent review

Reviewer: fresh `/root/d07_review` subagent, read-only review against prerequisite base
`91777165cadba5ce81a1d7fdbc4de78563f95ca4` and the authorized BRIEF.md.

Initial source checkpoint `b2fef9e` had eight findings: metadata-only recall, fixed eight-second
tail, editor/load resource-limit mismatch, absent runtime feedback, incomplete editor seeding,
collector internal-storage leak, identity overflow, and framework MIDI queue allocation before
production processing. All were treated as substantive; no completion approval was requested.

Independent recheck inspected `94a22bf5c4675a859f4f708ccdc41ebe81e4b902` and reran eleven plugin
tests successfully. Reviewer resolved R1 and R4–R8. Its outer-pointer/lifecycle delegation and
bounded MIDI fallback inspection found no outstanding defect. Concurrent later edits are not
covered by that reviewed head.

Remaining findings at that head:

- R2: a gated modulation route into a free oscillator falsely proved a finite tail; KEY-TRIGGER
  could retain a long envelope release after the current knob was reduced.
- R3: failed topology followed by a knob edit could be promoted into persistent plugin state.
- R9: advertised monophonic host gain modulation changed readback but not audio.
- R10: retaining the first 128 parameter events lost the final host gain.

Follow-up fixes restrict tail proof to named silence-preserving audio processors, discard
parameter/control paths from that proof, retain the maximum release bound in KEY-TRIGGER mode,
semantically compile candidates after a failure before accepting them, render modulated gain
while persisting its base, and coalesce saturated value/modulation events separately to their
last values. Tail traversal is memoized. Tests cover failed topology plus unrelated knob edits,
free-oscillator modulation, latched-release bounds, 257 gain events plus modulation, exact audio
scaling, raw MIDI flood release recovery, reset and reactivation. Standalone's intentional
runtime-control behavior following compile failure remains unchanged (21 regressions pass).

Final independent source/evidence recheck is still required after final product changes and
actual-host/package verification. This document does not approve D07 or owner sound/feel.

## Source recheck at c9541bc

Reviewer independently inspected `c9541bcf9caaa377767caf6907c29ebbd694369e` and reported
R2/R3/R9/R10 resolved within the declared 2,048-event intake bound, with no new substantive
source finding. It reran all eleven plugin tests successfully and independently decoded the
states in two-instances.rpp, reopened.rpp and after-cycles.rpp: exact equality across those
snapshots, with the two distinct module/cable/gain combinations retained. It corroborated
host-control, ten-cycle and virtual-recording logs and validator 35-success/9-skip results.
This was a source/evidence checkpoint, not final D07 acceptance. The reviewer's subsequent
turn ended at the account usage limit; final package/remaining evidence review is pending.

## Final source and acceptance recheck — 2026-10-04

Fresh read-only `/root/final_review` inspected exact final product
`ff11434c6502d2cce5a3894544bec10f5ceea6d6`. It found no new substantive source defect.
The final toolbar change separates document/action controls into a second row with a
64-pixel panel; handlers and sound/state behavior are unchanged. Reviewer independently
reran plugin and UI library tests: 24 passed, zero failed.

Acceptance remains incomplete: independently corroborated repeat-render failure
(4,915,615 differing samples, maximum absolute difference 0.24531); editor-open timing
cases request opening without recording verified handles; physical controller, round-trip
latency and owner sound/feel remain pending. The ten-cycle log alone proves transitions,
transport and readback, not uninterrupted audio across every transition. Reviewer judged
a draft PR appropriate. Prior sound/state evidence is reusable for the toolbar-only change
with exact provenance; final binary/package/validator and actual toolbar views were refreshed
by the implementer afterward and are recorded separately in REPORT.md. No owner approval.

Final reviewer evidence recheck at ff11434 independently counted 597/0/22 workspace tests, 35 validator successes/9 skips, checked final package gain/render logs and exact decoded-state equality, and visually inspected both final 1280 theme screenshots: Save, Save As, Perform and Routing are distinct. It found no new substantive finding and supported draft submission. Final 1440 refresh, repeat render, verified editor-open timing, physical controller/latency and owner review remain open.

## 2026-10-04 independent diagnostic/package recheck

Reviewer `/root/closeout_review` received exact merged base91777165cadba5ce81a1d7fdbc4de78563f95ca4,
product ff11434c6502d2cce5a3894544bec10f5ceea6d6, initial submitted dd8fa7f70040ba0df3d1d2018cd377cefeeab733,
focused script diff, contracts and raw evidence. No product source change/new source defect.
Reviewer checked trace ABI against pinned CLAP/nice-plug and independently reproduced serial
PCM failure (4,915,249 samples; maximum0.2427147552371025). Suppressed mode forwarding
retains second starts: no causal resolution or product override justified.

P2 timing-scope finding: lifetime callback counts include startup/teardown. Disposition:
REPORT/README explicitly separate playback duration and callback arrival span; raw rows retained.
P2 runner-validation finding: unchecked results could be published. Disposition: retain per-case
JSON then assert completion/exit, callbacks, actual GUI checks and negotiated rate/buffer/mode.
All recorded cases complete/exit0 with three real GUI checks. Backend observation disposition:
inactive0→active1 device counters remain distinct from REAPER0 and physical latency.

Reviewer independently verifies all21 durable package manifest files, packaged/current binary
identity SHA256998020860c7cb6f027fa74462ed4cbd0afa02d52167ebe40fdc6111e36d21bd4 and valid
unchanged-product relocated complete recall evidence. Final case/theme/document recheck follows.
Engineering remains incomplete for repeated-render acceptance; owner acceptance pending.

Final reviewer recheck: all eight cases complete/exit0 at48kHz/256/JACK; three actual GUI
checks and distinct two-instance handles verified. Seven durable WAV hashes match. Final1440
A-light/A-dark controls are distinct/readable; final1280 remains valid. P2 dispositions
accepted; no new substantive finding. Draft submission supported, engineering incomplete.
