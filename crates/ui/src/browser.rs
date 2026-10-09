//! The sound browser (left panel), the document the editor holds (name, where it came from,
//! the state last saved), audition and piece transport, and the Save / Save As / Rename /
//! unsaved-changes dialogs. Library files: `library.rs`. Rules:
//! docs/find-play-save/README.md.
//!
//! - **Unsaved** means the patch differs from the state last opened or saved
//!   (`PatchState` equality), so undoing back to it is clean again. The audio-rebuild flag
//!   (`PatchEditor::is_dirty`) and runtime state (transport, edit banks) never count.
//! - Browsing never makes sound: a click selects, **Open** (or a double click) loads.
//!   Keyboard sounds sound only from **Play**; pieces open stopped and run from **Start**.
//! - A failed open or save changes nothing in the editor.

use crate::kit::{self, Ic, Tip, Tone};
use crate::style::{Role, Style};
use crate::wheel::OwnedScroll;
use egui::{vec2, Sense};
use kabl_core::{PatchLog, PatchState};
use kabl_engine::patch_engine::{Command, MAX_PREVIEW_NOTES, MAX_PREVIEW_SECS};
use kabl_modules::builtins::Transport;

use crate::library::{Entry, LibError, Library, Meta, Origin, CATEGORIES};
use crate::{PatchEditor, UiState};

pub const PANEL_W: f32 = 300.0;
pub const INIT_ID: &str = "factory:init-keyboard";

/// Where the open patch lives.
#[derive(Debug, Clone, PartialEq)]
pub enum DocOrigin {
    /// Not saved anywhere yet (a new sound, the built-in start patch).
    New,
    /// A library sound, by id.
    Library(String),
    /// A patch folder opened by path (`--patch`, the advanced folder field).
    Folder(String),
}

/// The open document: what Save writes and what "unsaved" compares against.
#[derive(Debug, Clone)]
pub struct Doc {
    pub name: String,
    pub origin: DocOrigin,
    /// Category, tags and description carried into a Save As.
    pub meta: Meta,
    pub saved: PatchState,
}

impl Doc {
    pub fn new(name: &str, origin: DocOrigin, state: &PatchState) -> Doc {
        let (keys, sequence) = Meta::play_of(state);
        Doc {
            name: name.to_string(),
            origin,
            meta: Meta {
                keys,
                sequence,
                ..Meta::default()
            },
            saved: state.clone(),
        }
    }
}

/// What waits for the unsaved-changes question.
#[derive(Debug, Clone, PartialEq)]
pub enum Pending {
    Open(String),
    OpenFolder(String),
    New,
    Quit,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Dialog {
    Unsaved {
        then: Pending,
        error: Option<String>,
    },
    SaveAs {
        name: String,
        category: String,
        tags: String,
        /// The user sound that has the name tried last (id, that name), offered for
        /// replacement only while the name field still says that name.
        taken: Option<(String, String)>,
        then: Option<Pending>,
        error: Option<String>,
    },
    Rename {
        id: String,
        name: String,
        error: Option<String>,
    },
    /// Restore the comparison reference over the whole current patch (`compare.rs`).
    Restore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    Favorites,
    Recent,
    Factory,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chord {
    Single,
    Major,
    Minor,
}

impl Chord {
    fn intervals(self) -> &'static [u8] {
        match self {
            Chord::Single => &[0],
            Chord::Major => &[0, 4, 7],
            Chord::Minor => &[0, 3, 7],
        }
    }
    fn label(self) -> &'static str {
        match self {
            Chord::Single => "Note",
            Chord::Major => "Major",
            Chord::Minor => "Minor",
        }
    }
}

/// Audition settings (view state, not saved).
#[derive(Debug, Clone)]
pub struct Audition {
    pub note: u8,
    pub chord: Chord,
    pub velocity: u8,
    pub secs: f32,
}

impl Default for Audition {
    fn default() -> Self {
        Audition {
            note: 60,
            chord: Chord::Single,
            velocity: 90,
            secs: 1.5,
        }
    }
}

impl Audition {
    pub fn notes(&self) -> Vec<u8> {
        self.chord
            .intervals()
            .iter()
            .map(|i| self.note.saturating_add(*i).min(127))
            .collect()
    }

    pub fn command(&self, sample_rate: f32) -> Command {
        let mut notes = [0u8; MAX_PREVIEW_NOTES];
        let n = self.notes();
        notes[..n.len()].copy_from_slice(&n);
        Command::Preview {
            notes,
            count: n.len() as u8,
            velocity: self.velocity,
            blocks: (self.secs.min(MAX_PREVIEW_SECS) * sample_rate
                / kabl_engine::graph::BLOCK as f32)
                .ceil() as u32,
        }
    }
}

pub fn note_name(n: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!("{}{}", NAMES[n as usize % 12], n as i32 / 12 - 1)
}

/// Browser view state. Never saved with a patch, never undone.
pub struct Browser {
    pub query: String,
    pub filter: Filter,
    pub category: Option<String>,
    pub selected: Option<String>,
    pub audition: Audition,
    pub dialog: Option<Dialog>,
    /// A preview was started and may still sound (egui time it ends).
    pub preview_until: Option<f64>,
    pub folder: String,
    /// The advanced patch-folder disclosure is open.
    pub folder_open: bool,
    /// The window had focus last frame.
    focused: bool,
    /// The standalone app runs file operations on one owned worker. Library-only tests use
    /// the synchronous path so their observable calls remain deterministic.
    pub async_io: bool,
    /// Deterministic worker delay for recovery tests.
    pub io_delay_ms: u64,
    io: Option<IoTask>,
}

#[derive(Clone)]
enum IoRequest {
    Perform(Pending),
    Save(Option<Pending>),
    SaveAs {
        name: String,
        category: String,
        tags: String,
        replace: Option<String>,
        then: Option<Pending>,
    },
    SaveFolder(String),
}

struct IoTask {
    handle: std::thread::JoinHandle<IoDone>,
    before: PatchState,
    origin: Option<DocOrigin>,
    request: IoRequest,
}

struct IoDone {
    library: Option<Library>,
    doc: Option<Doc>,
    log: Option<PatchLog>,
    load_stopped: bool,
    selected: Option<String>,
    message: Option<String>,
    error: Option<(Option<(String, String)>, String)>,
}

impl Drop for Browser {
    fn drop(&mut self) {
        if let Some(task) = self.io.take() {
            // An in-flight transactional write must not be detached at shutdown.
            let _ = task.handle.join();
        }
    }
}

impl Default for Browser {
    fn default() -> Self {
        Browser {
            query: String::new(),
            filter: Filter::All,
            category: None,
            selected: None,
            audition: Audition::default(),
            dialog: None,
            preview_until: None,
            folder: "my-patch".into(),
            folder_open: false,
            focused: true,
            async_io: false,
            io_delay_ms: 0,
            io: None,
        }
    }
}

pub fn io_busy(ui: &UiState) -> bool {
    ui.browser.io.is_some()
}

