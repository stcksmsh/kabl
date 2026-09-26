# D04 review and closeout coordinator — clean-context launch prompt

You are the new coordinating and fixing agent for stcksmsh/kabl PR #6:
https://github.com/stcksmsh/kabl/pull/6

Your complete assignment is to finish independent review, fix in-scope findings, obtain an
independent final recheck and return the D03-R2 + D04 handoff. Work autonomously through
that sequence. You are not restarting implementation or taking over the product roadmap.
Kosta merges PRs and supplies hands-on acceptance. Do not merge or start D05.

## Roles and authority

You coordinate the work and make product fixes. Start a fresh reviewer subagent that reads
the actual code independently. The reviewer does not implement fixes. Return the corrected
code to that reviewer for a final recheck; if its context is lost, start another fresh
reviewer with the complete review record.

There is one product writer: you. Use sequential handoffs, not simultaneous implementations.
Do not send the review, fix and recheck prompts to yourself as if that were independent
review. The previously prepared role prompts are resources for this assignment:

- docs/product-research/briefs/D04-REVIEW-LAUNCH.md — for the reviewer subagent.
- docs/product-research/briefs/D04-FIX-LAUNCH.md — for your fixing phase.
- docs/product-research/briefs/D04-RECHECK-LAUNCH.md — for the reviewer after fixes.

For this coordinated run, override those prompts' optional reviewer push permission:
the reviewer returns its report; you record it faithfully and make all commits/pushes.
The reviewer may run checks in an isolated checkout but must not alter your working tree.
Preserve its conclusions separately from your responses.

If reviewer capacity is unavailable, record the concrete limitation, keep the PR draft,
and return a resumable handoff. Never substitute self-review or wait indefinitely.

## State at issue

- PR #6 is draft, open and unmerged.
- Branch: claude/d02-musical-controls-knjmgu. The name was environment-required and
  does not describe the current batch.
- Assignment base: 4f7e72950a93765b79d3f229df00ce11d50e7c51.
- Submitted head: b2171180ae72f7d0e82cc172389ac5c191c464ad.
- Tested product head: b2e9fd6.
- Evidence build: e7dca9a.
- The supervisor verified: after e7dca9a, the only Rust changes moved one test to its
  own file and added a comment; after b2e9fd6, changes are docs/evidence only.
- Checked logs: 540 passed, 0 failed, 16 ignored; clippy clean at b2e9fd6.
  These are existing evidence, not tests you or your reviewer have executed.
- No reviewed head exists. The attempted reviewer was rate-limited before producing
  any finding. REVIEW.md explicitly records that gap.
- Master at issue: e89b1979d0b57df7584b683734ca7eae2a6fdc4d. It contains the three role
  prompts; the commit containing this coordinator prompt follows it.
  Master does not contain the D04 implementation.

Fetch current origin/master and the actual PR branch before editing. Record full SHAs,
inspect intervening changes and preserve unrelated work. Use the PR branch as the product
starting point, not master. Do not reset it to master or overwrite the existing work.
Read prompt files from origin/master with git show when absent on the PR branch; no merge
is needed solely to read them. If master has advanced with product code, integrate without
dropping either side and have the reviewer inspect the combined result.

## Read first

1. Applicable repository instructions.
2. Current PR-head HANDOFF and STATUS top section, noting the missing independent review.
3. docs/product-research/AGENT-WORKFLOW.md in full.
4. docs/product-research/briefs/D04.md in full, then the three role prompts from master.
5. PR-head docs/runtime-controls/{REPORT,REVIEW,design,classification,README,CHECKLIST}.md.
6. D03-R2 additions in docs/signal-inspection/REPORT.md, relevant D03-R1 review and
   design.md "Stream-error ownership", and the latest decisions.md entries.
7. Actual base-to-head diff, relevant production dependencies, tests and evidence.

The D04 brief is the acceptance contract. Do not restart competitor research or read the
whole repository history.

## Product and acceptance constraints

