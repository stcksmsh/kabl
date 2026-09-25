# Post-D01 interaction plan and cross-feature audit

Date: 2026-09-25. Source audit at `30b9abdf9cbd9b95306439792e53933091ebc27a`.
Scope: planning for work AFTER D01. No implementation authorization.

## 1. Owner instructions and document precedence

Kosta asked to retain most of the roadmap, resolve the patch/module question and find similar interaction gaps before they become expensive. His explicit follow-up was: **"D01 stays the same, no updates to this agent's work."**

Accordingly:

- D01's launch prompt, brief, scope and acceptance criteria are frozen for this review. Its agent continues without this addendum imposing work.
- Review D01 against its original contract. Record new opportunities separately; do not turn them into D01 defects.
- No changes to the active agent's code, README, CHECKLIST, REPORT, HANDOFF/STATUS or shared agent-workflow contract are part of this planning update.
- This document supplements PRODUCT-PLAN for D02 and later. It does not approve any pending batch or authorize implementation.
- Cloud engineering may proceed through separately authorized batches while laptop evidence and owner reviews remain explicitly pending. A specific blocking defect can change sequencing; unavailable hardware alone must not create fake completion.
- The behaviors below are **supervisor recommendations**, not recorded owner approvals. Existing owner-approved behavior remains the compatibility baseline.

This is a systematic source and workflow audit, not an exhaustive proof of usability. No builds, runtime experiments, listening tests or novice tests were run for it. Recheck the resulting D01 code before preparing the next implementation brief.

## 2. The missing distinction: content, instance and action

Sharing a graph representation does not make opening a document and inserting a component the same operation.

Recommended eventual vocabulary:

| Object | Meaning | Main action |
|---|---|---|
| Patch | Complete working document: graph, values, arrangement/performance configuration and embedded dependencies | Open patch; replaces the current document with edit protection |
| Module | Insertable component with declared ports/controls; either built-in, composite or code-backed | Add module; creates an independent instance |
| Instrument / effect / utility | Musical role of a module; useful browser filters, not separate engines | Add the selected role |
| Module preset | Saved settings for a particular module definition/version; composite settings may include its internal graph | Apply to a compatible selected instance, or add a new instance with those settings |
| Reusable module definition | Versioned structure/interface/assets from which instances are created | Create reusable module / save new version |
| Module instance | One use inside the current patch, with its own state and identity | Edit this instance |
| Patch template | A starting patch opened as a new unsaved document | New from template; no requirement to introduce a new file format |

Do not present all these as top-level browser tabs. Start with clear **Open patch** and **Add module** contexts; instrument/effect/utility and Factory/Your Sounds are filters/provenance. Factory/user and patch/module are independent axes. A patch named "Pad" does not become insertable merely because of its name or tag.

Keep the word "preset" qualified when the target is ambiguous. Replacing a patch, replacing a module and applying compatible settings need visibly different verbs and target scope. Dragging adds only content advertised as insertable; a whole-patch drop must not silently overwrite the rack or merge raw graphs.

**Recommendation for a Surge-style sound:** allow one musical creation to have both a complete playable patch form and an insertable instrument form through an explicit, validated packaging operation. Do not require two separately maintained synth engines. Do not promise that every arbitrary multi-part patch is automatically convertible.

## 3. Recommended answer to MIDI/audio boundaries

A complete patch can contain several keyboard modules and several voices. When packaged as an instrument, its chosen external connections become a declared component interface.

### Physical connections versus logical ports

