# Kabl audiovisual direction: product and architecture audit

Part of the AV investigation package: [decisions](KABL_AV_DECISIONS_AND_OPEN_QUESTIONS.md) ·
[roadmap](KABL_AV_ROADMAP_AND_SCOPE_GATES.md) · [mockup brief](KABL_AV_MOCKUP_BRIEF.md).

Date 2026-11-19. Investigation only: no production code, format or dependency was changed.
Statements are labelled **[verified]** (read in source or run), **[doc]** (stated by a repo document, not
re-checked), **[not found]** (searched for, absent) or **[inference]**.

## 0. Summary (answers to the brief's §12 format)

1. **What Kabl is today.** A Linux-first modular synthesizer in Rust: a 53-kind built-in module catalogue
   (plus wasm "code modules"), an egui/eframe rack with three scenes (Rack, Perform, Sounds), functional
   cables, composites, macros, a factory bank, a standalone app (cpal + midir) and a CLAP instrument plugin.
   Audio is processed in fixed 64-sample blocks on a graph compiled off the audio thread and swapped with a
   crossfade. There is **no** visual runtime, no video, no node view, and no non-audio/non-scalar port type.
2. **Distance from the AV vision.** The audio core is sound and unusually well-fenced (RT discipline is
   tested). The AV vision is **far** at the data-model level (ports are one closed `f32`-signal enum; no
   events, no structured values, no domains) and **moderately far** at the UI level (one rack projection; no
   second projection; view position is stored in patch state). It is **close** in two areas: the
   audio-to-UI tap infrastructure (probe/face taps) is exactly the shape of a safe audio-to-visual bridge, and
   the code-module sandbox already shows how to contain untrusted code.
3. **Five biggest gaps**, by dependency: (a) V1.0 itself is not finished (the roadmap's WOW phases, owner
   acceptance of D01–D12, release hygiene are open); (b) the port/signal model has no event, structured or
   resource kinds and no domain concept; (c) no stable semantic/presentation split (`ModuleState.pos` is
   semantic state; composites carry `pos` and `panel`); (d) no visual runtime or render surface and no clock
   relationship between musical time and frame time; (e) module identity is audio-only: `Module` is one trait
   with one `process(io)`.
4. **Owner-only decisions.** See the decision register; the load-bearing ones are D-01 (is V1.1 generated-only),
   D-02 (is the node view a separate projection editable on day one), D-03 (are view positions project data),
   D-04 (visual runtime hosting: in-process thread vs sidecar process), D-05 (plugin visual boundary).
5. **Safe to mock up now.** Rack + node projections of one audio patch, the module browser and inspector,
   Perform, the audio-side signal inspection, a generated-visual output preview and fullscreen output **as
   labelled hypotheses**. **Misleading to mock up as real:** any event/structured port, a video source, a
   shared-state module with visual ports, DAW-hosted visuals, a user module authored across domains.
6. **Next action.** Finish V1.0 per the WOW roadmap, and run spikes S1 and S2 (see roadmap) in a throwaway
   worktree only after the owner confirms D-01. Mockups can start in parallel from the mockup brief.

## 1. Repository snapshot [verified]

