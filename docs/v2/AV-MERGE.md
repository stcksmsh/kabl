# kabl V2: merging audio and video — joint modules and containers

Date 2026-11-22. Status: proposal, not owner decisions. Builds on [AUDIO.md](AUDIO.md), [VIDEO-CORE.md](VIDEO-CORE.md) and the earlier
[av-audit](../av-audit/KABL_V2_PLAN.md) package. Where this file disagrees with av-audit, §10 says why.

## 0. The idea and the verdict

The owner's model:
- **One modular graph, two domains.**
- Any audio signal can drive anything in video. Example: an ADSR's level becomes a feedback zoom.
- Any video signal can drive audio. Example: brightness becomes filter cutoff.
- **A module can have inputs and outputs in both domains.**
- **A module can be a container** of other modules, nested, crossing domains.

**Verdict: feasible, and kabl is closer than the av-audit package assumed.**
- **Containers are nearly free.** kabl's composites (D09) are already ownership metadata over a flat graph: "no wrapper DSP node, recursive engine" (`docs/composites/design.md`). Nesting goes to depth 8, with exposed ports and controls, panels, and an immutable library.
  - A container that holds audio *and* video leaves needs only one thing: each leaf's compiler must know its domain.
  - Containers cost nothing at runtime.
- **Cross-domain cables need three things:**
  - ports tagged with a domain;
  - a compiler that cuts the graph into an audio part and a visual part;
  - a **bridge** that carries values and events between them with sample timestamps.
- **Joint modules** are one module identity (id, params, state, panel) with **two executors**: an audio `Module` and a `VisualModule`. Each executor owns the ports of its domain, and a private bridge slot links them.
- Hard rules carried over from kabl and av-audit:
  - audio never waits for video;
  - nothing on the audio thread allocates, locks or touches the GPU;
  - textures never cross into the audio domain, only values, events and small arrays do.

## 1. Concepts

### 1.1 Domains and port types

| Domain | Runs on | Types |
|---|---|---|
| **Audio** | audio callback, 64-sample blocks | `Audio`, `Cv`, `UnipolarCv`, `Gate`, `Pitch` (existing, all f32 per sample) |
| **Visual** | render thread, one step per frame, GPU | `Video` (texture), `Value` (f32 per frame), `Trigger` (timestamped events), later `Path`, `Array` |

`PortInfo` gains `domain` (`kabl/crates/modules/src/info.rs:59`). `PortType` stays audio-only; visual types live in synteza's `VisualInfo`.
Today `PortType` is metadata only, and **neither the compiler nor the UI checks type compatibility** (`compile.rs:107-150` has no type error).
Cross-domain wiring is the moment to add a real check, at least across domains.

### 1.2 Conversions: what a cross-domain cable does

Every cross-domain cable carries a **converter**. The default is picked from the endpoint types. It shows as a chip on the cable and is editable, like functional-cable parameters (`kabl/crates/cables`). This follows av-audit D-11 ("explicit, never implicit"), but users never have to insert a converter module by hand.

| From → To | Default converter | Options |
|---|---|---|
| `Cv`/`UnipolarCv`/`Pitch` → `Value` | **sample at frame time** (interpolated between blocks) | hold, peak since last frame, mean since last frame, slew (ms) |
| `Gate` → `Trigger` | **rising edge**, sample-accurate, velocity = level | falling edge, both, gate-as-Value |
| `Gate` → `Value` | level | — |
| `Audio` → `Value` | **envelope** (peak/RMS since last frame) | band (low/mid/high), pitch-follow |
| `Audio` → `Array` | **ring** (last N samples: scope) | spectrum (N bins), triggered scope (zero-cross locked) |
| `Value` → `Cv` | **hold + ramp across the frame period** | slew, step |
| `Trigger` → `Gate` | pulse (ms) | toggle |
| `Video` → `Value` | **mean luma** of a region (async readback, 1–3 frames late) | mean RGB, max, region rect |
| `Video` → `Array` | **one row/column** of pixels (scanline) | ring of rows |
| `Array` → `Audio` | only through a joint module (e.g. `osc.scan`), never directly | — |
| `Video` → `Audio` | **refused** | use a joint module |

Polyphony: a voice-rate audio source crossing into video gets an **aggregation** option:
- `max`, `sum`, `newest voice`, `voice k`;
- or **per-voice lanes**, which feed a voice-rate visual module and become instanced draws (§5, "one particle per note").

kabl today *averages* voice → global (`compile.rs` header). For visuals the default should be `max` for levels and `any` for gates.

### 1.3 Executors and joint modules

