#!/usr/bin/env bash
# Builds the package, installs it outside the checkout, hides the checkout's patches/ and runs
# it with a fresh HOME (as D01's portability check), exercising two recipes.
#   DISPLAY=:97 docs/signal-inspection/scripts/package.sh
set -euo pipefail
REPO=$(cd "$(dirname "$0")/../../.." && pwd)
cd "$REPO"
TAR=$(packaging/linux/package.sh | tail -1)
DEST=$(mktemp -d /tmp/kabl-install.XXXX)
OUT=target/signal-inspection-package
rm -rf "$OUT"; mkdir -p "$OUT"
OUT=$(realpath "$OUT")
tar -xzf "$TAR" -C "$DEST"
BIN=$(ls -d "$DEST"/kabl-*/bin/kabl-ui)
mkdir -p "$DEST/home"
mv "$REPO/patches" "$REPO/patches.hidden"
trap 'mv "$REPO/patches.hidden" "$REPO/patches"' EXIT
(cd "$DEST" && env -u XDG_DATA_HOME -u XDG_STATE_HOME -u KABL_USER_DIR -u KABL_FACTORY_DIR -u KABL_LOG_DIR \
  HOME="$DEST/home" KABL_BIN="$BIN" KABL_APP_LOG="$OUT/app.log" PIPEWIRE_NODE=kabl_rec \
  python3 "$REPO/docs/rack-migration/drive.py" "$REPO/docs/signal-inspection/scripts/package.txt" \
  1440x900 "$OUT" -)
{
  echo "package: $(basename "$TAR"), installed in $DEST, HOME=$DEST/home, checkout patches hidden"
  echo "--- package files (recipes)"; (cd "$DEST" && find . -path '*recipes*' -name sound.toml | sort)
  echo "--- README.txt logging lines"; grep -A2 '^Logs' "$DEST"/kabl-*/README.txt
  echo "--- files written under HOME"; (cd "$DEST/home" && find . -type f | sort)
  echo "--- log"; cat "$DEST/home/.local/state/kabl/logs/kabl.log"
} > "$OUT/package.txt"
cat "$OUT/package.txt"
