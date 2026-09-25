//! The sound library behind the browser: factory sounds shipped with kabl (read-only) and the
//! user's own sounds, both plain patch directories (`kabl_core::save` format). No `egui`
//! here, so it is tested headlessly (`tests/library.rs`).
//!
//! - A sound is a patch directory. Its optional `sound.toml` holds browser metadata (name,
//!   category, tags, description, how it plays); the op log stays the only musical state.
//! - Identity is `factory:<relative dir>` or `user:<dir name>`, never the display name, so a
//!   rename keeps favorites and recents, and two sounds may look alike without colliding.
//! - Favorites and recents (`library.json`) are library preferences: never in a patch, never
//!   undone.
//! - A user save writes a complete new directory beside the old one, reads it back, then
//!   swaps it in with renames (`commit_dir`). An interrupted swap is repaired on the next scan.
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
pub const MAX_NAME: usize = 60;

/// The category vocabulary, in browser order.
pub const CATEGORIES: &[&str] = &[
    "Basic",
    "Bass",
    "Lead",
    "Pad",
    "Strings",
    "Wind",
    "Percussion",
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
            "{} {} {} {}",
            self.meta.name,
            self.meta.category,
            self.meta.tags.join(" "),
            self.meta.description
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
        }
    }
}

fn io(what: &str, path: &Path, e: std::io::Error) -> LibError {
    LibError::Io(format!("{what} {}: {e}", path.display()))
}

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
    candidates.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches"));
    candidates
        .into_iter()
        .find(|d| d.is_dir())
        .map(|d| d.canonicalize().unwrap_or(d))
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

fn is_patch_dir(d: &Path) -> bool {
    d.join("meta.toml").is_file() && d.join("log.jsonl").is_file()
}

