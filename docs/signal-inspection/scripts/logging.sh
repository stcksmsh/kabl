#!/usr/bin/env bash
# Real-binary logging checks (release build, cloud container). Writes
# target/signal-inspection-logging/*.txt; see README "Logging".
#   DISPLAY=:97 docs/signal-inspection/scripts/logging.sh
set -uo pipefail
BIN=${KABL_BIN:-./target/release/kabl-ui}
OUT=target/signal-inspection-logging
rm -rf "$OUT"; mkdir -p "$OUT"
run() { # name, extra env..., then the drive script
  local name=$1; shift
  env "$@" KABL_USER_DIR="$OUT/$name" KABL_BIN="$BIN" PIPEWIRE_NODE=kabl_rec KABL_APP_LOG="$OUT/$name.stderr" \
    KABL_MIDI_PIPE=/tmp/kabl-midi KABL_PLAYER=1 KABL_ARGS="--midi kabl-pipe --perform --rate 48000 --frames 256 --record-dir $OUT/$name/recordings" \
    python3 docs/rack-migration/drive.py docs/signal-inspection/scripts/logging-run.txt 1440x900 "$OUT/$name-shots" patches/init-keyboard
  echo "exit $?" > "$OUT/$name.exit"
}
# 1. Release default: INFO and above only.
run default KABL_SAVE_FAIL=staged KABL_RECORD_FAIL_AFTER=24000
# 2. The same binary with KABL_LOG=debug.
run debug KABL_LOG=debug KABL_SAVE_FAIL=staged KABL_RECORD_FAIL_AFTER=24000
# 3. An unwritable log location (a file where the directory would be): the app runs, the
#    failure is counted and shown.
mkdir -p "$OUT/blocked"; : > "$OUT/blocked/file"
run blocked KABL_LOG_DIR="$OUT/blocked/file/logs" KABL_SAVE_FAIL=staged KABL_RECORD_FAIL_AFTER=24000
# 4. A patch that fails to load at start: ERROR, exit 1, the line reaches the file.
env KABL_USER_DIR="$OUT/badload" timeout 8 "$BIN" --patch "$OUT/no-such-patch" \
  > /dev/null 2> "$OUT/badload.stderr"; echo "exit $?" > "$OUT/badload.exit"
for n in default debug; do
  f="$OUT/$n/logs/kabl.log"
  {
    echo "== $n: $(wc -l < "$f") lines; by level:"
    awk '{print $3}' "$f" | sort | uniq -c
    echo "-- lines (session and time columns kept):"
    cat "$f"
  } > "$OUT/$n.txt"
done
{ echo "== blocked stderr"; cat "$OUT/blocked.stderr"; ls -la "$OUT/blocked"; } > "$OUT/blocked.txt"
{ echo "== badload ($(cat "$OUT/badload.exit")): $OUT/badload/logs/kabl.log"; cat "$OUT/badload/logs/kabl.log"; } > "$OUT/badload.txt"
