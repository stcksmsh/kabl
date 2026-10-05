# D08 current handover

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

## Historical record before this strict repair


## R6 investigation return — 2026-10-05

No repair was shipped. R6 remains failed; engineering acceptance remains incomplete. [RT-OWNERSHIP](RT-OWNERSHIP.md) documents broader framework ownership/handoff changes needed and two concrete alternatives. Production remains133fdb89ac56a396db0c7c961c74067782b34efb, same17cfcc408aacf341971ed5dfc35ddd6356445d16273c9c7610b16cef7c4770ee binary. Fresh independent reviewer `/root/d08_r6_review` confirms this disposition. Final checks and fresh-process renders/recall are indexed in evidence/r6-audit/index.json. Use sibling d08-r6-owner-test for updated records; original bundle/archive retained. PR#11 stays draft. No merge/D09/owner acceptance. This return does not establish that repair is impossible; it establishes why a mutex-only patch cannot meet the requirement.

Source and evidence prepared; engineering submission remains draft with strict no-lock acceptance gap and owner checks pending. Source133fdb89ac56a396db0c7c961c74067782b34efb on codex/d08-host-production, isolated /home/stcksmsh/Programming/Github/kabl/.worktrees/d08. Basec6bb8ccdab50dfe91a2b1f002d8c2766351697fc. Draft PR#11: https://github.com/stcksmsh/kabl/pull/11. No merge/D09 or other-chat messages. Read REPORT/REVIEW/README/design/CHECKLIST and evidence/provenance.json, plus evidence/submission.json for PR metadata and owner bundle/submission.json for exact final pushed head.

All covered reviewer correctness findings fixed and source rechecked. Final workspace610passed/0failed/22ignored; plugin23/0; strictClippy/release pass; validator35success/9skips. Actual final native record/replay, editor-closed virtual CC, Host transport, complete source-free recall, four fitting theme/size views and eight final timing cases are indexed. Full final renders38/39/41/42 match under policy, warm-ups37/40 retained. Ordinary renders still differ.

Owner bundle: /home/stcksmsh/.codex/visualizations/2026/10/04/01a10707-3ccc-73d2-b3af-ee17bddde50b/d08-owner-test. Run ./launch.sh; full raw evidence in sibling d08-final-evidence, original checkpoint retained. Root checkout/user files/.ai/other worktrees untouched. No task-owned host/display remains after runner cleanup.

Remaining acceptance: inherited nice-plug process/start/reset parking_lot::Mutex;4/4 launch support only; bounded64frame timing; dense two-editor Dummy-backend errors; physical controller/listening/latency/feel and explicit Kosta production-beta disposition. Do not claim engineering complete, merged or owner accepted. Subsequent repair requires Kosta's scope; do not start D09.