fn start_io(editor: &PatchEditor, ui: &mut UiState, request: IoRequest) {
    if io_busy(ui) {
        message(ui, "finish the current file operation first".into());
        return;
    }
    let before = editor.state().clone();
    let origin = ui.doc.as_ref().map(|d| d.origin.clone());
    let log = editor.log().clone();
    let library = ui.library.clone();
    let doc = ui.doc.clone();
    let work = request.clone();
    let delay = ui.browser.io_delay_ms.min(5000);
    let handle = std::thread::Builder::new()
        .name("kabl-document-io".into())
        .spawn(move || {
            if delay > 0 {
                std::thread::sleep(std::time::Duration::from_millis(delay));
            }
            let mut scratch = UiState {
                library,
                doc,
                ..UiState::default()
            };
            let mut scratch_editor = PatchEditor::from_log(log);
            let mut error = None;
            match work {
                IoRequest::Perform(p) => perform(&mut scratch_editor, &mut scratch, p),
                IoRequest::Save(_) => {
                    error = save(&mut scratch_editor, &mut scratch, None).map(|e| (None, e));
                }
                IoRequest::SaveAs {
                    name,
                    category,
                    tags,
                    replace,
                    ..
                } => {
                    error = save_as(
                        &mut scratch_editor,
                        &mut scratch,
                        &name,
                        &category,
                        &tags,
                        replace,
                        None,
                    )
                    .err();
                }
                IoRequest::SaveFolder(path) => {
                    match crate::library::save_folder(
                        std::path::Path::new(&path),
                        scratch_editor.log(),
                    ) {
                        Ok(()) => {
                            let name = std::path::Path::new(&path)
                                .file_name()
                                .map_or(path.clone(), |n| n.to_string_lossy().to_string());
                            scratch.doc = Some(Doc::new(
                                &name,
                                DocOrigin::Folder(path.clone()),
                                scratch_editor.state(),
                            ));
                            message(&mut scratch, format!("saved to {path}"));
                        }
                        Err(e) => {
                            let text = save_failed(&e);
                            message(&mut scratch, text.clone());
                            error = Some((None, text));
                        }
                    }
                }
            }
            IoDone {
                library: scratch.library,
                doc: scratch.doc,
                log: scratch.loaded.then(|| scratch_editor.log().clone()),
                load_stopped: scratch.load_stopped,
                selected: scratch.browser.selected.take(),
                message: scratch.last_message,
                error,
            }
        })
        .expect("spawn one document I/O worker");
    ui.browser.io = Some(IoTask {
        handle,
        before,
        origin,
        request,
    });
    message(
        ui,
        "file operation in progress; editing remains available".into(),
    );
}

/// Reconcile one finished transaction without blocking the editor or the CC pump on I/O.
/// A load never replaces an edit accepted while the worker was reading its patch.
pub fn poll_io(editor: &mut PatchEditor, ui: &mut UiState) {
    if !ui
        .browser
        .io
        .as_ref()
        .is_some_and(|t| t.handle.is_finished())
    {
        return;
    }
    let task = ui.browser.io.take().unwrap();
    let Ok(done) = task.handle.join() else {
        message(ui, "file operation failed; working patch kept".into());
        return;
    };
    let same_doc = ui.doc.as_ref().map(|d| &d.origin) == task.origin.as_ref();
    match task.request {
        IoRequest::Perform(p) => {
            if let Some(log) = done.log {
                if same_doc && editor.state() == &task.before {
                    replace_patch(editor, ui, log);
                    ui.doc = done.doc;
                    ui.load_stopped = done.load_stopped;
                    ui.library = done.library;
                    ui.last_message = done.message;
                } else {
                    message(
                        ui,
                        "the patch changed while loading; confirm the new open".into(),
                    );
                    request(editor, ui, p);
                }
            } else {
                ui.last_message = done.message;
            }
        }
        IoRequest::Save(then) => {
            if let Some((_, e)) = done.error {
                if let Some(p) = then {
                    ui.browser.dialog = Some(Dialog::Unsaved {
                        then: p,
                        error: Some(e),
                    });
                } else {
                    message(ui, e);
                }
            } else if same_doc {
                ui.library = done.library;
                ui.doc = done.doc;
                ui.last_message = done.message;
                if let Some(p) = then {
                    request(editor, ui, p);
                }
            }
        }
        IoRequest::SaveAs {
            name,
            category,
            tags,
            then,
            ..
        } => {
            if let Some((taken, e)) = done.error {
                ui.browser.dialog = Some(Dialog::SaveAs {
                    name,
                    category,
                    tags,
                    taken,
                    then,
                    error: Some(e),
                });
            } else if same_doc {
                ui.library = done.library;
                ui.doc = done.doc;
                ui.browser.query.clear();
                ui.browser.category = None;
                ui.browser.selected = done.selected;
                ui.last_message = done.message;
                if let Some(p) = then {
                    request(editor, ui, p);
                }
            }
        }
        IoRequest::SaveFolder(_) => {
            if done.error.is_none() && same_doc {
                ui.doc = done.doc;
            }
            ui.last_message = done.message;
        }
    }
}

pub fn is_modified(editor: &PatchEditor, ui: &UiState) -> bool {
    ui.doc.as_ref().is_some_and(|d| &d.saved != editor.state())
}

/// Clears everything tied to the old patch and installs `log` as a fresh graph. Runtime
/// state (transport, launches, notes) does not carry into the new sound.
pub fn replace_patch(editor: &mut PatchEditor, ui: &mut UiState, log: PatchLog) {
    ui.launches.push(Command::PreviewStop);
    ui.browser.preview_until = None;
    *editor = PatchEditor::from_log(log);
    if editor
        .state()
        .modules
        .keys()
        .any(|&id| editor.state().label(id, "guide").is_some())
    {
        ui.perform_open = true;
    }
    // A Load replaces a live patch; the audio host only rebuilds on `take_dirty`.
    editor.mark_dirty();
    ui.loaded = true;
    ui.delay_status.clear();
    ui.seq_banks.clear();
    ui.seq_steps.clear();
    ui.clock_running.clear();
    ui.lfo_status.clear();
    ui.edit_bank.clear();
    ui.selected_module = None;
    ui.inspected = None;
    ui.selected_route = None;
    ui.pending_output = None;
    ui.choose = None;
    ui.learn = None;
    ui.takeover.clear();
}

/// Asks before `p` replaces unsaved work; runs it at once when nothing is unsaved.
pub fn request(editor: &mut PatchEditor, ui: &mut UiState, p: Pending) {
    if io_busy(ui) {
        message(ui, "finish the current file operation first".into());
        return;
    }
    if is_modified(editor, ui) {
        ui.browser.dialog = Some(Dialog::Unsaved {
            then: p,
            error: None,
        });
    } else {
        perform(editor, ui, p);
    }
}

/// A library id's kind for the log: the factory id, or just "user" (user ids carry names
/// the user typed, which the log does not record).
fn origin_kind(id: &str) -> &str {
    if id.starts_with("factory:") {
        id
    } else {
        "user"
    }
}

