# yourhandle.gain

A kabl code module written with `kabl-module-sdk`. **Preview**: until kabl 1.0 the ABI and the SDK
API may change, breaking.

```
cargo test            # your unit tests, on the host
cargo kabl build      # reproducible wasm build, adds the licence notices, validates
cargo kabl validate   # re-check a built blob
```

The shippable file is `target/wasm32-unknown-unknown/release/kabl/<id>.wasm`. Ship the output of
`cargo kabl build`: it writes the `kabl.notices` section (licence notices for the Rust toolchain
code every module links) after linking. A blob built with plain `cargo build` has no notice, and
`cargo kabl validate` warns about it.

Edit `src/lib.rs`: `META` (id, ports, params) and `process`. The id is `githubhandle.module`;
`kabl.`, `demo.` and `test.` are reserved. Port and param indices are constants you keep in step
with `META`; a mismatch is a compile error.

`rust-toolchain.toml` pins the compiler: two clean builds on that toolchain give identical bytes.

## Licence

0BSD, see `LICENSE`. Replace it with your own if you like; the SDK you depend on is 0BSD too.
