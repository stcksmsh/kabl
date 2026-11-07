//! The shared file browser: a folder list drawn with the chrome kit, used wherever the
//! application asks for a file or a folder (wavetable import, loading and saving a patch folder,
//! panel art, composite packages, the recordings folder). It is not a native dialog, so it looks
//! and behaves the same in the standalone and inside a plugin editor, and cannot hide behind a
//! host window. A folder is listed on a thread of its own (a slow share never holds the
//! interface); a listing is capped and says so. Control thread only.
//!
//! A window using it keeps its own path field as the fallback; the browser only fills it in.

use crate::kit::{self, g4, Button, Ic, Tone};
use crate::style::{Font, Role, Style};
use crate::UiState;
use egui::{pos2, vec2, Align2, FontId, Rect, Response, Sense, Stroke};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

/// Rows one listing shows (a folder with more is cut, and says so).
const MAX_ENTRIES: usize = 2000;
/// Directory entries one listing looks at before it gives up (a folder of millions of files).
const MAX_SCANNED: usize = 100_000;

/// Frames a pick waits for the place that asked before it is dropped.
const MAX_PICK_AGE: u8 = 60;

/// Width of the places column.
const SIDEBAR_W: f32 = 164.0;
/// Height of a list row; rows sit 2 px apart.
const ROW_H: f32 = 32.0;

/// What a use of the browser shows besides folders: files with one of `exts` (lower case, no
/// dot); none means folders only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spec {
    pub exts: &'static [&'static str],
    /// What the files are called in the empty-folder message.
    pub what: &'static str,
    /// Lists `.kabl` files as patches (picked whole); folders are only folders.
    pub patches: bool,
}

pub const WAV: Spec = Spec {
    exts: &["wav"],
    what: ".wav files",
    patches: false,
};
pub const IMAGE: Spec = Spec {
    exts: &["png"],
    what: ".png images",
    patches: false,
};
pub const PACKAGE: Spec = Spec {
    exts: &["json"],
    what: ".json packages",
    patches: false,
};
pub const FOLDERS: Spec = Spec {
    exts: &[],
    what: "files",
    patches: false,
};

pub const PATCHES: Spec = Spec {
    exts: &[],
    what: "kabl patches (.kabl)",
    patches: true,
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
    /// A `.kabl` patch file.
    patch: bool,
    /// Seconds since 1970 of the last change, when the system says.
    modified: Option<u64>,
    /// Recent list only: where the patch is (it is not in the folder shown).
    full: Option<PathBuf>,
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
    /// Folders left by browsing, newest last (Back).
    back: Vec<PathBuf>,
    rx: Option<mpsc::Receiver<Listing>>,
    listing: Option<Listing>,
    /// Text of the Filter field: only names containing it are listed (any case).
    filter: String,
    /// The file or folder name typed in Save mode.
    pub name: String,
    /// A Save target that exists, waiting for the answer to "Replace it?".
    confirm: Option<PathBuf>,
    error: Option<String>,
    /// The patch picked in Open mode, waiting for the Open button.
    selected: Option<String>,
    /// The Recent place is shown instead of the folder.
    recent: bool,
    /// Save mode: the Name field takes focus on its first frame.
    focus_name: bool,
    /// Height of the places column and the list.
    list_h: f32,
    /// How far the places column and the rule above it reach past `ui` to the frame's edges
    /// (the modal's body margin; 0 when the browser sits inside another panel).
    bleed: f32,
}

impl FileBrowser {
    pub fn new(prefix: &'static str, mode: Mode, spec: Spec, dir: PathBuf, name: String) -> Self {
        let mut b = FileBrowser {
            prefix,
            mode,
            spec,
            dir: PathBuf::new(),
            back: Vec::new(),
            rx: None,
            listing: None,
            filter: String::new(),
            name,
            confirm: None,
            error: None,
            selected: None,
            recent: false,
            focus_name: mode == Mode::Save,
            list_h: 300.0,
            bleed: 0.0,
        };
        b.go(dir);
        b
    }

    /// Shows `dir`; the folder left is kept for Back.
    fn go(&mut self, dir: PathBuf) {
        if !self.dir.as_os_str().is_empty() && self.dir != dir {
            self.back.push(std::mem::take(&mut self.dir));
        }
        self.show_dir(dir);
    }

