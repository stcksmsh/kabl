#!/usr/bin/env bash
# One measured real-app run (docs/midi-timing/REPORT.md, "Laptop measurements"): the virtual
# controller plays EVENTS live (play_events.py | midi_player: scripted MIDI, not a physical
# controller) while drive.py runs the app on DISPLAY with UI script UI; the app records its
# own output (Perform > Record) and writes callback/MIDI statistics.
#   BIN=target/release/kabl-ui PATCH=docs/midi-timing/fixtures/gate-probe \
#   UI=docs/midi-timing/scripts/m-idle.txt EVENTS=docs/midi-timing/scripts/pulse.events \
#   OUT=/tmp/d06-measure NAME=d06-light-idle docs/midi-timing/scripts/measure.sh
# Needs a release build of examples/midi_player in ./target and Xvfb (or any X display).
set -euo pipefail
D=${OUT:?}/${NAME:?}
rm -rf "$D"
mkdir -p "$D"
python3 docs/midi-timing/scripts/play_events.py "${EVENTS:?}" "${DELAY:-6}" \
  | ./target/release/examples/midi_player 2> "$D/player.log" &
# A failed UI script must not leave this run's controller playing into the next run.
trap 'pkill -f "play_events.py ${EVENTS}" 2>/dev/null; pkill -x midi_player 2>/dev/null; true' EXIT
sleep 0.7
KABL_BIN=${BIN:?} KABL_USER_DIR="$D/user" KABL_STATS_FILE="$D/stats.txt" KABL_APP_LOG="$D/app.log" \
  KABL_DRIVE_LOG="$D/drive.log" \
  KABL_ARGS="--midi kabl-player --rate 48000 --frames ${FRAMES:-256} --perform --record-dir $D/rec" \
  python3 docs/rack-migration/drive.py "${UI:?}" 1440x900 "$D/shots" "${PATCH:?}"
wait
ls "$D"/rec/*.wav > /dev/null
