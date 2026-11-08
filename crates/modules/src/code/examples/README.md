# Example code modules

`demo_logic.wasm` (`demo.logic`) and `demo_bitfilter.wasm` (`demo.bitfilter`) are built from
`sdk/examples/logic` and `sdk/examples/bitfilter` with `kabl-module-sdk` (they carry
`abi_minor 1`, `layout_ext` and the state exports). They are embedded with `include_bytes!` in
`../host.rs` and registered like any other blob.

To rebuild after the SDK or the ABI changes (nothing is frozen before 1.0, so a breaking change
means rebuilding these and adding a changelog line):

```
rustup target add wasm32-unknown-unknown   # once
cd sdk && cargo build --release --locked --target wasm32-unknown-unknown -p sdk-demo-logic -p sdk-demo-bitfilter
cp target/wasm32-unknown-unknown/release/sdk_demo_logic.wasm ../crates/modules/src/code/examples/demo_logic.wasm
cp target/wasm32-unknown-unknown/release/sdk_demo_bitfilter.wasm ../crates/modules/src/code/examples/demo_bitfilter.wasm
```

`crates/modules/tests/code_compat.rs` loads both blobs in the real engine as the start of the
compatibility corpus (a regression check that may be regenerated before 1.0; it binds at the
1.0 freeze).