fn doc_kind(o: &DocOrigin) -> &str {
    match o {
        DocOrigin::New => "new",
        DocOrigin::Library(id) => origin_kind(id),
        DocOrigin::Folder(_) => "folder",
    }
}

fn message(ui: &mut UiState, text: String) {
    ui.last_message = Some(text);
}

/// Does `p` without asking. A failure leaves the editor as it was and says why.
pub fn perform(editor: &mut PatchEditor, ui: &mut UiState, p: Pending) {
    if ui.browser.async_io && p != Pending::Quit {
        start_io(editor, ui, IoRequest::Perform(p));
        return;
    }
    match p {
        Pending::Quit => ui.quit_now = true,
        Pending::Open(id) => {
            let Some(lib) = ui.library.as_mut() else {
                return;
            };
            match lib.read(&id) {
                Ok(log) => {
                    let e = lib.get(&id).cloned();
                    let note = lib.touch(&id);
                    let Some(e) = e else { return };
                    replace_patch(editor, ui, log);
                    log::info!(target: "doc", "open ok origin={} editor={}", origin_kind(&id), editor.instance());
                    ui.load_stopped = Meta::play_of(editor.state()).1;
                    ui.doc = Some(Doc {
                        name: e.meta.name.clone(),
                        origin: DocOrigin::Library(id),
                        meta: e.meta.clone(),
                        saved: editor.state().clone(),
                    });
                    message(
                        ui,
                        note.unwrap_or_else(|| {
                            format!(
                                "opened \"{}\"{}",
                                e.meta.name,
                                if e.meta.sequence {
                                    " (stopped: press Start)"
                                } else {
                                    ""
                                }
                            )
                        }),
                    );
                }
                Err(err) => {
                    log::warn!(target: "doc", "open failed origin={} error={}; working patch kept", origin_kind(&id), err.log_text());
                    message(
                        ui,
                        format!("couldn't open: {err}. Your sound is unchanged."),
                    )
                }
            }
        }
        Pending::OpenFolder(path) => {
            // Settle an interrupted save first, and say so.
            let repaired = crate::library::repair_folder(std::path::Path::new(&path));
            match crate::library::read_patch(std::path::Path::new(&path)) {
                Ok(log) => {
                    replace_patch(editor, ui, log);
                    log::info!(target: "doc", "open ok origin=folder editor={}", editor.instance());
                    let name = std::path::Path::new(&path)
                        .file_name()
                        .map_or(path.clone(), |n| n.to_string_lossy().to_string());
                    ui.doc = Some(Doc::new(
                        &name,
                        DocOrigin::Folder(path.clone()),
                        editor.state(),
                    ));
                    message(
                        ui,
                        repaired.map_or_else(
                            || format!("loaded {path}"),
                            |n| format!("loaded {path} ({n})"),
                        ),
                    );
                }
                Err(err) => message(ui, {
                    log::warn!(target: "doc", "open failed origin=folder error={}; working patch kept", err.log_text());
                    format!(
                        "load failed: {err}. Your sound is unchanged.{}",
                        repaired.map_or(String::new(), |n| format!(" ({n})"))
                    )
                }),
            }
        }
        Pending::New => {
            let log = ui
                .library
                .as_ref()
                .and_then(|l| l.read(INIT_ID).ok())
                .unwrap_or_else(|| {
                    PatchEditor::seed_from(&kabl_standalone::default_patch())
                        .log()
                        .clone()
                });
            replace_patch(editor, ui, log);
            let mut doc = Doc::new("Untitled", DocOrigin::New, editor.state());
            doc.meta.category = "Basic".into();
            ui.doc = Some(doc);
            log::info!(target: "doc", "new editor={}", editor.instance());
            message(ui, "new sound from Init Keyboard".into());
        }
    }
}

/// Save to where the document lives, or ask for a name (factory, new). `then` runs after a
/// successful save. Returns an error message when the save failed (the dialog shows it).
pub fn save(editor: &mut PatchEditor, ui: &mut UiState, then: Option<Pending>) -> Option<String> {
    let doc = ui.doc.clone()?;
    if ui.browser.async_io {
        let writable = match &doc.origin {
            DocOrigin::Library(id) => ui
                .library
                .as_ref()
                .and_then(|l| l.get(id))
                .is_some_and(|e| e.origin == Origin::User),
            DocOrigin::Folder(path) => !ui
                .library
                .as_ref()
                .is_some_and(|l| l.is_factory_path(std::path::Path::new(path))),
            DocOrigin::New => false,
        };
        if writable {
            start_io(editor, ui, IoRequest::Save(then));
        } else {
            open_save_as(ui, then);
        }
        return None;
    }
    let result = match &doc.origin {
        DocOrigin::Library(id)
            if ui
                .library
                .as_ref()
                .and_then(|l| l.get(id))
                .is_some_and(|e| e.origin == Origin::User) =>
        {
            // The sound keeps its library name and metadata; only the patch is new.
            let lib = ui.library.as_mut().unwrap();
            let meta = lib.get(id).unwrap().meta.clone();
            lib.save(editor.log(), meta, Some(id)).map(|_| ())
        }
        DocOrigin::Folder(path)
            if !ui
                .library
                .as_ref()
                .is_some_and(|l| l.is_factory_path(std::path::Path::new(path))) =>
        {
            crate::library::save_folder(std::path::Path::new(path), editor.log())
        }
        _ => {
            open_save_as(ui, then);
            return None;
        }
    };
    match result {
        Ok(()) => {
            // The library's name for the sound is the one on disk (an outside edit may differ).
            let name = match &doc.origin {
                DocOrigin::Library(id) => ui
                    .library
                    .as_ref()
                    .and_then(|l| l.get(id))
                    .map_or(doc.name.clone(), |e| e.meta.name.clone()),
                _ => doc.name.clone(),
            };
            if let Some(d) = ui.doc.as_mut() {
                d.saved = editor.state().clone();
                d.name = name.clone();
            }
            log::info!(target: "doc", "save ok origin={}", doc_kind(&doc.origin));
            message(ui, format!("saved \"{name}\""));
            if let Some(p) = then {
                perform(editor, ui, p);
            }
            None
        }
        Err(e) => {
            log::warn!(target: "doc", "save failed origin={} error={}; edits kept", doc_kind(&doc.origin), e.log_text());
            let text = save_failed(&e);
            message(ui, text.clone());
            Some(text)
        }
    }
}

/// What a failed save tells the user: the saved copy on disk is unchanged (every save path
/// restores it on failure) unless restoring itself failed.
fn save_failed(e: &LibError) -> String {
    match e {
        LibError::Interrupted(_) => format!("save failed: {e}. Your edits are still here."),
        _ => format!("save failed: {e}. Nothing was overwritten; your edits are still here."),
    }
}

