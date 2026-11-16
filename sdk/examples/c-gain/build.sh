#!/bin/sh
# Builds gain.c to wasm with clang (the wasm32 backend) and rust-lld, which ships with rustup's
# toolchain, so no wasm-ld package is needed. Output: ./c-gain.wasm
# Usage: build.sh [path-to-wasm-ld]   (default: wasm-ld if on PATH, else rustup's rust-lld)
set -eu
cd "$(dirname "$0")"

if [ $# -ge 1 ]; then
    LLD=$1
elif command -v wasm-ld >/dev/null 2>&1; then
    LLD=wasm-ld
else
    LLD="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/^host: //p')/bin/gcc-ld/wasm-ld"
fi

clang --target=wasm32 -O2 -ffreestanding -fno-builtin -nostdlib -c gain.c -o gain.o
"$LLD" --no-entry --export=init --export=process --export=reset --export-memory \
    --no-gc-sections -z stack-size=4096 --initial-memory=65536 -o c-gain.wasm gain.o
rm -f gain.o
echo "built c-gain.wasm ($(wc -c < c-gain.wasm) bytes)"
