#!/usr/bin/env bash
# Prints the CHANGELOG.md section "## <version>" (without its heading). Fails if missing or empty.
set -euo pipefail
version=${1:?usage: changelog-section.sh X.Y.Z [CHANGELOG.md]}
file=${2:-CHANGELOG.md}
out=$(awk -v v="$version" '
  /^## / { if (on) exit; on = ($2 == v); next }
  on { print }' "$file")
if [ -z "$(printf '%s' "$out" | tr -d '[:space:]')" ]; then
  echo "no CHANGELOG.md section for $version" >&2
  exit 1
fi
printf '%s\n' "$out"
