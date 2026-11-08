//! Which code a song runs (D12): by blob hash, not by kind.
//!
//! In a plug-in host every kabl instance shares one process, so song A (`demo.x` v1) and song B
//! (`demo.x` v2, or a copy that failed to load and became a stand-in) must not overwrite each
//! other's module. The process-wide tables are therefore keyed by [`CodeHash`]; a song carries a
//! [`CodeScope`] that maps each kind it embeds to the exact blob, and the engine compiles with it
//! (`kabl_engine::compile::compile_scoped`). Code without a scope entry (the example modules,
//! anything installed) resolves to the kind's most recently registered blob, as before.
//!
//! The scope also carries the song's saved module settings (see [`super::state`]): read from the
//! file when it opens, handed to each running instance when its graph is built, and collected
//! from the running graph when the song is saved.

use super::module::CodeModule;
use super::state::{SavedState, StateCell};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// SHA-256 of a blob's bytes: the identity a song embeds it under (the same as
/// `kabl_core::embedded::CodeHash`).
pub type CodeHash = [u8; 32];

pub fn hash_of(bytes: &[u8]) -> CodeHash {
    Sha256::digest(bytes).into()
}

/// The table key of the stand-in for a blob (or for a module with none): a different key from
/// the blob's own, so one song can run a blob while another has declined the same bytes.
pub fn standin_key(of: &[u8]) -> CodeHash {
    let mut h = Sha256::new();
    h.update(b"kabl stand-in\0");
    h.update(of);
    h.finalize().into()
}

/// The code one song uses: kind to table key, and the settings of its code-module instances.
/// Cheap to clone (clones share the settings); build it while loading the song and hand it to
/// the engine (`compile_scoped`) and to the save path.
#[derive(Clone, Default)]
pub struct CodeScope {
    by_kind: Arc<BTreeMap<String, CodeHash>>,
    session: Arc<Session>,
}

/// What a song keeps for its code-module instances, by module id.
#[derive(Default)]
struct Session {
    /// The running graph's cell for each instance (lane 0 of the latest compile).
    cells: Mutex<BTreeMap<u64, (String, Arc<StateCell>)>>,
    /// Last known settings: read from the file, or captured from the running graph.
    last: Mutex<BTreeMap<u64, (String, SavedState)>>,
    /// Things the user should hear about, once each.
    notes: Mutex<Vec<String>>,
}

impl std::fmt::Debug for CodeScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodeScope")
            .field("by_kind", &self.by_kind)
            .finish_non_exhaustive()
    }
}

impl PartialEq for CodeScope {
    fn eq(&self, other: &Self) -> bool {
        self.by_kind == other.by_kind
    }
}
impl Eq for CodeScope {}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Session {
    fn note(&self, text: String) {
        let mut n = lock(&self.notes);
        if !n.contains(&text) {
            n.push(text);
        }
    }
}

impl CodeScope {
    /// Something the user should be told, once (see [`CodeScope::take_notes`]).
    pub fn note(&self, text: String) {
        self.session.note(text);
    }

    pub fn new() -> CodeScope {
        CodeScope::default()
    }

    /// This scope with `kind` resolving to `key` (the settings stay shared).
    pub fn with(&self, kind: &str, key: CodeHash) -> CodeScope {
        let mut m = (*self.by_kind).clone();
        m.insert(kind.to_owned(), key);
        CodeScope {
            by_kind: Arc::new(m),
            session: Arc::clone(&self.session),
        }
    }

    /// The same code with no settings: for a compile that only checks the song builds.
    pub fn detached(&self) -> CodeScope {
        CodeScope {
            by_kind: Arc::clone(&self.by_kind),
            session: Arc::default(),
        }
    }

    pub fn key(&self, kind: &str) -> Option<CodeHash> {
        self.by_kind.get(kind).copied()
    }

    pub fn is_empty(&self) -> bool {
        self.by_kind.is_empty()
    }

    /// Settings read from a file for module instance `instance` of `kind`; the running module
    /// loads them when its graph is built, and a save writes them back if it never could.
    pub fn set_kept_state(&self, instance: u64, kind: &str, s: SavedState) {
        lock(&self.session.last).insert(instance, (kind.to_owned(), s));
    }

    /// The last known settings of every instance, to put in a file.
    pub fn settings(&self) -> Vec<(u64, SavedState)> {
        lock(&self.session.last)
            .iter()
            .map(|(&i, (_, s))| (i, s.clone()))
            .collect()
    }

    /// Called by the compiler for the lane-0 instance of module `instance`: gives it a
    /// [`StateCell`] and queues the song's known settings for it (migrated first when they are
    /// from an older layout).
    pub fn attach(&self, instance: u64, kind: &str, module: &mut CodeModule) {
        let Some(cap) = module.state_cap() else {
            return;
        };
        lock(&self.session.cells).remove(&instance);
        let saved = lock(&self.session.last)
            .get(&instance)
            .filter(|(k, _)| k == kind)
            .map(|(_, s)| s.clone());
        let cell = Arc::new(StateCell::new(cap));
        if let (Some(saved), Some(group)) = (saved, super::group_in(self, kind)) {
            match group.settings_for_load(&saved) {
                Ok(now) => {
                    cell.queue_load(&now);
                }
                Err(why) => {
                    // Unusable settings are never overwritten: the module runs at its defaults
                    // without a cell, so a save cannot replace the song's bytes with them.
                    self.session.note(format!(
                        "{kind} (module {instance}) starts at its defaults: {why}. The song keeps the old settings."
                    ));
                    return;
                }
            }
        }
        module.set_state_cell(Arc::clone(&cell));
        lock(&self.session.cells).insert(instance, (kind.to_owned(), cell));
    }

    /// Asks every running instance to save its settings at the start of its next block.
    pub fn request_capture(&self) {
        for (_, c) in lock(&self.session.cells).values() {
            c.request_save();
        }
    }

    /// Takes in the settings instances have saved since [`CodeScope::request_capture`]; a failed
    /// save keeps the previous settings and leaves a note. Returns how many were taken.
    pub fn collect(&self) -> usize {
        let cells: Vec<(u64, String, Arc<StateCell>)> = lock(&self.session.cells)
            .iter()
            .map(|(&i, (k, c))| (i, k.clone(), Arc::clone(c)))
            .collect();
        let mut n = 0;
        for (id, kind, c) in cells {
            if let Some(s) = c.take_saved() {
                lock(&self.session.last).insert(id, (kind, s));
                n += 1;
            }
            if let Some(f) = c.take_fail() {
                self.session.note(format!(
                    "code module {id} {}; its previous settings are kept",
                    f.label()
                ));
            }
        }
        n
    }

    /// [`CodeScope::request_capture`], then waits up to `wait` for the audio thread to answer
    /// (it does so at the start of its next block). An engine that is not running answers
    /// nothing: the last known settings stand.
    pub fn capture(&self, wait: Duration) {
        self.request_capture();
        let deadline = Instant::now() + wait;
        loop {
            self.collect();
            let waiting = lock(&self.session.cells)
                .values()
                .any(|(_, c)| c.save_pending());
            if !waiting || Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Notes for the user since the last call.
    pub fn take_notes(&self) -> Vec<String> {
        std::mem::take(&mut *lock(&self.session.notes))
    }
}
