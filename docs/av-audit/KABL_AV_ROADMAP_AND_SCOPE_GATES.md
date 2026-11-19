# Kabl AV: roadmap and scope gates

Part of the AV package: [audit](KABL_AV_PRODUCT_AND_ARCHITECTURE_AUDIT.md) · [decisions](KABL_AV_DECISIONS_AND_OPEN_QUESTIONS.md) · [mockup brief](KABL_AV_MOCKUP_BRIEF.md).

Release order is fixed by the owner: **V1.0 → 1.0.x (audio-only) → V1.1 → 1.2+ → V2.0**. Everything below inside V1.1+ is a proposal.

## V1.0: complete standalone audio instrument

Source of truth: `docs/ROADMAP-WOW-2026-11-13.md` (Kosta's decisions of 11-13). Order: ablate → finish D12 → Living rack → Motion and feel → Sound content →
First 60 seconds → release hygiene → one owner-acceptance session for D01-D12. This package adds no AV scope to 1.0.

Candidate 1.0 checklist, validated against the repo:

| Requirement (brief §8) | State | Gap |
|---|---|---|
| Launch, device config, output | Standalone exists (cpal/midir, stream-error counters) | Stall recovery observed once on cloud runs; owner acceptance pending |
| Module library, editing workflow | 53 kinds, rack editor, undo, composites | README count stale |
| Save/load/duplicate/rename/organise | Sounds browser, user library under `~/.local/share/kabl/sounds` | Owner acceptance pending |
| Presets/examples | Factory bank 81 of ~100 | Phase 5 |
| Sequencing/modulation/effects | Present | — |
| Performance controls | Perform view, macros, scenes | Macro redesign (Phase 5) |
| Monitoring/diagnostics | probe, face taps, signal inspection | Cables not yet signal-reactive (Phase 3); no "why silent" flow |
| Stable format, recovery | Schema 8; migration to be deleted; older files rejected | Decide compatibility policy before 1.0 tag (D-07) |
| Packaging, release process | `packaging/linux/package.sh`, tarball | Release hygiene phase |
| Plugin boundary | CLAP stereo + aux in, MIDI CC | Documented limits; dense-editor backend errors unresolved |

**Gate G1 (1.0 → 1.0.x):** roadmap Phases 1-7 done, owner acceptance recorded, CI green, schema policy written.

## V1.0.x: audio-only

Fixes, performance, usability. Rule: no visual module, port type or schema field lands here. Allowed AV-adjacent work: none, except bug fixes found by spikes.

## Pre-V1.1 spikes (throwaway worktrees, not shipped)

| ID | Proves | Setup | Pass | Fail → consequence | Scope |
|---|---|---|---|---|---|
| S1 | Audio→visual bridge is safe and measurable | Test-only harness: osc + ADSR run in `PatchEngine`; publish per-block scalar snapshot + gate edge events through `rtrb` to a consumer thread | Zero audio-thread allocations (`assert_no_alloc`), no xrun delta vs baseline over 10 min, event loss counted and = 0 below capacity, overflow policy exercised | Any alloc/lock ⇒ redesign bridge before D-15 | 2-3 days |
| S2 | Timing: sample time ↔ frame time | Consumer renders a headless frame loop at 60 Hz; log `(sample_pos, monotonic)` | p99 event→frame latency and jitter reported; stable under 2x CPU load | Jitter > ~1 frame at p99 ⇒ add look-ahead/render-ahead and revisit D-16 | 1-2 days |
| S3 | One patch, two projections | Read a factory `.kabl`; build a read-only node layout with its own layout store; round-trip save | Same `ModuleId`/`CableId`/`Op` log; layout store separate; old file opens | Layout leaks into semantic equality ⇒ D-03 option B first | 3-4 days |
| S4 | Visual runtime host + output window | egui custom GL paint (in-process) vs second eframe viewport vs sidecar; draw a shader driven by S1 values | 60 fps at the target resolution on the owner's laptop while audio runs under load; fullscreen on second display; a forced GPU/panic fault leaves audio running | Panic aborts audio (`panic = "abort"`) ⇒ choose sidecar (D-04 B) | 3-5 days |
| S5 | Cross-domain module identity | One ADSR: audio out, gate-edge events, visual-readable scalar, one parameter set | Single parameter edit visible in both executors, no duplicate state | Needs two copies of state ⇒ revisit D-10 | 2 days |

Minimum before mockups: none (mockups are hypothesis-labelled). Minimum before committing V1.1 scope: S1, S2, S4.

## V1.1: first visual slice (proposal)

- **Outcome:** a musician connects an envelope/oscillator/sequencer-derived value or event to a parameter of a generated visual and sees it immediately, saves it, and plays it fullscreen.
- **Scope:** one visual output surface (embedded + one fullscreen window); 4-6 generators/effects (e.g. field, shape, feedback, colour-grade) plus one output module; a fixed
  list of sources: scalar (envelope level, LFO, oscillator phase/frequency), edge events (gate on/off, step, bar), patch clock; a visible bridge-health readout; node view
  **read-only mirror** (D-02) or the rack with domain badges; save/load of AV patches (schema bump).
- **Depends on:** G1, D-01, D-04, D-06, D-13, D-15, D-16, S1/S2/S4/S5 passing.
- **Acceptance:** (1) a 1.0 patch opens unchanged and sounds identical (golden renders unchanged); (2) with visuals on, xrun counter unchanged vs baseline in the existing
  perf gate; (3) killing/stalling the visual runtime leaves audio running and shows a banner; (4) first-visual path ≤ N clicks (set after mockup user test); (5) save/reload
  keeps visual parameters and connections; (6) plugin: loads, audio works, visual modules show a clear "standalone only" state.
- **Evidence:** automated bridge tests, soak test, recorded clips, owner session.
- **Non-goals:** video playback/input, general compositing, user visual modules, multi-output, recording, node-view editing, plugin visuals.

## V1.2 onward (vertical slices, each independently testable)

1. Node view editing + connection parity tests. 2. Video playback with assets (after D-09). 3. More effects/compositing, scenes/snapshots for visuals.
4. Beat/frame sync improvements and diagnostics (event inspector). 5. Cross-domain composites (D-12). 6. Multi-output, capture/record.
7. Plugin editor preview if S4-on-REAPER passes. 8. Windows/macOS output paths.

## V2.0 (outcomes, not a design)

First-class audio, control, event and visual domains in one semantic patch; two mature projections; cross-domain modules with shared semantics; robust output
pipeline; user cross-domain modules on a frozen ABI (D-21); complete persistence and portability; defined DAW boundary.

## Verification of this investigation

Commands run in the worktree `av-audit` (base `4f4c34e9`): `cargo test --workspace --no-fail-fast` Result: 1311 passed, 0 failed, 42 ignored (summed over all test binaries; no failures or panics in the log).
No code was modified.
