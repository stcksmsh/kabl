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

use egui::{Color32, RichText};
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
    /// The window had focus last frame.
    focused: bool,
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
            focused: true,
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
    if is_modified(editor, ui) {
        ui.browser.dialog = Some(Dialog::Unsaved {
            then: p,
            error: None,
        });
    } else {
        perform(editor, ui, p);
    }
}

fn message(ui: &mut UiState, text: String) {
    ui.last_message = Some(text);
}

/// Does `p` without asking. A failure leaves the editor as it was and says why.
pub fn perform(editor: &mut PatchEditor, ui: &mut UiState, p: Pending) {
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
                    ui.load_stopped = e.meta.sequence;
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
                Err(err) => message(
                    ui,
                    format!("couldn't open: {err}. Your sound is unchanged."),
                ),
            }
        }
        Pending::OpenFolder(path) => {
            match crate::library::read_patch(std::path::Path::new(&path)) {
                Ok(log) => {
                    replace_patch(editor, ui, log);
                    let name = std::path::Path::new(&path)
                        .file_name()
                        .map_or(path.clone(), |n| n.to_string_lossy().to_string());
                    ui.doc = Some(Doc::new(
                        &name,
                        DocOrigin::Folder(path.clone()),
                        editor.state(),
                    ));
                    message(ui, format!("loaded {path}"));
                }
                Err(err) => message(ui, format!("load failed: {err}. Your sound is unchanged.")),
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
            message(ui, "new sound from Init Keyboard".into());
        }
    }
}

/// Save to where the document lives, or ask for a name (factory, new). `then` runs after a
/// successful save. Returns an error message when the save failed (the dialog shows it).
fn save(editor: &mut PatchEditor, ui: &mut UiState, then: Option<Pending>) -> Option<String> {
    let doc = ui.doc.clone()?;
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
            if let Some(d) = ui.doc.as_mut() {
                d.saved = editor.state().clone();
            }
            message(ui, format!("saved \"{}\"", doc.name));
            if let Some(p) = then {
                perform(editor, ui, p);
            }
            None
        }
        Err(e) => {
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
    if ui_state.browser.dialog.is_none()
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
pub fn toolbar(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui) {
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
            [100.0, 20.0],
            egui::Label::new(RichText::new(text).strong()).truncate(),
        )
        .on_hover_text(if modified {
            format!("{name}: unsaved changes")
        } else if factory {
            format!("{name}: factory sound (Save makes your own copy)")
        } else {
            name.clone()
        });
    hit(ui_state, "doc-name", &r);
    let r = ui.button("Save").on_hover_text("Ctrl+S");
    hit(ui_state, "save", &r);
    if r.clicked() {
        save(editor, ui_state, None);
    }
    let r = ui.button("Save As");
    hit(ui_state, "save-as", &r);
    if r.clicked() {
        open_save_as(ui_state, None);
    }
}

fn badge(ui: &mut egui::Ui, e: &Meta) {
    let (text, color) = match (e.keys, e.sequence) {
        (true, true) => ("Seq + Keys", Color32::from_rgb(150, 110, 200)),
        (false, true) => ("Sequence", Color32::from_rgb(200, 130, 60)),
        (true, false) => ("Keys", Color32::from_rgb(70, 140, 200)),
        (false, false) => ("—", Color32::GRAY),
    };
    ui.label(RichText::new(text).small().color(color));
}

