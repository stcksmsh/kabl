# File browser on Windows

kabl's own file browser (`crates/ui/src/filebrowser.rs`) works on both platforms. The path rules are pure functions that take a platform (`Os::Unix` or `Os::Windows`), so both are tested on Linux (`crates/ui/tests/file_browser.rs`, module `path_styles`).

- **Home**: `paths::home_dir` (`crates/ui/src/paths.rs`): `HOME` on Linux, `USERPROFILE` on Windows. The Home button, the Home place and the `Home` crumb all use it.
- **Roots**: the root crumb is `/` on Linux, `C:\` (the drive) on Windows, and `\\server\share` for a UNC path. When nothing else is known the browser starts at the root of the system: `/`, or `%SystemDrive%\` (`C:\` when unset).
- **Drive list ("This PC")**: Up from a drive root (`C:\`) shows the drives that exist, probed as `A:\` to `Z:\`. The probe runs on a thread, so a drive without media or a stalled network drive only delays the list; the UI does not wait. In this view Choose this folder and Save are off, and Up is off. Back returns to the folder left.
- **Up** from `/` stays at `/`. Up from a UNC share's root does nothing (UNC shares are not listed as roots).
- **Breadcrumbs**: `C:\Users\x\Documents` shows `C:\ > Users > x > Documents`, or `Home > Documents` when inside the profile folder. Drive letters and names compare without regard to case on Windows.
- **Typed paths**: `/` and `\` are both accepted on Windows; `C:/Users/x` is read as `C:\Users\x`.
- **Linux**: unchanged. Its crumbs are still built from the `Path` itself, so names that are not UTF-8 keep working.

## Drive list and media

The drive letters come from `GetLogicalDrives` (declared in `filebrowser.rs` with a one-function `extern "system"`; no new dependency). Nothing is probed, so an empty card reader or optical drive does not raise "There is no disk in the drive" and a dead network drive does not stall the list. Such a drive is listed; opening it shows the read error in the list.

Back keeps the drive list in its history: Up from `C:\` shows the drives, opening `D:\` and pressing Back returns to the drives, and Back again to the folder before.

Verbatim paths (`\\?\C:\x`, `\\?\UNC\srv\share`) are read as the plain drive and UNC paths.

## Known gaps

- Unpaired UTF-16 surrogates in a Windows name are shown with a replacement character (names go through `to_string_lossy`).
- Drive-relative paths (`C:foo`) are not handled specially.

## Needs a real Windows check

For the W acceptance pass, on a real Windows machine:

- [ ] The drive list ("This PC") in the real UI: Up from `C:\`, open `D:\`, Back, Back.
- [ ] Empty card readers, optical and disconnected network drives: no system dialog, no stall, a clear message when opened.
- [ ] Home button and `Home` crumb come from `%USERPROFILE%`.
- [ ] Crumbs on the real `Documents` folder (also when it is redirected, e.g. to OneDrive).
- [ ] UNC browsing (`\\server\share\...`): crumbs, Up stops at the share.
- [ ] Save and open a `.kabl` file in Documents.
