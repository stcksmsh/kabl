# Windows packaging

- `vst3/`: builds `kabl.vst3` from the CLAP with clap-wrapper (CI job `windows-vst3`).
- `installer/`: the Inno Setup installer and the portable zip (CI job `windows-installer`).
- `cpp-notices.md`: licences of the C++ libraries in `kabl.vst3`, appended to
  `THIRD-PARTY-NOTICES.md` by `packaging/gen-notices.py`.

## Installer and zip

CI job `windows-installer` builds both on `windows-latest` from its own release build
(`kabl-ui.exe`, `kabl_clap.dll`, and `kabl.exe`, which only packs the factory sounds and is not
shipped) and the `windows-vst3` job's bundle (`kabl-windows-vst3`), and uploads them as `kabl-windows-installer`
(`kabl-<version>-windows-x64-setup.exe`) and `kabl-windows-zip` (`kabl-<version>-windows-x64.zip`).
Neither is code-signed.

1. `installer/stage.sh BIN_DIR VST3_DIR [OUT]` (Git Bash, repository root) writes the package
   folder `kabl-<version>-windows-x64/`, with the factory sounds packed by `kabl.exe pack-tree`,
   and zips it. The folder's layout is listed at the top of the script.
2. `iscc /DAppVersion=<version> /DStageDir=<that folder> /DOutputDir=<dir> installer\kabl.iss`
   builds the installer from the same folder, so both hold the same files.

The kabl icon (`crates/ui/assets/windows/kabl.ico`) is embedded in `kabl-ui.exe` by `crates/ui/build.rs`; the
setup program uses it (`SetupIconFile`), and the Start menu entry and uninstall entry show the exe's icon.

What the installer does (per machine, needs admin):

| Installed to | From the package |
| --- | --- |
| `C:\Program Files\kabl` | `kabl-ui.exe`, `share\kabl\patches`, `share\kabl\factory-composites`, licences, `README.txt` |
| `%CommonProgramFiles%\VST3\kabl.vst3` | `VST3\kabl.vst3` (the bundle) |
| `%CommonProgramFiles%\CLAP\kabl.clap` | `CLAP\kabl.clap` |
| Start menu | `kabl`, starting `kabl-ui.exe` |

It shows `LICENSE` (GPL-3.0-or-later) before installing. It changes no `PATH` and writes no
registry keys beyond Inno Setup's uninstall entry. An upgrade replaces the factory content
and the VST3 bundle whole. The uninstaller removes exactly what was installed; user data in
`%APPDATA%\kabl` (sounds, library, config) and `%LOCALAPPDATA%\kabl` (logs) is never touched.
The `AppId` in `kabl.iss` must never change: Windows matches upgrades by it.

## CI checks

`installer/test-install.ps1` runs on the runner after the build, one step per check:

- `install`: silent install (`/VERYSILENT /SUPPRESSMSGBOXES`), then every file above exists.
- `zip`: the zip's file list equals the installed one.
- `launch`: the installed `kabl-ui.exe` starts on Mesa's software OpenGL (its DLLs copied
  beside the exe for the run, then removed), and within 120 s its log shows `start kabl-ui`
  and `factory sounds:` at the installed `share\kabl\patches`; then it is stopped. A crash,
  a timeout or "not found" fails the step.
- `upgrade`: marker files in `%APPDATA%\kabl` and `%LOCALAPPDATA%\kabl`, install again,
  uninstall; the installed files are gone and the markers remain.

Not covered by CI (a VM pass): a user name with spaces or non-ASCII letters, hosts finding
the plugins after a fresh install, and the interactive wizard.
