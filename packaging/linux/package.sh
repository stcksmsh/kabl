#!/usr/bin/env bash
# Builds a relocatable Linux package of kabl (docs/find-play-save/README.md, "Install"):
#
#   kabl-<version>-linux-<arch>/
#     bin/kabl-ui
#     share/kabl/patches/...        factory sounds (found relative to bin/)
#     README.txt
#
# and kabl-<version>-linux-<arch>.tar.gz next to it, in $OUT (default target/dist).
# Extract anywhere and run bin/kabl-ui, or copy bin/ and share/ under a prefix such as
# /usr/local. Nothing is read from the source checkout at run time.
set -euo pipefail
cd "$(dirname "$0")/../.."
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
NAME="kabl-$VERSION-linux-$(uname -m)"
OUT=${OUT:-target/dist}
cargo build --release -p kabl-ui --bin kabl-ui
rm -rf "$OUT/$NAME" "$OUT/$NAME.tar.gz"
mkdir -p "$OUT/$NAME/bin" "$OUT/$NAME/share/kabl"
cp target/release/kabl-ui "$OUT/$NAME/bin/"
cp -r patches "$OUT/$NAME/share/kabl/patches"
cat > "$OUT/$NAME/README.txt" <<TXT
kabl $VERSION (Linux, $(uname -m)), built from $(git rev-parse --short HEAD 2>/dev/null || echo unknown).
License: MIT OR Apache-2.0.

Run:        ./bin/kabl-ui                  (opens the sound browser)
            ./bin/kabl-ui --size 1280x800  (smaller window)
Factory sounds: share/kabl/patches (read-only; Save As makes your own copy).
Your sounds:    \$XDG_DATA_HOME/kabl/sounds, usually ~/.local/share/kabl/sounds
                (favorites and recents: ~/.local/share/kabl/library.json).
Overrides:      KABL_FACTORY_DIR=<dir>, KABL_USER_DIR=<dir>.
Needs: ALSA (libasound2), X11 or Wayland with OpenGL, libxkbcommon-x11.
TXT
tar -C "$OUT" -czf "$OUT/$NAME.tar.gz" "$NAME"
echo "$OUT/$NAME.tar.gz"
