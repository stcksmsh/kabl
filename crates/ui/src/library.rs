//! The sound library behind the browser: factory sounds shipped with kabl (read-only) and the
//! user's own sounds, both `.kabl` files. No `egui` here, so it is tested headlessly
//! (`tests/library.rs`). Folders are ignored: the app reads and writes only `.kabl`.
//!
//! - A sound is one `.kabl` file. Its metadata frame holds the `sound.toml` text (name,
//!   category, tags, description, how it plays); the op log stays the only musical state.
//! - Identity is `factory:<relative path>` or `user:<file name>` (no extension), never the
//!   display name, so a
//!   rename keeps favorites and recents, and two sounds may look alike without colliding.
//! - Favorites and recents (`library.json`) are library preferences: never in a patch, never
//!   undone.
//! - A user save writes a complete new file beside the old one, reads it back, then renames it
//!   over the old one.
//!
//! Locations: docs/find-play-save/README.md, "Where things live".

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use kabl_core::{PatchLog, PatchState};
use serde::{Deserialize, Serialize};

pub const META_FILE: &str = "sound.toml";
pub const PREFS_FILE: &str = "library.json";
pub const MAX_RECENTS: usize = 12;
/// Patch files the Recent place of the file browser remembers.
pub const MAX_PATCH_RECENTS: usize = 10;
pub const MAX_NAME: usize = 60;

/// The category vocabulary, in browser order.
/// The shelves of the Sounds scene's category list, in the mockup's order.
pub const SHELVES: [&str; 8] = [
    "Bass", "Lead", "Keys", "Pad", "Pluck", "Groove", "Piece", "Voice",
];

pub const CATEGORIES: &[&str] = &[
    "Sounds",
    "Sequences",
    "Performances",
    "Basic",
    "Bass",
    "Lead",
    "Keys",
    "Pad",
    "Pluck",
    "Strings",
    "Wind",
    "Percussion",
    "Groove",
    "Voice",
    "Piece",
    "Study",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Factory,
    User,
}

/// Browser metadata of one sound (`sound.toml`). Every field is optional in the file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Meta {
    pub name: String,
    pub category: String,
    pub tags: Vec<String>,
    pub description: String,
    /// Has a keyboard input (`midi.in`): the audition plays it.
    pub keys: bool,
    /// Has a clock: it runs a sequence (Start/Stop).
    pub sequence: bool,
}

impl Meta {
    /// Broad musical purpose, including metadata written before purpose filters existed.
    pub fn content_type(&self) -> &'static str {
        match (self.keys, self.sequence) {
            (true, true) => "Performances",
            (_, true) => "Sequences",
            _ => "Sounds",
        }
    }

    /// The category list of the Sounds scene: the mockup's eight shelves. Data categories
    /// outside it map to the nearest one; `None` (Basic and the purpose words) shows under All.
    pub fn shelf(&self) -> Option<&'static str> {
        Some(match self.category.as_str() {
            "Bass" => "Bass",
            "Lead" | "Wind" => "Lead",
            "Keys" => "Keys",
            "Pad" | "Strings" => "Pad",
            "Pluck" => "Pluck",
            "Groove" | "Percussion" => "Groove",
            "Piece" | "Study" => "Piece",
            "Voice" => "Voice",
            _ => return None,
        })
    }

    pub fn in_category(&self, category: &str) -> bool {
        self.category == category || self.content_type() == category
    }

    /// How a patch plays, read from its modules.
    pub fn play_of(state: &PatchState) -> (bool, bool) {
        let has = |k: &str| state.modules.values().any(|m| m.kind == k);
        (has("midi.in"), has("clock"))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub id: String,
    pub origin: Origin,
    pub dir: PathBuf,
    pub meta: Meta,
}

