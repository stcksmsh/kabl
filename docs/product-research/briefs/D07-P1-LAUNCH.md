# D07-P1 — isolated REAPER/CLAP integration proof

You are a fresh cloud implementing agent for stcksmsh/kabl. Kosta authorized this bounded
prototype to run in a separate cloud session and branch while another agent implements D05.
Build and test a runnable integration proof; do not implement full D07 or interfere with D05.

## Outcome and starting state

Prove that kabl's current engine and actual egui editor can operate inside a Linux CLAP
instrument in REAPER: notes and stereo sound, one editable persistent control, host project
state recall, and editor close/reopen while processing continues.

Expected master at issue: e951b2535d817bc7c21872cd0e46e815d35747ac. Its product baseline
is the D04 merge 5e64101486e942649e1bab6fb9f4f79d7fecbb75, matching tested/reviewed 58a5934.
The commit containing this prompt follows it. Fetch origin/master, record its exact SHA,
inspect intervening changes and preserve them. D05 is active in another branch; do not
depend on or modify its unmerged work. Do not assume its branch name.

D04 and earlier hands-on reviews remain pending. No laptop/controller/listening acceptance
can be supplied by cloud runs. This prototype informs D07; it does not complete D06/D07 or
settle the final plugin architecture.

## Strict parallel-work boundary

Create a distinct branch from current master; use the cloud's required branch name if
necessary, but never reuse the D05 agent's branch. Make a separate draft PR for Kosta.

Your only writable tracked paths:
- prototypes/reaper-clap/ — independent prototype Cargo workspace, code, its own manifest,
  lockfile, scripts and local build configuration.
- docs/reaper-clap-prototype/ — report, design, review, evidence and integration recommendations.

Root/shared Cargo.toml, Cargo.lock, .cargo/, production crates (including crates/clap),
packaging, patches, D05 files, HANDOFF, STATUS, decisions.md and other shared docs are
read-only. Record your decisions/status in your own report; the overseer updates shared
records. No product pushes to master, force pushes, PR merging or .ai/ changes. Format
changed Rust files only.

Use path dependencies on existing crates where practical, without changing their source.
Keep prototype dependency/build artifacts in the isolated workspace. Do not vendor a fork
of the engine/editor or modify dependencies in place to evade the boundary. Prototype glue
and adapters are allowed. If an essential seam cannot be bridged without a production
change, document its precise interface and a minimal proposed diff in your report; do not
apply it. Continue independent useful checks and return a bounded feasibility limitation.

D05 has priority on production lifecycle/control code. These boundaries override the normal
workflow's master-push and shared-document-update instructions for this task.

## Ordered reading

1. Applicable repository instructions.
2. docs/HANDOFF.md and STATUS.md top, as baseline context only.
3. docs/product-research/AGENT-WORKFLOW.md in full, applying the parallel boundary above.
4. docs/PRODUCT-PLAN.md D06–D08; relevant plugin research in
   docs/product-research/RESEARCH.md. Reuse prior research, answer only concrete gaps.
5. docs/runtime-controls/{design,REPORT,REVIEW}.md, especially control/document ownership
   and the final recheck. Read docs/product-research/briefs/D05.md to understand overlap.
6. Actual public APIs in crates/ui, engine and core, the existing CLAP stub, serialization,
   runtime-control delivery and the fixed-block processing contract.

## Implementation scope

1. Inspect current primary documentation/source for candidate CLAP frameworks and egui
   embedding, including the previously considered nice-plug/egui and Clack approaches.
   Verify current compatibility and licensing; do not inherit outdated maintenance claims.
   Choose one for the proof and pin exact versions/revisions. Compare on paper/source first;
   build one successful path, and change approach only for a concrete blocker. This is
   a recommendation backed by code, not blanket owner approval of a dependency strategy.

2. Build a loadable Linux CLAP instrument using kabl's real engine. Let REAPER own audio,
   MIDI and transport facilities. Do not instantiate standalone CPAL, midir, a device
   watchdog or a recovery loop inside the plugin.

3. Embed kabl's actual current egui rack/editor, wired to the same patch the engine plays.
   An unrelated demo GUI or rendered screenshot is not sufficient. Demonstrate one real
   parameter edit audibly, and that editor close/reopen preserves it. Keep DSP and control
   delivery independent of editor painting.

