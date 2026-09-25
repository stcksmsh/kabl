You are implementing one owner-authorized batch for stcksmsh/kabl: D03, Signal inspection and three listening recipes.

Read docs/product-research/briefs/D03.md and docs/product-research/AGENT-WORKFLOW.md, then the brief's ordered reading list. Fetch current origin/master first. The brief's product baseline is 4eb410465a34fc47fadd35a807fb649ecfa2bad2; the later planning commit adds this brief and records authorization. Preserve and reconcile any newer work.

Kosta authorized cloud engineering while his laptop is unavailable for approximately two days. Hands-on reviews for D01/D01-R1, D02, Composition + Motion and Sound Palette remain deferred, not accepted. D00 is incomplete.

Implement D03 fully: one bounded selected-signal inspector; an observation-led silence aid; one reversible comparison reference; exactly three optional recipes (pluck to pad, filter/LFO movement, interlocking sequences). Preserve sound, document identity, undo/history and failed-save protection. Read the compiler's buffer reuse before adding a tap. Measurements must identify the selected signal and graph and reject stale results. Do not infer device failure or musical intent from zero.

The brief defines comparison and recipe state ownership. Prefer the smallest safe interaction; an explicit restore/undo comparison is allowed. No hidden loss of working edits, no forced lessons, no normalization, no runtime-control redesign or new synthesis.

Also implement the brief's operational logging addition: production INFO/WARN/ERROR by default, DEBUG/TRACE available through a runtime override, bounded rotating local files, and no logging/formatting/I/O on the audio thread. Preserve existing evidence hooks and measure failure/overflow behavior. This is separate from PatchLog.

Work through internal increments without approval pauses. Use small working commits. Do not format the workspace, touch .ai/, force-push or merge PRs. The parallel exception is over: use the normal sequential workflow; if the cloud requires a branch, push it and provide a PR for Kosta.

Produce docs/signal-inspection/{design,README,CHECKLIST,REPORT,REVIEW}.md, real-app evidence, package verification and exact test/performance records. Keep cloud stand-ins and pending laptop checks explicit. Close the two carried D02 real-app evidence gaps identified in the brief.

Before submission, start a fresh reviewer subagent with the brief, exact base/head, actual code/diff and evidence. Fix in-scope findings and obtain the final recheck. Return exact tested/reviewed/submitted commits and the structured report. Update HANDOFF/STATUS and append decisions without changing historical approvals.

Stop after D03. Do not start D04. Ask Kosta only about consequential product tradeoffs that cannot meet the brief, not routine engineering choices.