- Standalone owns physical audio/MIDI connections at the application boundary; the plugin gets them from REAPER.
- An inserted instrument has a logical note input and an audio output, with independent keyboard/voice state inside.
- Device selection is environment configuration. A portable sound must not claim a particular physical device just because its author used it.
- Composite internals cannot independently acquire devices, send directly to the root output or consume all incoming notes through hidden global registration.
- Supporting an event inlet requires an event-routing design. It is NOT just renaming the existing pitch/gate jacks as MIDI.
- CV-controlled components may instead expose pitch, gate and velocity as signal ports. This is a distinct interface: it cannot automatically preserve arbitrary polyphonic note identity, sustain and independent voice allocation.
- Ports declare signal/event type, channel layout and voice scope. Note events, audio, pitch, gate and clock are not interchangeable because their connectors look similar.

### Example: two independent synthesizers in one patch

A root note source routes explicitly to instrument A and instrument B. Each allocates its own voices and keeps its own sustain/glide state. A and B expose stereo audio; a visible mixer feeds the root output. Sending the same notes to both is intentional layering. Routing to only A must leave B untouched.

Basic independent destination routing is required before claiming reusable instruments work. Full keyboard split zones, MPE, multiple hardware devices and multi-output host buses remain separate expansions.

For first-use convenience, recommend **Add and connect** for compatible instruments: show the chosen note source and mixer destination, create ordinary inspectable connections and make it one undoable edit. Ordinary drag/drop adds an unconnected module. Never auto-connect merely because browser focus moved. If no unambiguous source/destination exists, ask for that selection rather than guessing.

This convenience is a proposed D09 interaction, not D01 work and not a hidden routing rule.

### Packaging a patch with midi.in and out

The packaging flow should:

1. Select the graph to package; identify external signal routes, note sources, final outputs, clock references, cues, macros and controller mappings.
2. Suggest a boundary for a simple one-keyboard/one-output voice.
3. Map its keyboard ingress to a declared note inlet while retaining keyboard settings inside the component.
4. Convert the selected final output into an ordinary component audio outlet. At the root, output reaches the device/host; inside a component it reaches the parent graph.
5. Expose selected controls and give the interface a useful default panel. Preserve an inspectable internal graph.
6. Validate the conversion and provide an isolated audition. Commit creation only when validation succeeds; cancel/failure preserves the source.
7. Verify the component against the source with the same events, timing and seeds. Grouping must not change gain, voice averaging, feedback delay or random sequences accidentally.

For multiple keyboard sources or outputs, preserve separate boundaries or explicitly select a mix; do not collapse them silently. A self-running composition also needs clock/start/stop rules. The first creator milestone may reject an ambiguous conversion with a precise explanation, but should not save a broken module.

Raw group insertion, if ever implemented, is a different command from encapsulation and has its own ID/remapping rules. It is not a shortcut for this feature.

## 4. What the code actually establishes

These findings are from the pinned source, not from runtime tests:

| Source | Observed behavior | Consequence |
|---|---|---|
| `crates/engine/src/patch_engine.rs::key` | Broadcasts each KeyEvent to every active keyboard | Instance-specific routing is not present on this path |
| `crates/engine/src/keyboard.rs::KeyEvent` / `from_midi` | Carries note/velocity, off, sustain and all-off; discards MIDI channel | Channel/source identity must be deliberately retained before selective routing |
| `patch_engine.rs::MAX_KEYBOARDS` and `sync` | Fixed capacity of 8; additional keyboards are not registered | Insertion must validate capacity; never let an apparently valid ninth instrument silently receive no notes |
| `crates/modules/src/info.rs::PortType` | Audio/CV/unipolar CV/gate/pitch; no note-event port type | A note inlet is an architectural contract, not a current patch cable |
| `crates/engine/src/compile.rs`, handling `kind == "out"` | Reassigns the compiled final left/right buffer pair for each out encountered | Multiple out modules are not automatically mixed; the last encountered wins in this path |
| `compile.rs`, voice propagation and per-voice/global scheduling | Voice reachability comes from midi.in and downstream routes | Encapsulation must preserve voice domains and reduction semantics |
| `compile.rs`, `Seq::seed(id)` and `Noise::seed(id, lane)` | Randomness depends on module identity | ID remapping can change sound or probability sequence even if wiring/values match |
| `crates/ui/src/editor.rs::from_log` | Preserves an existing patch log and derives next numeric module/cable IDs from current maxima | Reusing a complete document is not safe insertion; fresh instance IDs and reference remapping are needed |
| `crates/core/src/format.rs` | Directory format stores log/checkpoint/meta; authoritative loading replays log | Packages and plugin snapshots need deliberate dependency/history boundaries, not external path references |

