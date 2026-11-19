#!/usr/bin/env bash
# Install slopstop + the linters it drives. Best effort: never uses sudo itself,
# prints the apt line for system packages instead.
#   ./install.sh                 # core + every language
#   ./install.sh rust python     # core + only these (rust python kotlin web lua shell cpp go shaders)
set -euo pipefail

KIT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="${HOME}/.local/bin"
mkdir -p "$BIN"
ln -sf "$KIT/slop.py" "$BIN/slop"
chmod +x "$KIT/slop.py"
echo "linked $BIN/slop -> $KIT/slop.py"

want() { [[ $# -eq 0 ]] && return 0; [[ " ${LANGS} " == *" $1 "* || -z "${LANGS}" ]]; }
LANGS="$*"
APT=()

pyinst() {  # prefer pipx (isolated), fall back to pip --user
  for p in "$@"; do
    command -v pipx >/dev/null && pipx install --quiet "$p" 2>/dev/null && continue
    python3 -m pip install --user --quiet --break-system-packages "$p" || echo "!! pip install $p failed"
  done
}
npminst() {
  command -v npm >/dev/null || { echo "!! npm missing: skipping $*"; return; }
  npm install -g --silent "$@" 2>/dev/null || npm install -g --silent --prefix "$HOME/.local" "$@" || echo "!! npm install $* failed"
}
cargoinst() {
  command -v cargo >/dev/null || { echo "!! cargo missing: skipping $*"; return; }
  for c in "$@"; do command -v "${c%%-cli}" >/dev/null || cargo install --locked --quiet "$c" || echo "!! cargo install $c failed"; done
}

# core: structural rules + clone detection
pyinst ast-grep-cli
npminst jscpd
command -v gitleaks >/dev/null || echo "-- optional: gitleaks (secrets) -> https://github.com/gitleaks/gitleaks/releases (built-in regex fallback is used without it)"

want python && pyinst ruff basedpyright vulture
want shell  && { pyinst shellcheck-py; }
want web    && npminst @biomejs/biome
want lua    && cargoinst selene
want cpp    && APT+=(cppcheck clang-tidy)
want shaders && cargoinst naga-cli
want go     && echo "-- go: 'go vet' is used automatically; optional golangci-lint v2 -> https://golangci-lint.run"
want rust   && { command -v cargo >/dev/null && rustup component add clippy 2>/dev/null || true; }
if want kotlin && ! command -v detekt-cli >/dev/null; then
  if command -v java >/dev/null; then
    V=1.23.8
    mkdir -p "$HOME/.local/share"
    curl -fsSL -o /tmp/detekt.zip "https://github.com/detekt/detekt/releases/download/v$V/detekt-cli-$V.zip" \
      && unzip -qo /tmp/detekt.zip -d "$HOME/.local/share" \
      && ln -sf "$HOME/.local/share/detekt-cli-$V/bin/detekt-cli" "$BIN/detekt-cli" \
      && echo "installed detekt-cli $V" || echo "!! detekt download failed"
  else
    echo "!! kotlin: detekt needs a JDK (java) on PATH"
  fi
fi

if ((${#APT[@]})); then
  missing=(); for p in "${APT[@]}"; do command -v "$p" >/dev/null || missing+=("$p"); done
  ((${#missing[@]})) && echo "-- run: sudo apt install ${missing[*]}"
fi

case ":$PATH:" in *":$BIN:"*) ;; *) echo "-- add $BIN to PATH";; esac
echo "done. In a repo: 'slop init' then 'slop doctor'. Kit self-test: 'slop selftest'."
