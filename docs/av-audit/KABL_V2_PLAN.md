# Kabl V2.0 plan: from the AV proof to a full audiovisual instrument

Date 2026-11-19. Status: **proposal**. Release order V1.0 → 1.0.x (audio-only) → V1.1 → 1.2+ → V2.0 is owner-fixed;
everything else here is a recommendation for Kosta to accept, change or reject.

Builds on: [audit](KABL_AV_PRODUCT_AND_ARCHITECTURE_AUDIT.md) · [decisions](KABL_AV_DECISIONS_AND_OPEN_QUESTIONS.md) ·
[roadmap and gates](KABL_AV_ROADMAP_AND_SCOPE_GATES.md) · [architecture and competition](KABL_AV_ARCHITECTURE_AND_COMPETITION_PLAN.md) ·
[mockup brief](KABL_AV_MOCKUP_BRIEF.md). This document supersedes the V1.1+ sections of the roadmap where they differ.

## 1. What the proof established

`tools/examples/av_proof.rs` (commits `dacbed52`, `610b18f7`) renders a factory piece offline and *performs* it. The
patch's own cues fire on the bar, sequencer banks change, macros morph and a lead phrase plays. It then draws a 1080p60 video
whose every element follows the patch's structure. Kosta's verdict: "It works" and "It looks good". Masters:
`~/Videos/kabl-av/` (not in git).

**Confirmed by the proof**

1. *Structure-reactive is visibly different from audio-reactive.* Steps, coincidences between lanes, envelope attacks, clock ticks,
   cue changes and notes read as cause and effect. An FFT of the mix cannot show which lane fired.
2. *The audio engine needs no change to feed visuals.* Everything came out through the existing face taps (`engine/src/facetap.rs`),
   the runtime value path and the cue launcher (`kabl_ui::cues::launch`). Golden audio behaviour is untouched.
3. *Analysis belongs on the visual side.* A 40-band spectrum from a copied 2048-sample audio ring costs almost nothing there.
4. *The interesting performance surface already exists.* Cues, banks and macros made the demo; the visuals only had to listen.

**Limits the proof exposed (each becomes a requirement)**

| Finding | Evidence | Requirement |
|---|---|---|
| Tap reports arrive about every 25 ms, and edges are inferred from min/max windows | `PatchEngine` face window = probe window / 2 | **R1** Timestamped event ring: gate edges, steps, envelope stage, notes, cue landings, each with a sample position |
| 16-tap cap reached by `composition` | `MAX_FACE_TAPS = 16` | **R2** A telemetry bus sized per patch at compile time, separate from the UI face taps |
| `osc.va` exposes no waveform | only fm, fm6, wt, env, lfo implement `Module::view` | **R3** An audio ring per chosen output, copied in the callback and analysed on the visual side; `view()` stays optional |
| `midi.in` gate tap flickers | the face tap follows the loudest voice instance | **R4** Explicit polyphonic aggregation per telemetry output: any-voice gate, newest note, max level, per-voice lanes |
| Cue requests and cue *landings* differ (bar quantised) | labels were placed at request time | **R5** The engine reports when a launch lands, as an event |
| CPU frames at 1080p60 run about 5.6 fps on 20 cores | 72 s took about 400 s | **R6** GPU visual runtime (wgpu) for live use; keep the offline CPU path for golden clips and CI |
| Looks were hand-written Rust | `Visual::step` | **R7** Looks as data: shader modules plus bindings, editable without a rebuild |

## 2. Product target for V2.0

A musician opens kabl, plays a piece, and a Look already moves with it. They can see and change what drives what. A power user opens the same patch in a node view and builds their own visual chains. A shader artist drops in one file and it becomes a module.

User tiers (from the architecture plan): **Looks** (one click, auto-bound) → **bindings** (Bitwig-style source slots on any visual parameter) →
**node view** (same patch, full freedom) → **shader modules** (one file, hot reload) → **telemetry outputs** in code modules → first-party Rust.

## 3. Architecture at V2.0

```
ONE PATCH (core op log; schema N)
  modules · cables · bindings · visual nodes · looks · per-view layout (side-table, not semantic)
        │ compile                                   │ compile
        ▼                                           ▼
AUDIO DOMAIN (unchanged contract)            VISUAL DOMAIN (wgpu; own thread, then own process)
  64-sample blocks; modules may declare        frame graph: shader modules, feedback buffers,
  telemetry outputs (scalar | event | ring)    compositing, outputs (window, fullscreen, record)
        │ writes fixed-size records                  ▲ reads snapshot, drains events, analyses rings,
        ▼                                            │ maps sample time → frame time
  TELEMETRY BUS (R1–R5): per-block scalar snapshot (latest wins) · event ring with sample positions
  (drop-newest + counter) · audio rings · launch-landed events · clock pairs (sample_pos, monotonic_ns)
```

Invariants: audio never waits for visuals; visuals never write audio state; the audio-to-visual direction carries only fixed-size data;
visual-to-audio control (rare, e.g. a visual LFO driving a filter) goes through the existing runtime value path at block rate.

## 4. Milestones

Agent-time estimates assume one agent, sequential, with Kosta reviews between milestones. Calendar time depends on review latency and on
V1.0 work, which keeps priority.

### M0. Offline proof — done (2026-11-19)
The clips above; the tool stays in `tools/` as the golden-clip renderer.