impl Entry {
    /// Case-insensitive match of every word of `query` against name, category, tags and
    /// description.
    pub fn matches(&self, query: &str) -> bool {
        let hay = format!(
            "{} {} {} {} {}",
            self.meta.name,
            self.meta.category,
            self.meta.tags.join(" "),
            self.meta.description,
            self.meta.content_type()
        )
        .to_lowercase();
        query
            .to_lowercase()
            .split_whitespace()
            .all(|w| hay.contains(w))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub favorites: BTreeSet<String>,
    /// Newest first.
    pub recents: Vec<String>,
    /// Paths of the patch files last opened or saved through the file browser, newest first.
    pub patch_recents: Vec<String>,
    /// Remembered answer of the custom-code notice per content hash (lower-case hex SHA-256):
    /// true = Open, false = Cancel.
    pub code_trust: std::collections::BTreeMap<String, bool>,
}

#[derive(Debug)]
pub enum LibError {
    /// The name is empty or too long.
    BadName(String),
    /// A user sound (this id) already has that name.
    NameTaken(String),
    /// Factory sounds are never written.
    Factory,
    /// A replacement names a sound whose name isn't the one being saved (a stale
    /// confirmation): nothing is written.
    ReplaceMismatch {
        target: String,
        name: String,
    },
    Io(String),
    /// A save failed half way and putting the previous files back failed too: they are kept
    /// aside and restored on the next start or open (the message says where).
    Interrupted(String),
    /// The patch files could not be read or parsed.
    Unreadable(String),
    /// The patch parsed but does not compile (unknown module, bad port, ...).
    Uncompilable(String),
    /// The patch carries code that has no remembered answer yet: ask first (the notice), then
    /// open again.
    CodeNotice(Vec<crate::code_notice::CodeEntry>),
    /// The patch carries code the user declined before. The answer is cleared so the next try
    /// asks again.
    CodeDeclined(Vec<crate::code_notice::CodeEntry>),
}

impl std::fmt::Display for LibError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LibError::BadName(why) => write!(f, "{why}"),
            LibError::NameTaken(_) => write!(f, "a sound with that name already exists"),
            LibError::Factory => write!(f, "factory sounds can't be overwritten: use Save As"),
            LibError::ReplaceMismatch { target, name } => write!(
                f,
                "\"{target}\" is not called \"{name}\", so it was not replaced"
            ),
            LibError::Io(e) | LibError::Interrupted(e) => write!(f, "{e}"),
            LibError::Unreadable(e) => write!(f, "can't read the patch: {e}"),
            LibError::Uncompilable(e) => write!(f, "the patch doesn't build: {e}"),
            LibError::CodeNotice(_) => write!(f, "the patch contains custom code"),
            LibError::CodeDeclined(_) => write!(
                f,
                "that sound has custom code you declined, so it wasn't opened; open it again to be asked once more"
            ),
        }
    }
}

impl LibError {
    /// The error for the log: its kind and, for I/O, the system's reason, without the paths
    /// and names in the message (user sound paths and names are typed by the user).
    pub fn log_text(&self) -> String {
        let reason = |e: &str| e.rsplit(": ").next().unwrap_or("").to_string();
        match self {
            LibError::BadName(_) => "bad-name".into(),
            LibError::NameTaken(_) => "name-taken".into(),
            LibError::Factory => "factory".into(),
            LibError::ReplaceMismatch { .. } => "replace-mismatch".into(),
            LibError::Io(e) => format!("io ({})", reason(e)),
            LibError::Interrupted(e) => format!("interrupted ({})", reason(e)),
            LibError::Unreadable(e) => format!("unreadable ({})", reason(e)),
            LibError::Uncompilable(_) => "uncompilable".into(),
            LibError::CodeNotice(_) => "code-notice".into(),
            LibError::CodeDeclined(_) => "code-declined".into(),
        }
    }
}

fn io(what: &str, path: &Path, e: std::io::Error) -> LibError {
    LibError::Io(format!("{what} {}: {e}", path.display()))
}

#[derive(Clone)]
pub struct Library {
    /// Where factory sounds were found, if anywhere.
    pub factory_dir: Option<PathBuf>,
    /// User data root: `sounds/` and `library.json`.
    pub user_root: PathBuf,
    pub entries: Vec<Entry>,
    pub prefs: Prefs,
    /// Problems met while scanning (unreadable metadata, repaired saves, no factory dir).
    pub notes: Vec<String>,
}

