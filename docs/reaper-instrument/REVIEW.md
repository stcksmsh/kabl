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
