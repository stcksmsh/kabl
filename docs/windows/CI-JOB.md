# Windows CI job (W1, OD-W9: native MSVC on `windows-latest`)

Agents may not edit `.github/`, so this job is written here for Kosta to add to
`.github/workflows/ci.yml` under `jobs:`. It builds the three crates that ship on Windows
(kabl-ui, kabl-clap, kabl-standalone) with the MSVC toolchain and runs their tests.

```yaml
  windows:
    runs-on: windows-latest
    timeout-minutes: 45
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.100.0
      - uses: Swatinem/rust-cache@v2
      - name: Build ui, clap and standalone
        run: cargo build --locked -p kabl-ui -p kabl-clap -p kabl-standalone --all-targets
      - name: Tests
        run: cargo test --locked -p kabl-ui -p kabl-clap -p kabl-standalone
```

Notes on the job:

- The default Rust host on `windows-latest` is `x86_64-pc-windows-msvc`; the MSVC build
  tools are preinstalled, so `zstd-sys` (C) compiles with `cl.exe` and needs no extra step.
- No system libraries are needed: cpal uses WASAPI, and the GUI stack needs no apt-style
  packages on Windows.
- Tests that open a real audio stream are already `#[ignore]` (they need an ALSA null PCM),
  so the runner needing no audio device is fine.
- Lint stays on Linux (`.github/scripts/clippy.sh`). A Windows clippy run of kabl-ui and
  kabl-clap with `--all-targets -- -D warnings` was clean from Linux (see below), so a
  `cargo clippy` step can be added later without code changes for those two crates.

## Tests that must be skipped on Windows before this job is green

Found by `cargo clippy --target x86_64-pc-windows-gnu --all-targets` from Linux, which
type-checks every test target for Windows. Nothing has run on a Windows host yet, so the
second list is from reading the code, not from a run.

Does not compile on Windows (the job fails to build until it is gated):

- `crates/standalone/tests/applog_full_disk.rs`: uses `std::os::unix::fs::symlink` to
  `/dev/full`. Needs `#![cfg(unix)]` at the top of the file. This file was outside the
  W1a slice's scope, so the gate is a follow-up change.

Compiles, but depends on Linux paths at run time (check on the first Windows run):

- `crates/ui/tests/record.rs`, `write_failures_are_visible`: expects
  `/proc/kabl-no/take.wav` to be unwritable and probes `/dev/full`. The `/dev/full` part
  is already guarded by `if let Ok(..)`; the `/proc` path resolves to `\proc\...` on the
  current drive and should still fail to open, but if it does not, the test needs
  `#[cfg(unix)]`.
- `crates/ui/tests/library.rs` already gates its permission test with `#[cfg(unix)]`.

## Cross-check from Linux

The `x86_64-pc-windows-gnu` target type-checks the same Rust code as MSVC. `zstd-sys` needs
a C cross compiler for that target; for `cargo check` and `cargo clippy` nothing is linked,
so the host's libzstd can stand in through pkg-config:

```
ZSTD_SYS_USE_PKG_CONFIG=1 PKG_CONFIG_ALLOW_CROSS=1 \
  cargo check --target x86_64-pc-windows-gnu -p kabl-ui -p kabl-standalone -p kabl-clap
```

This does not produce a Windows binary; the CI job above is the real build.
