# Stereo 2a+2b handover

Branch `claude/stereo-2a` (worktree `.worktrees/widget-set`), PR #56. Report to session "Kabl overseer". Never merge. Stop when done.

## Task (condensed)
Stereo steps 2a (width-2 ports, `split`/`join`, schema 8) and 2b (migration v7 to v8) per `docs/stereo/DESIGN.md` section 7, as one PR.
Constraint: bit-identical. `legacy_patches`, `golden_renders`, `factory_levels` pass with the hash files untouched.

## Done (steps 1 to 4 complete)
- 2a: `PortInfo.width`, width-2 ports on chorus/reverb/fx/out/audio.in/delay/pan/vocoder/compressor, `split` (voice rate) and
  `join` (global rate) in `modules/src/builtins/wiring.rs`, aliased in `compile.rs`; schema 8.
- 2b: `crates/core/src/migrate.rs` (`migrate_state`, `migrate_v7_to_v8`, one Group op ending in Annotate "migrated to schema 8"),
  undo floor in `log.rs`, hook in `format::load`, composite exposure rename, `crates/core/examples/rewrite_v8.rs`.
- `patches/**` and `docs/**` rewritten with the script (commit 6adbd96); `crates/ui/examples` and `tests/library.rs` updated.
- Plugin: `SoundState` version 5, `migrate_state` in `decode` for versions below 5.
- composite_demo panic ("missing interface target in_l") was a stale example (old half-port names), not an engine bug. Fixed in the
  example; regenerated `docs/composites/examples/**`. Master runs the example fine.
- REAPER check (offline render, `kabl_agents` sink): a project saved by the master plugin and a v4 fixture both load in the #56
  plugin, render sample-identical to master (max diff 0.0), save as state v5, and reopen unchanged (state identical).
- Lint, full tests, and every `crates/ui/examples` binary run once.

## Remaining: stereo steps 3 to 5 (not in this PR)
3. UI jacks and cables: stereo jacks, split/join legs in the cable UI (after visual stage 2).
4. `osc.va` width, pan and widener.
5. Demo, REAPER evidence and benchmarks (stereo +39 to 48 % CPU measured; Pi 4 unmeasured).
Delay input stays mono (C11).

## Facts
- A width-2 port is two consecutive io slots; tap/probe numbers are slots.
- `compile.rs`: `port_slots`, `stereo_pair`, split/join aliasing just before `lane_outputs`.
- Builders in tests keep old names and end with `PatchEditor::migrated()`.
- Master plugin saves the lowest state version that fits (2), not 4; #56 always saves 5.
- Wav byte compare differs in the header only; compare sample data.
- Run audio checks with `PIPEWIRE_NODE=kabl_agents PULSE_SINK=kabl_agents`.

## Decisions for Kosta
- Migration keeps functional-cable pairs as split/join legs (C12).
- `checkpoint.json` regenerates through `save`.
