# kabl — product direction and sequential delivery plan

Date: 2026-09-25. Repository audit: `master` at `c1def5361d66caeee477b2be5c737c7e343552f3`.

**Status: researched proposal, not authorization to implement every batch.** Kosta requested this plan and confirmed the priorities below. He has not approved the proposed delivery order or the pending Composition and Sound Palette batches. A planning commit is not product approval.

Read alongside [research and sources](product-research/RESEARCH.md), [agent workflow](product-research/AGENT-WORKFLOW.md), and the [first implementation brief](product-research/briefs/D01.md). The older `docs/PLAN.md` is the historical rack-design plan, not this continuation roadmap.

## Post-D01 planning addendum — 2026-09-25

Kosta explicitly instructed: **D01 stays the same; no updates to its agent's work.**
The active D01 brief and launch prompt remain its complete review contract. Do not apply
later design findings retroactively or pause its work for this planning addendum.

For D02 and later, read [Post-D01 interaction plan](product-research/POST-D01-INTERACTION-PLAN.md).
It resolves the proposed patch/module model, records source-grounded risks, and assigns
28 interaction questions and 12 workflow scenarios to future scopes. Most ordering remains
unchanged; routing, state ownership and replacement rules must be explicit before their
dependent features. All new architecture remains recommended, not owner-approved.

Laptop checks and the two previous hands-on reviews remain pending while separately
authorized cloud engineering proceeds. This is a review deferral, not acceptance or a
claim that D00 is complete. No later batch is authorized by this addendum.

## 1. Product intent

Build a musical instrument Kosta uses himself, and which other musicians can use without his help. It must support both evolving, performed, interlocking sequences in the spirit of Tangerine Dream's Encore/Ricochet/Force Majeure era and ordinary MIDI instrument production in REAPER. These are two uses of one engine and patch model, not separate products or a reason to build a DAW.

The central promise is: **start with a useful sound, change it intentionally, understand what made the difference, and reuse what you learned.** Reduce time spent learning kabl-specific procedures; do not pretend that sound design itself requires no learning.

Confirmed by Kosta in this planning session:

- The next few deliverables should prioritize shaping sounds quickly.
- The first external audience is musicians new to modular synthesis, not people assumed to know nothing about music.
- Linux and REAPER come first.
- Kosta's laptop is the performance baseline. Pi was an example, never a product requirement. Improve efficiency to broaden access when feasible; do not turn hypothetical low-end hardware into a release gate.
- Users should be able to make their own modules, with light/dark background images, chosen control/jack positions and size.
- Composites made from existing components are desirable, **and** custom DSP/code/behaviour is a long-term goal. Composites do not silently replace that goal.
- Fresh implementing agents run sequentially in separate containers, started by Kosta. The supervisor scopes and checks work; only Kosta approves it hands-on.

Recommended product principles, subject to adoption:

1. A working sound precedes a lesson. No forced tutorial or feature unlocking.
2. Musical controls reveal their real targets. “Brightness” must lead to its actual filter/oscillator routes, not become an opaque second parameter model.
3. Complexity can be opened, inspected, and edited. A reusable voice has a convenient front panel and inspectable internals.
4. Existing rack character stays. Improve access around the approved rack; do not replace it with an unrelated interface or move modules when cables are hidden.
5. Current patch state is authoritative. Help must describe the actual patch, and admit what it cannot infer.
6. Old music keeps working. Sound, identifiers, automation and saved assets need deliberate compatibility rules.
7. Prefer a small, coherent instrument over feature-count parity with mature synths.

## 2. Where kabl actually is

This is a source/document audit, not a fresh build, listening test or hands-on usability study. Reported verification is 407 tests passing, 13 ignored writers, clean workspace clippy; these results were not rerun in this planning session.