    /// Shows `dir`, listed on a thread of its own.
    fn show_dir(&mut self, dir: PathBuf) {
        let (tx, rx) = mpsc::channel();
        let (there, spec, folders_only) = (dir.clone(), self.spec, self.mode == Mode::Folder);
        std::thread::spawn(move || {
            let _ = tx.send(list(&there, spec, folders_only));
        });
        self.dir = dir;
        self.recent = false;
        self.rx = Some(rx);
        self.listing = None;
        self.confirm = None;
        self.error = None;
        self.selected = None;
        self.filter.clear();
    }

    /// Shows the recent patches of the library (in place of a folder listing).
    fn show_recent(&mut self, ui_state: &UiState) {
        let rows = ui_state
            .library
            .as_ref()
            .map_or_else(Vec::new, |l| l.recent_patches());
        let rows = rows
            .into_iter()
            .map(|p| Entry {
                name: p
                    .file_name()
                    .map_or(String::new(), |n| n.to_string_lossy().into_owned()),
                dir: p.is_dir(),
                patch: true,
                modified: std::fs::metadata(&p)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs()),
                full: Some(p),
            })
            .collect();
        self.rx = None;
        self.listing = Some(Ok((rows, false)));
        self.recent = true;
        self.confirm = None;
        self.error = None;
        self.selected = None;
        self.filter.clear();
    }

    /// The path of the row called `name`.
    fn path_of(&self, name: &str) -> PathBuf {
        if let Some(Ok((rows, _))) = &self.listing {
            if let Some(p) = rows
                .iter()
                .find(|e| e.name == name)
                .and_then(|e| e.full.clone())
            {
                return p;
            }
        }
        self.dir.join(name)
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
        // A patch is saved as a `.kabl` file.
        let exts: &[&str] = if self.spec.patches {
            &["kabl"]
        } else {
            self.spec.exts
        };
        let has = Path::new(name).extension().is_some_and(|e| {
            exts.iter()
                .any(|x| e.to_string_lossy().eq_ignore_ascii_case(x))
        });
        Ok(match exts.first() {
            Some(ext) if !has => self.dir.join(format!("{name}.{ext}")),
            _ => self.dir.join(name),
        })
    }

    /// Draws the toolbar (Back, Up, Home, breadcrumb, Filter), the places column and the list
    /// in `ui`, and reports what the user did. The actions are [`FileBrowser::footer`]'s.
    pub fn show(&mut self, ui: &mut egui::Ui, st: &Style, ui_state: &mut UiState) -> Event {
        self.poll();
        if self.rx.is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(30));
        }
        let prefix = self.prefix;
        let key = |k: &str| format!("{prefix}:{k}");
        let (mut go, mut back, mut pick, mut close) = (None::<PathBuf>, false, None, false);
        let home = home();
        ui.horizontal(|ui| {
            ui.set_height(32.0);
            let mut ibtn = |ui: &mut egui::Ui, k: &str, ic: Ic, on: bool| {
                let r = kit::icon_button(ui, st, ic, false, on);
                ui_state.record(key(k), r.rect);
                r.clicked()
            };
            back = ibtn(ui, "back", Ic::Back, !self.back.is_empty());
            if ibtn(ui, "up", Ic::Up, self.dir.parent().is_some()) {
                go = self.dir.parent().map(Path::to_path_buf);
            }
            if ibtn(ui, "home", Ic::Home, home.is_some()) {
                go = home.clone();
            }
            ui.add_space(8.0);
            // Inside another panel there is no footer: a close button ends the row.
            let inline = self.bleed == 0.0;
            let crumbs_w = (ui.available_width() - if inline { 252.0 } else { 212.0 }).max(60.0);
            if self.recent {
                // Recent is a list, not a folder: one crumb, nothing to click.
                let text = egui::RichText::new("Recent")
                    .font(semibold(st, 13.0))
                    .color(st.roles.text);
                ui.add(egui::Label::new(text));
            } else if let Some(d) = breadcrumb(ui, st, &self.dir, home.as_deref(), crumbs_w) {
                go = Some(d);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if inline {
                    let r = kit::icon_button(ui, st, Ic::Close, false, true);
                    ui_state.record(key("browse-close"), r.rect);
                    close = r.clicked();
                }
                let r = kit::field(
                    ui,
                    st,
                    &mut self.filter,
                    "Filter",
                    Some(Ic::Search),
                    Some(200.0),
                );
                ui_state.record(key("filter"), r.rect);
            });
        });
        ui.add_space(14.0);
        // Save's question and errors sit above the list, which gives up their height.
        let mut list_h = self.list_h;
        if let Some(path) = &self.confirm {
            let name = path
                .file_name()
                .map_or(String::new(), |n| n.to_string_lossy().into_owned());
            g4::banner(
                ui,
                st,
                g4::Sev::Warn,
                &format!("{name} exists. Replace it?"),
                None,
            );
            list_h -= 44.0;
        } else if let Some(e) = &self.error {
            g4::banner(ui, st, g4::Sev::Bad, e, None);
            list_h -= 44.0;
        }
        let (area, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), list_h.max(96.0)), Sense::hover());
        let r = &st.roles;
        let side = Rect::from_min_max(
            pos2(area.left() - self.bleed, area.top()),
            // Down to the footer's rule: the modal's 4 px body margin and 12 px gap.
            pos2(
                area.left() - self.bleed + SIDEBAR_W,
                area.bottom() + if self.bleed > 0.0 { 16.0 } else { 0.0 },
            ),
        );
        {
            let p = ui.painter();
            p.rect_filled(side, 0.0, r.bar);
            p.vline(side.right(), side.y_range(), Stroke::new(1.0, r.line));
            p.hline(
                (area.left() - self.bleed)..=(area.right() + self.bleed),
                area.top(),
                Stroke::new(1.0, r.line),
            );
        }
        if let Some(d) = places(
            ui,
            st,
            ui_state,
            prefix,
            side,
            (&self.dir, self.recent, self.spec.patches),
            home,
        ) {
            if d.as_os_str().is_empty() {
                self.show_recent(ui_state);
            } else {
                go = Some(d);
            }
        }
        // The list: Name / Modified header, then the rows.
        let list = Rect::from_min_max(pos2(side.right() + 1.0, area.top() + 1.0), area.max);
        let date_x = list.right() - 130.0;
        let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(list.shrink2(vec2(8.0, 0.0))));
        let ui = &mut inner;
        let filter = self.filter.trim().to_lowercase();
        match &self.listing {
            None => {
                ui.add_space(14.0);
                kit::label(ui, st, Role::Body, Tone::Text2, "Reading the folder...");
            }
            Some(Err(e)) => {
                ui.add_space(14.0);
                kit::label(ui, st, Role::Body, Tone::Bad, e);
            }
            Some(Ok((rows, _))) if rows.is_empty() => {
                ui.add_space(((list.height() - 204.0) / 2.0).max(0.0));
                let (title, line) = if self.spec.patches {
                    (
                        "No patches here".to_string(),
                        "This folder has no folders or patches.".to_string(),
                    )
                } else if self.spec.exts.is_empty() {
                    (
                        "No folders here".to_string(),
                        "This folder has no folders in it.".to_string(),
                    )
                } else {
                    (
                        format!("No {} here", self.spec.what),
                        format!("This folder has no folders or {}.", self.spec.what),
                    )
                };
                let acts: Vec<g4::EmptyAction> = self
                    .dir
                    .parent()
                    .map(|_| g4::EmptyAction {
                        label: "Go up",
                        primary: false,
                        icon: Ic::Up,
                    })
                    .into_iter()
                    .collect();
                let rs = g4::empty_state(ui, st, Ic::Folder, &title, &line, &acts, None);
                if let Some(r) = rs.first() {
                    ui_state.record(key("go-up"), r.rect);
                    if r.clicked() {
                        go = self.dir.parent().map(Path::to_path_buf);
                    }
                }
            }
            Some(Ok((rows, cut))) => {
                let lab = semibold(st, 11.0);
                let y = list.top() + 22.0;
                let p = ui.painter();
                p.text(
                    pos2(list.left() + 44.0, y),
                    Align2::LEFT_CENTER,
                    "NAME",
                    lab.clone(),
                    r.text3,
                );
                p.text(
                    pos2(date_x, y),
                    Align2::LEFT_CENTER,
                    "MODIFIED",
                    lab,
                    r.text3,
                );
                ui.add_space(36.0);
                egui::ScrollArea::vertical()
                    .id_salt((prefix, "browse"))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        let mut shown = 0;
                        for e in rows.iter().filter(|e| {
                            filter.is_empty() || e.name.to_lowercase().contains(&filter)
                        }) {
                            shown += 1;
                            let item = !e.dir || e.patch;
                            let selected = match self.mode {
                                Mode::Save => {
                                    item && (e.name == self.name.trim()
                                        || self.selected.as_deref() == Some(&e.name))
                                }
                                _ => e.patch && self.selected.as_deref() == Some(&e.name),
                            };
                            let resp = row(ui, st, e, selected, date_x);
                            let k = if e.patch {
                                "patch"
                            } else if e.dir {
                                "dir"
                            } else {
                                "file"
                            };
                            ui_state.record(key(&format!("{k}:{}", e.name)), resp.rect);
                            if resp.double_clicked() && e.patch && self.mode == Mode::Open {
                                pick =
                                    Some(e.full.clone().unwrap_or_else(|| self.dir.join(&e.name)));
                            } else if resp.clicked() {
                                if e.dir && !e.patch {
                                    go = Some(self.dir.join(&e.name));
                                } else if self.mode == Mode::Save {
                                    self.name.clone_from(&e.name);
                                    self.confirm = None;
                                } else if e.patch {
                                    self.selected = Some(e.name.clone());
                                } else {
                                    pick = Some(self.dir.join(&e.name));
                                }
                            }
                        }
                        if shown == 0 {
                            ui.add_space(8.0);
                            kit::label(
                                ui,
                                st,
                                Role::Body,
                                Tone::Text2,
                                format!(
                                    "Nothing here matches \u{201c}{}\u{201d}.",
                                    self.filter.trim()
                                ),
                            );
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
                    });
            }
        }
        if back {
            if let Some(d) = self.back.pop() {
                self.show_dir(d);
            }
            Event::None
        } else if let Some(dir) = go {
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

    /// The footer's actions, for a right-to-left row (primary first, so it lands rightmost):
    /// Cancel and Choose this folder, Cancel and Save with the Name field, or Cancel and Replace
    /// for a Save target that exists. A mono caption on the left says what is listed.
    pub fn footer(&mut self, ui: &mut egui::Ui, st: &Style, ui_state: &mut UiState) -> Event {
        let prefix = self.prefix;
        let key = |k: &str| format!("{prefix}:{k}");
        let mut button =
            |ui: &mut egui::Ui, k: &str, text: &str, primary: bool, w: f32, on: bool| {
                let b = Button::new(st, text).g4(32.0).min_width(w).enabled(on);
                let r = ui.add(if primary { b.primary() } else { b });
                ui_state.record(key(k), r.rect);
                r.clicked()
            };
        if let Some(path) = self.confirm.clone() {
            if button(ui, "replace", "Replace", true, 116.0, true) {
                return Event::Pick(path);
            }
            if button(ui, "replace-cancel", "Cancel", false, 100.0, true) {
                self.confirm = None;
            }
            return Event::None;
        }
        let mut event = Event::None;
        match self.mode {
            Mode::Folder => {
                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter))
                    && ui.memory(|m| m.focused().is_none());
                if button(ui, "choose", "Choose this folder", true, 116.0, true) || enter {
                    event = Event::Pick(self.dir.clone());
                }
            }
            Mode::Save => {
                let mut submit = button(ui, "save", "Save", true, 116.0, true);
                if button(ui, "browse-close", "Cancel", false, 100.0, true) {
                    return Event::Close;
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    kit::label(ui, st, Role::Body, Tone::Text2, "Name");
                    ui.add_space(8.0);
                    let suffix = if self.spec.patches {
                        Some(".kabl".to_string())
                    } else {
                        self.spec.exts.first().map(|e| format!(".{e}"))
                    };
                    let w = ui.available_width() - 16.0;
                    let r = kit::field(ui, st, &mut self.name, "Name", None, Some(w.max(80.0)));
                    ui_state.record(key("name"), r.rect);
                    if std::mem::take(&mut self.focus_name) {
                        r.request_focus();
                    }
                    submit |= r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if let Some(s) = suffix {
                        // Dim, inside the field's right edge.
                        ui.painter().text(
                            pos2(r.rect.right() - 12.0, r.rect.center().y),
                            Align2::RIGHT_CENTER,
                            s,
                            st.font(Role::Value),
                            st.roles.text3,
                        );
                    }
                });
                if submit {
                    match self.target() {
                        Err(e) => self.error = Some(e),
                        Ok(path) if path.exists() => {
                            self.error = None;
                            self.confirm = Some(path);
                        }
                        Ok(path) => event = Event::Pick(path),
                    }
                }
                return event;
            }
            Mode::Open if self.spec.patches => {
                let chosen = self.selected.as_ref().map(|n| self.path_of(n));
                let r = button(ui, "open", "Open", true, 100.0, chosen.is_some());
                if r {
                    if let Some(p) = chosen {
                        event = Event::Pick(p);
                    }
                }
            }
            Mode::Open => {}
        }
        if button(ui, "browse-close", "Cancel", false, 100.0, true) {
            return Event::Close;
        }
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            let what = if self.mode == Mode::Folder {
                "Folders"
            } else {
                self.spec.what
            };
            kit::label(ui, st, Role::Value, Tone::Text3, what);
        });
        event
    }
}

