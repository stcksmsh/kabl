# File browser on Windows

kabl's own file browser (`crates/ui/src/filebrowser.rs`) works on both platforms. The path rules are pure functions that take a platform (`Os::Unix` or `Os::Windows`), so both are tested on Linux (`crates/ui/tests/file_browser.rs`, module `path_styles`).

- **Home**: `paths::home_dir` (`crates/ui/src/paths.rs`): `HOME` on Linux, `USERPROFILE` on Windows. The Home button, the Home place and the `Home` crumb all use it.
- **Roots**: the root crumb is `/` on Linux, `C:\` (the drive) on Windows, and `\\server\share` for a UNC path. When nothing else is known the browser starts at the root of the system: `/`, or `%SystemDrive%\` (`C:\` when unset).
- **Drive list ("This PC")**: Up from a drive root (`C:\`) shows the drives that exist, probed as `A:\` to `Z:\`. The probe runs on a thread, so a drive without media or a stalled network drive only delays the list; the UI does not wait. In this view Choose this folder and Save are off, and Up is off. Back returns to the folder left.
- **Up** from `/` stays at `/`. Up from a UNC share's root does nothing (UNC shares are not listed as roots).
- **Breadcrumbs**: `C:\Users\x\Documents` shows `C:\ > Users > x > Documents`, or `Home > Documents` when inside the profile folder. Drive letters and names compare without regard to case on Windows.
- **Typed paths**: `/` and `\` are both accepted on Windows; `C:/Users/x` is read as `C:\Users\x`.
- **Linux**: unchanged. Its crumbs are still built from the `Path` itself, so names that are not UTF-8 keep working.