| Product area | Evidence at the audited commit | Consequence |
|---|---|---|
| Rack and editing | Owner-approved rack, themes, cable presentations, modulation routes, undo, save/load, live graph swaps | Preserve these foundations; do not restart the UI or engine |
| Sound engine | Registry contains **21 built-in kinds**, including clock/cues/macros as well as sound modules | Root README's “9 modules” is stale; more oscillators are not the immediate bottleneck |
| Standalone composition | Interlocking sequences, transport, delay/reverb, performance controls and recording exist; Composition adds banks/cues/probability/synced LFO | Much of the musical north star is implemented; usability and owner evidence are incomplete |
| Playable palette | Six voices plus the combined piece; mono/poly/legato, glide, sustain, per-voice noise | Enough variety for the first sound-shaping workflow; Sound Palette awaits listening review |
| Finding/saving sounds | Toolbar uses a text path and Save/Load; factory examples live in repo directories | No proper preset-discovery and user-library workflow |
| Explaining/learning | `ModuleInfo.explain` and lesson metadata exist; UI does not consume the explain field; `learn` is a stub; confusions log is empty | The main differentiation is largely unbuilt and untested |
| Host use | `crates/clap/src/lib.rs` is a stub | No usable REAPER instrument yet |
| Control delivery | Audio-affecting editor changes rebuild the graph; MIDI CC mappings are processed through the UI | DAW automation and editor-closed behaviour need a real runtime control path |
| Timing | `PatchEngine::process_block` takes 64-frame arrays; callback adapter drains notes before rendering | Host event offsets and arbitrary host buffer lengths need explicit support |
| Keyboard expression | Notes and sustain implemented; `KeyEvent` ignores channels and has no pitch bend/pressure | Conventional expressive MIDI use is incomplete; CC learn alone is not a modulation-wheel source |
| Reusable modules | `pedals` is a stub; module registry is static | User-authored composites and code modules are not implemented |
| Panel customization | Skin metadata/renderer supports embedded images, positions and light/dark variants; no user-facing authoring/import workflow | Existing mechanism is a foothold, not a delivered creator feature |
| Reliability | Stream-stall detection exists; recovery does not. Dense cloud runs had xruns/stalls | Add a bounded recovery path; establish laptop evidence before tuning by VM estimates |
| Distribution | Source repository, examples and build commands; no release pipeline visible in the audited tree | A fresh-user installation test is needed before calling it shareable |

Grounding: `crates/modules/src/registry.rs`, `info.rs`, `skin.rs`; `crates/ui/src/{lib,main,editor}.rs`; `crates/engine/src/{keyboard,compile,patch_engine}.rs`; `crates/core/src/format.rs`; `crates/{clap,learn,pedals,cables}/src/lib.rs`. See research for permalinks.

### Approval and evidence ledger

- Closed: rack migration, sequencing, interlocking sequences, echo, performance.
- **2026-09-25 update:** Kosta authorized D01 as a whole and explicitly deferred the Composition + Motion and Sound Palette hands-on reviews (laptop unavailable); both remain pending, not approved. D00 is not complete. D01 is built, awaiting Kosta's review: `docs/find-play-save/`.
- Composition + Motion: merged, awaiting Kosta's hands-on review.
- Sound Palette: merged through PR #1, awaiting Kosta's listening/keyboard review.
- Sound Palette cloud evidence used a FIFO MIDI stand-in and silent sink. Dense-piece take and walkthrough used 512 frames. Laptop performance at 48 kHz/256 is not established by those recordings.
- Composition's isolated 3–5 ms callback remains unexplained. Its README's exclusion of several causes is stronger than the evidence warrants.
- Stream stall after cloud xrun bursts is observed; laptop occurrence is unknown. The checklist needs an explicit stall/recovery observation, not only xrun counting.
- No 96 kHz/64-frame or Pi performance claim. Pi is not an active target.

### How far from the end goal?

**Musical building blocks: relatively far along. Approachable product: early. Reliable DAW instrument: not implemented. Creator ecosystem: foundational metadata only.** A numeric “percent complete” would conceal these separate axes.

