# D09 provisional composite design

## Outcome and exclusions

Select a working voice or effect, name a small interface, reuse two independently editable instances, and inspect the internals without losing musical state. Deliver encapsulate, open/inspect, duplicate, expose, edit, undo, publish, insert, save and reload through existing application controls.

No custom DSP runtime, recursive audio engine, panel authoring, marketplace, dynamic host parameter enumeration, general hierarchy editor, or D10 material/visual implementation. D10's package is relevant only for stable bindings, understandable access to internals, and accessible fallback controls. Its recommendation text is not owner selection. Its later OWNER-PREFERENCE records an actual V3/satin preference, but that does not authorize D09 to implement those visuals.

## Chosen engineering representation

Keep the document's existing globally identified leaf modules and cables as the sole mutable graph. Add typed composite metadata: independently identified instances, membership/parent ownership, a stable public interface, name/help and optional library provenance. A composite's internal graph is the subset of those leaf modules and cables owned by its membership tree; it is not a second editable copy.

Compilation validates the ownership tree and produces the existing flat leaf graph for the current compiler. Since insertion has already materialized private leaves, flattening is an identity-preserving projection rather than repeated expansion. All descendant leaves, external cables and internal cables participate together in one schedule. Composite metadata is not a DSP module and must not be sent to registry::info_for as an unknown kind. No composite step, buffer, voice allocator, nested PatchEngine or block adapter is introduced.

This is a real editable composite model: instances own private members, can be collapsed into a public face, have named ports/controls, support nesting and reusable package export/import. It is more than cosmetic selection. A hierarchy whose runtime leaves exist only inside nested definitions was considered, but would force new identities through existing cue/automation/runtime consumers and create competing mutable state. Do not serialize two authoritative copies of the same live internal graph.

Before product edits, prove the proposed projection covers every compiler and runtime-diff entry point. If the repaired D08 contracts require another representation, reconcile this document rather than silently introducing an alternate engine.

## Identity and interface

Use document-local u64 CompositeId in a separate namespace. Persist it; do not use PatchEditor.instance(), a UI widget ID, name, position or compiled index. Every leaf ModuleId and CableId remains unchanged during encapsulation, opening, moving, renaming, interface edits and undo. Duplication allocates a fresh composite tree and disjoint leaf/cable IDs; redo restores the same allocated identities.

An exposed control or port has an opaque stable ID, separate from its editable label and list position. Initial IDs can use bounded monotonically allocated ASCII tokens. Persist the next-ID high-water marks so a removed interface ID is never reused by a different binding. Interface bindings name the existing leaf parameter or port, with kind/type/direction validation. One control aliases one existing base parameter; a musical multi-target control uses an existing macro module and its existing routes. No alternate smoothing, range or modulation model is added.

A public parameter can also have an exposed modulation destination. Its route still targets PortRef::Param on the real leaf. UI resolves a composite endpoint to that leaf when connecting; persisted cables retain exact leaf endpoints and CableIds. Every crossing cable gets a boundary exposure during encapsulation, including routes to hidden controls. Multiple existing routes are preserved, not disconnected by a recreated connect gesture. Signals keep their existing type and input replacement rules.

Rename/reorder keeps IDs and bindings. A binding cannot be silently retargeted. Removing an exposure used by external routing is refused with an explanation; the user can first disconnect or retain that exposure. Deleting a leaf applies existing deletion/reference cleanup and retires invalid exposed entries. Undo restores the group and its references; D08 automation lane retirement remains durable even if Undo brings a deleted leaf back.

Allocate new leaf/cable IDs above identities reserved by current state, undo/redo entries and inverse operations. Existing from_log derives counters only from current live maxima, so D09 must explicitly prevent deleted historical IDs from being reused by imports/duplicates. For cue clock IDs encoded as f32, require exact round-trip representation and fail before edits if a newly allocated reference cannot be represented. Do not introduce a global cue-schema migration just to build composites.

## Workflow through existing UI

Use a bounded selection action in the rack: choose members, then Encapsulate. Stage an instance and auto-expose every crossing endpoint before one undoable transaction. Unselected module placement and all actual cables remain intact. An empty or overlapping selection fails usefully. A composite card uses existing theme/text/control primitives, shows its name, public controls/ports, and an Open internals action. No user panel artwork is needed.

Open internals reveals the original modules in the existing rack with a breadcrumb/return action. Edits use the current PatchEditor, routing, parameter gestures, inspector, macro explanations and pin reveal. A pin or automation target hidden inside a closed composite opens its containing path when revealed. Open/close is view state, not an audio edit or a new history.

Edit this instance is explicit. There is no shared mutable definition behind two instances. Runtime parameter edits stay runtime parameter delivery; internal topology edits use existing compile/swap/deferred destruction. Encapsulation and interface/layout changes should produce Outcome::Nothing for audio delivery when leaves/routes are unchanged. Ordinary project Undo remains chronological and each transaction names its instance. Editing B must never alter A; undoing an A edit must never restore a shared graph used by B. A separate per-instance undo stack is a product alternative recorded in CHOICES, not silently added.

