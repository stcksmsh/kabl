# Kabl AV: decisions and open questions

Part of the AV package: [audit](KABL_AV_PRODUCT_AND_ARCHITECTURE_AUDIT.md) · [roadmap](KABL_AV_ROADMAP_AND_SCOPE_GATES.md) · [mockup brief](KABL_AV_MOCKUP_BRIEF.md).

"Rec." is a recommendation, not an owner decision. "Owner?" = Yes when only Kosta can decide. Confidence: H/M/L.
Evidence paths are in the audit.

## Confirmed by owner (do not reopen)

Release order V1.0 → 1.0.x audio-only → V1.1 first visuals → 1.2+ → V2.0; two views of one patch; standalone-first;
modules may be single-domain; no GPU/video work on the audio callback. Source: the brief §2.

## Product and release scope

| ID | Question | Evidence | Options | Rec. | Consequence | Owner? |
|---|---|---|---|---|---|---|
| D-01 | What does "video" mean in V1.1? | No decoder, no texture path, no asset model | A generated visuals only; B A + video playback; C A + camera/video input | **A** (M-H). Playback adds codecs, licence, assets, sync; input adds device permissions. Revisit at 1.2. | V1.1 stays one vertical slice; no asset schema in 1.1 beyond presets | Yes |
| D-07 | Is V1.0 shipping before any AV format change? | Schema 8; old schemas rejected; roadmap puts hygiene last | A yes, AV schema work starts after 1.0 tag; B reserve fields now | **A** unless S1 shows a needed hook (M) | 1.0 patches must open in 1.1 | Yes |
| D-08 | What is V1.0's completion checklist? | `ROADMAP-WOW` phases 1-7 and owner acceptance of D01-D12 | Use roadmap as is; add AV-readiness items | Roadmap as is; AV adds **zero** items to 1.0 except what S1/S3 prove unavoidable (M) | Protects the audio-first rule | Confirm |

## Graph semantics and module model

| ID | Question | Evidence | Options | Rec. | Consequence | Owner? |
|---|---|---|---|---|---|---|
| D-03 | Is node/rack layout project data? | `ModuleState.pos`, `Op::MoveModule`, `Composite.pos/panel` hold layout in `PatchState` | A keep `pos` as rack layout, add `views: {rack, node}` side-table outside semantic hash; B move all layout to a per-view store; C one shared layout | **A then B** (M). Add a node-view layout side-table keyed by `ModuleId`; do not touch `pos` for 1.0. | Undo still works through ops; semantic equality ignores layout | Partly |
| D-06 | How do modules declare domains? | `PortInfo{name, port_type, direction, width}`; `PortType` closed, metadata only | A add `domain` + new `PortType` variants; B separate `InterfaceInfo` per domain next to `ports`; C capability flags only | **B** (M). Keeps the audio port table and compiler path unchanged; adds a second, typed table for event/value/visual interfaces. | Old modules have an empty second table; no compile change for audio | No |
| D-10 | One instance with domain adapters, or parent/child sub-instances? | `Module` is one audio-thread object; code modules instantiate per voice | A one semantic instance (id, params, state) + optional per-domain executors (audio exists, visual optional); B separate instances linked by shared params | **A** (M). Shared *parameters and identity* only; no shared mutable runtime memory. | Visual executor reads published snapshots; absence of it leaves the module valid | Confirm |
| D-11 | Cross-domain connection semantics | Cables are f32 blocks; rate classes: Voice/Global; cycles get 1-block delay | Explicit adapters: audio→control (aggregate), control→event (threshold/edge), event→visual (trigger), with a visible converter on the cable | **Explicit conversion, never implicit** (M-H) | UI shows converter chip; invalid pairs refuse to connect | No |
| D-12 | Composite public interface across domains | Composite exposes `ports` and `controls` (audio PortRef) | Extend `Exposure` with domain tag | Defer to V1.2 (L) | No composite change in 1.1 | No |

## Signal types

| ID | Question | Evidence | Options | Rec. | Consequence | Owner? |
|---|---|---|---|---|---|---|
| D-13 | Type taxonomy for 1.1 | Today: Audio, Cv, UnipolarCv, Gate, Pitch | Add only: **Event** (timestamped, bounded) and **Scalar-for-visual** (value + rate); **Frame/Texture** is visual-domain-only and never crosses the bridge | Event + sampled scalar only (M-H). Colour/vector as groups of scalars in 1.1, structured types in 1.2+ | Keeps bridge to POD fixed-size records | No |
| D-14 | Is a Gate an event? | `Gate` = level ≥ 0.5 | Keep Gate as level; derive edge events at the bridge | **Derive** (H). Do not call levels events (brief §15) | Existing patches unchanged | No |

## Runtime and synchronisation

