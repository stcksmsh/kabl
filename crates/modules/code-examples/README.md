# Example code modules (D11 PR 5)

Two `no_std` wasm modules following ABI v0 (`../src/code/abi.rs`):

- `logic/` is `demo.logic`, a global control-rate module: min, max, AND, XOR or threshold of two
  CV inputs, an offset, and a gate that freezes the output.
- `bitfilter/` is `demo.bitfilter`, a per-voice stereo filter: two cascaded low-pass biquads per
  channel, resonance, soft-clip drive, and a cutoff input at one octave per unit.

Each carries its metadata in a `kabl.meta` custom section (see `parse_meta` in
`../src/code/wasmi_engine.rs`). State lives in the module's own linear memory behind a version
word.

The built blobs are committed in `../src/code/examples/` and embedded in kabl with
`include_bytes!` (`../src/code/host.rs`). To rebuild them:

```
rustup target add wasm32-unknown-unknown   # once
crates/modules/code-examples/build.sh
```

The build is reproducible: a second run on the same toolchain gives byte-identical files.
This directory is its own workspace (not part of kabl's), so a normal `cargo build` ignores it.
