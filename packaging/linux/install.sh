#!/usr/bin/env bash
# Installs (or removes) kabl from an extracted release tarball. Run it from the tarball directory.
#
#   ./install.sh [--prefix DIR] [--clap-dir DIR]     default: ~/.local and ~/.clap
#   ./install.sh --uninstall [--prefix DIR] [--clap-dir DIR]
#
# Standalone: DIR/bin/kabl-ui, DIR/share/kabl/patches (factory sounds, found relative to the
# binary), a desktop entry and icons under DIR/share. CLAP plugin: CLAP-DIR/kabl.clap.
# Uninstall removes exactly those. Your own sounds (~/.local/share/kabl/sounds), library and
# logs are never touched.
set -euo pipefail
src=$(cd "$(dirname "$0")" && pwd)
prefix=$HOME/.local
clap=$HOME/.clap
uninstall=0
while [ $# -gt 0 ]; do
  case $1 in
    --prefix) prefix=${2:?--prefix needs a directory}; shift 2 ;;
    --clap-dir) clap=${2:?--clap-dir needs a directory}; shift 2 ;;
    --uninstall) uninstall=1; shift ;;
    -h|--help) sed -n '2,10p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

refresh_caches() {
  if command -v gtk-update-icon-cache >/dev/null 2>&1 && [ -f "$prefix/share/icons/hicolor/index.theme" ]; then
    gtk-update-icon-cache -q -t "$prefix/share/icons/hicolor" || true
  fi
  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q "$prefix/share/applications" || true
  fi
}

if [ "$uninstall" = 1 ]; then
  rm -f "$prefix/bin/kabl-ui" "$prefix/share/applications/kabl.desktop" "$clap/kabl.clap"
  rm -rf "$prefix/share/kabl/patches"
  rm -f "$prefix"/share/icons/hicolor/*/apps/kabl.png "$prefix/share/icons/hicolor/scalable/apps/kabl.svg"
  rmdir "$prefix/share/kabl" 2>/dev/null || true # only if empty: user sounds live there too
  refresh_caches
  echo "kabl removed from $prefix and $clap (your sounds are untouched)"
  exit 0
fi

install -Dm755 "$src/bin/kabl-ui" "$prefix/bin/kabl-ui"
install -Dm755 "$src/lib/clap/kabl.clap" "$clap/kabl.clap"
rm -rf "$prefix/share/kabl/patches"
mkdir -p "$prefix/share/kabl"
cp -r "$src/share/kabl/patches" "$prefix/share/kabl/patches"
mkdir -p "$prefix/share/icons"
cp -r "$src/share/icons/hicolor" "$prefix/share/icons/"
mkdir -p "$prefix/share/applications"
sed "s|^Exec=.*|Exec=$prefix/bin/kabl-ui|" "$src/share/applications/kabl.desktop" > "$prefix/share/applications/kabl.desktop"
refresh_caches
echo "kabl installed: $prefix/bin/kabl-ui, plugin $clap/kabl.clap"
case ":$PATH:" in *":$prefix/bin:"*) ;; *) echo "note: $prefix/bin is not on PATH; use the desktop entry or the full path" ;; esac
