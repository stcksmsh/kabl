# kabl — research and product implications

Researched 2026-09-25. Supports [PRODUCT-PLAN.md](../PRODUCT-PLAN.md).

## Method and limits

Read current kabl handoff/batch docs and targeted production code, then consulted official product pages, manuals, framework documentation and upstream source. This is a selective comparison against the workflows kabl needs, not an exhaustive market census. No competitor was installed or benchmarked in this session. Descriptions of competitor features below are documented capabilities; statements about what kabl should adopt are design judgments. Vendor claims that something is intuitive or sounds excellent are not independent usability/listening evidence. Prices, market share and future release predictions are intentionally excluded.

## 1. Alternatives users already have

| Instrument/system | Relevant documented capability | Implication for kabl; pattern to adopt |
|---|---|---|
| [Vital](https://vital.audio/) | Animated modulation/filter/waveform feedback, drag-and-drop modulation with preview, free edition, native Linux support | Visual modulation is not unique. Adopt immediate reversible feedback; compete on patch understanding and reuse, not another waveform display |
| [Surge XT](https://surge-synthesizer.github.io/manual-xt/) | Patch search/categories/favorites, unsaved-change protection, contextual help, routing inspection, scenes, Linux CLAP/VST3 | It is a strong free production baseline. Sound discovery and safe editing are basic requirements, not optional polish |
| [Serum 2](https://xferrecords.com/products/serum-2) and [module operations](https://xferrecords.com/web-manual/serum-2/accessing-the-oscillator-or-filter-menu) | Broad synthesis instrument; module copy/paste, initialization, locking and modulation-aware operations | Preserve useful work when experimenting. Later compatible module replacement should retain related settings/routes where meaning matches; do not chase synthesis breadth first |
| [Pigments](https://www.arturia.com/products/software-instruments/pigments/overview) | Performance-oriented Play View, visible modulation, in-app tutorial, multiple engines and substantial factory content | A simple performance surface can coexist with deep editing. Kabl needs the transition from a musical control to the real mechanism, not a duplicate editor |
| [VCV Rack Pro](https://vcvrack.com/manual/RackPro) and [Rack](https://vcvrack.com/Rack) | Standalone and DAW plugin modular environment with a large module ecosystem | Rack appearance and modular flexibility are established. Avoid competing primarily on module count; retain understandable boundaries and convenient musical entry points |
| [Cardinal](https://github.com/DISTRHO/Cardinal) | Free, self-contained modular plugin with included modules; no external Rack module loading; README lists several plugin formats and calls CLAP work-in-progress | Free modular-in-a-DAW already exists. Learn from self-contained dependencies and host-owned processing; don't present its listed formats as equally mature |
| [Bitwig Grid](https://www.bitwig.com/userguide/latest/welcome_to_the_grid/) | Interactive module help, local signal scopes, compatible module replacement; Polymer can become an editable Grid patch | Closest precedent for contextual modular assistance. Adopt local explanation and moving from approachable instrument to inspectable patch; kabl remains usable in REAPER rather than requiring this DAW |
| [u-he Hive](https://u-he.com/products/hive/) / [Bazille](https://u-he.com/products/bazille/) | Hive combines sound shaping with sequencer/arpeggiator and modulation; Bazille combines modular synthesis and a segmented modulation sequencer | Evolving sequences are already served. Kabl's opportunity is comprehensible multi-layer performance and reusable instruments, not claiming to invent sequencer-driven synthesis |
| [Phase Plant](https://kilohearts.com/docs/phase_plant) | Generator stack, modular effects and modulation, including audio-rate cross-modulation | Learn functional grouping and clear stages. Do not make flexible audio-rate parameter modulation a prerequisite for simple sound-shaping workflows |
| [Syntorial](https://www.syntorial.com/learn-more/) | Interactive sound-matching/ear-training exercises; its training synth is also available as a plugin | The teaching market is real. Adopt small audible experiments and transfer tasks, but do not build an extensive course before ordinary music-making works |
| [Ableton Learning Synths](https://learningsynths.ableton.com/en/get-started) / [Playground](https://learningsynths.ableton.com/playground) | Interactive synthesis learning and a playground | Useful reference for presenting one controllable concept at a time. Browser content was partly script-rendered during research, so detailed interactions were not independently exercised |
| [Synplant](https://soniccharge.com/synplant) | Exploratory sound variation and Genopatch, which derives synth patches from reference audio | “Get a sound quickly” can also mean search/inference, not only knobs. Kabl should first test deterministic, inspectable editing; an audio-to-patch model would be a separate project |
| [Reaktor](https://www.native-instruments.com/products/reaktor) / [User Library](https://userlibrary.native-instruments.com/) | Building instruments from modules/macros and sharing user-created instruments/effects | Creator freedom and a user library are established precedents. Adoption requires worthwhile modules and reliable sharing, not merely publishing an SDK |
| [Max encapsulation](https://docs.cycling74.com/userguide/subpatchers/) / [bpatchers](https://docs.cycling74.com/userguide/bpatchers/) | Group objects into a component, expose inlets/outlets, inspect internals, reuse patch files and embed interfaces | Strong model for kabl's composites and custom panels. Preserve explicit distinction between an embedded instance and a shared definition |
| [plugdata](https://plugdata.org/) / [compiler documentation](https://plugdata.org/docs/book/CompilingPatches.html) | Visual audio programming, standalone/plugin use and export through Heavy/hvcc; compilation supports a subset of objects | Demonstrates patch-to-code/product workflows. Do not promise that all arbitrary patch behaviour can be compiled/exported without restrictions |

The personal comparison set should initially be Surge XT and Vital (normal production), Cardinal/Rack (patching), and Bitwig's documented assistance patterns. Commercial references are design research, not a recommendation that Kosta buy a collection of tools.

## 2. What the differentiation can actually be

Inference from the comparison: a claim such as “modular, visual and easy” is too broad. Kabl needs an observable sequence of actions competitors may distribute across different screens/tools:

1. Choose a relevant playable sound.
2. Change a musical property with a small set of exposed controls.
3. Reveal exactly which modules and routes produced that change.
4. Compare and undo without losing the sound.
5. Open a reusable component and learn from its internals.
6. Use the same creation in a song or package it for someone else.

This combination may be compelling; research alone does not prove it wins. Time-to-result, successful recovery, transfer to a new patch and repeated voluntary use are the evidence to collect. Avoid vague ease-of-use scores produced by implementing agents.

Tangerine Dream is a musical scenario, not an emulation specification. The current sequencers, cues, modulation and effects already address much of that scenario. Further DSP should follow a named missing sound or performance action. A literal recreation of a particular album, Mellotron, tape machine or historical synth is not implied.

## 3. Composites: benefits and costs

Kosta explicitly wants reusable components and code-defined behaviour. These are complementary layers.

**Composites:** build from existing modules, expose a useful interface, preserve inspectable internals. Advantages: less repeated wiring; a smaller working surface; reusable musical ideas; a natural learning progression; an easier route for nonprogrammers to contribute. These are architectural/product advantages, not a guarantee of lower CPU load.

**Code modules:** introduce new algorithms, control logic or calculations. Advantages: extend the sonic and behavioural vocabulary beyond the shipped primitives. They require a supported execution, state and version contract; writing an algorithm does not eliminate the need for good controls and documentation.

The interaction to avoid is blackboxing that hides everything permanently. A “soft strings voice” can expose tone/attack/motion/space while an Inspect action reveals its actual graph. A panel's labels should describe that interface, not claim all internal complexity disappeared.

Important kabl-specific decisions:

- Keep exposed port/control IDs stable when rearranging or renaming the face.
- Compile composites into the existing schedule if practical; visual grouping alone should not add an audio buffer or change voice averaging.
- Embed the used definition and assets in project state; updates are explicit. Library updates must not alter old REAPER projects behind the user's back.
- Separate “edit this instance” from “publish a revised reusable component.”
- Preserve module identities across wrapping/unwrapping where needed for live state, routes and automation.
- Scope nesting and recursion. Start with supported finite composites; invalid dependency cycles should produce useful errors.
- Reuse one manifest for help, controls, assets and compatibility across composites and later code modules.

These are recommendations. The exact persistence schema and implementation belong to the relevant batch.

## 4. Plugin route and libraries

[REAPER officially supports CLAP on Linux](https://www.reaper.fm/). Native Linux CLAP is sufficient for the first requested DAW workflow; VST3 is not a prerequisite merely because users often call every plugin a VST.

| Candidate/resource | Verified finding | Recommendation |
|---|---|---|
| [Original NIH-plug](https://github.com/robbert-vdh/nih-plug) | Upstream README says maintenance mode and points to a community fork. It documents parameters, state, events and GUI adapters | Do not blindly follow the old kabl stub's NIH-plug choice. Read exact pinned code and dependency licenses |
| [nice-plug documentation](https://docs.rs/nice-plug/latest/nice_plug/) and [egui adapter](https://docs.rs/nice-plug-egui/latest/nice_plug_egui/) | Published framework/adapter docs expose plugin lifecycle, persistent state, parameter IDs, editor integration and bundling; documentation remains incomplete | First high-level candidate. Prove actual editor compatibility and lifecycle in REAPER; a crates.io page is not a tested kabl integration |
| [Clack](https://github.com/prokopyl/clack) | Safe low-level Rust wrappers for CLAP, split plugin/host/extensions crates | Fallback where control/compatibility is better; more lifecycle/state/GUI glue is likely required |
| [CLAP events](https://github.com/free-audio/clap/blob/main/include/clap/events.h) | Event headers carry sample offsets; native note events have semantics distinct from raw MIDI | Advertise and implement a deliberate note dialect. Do not just call the standalone raw-MIDI handler at callback start |
| [CLAP parameters](https://github.com/free-audio/clap/blob/main/include/clap/ext/params.h) | Parameters have stable IDs and host interaction rules | Begin with stable automation slots; don't derive host identity from mutable rack positions or indices |
| [CLAP state](https://github.com/free-audio/clap/blob/main/include/clap/ext/state.h) | Host-facing save/load stream contract | Serialize the actual self-contained instrument state, not a reference to `patches/foo` |
| [CLAP validator](https://github.com/free-audio/clap-validator) | Automated validation/test tool | Add to plugin validation; supplement with actual REAPER save/reload, render and UI lifecycle tests |

Kabl currently uses egui/eframe 0.35. Exact adapter version compatibility remains unproven. The old NIH egui adapter uses pinned baseview/egui-baseview dependencies. Do not force a broad GUI downgrade or framework rewrite based on a name match. An integration proof should either reuse the real rack successfully or document the smallest compatible bridge before proceeding.

The difficult work is not the `.clap` extension: runtime parameters, sample-timed events, host transport, stable automation, instance isolation, closed-editor processing and project recall are the actual product contract. CLAP support does not automatically implement MPE, host synchronization or sample accuracy in kabl's engine.

## 5. Code-module execution route

[Faust's compiler](https://faustdoc.grame.fr/manual/compiler/) supports DSP generation to several targets, including Rust and WebAssembly. That makes it a candidate authoring route, not proof that embedding a compiler is appropriate or that generated modules meet kabl's state/RT contracts.

[Wasmtime documents](https://docs.wasmtime.dev/examples-interrupting-wasm.html) fuel and epoch interruption. Fuel can give deterministic instruction-budget interruption but adds overhead; epochs have different timing characteristics. Neither establishes that a module completes within a 5.333 ms audio callback. Runtime, allocation and failure behaviour must be tested on the real processing path.

Recommended feasibility shape: prepare/compile off-thread; bounded memory, port and state sizes; one block-oriented ABI; no audio-thread I/O; per-instance state; deterministic reset where promised; explicit handling of traps/budget exhaustion/nonfinite samples; benchmark against a native reference. Start with one authoring toolchain and two meaningful examples. Defer native shared-library loading unless there is an explicit later decision accepting its isolation and portability tradeoffs.

Avoid promising an “infinite range” as a literal engineering guarantee. User code expands the design space, while every supported runtime has performance and API limits. A clear limit is compatible with creative freedom; silent crashes, missing assets and changed old songs are not.

## 6. Repository observations worth carrying forward

Source links are pinned to the audited commit:

- [Registry: 21 built-ins](https://github.com/stcksmsh/kabl/blob/c1def5361d66caeee477b2be5c737c7e343552f3/crates/modules/src/registry.rs).
- [UI: path-based Save/Load and rack presentation](https://github.com/stcksmsh/kabl/blob/c1def5361d66caeee477b2be5c737c7e343552f3/crates/ui/src/lib.rs).
- [App: audio adapter, CC/UI handling, graph rebuild, stall detection](https://github.com/stcksmsh/kabl/blob/c1def5361d66caeee477b2be5c737c7e343552f3/crates/ui/src/main.rs).
- [Engine: fixed-block process, keyboard and launch ownership](https://github.com/stcksmsh/kabl/blob/c1def5361d66caeee477b2be5c737c7e343552f3/crates/engine/src/patch_engine.rs).
- [Keyboard: raw MIDI subset, channel ignored](https://github.com/stcksmsh/kabl/blob/c1def5361d66caeee477b2be5c737c7e343552f3/crates/engine/src/keyboard.rs).
- [Format: schema v3, directory files and op-log replay](https://github.com/stcksmsh/kabl/blob/c1def5361d66caeee477b2be5c737c7e343552f3/crates/core/src/format.rs).
- [Skin: embedded images, controls, dimensions, theme variants](https://github.com/stcksmsh/kabl/blob/c1def5361d66caeee477b2be5c737c7e343552f3/crates/modules/src/skin.rs).
- [Module metadata: explain and lesson fields](https://github.com/stcksmsh/kabl/blob/c1def5361d66caeee477b2be5c737c7e343552f3/crates/modules/src/info.rs).
- [Plugin stub](https://github.com/stcksmsh/kabl/blob/c1def5361d66caeee477b2be5c737c7e343552f3/crates/clap/src/lib.rs), [learning stub](https://github.com/stcksmsh/kabl/blob/c1def5361d66caeee477b2be5c737c7e343552f3/crates/learn/src/lib.rs), [composites stub](https://github.com/stcksmsh/kabl/blob/c1def5361d66caeee477b2be5c737c7e343552f3/crates/pedals/src/lib.rs).

Additional integration risks found by inspection, not asserted as reproduced bugs: hard capacities (8 keyboards, 32 pending launches, 16 launch targets); note-queue push results currently discarded at some boundaries; multi-file patch save is not transactional; compile-time versus runtime parameter ownership is not yet separated. Future batches should address a limit when their supported workflow crosses it, and avoid silent failure. This is not a mandate for an unrelated broad refactor.

Documentation drift: root README has outdated module/workspace capability descriptions; historical PLAN and planning-prompt describe earlier states; Composition README says local/not pushed; HANDOFF's generic exclusions still mention probability. Current code and the top of HANDOFF/STATUS outrank these. GitHub default branch was changed to master and verified during this session.

## 7. Evidence still missing

- Kosta's two pending batch reviews, laptop dense-patch performance and stall observations.
- Direct observation of musicians new to modular using kabl.
- Comparative task tests in kabl versus selected alternatives. Manuals establish features, not comparative speed.
- Actual REAPER editor/timing/state integration with a selected framework.
- Sound-quality judgments on high-note aliasing and heavily driven patches; measurements alone do not decide whether the limitation matters musically.
- Practical demand and authoring effort for custom modules. A platform has no community advantage until useful creations exist.

No gap above is filled by a scripted video, an unverified agent report or a marketing claim.