/// The factory sound directory: `KABL_FACTORY_DIR`, else `<exe>/../share/kabl/patches`
/// (the package layout), else `$XDG_DATA_DIRS/kabl/patches`, else the source checkout's
/// `patches/` (a development build).
pub fn find_factory_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("KABL_FACTORY_DIR") {
        return Some(PathBuf::from(d));
    }
    let mut candidates = Vec::new();
    if let Some(bin) = std::env::current_exe().ok().and_then(|e| {
        e.canonicalize()
            .ok()
            .and_then(|e| e.parent().map(Path::to_path_buf))
    }) {
        candidates.push(bin.join("../share/kabl/patches"));
    }
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    candidates.extend(
        data_dirs
            .split(':')
            .map(|d| Path::new(d).join("kabl/patches")),
    );
    // A development build: the checkout's `patches/` packed by this crate's build script.
    candidates.push(built_factory_dir());
    candidates
        .into_iter()
        .find(|d| d.is_dir())
        .map(|d| d.canonicalize().unwrap_or(d))
}

/// The factory sounds as `.kabl` files, packed from the checkout's `patches/` folders when this
/// crate was built (`build.rs`). What a development build and the tests use.
pub fn built_factory_dir() -> PathBuf {
    PathBuf::from(env!("KABL_FACTORY_BUILD"))
}

/// `KABL_USER_DIR`, else `$XDG_DATA_HOME/kabl`, else `~/.local/share/kabl`.
pub fn default_user_root() -> PathBuf {
    if let Some(d) = std::env::var_os("KABL_USER_DIR") {
        return PathBuf::from(d);
    }
    if let Some(d) = std::env::var_os("XDG_DATA_HOME").filter(|d| !d.is_empty()) {
        return PathBuf::from(d).join("kabl");
    }
    let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from);
    home.join(".local/share/kabl")
}

fn is_patch_file(p: &Path) -> bool {
    p.is_file() && is_kabl_path(p)
}

/// Reads a sound file: the metadata frame (`sound.toml` text) or the defaults, with the play
/// flags taken from the patch itself.
fn read_meta(file: &Path, fallback_name: &str, notes: &mut Vec<String>) -> Meta {
    let by_default = Meta {
        name: fallback_name.to_string(),
        ..Meta::default()
    };
    let (log, text) = match kabl_core::format::load_kabl_meta(file) {
        Ok(v) => v,
        Err(e) => {
            notes.push(format!("{}: unreadable ({e:?})", file.display()));
            return by_default;
        }
    };
    let mut meta = match text {
        Some(t) => toml::from_str::<Meta>(&t).unwrap_or_else(|e| {
            notes.push(format!("{}: unreadable metadata ({e})", file.display()));
            Meta::default()
        }),
        None => Meta::default(),
    };
    if meta.name.trim().is_empty() {
        meta.name = by_default.name;
    }
    (meta.keys, meta.sequence) = Meta::play_of(log.state());
    meta
}

/// Directory name for a new sound called `name`: lower-case letters, digits and dashes.
pub fn slug(name: &str) -> String {
    let mut s = String::new();
    for c in name.trim().chars() {
        if c.is_alphanumeric() {
            s.extend(c.to_lowercase());
        } else if !s.ends_with('-') && !s.is_empty() {
            s.push('-');
        }
    }
    let s = s.trim_end_matches('-').to_string();
    if s.is_empty() {
        "sound".into()
    } else {
        s
    }
}

fn check_name(name: &str) -> Result<String, LibError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(LibError::BadName("give the sound a name".into()));
    }
    if name.chars().count() > MAX_NAME {
        return Err(LibError::BadName(format!(
            "names are at most {MAX_NAME} characters"
        )));
    }
    Ok(name.to_string())
}

/// Reads a `.kabl` file and checks that it compiles, without touching anything else:
/// the caller replaces its working sound only on `Ok`. Embedded code is registered only when
/// `trust` (the remembered notice answers) allows it.
pub fn read_patch(
    file: &Path,
    trust: &std::collections::BTreeMap<String, bool>,
) -> Result<PatchLog, LibError> {
    read_patch_declined(file, trust).map(|(log, _)| log)
}

/// `read_patch`, also returning the code the user had declined (now stand-ins), so the caller
/// can forget that answer and ask again next time.
pub fn read_patch_declined(
    file: &Path,
    trust: &std::collections::BTreeMap<String, bool>,
) -> Result<(PatchLog, Vec<crate::code_notice::CodeEntry>), LibError> {
    let (log, declined, scope) = read_patch_scoped(file, trust)?;
    // Single-song callers (tests, tools): this is now the open song.
    kabl_modules::code::host::set_active_scope(scope);
    Ok((log, declined))
}

