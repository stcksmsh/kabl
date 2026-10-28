# Stereo 2a+2b handover

Branch `claude/stereo-2a` (worktree `.worktrees/widget-set`). Report to session "Kabl overseer". Never merge. Stop when done.

## Task (condensed)
Stereo step 2a per `docs/stereo/DESIGN.md` section 7 (Kosta: option c, C1-C13 accepted, every L/R pair becomes one stereo jack), and
now also 2b (migration), as ONE PR, green at the end. Bit-identical: `legacy_patches`, `golden_renders`, `factory_levels`
pass with hash files untouched. Out of scope until overseer says PR #53 merged: edits to `crates/ui/examples/*`, `patches/**`,
`crates/ui/tests/library.rs` and running the rewrite script over `patches/**`. Write the script, do not run it there.
Lint (fmt --check, clippy --workspace --all-targets -D warnings) + tests before push. Draft PR. `cargo clean` at the end.

## Done
- 8665990 and later (branch pushed): `PortInfo.width`, width-2 ports, `split` (voice rate) / `join` (global rate, so a voice source is
  summed into it exactly as into the old half port) in `modules/src/builtins/wiring.rs`, aliased in `compile.rs`; schema 8.
- 2b in `crates/core/src/migrate.rs`: `migrate_state`, `migrate_v7_to_v8` (one Group op ending in Annotate "migrated to schema 8"),
  undo floor in `log.rs` (also restored on load from that marker), hook in `format::load`, composite exposure rename,
  `crates/core/examples/rewrite_v8.rs` (NOT yet run on patches/**; dry run on a copy: 110 folders, idempotent),
  clap `SoundState` version 5 + `migrate_state` in `decode`, fixtures `crates/core/tests/migrate.rs`.
- `legacy_patches`, `golden_renders`, factory_levels pass with hash files untouched (loader migrates unmodified patches/**).
- All of `kabl-engine`, `kabl-core`, `kabl-modules`, `kabl-clap` tests pass.

## Remaining, in order
1. `kabl-ui` tests: 24 fail (log: run `cargo test -p kabl-ui --no-fail-fast`). Causes:
   a. "committed_*_match_builders" (composition, echo, interlocking, performance, expressive, sound_engines, sound_palette, arp_demos,
      drum_demos, library init-keyboard/recipes): the test builders (Rack/PatchEditor) must equal the committed files. My regex rename
      of old port names in these files is WRONG for builders. Plan: `git checkout origin/master -- ` those ui/tests builder files (and
      `ui/src/recipes.rs`), keep the old names in builders, and compare/compile `migrated(builder)` (add a small
      `kabl_core::migrate_state` call on the built `PatchState`), so builder == script output by construction.
      Needs a one-line public editor helper or `migrate_state(&mut state.clone())` at each comparison/compile site.
   b. Source misfires of my rename (`delay`/`duck`/`chorus` `left`/`right` became `in`): arp_demos (fixed), drum_demos (fixed),
      echo.rs:70-71, sound_palette.rs:858-861 (use a `split`), signal_inspection.rs (cable_to ... "in" returns None), composites.rs:276
      (pan "out"?), runtime_controls.rs:251 (left 2 right 3).
2. After overseer says #53 merged: rebase, update `crates/ui/examples` (same migrate approach or v8 names), `library.rs`, run
   `cargo run -p kabl-core --example rewrite_v8 -- patches`, check `git diff --stat patches`, tests.
3. `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, push, draft PR, `cargo clean`.
4. Report to overseer: audit list (modules: chorus reverb eq flanger phaser limiter bitcrusher vocoder compressor out audio.in delay pan),
   what 2b left, known non-bit-identical case: none found (all shipped patches identical).

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