The present evidence demonstrates scripted capabilities and several approved hands-on workflows. It does not demonstrate that a new musician can find a sound, understand a patch or complete a track without help.

## 3. Competitive conclusion

The market already has visual modulation, macro pages, presets, tutorials, modular patching and user-made instruments. None alone distinguishes kabl. Vital/Surge make the free production-synth baseline strong; Rack/Cardinal establish modular breadth; Bitwig establishes strong in-context assistance; Max/Reaktor establish user-built instruments. Detailed comparison and sources are in RESEARCH.md.

The opportunity is a deliberately coherent combination:

- **Immediate musical use:** curated, playable starting points with useful controls.
- **Explanations connected to the sound:** trace from a musical control to the exact routes and modules changing it.
- **Learning without leaving the instrument:** short optional experiments on the patch being used.
- **Reusable instruments that open up:** custom panels for performance, internals for learning and editing.
- **Standalone performance and normal REAPER production:** one sound and one patch identity across both.

This is a hypothesis to test, not a claim of novelty or superiority. Learning and sound quality must be demonstrated with people and listening, respectively.

### Adopt, adapt, defer

| Adopt/adapt | Why | Avoid importing with it |
|---|---|---|
| Surge-style patch search, categories, favorites, save protection | Gets existing sounds into users' hands | A giant metadata/database project |
| Vital-style visible modulation and reversible experimentation | Makes cause/effect legible | Decorative animation or assuming every destination needs audio-rate modulation |
| Bitwig-style contextual help, selected signal inspection, compatible replacement | Teaches at the point of use | Hidden auto-wiring that users cannot discover |
| Pigments-style emphasis on performance controls | Reduces first-contact complexity | A second editor with duplicate parameter state |
| Syntorial/Ableton-style short listening experiments | Hearing a difference teaches more than a paragraph alone | Mandatory curriculum, locked controls, scoring subjective sound quality |
| Max/Reaktor-style encapsulation and reusable interfaces | Turns a complex patch into a reusable musical tool | Live-linked updates silently changing old songs |
| Cardinal-style self-contained project dependencies | Reliable interchange and host recall | Loading an unbounded external plugin ecosystem inside the first release |

Copy interaction principles, not proprietary source, presets, artwork or instructional text. Reuse code only after reviewing the exact dependency and its license; the current workspace declares MIT OR Apache-2.0.

## 4. Measure the actual promise

Proposed formative tests, not universal guarantees. Record results before and after; failures alter the next brief.

| Task | Initial acceptance target | Evidence |
|---|---|---|
| First useful sound after launch | Find and audition a named factory category within 60 seconds, no terminal or manual | Unassisted screen recording; distinguish app startup from installation time |
| Intentional edit | Make a pad brighter and slower to attack, or a lead less harsh, within 2 minutes | User identifies changed controls and judges the result |
| Understand the edit | Find one underlying target and explain what it changes | Ask user to demonstrate, not repeat help text |
| Recover | Undo an experiment; diagnose one prepared silent patch | Exact state recovery and correct observed cause, without supervisor hints |
| Keep the work | Save as a new sound, reopen it, retain original factory patch | Reopened patch and render/state checks |
| Transfer learning | Perform an analogous change on a different patch | No step-by-step guidance on the second patch |
| REAPER production | Record MIDI, automate a control, close/reopen project, render with plugin editor closed | Project and reproducible audio; no reliance on external patch paths |
| Creator use | Package a voice, reuse two independent instances, share and inspect it | Fresh-profile import with artwork and dependencies intact |

Start with Kosta, then 3–5 musicians new to modular if available. These are formative observations, not statistically valid market research. Kosta's prior knowledge cannot establish beginner ease on its own. Recruit/test only when Kosta arranges or authorizes it; do not contact people independently.

## 5. Delivery order

