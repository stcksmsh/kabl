#!/usr/bin/env bash
# D04 delivery check: builds the package, installs it outside the checkout, hides the
# checkout's patches/, runs it with a fresh HOME on the installed Evolving Pad with the fifo
# MIDI stand-in, turns a mapped CC and a knob, and quits. Checks the log and the stats line.
#   DISPLAY=:97 docs/runtime-controls/scripts/package.sh
set -euo pipefail
REPO=$(cd "$(dirname "$0")/../../.." && pwd)
cd "$REPO"
TAR=$(packaging/linux/package.sh | tail -1)
DEST=$(mktemp -d /tmp/kabl-install.XXXX)
OUT=$(realpath -m target/runtime-controls-package)
rm -rf "$OUT"; mkdir -p "$OUT"
tar -xzf "$TAR" -C "$DEST"
BIN=$(ls -d "$DEST"/kabl-*/bin/kabl-ui)
PAD=$(ls -d "$DEST"/kabl-*/share/kabl/patches/palette/pad)
mkdir -p "$DEST/home" "$DEST/target/release/examples"
# drive.py starts the fifo MIDI stand-in by a relative path.
cp target/release/examples/midi_player "$DEST/target/release/examples/"
mv "$REPO/patches" "$REPO/patches.hidden"
trap 'mv "$REPO/patches.hidden" "$REPO/patches"' EXIT
(cd "$DEST" && env -u XDG_DATA_HOME -u XDG_STATE_HOME -u KABL_USER_DIR -u KABL_FACTORY_DIR -u KABL_LOG_DIR \
  HOME="$DEST/home" KABL_BIN="$BIN" KABL_APP_LOG="$OUT/app.log" PIPEWIRE_NODE=kabl_rec \
  KABL_STATS_FILE="$OUT/stats.txt" KABL_MIDI_PIPE=/tmp/kabl-midi KABL_PLAYER=1 \
  KABL_ARGS="--midi kabl-pipe --perform --rate 48000 --frames 256" \
  python3 "$REPO/docs/rack-migration/drive.py" "$REPO/docs/runtime-controls/scripts/package.txt" \
  1440x900 "$OUT" "$PAD")
{
  echo "package: $(basename "$TAR"), installed in $DEST, HOME=$DEST/home, checkout patches hidden"
  echo "--- stats"; cat "$OUT/stats.txt"
  echo "--- log"; cat "$DEST/home/.local/state/kabl/logs/kabl.log"
} > "$OUT/package.txt"
cat "$OUT/package.txt"
