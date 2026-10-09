# Checkpoint: sound-engines-2 (stacked on PR #17), work in progress

Written when the user asked to stop and restart with fresh context. Branch `claude/sound-engines-2`,
worktree `/secondary/Programming/Github/kabl/.worktrees/sound-engines-2`, based on `claude/sound-engines`
(`d7980f5`, PR #17). No PR opened yet. Do not rebase onto the cables branch; do not add commits to PR #17.

## Task (from Kabl overseer, relaying Kosta)

Make FM/wavetable complete enough for a 100+ preset bank. Scope: (1) practical multi-operator FM
(decide composite vs dedicated module, record in docs/decisions.md); (2) correct timbre in chains deeper
than two operators, tested against an analytic spectrum at several pitches; (3) 2x default, opt-in 4x per
module, measure cost and aliasing, append to docs/benchmarks.md; (4) display data for the future UI (current
table, position, one output cycle) via the existing signal-inspection path, no audio-thread alloc/lock;
(5) 3-4 more demo patches with clips (4+ operator EP and bell, an enveloped wavetable sweep pad) in
`patches/sound-engines/demos/`. Constraints: PR #17 patches/state sound identical (proved by test), no
crates/ui/src beyond registration, minimal engine/core, verify in plugin + REAPER with the existing scripts
(`docs/sound-engines/scripts/host.py`, `validator_diff.py`), workspace tests + strict clippy, short comments.
Finish with one report to "Kabl overseer" (PR link, structure chosen, measurements, verification, unverified,
clips, decisions), open a separate draft PR, do not merge, then stop. Leave to Kosta: demos as factory sounds,
and (now partly in scope) 4x / deeper-chain decisions.

## Decision taken (not yet written into docs/decisions.md)

Dedicated module `osc.fm6`, not a composite. A composite is only grouping of leaf `osc.fm` modules in one flat
graph, so every hop keeps its 10-sample latency. Inside one module all six operators run at the internal
rate and are filtered down once, so operators meet with zero latency at any chain depth (item 2 solved by
structure) and an operator costs one sine per internal sample. `osc.fm` stays for custom wiring (its limits
documented). 8 algorithms, key-sync phases on note-on, per-operator ratio/fine/level/ADSR/velocity.

## Done and committed

- `70a0aaf` golden renders: `crates/engine/tests/golden_renders.{rs,txt}` hashes of 26 committed patches,
  recorded with PR #17's build; passes on this branch after the changes below (checked after osc.fm edits).

## Done in the working tree (uncommitted until this checkpoint commit; builds were in progress)

- `crates/modules/src/view.rs`: `ModuleView` (valid, position, 128-point cycle), `Ring` (4096 samples) with
  `cycle()`; `Module::view` default no-op in module.rs; re-exported in lib.rs.
- `crates/modules/src/fmdsp.rs`: shared `lowpass::<N>(factor)` (bit-identical to the old 2x filter) and
  `weights4()` (6-point Lagrange quarter points).
- `osc.wt`: ring, `view` (position + cycle), `carry_from` copies ring.
- `osc.fm`: new last param `oversample` (0 = 2x default, 1 = 4x), 4x path (65 taps, quarter-point pm
  interpolation, HOP stays 10), ring + `view`. 2x unchanged: golden test and fm tests passed.
- `osc.fm6` (`osc_fm6.rs`, registered in mod.rs/registry.rs): 6 ops, ports pitch/gate/velocity/out, params
  base_hz, algorithm, feedback, index, fine, oversample, then ratioN fineN levelN attackN decayN sustainN
  releaseN velN (N=1..6). Compiles; NOT yet tested at all.
- Engine: `probe.rs` `ProbeReport.view`, `compile.rs` fills it from the loudest voice at window end,
  `runtime.rs` structural (`oversample`, `algorithm`) and module-smoothed (`osc.fm6` level*/index/feedback)
  entries. `crates/ui/tests/signal_inspection.rs` literal got `view: Default::default()`.
  A `cargo build --workspace --tests` was running when stopped; result unknown.

## Still to do (in order)

1. Check the workspace builds; fix anything. Run `cargo test --workspace --no-fail-fast`.
2. UI registration only (crates/ui/src): `help.rs` param/port help for `osc.fm6` (all 54 params: use pattern
   arms), `osc.fm` `oversample`; `routing.rs` short_name ("FM6"), step labels (`ALGORITHM_LABELS`,
   `RATIO_LABELS` for `ratio1..6`, `oversample` "2X"/"4X"); `theme.rs` colour group.
3. Wavetable display data: `wavetable::factory_frames(i)`; document that user tables decode with
   `decode_canonical`.
4. Tests: `crates/modules/tests/fm6.rs` (analytic spectrum vs an independent f64 evaluation of each algorithm at
   130.8/523.3/1046.5 Hz incl. 3+ deep chains and key sync; velocity; envelopes; gate; silence cost
   shortcut; NaN; carry_from; state), 4x vs 2x aliasing table, `view` tests (osc.wt/osc.fm/osc.fm6),
   engine probe test that `ProbeReport.view` arrives with a position and a cycle and allocates nothing,
   no-alloc test with osc.fm6 on 8 voices.
5. Benchmarks: add osc.fm6 and 4x rows to `crates/modules/examples/bench_sources.rs`; append numbers to
   docs/benchmarks.md; aliasing 2x vs 4x table (extend `print_alias_table`).
6. Demos (builders in `crates/ui/tests/sound_engines.rs`, written to `patches/sound-engines/demos/`, clips to
   `docs/sound-engines/audio/` via ffmpeg m4a): 4-op+ EP, 5-op+ bell, enveloped wavetable sweep pad, optionally
   an FM bass. Check levels (peak < 0.98) like the PR #17 demos; `.gitignore` excludes `*.wav` (use `git add -f`
   for any needed wav; the factory tables already have an exception).
7. Docs: docs/decisions.md entry (composite rejected, why), docs/sound-engines/README.md section, display-data
   contract.
8. REAPER + validator pass with the existing scripts (copy `scratch/clap-validator` and
   `scratch/baseline-libkabl_clap.so` from the sound-engines worktree's scratch/ if missing; they are
   gitignored), include an osc.fm6 track; update host.py fixture for osc.fm6 and the 4x option.
9. `cargo clippy --workspace --all-targets -- -D warnings`; rustfmt only on files in this diff (master has 17
   unrelated fmt diffs); push; open a separate draft PR against master; report once to "Kabl overseer".

## Tooling notes

- Bash tool in this worktree session refuses compound shell with globs/loops/heredoc redirects; use Write/Edit,
  `python3 - <<'PYEOF'` edits, one plain command per call. `gh pr edit` fails (classic projects); use
  `gh api -X PATCH repos/stcksmsh/kabl/pulls/N -F body=@file`.
- Peer "user-login" is not reachable; the reporting peer is "Kabl overseer".