**Default order is sequential.** Each D-number is one separately authorized batch, one fresh implementing agent, small working commits within it, one end-of-batch review. Internal steps are not approval gates. If a scoped batch turns out too large, the supervisor narrows the outcome before Kosta starts it; the implementer does not silently expand it.

| Batch | Usable outcome | Depends on | Main risk |
|---|---|---|---|
| D00 | Accepted baseline and explicit unresolved evidence | Existing batches | Mistaking merged/scripted work for owner approval |
| D01 | Find, play, modify and save a sound | D00 review disposition | Losing edits or requiring repo paths |
| D02 | Understand what a musical control changes | D01 | Explanations detached from actual routing |
| D03 | Diagnose and learn by listening inside a patch | D02 | Guessing why silence occurs; tutorial overhead |
| D04 | Turn controls without rebuilding the whole graph | D03 | State races and compatibility regressions |
| D05 | Recover audio; reduce proven redundant work | D04 | Changed mono gain/state or false recovery loops |
| D06 | Correct expressive MIDI and host-ready event timing | D05 | Buffer-dependent notes, modulation or feedback |
| D07 | Play and reopen kabl as a Linux REAPER instrument | D06 | GUI integration and incomplete state recall |
| D08 | Automate and synchronize a complete REAPER arrangement | D07 | Unstable IDs, host/editor coupling, transport ambiguity |
| D09 | Package an inspectable patch as a reusable module | D08 | Identity, voice scope, dependency/version semantics |
| D10 | Design and share custom light/dark module panels | D09 | Unreadable or unreachable controls; missing assets |
| D11 | Prove bounded custom-code modules on the real engine | D10 | DSP execution cost and state/failure handling |
| D12 | Supported creator SDK and portable module packages | D11 findings | Version compatibility and creator burden |

Useful stopping points: **D03 approachable standalone preview; D08 personal-production beta; D10 composite creator preview; D12 code creator beta.** Each needs the release hygiene below. Do not delay using D08 until a community ecosystem exists.

### D00 — Baseline and review disposition

No feature expansion. Assemble the two current checklists and launch commands; add explicit stall and Composition-spike observations; correct active doc drift without rewriting historical evidence. Kosta listens/plays at 48 kHz/256 on his laptop. Capture exact commit, device/backend, controller and RT status. Record two separate approval outcomes, or specific defects/deferred checks.

If listening or usability defects invalidate a starting patch, fix them in the relevant existing batch before building a browser around it. If laptop instability blocks ordinary playing, pull the necessary part of D05 forward. Unrelated cosmetic defects need not block D01 if Kosta explicitly accepts deferral.

Acceptance: an accurate status ledger and a specific first-batch authorization. No agent can manufacture owner evidence. Existing 407-test report is historical until rerun against the implementation head.

### D01 — Find, play, save

Outcome: open kabl, choose a voice by name/category, audition it without hardware, change its existing performance controls, save a personal version, and find it again.

Scope: factory/user separation; searchable browser with a small tag vocabulary; favorites/recent sounds; simple safe audition note/chord for keyboard patches and explicit start/stop for sequences; Save As and unsaved-change protection. Make the existing six palette voices and composition demos discoverable. Offer a simple initialized keyboard patch as a starting point. Reuse the Perform panel, named macros and current patch format.

Internal increments: metadata and discovery; load/audition; save protection and user library; real-app evidence and owner checklist. Avoid a wholesale file-format rewrite or cloud account. Prefer simple local metadata; no database unless measurement makes it necessary.

Acceptance: all first-use/save tasks above work at both viewport sizes; failed load/save preserves the working patch; factory files remain unchanged; every preview stops; no stuck preview notes after Load or device changes. No manual path typing for the normal path. Fully specified in [D01 brief](product-research/briefs/D01.md).

### D02 — Musical controls that explain themselves

Outcome: a user can turn “Brightness” or “Motion” and immediately find what it changes.