fn open_save_as(ui: &mut UiState, then: Option<Pending>) {
    let (name, category, tags) = match &ui.doc {
        Some(d) => (
            match &d.origin {
                DocOrigin::Library(id) if id.starts_with("factory:") => format!("My {}", d.name),
                _ => d.name.clone(),
            },
            if d.meta.category.is_empty() {
                "Basic".into()
            } else {
                d.meta.category.clone()
            },
            d.meta.tags.join(", "),
        ),
        None => ("Untitled".into(), "Basic".into(), String::new()),
    };
    ui.browser.dialog = Some(Dialog::SaveAs {
        name,
        category,
        tags,
        taken: None,
        then,
        error: None,
    });
}

/// Save As with the dialog's fields. `replace`: the user agreed to overwrite that sound.
fn save_as(
    editor: &mut PatchEditor,
    ui: &mut UiState,
    name: &str,
    category: &str,
    tags: &str,
    replace: Option<String>,
    then: Option<Pending>,
) -> Result<(), (Option<(String, String)>, String)> {
    if ui.browser.async_io {
        start_io(
            editor,
            ui,
            IoRequest::SaveAs {
                name: name.into(),
                category: category.into(),
                tags: tags.into(),
                replace,
                then,
            },
        );
        return Ok(());
    }
    let Some(lib) = ui.library.as_mut() else {
        return Err((None, "the sound library isn't available".into()));
    };
    let meta = Meta {
        name: name.trim().to_string(),
        category: category.to_string(),
        tags: tags
            .split(',')
            .map(|t| t.trim().to_lowercase())
            .filter(|t| !t.is_empty())
            .collect(),
        description: ui
            .doc
            .as_ref()
            .map(|d| d.meta.description.clone())
            .unwrap_or_default(),
        ..Meta::default()
    };
    match lib.save(editor.log(), meta, replace.as_deref()) {
        Ok(id) => {
            let e = lib.get(&id).cloned();
            if let Some(e) = e {
                ui.doc = Some(Doc {
                    name: e.meta.name.clone(),
                    origin: DocOrigin::Library(id.clone()),
                    meta: e.meta.clone(),
                    saved: editor.state().clone(),
                });
                // Show the new sound in the list, whatever was searched before.
                ui.browser.query.clear();
                ui.browser.category = None;
                ui.browser.selected = Some(id);
                log::info!(target: "doc", "save-as ok origin=user");
                message(ui, format!("saved \"{}\" in Your Sounds", e.meta.name));
            }
            if let Some(p) = then {
                perform(editor, ui, p);
            }
            Ok(())
        }
        Err(LibError::NameTaken(id)) => Err((
            Some((id, name.trim().to_string())),
            format!("\"{}\" is already in Your Sounds.", name.trim()),
        )),
        Err(e) => {
            log::warn!(target: "doc", "save-as failed error={}; edits kept", e.log_text());
            let text = save_failed(&e);
            message(ui, text.clone());
            Err((None, text))
        }
    }
}

fn hit(ui_state: &mut UiState, key: &str, r: &egui::Response) {
    ui_state.record(key.to_string(), r.rect);
}

/// Every frame, before drawing: focus loss stops a preview; Ctrl+S saves.
pub fn frame_input(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &egui::Ui) {
    if ui_state.doc.is_none() {
        ui_state.doc = Some(Doc::new("Untitled", DocOrigin::New, editor.state()));
    }
    let focused = ui.input(|i| i.focused);
    if ui_state.browser.focused && !focused && ui_state.browser.preview_until.is_some() {
        ui_state.launches.push(Command::PreviewStop);
        ui_state.browser.preview_until = None;
    }
    ui_state.browser.focused = focused;
    let now = ui.input(|i| i.time);
    if ui_state.browser.preview_until.is_some_and(|t| now > t) {
        ui_state.browser.preview_until = None;
    }
    if !io_busy(ui_state)
        && ui_state.browser.dialog.is_none()
        && ui.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::S,
            ))
        })
    {
        save(editor, ui_state, None);
    }
}

/// The document name, unsaved mark, Save and Save As, for the toolbar.
pub fn toolbar(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui, st: &Style) {
    let modified = is_modified(editor, ui_state);
    let name = ui_state
        .doc
        .as_ref()
        .map_or("", |d| d.name.as_str())
        .to_string();
    let factory = matches!(
        ui_state.doc.as_ref().map(|d| &d.origin),
        Some(DocOrigin::Library(id)) if id.starts_with("factory:")
    );
    let text = format!("{}{name}", if modified { "● " } else { "" });
    let r = ui
        .add_sized(
            [140.0, 20.0],
            egui::Label::new(kit::rich(st, Role::H3, Tone::Text, text)).selectable(false).truncate(),
        )
        .tip(
            st,
            &if modified {
                format!("{name}: unsaved changes")
            } else if factory {
                format!("{name}: factory sound (Save makes your own copy)")
            } else {
                name.clone()
            },
        );
    hit(ui_state, "doc-name", &r);
    let r = ui
        .add(kit::Button::new(st, "Save").icon(Ic::Save).enabled(!io_busy(ui_state)))
        .tip(st, "Ctrl+S");
    hit(ui_state, "save", &r);
    if r.clicked() {
        save(editor, ui_state, None);
    }
    let r = ui.add(kit::Button::new(st, "Save As"));
    hit(ui_state, "save-as", &r);
    if r.clicked() {
        open_save_as(ui_state, None);
    }
}

fn badge(ui: &mut egui::Ui, st: &Style, e: &Meta) {
    let (text, c) = match (e.keys, e.sequence) {
        (true, true) => ("Seq + Keys", st.sections.fx.base),
        (false, true) => ("Sequence", st.sections.timing.base),
        (true, false) => ("Keys", st.sections.osc.base),
        (false, false) => ("—", st.roles.text3),
    };
    kit::tag(ui, st, text, c);
}

/// One sound as a card row: star, name, category, play-kind tag. Returns (row, star) responses.
fn sound_row(ui: &mut egui::Ui, st: &Style, e: &Entry, fav: bool, sel: bool) -> (egui::Response, egui::Response) {
    let w = ui.available_width();
    let (rect, row) = ui.allocate_exact_size(vec2(w, st.metrics.row_h), Sense::click());
    let hover = ui.ctx().animate_bool_with_time(row.id.with("hover"), row.hovered(), Style::secs(st.motion.hover));
    kit::row_bg(ui, st, rect, sel, hover);
    let star_rect = egui::Rect::from_center_size(rect.left_center() + vec2(st.sp(3) + 4.0, 0.0), vec2(st.metrics.control_h, st.metrics.control_h));
    let star = ui.interact(star_rect, row.id.with("star"), Sense::click());
    let ink = if fav { st.roles.warn } else if star.hovered() { st.roles.text } else { st.roles.text3 };
    kit::icon(ui.painter(), if fav { Ic::StarFill } else { Ic::Star }, star_rect.center(), st.metrics.icon + 2.0, ink);
    let x = star_rect.right() + st.sp(1);
    let right_pad = 78.0;
    let name = kit::fit_text(ui, st, &e.meta.name, rect.right() - right_pad - x);
    let g = ui.painter().layout_no_wrap(name, st.font(Role::Body), st.roles.text);
    ui.painter().galley(egui::pos2(x, rect.top() + st.sp(2)), g, st.roles.text);
    let cat = if e.meta.category.is_empty() { "Uncategorized" } else { &e.meta.category };
    let g = ui.painter().layout_no_wrap(cat.to_string(), st.font(Role::Caption), st.roles.text2);
    ui.painter().galley(egui::pos2(x, rect.bottom() - st.sp(2) - g.size().y), g, st.roles.text2);
    let mut tag_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(st.sp(2), 0.0))).layout(egui::Layout::right_to_left(egui::Align::Center)));
    badge(&mut tag_ui, st, &e.meta);
    (row, star)
}