| ID | Question | Evidence | Options | Rec. | Consequence | Owner? |
|---|---|---|---|---|---|---|
| D-04 | Where does the visual runtime run? | `panic = "abort"` release profile; single eframe viewport; dense sessions near budget | A in-process thread; B separate process (sidecar) over shared memory/socket; C in the UI thread | **A for 1.1 prototype, decision gate after S4** (L-M): sidecar gives crash and driver isolation, in-process is simpler and allows plugin editor use | If B: a stable bridge protocol is also the Sinteza integration seam | Yes |
| D-15 | Bridge mechanism | `rtrb` SPSC, atomics, fixed-size `Copy` reports (probe/facetap), `basedrop` | A SPSC ring of timestamped events + per-block scalar snapshot (latest-wins); B triple buffer for snapshots only; C MPMC | **A** (M). Overflow: drop oldest scalar, drop-newest + counter for events; never block | Counters surface in the diagnostics panel | No |
| D-16 | Timing reference for visual changes | Engine 64-sample grid, fixed `LATENCY`; patch clock; no frame clock | Stamp events with absolute sample position; publish `(sample_pos, monotonic_ns)` per block; visual runtime converts and renders at its own rate with a configurable look-ahead | **Sample time on the wire, musical time derived, frame time local** (M) | Late events apply on the next frame, never retroactively; jitter measured in S2 | No |
| D-17 | Failure isolation | See D-04 | Watchdog on visual heartbeat; audio never waits for visual ack | Required regardless of D-04 (H) | Visual stall shows a banner; audio continues | No |

## Editor views

| ID | Question | Evidence | Options | Rec. | Consequence | Owner? |
|---|---|---|---|---|---|---|
| D-02 | Is the node view editable on day one of V1.1? | Only one projection exists; `ui/src/lib.rs` 6.1k lines, split deferred | A read-only mirror first (select, inspect, navigate), B full edit | **A in 1.1, B in 1.2** (M) | Cuts the cross-view edit-consistency test burden in half | Yes |
| D-18 | View switch scope | Scene enum Rack/Perform/Sounds | Global toggle; per-selection "show in node view" | Global toggle with selection preserved (L: needs mockup test) | Mockup 2 shows this | Hypothesis |
| D-19 | Hidden connections | Rack hides cables by design (decisions on drawer/routing) | Auto-collapse event/visual edges in rack; show as a badge | Badge + expand (L) | | Hypothesis |

## Persistence

| ID | Question | Evidence | Options | Rec. | Consequence | Owner? |
|---|---|---|---|---|---|---|
| D-09 | Assets and AV state in `.kabl` | Embedded `tables` only; zstd dictionary container | Embed small assets, link large ones with hash + missing-asset placeholder | Generated-only 1.1 needs none; plan "embed ≤ N MB, link above" for 1.2 (L) | Missing asset ⇒ module loads disabled with message | Later |
| D-20 | Missing-module behaviour | Code modules fault-disable; kind is a `String` | Preserve unknown kinds as opaque, inert nodes | Keep (check current behaviour in S3) (L) | Required for 1.0 patches opened in older builds | No |

## Extension system

| ID | Question | Evidence | Options | Rec. | Consequence | Owner? |
|---|---|---|---|---|---|---|
| D-21 | When may users author cross-domain modules? | wasmi sandbox, ABI preview (`docs/sdk/ABI.md`), D12 unfinished | After ≥ 2 first-party cross-domain modules and an ABI v1 freeze | V2.0 (M-H). Visual code never in the audio sandbox path; separate budget and scope | | Yes |

## Platform and output

| ID | Question | Evidence | Options | Rec. | Consequence | Owner? |
|---|---|---|---|---|---|---|
| D-05 | Plugin visual boundary | CLAP has no video output; GUI via egui editor; stereo out + stereo aux in | Standalone-only visual output in 1.1; plugin editor shows an embedded preview only after S4 on REAPER | **Standalone-only first** (M) | Plugin patches with visual modules load audio part and show "visuals need standalone" | Yes |
| D-22 | Output surfaces | eframe single viewport | Embedded panel + one fullscreen window on chosen display; capture/record later | Same (M) | | Confirm |
| D-23 | Platform order | Linux only for 1.0 | Linux; then Windows/macOS | Not an AV decision; flag that GL/driver behaviour differs | | Yes |
| D-24 | Sinteza/Afekt boundary | No code link found | Protocol seam after D-04; no dependency | Out of scope for 1.1 | | No |

## Brief §13 questions: where answered

1 → D-08, roadmap V1.0. 2-3 → D-01. 4 → mockup brief §1. 5 → mockup brief §6. 6-7 → audit §6, D-03. 8-9 → D-06, D-10. 10 → D-11, D-13.
11 → D-12. 12-13 → D-15, S1. 14-15 → D-16, D-15. 16 → D-04, D-17. 17 → audit §6 (adequate for editor, unproven for engine; S4). 18-22 → D-02, D-03,
D-18, mockup. 23 → D-21, roadmap. 24 → D-07. 25 → D-09. 26 → D-05. 27 → D-04, D-23 (unknown; needs S4).