/// [`read_patch_declined`] that leaves the process's open song alone and returns the song's
/// [`CodeScope`](kabl_modules::code::CodeScope) instead: the document owns it, and whoever
/// accepts the open makes it current. A read whose result is discarded changes nothing.
pub fn read_patch_scoped(
    file: &Path,
    trust: &std::collections::BTreeMap<String, bool>,
) -> Result<
    (
        PatchLog,
        Vec<crate::code_notice::CodeEntry>,
        kabl_modules::code::CodeScope,
    ),
    LibError,
> {
    let mut declined = Vec::new();
    let (log, _, code) = kabl_core::format::load_kabl_full(file)
        .map_err(|e| LibError::Unreadable(format!("{e:?}")))?;
    let mut scope = kabl_modules::code::CodeScope::new();
    if !code.is_empty() {
        let entries = crate::code_notice::entries(&code);
        match crate::code_notice::gate(&entries, trust) {
            crate::code_notice::Gate::Ask => return Err(LibError::CodeNotice(entries)),
            // Declined: the patch still opens, with stand-ins for the code.
            crate::code_notice::Gate::Refuse => {
                scope = crate::code_notice::decline(&code, log.state());
                declined = entries;
            }
            crate::code_notice::Gate::Open => {
                scope = crate::code_notice::register(&code, log.state());
            }
        }
    }
    let scope = kabl_standalone::code_load::stand_in_missing_in(log.state(), &scope);
    // Only checks that the song builds: it must not take the live graph's settings cells.
    kabl_engine::compile::compile_scoped(
        log.state(),
        48000.0,
        kabl_standalone::DEFAULT_VOICE_COUNT,
        &scope.detached(),
    )
    .map_err(|e| LibError::Uncompilable(e.to_string()))?;
    Ok((log, declined, scope))
}

/// Whether `p` names a `.kabl` file (by its extension, not by what is on disk).
pub fn is_kabl_path(p: &Path) -> bool {
    p.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("kabl"))
}

/// Where `save_patch` writes for `path`: a `.kabl` path as it is, any other name as `name.kabl`.
pub fn patch_target(path: &Path) -> PathBuf {
    if is_kabl_path(path) {
        return path.to_path_buf();
    }
    let mut s = path.as_os_str().to_owned();
    s.push(".kabl");
    PathBuf::from(s)
}

/// Saves `log` as a `.kabl` file (see `patch_target`), reads it back to check it, and returns
/// the file written. A failed save leaves the old file in place (`save_kabl` renames last).
pub fn save_patch(path: &Path, log: &PatchLog) -> Result<PathBuf, LibError> {
    let target = patch_target(path);
    fail_point("kabl")?;
    let code = crate::code_notice::embedded_for(log.state());
    kabl_core::format::save_kabl_full(&target, log, None, Some(&code))
        .map_err(|e| LibError::Io(format!("can't save to {}: {e:?}", target.display())))?;
    let back = kabl_core::format::load_kabl(&target)
        .map_err(|e| LibError::Io(format!("saved copy doesn't read back: {e:?}")))?;
    if back.state() != log.state() {
        return Err(LibError::Io("saved copy doesn't match the sound".into()));
    }
    Ok(target)
}

thread_local! {
    static FAIL_AT: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Test hook: the next saves on this thread fail at `stage` (`None` clears it). The app
/// reads the same stage from `KABL_SAVE_FAIL` (a test hook for scripted real-app runs, like
/// `KABL_RECORD_FAIL_AFTER`; never set in normal use). Stages: `staged` (the new copy is written and
/// read back, nothing replaced yet), `replace:dir` (a sound of yours moved aside),
/// `replace:<file>` (a folder save after that file was replaced: `meta.toml`,
/// `checkpoint.json`, `log.jsonl`), `kabl` (a `.kabl` patch file, before anything is written).
pub fn fail_saves_at(stage: Option<&str>) {
    FAIL_AT.with(|f| *f.borrow_mut() = stage.map(str::to_string));
}

fn fail_point(stage: &str) -> Result<(), LibError> {
    let hit = FAIL_AT.with(|f| f.borrow().as_deref() == Some(stage))
        || std::env::var("KABL_SAVE_FAIL").is_ok_and(|v| v == stage);
    if hit {
        Err(LibError::Io(format!("injected failure at {stage}")))
    } else {
        Ok(())
    }
}

/// Writes `text` to `path` through a temporary file and a rename.
fn write_atomic(path: &Path, text: &str) -> Result<(), LibError> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, text).map_err(|e| io("can't write", &tmp, e))?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        io("can't write", path, e)
    })
}

