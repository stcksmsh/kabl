# D03-R2 + D04 independent review — launch prompt

You are a fresh independent reviewer for stcksmsh/kabl PR #6:
https://github.com/stcksmsh/kabl/pull/6

Review the actual combined D03-R2 + D04 implementation. Do not implement product fixes.
You replace the reviewer that was rate-limited before producing any findings; there is
no completed review to inherit. This is not a request to repeat the implementation.

## Establish the target

Fetch origin and PR #6. Verify its actual head and record full SHAs.
Expected base: 4f7e72950a93765b79d3f229df00ce11d50e7c51.
Expected submitted head: b2171180ae72f7d0e82cc172389ac5c191c464ad.
Reported tested product head: b2e9fd6; evidence build: e7dca9a.
Branch: claude/d02-musical-controls-knjmgu (environment-required name).

The supervisor verified that b217118 differs from b2e9fd6 only in docs/evidence, and the
Rust changes after e7dca9a only move a test and add a comment. Recheck if the head advanced.
Do not review origin/master as if it contains this PR. Preserve unrelated work; use an
isolated checkout if needed. No reset of someone else's workspace or force push.

The prompts were added to master after the PR base. Read them with git show origin/master
if they are absent on the PR branch; that alone does not require merging master.

## Read

1. Applicable repository instructions, docs/product-research/AGENT-WORKFLOW.md.
2. docs/product-research/briefs/D04.md in full: the acceptance contract for both parts.
3. PR-head docs/runtime-controls/{REPORT,REVIEW,design,classification,README}.md and CHECKLIST.md.
4. D03-R2 additions in docs/signal-inspection/REPORT.md and the relevant D03-R1
   stream-error ownership/review records.
5. Actual base-to-head diff, touched production paths, tests and cited evidence.
   Follow dependencies only where they matter to the issued contract.

## Review coverage

Review correctness, data/state preservation, real-time safety and whether evidence proves
the acceptance claims. Do not turn optional future work or stylistic preferences into
acceptance failures.

Explicitly cover:
- Runtime/structural classification, defaults/legacy aliases, module state and route semantics.
- Document/target identity, revisions, same-value edits, deletion/recreation, new documents,
  active/incoming/pending graphs, stale build rejection and last-value convergence.
- Saturation/coalescing, paused consumers, total ownership bounds, per-callback work,
  allocation AND deallocation, graph retirement and teardown.
- Production control thread/Core mutex, lock order, save/undo/redo, failed compilation,
  comparison/recipes, mapping changes, CC pickup, button edge/reconnect behavior and UI pause.
- Accurate requested/applied/failed state: consuming a message is not necessarily applying
  it to the graph that will become audible.
- Inspector freshness across runtime versus topology edits and failed compiles.
- D03-R2 missing-audio-input diagnosis without false faults for optional/defaulted inputs,
  plus the error-message age clarification.
- Fixed-setting compatibility and transition behavior; evidence provenance, test relevance,
  censored performance runs and package behavior.

Two supervisor concerns need targeted investigation; they are NOT reproduced failures:
1. design.md claims separate action and parameter queues need no relative ordering.
   Test counterexamples involving bank data/launch, clock changes/Restart, document
   replacement and queued actions. State exactly what ordering is or is not guaranteed.
2. MAX_RAMPS=32 makes additional simultaneous targets jump. Determine the reachable cases,
   audible/compatibility consequence and compliance with the brief. Do not silently
   approve a product tradeoff on Kosta's behalf.

Also scrutinize unsupported bounds: holding Core during layout, compile or file operations
may exceed "one layout pass" of CC delay. Bound claims with source and measurements, not
assumptions. Report only concrete implications for the authorized behavior.

The narrow backend-error overflow free/allocator-lock exception is already owner-accepted.
It does not exempt new normal control/render code. The older first-callback RT-priority
request remains a disclosed pre-D03 exception.

## Verification and output

Run targeted checks that resolve concrete risks; inspect existing logs and scripts.
Reported workspace result is 540 passed / 0 failed / 16 ignored and clean clippy at b2e9fd6.
Distinguish your executed results from those checked in the implementer's artifacts.
Do not claim a full callback allocation check from a helper-only test, or hardware evidence
from FIFO MIDI/Xvfb/null-sink runs. Do not invent a cause for stream stalls or old spikes.

Append your review to docs/runtime-controls/REVIEW.md, preserving the rate-limit history.
For every finding: stable ID, severity, exact file/function/location, failure scenario,
evidence or reproducer, impact, and requested correction. Separate confirmed findings,
source-grounded risks requiring reproduction, evidence gaps and nonblocking follow-ups.
No findings is valid only with explicit coverage and limits. Record full reviewed base/head,
commands/results and whether you recommend changes or engineering completion.

You may commit and push review-document changes only to the PR branch, when no implementing
agent is writing there. If access prevents this, return the full review for the implementing
agent to record; do not claim it was saved. Do not send PR comments or submit a GitHub approval.
Do not modify product code, merge the PR, start D05 or mark owner acceptance.

Return the review to Kosta/the supervisor, then stop. Stay available for the final recheck
after the implementing agent fixes findings. Do not wait indefinitely for other agents.