```
                 ┌───────────── module identity (ModuleId, kind, params, state, panel) ─────────────┐
                 │                                                                                   │
  audio ports ──►│ audio executor: kabl Module::process     ──private bridge slot──►  visual executor │──► video ports
                 │ (audio thread, per voice lane)            ◄───────────────────────  VisualModule   │◄── video ports
                 └───────────────────────────────────────────────────────────────────────────────────┘
```

- A *kind* registers 0–1 audio executor and 0–1 visual executor: audio-only (today's 53), visual-only (VIDEO-CORE §3) or joint.
- **Params are shared.** One `ParamInfo` list. The runtime value path (`RuntimeTarget::Param`) fans out to both executors through their own queues.
- A joint module's private link is a fixed-size **telemetry record**, written by the audio half each block. It is declared like `Module::view` today (`kabl/crates/modules/src/view.rs`) but sized by the module. The visual half reads it at frame time.
- Instance multiplicity: the audio half may have N voice lanes × 2 channels (`compile.rs:1281` voicing, stereo doubling). The visual half has 1 instance, or one per voice lane when the kind is voice-rate in both domains.
- State carry stays per executor, keyed by `(ModuleId, voice)`. It matches kabl's `carry_state` and synteza's graph swap.

### 1.4 Containers (composites) across domains

- `Composite.members` is a set of `ModuleId`s (`kabl/crates/core/src/composite.rs`), so it can hold leaves of either domain unchanged.
- `Exposure{label, target: PortRef}` does not need a domain tag: the domain is looked up from the target port's info. `validate_composites_scoped` (`compile.rs:3334`) must consult the unified port table.
- A composite's exposed ports can be any mix: audio in, video out, Value in, Gate out.
- **A container is the unit of reuse for an AV instrument.** Example, a "Bloom Bass" card:
  - audio: osc → ladder → VCA ← ADSR;
  - video: shape → feedback → colourise;
  - internal cables: ADSR → shape size, pitch → hue;
  - exposed: 4 macros (Cutoff+Glow, Decay+Trails, ...), audio out, video out.
- **Looks** = visual composites with exposed `Value`/`Trigger` inputs named by role (`clock`, `main env`, `lead gate`, `energy`). Auto-binding (av-audit) means connecting those exposed inputs by role when a Look is dropped into a patch. The connections are visible cables, never hidden state.
- Voice-rate containers: a composite whose leaves are voice-rate in both domains instances its visual half per voice, so "each note is a particle with its own sound and shape" is a single container.
- Package limits (`kabl/crates/ui/src/composites.rs:20`: 128 modules, 512 cables) need a separate visual budget (texture count and GPU cost).

## 2. Data model changes (kabl core)

| Change | Where | Notes |
|---|---|---|
| `PortInfo.domain` (audio default) | `modules/src/info.rs` | every builtin literal defaults to audio; code-module meta parser gains `port in video ...` later |
| Visual kinds in the registry | `modules/src/registry.rs` → `create_in` + new `create_visual(kind)` | kind strings in one namespace, e.g. `v.osc`, `v.feedback` |
| `ModuleState`, `CableState`, `PortRef` | `core/src/state.rs`, `op.rs` | **unchanged**: ports are named strings, cables are endpoints |
| Converter params on cables | `CableState.params` | same mechanism as functional cables (`length`, `prob`, ...): `conv`, `slew_ms`, `agg`, `region` |
| Patch-wide visual settings | new small table: resolution, fps cap, output assignments | not in `ModuleState` |
| Node-view layout | side-table (av-audit D-03) | later; the rack is the first editor |
| Schema 9 | `core/src/format.rs` | **must accept v8**; today `check_schema` is strict equality. Add the migration chain first (AUDIO A0.6) |

No new `Op` variants are needed for the core idea. `AddModule` with a visual kind, `Connect` across domains and `SetParam` on cable converters all fit the existing ops. The `Op` doc comment asks for a decisions.md entry for new variants; this design avoids them.

## 3. Compilation

```
PatchState ──(control thread)──► partition by port domain
                                   ├─► audio graph   → kabl engine compile (existing) + bridge-out/bridge-in steps
                                   ├─► visual graph  → synteza-graph compile (VIDEO-CORE §2.4)
                                   └─► bridge plan   → slot table: (source ModuleId, port, lane) → (dest, converter)
```

1. **Partition.** Each module's ports go to the side of their domain. Joint modules appear on both sides with the same `ModuleId`.
2. **Cross edges become bridge slots.**
   - On the audio side, the compiler appends a `Step::BridgeOut{slot, buffer, conv}` after the producer. It works like the existing `Step::Cable` (a non-module compiled node) and:
     - copies the block's last value plus min/max/mean;
     - scans a `Gate` buffer for edges and pushes `(sample_pos, strength)` events, so edges are sample-accurate with no change to modules;
     - copies audio into a ring for `Array` conversions.
   - Visual → audio edges become `Step::BridgeIn{slot, ramp}` before the consumer.
3. **Cycles across domains are legal.** They cost at least one block plus one frame. Audio-only cycles keep the existing one-block delay.
4. **Limits.** The bridge is sized at compile time: max slots, max events per block, ring sizes. Overflow drops the newest event, increments a counter and shows on the cable chip. It never blocks.
5. **Deliver both graphs with the same revision number.**
   - The audio graph goes through the existing `ToAudio::Graph` queue; the visual graph goes through its own queue.
   - The bridge slot table is versioned by revision.
   - During kabl's 15 ms audio crossfade, the *incoming* graph publishes once its fade starts.
   - The visual side ignores slots from an older revision once it has swapped.
6. Remove `MAX_INPUTS = 4` as a constraint for joint modules. Their visual ports don't count against audio io slots. Fix the SDK mismatch anyway (AUDIO A0.3).

## 4. The bridge

### 4.1 Audio → visual

| Channel | Mechanism | Payload |
|---|---|---|
| Values | **latest-wins snapshot**: triple buffer of a fixed slab, written once per block | per slot: last, min, max, mean, block end sample |
| Events | SPSC ring (`rtrb`, as used today) | `(slot, sample_pos, strength)`; cue landings, note on/off, steps |
| Rings | fixed SPSC sample ring per `Array` slot | audio samples |
| Clock pairs | in every snapshot | `(block_end_sample, monotonic_ns, output_latency_samples)` |

New engine events (av-audit R1/R5) come for free from `BridgeOut` edge scans. Exception: **cue launch landed**, which `PatchEngine::launch` must start emitting (`patch_engine.rs`, `PendingLaunch`). Note-on/off comes from `midi.in`'s gate and pitch outputs.

Do **not** reuse face taps for this. `facetap.rs` is single-consumer (owned by `ui/src/displays.rs`), capped at 16 and windowed at ~25 ms (the av_proof's limits). Keep face taps for the rack displays and add the bridge beside them, pushed from the same callbacks (`ui/src/app/callback.rs`, `clap/src/lib.rs`).

### 4.2 Visual → audio

- Values: SPSC ring of `(slot, frame_sample_pos, value)`. `BridgeIn` holds the latest and ramps to it over one frame period. Latency ≈ one frame + the ramp.
- Arrays (scanline → wavetable): async GPU readback → triple buffer → a joint module's audio half reads the latest complete array. Latency 1–3 frames.

### 4.3 Timing: making a drum hit and its flash coincide

- Master clock = the audio sample counter.
- For each frame the render thread computes `s_present`, the sample position the listener hears at the moment the frame is displayed:
  `s_present = block_end_sample + (t_present − monotonic_ns)·sr − output_latency_samples`.
  `t_present` is the predicted vsync plus the display latency.
- Values are interpolated at `s_present`. Events with `sample_pos ≤ s_present` that were not yet consumed apply this frame, with their sub-frame offset passed to the module (for motion blur and particle spawn age).
- Video usually presents later than audio is heard. The residual offset is shown and can be trimmed by the user with an **AV offset** setting (as in DJ and VJ software), measured once per setup with a flash+click test patch.
- **Lookahead for known events:**
  - the sequencer, drum grid and cues know their next steps;
  - a later option lets these modules publish scheduled events a little early, so visuals can anticipate (pre-roll a shockwave);
  - not needed for v1 of the bridge.
- **Offline render** (golden clips, "render to file") runs in lockstep: render audio up to frame k's sample, then frame k. Visual → audio uses a fixed one-frame delay. The output is deterministic.

### 4.4 Threads

```
audio callback  ── existing; + BridgeOut/BridgeIn steps (no alloc, no locks)
control thread  ── compiles both graphs (existing kabl-graph-compile thread)
render thread   ── new; owns wgpu Device/Queue (shared with egui-wgpu), VisualModules, frame loop, async readbacks
analysis thread ── new; synteza analysers for Audio→Value "band/pitch" converters and analyze.* modules (fed by bridge rings)
UI thread       ── egui; shows visual previews as egui textures from the shared Device
```

### 4.5 Failure isolation

- `kabl/Cargo.toml` sets `panic = "abort"`, so any panic in-process kills audio.
- **Recommendation: in-process render thread, written panic-free:**
  - no `unwrap` in the visual crates (enforced by clippy `unwrap_used`/`expect_used` there);
  - wgpu errors captured with error scopes and the uncaptured-error handler;
  - device-loss recovery rebuilds the visual graph;
  - a heartbeat watchdog: if frames stop, show a banner while audio keeps playing.
- **The synteza runner is the process-isolated option** for installations: kabl streams the bridge over a local socket or shared memory to `synteza run`. This decides av-audit D-04 without blocking: in-process for the instrument, sidecar for shows.

## 5. Joint-module catalogue (examples that show off the idea)

| Module | Audio side | Video side | Why it's interesting |
|---|---|---|---|
| `osc.scan` (scanline oscillator) | wavetable oscillator playing a row of pixels | `Video` in, row position, scan direction | **video becomes sound**; feedback patches literally sound like they look |
| `scope.av` | audio in (stereo) | `Path` out (X/Y Lissajous) + `Video` | oscilloscope music; synteza beam |
| `v.osc` with pitch | `Pitch` in (V/oct) | stripes or rings at a frequency locked to the note | visuals in tune: a fifth looks like a 3:2 pattern |
| `env.av` | ADSR | `Value` + `Trigger` outputs with stage | one envelope shapes sound and image identically |
| `voice.particles` (voice-rate) | gate, pitch, velocity per voice | one particle/sprite per voice: hue from pitch, size from envelope | polyphony becomes visible |
| `fx.reverb.av` | reverb | `Value` of tail energy → feedback decay | the room's size is the image's memory |
| `analyze.av` | audio in | bands, onset `Trigger`, tempo phase | external audio into the same graph (synteza's analysers) |
| `luma.cv` | `Cv` out | `Video` in, region | video drives the synth (Lumen-style, inverted) |
| `cue.scene` | cue landing | switches or crossfades visual subgraphs | structure-exact scene changes |
| `granular.av` (later) | granular sampler | grain cloud rendered as points | needs the asset model |

Most of these are thin. The engine work is the bridge; each joint module is a few hundred lines once that exists.

## 6. Assets (needed by samples, wavetables, images, video, shaders)

kabl already embeds wavetables and code modules in the patch. Generalise this to one **asset store per patch**:
- content-hashed;
- small assets embedded, large ones linked with a missing-asset placeholder (av-audit D-09);
- shared by wavetables, `.scl` tunings, shader files, images, video clips and samples.

Do this once, before V5 video input and before any sampler.

## 7. UI

- **The rack is the first editor for both domains**, as in Lumen, which is itself a rack.
  - Visual modules are rack modules with the same knobs, jacks and panels.
  - Cables show their domain by style: audio as today; video thicker with a texture tint; Value/Trigger dashed. Cross-domain cables carry the converter chip.
- **Previews on module faces**: every visual module can show its output thumbnail (egui texture from the shared device, about 128 px, updated at a reduced rate).
- **Perform view** gets a main visual output preview and the Look picker. Fullscreen outputs are `out.window` modules, assigned to displays.
- **Bindings are cables.** av-audit proposed bindings as a separate patch-data concept "not cables". kabl already has modulation routes: cables into `PortRef::Param` with `amount`/`bypass`, `RuntimeTarget::Route`, and routes into routes. A cable into a visual module's param is the same thing. An inspector "binding slot" list per parameter is a *view* of those routes, not a second system. This keeps the principle "musical controls reveal their real targets".
- Node view: later, as a second projection (av-audit D-02/D-03) once the rack proves the model.

eframe switches from `glow` to its `wgpu` backend (`kabl/crates/ui/Cargo.toml:13`). This needs a visual regression pass over kabl's custom painting (themes, cable glow, panels) and an accesskit check.

## 8. Plugin (CLAP, later VST3/AU)

- The audio side is unchanged. The bridge runs inside the plugin the same way.
- The editor can use egui-baseview's existing `wgpu` renderer (`kabl/vendor/egui-baseview/Cargo.toml:16`), so **previews in the plugin editor are plausible**, which av-audit D-05 didn't account for. Fullscreen output from a plugin is a separate window owned by the plugin (test per host).
- Fallback: the plugin streams the bridge to the `synteza` runner (the sidecar path), which works in any host.
- Order: standalone first; plugin preview after the standalone ships; plugin outputs after host tests.

## 9. Code modules (user extensions)

| Extension | Mechanism | When |
|---|---|---|
| Audio DSP | WASM ABI (exists) | now |
| Visual module | shader file (VIDEO-CORE §4); no CPU code | V2–V4 |
| Joint module | **package** = WASM audio half + shader visual half + a telemetry record layout declared in meta | after M4 |
| Visual CPU logic (e.g. a particle simulation) | WASM on the render thread with its own fuel budget (av-audit D-21's "separate sandbox") | late; most needs are met by shaders + Value modules |

## 10. Where this differs from the av-audit package

| av-audit | This plan | Why |
|---|---|---|
| D-06: a second `InterfaceInfo` table, "no compile change" | `PortInfo.domain` in one table; the compiler partitions | One table keeps composites, `PortRef`, the UI port lists and the code-meta path unchanged. "No compile change" was optimistic: exposure validation and port scans all assume one table |
| D-12: cross-domain composites deferred to 1.2+ | **in the first AV milestone** | Composites are flat metadata; cost is near zero once ports have domains, and they are the owner's core idea |
| Bindings as a new patch concept, not cables | bindings = route cables | Routes already exist; avoids a second modulation system |
| D-15: reuse face taps | separate bridge | Face taps are single-consumer, capped at 16 and windowed |
| D-13: Event + Scalar only | + `Array`, + visual→audio | Joint modules (`osc.scan`, `luma.cv`) need both directions |
| V1.1 non-goal: plugin visuals | plugin preview plausible after standalone | egui-baseview already has a wgpu renderer |
| "Semantic hash" for layout (D-03) | doesn't exist; equality includes `pos` | Define it when the node view lands |
| Release order V1.0 → 1.0.x → V1.1 → … → V2.0 (owner-fixed) | **kept**. Milestones below map onto it | — |

## 11. Milestones (merged roadmap)

| Milestone | Release | Content | Done when |
|---|---|---|---|
| **M0** | 1.0 | AUDIO A0 (tag, schema migration path, SDK limit fix); VIDEO V0 (synteza cleanup, licence) | v1.0.0 released; synteza CI green |
| **M1 live bridge proof** | branch | `BridgeOut` steps + snapshot/event ring behind a feature flag; synteza V1 runtime in a window; 2 visual modules; kill test | 10-min soak: 0 xrun delta, 0 audio-thread allocations; event→photon p50/p99 measured; killing the render thread leaves audio playing |
| **M2 domains** | 1.1 | `PortInfo.domain`; visual kinds in the rack; cross-domain cables with converter chips; schema 9 (opens 8); eframe on wgpu; previews on faces; `out.window`; VIDEO V2 P1 primitives | a musician patches ADSR → feedback zoom and gate → shape flash in the rack, saves, reloads, plays fullscreen |
| **M3 containers + Looks** | 1.1 | cross-domain composites (exposures of either domain); 4–6 Looks from synteza content (VIDEO V3); role-named exposed inputs and one-click connect | "Bloom Bass" container in the library: drop it in, play, sound and image together |
| **M4 joint modules** | 1.2 | executor pair per kind; telemetry records; `env.av`, `v.osc` with pitch, `scope.av`, `luma.cv`, `osc.scan`; voice-rate visuals (instancing) | `osc.scan` feedback patch and `voice.particles` run at 60 fps with 16 voices |
| **M5 shaders + offline** | 1.2 | shader modules + ISF import (VIDEO V4); "render to file" via lockstep; golden AV clips in CI on a software adapter | user drops a `.wgsl`/ISF file → module; golden clips pass |
| **M6 media + outputs** | 1.3–1.4 | asset store; camera/video in; record; Syphon/Spout/NDI; analysers (VIDEO V5/V6) | each output verified on its OS |
| **M7 V2.0** | 2.0 | user joint-module packages; plugin preview; node view; runner/sidecar mode; multiplatform release of all of it (AUDIO A4) | Windows/macOS/Linux builds of the full AV instrument; blind A/B vs Surge (audio) and vs Lumen (video) recorded |

AUDIO A1–A3 run beside M1–M4 in separate worktrees. They touch `modules/` and `engine/` voice code, while the AV work touches `core/`, compile partitioning, `ui/` and synteza.

## 12. Decisions needed from the owner

1. synteza licence (MIT/Apache vs GPL-3) and "synteza = engine library + runner, kabl = editor". See VIDEO-CORE §2.1.
2. Cross-domain composites in the first AV release (M3), pulling av-audit D-12 forward.
3. Bindings are route cables, not a separate concept.
4. In-process render thread with panic-free rules; the synteza runner is the isolated mode (D-04).
5. Visual → audio direction in scope from M4 (`luma.cv`, `osc.scan`).
6. Default polyphony aggregation for visuals: max/any rather than kabl's audio averaging.
7. Run M1 now, in a feature-flagged branch, in parallel with the 1.0 finish (av-audit already recommends this).
