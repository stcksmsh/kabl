You are implementing Kosta's authorized D05 assignment for stcksmsh/kabl: audio recovery and measured efficiency.

Fetch current origin/master. Read docs/product-research/briefs/D05.md and docs/product-research/AGENT-WORKFLOW.md, then the brief's ordered sources. Verified product baseline: 5e64101486e942649e1bab6fb9f4f79d7fecbb75, Kosta's PR #6 merge; product code matches tested/reviewed 58a5934. Preserve newer work. Prior owner reviews remain pending.

Implement explicit audio retry/device selection without lost edits, duplicate streams, stale commands or misleading recordings. Close the recovery-relevant pending-Load and Core-lock gaps. Add deterministic fault hooks and actual-backend evidence with honest limits. Then profile the mono-chain candidate; optimize only if measurements and compatibility tests justify it. A documented deferral of optimization is acceptable; incomplete recovery is not a completed batch.

Proceed through internal steps without approval pauses. Use one implementing agent and a fresh reviewer subagent at the end. Fix findings and obtain a final recheck of the actual combined product head. Produce the required docs/audio-recovery artifacts, final-head tests, real-app/package evidence, exact commits and a pending owner checklist.

Small commits; no .ai/, workspace formatting, force pushes or PR merging. Use a new PR if the cloud requires a branch. Do not infer a cause for historical stalls/spikes, claim laptop results from cloud runs or approve hands-on for Kosta. Stop after D05; do not implement D06.