/// The left panel.
pub fn panel(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.heading("Sounds");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let r = ui.button("Close");
            hit(ui_state, "browser-close", &r);
            if r.clicked() {
                ui_state.browser_open = false;
            }
            let r = ui
                .button("New")
                .on_hover_text("Start a new sound from Init Keyboard");
            hit(ui_state, "new", &r);
            if r.clicked() {
                request(editor, ui_state, Pending::New);
            }
        });
    });
    if ui_state.library.is_none() {
        ui.label("The sound library isn't available.");
        return;
    }
    let r = ui.add(
        egui::TextEdit::singleline(&mut ui_state.browser.query)
            .hint_text("Search name, category, tag")
            .desired_width(f32::INFINITY),
    );
    hit(ui_state, "search", &r);
    ui.horizontal_wrapped(|ui| {
        for (f, key, label) in [
            (Filter::All, "all", "All"),
            (Filter::Favorites, "favorites", "★ Favorites"),
            (Filter::Recent, "recent", "Recent"),
            (Filter::Factory, "factory", "Factory"),
            (Filter::User, "user", "Your Sounds"),
        ] {
            let r = ui.add(egui::Button::selectable(
                ui_state.browser.filter == f,
                label,
            ));
            hit(ui_state, &format!("filter:{key}"), &r);
            if r.clicked() {
                ui_state.browser.filter = f;
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("Category");
        let current = ui_state
            .browser
            .category
            .clone()
            .unwrap_or_else(|| "Any".into());
        let r = egui::ComboBox::from_id_salt("browser-category")
            .selected_text(current)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut ui_state.browser.category, None, "Any");
                for c in CATEGORIES {
                    ui.selectable_value(&mut ui_state.browser.category, Some(c.to_string()), *c);
                }
            });
        hit(ui_state, "category", &r.response);
    });
    ui.separator();

    let lib = ui_state.library.as_ref().unwrap();
    let b = &ui_state.browser;
    let visible = |e: &&Entry| {
        e.matches(&b.query)
            && b.category.as_ref().is_none_or(|c| &e.meta.category == c)
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

    let list_h = (ui.available_height() - 330.0).max(120.0);
    let mut open = None;
    let mut toggle = None;
    let mut forget = None;
    egui::ScrollArea::vertical()
        .max_height(list_h)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut last_origin = None;
            let narrowed =
                !ui_state.browser.query.trim().is_empty() || ui_state.browser.category.is_some();
            if rows.is_empty() && narrowed {
                ui.weak("No sound matches the search or category.");
            } else if rows.is_empty() {
                ui.weak(match filter {
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
                            ui.label(
                                RichText::new(match e.origin {
                                    Origin::Factory => "FACTORY",
                                    Origin::User => "YOUR SOUNDS",
                                })
                                .small()
                                .strong(),
                            );
                        }
                        ui.horizontal(|ui| {
                            let fav = favorites.contains(&e.id);
                            let r = ui
                                .add(egui::Button::new(if fav { "★" } else { "☆" }).frame(false))
                                .on_hover_text(if fav {
                                    "Remove from favorites"
                                } else {
                                    "Add to favorites"
                                });
                            ui_state.record(format!("fav:{}", e.id), r.rect);
                            if r.clicked() {
                                toggle = Some(e.id.clone());
                            }
                            let sel = ui_state.browser.selected.as_deref() == Some(&e.id);
                            let r = ui.add(
                                egui::Button::selectable(sel, "")
                                    .left_text(e.meta.name.as_str())
                                    .min_size(egui::vec2(150.0, 0.0)),
                            );
                            ui_state.record(format!("sound:{}", e.id), r.rect);
                            if r.clicked() {
                                ui_state.browser.selected = Some(e.id.clone());
                            }
                            if r.double_clicked() {
                                open = Some(e.id.clone());
                            }
                            ui.label(RichText::new(&e.meta.category).small().weak());
                            badge(ui, &e.meta);
                        });
                    }
                    Err(id) => {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{id}: not found")).weak())
                                .on_hover_text(
                                    "Removed or renamed outside kabl. Forget drops it from \
                                     favorites and recents.",
                                );
                            let r = ui.small_button("Forget");
                            ui_state.record(format!("forget:{id}"), r.rect);
                            if r.clicked() {
                                forget = Some(id.clone());
                            }
                        });
                    }
                }
            }
            if filter == Filter::All && !has_user && ui_state.browser.query.is_empty() {
                ui.label(RichText::new("YOUR SOUNDS").small().strong());
                ui.weak("None yet: Save As puts your sounds here.");
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
    ui.separator();

    // The selected sound.
    let selected = ui_state
        .browser
        .selected
        .clone()
        .and_then(|id| ui_state.library.as_ref().unwrap().get(&id).cloned());
    match &selected {
        Some(e) => {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&e.meta.name).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let r = ui.button("Open").on_hover_text("Load into the rack");
                    hit(ui_state, "open", &r);
                    if r.clicked() {
                        open = Some(e.id.clone());
                    }
                    if e.origin == Origin::User {
                        let r = ui.button("Rename");
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
                ui.label(
                    RichText::new(format!(
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
                    ))
                    .small(),
                );
                badge(ui, &e.meta);
            });
            if !e.meta.description.is_empty() {
                ui.label(RichText::new(&e.meta.description).small());
            }
            if !e.meta.tags.is_empty() {
                ui.label(
                    RichText::new(format!("tags: {}", e.meta.tags.join(", ")))
                        .small()
                        .weak(),
                );
            }
        }
        None => {
            ui.weak("Click a sound to see it; Open (or double-click) loads it.");
        }
    }
    if let Some(id) = open {
        request(editor, ui_state, Pending::Open(id));
    }
    ui.separator();
    play_section(editor, ui_state, ui);

    ui.separator();
    let folder = egui::CollapsingHeader::new("Patch folder (advanced)")
        .default_open(false)
        .show(ui, |ui| {
            let r = ui.add(
                egui::TextEdit::singleline(&mut ui_state.browser.folder)
                    .desired_width(f32::INFINITY),
            );
            hit(ui_state, "patch-path", &r);
            ui.horizontal(|ui| {
                let r = ui.button("Load folder");
                hit(ui_state, "load", &r);
                if r.clicked() {
                    let p = ui_state.browser.folder.clone();
                    request(editor, ui_state, Pending::OpenFolder(p));
                }
                let r = ui.button("Save to folder");
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
            ui.label(
                RichText::new(format!(
                    "Your sounds: {}\nFactory: {}",
                    lib.sounds_dir().display(),
                    lib.factory_dir
                        .as_ref()
                        .map_or("not found".into(), |d| d.display().to_string())
                ))
                .small()
                .weak(),
            );
        });
    hit(ui_state, "folder-header", &folder.header_response);
    let notes = ui_state.library.as_ref().unwrap().notes.clone();
    for n in notes {
        ui.label(
            RichText::new(n)
                .small()
                .color(Color32::from_rgb(200, 120, 40)),
        );
    }
}

/// Audition for keyboard sounds, Start/Stop for pieces: always the sound in the rack.
fn play_section(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui) {
    let name = ui_state
        .doc
        .as_ref()
        .map_or(String::new(), |d| d.name.clone());
    ui.label(RichText::new(format!("In the rack: {name}")).strong());
    let (keys, sequence) = Meta::play_of(editor.state());
    if keys {
        let mut chord_hits = Vec::new();
        let a = &mut ui_state.browser.audition;
        ui.horizontal(|ui| {
            let r = ui.small_button("−8").on_hover_text("An octave down");
            chord_hits.push(("note-down".to_string(), r.rect));
            if r.clicked() {
                a.note = a.note.saturating_sub(12).max(24);
            }
            let r = egui::ComboBox::from_id_salt("audition-note")
                .width(56.0)
                .selected_text(note_name(a.note))
                .show_ui(ui, |ui| {
                    for n in (36..=84).step_by(1) {
                        ui.selectable_value(&mut a.note, n, note_name(n));
                    }
                });
            chord_hits.push(("note".to_string(), r.response.rect));
            let r = ui.small_button("+8").on_hover_text("An octave up");
            chord_hits.push(("note-up".to_string(), r.rect));
            if r.clicked() {
                a.note = (a.note + 12).min(96);
            }
            for c in [Chord::Single, Chord::Major, Chord::Minor] {
                let r = ui.add(egui::Button::selectable(a.chord == c, c.label()));
                chord_hits.push((format!("chord:{}", c.label()), r.rect));
                if r.clicked() {
                    a.chord = c;
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Velocity");
            let r = ui.add(egui::Slider::new(&mut a.velocity, 1..=127).show_value(true));
            chord_hits.push(("velocity".to_string(), r.rect));
        });
        ui.horizontal(|ui| {
            ui.label("Length");
            let r = ui.add(
                egui::Slider::new(&mut a.secs, 0.25..=4.0)
                    .step_by(0.25)
                    .suffix(" s"),
            );
            chord_hits.push(("length".to_string(), r.rect));
        });
        let label = format!(
            "▶ Play {}{}",
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
            let r = ui.button(label).on_hover_text(&help);
            hit(ui_state, "play", &r);
            if r.clicked() {
                ui_state.launches.push(cmd);
                let now = ui.input(|i| i.time);
                ui_state.browser.preview_until = Some(now + secs);
            }
            let r = ui.add_enabled(
                ui_state.browser.preview_until.is_some(),
                egui::Button::new("■ Stop"),
            );
            hit(ui_state, "stop-preview", &r);
            if r.clicked() {
                ui_state.launches.push(Command::PreviewStop);
                ui_state.browser.preview_until = None;
            }
            if ui_state.browser.preview_until.is_some() {
                ui.label(RichText::new("sounding").small());
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(100));
            }
        });
        ui.label(RichText::new(help).small().weak());
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
            let r = ui.add_enabled(!running, egui::Button::new("▶ Start"));
            hit(ui_state, "start", &r);
            if r.clicked() {
                ui_state
                    .transport
                    .extend(clocks.iter().map(|&id| (id, Transport::Run)));
            }
            let r = ui.add_enabled(running, egui::Button::new("■ Stop"));
            hit(ui_state, "stop", &r);
            if r.clicked() {
                ui_state
                    .transport
                    .extend(clocks.iter().map(|&id| (id, Transport::Stop)));
            }
            ui.label(
                RichText::new(if running {
                    "sequence running"
                } else {
                    "sequence stopped"
                })
                .small(),
            );
        });
        ui.label(
            RichText::new("Start and Stop run every clock in the sound (the clock's own buttons do the same).")
                .small()
                .weak(),
        );
    }
    if !keys && !sequence {
        ui.weak("This sound has no keyboard input and no clock: nothing to audition.");
    }
}