### M1. Live proof (throwaway, before V1.1 is committed) — about 2 agent-weeks
- Telemetry bus prototype (R1, R2, R4 minimal) behind a feature flag in a branch; `assert_no_alloc` test; overflow counters.
- wgpu window running two of the proof's looks as WGSL shaders; fullscreen on a chosen display.
- Measure: event-to-photon p50/p99, frame pacing, xrun delta versus baseline over a 10-minute soak on Kosta's laptop.
- Kill the visual thread: audio continues. Decide in-process versus sidecar (D-04) from this.
- **Gate:** numbers in hand; Kosta plays it live with a keyboard and approves continuing.

### V1.0 → 1.0.x — audio only (unchanged)
No AV code ships. Only allowed AV-adjacent change: an internal, non-format telemetry hook if M1 proves the engine needs one, behind a flag and off by default.

### V1.1. First visual slice — about 6–8 agent-weeks
- Telemetry bus in the engine (R1–R5) with a fixed source list: envelope level and stage, sequencer step and gate, clock tick and bar,
  macro values, cue landed, note on/off, and an audio ring of the main output.
- Visual runtime (wgpu) with 6–8 shader modules ported from the proof: lattice, beams/arcs, waveform core, spectrum ring, particles,
  shockwave, feedback, palette scene.
- **Looks**: three or four factory Looks, auto-bound by role; a Look picker in Perform; an embedded preview panel; a fullscreen output window.
- Bindings v1: per-parameter source, amount and curve in an inspector (no node view yet); saved in the patch (schema bump).
- Bridge health readout (events/s, drops, latency).
- **Acceptance:** 1.0 patches open and sound byte-identical; xrun delta 0 under the perf gate with visuals on; a visual-runtime crash leaves audio
  playing; first Look visible within one click of opening a piece; save/reload keeps Looks and bindings; plugin shows "visuals need standalone".
- **Non-goals:** video files or cameras, node view editing, user shader import UI, plugin visuals, recording.

### V1.2. Node view and shader modules — about 6 agent-weeks
- Node view as a second projection of the same patch: read and edit, layout side-table (D-03), selection and undo shared with the rack.
- Shader module format: one WGSL (or ISF-style GLSL) file with a header for params and inputs; hot reload; errors shown on the node.
- Visual composites (a Look is a composite).
- Offline render from the app ("Render clip": the av_proof path, productised).

### V1.3. Performance and sync — about 4 agent-weeks
- Visual scenes per cue (cue landing switches Look or scene), macro-to-visual morphs, tempo-locked visual LFOs.
- Multi-output (two displays), output recording.
- Event inspector: trace a note from key to module to visual result.

### V1.4. Media — about 5 agent-weeks
- Video file playback as a visual source, with asset storage (embed small, link large, hash, missing-asset placeholder; D-09).
- Camera input (opt-in permission).
- Platform output check on Windows/macOS if the owner adds those platforms.

### V2.0. Full audiovisual instrument — about 6–8 agent-weeks after V1.4
- Cross-domain modules with one identity: telemetry outputs as part of a module's public interface in both views (D-06 option B, D-10 option A).
- Cross-domain composites with domain-tagged exposures (D-12).
- Code-module ABI v1 with telemetry outputs and a separate visual sandbox for shader code (D-21).
- Mature node view; Looks library and sharing; projector-grade output; documented plugin boundary (preview in the editor if REAPER tests pass).
- Stable AV patch format with migrations from every 1.x schema.

**Total from V1.0 tag to V2.0:** about 27–31 agent-weeks plus reviews. A realistic calendar range is roughly 9–14 months at the current one-agent
cadence, or less if Kosta allows a second agent on non-overlapping streams (engine/telemetry versus visual runtime/content).

## 5. Workstreams and ownership seams

| Stream | Crates | Starts | Can run parallel with |
|---|---|---|---|
| Telemetry bus (R1–R5) | `engine`, `modules` | M1 | Visual runtime |
| Visual runtime (wgpu, frame graph) | new `crates/visual` | M1 | Telemetry |
| Patch model (bindings, looks, layout side-table) | `core` | V1.1 | Content |
| Editor (inspector bindings, preview, node view) | `ui` | V1.1 / V1.2 | Runtime |
| Content (Looks, shader modules) | `patches/`, new `looks/` | V1.1 | everything; judged by clips |
| SDK (shader format, telemetry ABI) | `sdk`, `cargo-kabl` | V1.2 / V2.0 | — |

## 6. Risks

1. GPU and driver variance on Linux (Mesa, NVIDIA): M1 must run on Kosta's laptop, not only in CI.
2. `panic = "abort"` means an in-process visual panic kills audio: M1 decides whether a sidecar is required.
3. Telemetry cost in dense patches: budget it in the existing perf gate (1333 µs/block) from day one.
4. Scope creep into a VJ tool: hold the V1.1 non-goals; video waits for V1.4.
5. Format churn: one schema bump per milestone at most, with a migration test from every earlier schema.

## 7. Decisions needed from Kosta (short list)

| ID | Decision | Recommendation |
|---|---|---|
| D-01 | V1.1 is generated visuals only | Yes; video in V1.4 |
| D-02 | Node view arrives in V1.2, not V1.1 | Yes |
| D-04 | In-process versus sidecar visual runtime | Decide after the M1 kill test |
| D-05 | Plugin visuals | Standalone-only until V2.0 |
| New | Run M1 now, in parallel with V1.0 work | Yes, in a separate worktree touching only a feature-flagged branch |
| New | Telemetry bus may land behind a flag in 1.0.x (no format change) | Only if M1 shows it is needed |