Kabl supports standalone evolving/interlocking music and eventual conventional MIDI
instrument production in REAPER. Linux first. This assignment finishes:
- D03-R2: identify the disconnected intermediate audio input accurately and clarify
  the age of the last delivered stream-error message.
- D04: routine controls without recompilation, bounded real-time-safe delivery, stable
  target/generation ordering, editor-independent mapped MIDI control, coherent document,
  undo/save state, and preserved explanations/inspection.

Kosta accepted only the narrow D03-R1 exception: a full 16-slot stream-error queue may
drop a backend-owned error message in its callback, potentially taking the allocator's
internal lock during free. On the pinned cpal 0.18.2 ALSA build this concerns BackendError.
Normal control/render code gets no exemption for allocation, freeing, logging or locks.
The first-callback RT-priority request is an older documented exception.

D03, D02, D01/D01-R1, Composition + Motion and Sound Palette hands-on reviews remain
pending. D00 is incomplete. Cloud tests, FIFO MIDI, Xvfb and null-sink audio are not laptop,
controller-feel or beginner evidence. No cause is established for the cloud stream stalls
or Composition callback spike. Recovery remains D05.

## Review and fix sequence

1. Freeze the actual initial review head. Give the fresh reviewer the independent-review
   prompt, full base/head SHAs, actual code/diff and evidence. Let it inspect independently.
2. Record REVIEW.md coverage and findings with stable IDs, severity, exact failure scenario,
   code locations, supporting evidence and requested corrections. Distinguish observed
   failures, risks requiring reproduction, evidence gaps and optional follow-ups.
3. Reproduce and fix in-scope findings in small commits. Record dispositions; disagreements
   require evidence, not deletion of the review. Ask Kosta only when a consequential
   unapproved product tradeoff prevents meeting the brief.
4. Verify fixes and refresh affected evidence. Run required final workspace tests and
   clippy, record exact commands/results and artifact/build SHAs. Do not repeat unaffected
   media merely to make everything newer.
5. Have the independent reviewer recheck the final combined product head and every finding.
   Repeat fixes/recheck if needed. A new product change after recheck requires a relevant
   additional recheck; proven docs-only changes do not.
6. Update REPORT, REVIEW, HANDOFF, STATUS, CHECKLIST and decisions accurately; push the
   PR branch and return the final handoff. Keep the draft while required review/fixes
   remain incomplete. Do not merge.

The review prompt contains two particular concerns, not confirmed defects:
- Separate action/value queues may reorder meaningful combinations such as bank edits
  and launches, Restart and parameter edits, or replacement and queued actions.
- Beyond 32 simultaneous ramps, further targets jump rather than ramp; establish the
  reachable compatibility consequence instead of silently accepting a tradeoff.

Also require scrutiny of Core lock lifetime/latency, queue saturation and final-value
convergence, stale graph/target handling, callback ownership/destruction, CC pickup/buttons,
save/undo/redo, failed compiles, probe freshness and diagnosis defaults.

## Working and reporting rules

- Leave .ai/ untouched. AIW/Recall are not in use.
- Format changed Rust files only; no whole-workspace formatting.
- Small commits, no force push or history rewriting. Preserve concurrent planning updates.
- No new synthesis, plugin work, stream recovery or D05 expansion.
- No GitHub approval on Kosta's behalf and no automatic merge.
- Keep prose concise; distinguish observed, reported, inferred and proposed results.

Return:
- Exact starting, tested, reviewed and submitted commits; branch and PR.
- Separate D03-R2/D04 engineering dispositions and owner-review pending status.
- Reviewer coverage, each finding/disposition, final recheck and remaining gaps.
- Verification/evidence paths and any build mismatch with its actual implications.
- Concise owner checklist links and unresolved decisions.
- Clean/remaining working-tree state and confirmation that unrelated files were preserved.

Stop after the handoff. The overseer checks evidence, Kosta decides on merge, and D05 is
scoped against the accepted engineering result.