/// The open dialog, if any (modal). Escape cancels it.
pub fn dialogs(editor: &mut PatchEditor, ui_state: &mut UiState, ctx: &egui::Context) {
    let Some(dialog) = ui_state.browser.dialog.take() else {
        return;
    };
    let red = Color32::from_rgb(210, 70, 60);
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
            egui::Modal::new(egui::Id::new("dlg-unsaved")).show(ctx, |ui| {
                ui.set_width(380.0);
                ui.heading("Unsaved changes");
                ui.label(format!(
                    "\"{name}\" has changes you haven't saved. Save them before {what}?"
                ));
                if let Some(e) = &error {
                    ui.colored_label(red, e);
                }
                ui.horizontal(|ui| {
                    let r = ui.button("Save");
                    hit(ui_state, "dlg:save", &r);
                    if r.clicked() {
                        next =
                            save(editor, ui_state, Some(then.clone())).map(|e| Dialog::Unsaved {
                                then: then.clone(),
                                error: Some(e),
                            });
                    }
                    let r = ui.button("Don't save");
                    hit(ui_state, "dlg:discard", &r);
                    if r.clicked() {
                        next = None;
                        perform(editor, ui_state, then.clone());
                    }
                    let r = ui.button("Cancel");
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
            egui::Modal::new(egui::Id::new("dlg-save-as")).show(ctx, |ui| {
                ui.set_width(380.0);
                ui.heading("Save As");
                ui.label("Saves a copy in Your Sounds. Factory sounds stay as they are.");
                ui.horizontal(|ui| {
                    ui.label("Name");
                    let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(260.0));
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
                    ui.label("Category");
                    egui::ComboBox::from_id_salt("dlg-category")
                        .selected_text(category.clone())
                        .show_ui(ui, |ui| {
                            for c in CATEGORIES {
                                ui.selectable_value(&mut category, c.to_string(), *c);
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label("Tags");
                    ui.add(
                        egui::TextEdit::singleline(&mut tags)
                            .hint_text("comma separated")
                            .desired_width(260.0),
                    );
                });
                if let Some(e) = &error {
                    ui.colored_label(red, e);
                }
                ui.horizontal(|ui| {
                    let r = ui.button("Save");
                    hit(ui_state, "dlg:save", &r);
                    if r.clicked() {
                        submit = Some(None);
                    }
                    if let Some((id, _)) = &taken {
                        let r = ui.button("Replace it");
                        hit(ui_state, "dlg:replace", &r);
                        if r.clicked() {
                            submit = Some(Some(id.clone()));
                        }
                    }
                    let r = ui.button("Cancel");
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
            egui::Modal::new(egui::Id::new("dlg-rename")).show(ctx, |ui| {
                ui.set_width(340.0);
                ui.heading("Rename");
                let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(300.0));
                hit(ui_state, "dlg:name", &r);
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                }
                if let Some(e) = &error {
                    ui.colored_label(red, e);
                }
                ui.horizontal(|ui| {
                    let r = ui.button("Rename");
                    hit(ui_state, "dlg:save", &r);
                    submit |= r.clicked();
                    let r = ui.button("Cancel");
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
