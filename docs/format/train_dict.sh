#!/bin/bash
# Train the .kabl zstd dictionary from the factory patch logs, offline, with the zstd CLI.
#
#   docs/format/train_dict.sh [DICT_ID] [MAX_BYTES]       writes crates/core/assets/kabl-dict-<ID>.zstdict
#   docs/format/train_dict.sh --holdout [MAX_BYTES]       trains on every other patch, prints the
#                                                         size of the other half with and without it
#
# A dictionary is permanent once a release writes files with its id: never retrain an id, give
# the new dictionary a new id (and keep the old one in the binary so old files still open).
# Needs: zstd >= 1.5 (CLI). Run from the repository root.
set -euo pipefail
root=$(git rev-parse --show-toplevel)
mode=train; id=1; max=32768
if [ "${1:-}" = "--holdout" ]; then mode=holdout; max=${2:-$max}; else id=${1:-$id}; max=${2:-$max}; fi
work=$(mktemp -d); trap 'rm -rf "$work"' EXIT
mapfile -t logs < <(find "$root/patches" -name log.jsonl | sort)

# Samples: each log cut into 20-line chunks, so the trainer sees many small, frame-like samples.
sample() { # LOG PREFIX
  split -l 20 -d -a 4 "$1" "$work/s/$2."
}
mkdir -p "$work/s"
i=0
for l in "${logs[@]}"; do
  if [ "$mode" = holdout ] && [ $((i % 2)) -eq 1 ]; then i=$((i+1)); continue; fi
  sample "$l" "p$i"; i=$((i+1))
done

if [ "$mode" = train ]; then
  out="$root/crates/core/assets/kabl-dict-$id.zstdict"
  mkdir -p "$(dirname "$out")"
  zstd -q --train --dictID="$id" --maxdict="$max" -o "$out" "$work"/s/* 2>/dev/null
  echo "wrote $out ($(stat -c %s "$out") bytes, dictID $id, from ${#logs[@]} logs)"
  exit 0
fi

zstd -q --train --dictID=1 --maxdict="$max" -o "$work/d" "$work"/s/* 2>/dev/null
plain=0; withd=0; i=0
for l in "${logs[@]}"; do
  if [ $((i % 2)) -eq 1 ]; then
    plain=$((plain + $(zstd -19 -c "$l" | wc -c)))
    withd=$((withd + $(zstd -19 -c -D "$work/d" "$l" | wc -c)))
  fi
  i=$((i+1))
done
echo "held-out logs, zstd -19: plain $plain, with $(stat -c %s "$work/d")-byte dictionary $withd ($(( (plain - withd) * 100 / plain )) % smaller)"