impl Library {
    /// Scans both areas. Never fails: problems end up in `notes`.
    pub fn open(factory_dir: Option<PathBuf>, user_root: PathBuf) -> Library {
        let mut lib = Library {
            factory_dir,
            user_root,
            entries: Vec::new(),
            prefs: Prefs::default(),
            notes: Vec::new(),
        };
        lib.prefs = fs::read_to_string(lib.user_root.join(PREFS_FILE))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        lib.rescan();
        lib
    }

    pub fn sounds_dir(&self) -> PathBuf {
        self.user_root.join("sounds")
    }

    pub fn rescan(&mut self) {
        let mut notes = Vec::new();
        let mut entries = Vec::new();
        match &self.factory_dir {
            Some(root) => {
                let mut files = Vec::new();
                for e in fs::read_dir(root).into_iter().flatten().flatten() {
                    let p = e.path();
                    if is_patch_file(&p) {
                        files.push(p);
                    } else if p.is_dir() {
                        for e in fs::read_dir(&p).into_iter().flatten().flatten() {
                            if is_patch_file(&e.path()) {
                                files.push(e.path());
                            }
                        }
                    }
                }
                for file in files {
                    let rel = file
                        .strip_prefix(root)
                        .unwrap_or(&file)
                        .with_extension("")
                        .to_string_lossy()
                        .replace('\\', "/");
                    let fallback = file
                        .file_stem()
                        .map_or(rel.clone(), |n| n.to_string_lossy().to_string());
                    let meta = read_meta(&file, &fallback, &mut notes);
                    entries.push(Entry {
                        id: format!("factory:{rel}"),
                        origin: Origin::Factory,
                        dir: file,
                        meta,
                    });
                }
            }
            None => notes.push(
                "factory sounds not found: install the package's share/kabl/patches, or set \
                 KABL_FACTORY_DIR"
                    .into(),
            ),
        }
        let sounds = self.sounds_dir();
        for e in fs::read_dir(&sounds).into_iter().flatten().flatten() {
            let name = e
                .path()
                .file_stem()
                .map_or(String::new(), |n| n.to_string_lossy().to_string());
            if e.file_name().to_string_lossy().starts_with('.') || !is_patch_file(&e.path()) {
                continue;
            }
            let meta = read_meta(&e.path(), &name, &mut notes);
            entries.push(Entry {
                id: format!("user:{name}"),
                origin: Origin::User,
                dir: e.path(),
                meta,
            });
        }
        entries.sort_by(|a, b| {
            (a.origin == Origin::User, a.meta.name.to_lowercase())
                .cmp(&(b.origin == Origin::User, b.meta.name.to_lowercase()))
        });
        self.entries = entries;
        self.notes = notes;
    }

    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// The user sound already called `name` (case-insensitive), other than `except`.
    pub fn user_named(&self, name: &str, except: Option<&str>) -> Option<&Entry> {
        let n = name.trim().to_lowercase();
        self.entries.iter().find(|e| {
            e.origin == Origin::User
                && e.meta.name.trim().to_lowercase() == n
                && Some(e.id.as_str()) != except
        })
    }

    /// Reads a sound for loading; see `read_patch`.
    pub fn read(&self, id: &str) -> Result<PatchLog, LibError> {
        let e = self
            .get(id)
            .ok_or_else(|| LibError::Unreadable(format!("\"{id}\" is no longer in the library")))?;
        read_patch_scoped(&e.dir, &self.prefs.code_trust).map(|(log, _, _)| log)
    }