/// The left panel.
pub fn panel(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui) {
    let st = ui_state.style.clone();
    let st = &*st;
    if io_busy(ui_state) {
        kit::paragraph(ui, st, Role::Body, Tone::Text2, "Reading or saving a sound… editing remains available.");
        return;
    }
    ui.horizontal(|ui| {
        kit::label(ui, st, Role::Title, Tone::Text, "Sounds");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let r = ui.add(kit::Button::new(st, "Close").ghost());
            hit(ui_state, "browser-close", &r);
            if r.clicked() {
                ui_state.browser_open = false;
            }
            let r = ui
                .add(kit::Button::new(st, "New").icon(Ic::Plus))
                .tip(st, "Start a new sound from Init Keyboard");
            hit(ui_state, "new", &r);
            if r.clicked() {
                request(editor, ui_state, Pending::New);
            }
        });
    });
    kit::gap(ui, st, 1);
    if ui_state.library.is_none() {
        kit::label(ui, st, Role::Body, Tone::Text2, "The sound library isn't available.");
        return;
    }
    let r = kit::field(ui, st, &mut ui_state.browser.query, "Search name, purpose, mood, tempo", Some(Ic::Search), None);
    hit(ui_state, "search", &r);
    kit::gap(ui, st, 1);
    ui.horizontal_wrapped(|ui| {
        for (f, key, label) in [
            (Filter::All, "all", "All"),
            (Filter::Favorites, "favorites", "★ Favorites"),
            (Filter::Recent, "recent", "Recent"),
            (Filter::Factory, "factory", "Factory"),
            (Filter::User, "user", "Your Sounds"),
        ] {
            let r = ui.add(kit::Button::new(st, label).small().selected(ui_state.browser.filter == f));
            hit(ui_state, &format!("filter:{key}"), &r);
            if r.clicked() {
                ui_state.browser.filter = f;
            }
        }
    });
    ui.horizontal(|ui| {
        kit::label(ui, st, Role::Label, Tone::Text2, "Category");
        let current = ui_state
            .browser
            .category
            .clone()
            .unwrap_or_else(|| "Any".into());
        let r = kit::dropdown(ui, st, "browser-category", &current, ui.available_width(), |ui| {
            let r = kit::menu_item(ui, st, "Any", ui_state.browser.category.is_none());
            hit(ui_state, "category:Any", &r);
            if r.clicked() {
                ui_state.browser.category = None;
            }
            for c in CATEGORIES {
                let r = kit::menu_item(ui, st, c, ui_state.browser.category.as_deref() == Some(*c));
                hit(ui_state, &format!("category:{c}"), &r);
                if r.clicked() {
                    ui_state.browser.category = Some(c.to_string());
                }
            }
        });
        hit(ui_state, "category", &r);
    });
    kit::gap(ui, st, 1);
    kit::rule(ui, st);
    kit::gap(ui, st, 1);

    let lib = ui_state.library.as_ref().unwrap();
    let b = &ui_state.browser;
    let visible = |e: &&Entry| {
        e.matches(&b.query)
            && b.category.as_ref().is_none_or(|c| e.meta.in_category(c))
            && match b.filter {
                Filter::All | Filter::Recent => true,
                Filter::Favorites => lib.prefs.favorites.contains(&e.id),
                Filter::Factory => e.origin == Origin::Factory,
                Filter::User => e.origin == Origin::User,
            }
    };
    // Recent lists in recency order, including sounds that have gone missing.
    let rows: Vec<Result<Entry, String>> = match b.filter {
        Filter::Recent => lib
            .prefs
            .recents
            .iter()
            .filter_map(|id| match lib.get(id) {
                Some(e) => visible(&e).then(|| Ok(e.clone())),
                None => Some(Err(id.clone())),
            })
            .collect(),
        Filter::Favorites => {
            let mut v: Vec<_> = lib
                .entries
                .iter()
                .filter(visible)
                .cloned()
                .map(Ok)
                .collect();
            v.extend(
                lib.prefs
                    .favorites
                    .iter()
                    .filter(|id| lib.get(id).is_none())
                    .map(|id| Err(id.clone())),
            );
            v
        }
        _ => lib
            .entries
            .iter()
            .filter(visible)
            .cloned()
            .map(Ok)
            .collect(),
    };
    let favorites = lib.prefs.favorites.clone();
    let has_user = lib.entries.iter().any(|e| e.origin == Origin::User);
    let filter = b.filter;

    let list_h = (ui.available_height() - 410.0).max(120.0);
    let mut open = None;
    let mut toggle = None;
    let mut forget = None;
    egui::ScrollArea::vertical()
        .max_height(list_h)
        .auto_shrink([false, false])
        .show_owned(ui, |ui| {
            let mut last_origin = None;
            let narrowed =
                !ui_state.browser.query.trim().is_empty() || ui_state.browser.category.is_some();
            if rows.is_empty() && narrowed {
                kit::label(ui, st, Role::Body, Tone::Text3, "No sound matches the search or category.");
            } else if rows.is_empty() {
                kit::paragraph(ui, st, Role::Body, Tone::Text3, match filter {
                    Filter::Favorites => "No favorites yet: click ☆ next to a sound.",
                    Filter::Recent => "Nothing opened yet.",
                    Filter::User => "No sounds of yours yet: Save As puts them here.",
                    _ => "No sound matches.",
                });
            }
            for row in &rows {
                match row {
                    Ok(e) => {
                        if filter != Filter::Recent && last_origin != Some(e.origin) {
                            last_origin = Some(e.origin);
                            kit::gap(ui, st, 1);
                            kit::section(ui, st, match e.origin {
                                Origin::Factory => "Factory",
                                Origin::User => "Your Sounds",
                            });
                        }
                        let fav = favorites.contains(&e.id);
                        let sel = ui_state.browser.selected.as_deref() == Some(&e.id);
                        let (row, star) = sound_row(ui, st, e, fav, sel);
                        let star = star.tip(st, if fav { "Remove from favorites" } else { "Add to favorites" });
                        ui_state.record(format!("fav:{}", e.id), star.rect);
                        ui_state.record(format!("sound:{}", e.id), row.rect);
                        if star.clicked() {
                            toggle = Some(e.id.clone());
                        } else if row.clicked() {
                            ui_state.browser.selected = Some(e.id.clone());
                        }
                        if row.double_clicked() {
                            open = Some(e.id.clone());
                        }
                    }
                    Err(id) => {
                        ui.horizontal(|ui| {
                            kit::label(ui, st, Role::Body, Tone::Text3, format!("{id}: not found")).tip(
                                st,
                                "Removed or renamed outside kabl. Forget drops it from \
                                 favorites and recents.",
                            );
                            let r = ui.add(kit::Button::new(st, "Forget").small());
                            ui_state.record(format!("forget:{id}"), r.rect);
                            if r.clicked() {
                                forget = Some(id.clone());
                            }
                        });
                    }
                }
            }
            if filter == Filter::All && !has_user && ui_state.browser.query.is_empty() {
                kit::section(ui, st, "Your Sounds");
                kit::label(ui, st, Role::Body, Tone::Text3, "None yet: Save As puts your sounds here.");
            }
        });
    if let Some(id) = toggle {
        let note = ui_state.library.as_mut().unwrap().toggle_favorite(&id);
        if let Some(n) = note {
            message(ui_state, n);
        }
    }
    if let Some(id) = forget {
        let note = ui_state.library.as_mut().unwrap().forget(&id);
        if let Some(n) = note {
            message(ui_state, n);
        }
    }
    kit::gap(ui, st, 1);
    kit::rule(ui, st);
    kit::gap(ui, st, 1);

    // The selected sound.
    let selected = ui_state
        .browser
        .selected
        .clone()
        .and_then(|id| ui_state.library.as_ref().unwrap().get(&id).cloned());
    match &selected {
        Some(e) => {
            ui.horizontal(|ui| {
                kit::label_truncated(ui, st, Role::H3, Tone::Text, &e.meta.name);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let r = ui.add(kit::Button::new(st, "Open").primary()).tip(st, "Load into the rack");
                    hit(ui_state, "open", &r);
                    if r.clicked() {
                        open = Some(e.id.clone());
                    }
                    if e.origin == Origin::User {
                        let r = ui.add(kit::Button::new(st, "Rename"));
                        hit(ui_state, "rename", &r);
                        if r.clicked() {
                            ui_state.browser.dialog = Some(Dialog::Rename {
                                id: e.id.clone(),
                                name: e.meta.name.clone(),
                                error: None,
                            });
                        }
                    }
                });
            });
            ui.horizontal(|ui| {
                kit::label(
                    ui,
                    st,
                    Role::Caption,
                    Tone::Text2,
                    format!(
                        "{} · {}",
                        if e.meta.category.is_empty() {
                            "Uncategorized"
                        } else {
                            &e.meta.category
                        },
                        match e.origin {
                            Origin::Factory => "Factory",
                            Origin::User => "Your Sounds",
                        }
                    ),
                );
                badge(ui, st, &e.meta);
            });
            if !e.meta.description.is_empty() {
                kit::label_truncated(ui, st, Role::Caption, Tone::Text2, &e.meta.description).tip(st, &e.meta.description);
            }
            if !e.meta.tags.is_empty() {
                let tags = format!("tags: {}", e.meta.tags.join(", "));
                kit::label_truncated(ui, st, Role::Caption, Tone::Text3, &tags).tip(st, &tags);
            }
        }
        None => {
            kit::paragraph(ui, st, Role::Body, Tone::Text3, "Click a sound to see it; Open (or double-click) loads it.");
        }
    }
    if let Some(id) = open {
        request(editor, ui_state, Pending::Open(id));
    }
    kit::gap(ui, st, 1);
    kit::rule(ui, st);
    kit::gap(ui, st, 1);
    play_section(editor, ui_state, ui);

    kit::gap(ui, st, 1);
    kit::rule(ui, st);
    kit::gap(ui, st, 1);
    let open_f = ui_state.browser.folder_open;
    let header = ui.add(kit::Button::new(st, "Patch folder (advanced)").ghost().icon(if open_f { Ic::Down } else { Ic::Right }));
    if header.clicked() {
        ui_state.browser.folder_open = !open_f;
    }
    hit(ui_state, "folder-header", &header);
    if open_f {
        {
            let r = kit::field(ui, st, &mut ui_state.browser.folder, "", None, None);
            hit(ui_state, "patch-path", &r);
            ui.horizontal(|ui| {
                let r = ui.add(kit::Button::new(st, "Load folder"));
                hit(ui_state, "load", &r);
                if r.clicked() {
                    let p = ui_state.browser.folder.clone();
                    request(editor, ui_state, Pending::OpenFolder(p));
                }
                let r = ui.add(kit::Button::new(st, "Save to folder"));
                hit(ui_state, "save-folder", &r);
                if r.clicked() {
                    let p = ui_state.browser.folder.clone();
                    let path = std::path::Path::new(&p);
                    let text = if ui_state
                        .library
                        .as_ref()
                        .is_some_and(|l| l.is_factory_path(path))
                    {
                        "that folder is a factory sound: choose another folder, or Save As".into()
                    } else if ui_state.browser.async_io {
                        start_io(editor, ui_state, IoRequest::SaveFolder(p.clone()));
                        "saving folder in background".into()
                    } else {
                        match crate::library::save_folder(path, editor.log()) {
                            Ok(()) => {
                                let name = path
                                    .file_name()
                                    .map_or(p.clone(), |n| n.to_string_lossy().to_string());
                                ui_state.doc = Some(Doc::new(
                                    &name,
                                    DocOrigin::Folder(p.clone()),
                                    editor.state(),
                                ));
                                format!("saved to {p}")
                            }
                            Err(err) => save_failed(&err),
                        }
                    };
                    message(ui_state, text);
                }
            });
            let lib = ui_state.library.as_ref().unwrap();
            kit::paragraph(
                ui,
                st,
                Role::Caption,
                Tone::Text3,
                format!(
                    "Your sounds: {}\nFactory: {}",
                    lib.sounds_dir().display(),
                    lib.factory_dir
                        .as_ref()
                        .map_or("not found".into(), |d| d.display().to_string())
                ),
            );
        }
    }
    let notes = ui_state.library.as_ref().unwrap().notes.clone();
    for n in notes {
        kit::paragraph(ui, st, Role::Caption, Tone::Warn, n);
    }
}

