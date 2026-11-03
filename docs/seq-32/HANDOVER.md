# Sequencer to 32 steps: handover

## Task (condensed, from "Kabl overseer")

Branch `claude/seq-32` from origin/master (719bcf6; rebase or merge after #63 merges). Never merge, never git stash.
1. Engine (`crates/modules/src/builtins/seq.rs`): `STEPS` 8 -> 32 per pattern (banks A-D), `length` any integer 1..32, pitch + gate (and velocity, chance, ratchet) per step. Old patches load bit-identical: new steps default off, length/behaviour unchanged for existing values; golden/legacy/factory hash files untouched.
2. Host exposure (Kosta's rule): sequencer steps are patch data, NOT DAW-automatable. The host sees macros and existing global controls only. Verify how kabl-clap exposes params; if seq steps are exposed today, report it and keep the NEW steps out; do not change existing exposure without asking. Plugin state round-trip (REAPER save/reopen) must stay identical.
3. UI: minimal paging on the seq face (pages of 8 steps, page selector), no redesign. UI tests.
4. Tests: length 1..32, pattern switch, polymeter lengths (5, 7, 13, 21, 31), state carry.
5. Then restore the intended 16-step lines in every groove generator (`crates/ui/examples/presets_groove.rs`, group 1 + 2 + any preset with p9..p16 data silently dropped; grep the other `presets_*.rs`) and re-render; that changes sound, so produce a before/after tour per affected groove. Needs #63 (groove presets) merged or merge `claude/presets-groove-2` in.
Lint + full tests (debug), draft PR (overseer opens if denied), `cargo clean`, checkpoint, SendMessage "Kabl overseer". Out of scope: visual redesign, the drum block-edge click.

## Done

Lane rule done (Kosta decided: no per-step seq param is ever a lane target; `seq::is_step_param` + guard in `automation::resolve` + test). No shipped patch/fixture/golden targets a seq lane (checked docs evidence JSON, host_tests, composite fixture). Stored `length` is 16 in 14 groove logs, 8 in 2: raising max to 32 makes 9-16 silent until step 6 restores them. Engine/UI/generators still todo.

## Facts found

- seq.rs: `STEPS = 8`, `BANKS = 4`. Per bank 43 slots: p1-8, g1-8, length, (transpose in bank A only, after length), v1-8, gate_len, gate_mode, r1-8, then k1-8 ratchets stored after everything. `OLD_SLOTS = 35`, `S_RATCH = 35`, `BANK_B = 38`, `RATCH_BASE = 143`, total params 175. Index layout is fixed (`param_index(bank, slot)`); never move old indices.
- `process()` builds `Bank { pitches/gates/velocities/probs/ratchets: [_; STEPS] }` per block from `io.param`; `length` clamps `1..=STEPS`; `load_state` clamps step to `STEPS-1`; `Seq::new` parks on `STEPS-1`. `ramped()` in `engine/src/runtime.rs` already excludes kind "seq" from ramping.
- Plan: append 480 new params after index 175: per bank p9..p32, g9..g32, v9..v32, r9..r32, k9..k32 (names continue the old scheme, `b.`/`c.`/`d.` prefixes). Defaults: pitch 0, gate 0 (off), v 100, r 100, k 1. `length` max 32 (default stays 8). Extend `bank_of`/`slot_name`/`cleared`/`SLOTS` (used by `ui/src/banks.rs` bank copy/clear, `ui/src/rack.rs` face names and visibility, `ui/src/help.rs`, tests) so slots cover the new steps; keep numeric slots 0..43 stable (tests/composition.rs, sound_palette.rs, support/factory_controls.rs use `bank_param(b, 8 + k)` etc.).
- CHECK BEFORE raising `length` max: groove generators set `length` 16; look at `patches/presets/*/log.jsonl` (whether 16 or the clamped 8 is stored). A stored 16 would change sound once max is 32 (steps 9-16 default off = silent second half).
- CLAP host exposure (crates/clap/src/automation.rs): the host sees a fixed set (count asserted 18 in `state_bridge.rs` test: 16 automation lanes `SLOTS = 16` + others), not one param per module param. A lane may target any module param that is `ParamClass::Runtime` and not `Taper::Stepped` (`resolve`, `candidates`). So today seq pitches (p), velocities (v), chance (r), gate_len and transpose ARE lane candidates; gates, length, ratchets (stepped) are not. Decision for new steps: exclude them in `resolve`/`candidates` (kind "seq" and param index >= 175), leave existing candidates alone, report param count before/after (host count stays 18; candidate list only grows for the old params, unchanged). Verify with a clap test + REAPER save/reopen (`docs/presets/scripts/host.py`, `PH_SELF=1`).
- UI: seq face is generic (`rack.rs` lays out `info.params` filtered by `visible(info, name, edit_bank)`); edit bank lives in `ui_state.edit_bank` (lib.rs:221, `edit_bank_of`, set at lib.rs:748/1360/2702/3290, cleared in browser.rs:485, snapshotted in explain.rs:90/177/218, `edit_view` in rack_editor.rs:428). Add `edit_page` the same way and filter `visible` by page (page-local step positions); page selector as a select control.
- Release builds are slow after `cargo clean` (~9 min); debug library tests ~200 s. Run long things in the background with output to a file.

## Remaining, in order

1. Check stored `length` in groove patches (above). 2. Engine change + engine tests (length 1..32, bank switch, polymeter 5/7/13/21/31, carry_from/state), keep old tests green; golden/legacy hash tests untouched. 3. `automation.rs` exclusion + clap test. 4. UI paging + tests. 5. help.rs text ("1-8" -> "1-32"), explain. 6. Restore 16-step lines in generators, regenerate, before/after tours (needs #63). 7. Lint, full tests, REAPER save/reopen, draft PR, cargo clean, report.

## Working assumptions / open

- Decided: ALL per-step params hidden from lanes. Host count stays 18.
- Exposing 32 steps per pattern to the host: NOT doing it.

## Standing rules

Report to "Kabl overseer"; do not merge; no git stash; Caveman + Ponytail ultra in chat, normal English in code/commits/docs; `cargo clean` when done.

## Update (final)
Engine 32 steps, paging, lane exclusion, compile cap 655, clippy/tests green. Groove restore needed no generator edit: logs already hold p9..p16/g9..g16 + length 16 (the old engine clamped to 8). 13 grooves change (brick-dust, copper-mile, corner-store, dune-buggy, fault-line, glass-coast, lamp-oil, ore-cart, rope-walk, scrap-metal, slab-city, still-water, stoop-sale); Acid Line (length 8) renders byte-identical. Tour: docs/seq-32/groove-before-after-tour.{mp3,txt} (FLAC: /secondary/Programming/Github/kabl-tours/). Not done: REAPER save/reopen, level/peak check of the 13 changed grooves (factory_bank tests pass).