    /// `read` for opening a sound: code the user had declined opens as stand-ins, and that
    /// answer is forgotten so the next open asks again.
    pub fn read_for_open(
        &mut self,
        id: &str,
    ) -> Result<(PatchLog, kabl_modules::code::CodeScope), LibError> {
        let e = self
            .get(id)
            .ok_or_else(|| LibError::Unreadable(format!("\"{id}\" is no longer in the library")))?;
        let (log, declined, scope) = read_patch_scoped(&e.dir, &self.prefs.code_trust)?;
        if !declined.is_empty() {
            self.forget_code_trust(&declined);
        }
        Ok((log, scope))
    }

    /// Saves `log` as a user sound named `meta.name`. `replace`: the user sound this save may
    /// overwrite (its own id for Save, a same-named sound the user agreed to replace). Any
    /// other user sound with that name is `NameTaken`. Returns the saved entry's id.
    pub fn save(
        &mut self,
        log: &PatchLog,
        mut meta: Meta,
        replace: Option<&str>,
    ) -> Result<String, LibError> {
        meta.name = check_name(&meta.name)?;
        if let Some(taken) = self.user_named(&meta.name, replace) {
            return Err(LibError::NameTaken(taken.id.clone()));
        }
        let target = match replace {
            Some(id) => {
                let e = self
                    .get(id)
                    .ok_or_else(|| LibError::Io(format!("\"{id}\" is gone")))?;
                if e.origin == Origin::Factory {
                    return Err(LibError::Factory);
                }
                // Only the sound that has this name (or had it: Save in place) is replaced.
                if e.meta.name.trim().to_lowercase() != meta.name.to_lowercase() {
                    return Err(LibError::ReplaceMismatch {
                        target: e.meta.name.clone(),
                        name: meta.name.clone(),
                    });
                }
                e.dir.clone()
            }
            None => {
                let base = slug(&meta.name);
                let sounds = self.sounds_dir();
                let mut file = sounds.join(format!("{base}.kabl"));
                let mut n = 2;
                while file.exists() {
                    file = sounds.join(format!("{base}-{n}.kabl"));
                    n += 1;
                }
                file
            }
        };
        (meta.keys, meta.sequence) = Meta::play_of(log.state());
        let sounds = self.sounds_dir();
        fs::create_dir_all(&sounds).map_err(|e| io("can't create", &sounds, e))?;
        let text = toml::to_string_pretty(&meta).map_err(|e| LibError::Io(e.to_string()))?;
        let mut staged = target.clone().into_os_string();
        staged.push(".new");
        let staged = PathBuf::from(staged);
        let written = (|| {
            let code = crate::code_notice::embedded_for(log.state());
            kabl_core::format::save_kabl_full(&staged, log, Some(&text), Some(&code))
                .map_err(|e| LibError::Io(format!("can't save {}: {e:?}", staged.display())))?;
            // Read it back: only a copy that loads to the same sound replaces anything.
            let (back, back_meta) = kabl_core::format::load_kabl_meta(&staged)
                .map_err(|e| LibError::Io(format!("saved copy doesn't read back: {e:?}")))?;
            if back.state() != log.state() || back_meta.as_deref() != Some(text.as_str()) {
                return Err(LibError::Io("saved copy doesn't match the sound".into()));
            }
            fail_point("staged")?;
            fs::rename(&staged, &target).map_err(|e| io("can't write", &target, e))
        })();
        if written.is_err() {
            let _ = fs::remove_file(&staged);
        }
        written?;
        let id = format!(
            "user:{}",
            target.file_stem().unwrap_or_default().to_string_lossy()
        );
        self.rescan();
        self.touch(&id);
        Ok(id)
    }

    /// Changes a user sound's metadata (rename, category, tags): the file is rewritten with the
    /// same log, through a temporary file, so a failure leaves the sound as it was.
    pub fn set_meta(&mut self, id: &str, mut meta: Meta) -> Result<(), LibError> {
        meta.name = check_name(&meta.name)?;
        let e = self
            .get(id)
            .ok_or_else(|| LibError::Io(format!("\"{id}\" is gone")))?;
        if e.origin == Origin::Factory {
            return Err(LibError::Factory);
        }
        if let Some(taken) = self.user_named(&meta.name, Some(id)) {
            return Err(LibError::NameTaken(taken.id.clone()));
        }
        (meta.keys, meta.sequence) = (e.meta.keys, e.meta.sequence);
        let text = toml::to_string_pretty(&meta).map_err(|e| LibError::Io(e.to_string()))?;
        let (log, _, code) = kabl_core::format::load_kabl_full(&e.dir)
            .map_err(|e| LibError::Unreadable(format!("{e:?}")))?;
        kabl_core::format::save_kabl_full(&e.dir, &log, Some(&text), Some(&code))
            .map_err(|e| LibError::Io(format!("can't save: {e:?}")))?;
        self.rescan();
        Ok(())
    }