/// Audition for keyboard sounds, Start/Stop for pieces: always the sound in the rack.
fn play_section(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui) {
    let st = ui_state.style.clone();
    let st = &*st;
    let name = ui_state
        .doc
        .as_ref()
        .map_or(String::new(), |d| d.name.clone());
    kit::section(ui, st, "Audition");
    kit::label_truncated(ui, st, Role::Body, Tone::Text, format!("In the rack: {name}"));
    let (keys, sequence) = Meta::play_of(editor.state());
    if keys {
        let mut chord_hits = Vec::new();
        let a = &mut ui_state.browser.audition;
        ui.horizontal(|ui| {
            let r = ui.add(kit::Button::new(st, "−8").small()).tip(st, "An octave down");
            chord_hits.push(("note-down".to_string(), r.rect));
            if r.clicked() {
                a.note = a.note.saturating_sub(12).max(24);
            }
            let r = kit::dropdown(ui, st, "audition-note", &note_name(a.note), 64.0, |ui| {
                for n in 36..=84 {
                    if kit::menu_item(ui, st, &note_name(n), a.note == n).clicked() {
                        a.note = n;
                    }
                }
            });
            chord_hits.push(("note".to_string(), r.rect));
            let r = ui.add(kit::Button::new(st, "+8").small()).tip(st, "An octave up");
            chord_hits.push(("note-up".to_string(), r.rect));
            if r.clicked() {
                a.note = (a.note + 12).min(96);
            }
            for c in [Chord::Single, Chord::Major, Chord::Minor] {
                let r = ui.add(kit::Button::new(st, c.label()).small().selected(a.chord == c));
                chord_hits.push((format!("chord:{}", c.label()), r.rect));
                if r.clicked() {
                    a.chord = c;
                }
            }
        });
        ui.horizontal(|ui| {
            kit::label(ui, st, Role::Label, Tone::Text2, "Velocity");
            let mut v = f64::from(a.velocity);
            let txt = format!("{}", v as u8);
            let r = kit::slider(ui, st, &mut v, 1.0..=127.0, Some(1.0), &txt);
            a.velocity = v as u8;
            chord_hits.push(("velocity".to_string(), r.rect));
        });
        ui.horizontal(|ui| {
            kit::label(ui, st, Role::Label, Tone::Text2, "Length");
            let mut v = f64::from(a.secs);
            let txt = format!("{v:.2} s");
            let r = kit::slider(ui, st, &mut v, 0.25..=4.0, Some(0.25), &txt);
            a.secs = v as f32;
            chord_hits.push(("length".to_string(), r.rect));
        });
        let label = format!(
            "Play {}{}",
            note_name(a.note),
            match a.chord {
                Chord::Single => "",
                Chord::Major => " major",
                Chord::Minor => " minor",
            }
        );
        let help = format!(
            "Plays {} at velocity {} for {:.2} s into this sound's keyboard, like keys \
             held that long. One preview can't show a sound's whole range: try other notes.",
            a.notes()
                .iter()
                .map(|n| note_name(*n))
                .collect::<Vec<_>>()
                .join(" "),
            a.velocity,
            a.secs
        );
        let secs = a.secs as f64;
        let cmd = a.command(ui_state.sample_rate);
        for (k, r) in chord_hits {
            ui_state.record(k, r);
        }
        ui.horizontal(|ui| {
            let r = ui.add(kit::Button::new(st, label).icon(Ic::Play).primary()).tip(st, &help);
            hit(ui_state, "play", &r);
            if r.clicked() {
                ui_state.launches.push(cmd);
                let now = ui.input(|i| i.time);
                ui_state.browser.preview_until = Some(now + secs);
            }
            let r = ui.add(kit::Button::new(st, "Stop").icon(Ic::Stop).enabled(ui_state.browser.preview_until.is_some()));
            hit(ui_state, "stop-preview", &r);
            if r.clicked() {
                ui_state.launches.push(Command::PreviewStop);
                ui_state.browser.preview_until = None;
            }
            if ui_state.browser.preview_until.is_some() {
                kit::label(ui, st, Role::Caption, Tone::Accent, "sounding");
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(100));
            }
        });
        kit::paragraph(ui, st, Role::Caption, Tone::Text3, help);
    }
    if sequence {
        let clocks: Vec<_> = editor
            .state()
            .modules
            .iter()
            .filter(|(_, m)| m.kind == "clock")
            .map(|(&id, _)| id)
            .collect();
        let running = clocks
            .iter()
            .any(|id| ui_state.clock_running.get(id).copied().unwrap_or(true));
        ui.horizontal(|ui| {
            let r = ui.add(kit::Button::new(st, "Start").icon(Ic::Play).primary().enabled(!running));
            hit(ui_state, "start", &r);
            if r.clicked() {
                ui_state
                    .transport
                    .extend(clocks.iter().map(|&id| (id, Transport::Run)));
            }
            let r = ui.add(kit::Button::new(st, "Stop").icon(Ic::Stop).enabled(running));
            hit(ui_state, "stop", &r);
            if r.clicked() {
                ui_state
                    .transport
                    .extend(clocks.iter().map(|&id| (id, Transport::Stop)));
            }
            kit::label(
                ui,
                st,
                Role::Caption,
                if running { Tone::Accent } else { Tone::Text2 },
                if running { "sequence running" } else { "sequence stopped" },
            );
        });
        kit::paragraph(
            ui,
            st,
            Role::Caption,
            Tone::Text3,
            "Start and Stop run every clock in the sound (the clock's own buttons do the same).",
        );
    }
    if !keys && !sequence {
        kit::paragraph(ui, st, Role::Body, Tone::Text3, "This sound has no keyboard input and no clock: nothing to audition.");
    }
}

