#!/usr/bin/env bash
# Superseded by docs/rack-migration/closeout-real-x.sh (targets by key, real rack layout).
exec "$(dirname "$0")/../rack-migration/closeout-real-x.sh" "$@"
