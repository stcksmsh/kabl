# D08 submission report — 2026-10-04

Outcome: production CLAP in a two-track REAPER arrangement with stable native automation, Host/Free synchronization and an explicit repeatable full-render policy.

Engineering: **submitted as draft; acceptance incomplete**. Owner review: **pending**. Not merged, no D09, no owner sound/controller/production-beta acceptance. The inherited framework callback mutex is a strict acceptance failure, not a passed allocation test.

## Heads and environment

- Starting/base: `c6bb8ccdab50dfe91a2b1f002d8c2766351697fc` (D07 PR#10 merge), refreshed origin/master unchanged before continuation.
- Tested/reviewed/packaged product: `133fdb89ac56a396db0c7c961c74067782b34efb`.
- Production plugin SHA256: `17cfcc408aacf341971ed5dfc35ddd6356445d16273c9c7610b16cef7c4770ee`.
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
| Whole callback no locks | **Fail inherited framework** | independent R6; nice-plug process/start/reset Mutex |
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
