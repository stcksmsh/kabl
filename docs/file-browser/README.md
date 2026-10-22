# One file browser, and remembered folders

Branch `claude/levels-and-browser` (with the factory levels, see `docs/factory-levels/README.md`).

## What it is

`crates/ui/src/filebrowser.rs` is the browser that the wavetable editor had, made shared. It is drawn with the chrome kit, is not modal, and is not a native dialog, so it looks and behaves the same in the standalone and inside a plugin editor, and cannot hide behind a host window. A folder is listed on a thread of its own with the same caps as before (2000 rows, 100000 entries scanned); an unreadable folder shows its error, and a cut list says so. Hidden names are left out. Up, Home and Close are always there.

| Place | How to reach it | Mode and filter |
|---|---|---|
| Wavetable import | Wavetable editor, **Browse** (inside the editor window, as before) | pick a file, `.wav` |
| Open a patch | Sounds panel, Patch folder (advanced), **Browse to open** | folders only, **Choose this folder** |
| Save a patch to a folder | same section, **Browse to save** | save: folder list, name field, **Save**; an existing name asks "Replace it?" first |
| Panel art | Panel authoring window, **Browse** next to the path field | pick a file, `.png` |
| Composite package | Composites window, **Browse** next to the package path | pick a file, `.json` |
| Recordings folder | Perform view, **Browse** next to "to" (standalone only: a plugin records through the host) | folders only |

Every path field stays, so a path can still be typed or pasted; the browser only fills it in or acts on the pick. In Save mode a click on a file copies its name into the name field; a name with a folder part (`/`, `\`, `..`) is refused; the first wanted extension is added when the name has none (a patch is a folder, so it gets none). The window for the places other than the wavetable editor is one egui window (`filebrowser::window`, called from `show`); the place that asked reads its answer with `filebrowser::take(ui_state, Purpose)`, so one place never receives another's pick.

## Remembered folders

`<config>/kabl/settings.toml` (the file that holds the theme) has a `[folders]` table with the last folder of each use: `wavetables`, `patches`, `recordings`, `art`. It is written whenever one changes (browsing counts, a pick counts), shared by the standalone and every plugin instance like the theme, and validated when read: a folder that no longer exists is dropped, and its use starts in the folder of the path field, else home. A file from before this change (theme only) loads unchanged. Opening a patch folder remembers the folder the patch was in, not the patch. The recordings folder is also put in the recorder when the application starts, and a folder typed there is remembered when a take starts.

## Checks

- `crates/ui/tests/file_browser.rs` (10 tests, real egui input): the filter of each use, picks answer only their own place, folder mode, save mode (new name, existing name with Replace and Cancel, empty or path-like name refused), an unreadable folder, a folder cut at 2000 rows, the panel path field filled from a pick, and the folders surviving a restart (settings written, read back, a missing folder dropped, home as the start of an unremembered place).
- `crates/ui/tests/wavetable_editor.rs` runs unchanged against the shared browser.
- Real X input in the standalone (`kabl-ui`, Xvfb): Browse to save with a typed name (a patch folder appears), the same name again (the question appears), Browse to open (the patch loads), the recordings Browse. Captures: `media/standalone-*.webp`.
- Real X input in REAPER 7.75 (`scripts/host.py` starts it): the plugin's editor, Browse to save with a typed name (a patch folder appears), the same name again (the question appears), Browse to open, the wavetable editor's Browse into the remembered folder and a `.wav` picked and previewed. Captures: `media/plugin-*.webp`. The plugin's folders are read from `settings.toml` at start (the file was written beforehand) and written back as they change.

## Not done

- Panel art and composite package Browse were driven by tests only, not by real X input in either build.
- The browser is a window; drag-and-drop is as before (egui dropped files, the standalone only).
- No native dialog, no new dependency.