Scope: improve the existing performance presentation and add explicit reveal/highlight of a macro's actual destinations. Add concise module/parameter help, units, source/destination navigation, and clear base-value versus modulation-range explanation. Describe a patch's signal path from graph facts, with authored musical intent as separately identified metadata. Surface existing `ModuleInfo.explain` rather than starting an unrelated help engine.

Begin with current factory voices; author useful ranges/names per sound. Do not impose a universal “Brightness” control on arbitrary patches where it has no valid meaning. Keep full rack access and current cable modes. No LLM-generated explanations at runtime.

Acceptance: macro edits, direct edits, routes and undo remain the same state; help follows changed/deleted routes; all factory controls have understandable descriptions; D02 walkthrough shows a novice-facing action and the revealed mechanism. A visual rework must earn its complexity in the sound-shaping tasks.

### D03 — Signal inspection and three learning recipes

Outcome: inspect why a patch behaves as it does, then learn one useful technique without leaving the instrument.

Scope: bounded selected-signal telemetry (level/gate/pitch or a small scope as relevant), and an observation-led “Why no sound?” aid. Distinguish graph facts (“output disconnected”), measured facts (“no gate observed”), and possibilities (“filter may be suppressing this signal”). Do not claim to know the user's intent or diagnose a silent external device from a valid internal signal.

Build only three optional recipes: envelope from pluck to pad; filter/LFO movement; two interlocking sequences. Each has a starting patch, one audible change at a time, revealable targets, a reversible comparison, and an analogous unguided task. Users can exit and keep their sound. Add an ordinary patch comparison mechanism; do not silently normalize the actual instrument output to make a comparison persuasive.

Acceptance: selected inspection has measured bounded overhead and never blocks audio; an intentionally broken connection and missing trigger produce accurate observations; undo/exit restores or retains state as requested. Record real confusion in `docs/confusions.md`; scripted completion is not learning evidence.

### D04 — Runtime controls without recompilation

Outcome: routine knob, macro and controller moves update the playing engine directly, preparing reliable DAW automation and reducing edit overhead.

Scope: classify topology/structural changes versus runtime parameter changes. Add bounded, real-time-safe parameter delivery keyed by stable target identity, smoothing rules, and coherent state/undo synchronization. Keep graph compilation and deferred destruction off the audio thread. Preserve graph swaps for actual structural edits. Handle active, incoming and queued graph generations explicitly so a newer parameter edit cannot be overwritten by an older compiled graph.

Do not assume all params are nonstructural: voice policy, buffer requirements and module-specific preparation can require different treatment. Move production control semantics out of GUI polling; keep CC learning itself in the UI.

Acceptance: constant settings preserve legacy sound; rapid edits across swaps/undo/load retain the last intended values; no callback allocation, blocking or unbounded queue growth; closed-UI control delivery works. Compare dense-piece steady/turning-knob execution distributions against the old path on the same machine. Do not claim this explains the Composition spike.

### D05 — Recovery and measured efficiency

Outcome: a failed standalone audio stream is recoverable without losing the patch, and proven redundant work is reduced where worthwhile.

Scope: visible restart/reselect-audio action, bounded retry/error state, clear recorder consequences, deterministic note release and stream teardown. Reuse the existing stall detector. A device restart may end a take; never conceal a recording gap as a complete continuous take. No endless restart loop.

Then profile the mono-chain candidate after D04. Implement single-lane compilation only if still justified: chains reachable solely from mono keyboards, with correct treatment of mixed sources/consumers, polyphony transitions, modulation, noise, gain averaging, tails and overlapping swaps. This is not equivalent to marking the chain globally unvoiced: existing voice averaging must not multiply its level.

Acceptance: forced fault and real-backend recovery evidence distinguished; no duplicate audio threads or stuck notes; state/gain regressions tested against a reference; before/after timings at 48 kHz/256 on comparable hardware. If mono optimization is not justified, deliver profiling and recovery and explicitly defer it. The unexplained spike stays open until evidence supports a cause.