/// The open dialog, if any (modal). Escape cancels it.
pub fn dialogs(editor: &mut PatchEditor, ui_state: &mut UiState, ctx: &egui::Context) {
    if io_busy(ui_state) {
        return;
    }
    let Some(dialog) = ui_state.browser.dialog.take() else {
        return;
    };
    let st = ui_state.style.clone();
    let st = &*st;
    // The dialog for the next frame, unless an action opened another (Save As from Save).
    let next = match dialog {
        Dialog::Unsaved { then, error } => {
            let name = ui_state
                .doc
                .as_ref()
                .map_or(String::new(), |d| d.name.clone());
            let what = match &then {
                Pending::Quit => "quitting",
                Pending::New => "starting a new sound",
                _ => "opening another sound",
            };
            let mut next = Some(Dialog::Unsaved {
                then: then.clone(),
                error: error.clone(),
            });
            kit::modal(ctx, st, "dlg-unsaved", 380.0, |ui| {
                kit::dialog_title(ui, st, "Unsaved changes");
                kit::paragraph(
                    ui,
                    st,
                    Role::Body,
                    Tone::Text,
                    format!("\"{name}\" has changes you haven't saved. Save them before {what}?"),
                );
                if let Some(e) = &error {
                    kit::paragraph(ui, st, Role::Body, Tone::Bad, e);
                }
                kit::gap(ui, st, 2);
                ui.horizontal(|ui| {
                    let r = ui.add(kit::Button::new(st, "Save").primary());
                    hit(ui_state, "dlg:save", &r);
                    if r.clicked() {
                        next =
                            save(editor, ui_state, Some(then.clone())).map(|e| Dialog::Unsaved {
                                then: then.clone(),
                                error: Some(e),
                            });
                    }
                    let r = ui.add(kit::Button::new(st, "Don't save"));
                    hit(ui_state, "dlg:discard", &r);
                    if r.clicked() {
                        next = None;
                        perform(editor, ui_state, then.clone());
                    }
                    let r = ui.add(kit::Button::new(st, "Cancel").ghost());
                    hit(ui_state, "dlg:cancel", &r);
                    if r.clicked() {
                        next = None;
                    }
                });
            });
            next
        }
        Dialog::SaveAs {
            mut name,
            mut category,
            mut tags,
            mut taken,
            then,
            mut error,
        } => {
            let mut submit: Option<Option<String>> = None;
            let mut cancel = false;
            kit::modal(ctx, st, "dlg-save-as", 380.0, |ui| {
                kit::dialog_title(ui, st, "Save As");
                kit::paragraph(ui, st, Role::Body, Tone::Text2, "Saves a copy in Your Sounds. Factory sounds stay as they are.");
                kit::gap(ui, st, 2);
                ui.horizontal(|ui| {
                    kit::label(ui, st, Role::Label, Tone::Text2, "Name");
                    let r = kit::field(ui, st, &mut name, "", None, Some(280.0));
                    hit(ui_state, "dlg:name", &r);
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        submit = Some(None);
                    }
                });
                // A confirmation belongs to the name it was offered for: editing the name drops
                // it (and its message) before the buttons are drawn, so Replace it can never
                // send an old target with a new name.
                if taken
                    .as_ref()
                    .is_some_and(|(_, n)| n.to_lowercase() != name.trim().to_lowercase())
                {
                    taken = None;
                    error = None;
                }
                ui.horizontal(|ui| {
                    kit::label(ui, st, Role::Label, Tone::Text2, "Category");
                    let cur = category.clone();
                    kit::dropdown(ui, st, "dlg-category", &cur, 200.0, |ui| {
                        for c in CATEGORIES {
                            if kit::menu_item(ui, st, c, category == *c).clicked() {
                                category = c.to_string();
                            }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    kit::label(ui, st, Role::Label, Tone::Text2, "Tags");
                    kit::field(ui, st, &mut tags, "comma separated", None, Some(280.0));
                });
                if let Some(e) = &error {
                    kit::paragraph(ui, st, Role::Body, Tone::Bad, e);
                }
                kit::gap(ui, st, 2);
                ui.horizontal(|ui| {
                    let r = ui.add(kit::Button::new(st, "Save").primary());
                    hit(ui_state, "dlg:save", &r);
                    if r.clicked() {
                        submit = Some(None);
                    }
                    if let Some((id, _)) = &taken {
                        let r = ui.add(kit::Button::new(st, "Replace it"));
                        hit(ui_state, "dlg:replace", &r);
                        if r.clicked() {
                            submit = Some(Some(id.clone()));
                        }
                    }
                    let r = ui.add(kit::Button::new(st, "Cancel").ghost());
                    hit(ui_state, "dlg:cancel", &r);
                    cancel = r.clicked();
                });
            });
            match submit {
                _ if cancel => None,
                Some(replace) => save_as(
                    editor,
                    ui_state,
                    &name,
                    &category,
                    &tags,
                    replace,
                    then.clone(),
                )
                .err()
                .map(|(taken, e)| Dialog::SaveAs {
                    name: name.clone(),
                    category: category.clone(),
                    tags: tags.clone(),
                    taken,
                    then: then.clone(),
                    error: Some(e),
                }),
                None => Some(Dialog::SaveAs {
                    name,
                    category,
                    tags,
                    taken,
                    then,
                    error,
                }),
            }
        }
        Dialog::Rename {
            id,
            mut name,
            error,
        } => {
            let mut submit = false;
            let mut cancel = false;
            kit::modal(ctx, st, "dlg-rename", 340.0, |ui| {
                kit::dialog_title(ui, st, "Rename");
                let r = kit::field(ui, st, &mut name, "", None, None);
                hit(ui_state, "dlg:name", &r);
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                }
                if let Some(e) = &error {
                    kit::paragraph(ui, st, Role::Body, Tone::Bad, e);
                }
                kit::gap(ui, st, 2);
                ui.horizontal(|ui| {
                    let r = ui.add(kit::Button::new(st, "Rename").primary());
                    hit(ui_state, "dlg:save", &r);
                    submit |= r.clicked();
                    let r = ui.add(kit::Button::new(st, "Cancel").ghost());
                    hit(ui_state, "dlg:cancel", &r);
                    cancel = r.clicked();
                });
            });
            if cancel {
                None
            } else if submit {
                rename(ui_state, &id, &name).err().map(|e| Dialog::Rename {
                    id,
                    name,
                    error: Some(e),
                })
            } else {
                Some(Dialog::Rename { id, name, error })
            }
        }
        Dialog::Restore => {
            let mut next = Some(Dialog::Restore);
            let redo = editor.can_redo();
            let at = ui_state
                .compare
                .reference
                .as_ref()
                .map_or(String::new(), |r| r.at.clone());
            kit::modal(ctx, st, "dlg-restore", 380.0, |ui| {
                kit::dialog_title(ui, st, "Restore reference");
                kit::paragraph(
                    ui,
                    st,
                    Role::Body,
                    Tone::Text,
                    format!(
                        "Replace the whole current patch (values, cables, routes, labels, Perform \
                         pins, MIDI mappings) with the reference captured {at}?"
                    ),
                );
                kit::paragraph(
                    ui,
                    st,
                    Role::Body,
                    Tone::Text2,
                    "It is one undo step: Undo brings your current version back with all \
                     its edits.",
                );
                if redo {
                    kit::paragraph(
                        ui,
                        st,
                        Role::Body,
                        Tone::Bad,
                        "Like any edit, it replaces the steps you could Redo right now.",
                    );
                }
                kit::gap(ui, st, 2);
                ui.horizontal(|ui| {
                    let r = ui.add(kit::Button::new(st, "Restore").primary());
                    hit(ui_state, "dlg:restore", &r);
                    if r.clicked() {
                        let m = crate::compare::restore(editor, ui_state);
                        message(ui_state, m);
                        next = None;
                    }
                    let r = ui.add(kit::Button::new(st, "Cancel").ghost());
                    hit(ui_state, "dlg:cancel", &r);
                    if r.clicked() {
                        next = None;
                    }
                });
            });
            next
        }
    };
    let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    if ui_state.browser.dialog.is_none() && !escape {
        ui_state.browser.dialog = next;
    }
}

fn rename(ui_state: &mut UiState, id: &str, name: &str) -> Result<(), String> {
    let lib = ui_state
        .library
        .as_mut()
        .ok_or("the sound library isn't available")?;
    let meta = lib.get(id).map(|e| e.meta.clone()).unwrap_or_default();
    let name = name.trim().to_string();
    match lib.set_meta(
        id,
        Meta {
            name: name.clone(),
            ..meta
        },
    ) {
        Ok(()) => {
            if let Some(d) = ui_state
                .doc
                .as_mut()
                .filter(|d| d.origin == DocOrigin::Library(id.to_string()))
            {
                d.name = name.clone();
            }
            message(ui_state, format!("renamed to \"{name}\""));
            Ok(())
        }
        Err(LibError::NameTaken(_)) => Err(format!("\"{name}\" is already in Your Sounds.")),
        Err(e) => Err(format!("Not renamed: {e}")),
    }
}

/// Library for `Library::open` with the default locations.
pub fn open_library() -> Library {
    Library::open(
        crate::library::find_factory_dir(),
        crate::library::default_user_root(),
    )
}
