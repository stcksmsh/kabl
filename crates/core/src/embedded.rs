//! Code embedded in a `.kabl` file (D11, decisions 2 and 7): WebAssembly blobs with a SHA-256
//! hash, a module id and a version, the instances that reference them by hash (so identical code
//! is stored once), and the state blobs kept from an older module version.
//!
//! The container carries them as extra skippable frames (see `format.rs`), so a build that does
//! not know them skips them and a file without them loads as before.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::ModuleId;

/// Size cap per embedded blob (decision 2). Over it, saving fails and a file that has one does
/// not load.
pub const MAX_CODE_BLOB_BYTES: usize = 2 << 20;
/// Cap on distinct blobs in one file, so a hostile file cannot make the loader build a huge map.
pub const MAX_CODE_BLOBS: usize = 256;

/// SHA-256 of a blob's bytes.
pub type CodeHash = [u8; 32];

pub fn hash_of(bytes: &[u8]) -> CodeHash {
    Sha256::digest(bytes).into()
}

/// Lower-case hex, the form used in library preferences and messages.
pub fn hash_hex(h: &CodeHash) -> String {
    h.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmbedError {
    /// A blob over [`MAX_CODE_BLOB_BYTES`].
    TooLarge { size: usize, cap: usize },
    /// More than [`MAX_CODE_BLOBS`] distinct blobs.
    TooMany,
    /// An empty blob or an empty module id.
    Empty,
    /// The hash names no blob in this file.
    UnknownHash(String),
    /// The same hash was added again under a different module id or version.
    Conflict(String),
}

impl std::fmt::Display for EmbedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmbedError::TooLarge { size, cap } => write!(
                f,
                "custom code module is {size} bytes; the limit is {cap} bytes (2 MiB)"
            ),
            EmbedError::TooMany => write!(f, "more than {MAX_CODE_BLOBS} custom code modules"),
            EmbedError::Empty => write!(f, "custom code module has no id or no code"),
            EmbedError::UnknownHash(h) => write!(f, "no embedded code with hash {h}"),
            EmbedError::Conflict(h) => {
                write!(
                    f,
                    "code {h} is already embedded under another id or version"
                )
            }
        }
    }
}

impl std::error::Error for EmbedError {}

/// One embedded module: its id and version travel with the bytes; the hash is of the bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlob {
    pub module_id: String,
    pub version: u32,
    pub bytes: Vec<u8>,
}

/// The state an instance had under an older module version, kept so reopening with that version
/// restores it (and a later `migrate` export loses nothing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptState {
    pub instance: ModuleId,
    pub version: u32,
    pub bytes: Vec<u8>,
}

/// A module instance whose embedded code has a different version from the one now installed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionChange {
    pub instance: ModuleId,
    pub module_id: String,
    pub from: u32,
    pub to: u32,
}

