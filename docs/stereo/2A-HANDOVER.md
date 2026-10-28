# Stereo 2a+2b handover

Branch `claude/stereo-2a` (worktree `.worktrees/widget-set`). Report to session "Kabl overseer". Never merge. Stop when done.

## Task (condensed)
Stereo step 2a per `docs/stereo/DESIGN.md` section 7 (Kosta: option c, C1-C13 accepted, every L/R pair becomes one stereo jack), and
now also 2b (migration), as ONE PR, green at the end. Bit-identical: `legacy_patches`, `golden_renders`, `factory_levels`
pass with hash files untouched. Out of scope until overseer says PR #53 merged: edits to `crates/ui/examples/*`, `patches/**`,
`crates/ui/tests/library.rs` and running the rewrite script over `patches/**`. Write the script, do not run it there.
Lint (fmt --check, clippy --workspace --all-targets -D warnings) + tests before push. Draft PR. `cargo clean` at the end.

## Done
- 8665990: `PortInfo.width`, `ModuleInfo::{slots,slot_port,port_slot}`; width-2 `in`/`out` on chorus, reverb, eq, flanger, phaser,
  limiter, bitcrusher, vocoder (in), compressor, `out` (in), audio.in (out), delay (out), pan (out, in `crossfade.rs`);
  `split`/`join` (`modules/src/builtins/wiring.rs`) aliased in `compile.rs` (no instance, no step); step-1 aliases removed;
  schema 8. Script `rename` of old names in tests (many sites wrong, see Remaining).
- Audit: no other L/R pairs in `crates/modules`.

## Remaining, in order
1. 2b: `crates/core` `migrate_state(&mut PatchState)` (rules DESIGN section 4; legs through split/join for functional cables),
   `migrate_v7_to_v8` on `PatchLog` (ops appended, floor/undo barrier), hook in `format::load` for `schema_version < 8`,
   composite inner graphs + exposed ports, clap `SoundState` version 5 + `decode` calling `migrate_state`.
2. One fixture per edge case (only L, different sources, crossed, CV use, functional cable pair, cycle) in `crates/core/tests`.
3. Rewrite script (example in `crates/core/examples/` or `crates/engine/examples/`): load via `format::load`, `save`; do not run on `patches/**`.
4. Test sites that wire left and right from DIFFERENT sources or one side only (was mechanical-renamed wrongly to the same `in`):
   engine/tests {cable_routes, drum_clips, drums:56, echo, facetap, keyboard, launch, live_edit, sound_engines, sound_palette,
   utilities, voice_allocator, functional_cables:687, cue_macros}, ui/tests {arp_demos, composites, drum_demos, echo, expressive,
   interaction, interlocking, performance, signal_inspection, sound_engines, sound_palette, support/factory_bank}.
   Insert a `join` (`left`,`right` in, `out`) or `split`. Grep `STEREO-FIX` for the mono-destination sites.
5. After overseer says #53 merged: rebase, update `crates/ui/examples`, `library.rs`, run script, regenerate `checkpoint.json` or delete.
6. Lint, tests, push, draft PR, `cargo clean`, report CHECKPOINT/result to overseer.

## Facts
- Engine slots: a width-2 port = two consecutive io slots; tap/probe port numbers are slots (unchanged numbers).
- `compile.rs`: `port_slots`, `stereo_pair` (first width-2 port), split/join aliasing just before `lane_outputs`.
- Modules tests helpers use `info.slots(..)`, not `ports.len()`.
- Tests that read `patches/**`: arp, cable_routes, clock_position, timeline, probe, functional_morph, golden_renders, ui perform/cues/etc.
  They fail with `UnknownPort ... left` until migration lands; use them as the migration proof.
- Logs: `/home/stcksmsh/.claude-alt/jobs/85818718/tmp/` (not durable).
- Audio: run tests with `PIPEWIRE_NODE=kabl_agents PULSE_SINK=kabl_agents`.

## Decisions for Kosta
- Migration of a cable pair with functional settings keeps legs via split/join (C12).
- checkpoint.json: regenerate through `save` (proposed).
