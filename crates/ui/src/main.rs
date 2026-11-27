//! `kabl-ui` binary: the patchbay editor with live audio. The app lives in `app/`:
//! `startup` (arguments, first document, window), `audio` and `callback` (the output device and
//! the cpal callback, Sounds-preview mix included), `midi`, `session` (the shared core and the
//! control thread), `frame` (the per-frame UI), `retry` and `health`.

// Release builds on Windows are GUI-subsystem so no console opens beside the window; the log
// goes to a file via `kabl_standalone::applog`. Debug builds keep the console.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

fn main() -> eframe::Result<()> {
    app::run()
}
