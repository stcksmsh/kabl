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

use crate::filebrowser::{self, Mode, Purpose};
use crate::kit::g4::{self, Sev, Size};
use crate::kit::{self, Ic, Tip, Tone};
use crate::style::{Role, Style};
use crate::wheel::OwnedScroll;
use egui::{pos2, vec2, Rect, Sense};
use kabl_core::{PatchLog, PatchState};
use kabl_engine::patch_engine::{Command, MAX_PREVIEW_NOTES, MAX_PREVIEW_SECS};
use kabl_modules::builtins::Transport;
use std::collections::HashMap;

use crate::library::{Entry, LibError, Library, Meta, Origin, CATEGORIES, SHELVES};
use crate::{PatchEditor, UiState};

/// The Sounds scene: library column, selected-sound column.
const NAV_W: f32 = 230.0;
const DETAIL_W: f32 = 320.0;
/// The list column stops growing here; the detail column takes the rest.
const LIST_MAX_W: f32 = 760.0;
/// Sounds whose macros are read from disk per frame while the list fills in.
const MACROS_PER_FRAME: usize = 8;
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
    /// "This patch contains custom code modules": Open runs `then`, which reads the patch again
    /// now that the answer is remembered; Cancel drops it.
    CodeNotice {
        entries: Vec<crate::code_notice::CodeEntry>,
        then: Pending,
    },
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
    /// Macro names and positions per sound id, read from the sound's files on first sight.
    macros: HashMap<String, Vec<(String, f32)>>,
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
    /// The code scope of the song the open produced (see `UiState::open_scope`).
    open_scope: Option<kabl_modules::code::CodeScope>,
    load_stopped: bool,
    selected: Option<String>,
    message: Option<String>,
    error: Option<(Option<(String, String)>, String)>,
    /// A dialog the worker wants shown (the custom-code notice).
    dialog: Option<Dialog>,
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
            filter: Filter::Factory,
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
            macros: HashMap::new(),
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
                    match crate::library::save_patch(
                        std::path::Path::new(&path),
                        scratch_editor.log(),
                    ) {
                        Ok(target) => {
                            let t = target.to_string_lossy().into_owned();
                            scratch.doc = Some(Doc::new(
                                &patch_name(&target),
                                DocOrigin::Folder(t.clone()),
                                scratch_editor.state(),
                            ));
                            message(&mut scratch, format!("saved to {t}"));
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
                open_scope: scratch.open_scope.take(),
                load_stopped: scratch.load_stopped,
                selected: scratch.browser.selected.take(),
                message: scratch.last_message,
                error,
                dialog: scratch.browser.dialog.take(),
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
                    ui.open_scope = done.open_scope;
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
                // A failed open can still change the library (a declined code answer cleared).
                ui.library = done.library;
                if let Some(d @ Dialog::CodeNotice { .. }) = done.dialog {
                    ui.browser.dialog = Some(d);
                }
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
                if let (Some(DocOrigin::Folder(p)), Some(l)) =
                    (done.doc.as_ref().map(|d| &d.origin), ui.library.as_mut())
                {
                    let _ = l.touch_patch(std::path::Path::new(p));
                }
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
    ui.clock_pos.clear();
    ui.lfo_status.clear();
    ui.edit_bank.clear();
    ui.edit_page.clear();
    ui.page_pinned.clear();
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
            match lib.read_for_open(&id) {
                Ok((log, scope)) => {
                    ui.open_scope = Some(scope);
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
                    stand_in_warning(editor, ui);
                }
                Err(LibError::CodeNotice(entries)) => {
                    ui.browser.dialog = Some(Dialog::CodeNotice {
                        entries,
                        then: Pending::Open(id),
                    });
                }
                Err(LibError::CodeDeclined(entries)) => {
                    let note = lib.forget_code_trust(&entries);
                    message(
                        ui,
                        note.unwrap_or_else(|| LibError::CodeDeclined(entries).to_string()),
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
            let trust = ui
                .library
                .as_ref()
                .map(|l| l.prefs.code_trust.clone())
                .unwrap_or_default();
            match crate::library::read_patch_scoped(std::path::Path::new(&path), &trust) {
                Ok((log, declined, scope)) => {
                    ui.open_scope = Some(scope);
                    if !declined.is_empty() {
                        // Opened with stand-ins; the next open asks again.
                        if let Some(l) = ui.library.as_mut() {
                            l.forget_code_trust(&declined);
                        }
                    }
                    replace_patch(editor, ui, log);
                    log::info!(target: "doc", "open ok origin=folder editor={}", editor.instance());
                    let name = patch_name(std::path::Path::new(&path));
                    ui.doc = Some(Doc::new(
                        &name,
                        DocOrigin::Folder(path.clone()),
                        editor.state(),
                    ));
                    message(ui, format!("loaded {path}"));
                    stand_in_warning(editor, ui);
                }
                Err(LibError::CodeNotice(entries)) => {
                    ui.browser.dialog = Some(Dialog::CodeNotice {
                        entries,
                        then: Pending::OpenFolder(path),
                    });
                }
                Err(LibError::CodeDeclined(entries)) => {
                    let note = ui
                        .library
                        .as_mut()
                        .and_then(|l| l.forget_code_trust(&entries));
                    message(
                        ui,
                        note.unwrap_or_else(|| LibError::CodeDeclined(entries).to_string()),
                    );
                }
                Err(err) => message(ui, {
                    log::warn!(target: "doc", "open failed origin=folder error={}; working patch kept", err.log_text());
                    format!("load failed: {err}. Your sound is unchanged.")
                }),
            }
        }
        Pending::New => {
            ui.open_scope = Some(kabl_modules::code::CodeScope::new());
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
    // Where a patch file went when the save wrote a `.kabl` (a legacy folder saves beside it).
    let mut moved: Option<String> = None;
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
            crate::library::save_patch(std::path::Path::new(path), editor.log()).map(|t| {
                moved = Some(t.to_string_lossy().into_owned());
            })
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
                if let Some(p) = &moved {
                    d.origin = DocOrigin::Folder(p.clone());
                    d.name = patch_name(std::path::Path::new(p));
                }
            }
            if let (Some(p), Some(l)) = (&moved, ui.library.as_mut()) {
                let _ = l.touch_patch(std::path::Path::new(p));
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
/// Saves the patch to the folder `p` (not a factory sound), and says what happened.
fn save_to_folder(editor: &mut PatchEditor, ui_state: &mut UiState, p: String) {
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
        match crate::library::save_patch(path, editor.log()) {
            Ok(target) => {
                let t = target.to_string_lossy().into_owned();
                ui_state.doc = Some(Doc::new(
                    &patch_name(&target),
                    DocOrigin::Folder(t.clone()),
                    editor.state(),
                ));
                if let Some(l) = ui_state.library.as_mut() {
                    let _ = l.touch_patch(&target);
                }
                format!("saved to {t}")
            }
            Err(err) => save_failed(&err),
        }
    };
    message(ui_state, text);
}

/// The name a patch file or folder goes by: a `.kabl` file without the extension.
fn patch_name(p: &std::path::Path) -> String {
    let n = if crate::library::is_kabl_path(p) && p.is_file() {
        p.file_stem()
    } else {
        p.file_name()
    };
    n.map_or_else(
        || p.to_string_lossy().into_owned(),
        |n| n.to_string_lossy().into_owned(),
    )
}

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

/// The open sound as a dropdown: category kicker, name, unsaved dot, chevron. Its menu holds
/// Save and Save As.
pub fn doc_field(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui, st: &Style) {
    let modified = is_modified(editor, ui_state);
    let name = ui_state
        .doc
        .as_ref()
        .map_or("Untitled", |d| d.name.as_str())
        .to_string();
    let cat = ui_state
        .doc
        .as_ref()
        .map_or("", |d| d.meta.category.as_str())
        .to_uppercase();
    let factory = matches!(
        ui_state.doc.as_ref().map(|d| &d.origin),
        Some(DocOrigin::Library(id)) if id.starts_with("factory:")
    );
    let tip = if modified {
        format!("{name}: unsaved changes")
    } else if factory {
        format!("{name}: factory sound (Save makes your own copy)")
    } else {
        name.clone()
    };
    // Mockup: kicker mono 10 px, name 13 px medium.
    let kick = ui.painter().layout_no_wrap(
        cat,
        egui::FontId::new(10.0, egui::FontFamily::Monospace),
        st.roles.text3,
    );
    let nm_font = egui::FontId::new(13.0, crate::style::Font::SansMedium.family(st.fonts_ready));
    let nm = ui
        .painter()
        .layout_no_wrap(name, nm_font.clone(), st.roles.text);
    let pad = st.sp(2);
    let gap = st.sp(2);
    let dot = if modified { 3.0 * 2.0 + gap } else { 0.0 };
    let kick_w = if kick.size().x > 0.0 {
        kick.size().x + gap
    } else {
        0.0
    };
    let w = if ui.max_rect().width() >= 330.0 {
        210.0
    } else {
        170.0
    };
    let (rect, r) = ui.allocate_exact_size(vec2(w, st.metrics.control_h), Sense::click());
    let radius = st.shape_radius(st.slots.controls.field, rect.height());
    let hover = ui.ctx().animate_bool_with_time(
        r.id.with("hover"),
        r.hovered(),
        Style::secs(st.motion.hover),
    );
    let p = ui.painter();
    p.rect_filled(rect, radius, st.roles.surface);
    p.rect_stroke(
        rect,
        radius,
        egui::Stroke::new(1.0, crate::style::mix(st.roles.line, st.roles.text3, hover)),
        egui::StrokeKind::Inside,
    );
    let cy = rect.center().y;
    let mut x = rect.left() + pad;
    if kick_w > 0.0 {
        p.galley(
            egui::pos2(x, cy - kick.size().y / 2.0),
            kick.clone(),
            st.roles.text3,
        );
        x += kick_w;
    }
    let max_name = rect.right() - 12.0 - gap - dot - st.sp(2) - x;
    let shown = if nm.size().x > max_name {
        let full = nm.job.text.clone();
        kit::fit_text(ui, st, &full, max_name)
    } else {
        nm.job.text.clone()
    };
    let nm = ui.painter().layout_no_wrap(shown, nm_font, st.roles.text);
    let nx = nm.size().x;
    ui.painter()
        .galley(egui::pos2(x, cy - nm.size().y / 2.0), nm, st.roles.text);
    x += nx + gap;
    if modified {
        ui.painter()
            .circle_filled(egui::pos2(x + 3.0, cy), 3.0, st.roles.accent);
    }
    kit::icon(
        ui.painter(),
        Ic::Down,
        egui::pos2(rect.right() - st.sp(2) - 5.0, cy),
        st.metrics.icon,
        st.roles.text2,
    );
    let r = r.tip(st, &tip);
    hit(ui_state, "doc-name", &r);
    ui_state.record("sound-menu".into(), r.rect);
    egui::Popup::menu(&r)
        .frame(kit::popover_frame(st))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
        .width(180.0)
        .show(|ui| {
            let busy = io_busy(ui_state);
            let s = kit::menu_item(ui, st, "Save", false);
            hit(ui_state, "save-menu", &s);
            if s.clicked() && !busy {
                save(editor, ui_state, None);
            }
            let a = kit::menu_item(ui, st, "Save As...", false);
            hit(ui_state, "save-as", &a);
            if a.clicked() {
                open_save_as(ui_state, None);
            }
            let o = kit::menu_item(ui, st, "Browse sounds", ui_state.browser_open);
            hit(ui_state, "browse-sounds", &o);
            if o.clicked() {
                ui_state.browser_open = true;
            }
        });
}

/// The primary Save button, at the right end of the toolbar: 13 px semibold, 28 px tall,
/// text width + 28 px (mockup metrics).
pub fn save_button(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
) {
    let enabled = !io_busy(ui_state);
    let font = egui::FontId::new(
        13.0,
        crate::style::Font::SansSemibold.family(st.fonts_ready),
    );
    let g = ui
        .painter()
        .layout_no_wrap("Save".into(), font, st.roles.on_accent);
    let (rect, r) = ui.allocate_exact_size(
        vec2(g.size().x + 28.0, st.metrics.control_h),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let hover = ui.ctx().animate_bool_with_time(
        r.id.with("hover"),
        enabled && r.hovered(),
        Style::secs(st.motion.hover),
    );
    let fill = crate::style::mix(st.roles.accent, egui::Color32::BLACK, 0.10 * hover);
    let radius = st.shape_radius(st.slots.controls.button, rect.height());
    ui.painter().rect_filled(rect, radius, fill);
    ui.painter()
        .galley(rect.center() - g.size() / 2.0, g, st.roles.on_accent);
    let r = r.tip(st, "Ctrl+S");
    hit(ui_state, "save", &r);
    if r.clicked() && enabled {
        save(editor, ui_state, None);
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

/// The named macros of the sound file `dir`: (name, position 0..1). Empty when it has none.
fn macros_of(dir: &std::path::Path) -> Vec<(String, f32)> {
    let Ok(log) = kabl_core::format::load_kabl(dir) else {
        return Vec::new();
    };
    let state = log.state();
    let Some((&id, m)) = state.modules.iter().find(|(_, m)| m.kind == "macro") else {
        return Vec::new();
    };
    (1..=8)
        .filter_map(|k| {
            let key = format!("m{k}");
            let name = state.label(id, &format!("name.{key}"))?;
            Some((name.to_string(), m.params.get(&key).copied().unwrap_or(0.0)))
        })
        .collect()
}

/// The macros of `e`, loaded now when `budget` allows (a few sounds per frame, so a long list
/// never stalls one), else what is known so far.
fn macros_cached<'a>(b: &'a mut Browser, e: &Entry, budget: &mut usize) -> &'a [(String, f32)] {
    if !b.macros.contains_key(&e.id) && *budget > 0 {
        *budget -= 1;
        b.macros.insert(e.id.clone(), macros_of(&e.dir));
    }
    b.macros.get(&e.id).map_or(&[], |v| v.as_slice())
}

/// Column x positions of a list row: (name, macros, tags).
fn sound_cols(rect: Rect, st: &Style) -> [f32; 3] {
    let name = rect.left() + st.sp(3) + 4.0 + st.metrics.control_h / 2.0 + st.sp(1);
    let w = rect.right() - name;
    [name, name + w * 0.34, name + w * 0.72]
}

/// A read-only macro knob: skirt, accent value arc, pointer, name below.
fn macro_knob(ui: &mut egui::Ui, st: &Style, name: &str, v: f32, w: f32, compact: bool) {
    // The mockup's detail knobs: a disc of r 22 (18 compact) inside a 4 px accent arc.
    let r = if compact { 18.0 } else { 22.0 };
    let ring = r + 5.0;
    let h = 2.0 * ring + st.sp(4) + st.sp(1);
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let c = pos2(rect.center().x, rect.top() + ring + 2.0);
    let v = v.clamp(0.0, 1.0);
    let at = |t: f32, rad: f32| {
        let a = (135.0 + 270.0 * t).to_radians();
        c + vec2(a.cos(), a.sin()) * rad
    };
    let arc =
        |t: f32| -> Vec<egui::Pos2> { (0..=32).map(|i| at(t * i as f32 / 32.0, ring)).collect() };
    let p = ui.painter();
    let track = egui::Stroke::new(4.0, st.roles.inset);
    p.add(egui::Shape::line(arc(1.0), track));
    if v > 0.0 {
        p.add(egui::Shape::line(
            arc(v),
            egui::Stroke::new(4.0, st.roles.accent),
        ));
    }
    p.circle_filled(c, r, st.roles.raised);
    p.circle_stroke(c, r, egui::Stroke::new(1.0, st.roles.line2));
    p.line_segment(
        [at(v, r * 0.3), at(v, r * 0.8)],
        egui::Stroke::new(2.2, st.roles.text),
    );
    p.text(
        pos2(rect.center().x, rect.bottom() - 1.0),
        egui::Align2::CENTER_BOTTOM,
        name,
        st.font(Role::Label),
        st.roles.text,
    );
}

/// One sound as a row: star, name, macro names, tags.
fn sound_row(
    ui: &mut egui::Ui,
    st: &Style,
    e: &Entry,
    macros: &str,
    fav: bool,
    sel: bool,
) -> (egui::Response, egui::Response) {
    let w = ui.available_width();
    let (rect, row) =
        ui.allocate_exact_size(vec2(w, st.metrics.control_h + st.sp(2)), Sense::click());
    let hover = ui.ctx().animate_bool_with_time(
        row.id.with("hover"),
        row.hovered(),
        Style::secs(st.motion.hover),
    );
    // A selected row is the accent outline below; the others get the hover fill.
    kit::row_bg(ui, st, rect, false, if sel { 0.0 } else { hover });
    if sel {
        // The mockup's selected row: an accent outline.
        ui.painter().rect_stroke(
            rect,
            6.0,
            egui::Stroke::new(1.5, st.roles.accent),
            egui::StrokeKind::Inside,
        );
    }
    let star_rect = egui::Rect::from_center_size(
        rect.left_center() + vec2(st.sp(3) + 4.0, 0.0),
        vec2(st.metrics.control_h, st.metrics.control_h),
    );
    let star = ui.interact(star_rect, row.id.with("star"), Sense::click());
    let ink = if fav {
        st.roles.warn
    } else if star.hovered() {
        st.roles.text
    } else {
        st.roles.text3
    };
    kit::icon(
        ui.painter(),
        if fav { Ic::StarFill } else { Ic::Star },
        star_rect.center(),
        st.metrics.icon + 2.0,
        ink,
    );
    let [x, x_macros, x_tags] = sound_cols(rect, st);
    let mid = rect.center().y;
    let name = kit::fit_text(ui, st, &e.meta.name, x_macros - x - st.sp(2));
    let p = ui.painter();
    p.text(
        pos2(x, mid),
        egui::Align2::LEFT_CENTER,
        name,
        st.font(Role::Body),
        st.roles.text,
    );
    let macros = kit::fit_text(ui, st, macros, x_tags - x_macros - st.sp(2));
    let tags = e
        .meta
        .tags
        .iter()
        .take(2)
        .cloned()
        .collect::<Vec<_>>()
        .join(" · ");
    let tags = kit::fit_text(ui, st, &tags, rect.right() - x_tags - st.sp(2));
    let p = ui.painter();
    p.text(
        pos2(x_macros, mid),
        egui::Align2::LEFT_CENTER,
        macros,
        st.font(Role::Body),
        st.roles.text2,
    );
    p.text(
        pos2(x_tags, mid),
        egui::Align2::LEFT_CENTER,
        tags,
        st.font(Role::Caption),
        st.roles.text3,
    );
    (row, star)
}

/// An accent pill with the active category's name; clicking it clears the category.
fn category_chip(ui: &mut egui::Ui, st: &Style, label: &str) -> egui::Response {
    let font = st.font(Role::Caption);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font, st.roles.on_accent);
    let size = vec2(galley.size().x + 24.0, 24.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, 12.0, st.roles.accent);
    p.galley(
        pos2(rect.left() + 12.0, rect.center().y - galley.size().y / 2.0),
        galley,
        st.roles.on_accent,
    );
    resp.on_hover_text("Click to show every category")
}

/// The left panel.
pub fn panel(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui) {
    let st = ui_state.style.clone();
    let st = &*st;
    if io_busy(ui_state) {
        kit::paragraph(
            ui,
            st,
            Role::Body,
            Tone::Text2,
            "Reading or saving a sound… editing remains available.",
        );
        return;
    }
    if ui_state.library.is_none() {
        kit::label(
            ui,
            st,
            Role::Body,
            Tone::Text2,
            "The sound library isn't available.",
        );
        return;
    }
    // Three columns: the library (filters, categories), the list, the selected sound.
    let area = ui.available_rect_before_wrap();
    let gap = st.sp(3);
    // The library column runs to the panel edge, as in the mockup (230 px wide).
    let nav = Rect::from_min_size(
        pos2(area.left() - st.sp(3), area.top() + 7.0),
        vec2(NAV_W, area.height() - 7.0),
    );
    let mid_w = (area.width() - NAV_W - DETAIL_W - 2.0 * gap).clamp(300.0, LIST_MAX_W);
    let side_w = (area.width() - NAV_W - mid_w - 2.0 * gap).max(0.0);
    // A little in from the edge: the right-aligned buttons stay inside the scroll body.
    let side = Rect::from_min_max(
        pos2(area.right() - side_w, area.top()),
        area.max - vec2(st.sp(1), 0.0),
    );
    let mid = Rect::from_min_size(
        pos2(nav.right() + gap - st.sp(3), area.top()),
        vec2(mid_w, area.height()),
    );
    let col = |r: Rect| egui::UiBuilder::new().max_rect(r);
    let mut open = None;
    ui.scope_builder(col(nav), |ui| nav_column(editor, ui_state, ui, st));
    ui.scope_builder(col(mid), |ui| list_column(ui_state, ui, st, &mut open));
    ui.scope_builder(col(side), |ui| {
        detail_column(editor, ui_state, ui, st, &mut open)
    });
    if let Some(id) = open {
        request(editor, ui_state, Pending::Open(id));
    }
}

/// Row geometry of the Sounds scene's left column, from the mockup: library rows are 28 px
/// tall on a 32 px pitch, category rows 30 px.
const LIB_ROW: f32 = 28.0;
const LIB_PITCH: f32 = 32.0;
const CAT_PITCH: f32 = 30.0;

/// A section kicker, 16 px in from the column edge.
fn kicker(ui: &mut egui::Ui, st: &Style, text: &str) -> egui::Response {
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        kit::section(ui, st, text)
    })
    .inner
}

/// A library row: a filled card when selected, the count right-aligned and muted.
fn nav_row(ui: &mut egui::Ui, st: &Style, label: &str, count: usize, on: bool) -> egui::Response {
    let (cell, r) = ui.allocate_exact_size(vec2(ui.available_width(), LIB_PITCH), Sense::click());
    let rect = egui::Rect::from_min_size(
        cell.min + vec2(8.0, 0.0),
        vec2(cell.width() - 16.0, LIB_ROW),
    );
    let hover = ui.ctx().animate_bool_with_time(
        r.id.with("hover"),
        r.hovered(),
        Style::secs(st.motion.hover),
    );
    if on {
        let radius = st.shape_radius(st.slots.controls.button, rect.height());
        ui.painter().rect_filled(rect, radius, st.roles.raised);
        ui.painter().rect_stroke(
            rect,
            radius,
            egui::Stroke::new(1.0, st.roles.line),
            egui::StrokeKind::Inside,
        );
    } else {
        kit::row_bg(ui, st, rect, false, hover);
    }
    nav_text(
        ui,
        st,
        rect,
        cell.left() + 18.0,
        cell.right() - 18.0,
        label,
        count,
        on,
    );
    r
}

/// A category row: a radio dot (accent when selected), the name, a muted count.
fn cat_row(ui: &mut egui::Ui, st: &Style, label: &str, count: usize, on: bool) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), CAT_PITCH), Sense::click());
    let c = egui::pos2(rect.left() + 22.0, rect.center().y);
    let (fill, line) = if on {
        (st.roles.accent, st.roles.accent)
    } else {
        (egui::Color32::TRANSPARENT, st.roles.line2)
    };
    ui.painter()
        .circle(c, 4.0, fill, egui::Stroke::new(1.3, line));
    nav_text(
        ui,
        st,
        rect,
        rect.left() + 36.0,
        rect.right() - 18.0,
        label,
        count,
        on,
    );
    r
}