### D06 — Expressive, correctly timed MIDI

Outcome: bend a lead, add modulation-wheel expression, and schedule notes correctly regardless of host buffer partitioning.

Scope: pitch bend with a visible per-keyboard range; modulation wheel as a usable musical modulation source, distinct from absolute CC pickup; sustain/panic semantics; the event/processing adapter needed for host-timestamped events. Keep MPE, aftertouch and splits separate unless a concrete acceptance task needs them. Advertise only supported note dialects; preserve the identity guarantees required by the chosen input contract rather than silently dropping unsupported identity information.

Important invariant: adapting the 64-sample engine must not change musical results just because a host uses a different buffer size. Preserve the established modulation grid and feedback-delay semantics, or explicitly version a necessary change. Splitting processing at events without considering those rules is insufficient. Report any adapter latency accurately.

Acceptance: identical timestamped performance across 1/63/64/65/127/256/512 and mixed buffer partitions, within specified DSP tolerance; notes at boundaries and within blocks, repeated notes, pedal, note-off, reset and bends behave correctly. MIDI handling cannot depend on an editor repaint. Laptop standalone behaviour remains playable.

### D07 — A real REAPER instrument

Outcome: insert kabl on a track, play/record a palette voice, edit it, save the project, reopen it, and render with the editor closed.

Scope: CLAP first, stereo output and a supported MIDI/note input; reuse the engine, patch browser and editor. No CPAL/midir/device watchdog inside the plugin: REAPER owns audio and MIDI. Embed the complete patch state in host state, not a filesystem path. Standalone and host serialization must represent the same sound. State decoding/building must respect host lifecycle/thread rules.

First internal step: a runnable integration proof with the actual current egui editor, note input, one persistent control, state recall and editor reopen. Evaluate nice-plug/its egui adapter against Clack; original NIH-plug is in maintenance mode. Choose/pin a framework and record why, then finish the batch without a routine approval pause. A sample plugin with an unrelated GUI does not retire the integration risk.

Acceptance: current REAPER on Kosta's Linux setup; two independent instances; MIDI recording/playback; patch/routing edits; project relocation with source patch directories unavailable; state reload; editor open/close repeatedly; offline render; audio-rate reconfiguration as advertised. Run CLAP validator plus actual host tests. This milestone is an instrument preview, not yet a claim that all internal sequencers follow REAPER.

### D08 — Host automation, transport and personal-production beta

Outcome: use kabl for an actual REAPER arrangement, including host-synchronized sequences and reproducible automation.

Scope: a bounded bank of stable automation slots (recommended initial cap: 16) assignable to meaningful controls. Slot IDs never depend on panel order or graph indices. Persist mapping identities; deletion leaves an unassigned slot rather than silently reusing an automation lane. Stable macro/pin targeting comes before exposing thousands of dynamic module parameters.

Add explicit Host/Free clock mode and documented play/stop/seek/loop/tempo-change rules, with a stated initial time-signature scope. Preserve free clocks in standalone. Handle continuous CC and button actions without an open editor. Distinguish host automation playback from patch-edit undo and record gestures appropriately. State must contain mappings, sound settings and dependencies; transient held keys and queued launches need explicit reset rules, not accidental serialization.

Acceptance: record/play automation, close/reopen, change layout without retargeting; render a sequence across tempo changes and loop/seek boundaries; repeated offline renders match under the documented seed/reset policy; tail behaviour and reported latency are correct. Finish a short multi-track REAPER piece with at least a played voice and a sequenced voice. Add Linux install/uninstall instructions and a source-free installation test. This is the first production beta, contingent on Kosta's approval.

### D09 — Reusable, inspectable composite modules

Outcome: select an existing voice/effect, expose a small interface, use two copies independently and open either to understand it.

