# D07-P1 REAPER/CLAP proof

Status: draft, host acceptance blocked. This independent workspace exports a stereo CLAP instrument using kabl's real `PatchEngine` and `kabl_ui::show` rack. See [REPORT](REPORT.md), [REVIEW](REVIEW.md) and [CHECKLIST](CHECKLIST.md). It is not a production D07 plugin.

Linux build with Rust 1.98.1, pkg-config, ALSA and D-Bus development libraries:

```sh
cd prototypes/reaper-clap
cargo test --locked --lib
cargo build --locked --lib
mkdir -p "$HOME/.clap"
cp target/debug/libkabl_reaper_proof.so "$HOME/.clap/kabl-reaper-proof.clap"
python3 scripts/make-midi.py proof.mid
```

The independent lockfile pins Circuit-Stitch nice-plug revision `263b16877d0b0ad30921868338a6df46eadc9a46` with egui 0.35. If the lockfile cannot be regenerated in a new environment, use `cargo generate-lockfile` in this prototype workspace first and record the change. The fixture is an original three-note MIDI file. Use a new REAPER profile and project. The validator command (free-audio/clap-validator 0.4.1, commit `152b9823e992d782c5c1fd33bca0295478b919aa`) is:

```sh
clap-validator validate -j 1 --json target/debug/libkabl_reaper_proof.so > validation.json
```

The prior run exited 1 with state failures. Avoid `--in-process` for random-state fuzzing; the pinned framework can abort on an unbounded length field. See [evidence](evidence/). Complete the host walkthrough in CHECKLIST on a Linux desktop. The cloud X11 display could not start because Unix sockets were denied, so no REAPER observation is claimed.