/// The places column inside `side`: home, then the remembered folder of each use that still
/// exists; the one shown is marked. Returns the folder of a clicked place.
fn places(
    ui: &mut egui::Ui,
    st: &Style,
    ui_state: &mut UiState,
    prefix: &str,
    side: Rect,
    (dir, recent, patches): (&Path, bool, bool),
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    let mut list: Vec<(&str, &str, Ic, PathBuf)> = Vec::new();
    if let Some(h) = home {
        list.push(("home", "Home", Ic::Home, h));
    }
    // Sounds: the user's own sound library.
    let sounds = ui_state
        .library
        .as_ref()
        .map(|l| l.user_root.join("sounds"))
        .filter(|d| d.is_dir());
    for (place, label, ic) in [
        (Place::Patches, "Patches", Ic::Folder),
        (Place::Wavetables, "Wavetables", Ic::Wave),
        (Place::Recordings, "Recordings", Ic::Rec),
        (Place::Art, "Artwork", Ic::Panel),
    ] {
        if let Some(d) = ui_state.folders.get(place.key()).filter(|d| d.is_dir()) {
            list.push((place.key(), label, ic, d.clone()));
        }
    }
    if let Some(d) = sounds {
        list.push(("sounds", "Sounds", Ic::Star, d));
    }
    // Recent patches, last: an empty path stands for the list.
    if patches {
        list.push(("recent", "Recent", Ic::Clock, PathBuf::new()));
    }
    let r = &st.roles;
    let mut go = None;
    for (i, (k, label, ic, d)) in list.into_iter().enumerate() {
        let row = Rect::from_min_size(
            pos2(side.left() + 8.0, side.top() + 12.0 + i as f32 * 34.0),
            vec2(SIDEBAR_W - 16.0, 30.0),
        );
        if row.bottom() > side.bottom() {
            break;
        }
        let on = if d.as_os_str().is_empty() {
            recent
        } else {
            !recent && d == dir
        };
        let resp = ui.interact(row, ui.id().with((prefix, "place", i)), Sense::click());
        ui_state.record(format!("{prefix}:place:{k}"), row);
        let p = ui.painter();
        if on || resp.hovered() {
            p.rect_filled(row, 6.0, if st.dark { r.raised } else { r.surface });
        }
        if on {
            p.rect_stroke(row, 6.0, Stroke::new(1.0, r.line), egui::StrokeKind::Inside);
        }
        let col = if on { r.accent_text } else { r.text2 };
        kit::icon(
            p,
            ic,
            pos2(row.left() + 17.0, row.center().y),
            st.metrics.icon,
            col,
        );
        let font = if on {
            semibold(st, 13.0)
        } else {
            regular(st, 13.0)
        };
        let col = if on { r.text } else { r.text2 };
        p.text(
            pos2(row.left() + 34.0, row.center().y),
            Align2::LEFT_CENTER,
            label,
            font,
            col,
        );
        if resp.clicked() {
            go = Some(d);
        }
    }
    go
}