#[allow(clippy::too_many_arguments)]
fn nav_text(
    ui: &egui::Ui,
    st: &Style,
    rect: egui::Rect,
    x: f32,
    right: f32,
    label: &str,
    count: usize,
    on: bool,
) {
    let p = ui.painter();
    let ink = if on { Tone::Text } else { Tone::Text2 };
    p.text(
        egui::pos2(x, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        if on {
            egui::FontId::new(
                st.font(Role::Body).size,
                crate::style::Font::SansSemibold.family(st.fonts_ready),
            )
        } else {
            st.font(Role::Body)
        },
        kit::tone(st, ink),
    );
    p.text(
        egui::pos2(right, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        count.to_string(),
        st.font(Role::Caption),
        kit::tone(st, Tone::Text3),
    );
}

/// The library column scrolls when the window is short.
fn nav_column(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui, st: &Style) {
    egui::ScrollArea::vertical()
        .id_salt("kabl-browser-nav")
        .auto_shrink([false, false])
        .show_owned(ui, |ui| {
            nav_rows(ui_state, ui, st);
            nav_foot(editor, ui_state, ui, st);
        });
}

/// The library filters and the category list, each with the number of sounds it holds.
fn nav_rows(ui_state: &mut UiState, ui: &mut egui::Ui, st: &Style) {
    let Some(lib) = ui_state.library.as_ref() else {
        return;
    };
    let count = |f: Filter| match f {
        Filter::All => lib.entries.len(),
        Filter::Favorites => lib.prefs.favorites.len(),
        Filter::Recent => lib.prefs.recents.len(),
        Filter::Factory => lib
            .entries
            .iter()
            .filter(|e| e.origin == Origin::Factory)
            .count(),
        Filter::User => lib
            .entries
            .iter()
            .filter(|e| e.origin == Origin::User)
            .count(),
    };
    let filters = [
        (Filter::All, "all", "All sounds"),
        (Filter::Recent, "recent", "Recent"),
        (Filter::Factory, "factory", "Factory"),
        (Filter::User, "user", "My sounds"),
        (Filter::Favorites, "favorites", "Favourites"),
    ]
    .map(|(f, k, l)| (f, k, l, count(f)));
    let cats: Vec<(&str, usize)> = SHELVES
        .iter()
        .map(|c| {
            (
                *c,
                lib.entries
                    .iter()
                    .filter(|e| e.meta.shelf() == Some(*c))
                    .count(),
            )
        })
        .collect();
    let all = lib.entries.len();
    ui.spacing_mut().item_spacing.y = 0.0;
    kicker(ui, st, "Library");
    ui.add_space(7.0);
    for (f, key, label, n) in filters {
        let r = nav_row(ui, st, label, n, ui_state.browser.filter == f);
        hit(ui_state, &format!("filter:{key}"), &r);
        if r.clicked() {
            ui_state.browser.filter = f;
        }
    }
    ui.add_space(13.0);
    let head = kicker(ui, st, "Category");
    hit(ui_state, "category", &head);
    ui.add_space(6.0);
    let r = cat_row(ui, st, "All", all, ui_state.browser.category.is_none());
    hit(ui_state, "category:Any", &r);
    if r.clicked() {
        ui_state.browser.category = None;
    }
    for (c, n) in cats {
        let r = cat_row(
            ui,
            st,
            c,
            n,
            ui_state.browser.category.as_deref() == Some(c),
        );
        hit(ui_state, &format!("category:{c}"), &r);
        if r.clicked() {
            ui_state.browser.category = Some(c.to_string());
        }
    }
}

/// "New" and the way back to the rack, under the library column.
fn nav_foot(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui, st: &Style) {
    ui.add_space(st.sp(3));
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        let r = ui
            .add(kit::Button::new(st, "New sound").icon(Ic::Plus))
            .tip(st, "Start a new sound from Init Keyboard");
        hit(ui_state, "new", &r);
        if r.clicked() {
            request(editor, ui_state, Pending::New);
        }
    });
}

