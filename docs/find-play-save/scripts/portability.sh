#!/usr/bin/env bash
# Installs the package tarball outside the source checkout and runs it there with a fresh
# home directory, the checkout's patches/ moved out of the way (so a fallback to the source
# tree can't hide a packaging mistake). Needs an X display (Xvfb) and xdotool.
#   packaging/linux/package.sh
#   DISPLAY=:97 docs/find-play-save/scripts/portability.sh target/dist/kabl-0.1.0-linux-x86_64.tar.gz
set -euo pipefail
REPO=$(cd "$(dirname "$0")/../../.." && pwd)
TAR=$(realpath "$1")
DEST=${DEST:-$(mktemp -d /tmp/kabl-install.XXXX)}
OUT=${OUT:-$DEST/evidence}
mkdir -p "$DEST"
tar -xzf "$TAR" -C "$DEST"
BIN=$(ls -d "$DEST"/kabl-*/bin/kabl-ui)
mkdir -p "$DEST/home" "$OUT"
mv "$REPO/patches" "$REPO/patches.hidden"
trap 'mv "$REPO/patches.hidden" "$REPO/patches"' EXIT
cd "$DEST"
env -u XDG_DATA_HOME -u KABL_USER_DIR -u KABL_FACTORY_DIR HOME="$DEST/home" \
  KABL_BIN="$BIN" KABL_APP_LOG="$OUT/app.log" \
  python3 "$REPO/docs/rack-migration/drive.py" "$REPO/docs/find-play-save/scripts/portability.txt" \
  1440x900 "$OUT" -
echo "--- app log"; grep -v '^ALSA lib' "$OUT/app.log" || true
echo "--- saved files"; find "$DEST/home" -type f | sort
