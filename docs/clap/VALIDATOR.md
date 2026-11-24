# clap-validator results for the kabl CLAP

[clap-validator](https://github.com/free-audio/clap-validator) is the CLAP project's
conformance checker. It loads the plugin, creates instances and runs scan, process, note,
parameter, state, layout and transport tests against them, each in its own process.

## Last run

- Date: 2026-11-24, Linux x86_64, commit on branch `claude/clap-validator` (from master
  `69660507`).
- Validator: release `0.4.1` (`clap-validator --version` prints `clap-validator 0.4.1`), asset
  `clap-validator-0.4.1-127-g152b982-ubuntu-22.04.zip`, sha256
  `49edadcfb407ea0dd946ce418300e853fbd2660fa4b0d00e4f19ff8eef24ad90` (matches the digest
  GitHub publishes for the asset).
- Plugin: `cargo build --release -p kabl-clap`, `target/release/libkabl_clap.so` copied to
  `kabl.clap` (the same step `packaging/linux/package.sh` uses).
- Result, the same on three consecutive runs:

  ```
  44 tests run, 36 passed, 0 failed, 0 warnings, 8 skipped
  ```

No kabl code needed changing. No test is excluded with `--exclude`; the validator skips 8
tests on its own. Seven cover optional CLAP features kabl does not offer; one
(`process-note-wildcard`) follows from kabl's choice to accept notes in the MIDI dialect only.

## Skipped tests

| Test | Validator's reason | Why that is correct for kabl |
|---|---|---|
| `preset-discovery-crawl` | does not implement `clap.preset-discovery-factory/2` | kabl's sounds are browsed in its own editor; preset discovery is optional. |
| `preset-discovery-descriptor-consistency` | same | same |
| `preset-discovery-load` | same | same |
| `process-audio-double-out-of-place` | does not support 64-bit floating point audio | kabl processes 32-bit float; 64-bit is optional. |
| `process-audio-double-in-place` | same | same |
| `process-note-wildcard` | The plugin does not have any input note ports that support CLAP events | kabl's choice: its one note input port advertises the MIDI dialect only (see the note below). The test sends CLAP note events with wildcard ids, which kabl does not accept. |
| `layout-audio-ports-activation` | does not implement `audio-ports-activation` | optional extension; kabl's port layout is fixed. |
| `layout-configurable-audio-ports` | does not implement `configurable-audio-ports` | optional extension; kabl offers one layout. |

If kabl later adds one of these features, its test stops skipping and must pass.

Note on `process-note-wildcard`: kabl answers the `clap.note-ports` extension itself
(`get_extension_bridge` in `crates/clap/src/state_bridge.rs`), so the nice-plug wrapper's own
note-port declaration is never reached. kabl's `note_get` reports one input port, "MIDI
input", with `supported_dialects` and `preferred_dialect` both `CLAP_NOTE_DIALECT_MIDI`, and
no output port. This follows `docs/PRODUCT-PLAN.md` ("Advertise only supported note
dialects"). The validator skips the wildcard test when no input port supports the CLAP
dialect (`NotePort::supports_clap` at the release's commit `152b982`). The passing
`process-note-out-of-place-basic` and `process-note-inconsistent` tests therefore send
MIDI-dialect events only; CLAP note events, wildcard or not, are not exercised. If kabl ever
advertises the CLAP dialect, this test runs and must pass.

## How to rerun

1. Download the release asset for your OS from
   <https://github.com/free-audio/clap-validator/releases> into a scratch directory and
   check its sha256 against the digest shown on the release page.
2. Build the plugin and give it the `.clap` name:

   ```
   cargo build --release -p kabl-clap
   cp target/release/libkabl_clap.so kabl.clap        # Windows: kabl_clap.dll
   ```

3. Run it (no audio device is opened; the tests process audio offline):

   ```
   clap-validator validate kabl.clap --hide-output --only-failed
   ```

   The last line is the summary. Drop `--only-failed` to list every test, or add `--json`
   for a machine-readable report. `-t <regex>` runs only matching tests; `-x <regex>` skips
   them (document any such skip in the table above).

The Windows CI job that does the same on `windows-latest` is in
[`docs/windows/CI-JOB.md`](../windows/CI-JOB.md).
