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
- Lint stays on Linux (`.github/scripts/clippy.sh`). A Windows clippy run of kabl-ui,
  kabl-clap and kabl-standalone with `--all-targets -- -D warnings` was clean from Linux (see below), so
  a `cargo clippy` step can be added without code changes.

## Tests skipped on Windows

Found by `cargo clippy --target x86_64-pc-windows-gnu --all-targets` from Linux, which
type-checks every test target for Windows. Nothing has run on a Windows host yet, so the
second list is from reading the code, not from a run.

Gated out of Windows builds:

- `crates/standalone/tests/applog_full_disk.rs`: uses `std::os::unix::fs::symlink` to
  `/dev/full`, so the file is `#![cfg(unix)]`.

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

## Windows CLAP and clap-validator (W5)

A second job, also for `jobs:` in `.github/workflows/ci.yml`. It builds the release CLAP
with MSVC, names it `kabl.clap`, downloads clap-validator 0.4.1, checks the archive's sha256
against the digest GitHub publishes for it, and validates the plugin. clap-validator exits
non-zero when any test fails, so a failure fails the job. The Linux result and the expected
skips are in [`docs/clap/VALIDATOR.md`](../clap/VALIDATOR.md).

```yaml
  windows-clap-validator:
    runs-on: windows-latest
    timeout-minutes: 45
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.100.0
      - uses: Swatinem/rust-cache@v2
      - name: Build the CLAP
        run: cargo build --locked --release -p kabl-clap
      - name: Name it kabl.clap
        shell: pwsh
        run: Copy-Item target/release/kabl_clap.dll kabl.clap
      - name: Get clap-validator 0.4.1
        shell: pwsh
        run: |
          $url = "https://github.com/free-audio/clap-validator/releases/download/0.4.1/clap-validator-0.4.1-127-g152b982-windows.zip"
          Invoke-WebRequest $url -OutFile clap-validator.zip
          $hash = (Get-FileHash clap-validator.zip -Algorithm SHA256).Hash.ToLower()
          if ($hash -ne "d935c3af0a45c3911ea2e900f4aa5d6709dac82bb485f0c4ce28648ab2cd0c10") {
            throw "clap-validator.zip sha256 mismatch: $hash"
          }
          Expand-Archive clap-validator.zip -DestinationPath clap-validator
      - name: Validate
        run: clap-validator/clap-validator.exe validate kabl.clap --hide-output --only-failed
```

Notes on this job:

- The Windows archive holds a single `clap-validator.exe` at its root, so `Expand-Archive`
  puts it at `clap-validator/clap-validator.exe`. This was confirmed by downloading the
  asset and listing it with `unzip -l` on 2026-11-24. The Linux and macOS archives are laid
  out differently, so the step is Windows-only.
- The validator processes audio offline, so the runner needs no audio device.
- Updating the validator means changing the URL and the sha256 together; the digest is on
  the release page next to the asset.
- Not run on a Windows host yet. The first run is the check that the MSVC CLAP passes the
  same 36 tests as Linux with the same 8 skips.