/// Search and the list of sounds; a double-click on a row sets `open`.
fn list_column(ui_state: &mut UiState, ui: &mut egui::Ui, st: &Style, open: &mut Option<String>) {
    // "Search 64 sounds": the sounds of the library filter being shown.
    let in_filter = ui_state.library.as_ref().map_or(0, |lib| {
        let of = |o: Origin| lib.entries.iter().filter(|e| e.origin == o).count();
        match ui_state.browser.filter {
            Filter::All => lib.entries.len(),
            Filter::Recent => lib.prefs.recents.len(),
            Filter::Favorites => lib.prefs.favorites.len(),
            Filter::Factory => of(Origin::Factory),
            Filter::User => of(Origin::User),
        }
    });
    let r = kit::field(
        ui,
        st,
        &mut ui_state.browser.query,
        &format!("Search {in_filter} sounds"),
        Some(Ic::Search),
        None,
    );
    hit(ui_state, "search", &r);
    kit::gap(ui, st, 1);
    // The active category as a chip; a click drops it.
    if let Some(cat) = ui_state.browser.category.clone() {
        let r = category_chip(ui, st, &cat);
        hit(ui_state, "category-chip", &r);
        if r.clicked() {
            ui_state.browser.category = None;
        }
        kit::gap(ui, st, 1);
    }
    {
        let (rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), st.sp(4)), Sense::hover());
        let [x, x_macros, x_tags] = sound_cols(rect, st);
        for (x, label) in [(x, "NAME"), (x_macros, "MACROS"), (x_tags, "TAGS")] {
            ui.painter().text(
                pos2(x, rect.center().y),
                egui::Align2::LEFT_CENTER,
                label,
                st.font(Role::Caption),
                st.roles.text3,
            );
        }
    }
    let lib = ui_state.library.as_ref().unwrap();
    let b = &ui_state.browser;
    let visible = |e: &&Entry| {
        e.matches(&b.query)
            && b.category
                .as_deref()
                .is_none_or(|c| e.meta.shelf() == Some(c))
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

    // The panel body scrolls, so the list takes the room and the audition block sits
    // below it, reached by scrolling when the window is short.
    let list_h = ui.available_height();
    let mut budget = MACROS_PER_FRAME;
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
                kit::label(
                    ui,
                    st,
                    Role::Body,
                    Tone::Text3,
                    "No sound matches the search or category.",
                );
            } else if rows.is_empty() {
                kit::paragraph(
                    ui,
                    st,
                    Role::Body,
                    Tone::Text3,
                    match filter {
                        Filter::Favorites => "No favorites yet: click the star next to a sound.",
                        Filter::Recent => "Nothing opened yet.",
                        Filter::User => "No sounds of yours yet: Save As puts them here.",
                        _ => "No sound matches.",
                    },
                );
            }
            for row in &rows {
                match row {
                    Ok(e) => {
                        if filter != Filter::Recent && last_origin != Some(e.origin) {
                            last_origin = Some(e.origin);
                            kit::gap(ui, st, 1);
                            kit::section(
                                ui,
                                st,
                                match e.origin {
                                    Origin::Factory => "Factory",
                                    Origin::User => "Your Sounds",
                                },
                            );
                        }
                        let fav = favorites.contains(&e.id);
                        let sel = ui_state.browser.selected.as_deref() == Some(&e.id);
                        let macros = macros_cached(&mut ui_state.browser, e, &mut budget)
                            .iter()
                            .map(|(n, _)| n.as_str())
                            .collect::<Vec<_>>()
                            .join(" · ");
                        let (row, star) = sound_row(ui, st, e, &macros, fav, sel);
                        let star = star.tip(
                            st,
                            if fav {
                                "Remove from favorites"
                            } else {
                                "Add to favorites"
                            },
                        );
                        ui_state.record(format!("fav:{}", e.id), star.rect);
                        ui_state.record(format!("sound:{}", e.id), row.rect);
                        if star.clicked() {
                            toggle = Some(e.id.clone());
                        } else if row.clicked() {
                            ui_state.browser.selected = Some(e.id.clone());
                        }
                        if row.double_clicked() {
                            *open = Some(e.id.clone());
                        }
                    }
                    Err(id) => {
                        ui.horizontal(|ui| {
                            kit::label(ui, st, Role::Body, Tone::Text3, format!("{id}: not found"))
                                .tip(
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
                kit::label(
                    ui,
                    st,
                    Role::Body,
                    Tone::Text3,
                    "None yet: Save As puts your sounds here.",
                );
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
}

/// The selected sound: name, description, Open, the audition block and the folder tools.
fn detail_column(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    open: &mut Option<String>,
) {
    let body = egui::ScrollArea::vertical()
        .id_salt("kabl-browser-body")
        .auto_shrink([false, false])
        .show_owned(ui, |ui| {
            ui.set_max_width(ui.available_width() - st.sp(1));
            // The selected sound.
            let selected = ui_state
                .browser
                .selected
                .clone()
                .and_then(|id| ui_state.library.as_ref().unwrap().get(&id).cloned());
            match &selected {
                Some(e) => {
                    let kicker = format!(
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
                    );
                    kit::section(ui, st, kicker);
                    kit::label_truncated(ui, st, Role::Display, Tone::Text, &e.meta.name);
                    if !e.meta.description.is_empty() {
                        kit::paragraph(ui, st, Role::Body, Tone::Text2, &e.meta.description);
                    }
                    let compact = ui.available_width() < 330.0;
                    let mut one = 1;
                    let macros = macros_cached(&mut ui_state.browser, e, &mut one).to_vec();
                    if !macros.is_empty() {
                        kit::gap(ui, st, 1);
                        kit::section(ui, st, "Macros");
                        for chunk in macros.chunks(4) {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                let w = ui.available_width() / 4.0;
                                for (name, v) in chunk {
                                    macro_knob(ui, st, name, *v, w, compact);
                                }
                            });
                        }
                    }
                    kit::gap(ui, st, 1);
                    let r = ui
                        .add(
                            kit::Button::new(st, "Load sound")
                                .primary()
                                .min_width(ui.available_width()),
                        )
                        .tip(st, "Load into the rack");
                    hit(ui_state, "open", &r);
                    if r.clicked() {
                        *open = Some(e.id.clone());
                    }
                    ui.horizontal(|ui| {
                        let gap = st.sp(1);
                        ui.spacing_mut().item_spacing.x = gap;
                        let w = (ui.available_width() - gap) / 2.0;
                        let (keys, sequence) = Meta::play_of(editor.state());
                        let r = ui
                            .add(
                                kit::Button::new(st, "Audition")
                                    .min_width(w)
                                    .enabled(keys || sequence),
                            )
                            .tip(st, "Play the sound in the rack; options below");
                        hit(ui_state, "audition", &r);
                        if r.clicked() {
                            if keys {
                                let a = &ui_state.browser.audition;
                                let (cmd, secs) = (a.command(ui_state.sample_rate), a.secs as f64);
                                ui_state.launches.push(cmd);
                                let now = ui.input(|i| i.time);
                                ui_state.browser.preview_until = Some(now + secs);
                            } else {
                                let clocks = editor
                                    .state()
                                    .modules
                                    .iter()
                                    .filter(|(_, m)| m.kind == "clock")
                                    .map(|(&id, _)| (id, Transport::Run));
                                ui_state.transport.extend(clocks);
                            }
                        }
                        let r = ui
                            .add(kit::Button::new(st, "Open in rack").min_width(w))
                            .tip(st, "Load into the rack and show the rack");
                        hit(ui_state, "open-rack", &r);
                        if r.clicked() {
                            *open = Some(e.id.clone());
                            ui_state.browser_open = false;
                        }
                    });
                    // Quiet facts below the buttons.
                    kit::gap(ui, st, 1);
                    ui.horizontal(|ui| {
                        badge(ui, st, &e.meta);
                        if e.origin == Origin::User {
                            let r = ui.add(kit::Button::new(st, "Rename").small().ghost());
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
                    if !e.meta.tags.is_empty() {
                        let tags = format!("tags: {}", e.meta.tags.join(", "));
                        kit::label_truncated(ui, st, Role::Caption, Tone::Text3, &tags)
                            .tip(st, &tags);
                    }
                }
                None => {
                    kit::paragraph(
                        ui,
                        st,
                        Role::Body,
                        Tone::Text3,
                        "Click a sound to see it; Load sound (or double-click) loads it.",
                    );
                }
            }
            kit::gap(ui, st, 1);
            kit::rule(ui, st);
            kit::gap(ui, st, 1);
            play_section(editor, ui_state, ui);

            kit::gap(ui, st, 1);
            kit::rule(ui, st);
            kit::gap(ui, st, 1);
            if let Some(p) = filebrowser::take(ui_state, Purpose::PatchLoad) {
                let p = p.to_string_lossy().into_owned();
                ui_state.browser.folder.clone_from(&p);
                request(editor, ui_state, Pending::OpenFolder(p));
            }
            if let Some(p) = filebrowser::take(ui_state, Purpose::PatchSave) {
                let p = p.to_string_lossy().into_owned();
                ui_state.browser.folder.clone_from(&p);
                save_to_folder(editor, ui_state, p);
            }
            let open_f = ui_state.browser.folder_open;
            let header = ui.add(
                kit::Button::new(st, "Patch folder (advanced)")
                    .ghost()
                    .icon(if open_f { Ic::Down } else { Ic::Right }),
            );
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
                            save_to_folder(editor, ui_state, p);
                        }
                    });
                    ui.horizontal(|ui| {
                        let r = ui.add(kit::Button::new(st, "Browse to open"));
                        hit(ui_state, "browse-load", &r);
                        if r.clicked() {
                            let hint = ui_state.browser.folder.clone();
                            filebrowser::open(
                                ui_state,
                                Purpose::PatchLoad,
                                "Open a patch folder",
                                Mode::Open,
                                filebrowser::PATCHES,
                                "",
                                &hint,
                            );
                        }
                        let r = ui.add(kit::Button::new(st, "Browse to save"));
                        hit(ui_state, "browse-save", &r);
                        if r.clicked() {
                            let hint = ui_state.browser.folder.clone();
                            let name = ui_state
                                .doc
                                .as_ref()
                                .map_or(String::new(), |d| d.name.clone());
                            filebrowser::open(
                                ui_state,
                                Purpose::PatchSave,
                                "Save the patch to a folder",
                                Mode::Save,
                                filebrowser::PATCHES,
                                &name,
                                &hint,
                            );
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
        });
    ui_state.record("browser-body".into(), body.inner_rect);
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
    kit::label_truncated(
        ui,
        st,
        Role::Body,
        Tone::Text,
        format!("In the rack: {name}"),
    );
    let (keys, sequence) = Meta::play_of(editor.state());
    if keys {
        let mut chord_hits = Vec::new();
        let a = &mut ui_state.browser.audition;
        ui.horizontal(|ui| {
            let r = ui
                .add(kit::Button::new(st, "−8").small())
                .tip(st, "An octave down");
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
            let r = ui
                .add(kit::Button::new(st, "+8").small())
                .tip(st, "An octave up");
            chord_hits.push(("note-up".to_string(), r.rect));
            if r.clicked() {
                a.note = (a.note + 12).min(96);
            }
        });
        ui.horizontal(|ui| {
            for c in [Chord::Single, Chord::Major, Chord::Minor] {
                let r = ui.add(
                    kit::Button::new(st, c.label())
                        .small()
                        .selected(a.chord == c),
                );
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
            let r = ui
                .add(kit::Button::new(st, label).icon(Ic::Play))
                .tip(st, &help);
            hit(ui_state, "play", &r);
            if r.clicked() {
                ui_state.launches.push(cmd);
                let now = ui.input(|i| i.time);
                ui_state.browser.preview_until = Some(now + secs);
            }
            let r = ui.add(
                kit::Button::new(st, "Stop")
                    .icon(Ic::Stop)
                    .enabled(ui_state.browser.preview_until.is_some()),
            );
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
            let r = ui.add(
                kit::Button::new(st, "Start")
                    .icon(Ic::Play)
                    .primary()
                    .enabled(!running),
            );
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
                if running {
                    "sequence running"
                } else {
                    "sequence stopped"
                },
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
        kit::paragraph(
            ui,
            st,
            Role::Body,
            Tone::Text3,
            "This sound has no keyboard input and no clock: nothing to audition.",
        );
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    Plain,
    Primary,
    Danger,
}

/// Focuses a dialog's first field once, when the dialog opens (`dialogs` clears the flag).
fn first_focus(ui: &egui::Ui, r: &egui::Response) {
    let id = egui::Id::new("dlg-focused");
    if !ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false) {
        // Asked for until the field reports it has it (the first frame of a modal is a sizing pass).
        if ui.memory(|m| m.has_focus(r.id)) {
            ui.data_mut(|d| d.insert_temp(id, true));
        } else {
            r.request_focus();
        }
    }
}

/// A 32 px dialog footer button (G4-SPEC §3); records its rect under `key` for the tests.
fn dialog_button(
    ui: &mut egui::Ui,
    st: &Style,
    hits: &mut Vec<(&'static str, Rect)>,
    key: &'static str,
    label: &str,
    look: Look,
) -> bool {
    let b = kit::Button::new(st, label).g4(32.0);
    let b = match look {
        Look::Plain => b,
        Look::Primary => b.primary(),
        // The mockup: a red label with no outline.
        Look::Danger => b.ghost().tone(Tone::Bad),
    };
    let r = ui.add(b);
    hits.push((key, r.rect));
    r.clicked()
}

/// The open dialog, if any (modal). Escape cancels it.
pub fn dialogs(editor: &mut PatchEditor, ui_state: &mut UiState, ctx: &egui::Context) {
    // The pick of "Choose folder…" in the Save as dialog (the dialog closed to make room).
    let choose_id = egui::Id::new("dlg-choose-folder");
    if ctx.data(|d| d.get_temp::<bool>(choose_id)).unwrap_or(false) {
        if let Some(p) = filebrowser::take(ui_state, Purpose::PatchSave) {
            ctx.data_mut(|d| d.remove::<bool>(choose_id));
            let p = p.to_string_lossy().into_owned();
            ui_state.browser.folder.clone_from(&p);
            save_to_folder(editor, ui_state, p);
        } else if !filebrowser::is_open(ui_state, Purpose::PatchSave) {
            ctx.data_mut(|d| d.remove::<bool>(choose_id));
        }
    }
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
            let (mut fh, mut lh) = (Vec::new(), Vec::new());
            let (mut save_it, mut discard, mut cancel) = (false, false, false);
            let closed = g4::modal_framed(
                ctx,
                st,
                "dlg-unsaved",
                Size::S,
                "Unsaved changes",
                |ui| {
                    kit::paragraph(
                        ui,
                        st,
                        Role::Body,
                        Tone::Text,
                        format!("\u{201c}{name}\u{201d} has changes you haven\u{2019}t saved."),
                    );
                    kit::paragraph(
                        ui,
                        st,
                        Role::Body,
                        Tone::Text2,
                        format!("Save them before {what}?"),
                    );
                    if let Some(e) = &error {
                        kit::gap(ui, st, 1);
                        g4::banner(ui, st, Sev::Bad, "Could not save", Some(e));
                    }
                },
                |ui| {
                    discard =
                        dialog_button(ui, st, &mut lh, "dlg:discard", "Don't save", Look::Danger)
                },
                |ui| {
                    save_it = dialog_button(ui, st, &mut fh, "dlg:save", "Save", Look::Primary);
                    cancel = dialog_button(ui, st, &mut fh, "dlg:cancel", "Cancel", Look::Plain);
                },
            );
            for (k, r) in lh.into_iter().chain(fh) {
                ui_state.record(k.into(), r);
            }
            if save_it {
                next = save(editor, ui_state, Some(then.clone())).map(|e| Dialog::Unsaved {
                    then: then.clone(),
                    error: Some(e),
                });
            } else if discard {
                next = None;
                perform(editor, ui_state, then.clone());
            } else if cancel || closed {
                next = None;
            }
            next
        }
        Dialog::CodeNotice { entries, then } => match crate::code_notice::show(ctx, st, &entries) {
            None => Some(Dialog::CodeNotice { entries, then }),
            Some(open) => {
                let note = ui_state
                    .library
                    .as_mut()
                    .and_then(|l| l.remember_code_trust(&entries, open));
                if open {
                    perform(editor, ui_state, then);
                    if let Some(n) = note {
                        message(ui_state, n);
                    }
                } else {
                    message(
                        ui_state,
                        note.unwrap_or_else(|| "not opened: its code was not run".into()),
                    );
                }
                None
            }
        },
        Dialog::SaveAs {
            mut name,
            mut category,
            mut tags,
            mut taken,
            then,
            mut error,
        } => {
            // A confirmation belongs to the name it was offered for: editing the name drops it
            // (and its message), so Replace it can never send an old target with a new name.
            let stale = |taken: &Option<(String, String)>, name: &str| {
                taken
                    .as_ref()
                    .is_some_and(|(_, n)| n.to_lowercase() != name.trim().to_lowercase())
            };
            if stale(&taken, &name) {
                taken = None;
                error = None;
            }
            let replace_id = taken.as_ref().map(|(id, _)| id.clone());
            let retry = error.is_some() && taken.is_none();
            let is_taken = taken.is_some();
            let mut enter = false;
            let (mut bh, mut fh) = (Vec::new(), Vec::new());
            let (mut ok, mut replace, mut cancel, mut choose) = (false, false, false, false);
            let add_id = egui::Id::new("dlg-add-tag");
            let closed = g4::modal_framed(
                ctx,
                st,
                "dlg-save-as",
                Size::M,
                "Save as",
                |ui| {
                    if let Some(e) = &error {
                        let (sev, title) = if is_taken {
                            (Sev::Warn, "Name already in use")
                        } else {
                            (Sev::Bad, "Could not write the sound")
                        };
                        g4::banner(ui, st, sev, title, Some(e));
                        kit::gap(ui, st, 2);
                    } else {
                        kit::paragraph(
                            ui,
                            st,
                            Role::Body,
                            Tone::Text2,
                            "Saves a copy in Your Sounds. Factory sounds stay as they are.",
                        );
                        kit::gap(ui, st, 2);
                    }
                    let label = |ui: &mut egui::Ui, t: &str| {
                        ui.allocate_ui_with_layout(
                            vec2(96.0, st.metrics.field_h),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.add(
                                    egui::Label::new(kit::rich(st, Role::Body, Tone::Text2, t))
                                        .selectable(false),
                                )
                            },
                        );
                    };
                    ui.horizontal(|ui| {
                        label(ui, "Name");
                        let w = ui.available_width();
                        let r = kit::field(ui, st, &mut name, "", None, Some(w));
                        bh.push(("dlg:name", r.rect));
                        first_focus(ui, &r);
                        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            enter = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        label(ui, "Category");
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
                        label(ui, "Tags");
                        let mut list: Vec<String> = tags
                            .split(',')
                            .map(|t| t.trim().to_string())
                            .filter(|t| !t.is_empty())
                            .collect();
                        let mut gone = None;
                        for (i, t) in list.iter().enumerate() {
                            let r = g4::chip_label(ui, st, &format!("{t}  \u{d7}"), false, None);
                            let r = ui.interact(r.rect, add_id.with(i), Sense::click());
                            if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                gone = Some(i);
                            }
                        }
                        let mut add = ui
                            .data(|d| d.get_temp::<String>(add_id))
                            .unwrap_or_default();
                        let r = kit::field(ui, st, &mut add, "Add tag", None, Some(120.0));
                        let commit = (r.lost_focus()
                            && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                            || add.ends_with(',');
                        if commit {
                            let t = add.trim().trim_end_matches(',').trim().to_string();
                            if !t.is_empty() && !list.contains(&t) {
                                list.push(t);
                            }
                            add.clear();
                            r.request_focus();
                        }
                        if let Some(i) = gone {
                            list.remove(i);
                        }
                        ui.data_mut(|d| d.insert_temp(add_id, add));
                        tags = list.join(", ");
                    });
                },
                |_| {},
                |ui| {
                    let label = if retry { "Try again" } else { "Save" };
                    ok = dialog_button(ui, st, &mut fh, "dlg:save", label, Look::Primary);
                    if retry {
                        choose = dialog_button(
                            ui,
                            st,
                            &mut fh,
                            "dlg:choose-folder",
                            "Choose folder\u{2026}",
                            Look::Plain,
                        );
                    }
                    if replace_id.is_some() {
                        replace = dialog_button(
                            ui,
                            st,
                            &mut fh,
                            "dlg:replace",
                            "Replace it",
                            Look::Plain,
                        );
                    }
                    cancel = dialog_button(ui, st, &mut fh, "dlg:cancel", "Cancel", Look::Plain);
                },
            );
            for (k, r) in bh.into_iter().chain(fh) {
                ui_state.record(k.into(), r);
            }
            // The name may have been edited this frame: a Replace for the old name is void.
            let replace_ok = replace && !stale(&taken, &name);
            if choose {
                // The save goes to a folder picked in the file browser; the dialog closes.
                let hint = ui_state.browser.folder.clone();
                filebrowser::open(
                    ui_state,
                    Purpose::PatchSave,
                    "Save the patch to a folder",
                    Mode::Save,
                    filebrowser::FOLDERS,
                    name.trim(),
                    &hint,
                );
                ctx.data_mut(|d| d.insert_temp(egui::Id::new("dlg-choose-folder"), true));
                None
            } else if cancel || closed {
                None
            } else if ok || enter || replace_ok {
                let target = if replace_ok { replace_id } else { None };
                save_as(
                    editor,
                    ui_state,
                    &name,
                    &category,
                    &tags,
                    target,
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
                })
            } else {
                Some(Dialog::SaveAs {
                    name,
                    category,
                    tags,
                    taken,
                    then,
                    error,
                })
            }
        }
        Dialog::Rename {
            id,
            mut name,
            error,
        } => {
            let mut enter = false;
            let (mut bh, mut fh) = (Vec::new(), Vec::new());
            let (mut ok, mut cancel) = (false, false);
            let closed = g4::modal_framed(
                ctx,
                st,
                "dlg-rename",
                Size::S,
                "Rename sound",
                |ui| {
                    if let Some(e) = &error {
                        g4::banner(ui, st, Sev::Bad, "Not renamed", Some(e));
                        kit::gap(ui, st, 2);
                    }
                    let r = kit::field(ui, st, &mut name, "", None, None);
                    bh.push(("dlg:name", r.rect));
                    first_focus(ui, &r);
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        enter = true;
                    }
                },
                |_| {},
                |ui| {
                    ok = dialog_button(ui, st, &mut fh, "dlg:save", "Rename", Look::Primary);
                    cancel = dialog_button(ui, st, &mut fh, "dlg:cancel", "Cancel", Look::Plain);
                },
            );
            for (k, r) in bh.into_iter().chain(fh) {
                ui_state.record(k.into(), r);
            }
            if cancel || closed {
                None
            } else if ok || enter {
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
            let mut fh = Vec::new();
            let (mut ok, mut cancel) = (false, false);
            let closed = g4::modal_framed(
                ctx,
                st,
                "dlg-restore",
                Size::M,
                "Restore the reference?",
                |ui| {
                    kit::paragraph(
                        ui,
                        st,
                        Role::Body,
                        Tone::Text,
                        format!(
                            "Replaces the whole patch: values, cables, routes, labels, Perform \
                             pins and MIDI mappings, with the reference captured {at}."
                        ),
                    );
                    kit::gap(ui, st, 1);
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                        kit::icon(ui.painter(), Ic::Undo, r.center(), 16.0, st.roles.text3);
                        kit::paragraph(
                            ui,
                            st,
                            Role::Body,
                            Tone::Text2,
                            "One undo step brings your version back.",
                        );
                    });
                    if redo {
                        kit::paragraph(
                            ui,
                            st,
                            Role::Caption,
                            Tone::Bad,
                            "Like any edit, it replaces the steps you could Redo right now.",
                        );
                    }
                },
                |_| {},
                |ui| {
                    ok = dialog_button(ui, st, &mut fh, "dlg:restore", "Restore", Look::Primary);
                    cancel = dialog_button(ui, st, &mut fh, "dlg:cancel", "Cancel", Look::Plain);
                },
            );
            for (k, r) in fh {
                ui_state.record(k.into(), r);
            }
            if ok {
                let m = crate::compare::restore(editor, ui_state);
                message(ui_state, m);
                next = None;
            } else if cancel || closed {
                next = None;
            }
            next
        }
    };
    if next.is_none() {
        ctx.data_mut(|d| d.remove::<bool>(egui::Id::new("dlg-focused")));
    }
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

/// After opening a patch whose code could not run: one toast naming each module's id, version
/// and cause (it replaces the "opened ..." note, which would hide the warning).
fn stand_in_warning(editor: &PatchEditor, ui: &mut UiState) {
    let mut w = crate::code_notice::warnings(editor.state());
    w.extend(crate::module_install::missing_report(editor.state()));
    if !w.is_empty() {
        message(ui, w.join(" | "));
    }
}
