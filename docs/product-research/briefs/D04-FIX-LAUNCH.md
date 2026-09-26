# D03-R2 + D04 findings and closeout — implementing-agent prompt

Resume the existing D04 assignment on stcksmsh/kabl PR #6:
https://github.com/stcksmsh/kabl/pull/6
Branch: claude/d02-musical-controls-knjmgu.
Starting submitted head was b2171180ae72f7d0e82cc172389ac5c191c464ad, tested product b2e9fd6.

This prompt works in the existing implementation context or a fresh replacement context.
Preserve the existing context if available; no context wipe is required. Fetch the current
PR branch and origin/master, preserve newer work and review-only commits, and record exact
SHAs. Do not reset to master, redo D04 or create a competing implementation.

Read applicable instructions, docs/product-research/AGENT-WORKFLOW.md, briefs/D04.md under
that same directory, and PR-head docs/runtime-controls/{REPORT,REVIEW,design,README}.md.
Read the independent review in full. The earlier rate-limit entry is not a completed review.
If no independent findings/coverage exist yet, report that dependency and stop; do not
substitute self-review.

Address every in-scope finding in small working commits. Reproduce correctness risks first.
For each finding record fixed, not reproduced with evidence, or explicitly unresolved.
Do not silently defer acceptance failures or approve an audible compatibility tradeoff.
Kosta already accepted only the narrow D03-R1 backend-error overflow free/allocator-lock
exception. Keep all other brief constraints and prior decisions.

Run targeted regression checks for fixes and the required workspace test/clippy gates at
the final product head. Refresh affected media/performance/package evidence; do not redo
unaffected evidence without reason. Record exact artifact/build commits and differences.
Update REPORT, CHECKLIST, HANDOFF, STATUS and decisions accurately; keep owner checks pending.

Obtain an independent final recheck. Prefer the same reviewer in its separate context, using
D04-RECHECK-LAUNCH.md. If working inside an environment with reviewer subagents, supply one
the original review, exact fixes/diff and evidence. Never relabel implementer self-review.
Use sequential handoffs: reviewer and implementer must not push competing branch updates.
If independent review is unavailable, keep the PR draft and return that specific gap.

Return exact tested/reviewed/submitted SHAs, finding dispositions, affected verification and
separate D03-R2/D04 acceptance states. A docs-only commit after review is okay when proven
product-identical. A later product change requires another affected-path recheck.
Keep the PR draft while required review/fixes are incomplete. Only Kosta merges.

No force push, .ai/ changes, workspace-wide formatting, new synthesis, stream recovery,
roadmap expansion or D05 implementation. Stop at D04 closeout.
