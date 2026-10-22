# Vocoder: handover

Branch `claude/vocoder` (from origin/master), worktree `.worktrees/functional-cables`. Report to "Kabl overseer", do not merge, stop when done. Caveman/Ponytail ultra in chat; code, commits, PR text normal English. Stay under 50% context.

## Task as received (condensed)
Add a vocoder for the Daft Punk robot voice. (1) `vocoder` module: carrier + modulator in, bands 8..32, spacing, formant shift, attack/release, HF/unvoiced pass, mix. (2) Modulator source: internal (formant filter + noise + random vowel loop, no mic) AND audio input (standalone device; plugin sidechain/aux port, backward compatible with existing REAPER projects; if risky, build standalone side and report options). (3) Face + live band-level display, themes A/B light/dark. (4) Demo patches: robot phrase, live-voice; both pass `factory_levels_hold_the_target`. Constraints: nothing on the audio thread allocates/locks; safe 44.1 to 192 kHz; hostile input safe; device may vanish; no schema bump without asking; old patches/bank unchanged; keep shared-code changes small (agent 2: `claude/drums`, agent 1: `claude/presets-pilot`); CI green on final head (fmt, strict clippy, tests, package, validator, licenses); check standalone and REAPER. Out of scope: talkbox, pitch correction, sampler, presets beyond demos. Deliver for Kosta: clips (phrase dry then vocoded, band-count sweep, live-voice with CC0/self-made voice), listening notes, face captures. Report: what built, modulator decision + plugin port outcome, tests/totals, CPU at 32 bands, clip/capture paths, CI link, gaps.

## Done
- `vocoder` + `audio.in` modules (`crates/modules/src/builtins/{vocoder,audio_in}.rs`, registry, `tests/vocoder.rs`, 10 tests).
- Engine: `PatchEngine::set_audio_in`, `CompiledPatch::feed_audio`, `tests/audio_in.rs`.
- Standalone: `kabl-ui --input` opens default input (`crates/standalone/src/input.rs`); CLAP: optional second input port (`aux_input_ports`), queue in `Instrument`, fed in the timeline block hook.
- Face display (`fx_displays.rs::vocoder`), help, routing names, `needed_inputs`.
- Demos `patches/sound-engines/demos/{robot-voice,live-voice}` from `docs/vocoder/scripts/make_demos.py` (then `cargo run -p kabl-engine --example resave -- DIR` writes checkpoint.json); factory render feeds a synthetic voice to `audio.in` patches (`SyntheticVoice` in `crates/ui/tests/factory_bank.rs`).
- `render_midi --audio-in`, `docs/vocoder/scripts/make_clips.py`, voice `docs/vocoder/audio/voice-espeak.flac` (espeak-ng 1.51, en-us, self-generated).

## Remaining (in order)
1. Face captures A/B light/dark (`docs/rack-migration/drive.py` recipe, as `docs/effect-displays/scripts/capture.py`); needs release `kabl-ui`.
2. Clips: `python3 docs/vocoder/scripts/make_clips.py` (needs release `render_midi`, sox, ffmpeg).
3. REAPER: build plugin, A/B render of an old project (old build vs new) to prove same sound; test the aux input with a track routed in. Recipe `docs/wavetable-editor/host-*.{py,lua}`.
4. Validator baseline: `scratch/cv/bin/clap-validator` (installed with `cargo install --root scratch/cv`), diff with `docs/sound-engines/scripts/validator_diff.py`.
5. CPU at 32 bands (bench), `docs/vocoder/README.md`, `docs/factory-levels/README.md` note on synthetic voice and two new rows.
6. fmt, strict clippy (`.github/scripts/clippy.sh`), full tests, push, draft PR, CI link, report.

## Facts that cost effort
- Bash guard refuses heredocs/compound commands naming git paths; use Write/Edit, plain commands.
- Factory discovery needs `checkpoint.json` (use `resave`). Level tuning via the tail `gain` in the demo script.
- Unvoiced noise is gated by carrier level (no synth, no hiss). `bands` is a Linear knob (a 25-option selector overflowed the face).
- Vocoder multiplies: output follows voice level; default drive 24 dB suits a -20 dBFS voice.

## Open decisions for Kosta
Default drive 24 dB; fixed 5 kHz unvoiced corner; `--input` is opt-in (no mic opened unless asked).
