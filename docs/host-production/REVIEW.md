# D08 independent review

## D08 strict CLAP repair — 2026-10-05

Engineering complete; strict-profile repair submitted for owner review. Owner review pending. PR #11 remains draft and unmerged. No D09/D10.

R6 is repaired for the declared Kabl strict CLAP profile. Checked exclusive plugin
ownership, immutable activation snapshots, scalar tail publication and bounded SPSC
handoffs replace the reachable callback locks, retrying queues and generic state
rendezvous. State load publishes complete recalled controls without writing native
caches; serialized callbacks own those writes. Pending getters/editor/state observations
retain canonical recall until cache synchronization. Native IDs, v1/v2 state, graph
retirement, automation/flush ordering and Host/Free behavior are preserved.

Starting submitted investigation: `790f0c40a479bdaa55753bc8d63b7dec3729784b`.
Tested/reviewed/packaged product: `f7157619836075d0d7704e59be5d66e6224890b1`. Binary SHA256: `3baa8ee3cc0f2f99e1823e0a741d7301713098f99e92dc84f7556b8d0483aca4`.
Final submitted documentation head is recorded in the bundle's submission.json after
normal push; it changes no product source. Prior product `133fdb89ac56a396db0c7c961c74067782b34efb`
and its original evidence/bundles remain retained with that provenance.

Final checks: workspace612 passed/0 failed/22 ignored; plugin25; framework primitives4;
strict workspace/all-target Clippy, release and validator35 success/9 skips pass.
Actual REAPER Write/replay, closed-editor virtual CC/buttons, full fresh-process recall,
transport and GUI/plugin destruction/recreation are refreshed. All18 native values and
both complete states survive recreation. Two fresh unwrapped production processes each
retain three full36-second renders: first is warm-up, second/third match across processes
with PCM SHA256 `c535792b70919e23b5f8b1f25dd92333f397b6bcbb5e9c3ccb2e5bb1c7e2cb7b`.
Three ordinary renders still differ and remain retained. Additional source-free packaged
fresh recall and policy renders pass, with verified bundle mapping/cwd and unavailable
factory path; packaged warm-up is also retained.

Evidence: [strict-profile index](evidence/strict-profile/index.json),
[callback audit](evidence/strict-profile/callback-audit.md),
[timing summary](evidence/strict-profile/timing-summary.json).
Owner bundle: `/home/stcksmsh/.codex/visualizations/2026/10/04/01a10707-3ccc-73d2-b3af-ee17bddde50b/d08-strict-owner-test`; run `./launch.sh`.
Full raw archive: sibling `d08-strict-evidence`. Previous archives and failures remain intact.

Limits: valid CLAP host lifecycle contracts; off-audio producer backlog can grow when a
host permanently rejects delivery; no exhaustive DSP/host/machine-code proof. Dense two
open editors still show backend ERR=2; worst measured individual process5491.27µs exceeds
the5333.33µs JACK period. Physical xruns and round-trip latency are unmeasured. Reported
plugin latency remains64frames (1.333ms at48kHz). Physical controller, listening, feel,
latency and production-beta approval remain Kosta's checks. Ordinary-render nondeterminism
and dense-editor errors are not resolved by this repair.

Maintenance: retained ISC snapshot at upstream263b16877d0b0ad30921868338a6df46eadc9a46,
original/final source manifests, reproducible patch and upgrade audit checklist in
[vendor/nice-plug/STRICT-PROFILE.md](../../vendor/nice-plug/STRICT-PROFILE.md).
Repeat ownership/ring/state/editor audit and full regression/host evidence on upstream
changes. No Cargo cache mutation, merge, global plugin install or other-chat messages.

### Fresh independent reviewer

Reviewer `/root/d08_strict_review` is read-only and fresh for this repair. It reviewed
actual committed product f7157619836075d0d7704e59be5d66e6224890b1, competing callers,
unsafe Owner<T: Send> Sync boundary, AtomicRefCell/rtrb implementation, state/cache epochs,
CC payloads, notification/latency semantics and snapshot reconstruction. No remaining
actionable source finding in those paths. Final independent artifact recheck: PASS at actual product f7157619836075d0d7704e59be5d66e6224890b1. Reviewer independently verified the portable bundle, raw archive, packaged recall/render proof and current documentation; final submitted documentation-head recheck is recorded in submission.json.

| Finding | Correction and regression |
| --- | --- |
| P1 native output epoch check/write races recall | Main publishes separate18-control recall; callback alone writes native caches; canonical pending overlay + DSP cache-epoch guards; deterministic barrier old-write/recall test |
| P2 coalesced modulation hook lost | Separate value/mod dirty servicing + exact per-parameter offset atomics |
| CC stale cache/payload isolation | Dedicated CC values + state epochs; stale Value retires, accepted Begin retains End; real recall with old pending mask regression |
| Deferred activation latency outside activate (validator) | Activation context directly notifies within activate; inactive deferred task waits for activation; final validator passes |

Reviewer verified97 final source hashes,93 upstream hashes and exact patch reconstruction.
Review does not approve owner sound/controller/latency or prove arbitrary host/OS timing.
Historical R1–R5 resolutions remain valid and their regressions pass. R6's former
unresolved disposition below describes the prior product only and is superseded here.

## Historical record before this strict repair


## R6 fresh independent investigation review — 2026-10-05