/// The folder as clickable crumbs (Home for the home folder), cut from the front with an
/// ellipsis to fit `width`; the last crumb is semibold. Returns the folder of a clicked crumb.
fn breadcrumb(
    ui: &mut egui::Ui,
    st: &Style,
    dir: &Path,
    home: Option<&Path>,
    width: f32,
) -> Option<PathBuf> {
    let (mut at, rest, mut crumbs) = match home.and_then(|h| Some((h, dir.strip_prefix(h).ok()?))) {
        Some((h, rest)) => (
            h.to_path_buf(),
            rest,
            vec![("Home".to_string(), h.to_path_buf())],
        ),
        None => (PathBuf::new(), dir, Vec::new()),
    };
    for c in rest.components() {
        at.push(c);
        let s = c.as_os_str().to_string_lossy().into_owned();
        crumbs.push((if s.is_empty() { "/".into() } else { s }, at.clone()));
    }
    let r = &st.roles;
    let last = crumbs.len().saturating_sub(1);
    let galleys: Vec<_> = crumbs
        .iter()
        .enumerate()
        .map(|(i, (s, _))| {
            let (f, c) = if i == last {
                (semibold(st, 13.0), r.text)
            } else {
                (regular(st, 13.0), r.text2)
            };
            ui.painter().layout_no_wrap(s.clone(), f, c)
        })
        .collect();
    // 20 px per chevron; drop crumbs from the front (34 px for the ellipsis) until the rest fit.
    let fits = |from: usize| {
        let ellipsis = if from > 0 { 34.0 } else { 0.0 };
        galleys[from..]
            .iter()
            .map(|g| g.size().x + 20.0)
            .sum::<f32>()
            + ellipsis
            <= width
    };
    let mut first = 0;
    while first < last && !fits(first) {
        first += 1;
    }
    let (rect, _) = ui.allocate_exact_size(vec2(width, 32.0), Sense::hover());
    let p = ui.painter().with_clip_rect(rect);
    let (mut x, cy) = (rect.left(), rect.center().y);
    if first > 0 {
        p.text(
            pos2(x, cy),
            Align2::LEFT_CENTER,
            "\u{2026}",
            regular(st, 13.0),
            r.text3,
        );
        kit::icon(&p, Ic::Right, pos2(x + 20.0, cy), 12.0, r.text3);
        x += 34.0;
    }
    let mut clicked = None;
    for (i, g) in galleys.into_iter().enumerate().skip(first) {
        let w = g.size().x;
        let hit = Rect::from_min_size(pos2(x, cy - 12.0), vec2(w, 24.0)).intersect(rect);
        let resp = ui.interact(hit, ui.id().with(("crumb", i)), Sense::click());
        if i < last && resp.clicked() {
            clicked = Some(crumbs[i].1.clone());
        }
        p.galley(pos2(x, cy - g.size().y / 2.0), g, r.text);
        x += w + 6.0;
        if i < last {
            kit::icon(&p, Ic::Right, pos2(x + 4.0, cy), 12.0, r.text3);
            x += 14.0;
        }
    }
    clicked
}

