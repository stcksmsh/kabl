# D03-R2 + D04 final independent recheck — reviewer prompt

Recheck stcksmsh/kabl PR #6 after the implementing agent addresses the independent review.
Use the existing independent reviewer's context if available; otherwise use a fresh reviewer.
This is not implementation and not owner acceptance.

Fetch the current PR branch and origin/master; record exact full SHAs. Read the original
D04 brief, docs/runtime-controls/REVIEW.md (all findings and responses), the implementation
report, fix commits and changed evidence. Compare against the exact previously reviewed
head, not just the last commit. Do not assume the submitted head is the tested head.

Verify each disposition in the actual code, including all reachable paths affected by a fix.
Re-run targeted checks where needed. Look for regressions introduced by corrections, especially
action/value ordering, saturation, ramps, lifetime/teardown, document/save/history and probe
freshness. Recheck the combined code, not isolated patches against an obsolete base.
If current master adds product changes, require integration and review of that combined
result before recommending merge; planning-only changes do not invalidate product evidence.

Append the final recheck to REVIEW.md: reviewed base/head, per-finding disposition,
new findings if any, coverage, executed versus inspected evidence and limitations.
State whether any blocker/major or original acceptance failure remains. If a consequential
product tradeoff remains, identify it for Kosta instead of approving it yourself.

Commit/push review documentation only to the PR branch when it is the sole writer, or
return the text if access prevents saving. Do not modify product code, submit a GitHub
approval, merge, mark owner checks passed or start D05. Report exact reviewed product head
and any subsequent docs-only head. Hand unresolved findings back for another fix/recheck
cycle; otherwise return the engineering assessment to the supervisor and Kosta.