Scope: composite definition with stable exposed port/control IDs, internal graph, instance identity, documentation and bounded nesting. Recommend flattening at compile time into the existing graph schedule; avoid a recursive engine per composite unless profiling and semantics require it. Name the interface and expose musical controls using existing routing semantics.

Define embedded snapshots versus library definitions before coding. Recommendation: a song embeds the exact definition/version it used; publishing an update never changes existing songs automatically. “Edit this instance” and “publish a new version” are distinct actions. Graph cycles and voices still follow the engine's rules; grouping is not a magic change of rate.

Acceptance: encapsulate/open/duplicate/undo/save/reload; sound matches the original flat patch; two instances have independent histories; external routes, cues, pins and host automation retain identity; no extra latency from grouping. Reject recursive definitions and explain missing dependencies. Use one voice and one stereo effect as the examples, not a general-purpose language project.

### D10 — Custom panel authoring and sharing

Outcome: a user gives a composite its own instrument face and shares a complete, working package.

Scope: import separate light/dark images, place and size knobs/selectors/jacks, set panel width within rack constraints, label controls, preview both themes, and export/import assets plus definition and metadata. Begin with current rack height; arbitrary multi-row geometry is a later design decision. Built-ins retain their approved default skins.

Use the same exposed control IDs as D09. Provide accessible fallback controls when artwork is absent or unsuitable; all public parameters remain reachable. Validate bindings, control bounds/overlap, image size and package paths. Help/explain belongs to the package so a beautiful panel can still teach.

Acceptance: build a panel from the GUI, load into a fresh profile without original image paths, use at both viewport sizes/themes, resize/zoom without hit-test drift, rename controls without breaking automation, and restore a REAPER project after the library copy is removed. No marketplace/account dependency. Demonstrate with original or appropriately licensed artwork.

### D11 — Custom code feasibility with runnable modules

Outcome: run new behaviour that cannot merely be assembled from current built-ins, and establish a measured implementation route.

Scope: prototype a versioned code-module interface on the real engine, separate prepare/reset/process/state/metadata, with explicit port/parameter and voice rules. Preferred hypothesis: a bounded WebAssembly runtime for distributed modules, with compilation/validation and memory preparation outside audio. Compare a native reference; evaluate Faust as an authoring/front-end option, not a promised runtime shortcut. No requirement to support several languages at once.

Examples must include control-rate arithmetic/logic and a stateful audio processor, using the D10 panel metadata. Measure one and multiple instances, polyphony and graph swaps. Demonstrate trap, infinite-loop/budget exhaustion, nonfinite output and incompatible-version handling without taking down the instrument. A sandbox or fuel counter alone does not prove real-time suitability.

Acceptance: executable prototype, reproducible performance/fault data, documented limits and a recommended backend pinned to tested versions. If the preferred runtime misses the budget, return a specific alternative and evidence; do not ship an unsafe native-loader fallback or advertise unrestricted real-time code. The batch may end with a documented feasibility failure. That is useful evidence, not permission to expand scope indefinitely.

### D12 — Supported creator SDK and portable code packages

Outcome: another developer can implement, document, skin, install and share a new module without editing kabl's source tree.

Scope follows D11's demonstrated route: one template and build command, supported ABI, state/version rules, parameter metadata/help, package validation, example code modules, dependency/asset bundling and clear errors. No real-time file/network access or compilation. Explicit capability and resource limits are part of the SDK contract.

Acceptance: a fresh agent working only from the published SDK instructions creates a distinct useful module; it survives save/reload, automation, polyphony where advertised and host render. Existing projects retain exact module versions or fail with an actionable missing-version report. Publish no “infinite performance/freedom” claim. A local package workflow is enough; a community store, accounts and payments remain separate proposals.

## 6. Release hygiene and performance policy

Do this incrementally, not as an end-of-project cleanup:

