# D07-P1 REAPER/CLAP proof

Draft, isolated prototype. It exports a stereo CLAP instrument with kabl's real `PatchEngine` and `kabl_ui::show` rack. The pinned validator now passes; REAPER 7.80 scanned the plugin and rendered a two-instance MIDI project under Xvfb. Host parameter edits and the embedded editor remain unverified in this cloud session. See [REPORT](REPORT.md), [REVIEW](REVIEW.md), [CHECKLIST](CHECKLIST.md), and [host evidence](evidence/host-run.txt).

Build on Linux with Rust 1.98.1, pkg-config, ALSA and D-Bus development libraries:

```sh
cd prototypes/reaper-clap
cargo test --locked --lib
cargo build --locked --lib
```

The independent lockfile pins Circuit-Stitch nice-plug `263b16877d0b0ad30921868338a6df46eadc9a46`, clap-sys 0.5.0 and egui 0.35. Do not regenerate the lockfile for a normal build. Run free-audio/clap-validator 0.4.1 at `152b9823e992d782c5c1fd33bca0295478b919aa`:

```sh
clap-validator validate -j 1 --json target/debug/libkabl_reaper_proof.so > validation.json
```

Use a fresh REAPER profile and temporary `CLAP_PATH`. The exact scripted [host walkthrough](evidence/host-run.txt) uses [host-smoke.lua](scripts/host-smoke.lua), creates a two-track MIDI project and renders it offline. [proof.mid](evidence/proof.mid) is also available for manual insertion. The project fixture contains an absolute scratch render destination, so regenerate it in your own temporary directory before another render. The host run had no JACK audio device; do not infer live playback or editor recovery from its offline result.
