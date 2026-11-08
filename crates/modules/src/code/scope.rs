//! Which code a song runs (D12): by blob hash, not by kind.
//!
//! In a plug-in host every kabl instance shares one process, so song A (`demo.x` v1) and song B
//! (`demo.x` v2, or a copy that failed to load and became a stand-in) must not overwrite each
//! other's module. The process-wide tables are therefore keyed by [`CodeHash`]; a song carries a
//! [`CodeScope`] that maps each kind it embeds to the exact blob, and the engine compiles with it
//! (`kabl_engine::compile::compile_scoped`). Code without a scope entry (the example modules,
//! anything installed) resolves to the kind's most recently registered blob, as before.

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::Arc;

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

/// The code one song uses: kind to table key. Cheap to clone; build it while loading the song
/// and hand it to the engine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodeScope {
    by_kind: Arc<BTreeMap<String, CodeHash>>,
}

impl CodeScope {
    pub fn new() -> CodeScope {
        CodeScope::default()
    }

    /// This scope with `kind` resolving to `key`.
    pub fn with(&self, kind: &str, key: CodeHash) -> CodeScope {
        let mut m = (*self.by_kind).clone();
        m.insert(kind.to_owned(), key);
        CodeScope {
            by_kind: Arc::new(m),
        }
    }

    pub fn key(&self, kind: &str) -> Option<CodeHash> {
        self.by_kind.get(kind).copied()
    }

    pub fn is_empty(&self) -> bool {
        self.by_kind.is_empty()
    }
}
