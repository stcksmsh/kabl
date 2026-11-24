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

No kabl code needed changing. No test is excluded with `--exclude`; the 8 skips are the
validator's own decision, because each covers an optional CLAP feature kabl does not offer.

## Skipped tests

| Test | Validator's reason | Why that is correct for kabl |
|---|---|---|
| `preset-discovery-crawl` | does not implement `clap.preset-discovery-factory/2` | kabl's sounds are browsed in its own editor; preset discovery is optional. |
| `preset-discovery-descriptor-consistency` | same | same |
| `preset-discovery-load` | same | same |
| `process-audio-double-out-of-place` | does not support 64-bit floating point audio | kabl processes 32-bit float; 64-bit is optional. |
| `process-audio-double-in-place` | same | same |
| `process-note-wildcard` | The plugin does not have any input note ports that support CLAP events | Not explained yet; see the note below. Note handling is covered by `process-note-out-of-place-basic` and `process-note-inconsistent`, which pass. |
| `layout-audio-ports-activation` | does not implement `audio-ports-activation` | optional extension; kabl's port layout is fixed. |
| `layout-configurable-audio-ports` | does not implement `configurable-audio-ports` | optional extension; kabl offers one layout. |

If kabl later adds one of these features, its test stops skipping and must pass.

Note on `process-note-wildcard`: kabl's single note input port advertises both the CLAP and
the MIDI dialect (`supported_dialects = CLAP_NOTE_DIALECT_CLAP | CLAP_NOTE_DIALECT_MIDI` in
`vendor/nice-plug/crates/nice-plug/src/wrapper/clap/wrapper.rs`, `ext_note_ports_get`), and
the validator's source at the release's commit (`152b982`, `NotePort::supports_clap`) reads
that bit. The skip still happens on every run, in and out of process. The cause is not in
`crates/clap`, the skip is not a failure, and the wildcard path (note events with id, port,
channel or key set to -1) is therefore not yet exercised by the validator. Worth a look when
the VST3 wrapper (W6) is built, since clap-wrapper translates host notes into CLAP events.

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
