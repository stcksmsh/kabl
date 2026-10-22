//! The shared file browser: a folder list drawn with the chrome kit, used wherever the
//! application asks for a file or a folder (wavetable import, loading and saving a patch folder,
//! panel art, composite packages, the recordings folder). It is not modal and is not a native
//! dialog, so it looks and behaves the same in the standalone and inside a plugin editor, and
//! cannot hide behind a host window. A folder is listed on a thread of its own (a slow share
//! never holds the interface); a listing is capped and says so. Control thread only.
//!
//! A window using it keeps its own path field as the fallback; the browser only fills it in.

use crate::kit::{self, Button, Tone};
use crate::style::{Role, Style};
use crate::UiState;
use egui::pos2;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

/// Rows one listing shows (a folder with more is cut, and says so).
const MAX_ENTRIES: usize = 2000;
/// Directory entries one listing looks at before it gives up (a folder of millions of files).
const MAX_SCANNED: usize = 100_000;

/// Frames a pick waits for the place that asked before it is dropped.
const MAX_PICK_AGE: u8 = 60;

/// What a use of the browser shows besides folders: files with one of `exts` (lower case, no
/// dot); none means folders only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spec {
    pub exts: &'static [&'static str],
    /// What the files are called in the empty-folder message.
    pub what: &'static str,
}

pub const WAV: Spec = Spec {
    exts: &["wav"],
    what: ".wav files",
};
pub const IMAGE: Spec = Spec {
    exts: &["png"],
    what: ".png images",
};
pub const PACKAGE: Spec = Spec {
    exts: &["json"],
    what: ".json packages",
};
pub const FOLDERS: Spec = Spec {
    exts: &[],
    what: "files",
};

/// What a pick returns and how the browser behaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Click a file to pick it.
    Open,
    /// "Choose this folder" picks the folder being shown.
    Folder,
    /// A name field; "Save" picks `folder/name`, after a confirmation if it exists. A file
    /// click copies its name into the field. With folders only, the name is a folder's.
    Save,
}

/// Where a use of the browser starts and what is remembered between runs (the keys of the
/// `folders` table in `settings.toml`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Place {
    Wavetables,
    Patches,
    Recordings,
    Art,
}

impl Place {
    pub fn key(self) -> &'static str {
        match self {
            Place::Wavetables => "wavetables",
            Place::Patches => "patches",
            Place::Recordings => "recordings",
            Place::Art => "art",
        }
    }
}

/// Who asked for a pick, so only that place takes the answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    PatchLoad,
    PatchSave,
    PanelArt,
    Package,
    RecordingsFolder,
}

impl Purpose {
    fn place(self) -> Place {
        match self {
            Purpose::PatchLoad | Purpose::PatchSave => Place::Patches,
            Purpose::PanelArt | Purpose::Package => Place::Art,
            Purpose::RecordingsFolder => Place::Recordings,
        }
    }
}

/// One row: a folder, or a file of the wanted kind.
struct Entry {
    name: String,
    dir: bool,
}

/// A folder's rows (and whether the list was cut), or why it could not be read.
type Listing = Result<(Vec<Entry>, bool), String>;

/// What a frame of the browser did.
#[derive(Debug, PartialEq)]
pub enum Event {
    None,
    Pick(PathBuf),
    Close,
}

pub struct FileBrowser {
    /// Prefix of the hit-test keys, so each user keeps its own (`wt:dir:Tables`).
    prefix: &'static str,
    mode: Mode,
    spec: Spec,
    pub dir: PathBuf,
    rx: Option<mpsc::Receiver<Listing>>,
    listing: Option<Listing>,
    /// The file or folder name typed in Save mode.
    pub name: String,
    /// A Save target that exists, waiting for the answer to "Replace it?".
    confirm: Option<PathBuf>,
    error: Option<String>,
}