    /// Records `id` as the newest recent sound and saves the preferences.
    pub fn touch(&mut self, id: &str) -> Option<String> {
        self.prefs.recents.retain(|r| r != id);
        self.prefs.recents.insert(0, id.to_string());
        self.prefs.recents.truncate(MAX_RECENTS);
        self.save_prefs()
    }

    /// Records the patch file `path` as the newest recent patch and saves the preferences.
    pub fn touch_patch(&mut self, path: &Path) -> Option<String> {
        let s = path.to_string_lossy().into_owned();
        self.prefs.patch_recents.retain(|r| *r != s);
        self.prefs.patch_recents.insert(0, s);
        self.prefs.patch_recents.truncate(MAX_PATCH_RECENTS);
        self.save_prefs()
    }

    /// The recent patch files that still exist, newest first.
    pub fn recent_patches(&self) -> Vec<PathBuf> {
        let dirs = self.prefs.patch_recents.iter().map(PathBuf::from);
        dirs.filter(|p| p.exists()).collect()
    }

    pub fn toggle_favorite(&mut self, id: &str) -> Option<String> {
        if !self.prefs.favorites.remove(id) {
            self.prefs.favorites.insert(id.to_string());
        }
        self.save_prefs()
    }

    /// Drops a missing sound from favorites and recents.
    pub fn forget(&mut self, id: &str) -> Option<String> {
        self.prefs.favorites.remove(id);
        self.prefs.recents.retain(|r| r != id);
        self.save_prefs()
    }

    /// Remembers the custom-code notice's answer for these hashes (`library.json`).
    pub fn remember_code_trust(
        &mut self,
        code: &[crate::code_notice::CodeEntry],
        open: bool,
    ) -> Option<String> {
        crate::code_notice::remember(&mut self.prefs.code_trust, code, open);
        self.save_prefs()
    }

    /// Forgets a declined answer for these hashes, so the next open asks again.
    pub fn forget_code_trust(&mut self, code: &[crate::code_notice::CodeEntry]) -> Option<String> {
        for c in code {
            self.prefs.code_trust.remove(&c.hash);
        }
        self.save_prefs()
    }

    /// Writes `library.json`. A failure is reported (`Some(message)`), never fatal: the
    /// preferences stay in memory for this session.
    pub fn save_prefs(&self) -> Option<String> {
        let text = serde_json::to_string_pretty(&self.prefs).ok()?;
        if let Err(e) = fs::create_dir_all(&self.user_root) {
            return Some(format!(
                "favorites/recents not saved: can't create {}: {e}",
                self.user_root.display()
            ));
        }
        write_atomic(&self.user_root.join(PREFS_FILE), &text)
            .err()
            .map(|e| format!("favorites/recents not saved: {e}"))
    }

    /// True when `path` is inside the factory directory (never saved over).
    pub fn is_factory_path(&self, path: &Path) -> bool {
        let Some(f) = &self.factory_dir else {
            return false;
        };
        let abs = |p: &Path| {
            p.canonicalize()
                .or_else(|_| std::path::absolute(p))
                .unwrap_or_else(|_| p.to_path_buf())
        };
        // The target may not exist yet: resolve its nearest existing ancestor.
        let mut probe = path.to_path_buf();
        let mut tail = Vec::new();
        while !probe.exists() {
            match (probe.parent(), probe.file_name()) {
                (Some(p), Some(n)) => {
                    tail.push(n.to_owned());
                    probe = p.to_path_buf();
                }
                _ => break,
            }
        }
        let mut full = abs(&probe);
        for n in tail.iter().rev() {
            full.push(n);
        }
        full.starts_with(abs(f))
    }
}