fn read_meta(dir: &Path, fallback_name: &str, notes: &mut Vec<String>) -> Meta {
    let mut meta = match fs::read_to_string(dir.join(META_FILE)) {
        Ok(text) => match toml::from_str::<Meta>(&text) {
            Ok(m) => m,
            Err(e) => {
                notes.push(format!("{}: unreadable {META_FILE} ({e})", dir.display()));
                Meta::default()
            }
        },
        // A patch saved before the library (or copied in by hand): look inside it.
        Err(_) => match kabl_core::load(dir) {
            Ok(log) => {
                let (keys, sequence) = Meta::play_of(log.state());
                Meta {
                    keys,
                    sequence,
                    ..Meta::default()
                }
            }
            Err(_) => Meta::default(),
        },
    };
    if meta.name.trim().is_empty() {
        meta.name = fallback_name.to_string();
    }
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

/// Reads a patch directory and checks that it compiles, without touching anything else:
/// the caller replaces its working sound only on `Ok`.
pub fn read_patch(dir: &Path) -> Result<PatchLog, LibError> {
    repair_folder(dir);
    let log = kabl_core::load(dir).map_err(|e| LibError::Unreadable(format!("{e:?}")))?;
    kabl_engine::compile::compile(log.state(), 48000.0, kabl_standalone::DEFAULT_VOICE_COUNT)
        .map_err(|e| LibError::Uncompilable(e.to_string()))?;
    Ok(log)
}

/// Replaces `target` by the complete directory `staged` with renames: `target` → `.old`,
/// `staged` → `target`, then `.old` is removed. A failure puts the old one back. Not
/// crash-proof (no fsync); `repair` finishes or undoes an interrupted swap on the next scan.
fn commit_dir(staged: &Path, target: &Path) -> Result<(), LibError> {
    let old = sibling(target, "old");
    if old.exists() {
        if !target.exists() {
            // The only saved copy: put it back rather than clear it.
            fs::rename(&old, target).map_err(|e| io("can't restore", &old, e))?;
        } else {
            fs::remove_dir_all(&old).map_err(|e| io("can't clear", &old, e))?;
        }
    }
    let had = target.exists();
    if had {
        fs::rename(target, &old).map_err(|e| io("can't replace", target, e))?;
    }
    let moved = fail_point("replace:dir")
        .and_then(|()| fs::rename(staged, target).map_err(|e| io("can't write", target, e)));
    if let Err(e) = moved {
        if had {
            if let Err(back) = fs::rename(&old, target) {
                return Err(LibError::Interrupted(format!(
                    "{e}; the previous version is kept at {} ({back}) and is restored the next \
                     time kabl starts",
                    old.display()
                )));
            }
        }
        return Err(e);
    }
    if had {
        let _ = fs::remove_dir_all(&old);
    }
    Ok(())
}

thread_local! {
    static FAIL_AT: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Test hook: the next saves on this thread fail at `stage` (`None` clears it). The app
/// reads the same stage from `KABL_SAVE_FAIL` (a test hook for scripted real-app runs, like
/// `KABL_RECORD_FAIL_AFTER`; never set in normal use). Stages: `staged` (the new copy is written and
/// read back, nothing replaced yet), `replace:dir` (a sound of yours moved aside),
/// `replace:<file>` (a folder save after that file was replaced: `meta.toml`,
/// `checkpoint.json`, `log.jsonl`).
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

/// The patch files a folder save replaces, in the order it replaces them: the log, the only
/// authoritative one, last.
const FOLDER_FILES: [&str; 3] = ["meta.toml", "checkpoint.json", "log.jsonl"];
/// Staging directory a folder save keeps inside the patch folder while it works.
const FOLDER_STAGING: &str = ".kabl-save";

/// Marker file inside a staging directory kabl made: `writing` while the new copy is written,
/// `replacing` once it has been read back and files start moving, `done` (the save stands)
/// or `undone` (rolled back) before the staging is cleaned up. A `.kabl-save` without it is
/// not kabl's and is never touched.
const FOLDER_MARKER: &str = "kabl-staging";

/// Saves `log` into a patch folder the user chose, touching only its three patch files
/// (other files in the folder stay). The new files are written and read back in
/// `<dir>/.kabl-save/`, then each old file is moved into the staging directory and the new
/// one moved in, `log.jsonl` last. Any failure moves the old files back and removes files
/// the save added, so the folder is as it was. A crash half way is finished or undone by
/// `repair_folder` on the next open or save of that folder. Not crash-proof on power loss
/// (no fsync). A `.kabl-save` that kabl didn't make blocks the save instead of being used.
pub fn save_folder(dir: &Path, log: &PatchLog) -> Result<(), LibError> {
    repair_folder(dir);
    let staging = dir.join(FOLDER_STAGING);
    let marker = staging.join(FOLDER_MARKER);
    // An empty one (kabl stopped right after making it) just goes.
    let _ = fs::remove_dir(&staging);
    if marker.exists() {
        return Err(LibError::Interrupted(format!(
            "an earlier save here could not be undone: the previous files are kept in {}",
            staging.display()
        )));
    }
    if staging.exists() {
        return Err(LibError::Io(format!(
            "{} is in the way (kabl didn't make it): rename it to save here",
            staging.display()
        )));
    }
    let written = (|| {
        fs::create_dir_all(&staging).map_err(|e| io("can't create", &staging, e))?;
        fs::write(&marker, "writing").map_err(|e| io("can't write", &marker, e))?;
        kabl_core::save(&staging, log)
            .map_err(|e| LibError::Io(format!("can't save to {}: {e:?}", dir.display())))?;
        let back = kabl_core::load(&staging)
            .map_err(|e| LibError::Io(format!("saved copy doesn't read back: {e:?}")))?;
        if back.state() != log.state() {
            return Err(LibError::Io("saved copy doesn't match the sound".into()));
        }
        fail_point("staged")?;
        fs::write(&marker, "replacing").map_err(|e| io("can't write", &marker, e))
    })();
    if let Err(e) = written {
        remove_staging(&staging);
        return Err(e);
    }
    let mut result = Ok(());
    for f in FOLDER_FILES {
        let (live, new, old) = (
            dir.join(f),
            staging.join(f),
            staging.join(format!("{f}.old")),
        );
        let step = (|| {
            if live.exists() {
                fs::rename(&live, &old).map_err(|e| io("can't replace", &live, e))?;
            }
            fs::rename(&new, &live).map_err(|e| io("can't write", &live, e))?;
            fail_point(&format!("replace:{f}"))
        })();
        if let Err(e) = step {
            result = Err(e);
            break;
        }
    }
    if let Err(e) = result {
        if roll_back(dir, &staging) {
            let _ = fs::write(&marker, "undone");
            remove_staging(&staging);
            return Err(e);
        }
        return Err(LibError::Interrupted(format!(
            "{e}; putting the previous files back failed: they are kept in {} and the next \
             open or save of this folder settles it",
            staging.display()
        )));
    }
    // The save stands from here on, whatever happens to the cleanup.
    let _ = fs::write(&marker, "done");
    remove_staging(&staging);
    Ok(())
}

/// Undoes a folder save in the `replacing` phase: every `.old` goes back over its file, and a
/// patch file the save moved in where there was none (its staged copy gone, no `.old`) is
/// removed. True when everything was undone.
fn roll_back(dir: &Path, staging: &Path) -> bool {
    let mut ok = true;
    for f in FOLDER_FILES {
        let (live, new, old) = (
            dir.join(f),
            staging.join(f),
            staging.join(format!("{f}.old")),
        );
        let r = if old.exists() {
            fs::rename(&old, &live)
        } else if !new.exists() && live.exists() {
            fs::remove_file(&live)
        } else {
            Ok(())
        };
        ok &= r.is_ok();
    }
    ok
}

/// Removes a staging directory kabl made: only the files it writes, then the directory
/// itself (not recursively, so anything else in it stays and keeps the directory).
fn remove_staging(staging: &Path) {
    for f in FOLDER_FILES {
        let _ = fs::remove_file(staging.join(f));
        let _ = fs::remove_file(staging.join(format!("{f}.old")));
    }
    let _ = fs::remove_file(staging.join(FOLDER_MARKER));
    let _ = fs::remove_dir(staging);
}

/// Finishes or undoes a folder save that stopped half way (see `save_folder`). Acts only on
/// a staging directory with kabl's marker. `writing`: nothing in the folder changed.
/// `done` / `undone`: only the cleanup was left. `replacing`: the log moves last, so if the
/// new log is in place (its staged copy gone and `log.jsonl` present) the save completed and
/// stands; otherwise it is rolled back. Returns a note for the user when something was
/// restored.
pub fn repair_folder(dir: &Path) -> Option<String> {
    let staging = dir.join(FOLDER_STAGING);
    let phase = fs::read_to_string(staging.join(FOLDER_MARKER)).ok()?;
    let committed = !staging.join("log.jsonl").exists() && dir.join("log.jsonl").exists();
    let mut note = None;
    if phase.trim() == "replacing" && !committed {
        if roll_back(dir, &staging) {
            note = Some(format!(
                "{}: an interrupted save was undone; the folder is as it was before it",
                dir.display()
            ));
        } else {
            return Some(format!(
                "{}: an interrupted save could not be undone; the previous files are in {}",
                dir.display(),
                staging.display()
            ));
        }
    }
    remove_staging(&staging);
    note
}

/// `.name.old` / `.name.new` beside `dir`.
fn sibling(dir: &Path, what: &str) -> PathBuf {
    let name = dir
        .file_name()
        .map_or_else(Default::default, |n| n.to_string_lossy().to_string());
    dir.with_file_name(format!(".{name}.{what}"))
}

/// Finishes or undoes interrupted saves in `sounds`: a `.x.old` whose `x` is gone is put
/// back (the swap stopped half way), otherwise removed; leftover `.x.new` staging is removed.
fn repair(sounds: &Path, notes: &mut Vec<String>) {
    let Ok(rd) = fs::read_dir(sounds) else {
        return;
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some(rest) = name.strip_prefix('.') else {
            continue;
        };
        if let Some(base) = rest.strip_suffix(".old") {
            let target = sounds.join(base);
            if target.exists() {
                let _ = fs::remove_dir_all(e.path());
            } else if fs::rename(e.path(), &target).is_ok() {
                notes.push(format!("restored \"{base}\" after an interrupted save"));
            }
        } else if rest.ends_with(".new") {
            let _ = fs::remove_dir_all(e.path());
        }
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
                let mut dirs = Vec::new();
                for e in fs::read_dir(root).into_iter().flatten().flatten() {
                    let p = e.path();
                    if is_patch_dir(&p) {
                        dirs.push(p);
                    } else if p.is_dir() {
                        for e in fs::read_dir(&p).into_iter().flatten().flatten() {
                            if is_patch_dir(&e.path()) {
                                dirs.push(e.path());
                            }
                        }
                    }
                }
                for dir in dirs {
                    let rel = dir
                        .strip_prefix(root)
                        .unwrap_or(&dir)
                        .to_string_lossy()
                        .replace('\\', "/");
                    let fallback = dir
                        .file_name()
                        .map_or(rel.clone(), |n| n.to_string_lossy().to_string());
                    let meta = read_meta(&dir, &fallback, &mut notes);
                    entries.push(Entry {
                        id: format!("factory:{rel}"),
                        origin: Origin::Factory,
                        dir,
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
        repair(&sounds, &mut notes);
        for e in fs::read_dir(&sounds).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || !is_patch_dir(&e.path()) {
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
        read_patch(&e.dir)
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
        // A swap that stopped half way earlier this session is settled first, so its `.old`
        // (possibly the only saved copy) is put back rather than cleared by this save.
        repair(&self.sounds_dir(), &mut self.notes);
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
                let mut dir = sounds.join(&base);
                let mut n = 2;
                while dir.exists() || sibling(&dir, "old").exists() {
                    dir = sounds.join(format!("{base}-{n}"));
                    n += 1;
                }
                dir
            }
        };
        (meta.keys, meta.sequence) = Meta::play_of(log.state());
        let sounds = self.sounds_dir();
        fs::create_dir_all(&sounds).map_err(|e| io("can't create", &sounds, e))?;
        let staged = sibling(&target, "new");
        let _ = fs::remove_dir_all(&staged);
        let written = (|| {
            kabl_core::save(&staged, log)
                .map_err(|e| LibError::Io(format!("can't save {}: {e:?}", staged.display())))?;
            let text = toml::to_string_pretty(&meta).map_err(|e| LibError::Io(e.to_string()))?;
            fs::write(staged.join(META_FILE), text).map_err(|e| io("can't write", &staged, e))?;
            // Read it back: only a copy that loads to the same sound replaces anything.
            let back = kabl_core::load(&staged)
                .map_err(|e| LibError::Io(format!("saved copy doesn't read back: {e:?}")))?;
            if back.state() != log.state() {
                return Err(LibError::Io("saved copy doesn't match the sound".into()));
            }
            fail_point("staged")?;
            commit_dir(&staged, &target)
        })();
        if written.is_err() {
            let _ = fs::remove_dir_all(&staged);
        }
        written?;
        let id = format!(
            "user:{}",
            target.file_name().unwrap_or_default().to_string_lossy()
        );
        self.rescan();
        self.touch(&id);
        Ok(id)
    }

    /// Changes a user sound's metadata (rename, category, tags). Only `sound.toml` changes,
    /// through a temporary file: a failure leaves the sound as it was.
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
        write_atomic(&e.dir.join(META_FILE), &text)?;
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