Code links:
- [engine/patch_engine.rs](https://github.com/stcksmsh/kabl/blob/30b9abdf9cbd9b95306439792e53933091ebc27a/crates/engine/src/patch_engine.rs)
- [engine/keyboard.rs](https://github.com/stcksmsh/kabl/blob/30b9abdf9cbd9b95306439792e53933091ebc27a/crates/engine/src/keyboard.rs)
- [engine/compile.rs](https://github.com/stcksmsh/kabl/blob/30b9abdf9cbd9b95306439792e53933091ebc27a/crates/engine/src/compile.rs)
- [modules/info.rs](https://github.com/stcksmsh/kabl/blob/30b9abdf9cbd9b95306439792e53933091ebc27a/crates/modules/src/info.rs)
- [ui/editor.rs](https://github.com/stcksmsh/kabl/blob/30b9abdf9cbd9b95306439792e53933091ebc27a/crates/ui/src/editor.rs)
- [core/format.rs](https://github.com/stcksmsh/kabl/blob/30b9abdf9cbd9b95306439792e53933091ebc27a/crates/core/src/format.rs)

Do not silently "fix" the legacy multiple-out behavior by mixing all outs: that changes existing music. Preserve legacy loading and require explicit output choice for new conversion; version any changed root semantics.

## 5. Cross-feature decision register

"Proposed" means a recommended contract requiring incorporation into a later authorized brief. "Verify" means current evidence is insufficient. These are not additional D01 acceptance conditions.

| ID | Question / failure risk | Recommended behavior or required decision | First relevant later work |
|---|---|---|---|
| U01 | Open versus add versus replace looks identical | Explicit action and target; contextual browser, no inferred operation from filename | Post-D01 review; D09 insert |
| U02 | Preview changes the song, starts sequences or leaves notes | Distinguish preview of the loaded patch from isolated candidate audition; isolated preview must not mutate project state/automation or record into the take | Inspect D01 result; isolated audition only in a later scoped brief |
| U03 | Nested Save silently edits every instance or the library | Patch Save saves the current document including its instances. Edit instance stays local; save reusable version is a separate action | D09 |
| U04 | One label serves as library identity, instance identity and automation target | Separate definition/version, saved-entry identity, instance identity and stable exposed port/control IDs | D04/D08/D09 |
| U05 | Duplicating a synth duplicates its keyboard ownership or global bindings | Fresh instance state/identity; explicit note routing; hardware/CC bindings do not silently become global duplicates | D06/D09 |
| U06 | Multiple out modules overwrite or bypass the mixer | Explicit root output; internal outlets terminate at parent; preserve legacy sound through migration | D07 contract, D09 conversion |
| U07 | Mono/stereo, signal/event or per-voice/global cables connect with unclear meaning | Explain compatibility and explicit adapters; preserve intentional existing modulation; grouping cannot alter reductions | D02 explanation, D06/D09 implementation |
| U08 | Two synths share notes, sustain or bend unintentionally | Route source/channel/identity deliberately; preserve original note recipients through release, reroute and source loss | D06; D09 instances |
| U09 | Instrument deletion, replacement, panic and bypass behave inconsistently | Define lifetime per action. Deletion releases only owned notes; global panic remains explicit. Do not replay held notes into a replacement unexpectedly | D05/D06/D09 |
| U10 | Effect bypass kills tails or doubles wet/dry gain | Define mute, bypass and remove separately; no promise of CPU suspension while tails/state still run | D09 effect example; earlier only if scoped |
| U11 | Nested clocks or host/free switching double tempo or restart a song | Distinguish transport authority from local clock generation; inherited/free must be visible and persisted; conversion cannot silently change tempo/reset policy | D08; D09 sequences |
| U12 | Play, Stop, Restart, Seek and queued cues disagree | Publish a state table including tails, held notes, pending cues, LFO phase and randomized sequences | D08, extending existing composition rules |
| U13 | Knob, macro, modulation, MIDI CC and host automation all claim a value | Separate stored base, effective modulated value and current external control. Define who writes the base, ordering/touch/takeover, and undo; preserve current modulation math | D02 presentation; D04/D08 control |
| U14 | Replacing a sound redirects existing automation to a different knob | Stable slot identity plus mapping provenance; incompatible replacement leaves a visible unassigned slot rather than guessing by name/index | D08/D09 |
| U15 | Saving a sound versus saving the REAPER project restores different music | Host state embeds the working patch and exact dependencies; library saving is explicit, not required for host recall | D07/D08 |
| U16 | Machine/device preferences leak into shared patches | Separate portable musical state, project state, user preferences and transient performance state | D07 contract; D09 packaging |
| U17 | Library updates or deletion change existing songs | Embedded versioned snapshots; explicit update review; missing incompatible runtime retains recoverable document with error | D07 dependencies; D09–D12 |
| U18 | Undo inside a module loses the containing song or persists navigation | One document edit history with named grouped operations; navigation isn't a musical edit; library publishing is a distinct operation | D09; D02 reveal |
| U19 | Lesson/A-B exit erases unrelated edits or lies through level changes | Name snapshot scope, make restoration explicit, preserve work on exit; comparison normalization never changes saved output implicitly | D03 |
| U20 | Help describes an old route or opens the wrong instance | Resolve explanations against current graph and instance path; distinguish intent text from measured/structural facts | D02/D03/D09 |
| U21 | Browser/dialog keyboard shortcuts also trigger performance controls | Define focus priority for search/text fields, shortcuts and any audition keyboard; release lost-focus preview ownership | Inspect D01 evidence only against original brief; extend D02/D09 as needed |
| U22 | Composite recursion, excessive voices/ports or missing assets fails as unexplained silence | Preflight bounded capacities and dependencies, then commit atomically; actionable error and no partial insertion | D09–D12 |
| U23 | Group/duplicate/reopen changes seeded noise or probability patterns | Specify preserve-versus-new-seed per action; decouple musical seed identity from remapped IDs where needed | D08 reproducibility; D09 grouping |
| U24 | UI looks available while engine is compiling, stopped or stale | Distinguish requested/applied/error state; show recovery action and preserve last valid graph/work | D04/D05 |
| U25 | A custom panel hides necessary routing, controls or navigation | Generic fallback controls, visible instance path and Open internals; artwork cannot be the only way to discover a control | D09/D10 |
| U26 | "Stereo effect module" becomes mistaken for a REAPER effect-plugin promise | Internal audio-processing composites remain in scope; a host audio-input/effect plugin is a separate decision from the planned instrument | D07/D09; host effect format deferred |
| U27 | Export includes assets but not the exact code/version, or assumes another machine's paths | Portable package contains required definitions/assets and allowed code artifacts with version checks; missing runtime produces clear recovery, not replacement DSP | D10–D12 |
| U28 | Two plugin instances contend for devices, settings or global singleton state | Host owns I/O; independent patch/voice/automation state. Shared library preferences never become shared musical state | D07/D08 |

Priorities: resolve U01/U03/U04/U06/U08/U11/U13/U15 conceptually before building their dependents. Keep the rest assigned to the first feature that makes the behavior observable. Do not build future subsystems merely to close a planning row.

## 6. State ownership to use in future briefs

| State | Owner / persistence | Example |
|---|---|---|
| Sound structure and values | Current patch / host snapshot; embedded instance snapshots | Oscillator values, keyboard mode, cables, macro destinations |
| Performance configuration | Patch / host snapshot | Startup bank, cue definitions, explicitly chosen seed and clock mode |
| Host automation assignment | Plugin instance / host snapshot | Stable automation slot -> exposed control target |
| Environment connection | Standalone application/session preference or host | Physical MIDI port, audio backend, sample rate, buffer configuration |
| Library preferences | User profile | Favorites, recents and browser filters |
| Definition and assets | Versioned reusable package; embedded in dependent patch | Internal graph, panel image, exposed IDs, documentation |
| Live performance state | Runtime; reset/recovery rules rather than held-key persistence | Held notes, pedal latch, pending launch, effect tail |
| Navigation/presentation | Deliberate document or user preference classification | Rack layout may be part of document; focus/search is not sound state |

Persistent MIDI controller mappings need two parts: musical target binding and external source association. Do not export a private device name as a mandatory portable dependency. Decide how standalone bindings map to host input before advertising cross-environment recall.

## 7. Future interaction traces and acceptance scenarios

For each later brief, specify: initial context, action/target, immediately visible effect, audible effect, persistence, undo/cancel, failure behavior and external control interaction. Test the changed boundary, not every theoretical combination.

Maintain these cross-feature scenarios as implementation becomes available:

1. **Find/edit/keep:** open factory pad, modify, save personal sound, reopen. D01 judged solely by its existing contract.
2. **Two instruments:** add two instances, route the same notes to both, then isolate one. Sustain/release/reroute/deletion cannot affect unrelated ownership. Show the mixer and headroom.
3. **One versus all:** edit instance A; B and the library definition remain unchanged. Save/reopen patch; explicitly publish a new version; old songs stay unchanged.
4. **Pack and unpack:** turn a one-keyboard voice into a component and inspect it. Same events/seeds produce equivalent output. Repeat with an ambiguous multi-output patch and demonstrate recovery.
5. **Duplicate under automation:** duplicate a mapped instrument, rename/reorder it and remove the original. Existing automation cannot jump to the duplicate.
6. **Preset during playback:** attempt a compatible settings change and an incompatible replacement under held notes/automation. State exactly what survives; cancel restores the same musical state.
7. **Move between environments:** standalone patch -> REAPER project -> relocated project with original library removed. Two plugin instances remain independent and neither opens a device.
8. **Two sequenced components:** verify root/host transport, free mode, cue quantization, seek/loop and reload. No accidental double clock or implicit tempo takeover.
9. **Learn and return:** reveal a nested macro target, make a change, undo, leave a recipe and return to the same instance without losing other work.
10. **Missing or over-capacity:** missing definition/runtime/artwork, eighth versus ninth keyboard, recursive component and failed insertion each preserve the current document and explain the limit.
11. **Focus and audition:** type a name/search query while playing, switch focus and stop preview. No control shortcut or preview steals unrelated notes.
12. **Update and share:** create version 2, import on a fresh profile, retain version 1 in an existing song, verify assets and saved sound.

Fresh-agent preflight for D02 onward should include the relevant rows/traces and identify unresolved conflicts BEFORE implementation. It is a normal first step inside an authorized batch, not a new internal approval gate. Ask Kosta only for a consequential product choice; make routine engineering choices and document them.

Supervisor review must check the relevant interaction evidence alongside feature acceptance. New preferences discovered during review become next-scope work unless they violate the issued brief.

## 8. Roadmap changes: mostly scope clarification, not reordering

D01 remains unchanged. Keep D02–D12 in their current order unless owner feedback or a concrete dependency changes it. Before issuing D02, reconcile the D01 result with this decision register and settle the immediate terminology/control semantics. Do not commission a broad UI rewrite.

| Deliverable | Revision for its future brief |
|---|---|
| D02 | Explain current control sources and target scope consistently; verify navigation/focus. Do not implement composite browsing prematurely |
| D03 | Define lesson/A-B ownership and restoration; diagnostics report graph/measurement facts |
| D04 | Stable parameter identity and source ordering account for future instances without building composites now |
| D05 | Distinguish panic, release, stopped stream, retry and recording continuity |
| D06 | Preserve event source/channel/identity as needed; define routing and recipients through note release. Do not retain unavoidable broadcast as the only future contract. Advanced splits/MPE stay deferred |
| D07 | Define host versus library Save, portable versus device state, final output and independent plugin instances |
| D08 | Automation replacement policy, root clock authority, persistence/reset and seed reproducibility |
| D09 | Explicitly include validated patch-to-component conversion, event/audio boundary routing, two independent instrument instances, browser add/open distinction, instance edit/version save, generic front panel and an effect example |
| D10 | Artwork/layout authoring and portable assets on top of working D09 interfaces; it must not be the first time a reusable instrument is usable |
| D11–D12 | Carry the same interface, lifecycle, identity, dependency and failure rules into code modules |

D09 is now a visibly larger integration milestone than "flatten a graph." Preserve its identity in the roadmap, but before issuing it, estimate against the actual D08 code. If necessary, issue sequential bounded briefs D09a (boundary/conversion proof and minimal instrument) and D09b (reusable-library workflow, editing/versioning and effect coverage), each with its own working outcome. Do not ask one agent to implement an unbounded creator platform.

The original useful stopping points remain: D03 approachable standalone; D08 REAPER production beta; D10 custom-panel creator preview; D12 code creator beta. D08 still does NOT promise arbitrary reusable instruments; if Kosta needs layering saved sounds earlier, propose an explicit reordering after D01 rather than hiding the work inside D02.

## 9. What is decided, recommended and still unknown

**Owner-confirmed:** D01 unchanged; audit later work; retain most of the existing plan; laptop access later; sequential cloud agents.

**Recommended for adoption:** explicit Open/Add/Apply/Save-version actions; logical component I/O; source-aware note routing; explicit root mixing; independent embedded instances; inspectable internals; validated conversion; versioned library definitions; ordinary drag adds only, with a separate Add and connect convenience.

**Product choices to confirm before the affected brief:**
- Adopt the recommended add-only drag plus explicit Add and connect, or prefer a clearly indicated auto-connected insert by default? This changes first-use behavior, not the need for visible connections.
- Is first-generation conversion allowed to require explicit boundary selection for complex patches? Recommended yes; do not promise one-click conversion of any whole composition.
- Keep reusable instruments after the REAPER milestone, as currently planned, or make them an earlier musical priority? Default remains the current order.

**Technical questions for bounded proofs, not owner homework:** event-routing representation, voice/global reduction across a boundary, cross-boundary feedback scheduling, graph identity and seed preservation, exact host snapshot format, and capacity accounting. Require evidence when those batches are scoped; do not label an architectural hypothesis implemented or proven.

## 10. External comparison used for this audit

Primary documentation checked 2026-09-25:

- [VCV Rack menu/file operations](https://vcvrack.com/manual/MenuBar): separates complete patch file operations from editing actions.
- [VCV Rack module presets](https://vcvrack.com/manual/Presets): describes module settings/internal data and limitations of preset-contained storage.
- [VCV Rack Core](https://vcvrack.com/manual/Core): explicitly describes physical-device interface modules. This illustrates why hardware endpoints are a separate concern from reusable processing boundaries.
- [Bitwig containers](https://www.bitwig.com/userguide/latest/container/): defines containers by signal I/O; Instrument Layer combines parallel instruments with per-chain mixing; selectors describe note/tail behavior during switching.

These are interaction precedents, not claims that kabl implements the same designs or that a competitor has solved every problem. The proposed kabl contracts above are our design recommendations.
