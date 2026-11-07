#!/bin/sh
# Rebuild kernel.wasm (committed) and run the gate. Needs: rustup target add wasm32-unknown-unknown
set -e
cd "$(dirname "$0")"
cargo build --release --manifest-path kernel/Cargo.toml --target wasm32-unknown-unknown
cp kernel/target/wasm32-unknown-unknown/release/d11_kernel.wasm kernel.wasm
cargo run --release
