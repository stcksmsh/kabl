#!/usr/bin/env bash
# Builds a relocatable Linux package of kabl (docs/find-play-save/README.md, "Install"):
#
#   kabl-<version>-linux-<arch>/
#     bin/kabl-ui                          standalone
#     lib/clap/kabl.clap                   CLAP plugin
#     share/kabl/patches/...               factory sounds (found relative to bin/)
#     share/applications/kabl.desktop, share/icons/hicolor/...
#     install.sh                           installs (and --uninstall removes) under ~/.local and ~/.clap
#     LICENSE-*, THIRD-PARTY-NOTICES.md, ASSET-LICENSES.md, README.txt
#
# and kabl-<version>-linux-<arch>.tar.gz next to it, in $OUT (default target/dist).
# Extract anywhere and run bin/kabl-ui, or run ./install.sh. Nothing is read from the source
# checkout at run time.
set -euo pipefail
cd "$(dirname "$0")/../.."
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
NAME="kabl-$VERSION-linux-$(uname -m)"
OUT=${OUT:-target/dist}
cargo build --release -p kabl-ui --bin kabl-ui -p kabl-clap
rm -rf "$OUT/$NAME" "$OUT/$NAME.tar.gz"
mkdir -p "$OUT/$NAME/bin" "$OUT/$NAME/lib/clap" "$OUT/$NAME/share/kabl" "$OUT/$NAME/share/applications"
cp target/release/kabl-ui "$OUT/$NAME/bin/"
cp target/release/libkabl_clap.so "$OUT/$NAME/lib/clap/kabl.clap"
cp packaging/linux/kabl.desktop "$OUT/$NAME/share/applications/"
mkdir "$OUT/$NAME/share/icons"
cp -r packaging/linux/logo/hicolor "$OUT/$NAME/share/icons/hicolor"
cp packaging/linux/install.sh "$OUT/$NAME/"
cp LICENSE-MIT LICENSE-APACHE THIRD-PARTY-NOTICES.md ASSET-LICENSES.md "$OUT/$NAME/"
cp -r patches "$OUT/$NAME/share/kabl/patches"
cat > "$OUT/$NAME/README.txt" <<TXT
kabl $VERSION (Linux, $(uname -m)), built from $(git rev-parse --short HEAD 2>/dev/null || echo unknown).
License: MIT OR Apache-2.0 (LICENSE-MIT, LICENSE-APACHE; third parties and assets:
THIRD-PARTY-NOTICES.md, ASSET-LICENSES.md).

Install:    ./install.sh                   standalone and desktop entry under ~/.local,
                                           CLAP plugin as ~/.clap/kabl.clap
            ./install.sh --prefix DIR --clap-dir DIR2    elsewhere
            ./install.sh --uninstall       removes exactly those files (never your sounds)
Run:        ./bin/kabl-ui                  (opens the sound browser)
            ./bin/kabl-ui --size 1280x800  (smaller window)
Factory sounds: share/kabl/patches (read-only; Save As makes your own copy).
Your sounds:    \$XDG_DATA_HOME/kabl/sounds, usually ~/.local/share/kabl/sounds
                (favorites and recents: ~/.local/share/kabl/library.json).
Logs:           ~/.local/state/kabl/logs/kabl.log (INFO and above; up to 4 MiB rotated).
                KABL_LOG=debug ./bin/kabl-ui (or --log-level debug) adds detail;
                KABL_LOG_DIR=<dir> puts them elsewhere. The status bar's Log button copies the path.
Overrides:      KABL_FACTORY_DIR=<dir>, KABL_USER_DIR=<dir>.
Needs: ALSA (libasound2), X11 or Wayland with OpenGL, libxkbcommon-x11.
TXT
tar -C "$OUT" -czf "$OUT/$NAME.tar.gz" "$NAME"
echo "$OUT/$NAME.tar.gz"
