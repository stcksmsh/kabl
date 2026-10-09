# Plugin and host verification: functional cables

Date 2026-10-10. Build under test: branch `claude/functional-cables` on top of `7ed971c` plus the cable-param cap fix below, release `libkabl_clap.so`, sha256 `61f2ed41117096492301cdad5396c5ba4be8cbee1fee3aa6b6bbdf545644706e`. Machine: Linux 7.0.0-34-generic, REAPER 7.75/linux-x86_64 on a private Xvfb (`:147`) with metacity and PipeWire (`pw-jack`), 48 kHz, 256-sample buffer. Everything is scripted: no person operated REAPER, and this is not a hardware audio or controller check.

## A bug the host check found, and its fix

The plugin's state validator (`crates/clap/src/sound_state.rs`) and the composite validator (`crates/ui/src/composites.rs`) rejected any cable with more than 32 params. A cable with a 16-step pattern A, a pattern B and a morph has up to 71 (`kabl_cables::SLOTS` plus `amount` and `bypass`). When REAPER opened the Morph Arc project the plugin therefore refused the state and kept its default patch (seen in the first run: the project's state was not what REAPER saved back; the saved one was the default patch with output gain 1.0). Fixed with `kabl_cables::MAX_CABLE_PARAMS`; `state_accepts_a_cable_with_every_functional_param` (crates/clap) accepts a full cable and rejects one param more. The project and the standalone app never hit this because their loaders have no such cap.

## CLAP validator

`clap-validator 0.4.1 validate --json target/release/libkabl_clap.so`, compared with the recorded baseline `docs/factory-performance-bank/evidence/validator.json` by `scripts/validator_diff.py`: 35 success, 9 skipped, no test added or removed, no status changed, no failures. Result: `evidence/validator.json`.

## REAPER: save, full quit, reopen

Command: `cargo build --release -p kabl-clap && python3 docs/functional-cables/scripts/host.py` (adapted from `docs/sound-engines/scripts/` on `origin/claude/sound-engines`: same Xvfb/profile/CLAP_PATH/render method; the wavetable import and the baseline plugin are dropped). Evidence: `evidence/host/` (saved projects, mapped-plugin lines, REAPER info, `result.json`).

- Project: two tracks with the plugin. Track 1 is `patches/functional-cables/morph-arc` (four cables with pattern, probability on the echo send, morph 0.3, glide, pattern B). Track 2 is `echo-throws` (pattern and probability on two cables). The states were written by the script into a REAPER project template from the factory-bank evidence.
- REAPER loaded the plugin on both tracks (`fx_count=1`, not offline, plugin mapped in the process).
- REAPER saved the project, was quit completely, started again on the saved file, saved again and rendered the master mix (24 s, 48 kHz stereo, 32-bit float, the host's own renderer).
- State: the states in the project as written, as saved the first time and as saved after the reopen are identical (compared as float32), including every cable's pattern, probability, morph, glide and pattern B params (listed in `result.json`).
- Render: first render against the render after quit and reopen: relative difference 0.0, spectrum cosine 1.0, envelope correlation 1.0 (peak 0.254, all 24 seconds active, finite). The plugin plays the same bits after a reopen.
- The cables matter in the host: the same project with every functional-cable param removed renders differently (relative difference 1.80, spectrum cosine 0.62, envelope correlation 0.25).
- An old project (saved by an earlier build, state version 2) opened and saved by this build comes back with an identical state.

## Not verified

- Another host (Bitwig, Ardour, a real DAW on another OS) and macOS/Windows.
- Morph moved by host automation: no cable param is a host parameter yet, so the project holds morph at its saved value; morph in motion is covered by the engine tests and the promo recording of the standalone app, not by this host run.
- Human listening of the rendered files, a GUI session in the host (the plugin editor was not opened), real-time performance in the host, and the Pi-class machine.
- Render equality with the previous build for the old project (the baseline plugin was not run this time; the engine's golden hashes cover old patches bit for bit).