- Starting D01, launch outside the repo with factory resources discoverable. At preview milestones, provide a reproducible Linux build/package and exact install/run instructions.
- Every batch maintains README, decisions, measured limits, concise owner checklist, scripted walkthrough with audio where relevant, and screenshots at 1440×900 and 1280×800 in A-light/A-dark. Engine-only batches demonstrate the user-facing effect in the real app; plots/logs supplement rather than replace the walkthrough.
- Automated evidence must include commands, exit codes, commit, environment and artifact links. Headless/cloud evidence is useful but cannot substitute for hardware feel or laptop callback data.
- Reference live setting: 48 kHz/256 frames (5.333 ms callback budget). Seek zero observed xruns/late execution in a representative laptop session; report execution, arrival and xruns separately. Quantiles and maxima need run duration/count. No finite soak proves impossibility of a future miss.
- Performance corpus: simple voice, dense palette piece, repeated edits/swaps, two plugin instances, then composites/code modules. Measure each new layer's marginal cost. Do not extrapolate laptop pass/fail from the cloud CPU ratio.
- Do not optimize by removing musical behaviour, changing voice averaging, hiding aliases or reducing quality without an explicit product decision. Expensive options can have disclosed limits.
- Use regression/RT tests where they protect real contracts. Do not inflate tests around trivial presentation changes. Preserve legacy patches and the no-allocation audio path.
- Public artifacts need coherent license files/notices and declared artwork/patch licenses; exact dependencies are reviewed before incorporation. No pricing/business-model decision is required for these batches.

## 7. What is deliberately outside this sequence

| Deferred | Reason to defer | Reconsider when |
|---|---|---|
| More synthesis families: sampling, granular, large wavetable/FM engines | Existing palette can test the usability thesis first | A named musical task cannot be met well with current modules |
| Full song timeline or piano roll | REAPER supplies arrangement and MIDI editing | Standalone performance has a demonstrated need not served by cues |
| MPE, aftertouch, splits, multitimbrality, multi-output plugin | Not prerequisites for the chosen first workflow | Kosta or initial testers need a specific expression/routing task |
| Swing, ratchets, richer LFO/MSEG, phaser/tape emulation | Plausible musical additions, not all prerequisites | Listening/composition sessions identify the highest-value gap |
| Universal audio-rate parameter modulation or quality-tier overhaul | Expensive semantic/DSP expansion | Audibly needed for a scoped module or modulation task |
| AI text/audio-to-patch generation | Different technical project and not necessary for the learning path | Curated explanations/workflows demonstrably fail a useful goal |
| Marketplace/social accounts/cloud services | Package portability must work first | Real creators are publishing and requesting distribution support |
| Windows/macOS formats and installers; VST3/AU/LV2 | Linux CLAP/REAPER is the confirmed initial scope | A tested Linux beta and identifiable users justify expansion |
| Raspberry Pi gate, SIMD rewrite, framework rewrite | Not user requirements; performance must be measured | A verified bottleneck on supported machines requires them |
| Historical “cables as instruments” pattern/morph system | Existing routes, macros and sequencers already cover many cases | A specific musical interaction needs connection-local behaviour |

Deferred does not mean forbidden or permanently rejected. Add the smallest feature that solves a demonstrated task; don't turn this table into a ban on future synthesis.

## 8. Remaining decisions and next action

The following are **recommendations, not recorded owner decisions**: exact batch order; no forced learning locks; composite-first implementation; bounded automation slots; compile-time flattening; snapshot-based composite updates; initial fixed-height custom panels; Wasm as the code-runtime hypothesis.

Choose DSP/GUI libraries inside the relevant authorized batch based on an executable proof. Ask Kosta only when evidence forces a real product tradeoff (e.g. different musical behaviour, narrower customization, materially greater latency), not for routine implementation choices.

Next: Kosta reviews this direction/order and gives the disposition of the two current batches. Then start D01 using its brief, or a narrowly defined repair if current reviews reveal a blocker. Do not start D02–D12 just because this document exists. The supervisor issues one fresh, current brief after each batch, incorporating the actual resulting code and owner feedback.