4. Supply MIDI/note input and stereo output for a simple existing playable voice. Explicitly
   state supported note dialect, rate/buffer assumptions, event timing and adapter latency.
   The engine's fixed 64-frame blocks are a known D06 dependency: an isolated bounded adapter
   is allowed, but do not claim sample-accurate or buffer-partition-invariant behavior unless
   tested. No production timing/modulation rewrite, silent event loss or callback panic on an
   unsupported buffer. Support the tested host configuration safely and report wider limits.

5. Persist the complete relevant patch state in host state rather than a path to the source
   patch directory. Demonstrate a parameter plus a small patch/routing edit surviving save,
   REAPER exit, reopen and editor reopen. Decode/build state on appropriate lifecycle threads;
   no compilation, filesystem I/O, logging, locks or allocation in normal audio processing.
   Handle corrupt/incompatible state without replacing valid working state silently.

6. If needed for integration, expose one stable prototype host parameter and show it controls
   that actual patch target. This does not establish D08's dynamic automation mapping design.
   No host-synchronized sequencer implementation, multitimbrality, multi-output expansion,
   comprehensive preset browser redesign or new synthesis.

## Verification

Use an actual Linux REAPER installation and record its exact version, CLAP framework,
compiler, GUI backend and audio setup. A virtual display/null sink is acceptable cloud
evidence when labelled. Use official sources and normal licensing; do not distribute host
binaries or bypass licensing/access requirements.

Required demonstrations:
- REAPER discovers and inserts the instrument; a MIDI item produces an audible/rendered
  stereo result from the real kabl voice.
- Actual kabl editor opens, changes the control, closes and reopens repeatedly without
  lost state; playback and offline rendering work while the editor is closed.
- Save/reopen the project with the original patch source directory unavailable. The sound,
  control and patch edit restore from host state.
- Two instances have independent parameter/patch state. Editing one does not affect the other.
- Clean repeated instance creation/destruction and deactivate/reactivate; no plugin-owned
  audio device or background thread lifetime leak.
- Run an appropriate CLAP validator at its pinned version. Report warnings/failures and
  unsupported capabilities accurately.
- Check changed real-time paths and state/thread ownership. Test supported buffer boundaries,
  note release and state reload as relevant to the prototype, without pretending to close D06.

Use fresh temporary user profiles/project files and installation paths scoped to the
experiment. Never overwrite the owner's REAPER projects or preferences.

If REAPER or GUI hosting cannot run in the cloud environment, preserve build/validator
results and exact failure evidence. A different host or harness may provide supplemental
coverage, but cannot be presented as a REAPER pass. Return the missing host experiment and
resume instructions rather than expanding into unrelated work.

## Evidence, independent review and return

Deliver docs/reaper-clap-prototype/{README,REPORT,REVIEW,CHECKLIST}.md, a concise design/
dependency rationale, exact build/install/run commands and relevant host/validator logs.
Include a short scripted real-host walkthrough with audio, screenshots of the actual editor,
and a reproducible project/MIDI fixture where legally redistributable. Keep large media
curated. Generated plugin binaries/build caches remain untracked; provide reproduction steps.

Before submission, start a fresh reviewer subagent with this prompt, exact base/head, actual
code/diff and evidence. Review host lifecycle, state isolation, callback/thread ownership,
editor-independent processing, compatibility claims and adherence to file boundaries.
Fix in-scope findings and obtain a final recheck. Keep reviewer findings and your responses
distinct. If independent review is unavailable, mark the gap and leave the PR draft.

Return:
- Exact base, tested, reviewed and submitted commits; branch and draft PR.
- Acceptance matrix: observed pass/fail/unverified, evidence paths and environment.
- What the proof establishes, what it does not, and concrete D06/D07 integration requirements.
- Chosen framework/version/licenses and alternatives rejected with reasons.
- Suggested minimal production interfaces, without implementing them.
- Reviewer coverage, findings/dispositions and final recheck.
- Owner checklist and any blocked-environment resume instructions.
- Confirmation that only your two owned directories changed.

Keep the draft PR separate. Do not integrate D05 while it is active or ask that agent to
change scope. After D05/D06, the overseer will reconcile interfaces and decide which prototype
work belongs in D07. Do not merge, approve owner checks or proceed beyond this proof.