| Item | Value |
|---|---|
| Checkout | `/secondary/Programming/Github/kabl` |
| Root branch | `master`, **stale by CLAUDE.md rule**; untracked owner docs, `.ai/`, `.worktrees/`, `kabl-ui-handover.zip` |
| Reviewed tree | worktree `.claude/worktrees/av-audit`, branch `worktree-av-audit`, base `4f4c34e9` (merge of PR #165 `claude/macros-pad`) |
| Root HEAD at session start | `1bdb61e0` (merge PR #138) |
| Commits | 1274 on the reviewed branch; no tags |
| Agent rules | `CLAUDE.md` (untracked, root only): Caveman/Ponytail style, 50% context budget, disk, audio sink `kabl_agents`, never merge. `CONTRIBUTING.md`. No `AGENTS.md`. |
| Working tree untouched | The root checkout was only read. Untracked root docs read: `ROADMAP-WOW-2026-11-13.md`, `OVERSEER-PENDING-TASKS.md`. |

Contradiction: the root checkout's HEAD (`1bdb61e0`) is older than the worktree base (`4f4c34e9`). I used the newer tree.

Workspace crates (line counts from `find … | cat | wc -l`): `core` 3.5k, `cables` 0.6k, `modules` 31.8k,
`engine` 21.6k, `standalone` 2.5k, `ui` 69.3k, `clap` 5.2k, `cargo-kabl` 1.8k; plus `sdk/` (0BSD module SDK),
`tools/` (non-default members) and `vendor/nice-plug` (ISC plugin framework). Docs are per-topic folders under `docs/`.

## 2. Baseline hypotheses from the brief, checked

| Brief §6 hypothesis | Status | Evidence and consequence |
|---|---|---|
| Crates `core, cables, modules, engine, standalone, ui, clap` | Verified, plus `cargo-kabl` | `crates/`. |
| egui/eframe, glow | Verified | `crates/ui/Cargo.toml:12-14` (`egui 0.35`, `eframe` with `glow`). `Cargo.lock` also lists `egui-wgpu`/`wgpu`, but the ui crate enables only glow. Rendering is egui immediate-mode; no custom GL scene. |
| CLAP plugin; isolated REAPER prototype | Partially: CLAP verified; prototype slated for deletion | `crates/clap/src/lib.rs`. `prototypes/reaper-clap` is in the ablation list of `docs/ROADMAP-WOW-2026-11-13.md` Phase 1. |
| UI / control / audio-callback split | Verified | `crates/ui/src/control.rs:71` holds an `rtrb::Producer<ToAudio>`; `engine/src/runtime.rs:54` `ToAudio`; one FIFO for graphs, values, commands, transport (module doc, lines 1-15). |
| Compile on control thread, graph replace with crossfade | Verified | `engine/src/swap.rs` (`CROSSFADE_MS = 15`, `basedrop::Owned` deferred drop), `engine/src/compile.rs:937-976`. |
| Block 64 | Verified | `engine/src/graph.rs:8` `BLOCK = 64`; adapter `timeline.rs` maps any host buffer to 64-sample blocks at fixed `LATENCY = BLOCK`. |
| Module trait, registry, composites, wasm host | Verified, wasm more complete than the brief says | `modules/src/module.rs:124` trait `Module`; `registry.rs:177` `ALL_INFOS: [&ModuleInfo; 53]`; `modules/src/code/` has abi, budget, wasmi_engine (wasmi `=2.0.0`), package, install; `sdk/SDK.md`, `cargo-kabl`. D12 (package/SDK) is "mid-way" per the roadmap. |
| `PortType` closed enum, `f32` cables, no MIDI/events | Verified | `modules/src/info.rs:31-40`: `Audio, Cv, UnipolarCv, Gate, Pitch`. Metadata only: "`process()` doesn't branch on it" (`info.rs` doc). Ports declare `width` 1 or 2. |
| Compiler: flattening, buffer reuse, cycles, polyphony | Verified | `compile.rs` doc: cycles get an implicit one-block delay; per-voice compile; delay memory carried across recompiles. |
| `.kabl`: JSON-lines op log + zstd, schema 8 | Verified | `core/src/format.rs:22` `CURRENT_SCHEMA_VERSION = 8`; zstd level 19 with embedded dictionary `KABL_DICT_1`. Older schemas are **rejected** ("not supported", `format.rs:99-107`); the roadmap deletes migration code. |
| Module faces drawn by UI; scopes limited | Verified | `ui/src/rack/*`, `displays.rs`, `fx_displays.rs`; live displays through `engine/src/facetap.rs` and `probe.rs`. |
| CLAP stereo out, stereo aux in, MIDI CC, host clock, macro slots | Verified (clock/macros not re-read) | `clap/src/lib.rs:1150-1156`: stereo output, one 2-channel aux input, `MidiConfig::MidiCCs`. Host clock/macros per `docs/host-production`. |

Documentation contradictions to resolve (code wins): `README.md` says "24 kinds"; `ROADMAP-WOW` says 51; the
registry holds 53 `ModuleInfo`s. `docs/PRODUCT-PLAN.md` still says `crates/clap/src/lib.rs` is a stub and the
module registry is static; both are obsolete (dated 2026-09-25). `STATUS.md` has ~980 lines of dated batch
records and is hard to read as a current status.

## 3. What a user can do today

Verified from README, `ui/src/lib.rs` (`enum Scene { Rack, Perform, Sounds }`, line 1527) and the roadmap:

- Standalone `kabl-ui`: Sounds browser (factory bank in `patches/`, user sounds under `~/.local/share/kabl/sounds`), Rack
  editor (add module, patch cable, edit params, undo, live graph swap), Perform view (scenes, macros, banks), drawer routing
  UI, pattern/sequence/drum-grid/wavetable editors, recording, light/dark/graphite themes (theme setting still planned).
- Functional cables carry step patterns, probability, A/B morph and glide locked to the patch clock (`cables` crate).
- Composites with exposed ports/controls and an optional portable panel (`core/src/composite.rs`, `panel.rs`), nested via `parent`.
- Code modules: sandboxed wasm DSP behind the ordinary `Module` trait, `.kablmod` packages, `cargo kabl`. Preview-grade ABI
  ("may change until 1.0", `sdk/SDK.md`).
- MIDI in, transport, keyboard (note, sustain, bend), CLAP instrument in REAPER, host clock.
- Diagnostics: stream-error counters, selected-signal probe, face taps, `docs/signal-inspection`. "Why is there no sound?"
  is partly addressed by signal inspection; no dedicated silence-diagnosis flow was found [inference: not exhaustively searched].

Not present [not found]: node view, event/MIDI-typed cables, visual modules, any video or render surface, a second output
window, Windows/macOS support (Linux is the 1.0 platform per README), embedded asset store other than wavetables
(`PatchState.tables`).

I did not launch the app or capture screenshots in this session. Reason: the shared background job must not touch audio output
(CLAUDE.md Audio rule) and the evidence needed here is architectural. UX statements in §7 rest on the code and on the repo's own
design docs (`docs/design/`, `docs/ROADMAP-WOW-2026-11-13.md`), not on a fresh hands-on pass.

## 4. Architecture end to end

```
 UI thread (egui)              control thread(s)                audio callback (cpal / CLAP process)
 ─────────────────             ─────────────────                ────────────────────────────────────
 edit → Op ──► PatchLog/PatchState ─► runtime_changes(old,new)
   (core: ids, cables,              │   Runtime param?  yes ──► ParamSet{rev} ─┐
    composites, pos)                │   no: compile()   ──► CompiledGraph ─────┤  rtrb SPSC `ToAudio`
                                    │   (alloc, registry::create)              ▼  (graphs+values+cmds+transport, FIFO)
 save: format.rs JSONL+zstd         │                                    PatchEngine::process_block(64)
                                    │                                       swap.rs: 15 ms equal-power crossfade,
 UI reads ◄── Feedback atomics ◄────┼───────────────────────────────────    old graph dropped by basedrop collector
 UI reads ◄── probe.rs / facetap.rs reports (fixed-size Copy, no alloc)     voices/keyboard.rs, timeline.rs adapter
                                    │                                       code modules: wasmi instance per voice,
 midir callback ──► keyboard events ─┴────────────────────────────────►      fuel/memory budget, fault disables module
```

Key facts, with evidence:

- **Data model.** `PatchState { composites, tables, modules: BTreeMap<ModuleId, ModuleState>, cables: BTreeMap<CableId, CableState>, labels }`
  (`core/src/state.rs`). `ModuleState { kind: String, pos: Vec2, params: BTreeMap<String,f32> }`. `PortRef` names a module port.
  Ids are stable newtypes assigned by the op log. Every edit is an `Op` (`core/src/op.rs`: `AddModule, Connect, SetParam, MoveModule, Group, SetComposite, SetTable …`),
  so the log is the history and undo substrate.
- **Position is semantic.** `ModuleState.pos` and `Op::MoveModule` live in the patch; `Composite` also has `pos` and `panel`. A second
  projection would need either its own layout store or a decision that view layout is project data (D-03).
- **Module interface.** `ModuleInfo { kind, category, rate (Voice|Global), ports: &[PortInfo{name, port_type, direction, width}], params, … }` and
  `trait Module { prepare, process(&mut ProcessIo), reset, save_state, load_state, carry_from, view }`. One instance, one `process`,
  audio-thread only. `view()` is the existing "extra outputs for the UI" hook.
- **Runtime control.** `runtime.rs`: params classed `Runtime` (applied in place, revision-tagged so a stale value cannot override a newer graph) or `Structural` (compile).
- **RT discipline.** README claims no allocation or locking in the callback, enforced by `assert_no_alloc` tests. I did not re-derive this beyond
  reading `swap.rs`, `probe.rs`, `facetap.rs` module docs ("the audio thread allocates, formats and waits for nothing"). `docs/host-production`
  holds a callback audit and notes limits: dense two-editor sessions showed backend errors, plugin latency 64 frames, off-audio producer
  backlog can grow if a host rejects delivery.
- **Plugin vs standalone.** Standalone is not a wrapper over the plugin: separate entry points (`standalone/src`, `ui/src/main.rs`,
  `clap/src/lib.rs`) over the shared `engine`/`modules`; both use the shared UI crate (`EguiEditor<RackApp>` in the plugin).

## 5. Audiovisual fit (brief §7D)

Classes: **S** supported · **B** bounded extension · **R** significant architectural change · **U** unknown, needs spike · **N** not yet.

| # | Capability | Class | Evidence / note |
|---|---|---|---|
| 1 | One instance exposes audio and non-audio outputs | B | `Module::view` already exports a non-audio snapshot (`ModuleView`) from audio-thread state. A formal "telemetry/event output" is a trait extension, not a rewrite. |
| 2 | Discrete events, timestamped streams | R | No event port. `Gate` is a level signal. Sample-offset keyboard events exist (`timeline.rs`, `keyboard.rs` `KeyEvent`) but only into the keyboard, not between modules. Detecting gate edges from the signal is possible today (probe counts rises). |
| 3 | Scalar control across domains | B | Param values already cross by revisioned SPSC; the reverse direction (audio→UI) is done by atomics/reports. A bounded audio→visual scalar snapshot reuses `Feedback`/`facetap`. |
| 4 | Structured values (vector, colour, transform) | R | `PortType` is closed; ports are `f32` slots. Adding kinds touches `PortInfo`, compiler buffer allocation, cable UI, persistence. |
| 5 | Visual modules consuming outputs, producing resources | R | No resource type, no visual module concept, no scheduler. |
| 6 | Frame scheduler separate from audio | R | Nothing exists. Cleanest option is a separate thread or process with its own loop. |
| 7 | Safe audio→visual transfer | B | The pattern exists and is tested: fixed-size `Copy` reports, `rtrb`, atomics. Needs a ring of timestamped events and a bounded-overflow policy. |
| 8 | One semantic graph, two projections | U/B | Graph itself is domain-neutral enough (ids, kinds, port refs). Layout in semantic state is the snag (D-03). Spike S3. |
| 9 | Domain-specific ports per module | R | Requires a domain tag on `PortInfo` and per-domain compile. |
| 10 | Composites incl. nested | S | `Composite.parent`, exposures, `Definition`; D09/D10 merged. Not cross-domain. |
| 11 | User-authored cross-domain modules | N | Code modules are audio/control only, ABI preview. Do not extend ABI before a first-party cross-domain module exists. |
| 12 | Visual output window / fullscreen / multi-display | U | eframe single viewport. egui supports extra viewports [inference, not tried]. Spike S4. |
| 13 | Generated content vs video decode/processing | Generated: U. Video: N | No decoder or texture upload path. Video adds codecs, licensing, asset persistence. |
| 14 | Save/reopen AV patch with resources | R | Format has embedded `tables` only; no asset model. Schema bump needed; older schemas are already hard-rejected, so the 1.0 compatibility promise is the real constraint. |
| 15 | Plugin audio-only; plugin visuals | S for audio / U for visuals | CLAP has GUI extension but no video output. Only a plugin editor window could show visuals; unverified in REAPER. |

## 6. Findings per brief §5 (condensed)

- **RT isolation.** The pattern for crossing out of the audio thread is established; any AV bridge must be built from it and from nothing heavier.
  A visual failure cannot take the audio callback down if the visual runtime is a separate thread with its own panic boundary; `panic = "abort"` in
  `[profile.release]` (`Cargo.toml`) means a panic in **any** in-process thread aborts the whole app. This is a concrete argument for a sidecar
  process or `catch_unwind`-free code paths for the visual runtime (D-04).
- **Graph semantics.** `PortType` is documented as "metadata, not engine behaviour"; the compiler treats every cable as `f32` blocks. That keeps audio
  fast and makes the domain split a metadata-plus-compile problem, but it also means "event" cannot be represented honestly as a `Gate` level.
- **Timing.** Engine time is the 64-sample grid with fixed latency; there is a patch clock (`clock` module, transport). Visual frame time has no
  relation yet. Recommended reference: sample-position timestamps stamped at the block they occur, converted by the visual runtime using a published
  `(sample_pos, monotonic_ns)` pair per block. [recommendation, spike S2]
- **Extension.** Code modules prove a sandbox with budgets and fault disabling. They are the right base for later cross-domain user modules, after
  first-party cross-domain modules settle the interface.
- **Persistence.** No visual state, assets or view layout store; op log makes additions cheap, but 1.0 compatibility policy decides how much.
- **Renderer.** egui+glow is adequate for the editor; a general visual engine on it is unproven. egui offers custom paint callbacks into glow
  [inference]. Not recommended to rewrite the renderer; evaluate S4.

## 7. UX / information architecture

From code and design docs (hands-on pass not performed):

- Navigation: three scenes (Rack, Perform, Sounds) with a drawer for routing; sound-first onboarding is an explicit principle
  (`PRODUCT-PLAN.md` §1) and the "first 60 seconds" phase is open (`ROADMAP-WOW` Phase 6: no audible preview in Sounds, no first-run).
- Discovery: `ModuleInfo.explain`, recipes (`ui/src/recipes.rs`), `explain.rs`, `help.rs`. The roadmap lists missing sound preview and onboarding.
- Legibility of signal: cables are coloured by type (colours hard-coded in `ui/src/lib.rs`, Phase 3 moves them into `Style`); liveness (signal-reactive
  cables, jack LEDs) is planned and unbuilt. These are precisely the affordances a multi-domain view will lean on.
- Performance: Perform view with scenes/macros/banks exists; owner acceptance is pending.
- Space: the editor is a single window; embedded visual preview plus rack plus inspector will need a docked/resizable panel. The current drawer
  and Perform layout are fixed-position per the roadmap; the budget question for 1280x800 is open (mockup brief, Q5).

## 8. Gaps and dependency map

```
V1.0 finish (WOW 1-4, acceptance, hygiene) ──┐
semantic/presentation split (D-03) ──────────┼─► node projection (S3)
port metadata + domain tag (D-06) ───────────┼─► event + scalar bridge (S1, S2) ─► generated visual runtime (S4) ─► V1.1
visual runtime hosting (D-04) ───────────────┘                                    └► multi-domain module (S5)
asset/persistence policy (D-09) ───────────────────────────────────────────────────► save/load AV patch
cross-domain first-party modules ─► composites across domains ─► code-module ABI v1 (V2)
```

## 9. Risks and unknowns

1. `panic = "abort"` plus a GPU/driver fault in an in-process visual thread kills audio. Mitigate by design choice D-04 and measured test.
2. Linux-only 1.0; the brief asks for portability, and GPU/video paths differ per platform.
3. egui `ui/src/lib.rs` is very large and the roadmap itself defers splitting it; a second projection is cheaper after that refactor.
4. Schema is 8 with no migration promised below it; any AV schema bump must settle compatibility policy before 1.0 ships, not after.
5. Dense sessions already approach the callback budget (`docs/host-production`: worst process 5.49 ms vs 5.33 ms JACK period for two open editors).
   Adding visual work in-process competes for the same cores.
6. Plugin visual output is unverified.

## 10. Evidence log

- Read: `crates/{core,modules,engine,clap}/src` headers and the symbols cited above; `README.md`; `docs/PRODUCT-PLAN.md`; `docs/ROADMAP-WOW-2026-11-13.md`;
  `docs/OVERSEER-PENDING-TASKS.md`; `docs/STATUS.md` head; `sdk/SDK.md` head.
- Commands: `git log/status/branch`; `rg`-style greps for video/shader/texture (hits only egui UI code in `ui/src/{kit,lib,panels,cable_life}.rs` and a
  noise module; none implement a visual runtime) and for "node view" (no hits).
- Tests: `cargo test --workspace --no-fail-fast` on base `4f4c34e9`: 1311 passed, 0 failed, 42 ignored.
