# D08 submission report — 2026-10-04

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

Unchanged theme/fitting screenshots and silent walkthrough media are reused from product133fdb89ac56a396db0c7c961c74067782b34efb and d08-final-evidence, not claimed as new callback evidence. Prior warm-ups, ordinary-render failures and original timing JSON remain intact.

### Refreshed timing

| Case | Process p99 upper µs per active instance | Process max µs per active instance | Backend ERR max |
| --- | --- | --- | --- |
| light-1-closed | 170.0 | 442.52 | 0 |
| light-1-open | 190.0 | 412.9 | 0 |
| light-2-closed | 190.0, 180.0 | 394.19, 394.98 | 0 |
| light-2-open | 210.0, 210.0 | 1927.54, 428.83 | 0 |
| dense-1-closed | 1730.0 | 2027.54 | 0 |
| dense-1-open | 2030.0 | 2708.06 | 0 |
| dense-2-closed | 1780.0, 1760.0 | 2677.4, 2104.53 | 0 |
| dense-2-open | 2540.0, 2460.0 | 4969.91, 5491.27 | 2 |

Execution intervals come from an external proxy; production renders and owner binary are
unwrapped. P99 values are histogram upper bounds (10µs bins). Startup/teardown arrival
gaps, backend errors, physical xruns and latency are separate; raw summaries retain all.

## Historical record before this strict repair


## R6 repair investigation — 2026-10-05

The authorized investigation found that a mutex-only correction cannot satisfy the whole-callback requirement. No product change was shipped. R6 remains **failed**, and engineering remains **submitted as draft; acceptance incomplete**. This is an alternative/impact return under the repair brief's broader-redesign stop condition, not a completed repair or waiver. [RT-OWNERSHIP](RT-OWNERSHIP.md) records every plugin-object caller found, competing main-thread track notifications, the unconditional mutex-backed state-channel poll, three measured AtomicCell fallback locks, and retrying parameter/task queues with an unbudgeted drain. It proposes a pinned, ISC-licensed strict CLAP framework profile or a larger dedicated wrapper, including compatibility and maintenance costs.

Production source remains133fdb89ac56a396db0c7c961c74067782b34efb; rebuilt binary SHA256 remains17cfcc408aacf341971ed5dfc35ddd6356445d16273c9c7610b16cef7c4770ee. Workspace checks rerun610/0/22, strict plugin Clippy/release pass, validator35success/9skips. The compiled ownership probe exits2 as expected and explicitly reports failed acceptance, not a successful no-lock test. Additional fresh-process unwrapped production render/recall evidence is indexed in evidence/r6-audit/index.json. Baseline automation/CC/transport/screenshots/timings are reused unchanged with source/binary provenance; no callback behavior changed, so no new timing claim or new lifecycle repair regression is claimed. All prior warm-ups, failures and raw archives remain preserved.

Fresh independent reviewer `/root/d08_r6_review` independently confirms the broader callback-boundary scope and unresolved disposition; final committed artifact recheck is recorded in the new owner bundle's submission.json. Owner review remains pending; no merge or D09. The updated portable owner bundle is the sibling d08-r6-owner-test; old d08-owner-test remains preserved.

Outcome: production CLAP in a two-track REAPER arrangement with stable native automation, Host/Free synchronization and an explicit repeatable full-render policy.

Engineering: **submitted as draft; acceptance incomplete**. Owner review: **pending**. Not merged, no D09, no owner sound/controller/production-beta acceptance. The inherited framework callback mutex is a strict acceptance failure, not a passed allocation test.

## Heads and environment

