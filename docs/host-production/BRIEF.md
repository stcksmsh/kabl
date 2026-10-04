# D08 — Host production

Authorized by Kosta's pasted D08 request, 2026-10-04. Outcome: a REAPER arrangement with stable automation, synchronized sequences and reproducible completed renders.

Starting SHA: c6bb8ccdab50dfe91a2b1f002d8c2766351697fc, freshly fetched origin/master; D07 PR #10 merge is this exact ancestor. D07 submitted 41e023a950458270e9da86c94c84f536328e29a7; tested/reviewed product ff11434c6502d2cce5a3894544bec10f5ceea6d6. Dedicated branch codex/d08-host-production, isolated .worktrees/d08. Root master c41420c and existing D06/D07 worktrees are preserved. No competing D08 agent/chat found. No .ai/state.json exists; .ai remains untouched.

Environment observed: ThinkBook16, x86_64 Linux 7.0.0-34-generic, Rust/Cargo 1.93.0, REAPER /usr/local/bin/reaper, Xvfb available, DISPLAY=:1. Audio configuration, GUI access and devices require fresh verification; past D07 measurements are provenance, not D08 passes.

Read: supplied AGENTS instructions; product-research/AGENT-WORKFLOW; HANDOFF/STATUS current D07 closeout; PRODUCT-PLAN D08; reaper-instrument README/design/REPORT/REVIEW latest closeout; decisions latest D06/D07; runtime-controls and midi-timing contracts; production clap/engine/editor paths. D07 actual repeated renders failed; preserve evidence. Owner reviews remain pending despite merges.

## Scope and invariants

- Sixteen stable host slot identities, persisted target identities, prioritize pins/macros. Never infer identity from layout/graph index. Deletion explicitly unassigns and does not reuse lanes. Preserve output_gain and version-1 recall.
- Host/Free modes; standalone remains free. Define play/stop/seek/loop/tempo/missing transport/time signature, sequencer phase, launches, MIDI ownership, tails and precision.
- Deterministic Host reset/seed policy with explicit-schedule tests and completed actual REAPER repeats. Never reset oscillators on every MIDI note or change Free sound.
- Version state; atomic malformed-state rejection; transient keys/pedals/commands/history are not sound state. Callback bounded, no allocation/free/locks; graph destruction deferred. Host owns devices/MIDI.
- Editor-independent CC/buttons; runtime knobs do not compile graphs. Host playback stays separate from patch undo; gestures notify host.

Internal increments (no approval gates): slot/state contracts; runtime automation/gestures; transport/reset policy; focused regression/RT tests; actual host piece/recall/renders/measurements/screenshots; portable owner bundle; independent reviewer, fixes and actual-head recheck; focused commits/push and D08 PR.

## Acceptance and delivery

Automated: stable mapping/reorder/deletion, legacy and malformed state, timeline/reset repeatability, callback bounds and undo/standalone regressions.
Actual app: automation record/replay and full restart recall, closed-editor controls, Host transport/tempo/loop/seek, repeated completed renders, played + sequenced multi-track piece, releases/tails/latency, relocated Linux bundle/profile, real 1440×900 and 1280×800 light/dark, representative light/dense one/two-instance cases with verified editor handles.
Owner: listening/controller/physical latency/feel and explicit production-beta disposition remain unchecked.
Deliver README/design/REPORT/REVIEW/CHECKLIST, reproduction scripts, raw measurements, curated UI/audio, portable project and durable bundle; update HANDOFF/STATUS/decisions. Report exact starting/submitted/tested/reviewed/packaged heads and acceptance dispositions.

Excludes D09/D10, broad polish, custom SDK/formats and synthesis additions. Stop after D08 submission; no merge, D09 or messages to other chats. Access/hardware limitations must be explicit, never simulated passes. Independent fresh reviewer is authorized at final product stage.
