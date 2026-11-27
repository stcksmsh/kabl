#!/usr/bin/env bash
# Stages the Windows package of kabl and zips it (packaging/windows/README.md). Run from the
# repository root in Git Bash on Windows, after the release builds:
#
#   packaging/windows/installer/stage.sh BIN_DIR VST3_DIR [OUT]
#
# BIN_DIR holds kabl-ui.exe, kabl_clap.dll and kabl.exe (cargo --release; kabl.exe packs the
# factory sounds and is not shipped, as in the Linux package); VST3_DIR holds the kabl.vst3
# bundle folder. Writes, in OUT (default target/dist):
#
#   kabl-<version>-windows-x64/
#     kabl-ui.exe                          standalone
#     share/kabl/patches/...               factory sounds as .kabl files, packed from patches/
#     share/kabl/factory-composites/...    factory composites, copied from composites/
#     VST3/kabl.vst3/...                   VST3 bundle, for %CommonProgramFiles%\VST3
#     CLAP/kabl.clap                       CLAP plugin, for %CommonProgramFiles%\CLAP
#     LICENSE, COPYRIGHT, TRADEMARKS.md, THIRD-PARTY-NOTICES.md, ASSET-LICENSES.md, README.txt
#   kabl-<version>-windows-x64.zip         the same folder, the portable package
#
# kabl.iss installs that folder: VST3 and CLAP into the common folders, the rest into
# Program Files. Prints the staged folder's path.
set -euo pipefail
bin=${1:?usage: stage.sh BIN_DIR VST3_DIR [OUT]}
vst3=${2:?usage: stage.sh BIN_DIR VST3_DIR [OUT]}
OUT=${3:-target/dist}
# KABL_VERSION overrides the name only (release candidates: 1.0.0-rc.1); Cargo.toml stays X.Y.Z.
VERSION=${KABL_VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)}
NAME="kabl-$VERSION-windows-x64"
dir="$OUT/$NAME"
rm -rf "$dir" "$OUT/$NAME.zip"
mkdir -p "$dir/share/kabl" "$dir/VST3" "$dir/CLAP"
cp "$bin/kabl-ui.exe" "$dir/"
cp "$bin/kabl_clap.dll" "$dir/CLAP/kabl.clap"
cp -r "$vst3/kabl.vst3" "$dir/VST3/kabl.vst3"
test -f "$dir/VST3/kabl.vst3/Contents/x86_64-win/kabl.vst3"
cp LICENSE COPYRIGHT TRADEMARKS.md THIRD-PARTY-NOTICES.md ASSET-LICENSES.md "$dir/"
"$bin/kabl.exe" pack-tree patches "$dir/share/kabl/patches"
cp -r composites "$dir/share/kabl/factory-composites"
# CRLF so Notepad on older Windows shows the lines.
sed 's/$/\r/' > "$dir/README.txt" <<TXT
kabl $VERSION (Windows, x64), built from $(git rev-parse --short HEAD).
License: GPL-3.0-or-later (LICENSE; owner and notice: COPYRIGHT; name and logo: TRADEMARKS.md;
third parties and assets: THIRD-PARTY-NOTICES.md, ASSET-LICENSES.md).
VST is a registered trademark of Steinberg Media Technologies GmbH.
Source code for this exact build: https://github.com/stcksmsh/kabl, commit $(git rev-parse HEAD).
This build is not code-signed: Windows SmartScreen may warn before the first run.

Installer:  kabl-$VERSION-windows-x64-setup.exe puts the standalone in Program Files\\kabl with a
            Start menu entry, kabl.vst3 in %CommonProgramFiles%\\VST3 and kabl.clap in
            %CommonProgramFiles%\\CLAP. Uninstall from Settings > Apps; your sounds stay.
Portable:   run kabl-ui.exe from this folder. For the plugins, copy VST3\\kabl.vst3 (the whole
            folder) to C:\\Program Files\\Common Files\\VST3 and CLAP\\kabl.clap to
            C:\\Program Files\\Common Files\\CLAP, then rescan plugins in your host.
Run:        kabl-ui.exe                  (opens the sound browser)
            kabl-ui.exe --size 1280x800  (smaller window)
Factory sounds: share\\kabl\\patches (read-only; Save As makes your own copy).
Factory composites: share\\kabl\\factory-composites (read-only; inserting one embeds a copy in the patch).
Your sounds:    %APPDATA%\\kabl\\sounds (favorites and recents: %APPDATA%\\kabl\\library.json).
Logs:           %LOCALAPPDATA%\\kabl\\logs\\kabl.log (INFO and above; up to 4 MiB rotated).
                KABL_LOG=debug (or --log-level debug) adds detail; KABL_LOG_DIR=<dir> puts
                them elsewhere. The status bar's Log button copies the path.
Overrides:      KABL_FACTORY_DIR=<dir>, KABL_FACTORY_COMPOSITES_DIR=<dir>, KABL_USER_DIR=<dir>.
TXT
(cd "$OUT" && 7z a -tzip -bso0 "$NAME.zip" "$NAME")
echo "$dir"
