#!/bin/sh
# Rebuilds the committed example blobs. Needs: rustup target add wasm32-unknown-unknown
# Run from anywhere: crates/modules/code-examples/build.sh
# `cargo test -p kabl-modules --test code_examples` checks the committed blobs still load and
# carry the expected metadata.
set -e
cd "$(dirname "$0")"
cargo build --release --locked --target wasm32-unknown-unknown
out=../src/code/examples
mkdir -p "$out"
cp target/wasm32-unknown-unknown/release/demo_logic.wasm "$out/demo_logic.wasm"
cp target/wasm32-unknown-unknown/release/demo_bitfilter.wasm "$out/demo_bitfilter.wasm"
ls -l "$out"/*.wasm
