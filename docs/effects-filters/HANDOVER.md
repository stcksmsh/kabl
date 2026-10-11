# Effects and filters batch: handover

Worktree `.worktrees/sound-engines-2`, branch `claude/effects` (from origin/master, pushed). Style: Caveman and
Ponytail at ultra in chat; code, commits, PR text and docs in normal English. Report to the session named
"Kabl overseer"; do not merge; stop when done.

## Task as received (condensed)

Build for 1.0, following the conventions of chorus, drive, filter.ladder and the utilities: phaser, flanger,
compressor (sidechain in, gain-reduction out), EQ (decide the shape), limiter (lookahead only if latency
reporting works end to end), bitcrusher, and comb (pitch-tracked, +/- feedback), formant (vowel morph) and notch
filters (decide: mode of an SVF or its own module). Audio-rate modulation of every time-varying or cutoff-like
param without zipper noise; stereo like chorus/delay. Out of scope: live displays, presets, changes to existing
module sound, oversampling beyond what exists, D11. Constraints: no allocation or lock on the audio thread;
numerically safe at extremes, 44.1-192 kHz, block 1 to large; saved patches and the factory bank load unchanged
(no schema bump; if one seems needed, stop and tell the overseer); faces basic, small and local to the new
modules (agent 1 reworks faces on claude/faces, draft PR #24; merge origin/master after it lands); CI green on
the final head (fmt, strict clippy, tests, package, validator); check in a real host (standalone and the CLAP
plugin in REAPER). One clip per module (dry then wet) with a one-line "listen for". Re-render a clip after any
fix that could change it. Report: what was built, decisions, tests and totals, CPU per module on the existing
bench, clip paths, CI run link on the final head. Open a separate draft PR to master (push works from here).

## Done

- `1f36ed3` (pushed): 8 modules in `crates/modules/src/builtins/` (`phaser`, `flanger`, `compressor`, `eq`,
  `limiter`, `bitcrusher`, `filter_comb`, `filter_formant`; shared helpers in `fx.rs`), `notch` output added to
  `filter.svf`, registry (42 kinds), UI name/section/help/inspect entries, `smoothed_by_module`, clap tail list,
  `crates/modules/tests/effects.rs` (21 tests, all pass), dev-dep `assert_no_alloc` in kabl-modules,
  `docs/effects-filters/scripts/make_demos.py` and 9 clips in `docs/effects-filters/audio/*.m4a`.
- Workspace `cargo test` showed no failures; `cargo fmt --check` and `cargo clippy --workspace --all-targets -D warnings` clean.

## Done since (second session, committed in the "wip: faces, CPU, host evidence" commit)

- PR: draft https://github.com/stcksmsh/kabl/pull/25 (opened, CI not yet looked at).
- Faces (item 1): `docs/effects-filters/scripts/{face_patches.py,capture_faces.sh}`, light captures in
  `docs/effects-filters/faces/<kind>/light.png` (dark theme step fails: "no target theme:dark", same script as
  sound-engines; light only). Fixes: EQ knob labels were ambiguous ("Low, Low, Mid1, Mid1...") -> explicit labels in
  `crates/ui/src/routing.rs::param_label` ("Low freq", "Low gain", "Mid1 freq", ...); `filter.formant` title
  "Formant Filter" overlapped the +1 badge -> renamed "Formant" in info. Recaptured after the fix; the
  recaptured eq and formant PNGs have NOT been looked at yet. SVF face shows the Notch jack fine. Bitcrusher
  downsample value reads "4X" (unit upper-cased by fmt_value); left alone.
- CPU (item 2): `bench_palette_parts` does not enumerate kinds; added ignored test `cpu_per_module` in
  `crates/modules/tests/effects.rs` (`cargo test --release -p kabl-modules --test effects cpu_per_module -- --ignored --nocapture`).
  The test file's assert_no_alloc use is now `cfg(debug_assertions)` (the crate compiles to nothing in release).
  ns/sample at 48 kHz, block 256, noise in: phaser 82.5, flanger 123.9, compressor 14.9, eq 41.3, limiter 12.4,
  bitcrusher 9.9, filter.comb 44.4, filter.formant 14.3; references filter.svf 20.2, filter.ladder 60.7, chorus 64.1
  (0.047 % to 0.595 % of one core).
- Host (item 3): REAPER: `docs/effects-filters/scripts/host_effects.py` (needs `cargo build --release -p kabl-clap`,
  `-p kabl-engine --example patch_json` (new, `crates/engine/examples/patch_json.rs`), demos from make_demos.py,
  `SE_BASELINE_PLUGIN=scratch/baseline-master-libkabl_clap.so`). Result in `docs/effects-filters/evidence/host/result.json`:
  9 tracks load; states identical across save/quit/reopen/save; reopened render bit-identical; wet finite, peak 0.9895,
  rms 0.305, wet-vs-dry rel. diff 0.21; old factory-bank project state identical and render bit-identical to the
  baseline build. Standalone: `scripts/standalone.sh` opens each wet demo in kabl-ui, 10 s, audio device opened,
  real-time priority granted, screenshots in `evidence/standalone/<name>-wet/running.png`, logs beside them.
  Gotcha: pick an Xvfb display number other sessions do not use (96 collided; 143/144 fine).

## Remaining, in order

1. Look at the recaptured `faces/eq/light.png` and `faces/filter.formant/light.png`; if fine, done with faces.
4. Docs: `docs/effects-filters/README.md` (per module: ports, params, method, measured numbers, "listen for" line)
   and a "Effects and filters" entry in `docs/decisions.md`; link from `docs/STATUS.md` if the sound-engines entry is there.
5. After faces PR #24 lands: `git merge origin/master`, rerun tests; re-render clips if a sound-affecting fix landed.
6. Open the draft PR (title "Effects and filters: ..."), wait for CI, record the run link, run `packaging/linux/package.sh`
   and the validator as CI does if CI does not cover them. Send the final report to "Kabl overseer".

## Decisions (put in README, decisions.md and the report)

- EQ: low shelf, two bells (freq/gain/Q), high shelf; cut filters stay `filter.svf`/`filter.ladder`. TPT state-variable
  bands (Simper), not cookbook biquads: the biquad blew up under random per-sample modulation in the test.
- Notch: a fourth output `notch` (lp+hp) on `filter.svf`, not a module: the SVF already computes both halves. Guarded
  by `io.output_count() > 3` because hand-wired tests supply three buffers.
- Limiter: no lookahead. Latency to the host is one fixed constant (`timeline::LATENCY`, `set_latency_samples` in
  `crates/clap/src/lib.rs`); no per-patch latency reporting. Instant attack, final clamp to the ceiling.
- Compressor: unpatched `sc` is a held scalar, so it uses the internal detector; `gr` out is dB of reduction (>= 0).
- Param smoothing: the new kinds are in `runtime::smoothed_by_module` (module glides scalar edits, follows cables as buffers).

## Facts that cost effort

- `Rig` in `tests/common` is fixed at block 64 and scalar params; `effects.rs` has its own `Dut` driver.
- Patch dirs for `render_midi` need `meta.toml` (`schema_version = 5`) + `log.jsonl`; `make_demos.py` writes them to `scratch/` (git-ignored).
  Renders the clock-played music with an empty events file and `--tail 7`.
- `render_midi` needs `cargo build --release -p kabl-engine --example render_midi` first; ffmpeg encodes m4a.
- `cargo test --workspace` takes over 2 minutes: run it in the background. Vendor crate warnings are noise; grep for `^error`.
- Comb pitch is measured by echo centroid (`loop_period` in effects.rs), exact to 0.00 cents; autocorrelation was biased.

## Open for Kosta

Listening is his call (nothing here has been heard). Clip levels were set by peak only: dry and wet are not loudness-matched.
