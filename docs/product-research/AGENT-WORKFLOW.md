# Sequential blank-context agent workflow

Date: 2026-09-25. Companion to [PRODUCT-PLAN.md](../PRODUCT-PLAN.md). Proposed workflow implementing Kosta's stated supervision rules; it does not authorize future batches by itself.

## Ownership and state

Kosta chooses/authorizes a whole batch and starts one fresh implementing agent. The implementer works through internal steps, commits and pushes, then reports. The supervisor checks the actual diff, evidence and acceptance criteria. Kosta supplies hands-on acceptance. The next fresh agent begins only after the next scope is explicitly authorized. No simultaneous writers are assumed.

Maintain two separate status fields per batch:

- Engineering: not started / implementing / submitted / changes requested / evidence checked.
- Owner review: pending / changes requested / accepted, with date and link or quoted instruction.

“Merged,” “tests pass” and “supervisor evidence checked” never mean owner accepted. A cloud environment may finish engineering evidence while hardware checks remain pending. Do not force fake completion.

## What each brief must contain

1. One musical/user outcome and its exact batch identifier.
2. Current starting commit, prerequisite review dispositions and known open defects.
3. Bounded ordered reading list: current HANDOFF, STATUS top, relevant decisions, prior batch report, task-specific code. Do not tell a blank agent to read the whole history.
4. Scope, exclusions and compatibility requirements.
5. Suggested internal working increments, explicitly not approval gates.
6. Acceptance table separating automated/source, real-app, owner and optional external-user checks.
7. Required artifacts and exact reporting format below.
8. Stop conditions: completing this batch, a product tradeoff outside authorization, or an access/hardware limitation that cannot be resolved honestly.

The implementer chooses routine engineering details and records the reason and rejected alternative. It asks Kosta about product changes, not ordinary layout coordinates, helper functions or library plumbing already covered by the brief. An unavailable device is an evidence limitation, not permission to substitute a simulation and call it hardware proof.

## Start-of-batch procedure

- Check `git status` and remotes; preserve unrelated work. Fetch origin and establish the actual current `master` before editing.
- Confirm HEAD contains the expected starting commit. If master advanced, read the intervening diff and reconcile the brief; do not reset or overwrite it.
- Read applicable repository instructions, then the brief's ordered sources. Leave `.ai/` untouched; AIW/Recall are not in use.
- Return a short understanding of outcome, invariants and initial verification. Proceed without an internal approval pause when Kosta has authorized the batch.
- Record the environment: compiler/toolchain, OS, GUI/audio backend, available MIDI/audio hardware and any stand-ins. Never inherit a previous session's measured results as current results.

## During implementation

- Small working commits directly to master and push. No branches/PRs unless Kosta asks or the cloud environment requires its own branch; then report it and provide the PR for Kosta to merge.
- Never force-push, rewrite unrelated history or reformat the workspace. Run rustfmt only on changed files; the rev2 examples must not receive unrelated formatting changes.
- Keep existing sound/patch/undo/RT contracts intact. Test changes where those contracts are at risk, not merely to increase test counts.
- Record nontrivial decisions in `docs/decisions.md` as implementation rationale. Do not write an owner approval that did not occur.
- Keep large generated media curated: one useful walkthrough and necessary screenshots/clips. Logs need artifact paths and exact commands. Do not commit endless intermediate takes or incidental caches.
- Changes discovered outside scope go to a clearly marked follow-up list with impact; only fix immediately if necessary to deliver the authorized outcome.

## Batch evidence package

Use `docs/<batch-slug>/` with:

- `README.md`: behaviour, decisions, sources/licenses, known limits, tested commit and environment, reproduction commands and evidence links.
- `CHECKLIST.md`: short actions Kosta performs, expected results and space for pass/fail notes. Keep owner checks unchecked until he reports them.
- `REPORT.md`: structured agent return below.
- A walkthrough video and selected screenshots at 1440×900/1280×800, A-light/A-dark. Label scripts/simulated input explicitly. Audio-affecting changes need audible evidence; UI views must be real application screenshots.
- Focused logs/results where necessary, with raw execution/arrival/xrun telemetry separately labelled. Scope cloud versus laptop evidence precisely.

Update HANDOFF and STATUS accurately at the end. Link the batch record from HANDOFF, including built/pending-review status. Preserve older measurements with their historical context rather than silently replacing them.

## Required report back to the supervisor

```text
Batch ID / outcome:
Starting commit:
Submitted head commit:
Branch / pushed remote / PR if environment-required:
Engineering status:
Owner-review status: pending unless Kosta explicitly said otherwise

What now works (user-facing):
Acceptance matrix: criterion -> pass/fail/unverified -> evidence path
Changes: relevant files and purpose
Decisions: decision, why, rejected alternative, decisions.md location
Verification: exact command, commit, environment, exit result, totals
Real-app evidence: video/screenshots/audio and reproduction commands
Measurements: settings, run duration/count, execution/arrival/xruns separately
Compatibility: old patches, undo, state, sound, plugin automation where relevant
Limits/deviations: what is incomplete, unsupported or only simulated
Owner checklist: exact launch/install instructions and actions
Follow-ups: observed problem, impact, suggested scope (not implemented)
Repo status: clean/remaining changes; confirm .ai and unrelated files untouched
```

The report is an index into evidence, not the evidence itself. “Works,” “all tests pass” or an edited screenshot alone is insufficient.

## Supervisor review procedure

1. Resolve submitted head on GitHub; confirm the changes actually landed on the claimed branch and compare against the stated base.
2. Inspect code/diff for the important acceptance contracts, using targeted tests where tools/environment permit.
3. Check commands and artifacts correspond to that commit. Separate failure, unsupported environment and unverified claims.
4. Inspect representative real-app evidence and report concrete gaps against the brief, rather than introducing a new batch of preferences during review.
5. Return an engineering assessment plus the short owner checklist. Do not approve sound, feel or beginner ease on Kosta's behalf.
6. If changes are needed, issue a bounded repair brief against the actual head. If the agent is gone, a new agent uses this exact report and brief; no prior chat is required.
7. After Kosta's review, update the decision/status ledger with his actual disposition and prepare the next self-contained brief. Downstream briefs remain provisional until then.

## Reusable launch prompt

Replace the batch path and starting SHA with the supervisor-issued values before use:

```text
You are implementing one owner-authorized batch for stcksmsh/kabl.
Batch brief: <exact path>
Expected starting commit: <SHA>

Read the brief and docs/product-research/AGENT-WORKFLOW.md, then its ordered
reading list. Check current origin/master and reconcile any newer changes.
Implement this batch completely in small working increments; internal steps
are not approval gates. Preserve prior owner decisions and compatibility.
Do not implement another batch. Leave .ai untouched. Do not format the workspace.
Push working commits to master; if this environment mandates a branch, use it
and provide a PR for Kosta to merge. Never approve on Kosta's behalf.
Produce the required evidence, README, CHECKLIST and REPORT, update HANDOFF and
STATUS accurately, and return the report with the exact submitted commit.
Then stop for supervisor checking and Kosta's hands-on review.
```

This prompt contains no vendor-specific coordination system. Repository state and evidence are the handoff; the conversation only authorizes the next unit of work.