Reviewer `/root/d08_r6_review` is fresh for this investigation and read-only. Product remains133fdb89ac56a396db0c7c961c74067782b34efb; investigation begins at submitteda77804dd1c73f2fcb8e291fb9ebce5553bbeb988. Direct pinned framework/Crossbeam source review confirms that mutex-only repair cannot meet acceptance: competing main-thread track-info callbacks, unconditional zero-capacity try_recv mutex, large AtomicCell fallback SeqLocks, native task/output queue retries and unbudgeted output drain also require correction. params.flush reaches configuration/output/notification paths and belongs in the redesign scope. Safe full correction is possible in principle but remains unimplemented. Preferred strict CLAP framework profile and larger dedicated-wrapper alternatives are recorded in [RT-OWNERSHIP](RT-OWNERSHIP.md).

No product code or dependency changed; no new ownership/lifecycle implementation was available to review or test. R6 remains P1 **unresolved acceptance failure**. The reviewer did not independently rerun builds/host drivers, perform an exhaustive DSP audit, prove proposed replacement safety or approve owner checks. Final committed documentation/probe/raw-artifact recheck at the actual submission head is recorded in the new owner bundle's submission.json; this does not change the reviewed product head or waive R6. Original review below remains historical product evidence.

Reviewer: fresh read-only subagent `/root/d08_review`, explicitly required by Kosta's authorization. Base `c6bb8ccdab50dfe91a2b1f002d8c2766351697fc`. Initial reviewed product `333156d3daa5f78751efa7f466ea3e708d587b99`; rechecks `a64a2e243a57c55c84f7ad8f76c2e06630302314`, `c199347add575ffd975e815b959195d0572721a0`, final source `133fdb89ac56a396db0c7c961c74067782b34efb`.

## Findings and dispositions

| Finding | Failure scenario and requested correction | Disposition / verification |
|---|---|---|
| R1 P1, lib.rs Control.pump/render + automation.replace_bank | Separate graph/bank queues allow an old unassignment to restore an old base into a fresh browser document reusing IDs. Preserve document/session ownership; test actual queued crossing. Follow-up P2: later same-document edits and saturated bank retry also restore stale bases. | Fixed: browser publishes complete new Session/queues; restores drain before normal document FIFO; release payload resolves latest accepted bases against last successfully published bank. Regression covers fresh document, paused unassignment→later edit and full4-entry bank retry. a64a2e2/c199347/133fdb8. |
| R2 P2, RackApp/live_params | GUI setter queues SetValue; exact atomic changes after process. Gesture mask consumed with old native value; host echo cannot be required. Follow-up P1: old pending retry mask after state load can resurrect old exact values. | Fixed: exact requested value arrays + publication mask independent of delayed native atomics; load initializes pending arrays to saved bases under epoch publication. Requested-value and forced load/retry tests pass. a64a2e2. |
| R3 P2, get_extension_bridge/params.flush | Native flush updates atomics while Schedule retains old base/modulation; older flush can then override newer process sample-zero events. | Fixed: bounded flush bridge, separate pending base/modulation arrays, newer process sample-zero identity/type takes precedence. Exported active flush→process test verifies audible value/modulation without echo/reactivation; precedence regression passes. a64a2e2. |
| R4 P2, RackApp slot slider release | Explicit END followed by common pointer-up END causes unmatched duplicate gesture end. | Fixed: explicit END clears active flag; editor-close lifecycle retains one END for a begun GUI gesture. Direct source recheck; actual final Write/replay evidence added. a64a2e2. |
| R5 P2, Shared.load native parameters | Normalized setter retains native modulation; clamped unchanged output can also retain wrong native base. Recalled gain/slot differs from serialized state. | Fixed: clear native transient modulation before saved base setters; native base/modulated readback and pending scene tests pass. a64a2e2. |
| R6 P1 acceptance, pinned nice-plug wrapper process/start/reset | wrapper.rs2430/2115/2131 takes parking_lot::Mutex; delegated callback is not lock-free. Allocation guards cannot establish no-lock contract. | **Unresolved acceptance failure.** No new D08 locks, but whole-plugin no-lock requirement remains unmet. README/design/REPORT/CHECKLIST disclose it; no engineering-complete or production-beta acceptance claim. Unsafe bypass and unrelated broad framework rewrite rejected. |

Final source reviewer disposition at133fdb8: all reported correctness findings resolved within covered paths; no unresolved correctness findings in reviewed scope.23-pass plugin log, strict Clippy and release inspected. Binary SHA256 `17cfcc408aacf341971ed5dfc35ddd6356445d16273c9c7610b16cef7c4770ee` independently checked. The inherited mutex still blocks engineering-complete status.

## Coverage and limits

Direct code/diff plus D04/D06/D07 contracts: stable target identity, overlays/ramping, complete state migration/publication, browser versus host recall, queue ordering/backpressure, exact GUI handoff, native CC retries, flush precedence, Host/Free/reset paths, MIDI ownership and framework delegation. Initial reviewer independently decoded full36-second PCM renders32/33/35/36 and verified equality; final raw refresh is indexed in REPORT.

Reviewer did not edit source, run host/performance drivers, independently rerun tests, stress arbitrary concurrent host calls, listen, measure physical hardware or approve owner acceptance. Source review is not owner approval. Final artifact/provenance recheck will be recorded below after the refreshed evidence package is available.

Final artifact recheck completed at product133fdb89ac56a396db0c7c961c74067782b34efb. No new correctness or artifact findings within covered scope. Reviewer independently verified2915 durable manifest files,42completed renders, final38/39/41/42 + relocated2/3 + metric source26 identical full PCM; complete state recall equality; genuine9point native Write/replay; virtual CC/stopped Free screenshot; actual120/140/150transport; eight final timing cases/GUI records;610/0/22workspace +35success/9skipvalidator; requested screenshot dimensions and representative fitting views. Final Git submission/PR metadata is the only subsequent work; it changes no product source. Inherited mutex still prevents engineering-complete acceptance.