fn regular(st: &Style, size: f32) -> FontId {
    FontId::new(size, Font::Sans.family(st.fonts_ready))
}

fn semibold(st: &Style, size: f32) -> FontId {
    FontId::new(size, Font::SansSemibold.family(st.fonts_ready))
}

/// One 32 px list row: folder (accent icon, chevron) or file, name, modified date in mono.
fn row(ui: &mut egui::Ui, st: &Style, e: &Entry, selected: bool, date_x: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    let r = &st.roles;
    let p = ui.painter();
    if selected || resp.hovered() {
        p.rect_filled(rect, 6.0, if st.dark { r.raised } else { r.surface });
    }
    if selected {
        p.rect_stroke(
            rect,
            6.0,
            Stroke::new(1.5, r.accent),
            egui::StrokeKind::Inside,
        );
    }
    let cy = rect.center().y;
    let folder = e.dir && !e.patch;
    let (ic, col) = if folder {
        (Ic::Folder, r.accent_text)
    } else {
        (Ic::File, r.text2)
    };
    kit::icon(p, ic, pos2(rect.left() + 21.0, cy), st.metrics.icon, col);
    let font = if folder {
        FontId::new(13.0, Font::SansMedium.family(st.fonts_ready))
    } else {
        regular(st, 13.0)
    };
    let clip = Rect::from_min_max(
        pos2(rect.left() + 36.0, rect.top()),
        pos2((date_x - 16.0).max(rect.left() + 56.0), rect.bottom()),
    );
    let g = p.layout_no_wrap(e.name.clone(), font, r.text);
    p.with_clip_rect(clip)
        .galley(pos2(clip.left(), cy - g.size().y / 2.0), g, r.text);
    if let Some(secs) = e.modified.filter(|_| !folder) {
        // ponytail: UTC; local time needs time-zone data (a dependency).
        const MONTHS: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let (_, m, d) = crate::record::civil_date(secs);
        let day = secs % 86_400;
        let text = format!(
            "{d} {} {:02}:{:02}",
            MONTHS[(m - 1) as usize],
            day / 3600,
            day % 3600 / 60
        );
        p.text(
            pos2(date_x, cy),
            Align2::LEFT_CENTER,
            text,
            st.font(Role::Value),
            r.text3,
        );
    }
    if folder {
        kit::icon(
            p,
            Ic::Right,
            pos2(rect.right() - 16.0, cy),
            st.metrics.icon,
            r.text3,
        );
    }
    resp
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
        // A patch is a `.kabl` file; a folder is only a folder, whatever is inside it.
        let patch = spec.patches
            && !folders_only
            && meta.is_file()
            && crate::library::is_kabl_path(Path::new(&name));
        let wanted = patch
            || (!folders_only
                && meta.is_file()
                && Path::new(&name).extension().is_some_and(|e| {
                    spec.exts
                        .iter()
                        .any(|x| e.to_string_lossy().eq_ignore_ascii_case(x))
                }));
        if meta.is_dir() || wanted {
            if rows.len() >= MAX_ENTRIES {
                cut = true;
                break;
            }
            rows.push(Entry {
                name,
                dir: meta.is_dir(),
                patch,
                full: None,
                modified: meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs()),
            });
        }
    }
    // Real folders first, then files and patches.
    rows.sort_by_key(|e| (!e.dir || e.patch, e.name.to_lowercase()));
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
/// Marks `name` as the selected row of the open browser (captures use it).
pub fn preselect(ui_state: &mut UiState, name: &str) {
    if let Some(r) = &mut ui_state.files {
        r.browser.selected = Some(name.to_string());
    }
}