impl FileBrowser {
    pub fn new(prefix: &'static str, mode: Mode, spec: Spec, dir: PathBuf, name: String) -> Self {
        let mut b = FileBrowser {
            prefix,
            mode,
            spec,
            dir: PathBuf::new(),
            rx: None,
            listing: None,
            name,
            confirm: None,
            error: None,
        };
        b.go(dir);
        b
    }

    /// Shows `dir`, listed on a thread of its own.
    fn go(&mut self, dir: PathBuf) {
        let (tx, rx) = mpsc::channel();
        let (there, spec, folders_only) = (dir.clone(), self.spec, self.mode == Mode::Folder);
        std::thread::spawn(move || {
            let _ = tx.send(list(&there, spec, folders_only));
        });
        self.dir = dir;
        self.rx = Some(rx);
        self.listing = None;
        self.confirm = None;
    }

    fn poll(&mut self) {
        let Some(rx) = &self.rx else { return };
        match rx.try_recv() {
            Ok(l) => self.listing = Some(l),
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.listing = Some(Err("the folder could not be read".into()))
            }
        }
        self.rx = None;
    }

    /// `folder/name` for Save: the name with the first wanted extension added when it has none.
    fn target(&self) -> Result<PathBuf, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("Type a name first".into());
        }
        if name == "." || name == ".." || name.contains(['/', '\\']) {
            return Err("A name has no folders in it: browse to the folder instead".into());
        }
        let wanted = self.spec.exts.first();
        let has = Path::new(name).extension().is_some_and(|e| {
            self.spec
                .exts
                .iter()
                .any(|x| e.to_string_lossy().eq_ignore_ascii_case(x))
        });
        Ok(match wanted {
            Some(ext) if !has => self.dir.join(format!("{name}.{ext}")),
            _ => self.dir.join(name),
        })
    }

    /// Draws the browser in `ui` and reports what the user did.
    pub fn show(&mut self, ui: &mut egui::Ui, st: &Style, ui_state: &mut UiState) -> Event {
        self.poll();
        if self.rx.is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(30));
        }
        let prefix = self.prefix;
        let key = |k: &str| format!("{prefix}:{k}");
        let (mut go, mut pick, mut close) = (None::<PathBuf>, None::<PathBuf>, false);
        let home = home();
        ui.horizontal(|ui| {
            let mut small = |ui: &mut egui::Ui, k: &str, text: &str, on: bool| {
                let r = ui.add(Button::new(st, text).small().enabled(on));
                ui_state.record(key(k), r.rect);
                r.clicked()
            };
            if small(ui, "up", "Up", self.dir.parent().is_some()) {
                go = self.dir.parent().map(Path::to_path_buf);
            }
            if small(ui, "home", "Home", home.is_some()) {
                go = home.clone();
            }
            if small(ui, "browse-close", "Close", true) {
                close = true;
            }
            kit::label_truncated(
                ui,
                st,
                Role::Caption,
                Tone::Text2,
                self.dir.display().to_string(),
            );
        });
        egui::ScrollArea::vertical()
            .id_salt((prefix, "browse"))
            .max_height(280.0)
            .auto_shrink([false, false])
            .show(ui, |ui| match &self.listing {
                None => {
                    kit::label(ui, st, Role::Body, Tone::Text2, "Reading the folder...");
                }
                Some(Err(e)) => {
                    kit::label(ui, st, Role::Body, Tone::Bad, e);
                }
                Some(Ok((rows, cut))) => {
                    if rows.is_empty() {
                        let none = if self.spec.exts.is_empty() {
                            "No folders here.".to_string()
                        } else {
                            format!("No folders or {} here.", self.spec.what)
                        };
                        kit::label(ui, st, Role::Body, Tone::Text2, none);
                    }
                    for e in rows {
                        let text = if e.dir {
                            format!("{}/", e.name)
                        } else {
                            e.name.clone()
                        };
                        let r = kit::menu_item(ui, st, &text, false);
                        let k = if e.dir { "dir" } else { "file" };
                        ui_state.record(key(&format!("{k}:{}", e.name)), r.rect);
                        if r.clicked() {
                            if e.dir {
                                go = Some(self.dir.join(&e.name));
                            } else if self.mode == Mode::Save {
                                self.name.clone_from(&e.name);
                                self.confirm = None;
                            } else {
                                pick = Some(self.dir.join(&e.name));
                            }
                        }
                    }
                    if *cut {
                        kit::label(
                            ui,
                            st,
                            Role::Caption,
                            Tone::Warn,
                            "The list is cut: type the path instead.",
                        );
                    }
                }
            });
        match self.mode {
            Mode::Folder => {
                ui.horizontal(|ui| {
                    let r = ui.add(Button::new(st, "Choose this folder").primary().small());
                    ui_state.record(key("choose"), r.rect);
                    if r.clicked() {
                        pick = Some(self.dir.clone());
                    }
                });
            }
            Mode::Save => self.save_row(ui, st, ui_state, &mut pick),
            Mode::Open => {}
        }
        if let Some(e) = &self.error {
            kit::label(ui, st, Role::Caption, Tone::Bad, e);
        }
        if let Some(dir) = go {
            self.error = None;
            self.go(dir);
            Event::None
        } else if let Some(path) = pick {
            Event::Pick(path)
        } else if close {
            Event::Close
        } else {
            Event::None
        }
    }

    /// The name field, Save, and the overwrite question.
    fn save_row(
        &mut self,
        ui: &mut egui::Ui,
        st: &Style,
        ui_state: &mut UiState,
        pick: &mut Option<PathBuf>,
    ) {
        let key = |k: &str| format!("{}:{k}", self.prefix);
        if let Some(path) = self.confirm.clone() {
            kit::label(
                ui,
                st,
                Role::Body,
                Tone::Warn,
                format!(
                    "{} exists. Replace it?",
                    path.file_name()
                        .map_or(String::new(), |n| n.to_string_lossy().into_owned())
                ),
            );
            ui.horizontal(|ui| {
                let r = ui.add(Button::new(st, "Replace").primary().small());
                ui_state.record(key("replace"), r.rect);
                if r.clicked() {
                    *pick = Some(path.clone());
                }
                let r = ui.add(Button::new(st, "Cancel").ghost().small());
                ui_state.record(key("replace-cancel"), r.rect);
                if r.clicked() {
                    self.confirm = None;
                }
            });
            return;
        }
        let mut submit = false;
        ui.horizontal(|ui| {
            let r = kit::field(ui, st, &mut self.name, "Name", None, Some(260.0));
            ui_state.record(key("name"), r.rect);
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                submit = true;
            }
            let r = ui.add(Button::new(st, "Save").primary().small());
            ui_state.record(key("save"), r.rect);
            submit |= r.clicked();
        });
        if submit {
            match self.target() {
                Err(e) => self.error = Some(e),
                Ok(path) if path.exists() => {
                    self.error = None;
                    self.confirm = Some(path);
                }
                Ok(path) => *pick = Some(path),
            }
        }
    }
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Folders first, then the wanted files, each by name without regard to case; hidden names left
/// out. `folders_only` hides the files whatever `spec` says.
fn list(dir: &Path, spec: Spec, folders_only: bool) -> Listing {
    let mut rows = Vec::new();
    let mut cut = false;
    for (seen, entry) in std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .enumerate()
    {
        if seen >= MAX_SCANNED {
            cut = true;
            break;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let Ok(meta) = std::fs::metadata(entry.path()) else {
            continue;
        };
        let wanted = !folders_only
            && meta.is_file()
            && Path::new(&name).extension().is_some_and(|e| {
                spec.exts
                    .iter()
                    .any(|x| e.to_string_lossy().eq_ignore_ascii_case(x))
            });
        if meta.is_dir() || wanted {
            if rows.len() >= MAX_ENTRIES {
                cut = true;
                break;
            }
            rows.push(Entry {
                name,
                dir: meta.is_dir(),
            });
        }
    }
    rows.sort_by_key(|e| (!e.dir, e.name.to_lowercase()));
    Ok((rows, cut))
}

/// The folder a use starts in: the remembered one if it still is a folder, else the folder of
/// `hint` (a path field's text), else home, else the root.
pub fn start_dir(ui_state: &UiState, place: Place, hint: &str) -> PathBuf {
    ui_state
        .folders
        .get(place.key())
        .filter(|d| d.is_dir())
        .cloned()
        .or_else(|| {
            let h = Path::new(hint.trim());
            let dir = if h.is_dir() { Some(h) } else { h.parent() };
            dir.filter(|d| !d.as_os_str().is_empty() && d.is_dir())
                .map(Path::to_path_buf)
        })
        .or_else(home)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// A browser asked for by a window that has no room of its own for it.
pub struct Request {
    purpose: Purpose,
    title: &'static str,
    browser: FileBrowser,
}

/// The pick of a `Request`, for the place that asked.
pub struct Picked {
    purpose: Purpose,
    path: PathBuf,
    /// Frames it has waited for its owner.
    age: u8,
}

/// Opens the browser window for `purpose` (replacing one that is open).
pub fn open(
    ui_state: &mut UiState,
    purpose: Purpose,
    title: &'static str,
    mode: Mode,
    spec: Spec,
    name: &str,
    hint: &str,
) {
    let dir = start_dir(ui_state, purpose.place(), hint);
    ui_state.files = Some(Request {
        purpose,
        title,
        browser: FileBrowser::new("fb", mode, spec, dir, name.into()),
    });
}

/// Whether the browser window is open for `purpose`.
pub fn is_open(ui_state: &UiState, purpose: Purpose) -> bool {
    ui_state
        .files
        .as_ref()
        .is_some_and(|r| r.purpose == purpose)
}

/// What the browser picked for `purpose`, once.
pub fn take(ui_state: &mut UiState, purpose: Purpose) -> Option<PathBuf> {
    if ui_state
        .picked
        .as_ref()
        .is_some_and(|p| p.purpose == purpose)
    {
        ui_state.picked.take().map(|p| p.path)
    } else {
        None
    }
}

/// Draws the browser window of an open `Request`. Called once a frame.
pub fn window(ui_state: &mut UiState, ctx: &egui::Context) {
    if let Some(p) = &mut ui_state.picked {
        // The place that asked reads it within a few frames of the pick; later it is stale.
        if p.age >= MAX_PICK_AGE {
            ui_state.picked = None;
        } else {
            p.age += 1;
        }
    }
    let Some(mut req) = ui_state.files.take() else {
        return;
    };
    let st = ui_state.style.clone();
    let below_toolbar = ui_state
        .hits
        .get("rack")
        .map_or(56.0, |r| r.bottom() + 10.0);
    let mut open = true;
    let mut event = Event::None;
    egui::Window::new(req.title)
        .id(egui::Id::new("file-browser"))
        .default_pos(pos2(60.0, below_toolbar + 20.0))
        .default_width(460.0)
        .open(&mut open)
        .frame(kit::popover_frame(&st))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            event = req.browser.show(ui, &st, ui_state);
        });
    ui_state.folders_remember(req.purpose.place(), &req.browser.dir);
    match event {
        Event::Pick(path) => {
            // A patch is a folder: remember the folder it was in, not the patch itself.
            if req.purpose == Purpose::PatchLoad {
                if let Some(parent) = path.parent() {
                    ui_state.folders_remember(Place::Patches, parent);
                }
            }
            ui_state.picked = Some(Picked {
                purpose: req.purpose,
                path,
                age: 0,
            });
        }
        Event::Close => {}
        Event::None if open => ui_state.files = Some(req),
        Event::None => {}
    }
}
