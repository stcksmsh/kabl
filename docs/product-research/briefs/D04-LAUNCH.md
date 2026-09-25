You are implementing Kosta's authorized combined assignment for stcksmsh/kabl: finish D03, then implement D04. No parallel work.

Read docs/product-research/briefs/D04.md and docs/product-research/AGENT-WORKFLOW.md, then the brief's ordered sources. Fetch current origin/master. Product baseline: 6404b219837d87da59029390052e62ac6290da1f (merged D03 + D03-R1); the planning commit adds this assignment. Preserve newer work.

First complete D03-R2: correctly identify the disconnected intermediate audio input in Why no sound?, without treating optional inputs as faults, and disambiguate old delivered stream-error messages from current interval counts. Then complete D04: classified runtime controls without recompilation, stable identity and generation ordering, bounded real-time-safe delivery, coherent document/undo/save state, and mapped MIDI control independent of editor repainting. Follow the brief's exact invariants and acceptance matrix.

Kosta accepted only the narrow D03-R1 backend-error overflow free/allocator-lock exception. It does not relax the normal data/control callback contract. Owner reviews remain pending; cloud evidence is not laptop approval.

Work through the entire assignment without internal approval pauses. Small commits, no workspace formatting, no .ai/, no force pushes and no PR merging. If the cloud requires a branch, provide a new PR for Kosta.

Produce the required final-head tests, performance comparisons, real-app/package evidence and docs. Use a fresh reviewer subagent for the combined code; fix findings and obtain a final recheck. Return exact tested/reviewed/submitted commits and separate D03-R2/D04 dispositions. Stop after D04; do not implement D05.
