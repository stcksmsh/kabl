#!/usr/bin/env bash
# The D06 laptop matrix: light (gate-probe) and dense (sound-palette, its sequences running)
# patches, idle and active UI (m-active.txt), for the D06 build and a baseline build, twice.
#   BASE_BIN=/path/to/baseline/kabl-ui OUT=/tmp/d06-measure docs/midi-timing/scripts/measure-all.sh
set -euo pipefail
: "${BASE_BIN:?}" "${OUT:?}"
mkdir -p "$OUT"
light=docs/midi-timing/fixtures/gate-probe
dense=patches/sound-palette
sed 's/KNOB/knob:3.rate_hz/' docs/midi-timing/scripts/m-active.txt > "$OUT/ui-light-active.txt"
cp docs/midi-timing/scripts/m-idle.txt "$OUT/ui-light-idle.txt"
sed 's/^sleep 2$/sleep 2\nclick prun:1/' docs/midi-timing/scripts/m-idle.txt > "$OUT/ui-dense-idle.txt"
sed '0,/^sleep 2$/s//sleep 2\nclick prun:1/; s/KNOB/knob:13.cutoff_hz/' docs/midi-timing/scripts/m-active.txt > "$OUT/ui-dense-active.txt"
for rep in 1 2; do
  for build in d06 base; do
    bin=target/release/kabl-ui
    [ "$build" = base ] && bin=$BASE_BIN
    for cond in light-idle light-active dense-idle dense-active; do
      patch=$light
      [ "${cond%%-*}" = dense ] && patch=$dense
      BIN=$bin PATCH=$patch UI="$OUT/ui-$cond.txt" EVENTS=docs/midi-timing/scripts/pulse.events \
        NAME="$build-$cond-$rep" docs/midi-timing/scripts/measure.sh || echo "FAILED $build-$cond-$rep"
    done
  done
done
