# Faces and live displays (task C): handover

Branch `claude/faces` (from origin/master, merged with master after PR #22 fmt and #23 arp). Worktree `.worktrees/widget-set`. Draft PR #24. Report to "Kabl overseer", do not merge, stop when done. Caveman/Ponytail ultra in chat; code, commits, PR text normal English. Stay under 50% context.

## Task as received (condensed)
Polish module faces and give them live displays (bar: Vital / Arturia Pigments). Face polish for every built-in module (incl. nine utilities, arp, seq ratchet drawer); live displays (oscillators, filters, env, lfo; delay/reverb only if honest); modulated knobs show range and moving value; displays need room (jacks lower: **accepted by Kosta**, no longer an assumption). Behaviour unchanged, tests updated only for pixel positions. Frame time dense Composition 1440x900 with displays on: median < 8 ms, max < 16 ms (defect otherwise). Nothing on the audio thread allocates or locks; engine addition minimal and tested. Plugin editor 1180x680 and low zoom; verify in a real host. Out of scope: dense-patch legibility, cable management, wavetable editor/import, cable pattern editor restyle, theme B and loader. Before final push: `gh pr view 22/23` (merged), fmt, merge master, make the new CI pass. Final report to "Kabl overseer": captures and clip paths, bench (median, p95, max), test totals, CI run link on the final head. Do not merge.

## Done (commits on claude/faces)
- `b9596af` face polish: theme-driven faces (`theme::theme/panel_theme(&Style)`), `kit/face.rs` recipes (surface, hardware, title, knob body/scale/marker, `mod_range`, `live_dot`, jack with signal rim, `readout` fitting its slot, display frame/trace/playhead), `rack.rs` layout (jacks lower `JACK_Y` 296 / `IN_ROW_Y` 248, `Decor::Display` above the knobs, fm6 rows respaced, scope in fm6 header), static displays in `displays.rs`.
- `3783a81` engine face taps `crates/engine/src/facetap.rs` (up to 16 outputs, loudest-instance last/min/max, optional `view` as i8 cycle, window = half the probe window), `Command::Faces`, `Module::view` for `lfo` (phase) and `env.adsr` (stage). Tests `crates/engine/tests/facetap.rs` (levels, found/not found, views, same output taps on/off, no allocation across swaps), `crates/modules/tests/view.rs`.
- `20aae02` fmt, `7343e62` merge master. Later commits: host plumbing (standalone ring in `main.rs`, CLAP `RuntimeReport::Faces`), `displays::Wants` (visible displays + route sources, sent as `Command::Faces`, resent every 2 s, none when audio is off), `Live::accept`, knob live dot, bench live mode, clippy allowances, no spurious start command. Latest commits after the checkpoint: HIDE_BELOW 9.5, long module names step down a size, title fix, `bench_composition` face-tap switch, captures in `media/faces/` (see list).
- Verified before the last small commits: `cargo test -p kabl-ui` all pass; `cargo test --workspace` 859 passed before the spurious-command fix (13 failures, all from the extra `Faces(None)`, fixed; kabl-ui re-run clean); `cargo clippy --workspace --all-targets -- -D warnings` clean. **Re-run workspace tests and clippy on the final head for the totals.**
- Frame time (`cargo run --release -p kabl-ui --example bench_chrome 300 live`, rack only, audio on, fake live data every frame; dense Composition): 1440x900 light median 6.2 ms, p95 7.5, max 8.8; dark 5.9/6.9/9.0; 1280x800 3.3/4.3/5.5. A second run: 6.3/11.5/15.6 (noisy machine; max near the 16 ms limit once). At zoom 0.6: 4.8/5.3/6.0. Rerun a few times on a quiet machine for the report; if max stays above 16 ms, optimise `draw_jacks` (per-module `cabled` vec) first.
- Audio cost of 15 face taps on Composition (`KABL_BENCH_FACES=1 ... bench_composition`): median 216 -> 233 us of a 5333 us budget; p99.9 807 -> 828 us.
- Plugin in REAPER 7.75 (real host, `host/start.sh`): editor renders at 1180x680 with live displays; `media/faces/plugin-editor-1180x680-50pct.webp`.

## Final numbers (head after this commit)
Workspace tests 872 passed, 0 failed, 30 ignored; strict clippy clean; fmt clean; master already merged. Quiet-machine bench (dense Composition 1440x900 light, displays live, rack only, 3 runs): median 4.0 to 4.8 ms, p95 4.8 to 5.2 ms, max 5.3 to 6.0 ms. Earlier noisy runs reached max 15.6 ms.

## Captures (docs/design/app-target/media/faces/)
`<scene>-<size>-<light|dark>.webp` for scene simple, composition, fm6, wt at 1440x900 and 1280x800; `plugin-editor-1180x680-50pct`, `arp-73pct`, `seq-ratchet-drawer`, `authored-panel-voice` (composite with authored panel), `live-reference`, `live-composition-100pct`, `faces-live-composition.mp4` (about 25 s, composition playing at 100%: filter curve and cutoff dot, envelope playhead, LFO dots, knob live dots). Not every capture was re-looked at after the final title tweak; look at the matrix again before reporting.

## Remaining
1. Final checks on the head: `cargo fmt --all`, `gh pr view 22` / `23` (merged), merge origin/master if it moved, `cargo test --workspace` (totals), `cargo clippy --workspace --all-targets -- -D warnings`, push, make CI pass on PR #24 (format, clippy, tests, package, validator, install cycle, D05) and note the run link.
2. Update the PR #24 body (jacks lower accepted; list below) and mark it ready only if the overseer says so.
3. Look at every capture in `media/faces/`; recapture anything odd (tooling below).
4. Optional if time: delay echo bars, filter input-spectrum bars (departure from mockup), osc.va live view (no engine view today: its picture is the waveform from params).
5. Send the final report to "Kabl overseer": per-family changes, data sources (face taps -> `Live`; params for static parts), the one engine change (facetap, Module::view on lfo/env), low-zoom rule, frame times, totals, capture/recording paths, departures, unverified, decisions.

## Low-zoom rule (as built)
Text under 9.5 px on screen (zoom below about 0.8 for 12 px labels) is not drawn except module names (kept down to 6 px, floored at 11); the tag line goes with the small text. Displays: zoom below 0.45 frame only; 0.45 to 0.7 plain trace, no glow/fill/playhead; 0.7 and up full. Seen in `composition-*` captures (50%) and the plugin capture.

## Facts that cost effort
- Capture tooling in `/home/stcksmsh/.claude/jobs/4129379d/tmp/`: `cap/run1.sh OUT PATCH SIZE SCRIPT [scale]` (Xvfb :96 is shared with other sessions: a run can die if another uses :96; `f/rec2.sh` uses :196), scripts and `f/matrix.py OUTDIR` (16 shots), `f/media.sh` (webp into the repo), `host/start.sh` (REAPER on :148; click Terminate on the VST scan dialog with xdotool, close the update notice; `touch host/run/quit` to stop; never pkill). Release binary: `cargo build --release -p kabl-ui --bin kabl-ui`.
- The Bash guard refuses compound commands, heredocs and anything naming `.github` with other work: use Write/Edit and plain single commands; `python3 file.py` works.
- `FullAdsr` can stay in Decay (f32 rounding near sustain for long decays); level is right, the playhead still looks right.
- `ui_state.launches` is the shared command queue; tests assert its contents, so commands must not be sent when nothing changed.
- No culling in `show_rack`: wants are collected only for faces whose rect meets the painter clip.

## Departures from mockups (for the report)
No input spectrum bars in the filter display (curve for LP/BP/HP, patched ones bright); osc.va shows the waveform from its parameters, not a live cycle; env/lfo/wt/fm/fm6 and filter are live; no delay/reverb picture; fm6 scope sits in the header; text hidden at low zoom instead of floored.

## Decisions for Kosta
Jacks lower on faces with a display: accepted. Open: hide-below threshold 9.5 px; whether osc.va needs a live cycle (engine ring in the module); delay echo picture.