- Starting/base: `c6bb8ccdab50dfe91a2b1f002d8c2766351697fc` (D07 PR#10 merge), refreshed origin/master unchanged before continuation.
- Tested/reviewed/packaged product: `133fdb89ac56a396db0c7c961c74067782b34efb`.
- Production plugin SHA256: `17cfcc408aacf341971ed5dfc35ddd6356445d16273c9c7610b16cef7c4770ee`.
- Draft PR: [#11](https://github.com/stcksmsh/kabl/pull/11). Source/evidence snapshot: `df4dbf315b50168c7d48393e1110531c2661d4cb`; following submission metadata commit changes no product.
- Submitted head/PR: PR metadata is recorded in evidence/submission.json; exact final submitted SHA is recorded in owner bundle/submission.json after the last metadata commit. GitHub PR head is authoritative; these commits change no product source.
- Branch: `codex/d08-host-production`, isolated `/home/stcksmsh/Programming/Github/kabl/.worktrees/d08` (physical path under `/secondary/Programming/Github/kabl`). Normal push, no history rewrite.
- ThinkBook16/Linux7.0.0-34-generic x86_64; Rust/Cargo1.93.0; REAPER7.75; Xvfb/metacity. Actual JACK48kHz/256frames, period5.333ms. PipeWire Dummy-Driver; scripted X11 and REAPER virtual MIDI. No physical controller or physical latency claim.

## Acceptance matrix

| Criterion | Disposition | Evidence |
|---|---|---|
|16 stable persisted lanes; pins/macros priority; gain identity | Pass source/native host | automation.rs, host_tests; saved complete arrangement and recall-submission-comparison.json |
| Layout reorder cannot retarget; deletion/Undo never silently reuse | Pass automated/source | stable_lanes_legacy_state_and_atomic_rejection; explicit retired state/UI |
| Record native rack gesture and replay | Pass actual REAPER | rack-record-submission-host.txt; replay-submission-host.txt; media/walkthrough.mp4 |
| Closed-editor automation/CC/buttons | Pass actual scripted host; physical owner pending | both floating + FX chain handles asserted closed; cc-submission-host.txt and cc-button-result.png; mapped Run stops after virtual CC25 |
| Save/full quit/reopen mappings + complete states | Pass actual source-free host | recall-submission-host.txt + recall-submission-comparison.json, both complete states equal |
| Host play/stop/seek/loop/tempo + Free | Pass actual host + explicit-schedule tests | transport-submission-host.txt/.csv; partition-independent host_automation test; missing-transport test |
| Time signatures/launch/MIDI/tails |4/4 initial bar policy; explicit other limits | design.md; production Clock/Timeline; MIDI cleanup tests; render musical metrics |
| Repeated completed offline renders | Pass only declared policy | render-submission-policy.json; final full38/39/41/42 decoded PCM identical; warm-ups37/40 retained |
| Ordinary repetitions | Still fail; no general repeat claim | render-first-comparison.json, render-start-policy-comparison.json + retained WAVs |
| Musical activity/releases/tails/latency | Render metrics + source/host latency; owner listening pending | full36s actual render; peak0.6105888;32–36s peak2.8581e-8;64frame reported plugin latency; physical unmeasured |
| Old D07/standalone/undo contracts | Pass automated; no Free sound change | v1 identity/migration, full workspace610/0/22; standalone ui Host flag defaultfalse |
| Atomic invalid state + transient cleanup | Pass automated/exported | malformed/fullqueue rejection; pending epoch retries; native modulation recall; source lifetime reset tests |
| No callback allocation/free; bounded work; deferred collection | Pass guarded covered paths | exported/explicit-schedule allocation guards, fixed rings/limits, basedrop collection |
| Whole callback no locks | **Fail inherited framework** | R6; Mutex + fallback SeqLocks + state-channel mutex + retrying/unbudgeted handoffs; RT-OWNERSHIP.md |
| Short played + sequenced multi-track piece | Pass actual scripted arrangement | owner projects/arrangement.rpp; media/arrangement audio; virtual input honestly labeled |
| Portable Linux profile/factory-unavailable recall | Pass actual relocated owner binary | packaged-submission-first/second.txt mapped path; recall equality; source-free bundle manifest |
|1440×900/1280×800 light/dark | Pass real refreshed captures | media/ui-*.png, verified floating handle; full1180×680 rack fits host chrome |
| Light/dense one/two-instance open/closed timing | Eight final cases measured; no hard RT guarantee | host-measurements.json + timing-summary.json, GUI handles3checks/case, raw backend logs |
| Physical controller/listening/feel/production beta | **Unverified owner** | CHECKLIST unchecked |

## Verification and reproduction

Final product commands/logs: `cargo test --workspace` →610passed/0failed/22ignored (`workspace-tests-submission.txt`); `cargo test -p kabl-clap --lib` →23/0 (`reviewer-backpressure-tests.txt`); strict `cargo clippy -p kabl-clap --all-targets -- -D warnings` and release build exit0. Official clap-validator0.4.1 rev152b9823 validates final unwrapped plugin →35success/9skips (`validator-submission.json`). Sandbox validator socket failure retained separately; escalated harness completed successfully. Hashes/commands/head recorded in submission-verification.json and provenance.json.

[README](README.md) contains exact reproduction interfaces. Runner calls Audio_Init and waits for actual host setup. Initial constant120/position0 findings were startup-driver failures; corrected raw telemetry includes actual120→150→140tempo, seek8, loop8..9, stop0 and delayed Free0. Proxy timing/transport diagnostics are not the shipped plugin.

## Repeat policy and failed evidence

In each fresh process initialize audio, defer1s, set I_PERFFLAGS=3 on both tracks, close device, then complete three36s48kHz stereo float32 renders. First complete render is retained warm-up; second/third are compared. Final processes produce37/38/39 and40/41/42. Four accepted completed PCM payloads have SHA256 `c535792b70919e23b5f8b1f25dd92333f397b6bcbb5e9c3ccb2e5bb1c7e2cb7b`;0different samples. Warm-up exclusion is policy, not silent success selection. Ordinary repetitions and earlier wrong-init policies remain preserved. No oscillator resets on MIDI note were added.

Final musical PCM equals earlier initialized render26 byte-for-byte; its section RMS/peak metrics are reused by hash identity in render-musical-metrics.json. Activity continues0–24s, release/effect tails24–32s, near silence32–36s. Full float32 WAV stays in owner bundle/raw archive; curated audio is labeled24-bit FLAC conversion for listening only.

## Final timing measurements

Each case plays12seconds plus complete startup/teardown capture at48kHz/256frames. Actual floating handles and hidden FX chains are checked at start/6s/end. Light = init-keyboard6modules/7cables; dense = sound-palette94modules/156cables, verified in both saved states. Active instances have over1000callbacks; startup-only instances remain in raw data. No builds/tests ran during these measurements.

| Case | Execution p99 upper (µs, per active instance) | Execution max (µs) | REAPER node ERR max |
|---|---|---|---|
| light-1-closed | 170.0 | 287.63 | 0 |
| light-1-open | 180.0 | 267.53 | 0 |
| light-2-closed | 160.0, 160.0 | 244.53, 227.95 | 0 |
| light-2-open | 190.0, 190.0 | 399.65, 281.88 | 0 |
| dense-1-closed | 1660.0 | 1790.89 | 0 |
| dense-1-open | 1880.0 | 2485.04 | 0 |
| dense-2-closed | 1660.0, 1650.0 | 1805.05, 1719.94 | 0 |
| dense-2-open | 2410.0, 2520.0 | 3946.16, 5257.88 | 2 |

The period is5333.333µs. The proxy times one production process call, not the whole host graph or editor thread. Arrival maxima include startup/teardown interruptions and are separately retained; they are not processing duration. PipeWire ERR belongs to observed REAPER JACK nodes on Dummy-Driver; it is not measured physical xruns or latency. Older62addcb timing and failed first-driver cases remain preserved with explicit provenance.

## Review, changes and decisions

Fresh reviewer `/root/d08_review` reviewed exact base→product, direct contracts/code + raw evidence, then rechecked each fix at actual product head. [REVIEW](REVIEW.md) lists queue crossing/backpressure, exact GUI values, flush ordering, duplicate ends, modulation recall and dispositions. All covered correctness findings resolved. Framework lock remains unresolved acceptance failure. Reviewer did not independently run host tests or approve sound/feel.

Main changes: clap automation/schedule/state/bridge/control/editor ownership; engine cached runtime overlays + Host clock across active/incoming/pending; Clock host phase support; UI Host/Synced controls and CC rearming. No new dependencies, formats, standalone device backend or D09 features. Decisions appended to docs/decisions.md.

## Artifacts and owner actions

Durable owner bundle and raw evidence roots are indexed in provenance.json. Owner launcher is bundle/launch.sh; repeat launcher bundle/render-policy.sh; CHECKLIST gives save-copy/full-restart, automation, deletion, real controller and physical latency checks. Static files have SHA256 manifests; profile/runs remain mutable during use.

Initial immutable d08-checkpoint retains original failures. Final durable archive preserves all42completed renders, warm-ups, raw timing/reset/transport/backend logs, failed driver traces and saved project states. Curated four UI views, native gesture screenshot, CC result, silent scripted walkthrough and listening audio are in media/ and copied to owner bundle. No edited screenshots or physical-input substitutes are labeled as owner evidence.

## Limits and follow-ups

Inherited nice-plug parking_lot::Mutex prevents strict whole-callback lock-free acceptance. A focused framework ownership change needs subsequent authorized repair; unsafe bypass or broad dependency fork was not added. Ordinary render configurations remain nondeterministic; initial bar launches support4/4 only; parameter/transport precision uses64frame grid; modulation is global rather than per-note; mapped patches conservatively remain active. Dense open-editor backend errors remain visible in timing results. Owner listening/controller/feel/physical latency remain pending.

Repo scope: D08 worktree only; root user files, prior D06/D07 worktrees and .ai preserved. No global install, user profile mutation, merge, D09 or messages to other chats.