Duplicate copies the current edited instance, not an older library version. Copy internal cables, params, labels, child groups and public IDs/bindings with a full local-to-document identity remap. Original external cables and host lanes stay assigned to the original. New instance appears unpatched at its boundary. Internal cue/launch references are remapped; reference-only dependencies outside the selection are reported before insertion and require an explicit binding or inclusion. Do not accidentally retain foreign document IDs.

Pins can be copied as new pins with deterministic order; MIDI CC/button mappings are omitted from the new copy to avoid one controller unintentionally editing both. This is a visible duplication policy and a pending product default, not a compatibility change to existing mappings.

## Embedded recall and immutable library versions

A saved song embeds its exact graph, grouping, public interface, values/help and version provenance. The live embedded definition is represented by owned leaves plus composite metadata in that same document, not a path or an external mutable library entry. Editing changes only that embedded instance. Library provenance identifies its origin; edited instances are visibly marked modified relative to that version.

Publish a new version snapshots the current instance into a detached package with template-local identities, graph, child ownership/interface metadata, documentation, schema and definition/version identity. It writes an immutable new library entry using the existing stage/read-back/rename save pattern. Publishing does not mutate sibling instances or other songs, does not overwrite an earlier version and is not reversed by document Undo. Insertion materializes a private copy with fresh IDs. Automatic propagation and live links are excluded.

Use a small JSON package with embedded built-in graph and nested metadata, not a filesystem archive or loader executable. Built-in kinds are dependencies. A project does not consult the library during recall. Missing/updated library copies therefore have no effect. An unknown built-in kind, unsupported schema or genuinely incomplete embedded graph fails with the exact dependency/path/version and leaves the prior document/audio state intact. No fallback substitution or partial sound is called successful recall.

## Nesting, cycles and resources

Proposed first bound: eight composite levels, counting the outermost as level one; at most 128 composite nodes, 32 public ports and 32 public controls per instance. Public IDs and names are bounded; documentation uses the existing 4096-byte text ceiling. These are engineering limits, not hardware-performance promises. Apply D08's existing flattened graph/effect/serialized-size limits to all leaves and metadata together: 128 modules, 512 cables, eight delay/reverb/chorus modules, and the existing approximately 2 MiB patch cap. Do not raise the plugin cap to accommodate redundant templates.

Nested ownership is a forest: no self-parent, multi-parent ownership, overlapping sibling leaves, dangling child, or parent cycle. Packages carry complete definitions, not unresolved live library links. Their definition ancestry is checked by exact definition/version identity before materialization: A contains A, or A contains B contains A, is rejected with the ancestry path. Sibling reuse of A is allowed. The traversal visits a bounded number of nodes and refuses depth/resource overflow before mutation. Bounded JSON depth and input byte limits precede deserialization; duplicate map keys/IDs must be rejected rather than overwritten silently by a map decoder.

Definition recursion is different from a signal feedback loop. Existing signal cycles retain deterministic DFS back-edge selection and one 64-frame feedback delay. Grouping must not reorder IDs or cable iteration and must not reject legal signal cycles. Voices still derive from built-in rates, MIDI reachability, mono/noise rules and voice averaging across the entire flattened graph. A composite is not intrinsically mono or polyphonic, and grouping adds zero latency. D08's existing reported 64-frame plugin latency remains unchanged.

## Atomicity and persistence integration

Add typed operations for composite metadata with exact inverse behavior, default-empty PatchState fields and an intentional core schema increment. Record the Op vocabulary extension in decisions.md. Carry metadata through seed_from, from_log, save/replay, compare::diff/restore, browser and host SoundState; storing it only in checkpoint.json would lose it because core load replays log.jsonl.

Before accepting a package or encapsulation transaction, decode/validate a complete candidate, reserve/remap identities, resolve dependencies/interfaces/references, check limits and compile where sound changes. Only then append one Group of leaf and composite operations. A failed candidate changes no document, history, ID counter, automation bank, library or playing graph. Publishing stages and validates before committing a new library entry; errors retain the previous entry. Reuse D08's atomic state/session publication for host recall, including full-queue rejection and epoch isolation. No library IO, validation, recursive traversal or definition lookup enters the callback.

Legacy schema 1–3 patches and D07/D08 state envelopes must load unchanged with an empty composite collection. New composite-bearing envelopes must be explicitly versioned so older state versions cannot smuggle unsupported fields. The exact envelope migration is finalized after repaired D08 merges. Host state continues to store sound state rather than a complete editing log; standalone saved applied history survives using the existing log mechanism. Do not promise host-restart undo history that D08 does not currently preserve.
