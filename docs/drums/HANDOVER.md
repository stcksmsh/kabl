# Drums: handover

Branch `claude/drums` (from origin/master), worktree `.worktrees/sound-engines-2`, draft PR #38. Report to "Kabl overseer", do not merge, stop when done. Caveman/Ponytail ultra in chat; code, commits, PR text normal English. Stay under 50% context; checkpoint per `kabl/CLAUDE.md`.

## Task as received (condensed)
Add drum voices: kick, snare, clap, closed + open hat (open choked by closed), tom if cheap. Synthesised, 808/909 territory, punch and drive. Decide module-per-voice vs one kit module and justify. Gate/trigger + accent + velocity inputs, playable from seq/arp/clock and from MIDI notes (GM map 36 kick, 38 snare, 39 clap, 42 closed hat, 46 open hat, 45 tom); smallest MIDI routing change; say what was chosen. Faces in the current layout, live displays where honest, theme A/B light/dark. Demo patches: one groove (kick, snare/clap, hats from seq), one with kick feeding the compressor sidechain on bass/pad; both obey the factory level rule (`factory_levels_hold_the_target`).
Constraints: no allocation/locks on audio thread; numerically safe 44.1 to 192 kHz; sample-accurate triggers; click-free retrigger (say how); no schema bump without asking; old patches/bank unchanged; keep shared-code changes small (agent 3 does vocoder, agent 1 presets); CI green on final head (fmt, clippy 1.99 strict all targets, tests, package, validator, licenses); check in standalone and REAPER incl. MIDI item triggering.
Out of scope: drum sequencer UI/grid, samples, presets beyond demos.
For Kosta: one clip per voice (hits across settings + one line what to listen for), groove clip, pumping clip, face captures.
Final report to "Kabl overseer": what was built + module-structure decision, MIDI routing choice, tests and totals, CPU per voice, clip/capture paths, CI link, known gaps.

## Done (commits on claude/drums)
- Voices `crates/modules/src/builtins/drums.rs` (kinds `drum.kick|snare|clap|hat|tom`, all Rate::Global), `midi_note.rs` (`midi.note`), registered in `registry.rs`/`mod.rs`.
- Engine hook: `for_each_midi_note` in `compile.rs`, `note_key` in `patch_engine.rs::send_key` (shared code, ~15 lines).
- Tests: `crates/modules/tests/drums.rs` (17), `crates/engine/tests/drums.rs` (5, incl. sidechain duck), `crates/ui/tests/drum_demos.rs` (3).
- UI: `drum_displays.rs`, help text, rack.rs display arm, routing names/`note` format, style section.
- Demos `patches/sound-engines/demos/four-floor` and `pump` (built by `drum_demos.rs write_patches`), levels -18.7 and -18.8 dBFS.

## Decisions
- One module per voice: own jacks, face, mix, sidechain tap; idle voice costs almost nothing. Hat has `closed` and `open` gates on one envelope: a closed hit retakes the envelope with the short decay = the choke.
- MIDI: new `midi.note` (note + channel params, gate + velocity out, global rate, 10 ms pulse on the note-on sample). Not `midi.in` ranges: a voice-rate gate is averaged over voice lanes. Not one 12-output kit module: engine caps modules at 4 outputs (`MAX_OUTPUTS`).
- Retrigger: `Declick` moves the new hit's first sample onto the old last sample, difference fades over 1 ms. Unpatched `velocity` (held 0) means full velocity.
- Hat source: six PolyBLEP squares at the 808 frequencies, high-passed (metallic, no aliasing at 44.1 kHz), tanh-bounded.

## Remaining (in order)
1. Docs `docs/drums/README.md` (design, ports/params table, MIDI recipe).
2. CPU per voice: bench (`crates/modules/examples/bench_sources.rs` style); numbers into README.
3. Clips: `cargo test -p kabl-ui --test drum_demos write_renders -- --ignored` (target/drum-demos); per-voice clips; send files to Kosta.
4. Face captures under theme A/B light/dark (see `docs/effect-displays/scripts/capture.py`).
5. REAPER check incl. MIDI item (see `docs/sound-engines/scripts/host*.py`).
6. CI on the final head: fmt, clippy 1.99 strict, workspace tests, package, validator, licenses; update PR body; report.

## Facts that cost effort
- Bash guard refuses heredocs and compound git commands: use Write/Edit, plain commands, `python3 file.py` or `python3 -`-free edits via Write.
- Factory render launches bank B on every seq at 6 s: demo patches carry both banks.
- Param help text is required for every param (`help.rs` test); rack test wants stepped selectors small, so `midi.note` params are Linear knobs.