/// Shows the Recent place with `paths` as its patches (captures use it).
pub fn show_recent_for_capture(ui_state: &mut UiState, paths: &[PathBuf]) {
    let Some(r) = &mut ui_state.files else { return };
    let rows = paths
        .iter()
        .map(|p| Entry {
            name: p
                .file_name()
                .map_or(String::new(), |n| n.to_string_lossy().into_owned()),
            dir: p.is_dir(),
            patch: true,
            modified: std::fs::metadata(p)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs()),
            full: Some(p.clone()),
        })
        .collect();
    r.browser.rx = None;
    r.browser.listing = Some(Ok((rows, false)));
    r.browser.recent = true;
}

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

/// Draws the browser modal of an open `Request` (G4 modal L). Called once a frame.
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
    // At most 520 px tall and 110 px clear of the window; the list takes what the title,
    // toolbar and footer leave.
    let h = 520f32.min(ctx.content_rect().height() - 110.0);
    req.browser.list_h = (h - 182.0).max(160.0);
    req.browser.bleed = 24.0;
    let title = req.title;
    // Body and footer are separate closures over the same browser and state.
    let cell = std::cell::RefCell::new((&mut req.browser, &mut *ui_state, Event::None));
    let closed = g4::modal_framed(
        ctx,
        &st,
        "file-browser",
        g4::Size::L,
        title,
        |ui| {
            let (b, u, e) = &mut *cell.borrow_mut();
            let ev = b.show(ui, &st, u);
            if ev != Event::None {
                *e = ev;
            }
        },
        |_| {},
        |ui| {
            let (b, u, e) = &mut *cell.borrow_mut();
            let ev = b.footer(ui, &st, u);
            if ev != Event::None {
                *e = ev;
            }
        },
    );
    let event = cell.into_inner().2;
    ui_state.folders_remember(req.purpose.place(), &req.browser.dir);
    match event {
        Event::Pick(path) => {
            // A patch is a folder: remember the folder it was in, not the patch itself.
            if req.purpose == Purpose::PatchLoad {
                if let Some(parent) = path.parent() {
                    ui_state.folders_remember(Place::Patches, parent);
                }
            }
            if matches!(req.purpose, Purpose::PatchLoad | Purpose::PatchSave) {
                if let Some(l) = &mut ui_state.library {
                    // A failed save of the preferences only loses the list; nothing to tell.
                    let _ = l.touch_patch(&path);
                }
            }
            ui_state.picked = Some(Picked {
                purpose: req.purpose,
                path,
                age: 0,
            });
        }
        Event::Close => {}
        Event::None if !closed => ui_state.files = Some(req),
        Event::None => {}
    }
}
