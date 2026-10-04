# D07: KABL in REAPER

Engineering is implementing; owner review is pending. The production instrument is usable in the scratch host, but repeated REAPER renders have not reproduced identical PCM. D08 is not authorized. Do not merge this batch based on the automated checks alone.

KABL exposes a stereo-output CLAP instrument with the production rack and sound browser. REAPER owns the audio device and MIDI input. Output gain is the single persistent host control; full automation slots and Host/Free transport synchronization belong to D08. The complete graph, cable values, labels and parameter bases are embedded in each project state. Built-in D07 sounds need no external audio assets.

## Build and test

```sh
cargo build --release -p kabl-clap
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
python3 docs/reaper-instrument/scripts/make-bundle.py /absolute/new/bundle-directory
```

The CLAP binary is `target/release/libkabl_clap.so`, installed under the filename `kabl.clap`. The bundle builder creates an isolated REAPER profile, plugin directory, two-instance project, media and `launch.sh`; it does not change the user's normal configuration. Run `./launch.sh` from the resulting directory. An optional normal installation copies `plugins/kabl.clap` to `~/.clap`, after checking that this does not replace an existing installation. Remove that exact file to uninstall.

The persistent owner package is `/home/stcksmsh/.codex/visualizations/2026/10/03/01a10304-4ddb-7850-bdcf-4509411a23d5/d07-owner-test`. Its original project contains two independently edited instances: 6 modules/6 cables/gain 0.4 and 15 modules/30 cables/gain 0.55. The relocated package test loads this binary outside the checkout, with `KABL_FACTORY_DIR` pointing to an unavailable directory, verifies gain readback, renders, saves, and independently decodes both complete states for exact equality.

## Evidence

See [REPORT.md](REPORT.md), [REVIEW.md](REVIEW.md), [design.md](design.md) and [CHECKLIST.md](CHECKLIST.md). Raw results live in [evidence/](evidence/); scripts live in [scripts/](scripts/). Large media remain in the persistent owner package and `scratch/host` rather than Git.

- `media/d07-musical-demo.wav`: 36-second listening copy of the 52-second offline musical render, trimmed and uniformly scaled +16.15 dB.
- `media/live-reaper-demo.wav`: 34.6-second listening copy of the separate live REAPER-node capture, with the same uniform gain.
- `media/operation-1.mp4` and `media/editor-cycles.mp4`: actual scratch REAPER operation; silent videos, with audio supplied separately.
- `media/relocated-render.wav`: raw 52-second offline output from the outside-checkout package test.
- `projects/two-instances.rpp`: complete sound states and scripted MIDI; save copies for hands-on edits.

The original musical render is active above -60 dB RMS from 0.5 to 30.5 seconds, above -90 dB from 0.5 to 32.2 seconds, and has last-second RMS 2.25e-22. The long ending tests tail settling. Virtual keyboard input was recorded through REAPER's input queue; no physical controller was available. Ten editor cycles establish lifecycle transitions and transport/readback retention. Separate live capture establishes live audio; the cycle log alone does not prove uninterrupted audio across every transition.

## Reproducibility and limits

Free oscillator/LFO DSP advances during silent processing. Explicit CLAP reset clears transient DSP and MIDI history while preserving the sound. A fresh REAPER process or offline/online toggle is not evidence that reset occurred immediately before render frame zero. Both tested host policies produced different PCM; failures and hashes are preserved. Bounded telemetry establishes differing start/reset/pre-roll sequences; sole PCM cause remains unproven. Fresh-process and serial-offline policies both fail. Bit-exact REAPER render reproduction remains an engineering gap.

Measurements use a separate fixed-storage timing proxy around the actual exported CLAP process callback. This proxy is never shipped as the instrument. Results include clock/atomic overhead, startup and shutdown calls; callback time, backend errors and physical latency are distinct. The 12-second cases cover light/dense sounds, one/two instances and editor open/closed cases with actual floating handles verified at start, six seconds and end at negotiated 48 kHz/256. They are short observations, not a long-running stability guarantee. Physical round-trip latency and controller feel remain unmeasured.

## Dependencies and sources

Production uses nice-plug and nice-plug-egui pinned to `263b16877d0b0ad30921868338a6df46eadc9a46` (ISC), egui 0.35.0, clap-sys 0.5.0, basedrop 0.1.3 and rtrb 0.4.0. Cargo.lock fixes the resolved dependency set; KABL uses MIT OR Apache-2.0. Nice-plug license files are copied into the owner bundle. The production state extension replaces the framework's nontransactional parameter-then-persistent-field loader.

Reproduction scripts use the official [REAPER ReaScript API](https://www.reaper.fm/sdk/reascript/reascripthelp.html). CLAP integration uses the pinned framework and clap-sys declarations, with MIDI dialect only. The isolated prototype remains a regression/reference fixture; it is not the production state implementation.
