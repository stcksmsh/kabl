# D05 audio recovery

The status bar shows the audio phase and session. Choose an available output and click **Retry audio** to close the old stream and open a new one. Edits accepted while it is unavailable remain the source of truth. Retry ends current notes, tails, transport and any recording; press Start for clocks and Record for a new take. The old take is labelled interrupted and its path/error is shown. See [design](design.md) for ownership and known limits, [REPORT](REPORT.md) for evidence, [REVIEW](REVIEW.md) for independent review, and [CHECKLIST](CHECKLIST.md) for owner checks.

## Reproduction

From the repository root, use `cargo run --release -p kabl-ui -- --patch patches/init-keyboard --rate 48000 --frames 256`. In a development environment with a backend, `KABL_AUDIO_FAULT=stall KABL_AUDIO_FAULT_AFTER=30` simulates lost progress; use `error` for a synthetic non-xrun error. `KABL_AUDIO_FAIL_REOPEN_ATTEMPTS=1` makes the first Retry fail; `KABL_AUDIO_RETRY_DELAY_MS=500` exposes the Restarting interval. Remove these variables for ordinary use. With a Linux ALSA null PCM configured as default, `ALSA_CONFIG_PATH=/path/to/alsa-null.conf cargo test -p kabl-ui --bin kabl-ui recovery_tests::actual_ -- --ignored --nocapture` exercises the actual `cpal` backend path; this routes audio into a null sink.

The profile command is `cargo run --release -p kabl-engine --example mono_profile`. The sound sources are repository patches. No external visual/audio assets or third-party recordings were added.

The 2026-09-28 local cloud container has Ubuntu 24.04, Rust 1.98.1, an ALSA null PCM stand-in and no `/dev/snd` or usable local display. A hosted Actions Xvfb session launched the installed package and captured its injected stall, Retry and Running states at 1440×900 light; REPORT links the run and images. The workflow now exercises both sizes in both themes; see REPORT for its actual run status. There is no physical device loss, audible capture or hardware controller evidence. A blocked backend close has no guaranteed deadline. Kosta's laptop and controller acceptance is pending.
