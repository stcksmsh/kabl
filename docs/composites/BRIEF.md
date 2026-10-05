# D09 authorized implementation brief

Starting SHA: f870b3c4316b9ea819d35344de09f6b7d24c59ca. Branch: codex/d09-composites. Checkout: /secondary/Programming/Github/kabl/.worktrees/d09. Kosta authorized implementation on 5 October 2026. Live PR #11 is merged at this SHA; fetched origin/master equals this SHA and contains the merge. No intervening master changes exist at launch.

D08 submitted eb22ce7db9be63831f54e97d7308dbff42b3238f; tested/reviewed/packaged f7157619836075d0d7704e59be5d66e6224890b1. The updated independent REVIEW and callback audit close R6 within vendor/nice-plug/STRICT-PROFILE.md. Historical pre-repair references are not current safety evidence. The repair changes callback ownership, native cache/recall epochs and bounded output; D09 leaves those paths unchanged. Preserve the profile's valid-host assumptions and off-audio producer-backlog limitation.

## Outcome and boundaries

Encapsulate an existing voice/effect, expose stable controls/ports, duplicate independently, inspect/edit either instance, save/reload and publish immutable reusable versions. Preserve leaf/cable IDs, routes, cues, pins, native lanes, runtime smoothing, voice semantics, feedback schedule and zero added grouping latency. Embed exact graph/interface data; never load a library copy during project recall. Nesting is bounded; definition recursion and malformed input fail before mutation.

The resumed planning design and acceptance matrix accompany this brief. Kosta adopted the proposed defaults: new identities for duplicates, unpatched external boundary cables, new pin positions, no copied CC/buttons/native lane assignment, immutable versions, explicit unresolved dependency handling and existing library mechanisms. No D08 repair, D10 panel/visual work, custom runtime, marketplace or other-chat messages. No merge.

## Explicit history decision

Kosta selected chronological project Undo on 5 October 2026: “Independent instance state is required; separate per-instance undo histories are not required.” D09 uses the existing chronological project log. Undo reverses the latest project transaction regardless of which instance is open; Redo reapplies it. Interleaved edits to A and B remain isolated in state, while their actions share one chronological history. Standalone save/reload preserves the applied project log. Host recall preserves complete sound state, not a persistent Undo stack. This choice finalizes the history scope; no selective per-instance Undo or shared-route conflict policy is required.

## Reading and reconciliation

Reuse task-owned d09-planning/{CHECKPOINT,CHOICES,design,ACCEPTANCE,BRIEF.provisional}.md. Read updated D08 REPORT/REVIEW, strict-profile callback audit and vendor profile; core/state/log/op/format, editor/compare/runtime delivery, automation/state publication, rack/browser and relevant built-in metadata. No stale root HANDOFF/STATUS or broad historical reading. No applicable tracked/ancestor AGENTS.md was found; supplied user instructions govern. Leave .ai untouched; AIW/Recall are not in use.

## Increments and checks

1. Typed metadata/invertible ops, identity reservation, validation and flat projection. 2. Existing UI selection/encapsulate/open/expose/independent edits. 3. Duplicate/import remapping, immutable library versions and complete persistence. 4. Focused regressions, real-app voice/stereo effect, REAPER recall/automation/render, portable artifacts. 5. Fresh independent reviewer; fix/recheck actual product head, finalize docs/shared records and draft PR. These are internal increments, not approval gates.

Acceptance: encapsulate/open/duplicate/edit/agreed undo/save/reload; exact flat/grouped sound; independent state and agreed history; unchanged external identities and latency; bounded nesting/recursion; exact embedded recall without library; atomic useful failures; strict D08 contracts. Matrix records automated/source, real-app and owner evidence separately. Source-only tests never substitute for audible/real-app evidence.

Environment at launch: ThinkBook16 Linux 7.0.0-34-generic x86_64; Rust/Cargo 1.93.0; installed REAPER, Xvfb, xdotool and ffmpeg. Record actual backend/settings per run. Existing D08 64-frame latency, ordinary-render nondeterminism, dense editor backend errors and hardware/listening/feel/latency/production-beta owner gaps remain explicit. Do not erase or relabel earlier measurements.

Deliver README/design/REPORT/REVIEW/CHECKLIST, examples, exact commands/heads and portable owner artifacts. Reviewer reads direct diff/contracts/raw evidence and rechecks final product head. Commit focused changes, push normally, create/attach D09 draft PR. Report starting/submitted/tested/reviewed/packaged heads; engineering-complete, merged and owner-approved remain separate. Stop after submission.