impl VersionChange {
    /// The notice text of decision 7.
    pub fn notice(&self) -> String {
        format!(
            "Custom module {} updated from v{} to v{}: settings reset",
            self.module_id, self.from, self.to
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Embedded {
    blobs: BTreeMap<CodeHash, CodeBlob>,
    refs: BTreeMap<ModuleId, CodeHash>,
    kept: Vec<KeptState>,
}

impl Embedded {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.blobs.is_empty() && self.refs.is_empty() && self.kept.is_empty()
    }

    /// Stores `bytes` and returns their hash. Adding identical bytes again stores nothing new.
    pub fn add_blob(
        &mut self,
        module_id: &str,
        version: u32,
        bytes: Vec<u8>,
    ) -> Result<CodeHash, EmbedError> {
        if bytes.len() > MAX_CODE_BLOB_BYTES {
            return Err(EmbedError::TooLarge {
                size: bytes.len(),
                cap: MAX_CODE_BLOB_BYTES,
            });
        }
        if bytes.is_empty() || module_id.is_empty() || module_id.len() > u16::MAX as usize {
            return Err(EmbedError::Empty);
        }
        let hash = hash_of(&bytes);
        if let Some(have) = self.blobs.get(&hash) {
            if have.module_id != module_id || have.version != version {
                return Err(EmbedError::Conflict(hash_hex(&hash)));
            }
            return Ok(hash);
        }
        if self.blobs.len() >= MAX_CODE_BLOBS {
            return Err(EmbedError::TooMany);
        }
        self.blobs.insert(
            hash,
            CodeBlob {
                module_id: module_id.to_string(),
                version,
                bytes,
            },
        );
        Ok(hash)
    }

    /// Makes `instance` use the code with `hash`.
    pub fn bind(&mut self, instance: ModuleId, hash: CodeHash) -> Result<(), EmbedError> {
        if !self.blobs.contains_key(&hash) {
            return Err(EmbedError::UnknownHash(hash_hex(&hash)));
        }
        self.refs.insert(instance, hash);
        Ok(())
    }

    pub fn blob(&self, hash: &CodeHash) -> Option<&CodeBlob> {
        self.blobs.get(hash)
    }

    pub fn blobs(&self) -> impl Iterator<Item = (&CodeHash, &CodeBlob)> {
        self.blobs.iter()
    }

    pub fn refs(&self) -> impl Iterator<Item = (ModuleId, &CodeHash)> {
        self.refs.iter().map(|(i, h)| (*i, h))
    }

    pub fn instance_hash(&self, instance: ModuleId) -> Option<&CodeHash> {
        self.refs.get(&instance)
    }

    pub fn instance_blob(&self, instance: ModuleId) -> Option<&CodeBlob> {
        self.refs.get(&instance).and_then(|h| self.blobs.get(h))
    }

    /// Blobs no instance references (after modules were deleted); `write` leaves them out.
    pub fn prune_unreferenced(&mut self) {
        let used: std::collections::BTreeSet<CodeHash> = self.refs.values().copied().collect();
        self.blobs.retain(|h, _| used.contains(h));
    }

    /// Forgets the binding of a deleted instance (its kept states stay with the file).
    pub fn unbind(&mut self, instance: ModuleId) {
        self.refs.remove(&instance);
    }

    /// Records `instance`'s state under `version`; replaces an earlier one for the same pair.
    pub fn keep_state(&mut self, instance: ModuleId, version: u32, bytes: Vec<u8>) {
        match self
            .kept
            .iter_mut()
            .find(|k| k.instance == instance && k.version == version)
        {
            Some(k) => k.bytes = bytes,
            None => self.kept.push(KeptState {
                instance,
                version,
                bytes,
            }),
        }
    }

    pub fn kept_state(&self, instance: ModuleId, version: u32) -> Option<&[u8]> {
        self.kept
            .iter()
            .find(|k| k.instance == instance && k.version == version)
            .map(|k| &k.bytes[..])
    }

    pub fn kept_states(&self) -> &[KeptState] {
        &self.kept
    }

    /// Instances whose embedded version differs from the installed one (`installed` returns the
    /// installed version of a module id, `None` when there is no such module). The caller resets
    /// those instances to defaults after [`Embedded::keep_state`]-ing their old state, and shows
    /// [`VersionChange::notice`].
    pub fn version_changes(&self, installed: impl Fn(&str) -> Option<u32>) -> Vec<VersionChange> {
        let mut out = Vec::new();
        for (&instance, hash) in &self.refs {
            let Some(blob) = self.blobs.get(hash) else {
                continue;
            };
            if let Some(to) = installed(&blob.module_id).filter(|&v| v != blob.version) {
                out.push(VersionChange {
                    instance,
                    module_id: blob.module_id.clone(),
                    from: blob.version,
                    to,
                });
            }
        }
        out
    }

    // ---- frame payloads (used by format.rs)

    pub(crate) fn blob_payload(hash: &CodeHash, b: &CodeBlob) -> Vec<u8> {
        let mut p = Vec::with_capacity(38 + b.module_id.len() + b.bytes.len());
        p.extend_from_slice(hash);
        p.extend_from_slice(&b.version.to_le_bytes());
        p.extend_from_slice(&(b.module_id.len() as u16).to_le_bytes());
        p.extend_from_slice(b.module_id.as_bytes());
        p.extend_from_slice(&b.bytes);
        p
    }

    pub(crate) fn read_blob(&mut self, p: &[u8]) -> Result<(), String> {
        let hash: CodeHash = p.get(..32).ok_or("code frame is cut")?.try_into().unwrap();
        let version = u32::from_le_bytes(
            p.get(32..36)
                .ok_or("code frame is cut")?
                .try_into()
                .unwrap(),
        );
        let n = u16::from_le_bytes(
            p.get(36..38)
                .ok_or("code frame is cut")?
                .try_into()
                .unwrap(),
        ) as usize;
        let id = p.get(38..38 + n).ok_or("code frame is cut")?;
        let id = std::str::from_utf8(id).map_err(|e| e.to_string())?;
        let bytes = p[38 + n..].to_vec();
        if hash_of(&bytes) != hash {
            return Err(format!(
                "embedded code {} does not match its hash",
                hash_hex(&hash)
            ));
        }
        self.add_blob(id, version, bytes)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub(crate) fn ref_payload(instance: ModuleId, hash: &CodeHash) -> Vec<u8> {
        let mut p = instance.to_le_bytes().to_vec();
        p.extend_from_slice(hash);
        p
    }

    pub(crate) fn read_ref(&mut self, p: &[u8]) -> Result<(), String> {
        if p.len() != 40 {
            return Err("code reference frame has the wrong size".into());
        }
        let instance = u64::from_le_bytes(p[..8].try_into().unwrap());
        self.bind(instance, p[8..].try_into().unwrap())
            .map_err(|e| e.to_string())
    }

    pub(crate) fn kept_payload(k: &KeptState) -> Vec<u8> {
        let mut p = k.instance.to_le_bytes().to_vec();
        p.extend_from_slice(&k.version.to_le_bytes());
        p.extend_from_slice(&k.bytes);
        p
    }

    pub(crate) fn read_kept(&mut self, p: &[u8]) -> Result<(), String> {
        if p.len() < 12 {
            return Err("kept-state frame is cut".into());
        }
        let instance = u64::from_le_bytes(p[..8].try_into().unwrap());
        let version = u32::from_le_bytes(p[8..12].try_into().unwrap());
        self.keep_state(instance, version, p[12..].to_vec());
        Ok(())
    }
}
