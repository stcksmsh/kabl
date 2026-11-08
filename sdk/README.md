# kabl creator SDK (preview)

Write a kabl code module in safe Rust. **Preview**: until kabl 1.0 the ABI, meta format and this
API may change, breaking (D12 decisions, principle 0). Licence: 0BSD (see `LICENSE`); the kabl
host and `cargo kabl` are GPL-3.0-or-later and are not in this directory.

- `kabl-module-sdk/`: the `no_std`, dependency-free crate. `Module` trait, `kabl_module!`, the
  const `Meta` builder (emits `abi_minor` and the `kabl.meta` section), state save/load through a module-sized transfer buffer, and an opt-in bump arena
  (cargo feature `arena`).
- `examples/logic`, `examples/bitfilter`: `demo.logic` and `demo.bitfilter` ported to the SDK.
  They carry no `unsafe` (`#![forbid(unsafe_code)]`) and no layout code.
- `template/`: the project `cargo kabl new` copies (a working module, one test, README, 0BSD
  LICENSE, pinned toolchain).

`kabl.notices` (the Rust toolchain's licence notices, from `kabl-module-sdk/NOTICE.txt`) is
written by `cargo kabl build` after linking, so a blob built with plain `cargo build` has no
notice and `cargo kabl validate` warns. Always ship the output of `cargo kabl build`.

```
rustup target add wasm32-unknown-unknown            # once
cargo test                                          # host-free unit tests
cargo test --features arena -p kabl-module-sdk      # plus the arena
cargo build --release --target wasm32-unknown-unknown -p sdk-demo-logic -p sdk-demo-bitfilter
```

`cargo test` also builds both examples for wasm and checks exports, sections and size
(`kabl-module-sdk/tests/wasm_examples.rs`).

Not yet possible: loading these blobs into kabl. The current host rejects `abi_minor`; the host
side is D12 PR 0. The committed blobs in `crates/modules/src/code/examples/` are the old
hand-written ones until then.

Index constants: `macro_rules!` cannot mint identifiers from the builder, so write
`const CUTOFF_CV: usize = 2;` and check it with `const _: () = META.assert_in(CUTOFF_CV, "cutoff_cv");`
(also `assert_out`, `assert_param`). A rename or reorder is then a compile error.
