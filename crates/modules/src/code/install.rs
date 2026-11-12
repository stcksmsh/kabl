//! Installing `.kablmod` packages into the user module directory (D12 section 4.1 and the
//! licensing decision 4). One file per module version, `<id>-<version>.kablmod`; the directory
//! is the whole install state. Songs never depend on it: they carry their own copy of the code.
//!
//! Refusals: the same id and version with different code is refused (both hashes are shown);
//! an id first installed by one author is not taken over by another (first install wins).

use super::host::register_blob;
use super::package::{Manifest, Package};
use std::path::{Path, PathBuf};

/// The module directory under the user's kabl data directory.
pub fn dir(user_root: &Path) -> PathBuf {
    user_root.join("modules")
}

#[derive(Debug, Clone)]
pub struct Installed {
    pub manifest: Manifest,
    /// Lower-case hex SHA-256 of the wasm.
    pub hash: String,
    pub path: PathBuf,
    /// True when this was the first package ever installed: the caller shows
    /// [`super::package::REDISTRIBUTION`] once.
    pub first: bool,
}

/// Every valid package in `dir` (sorted by id then version), and one message per file that is
/// not valid. A missing directory is empty.
pub fn scan(dir: &Path) -> (Vec<Installed>, Vec<String>) {
    let (mut ok, mut bad) = (vec![], vec![]);
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "kablmod"))
        .collect();
    files.sort();
    for path in files {
        let read = std::fs::read(&path).map_err(|e| e.to_string());
        match read.and_then(|b| Package::from_bytes(&b)) {
            Ok(p) => ok.push(Installed {
                hash: p.manifest.sha256.clone(),
                manifest: p.manifest,
                path,
                first: false,
            }),
            Err(e) => bad.push(format!("{}: {e}", path.display())),
        }
    }
    ok.sort_by(|a, b| {
        (&a.manifest.id, a.manifest.version).cmp(&(&b.manifest.id, b.manifest.version))
    });
    (ok, bad)
}

/// Registers every installed package with the code registry (lowest version first, so the
/// highest is the latest of its id). Returns what could not load, one message each.
pub fn load_all(dir: &Path) -> Vec<String> {
    let (ok, mut bad) = scan(dir);
    for i in ok {
        let loaded = std::fs::read(&i.path)
            .map_err(|e| e.to_string())
            .and_then(|b| Package::from_bytes(&b))
            .and_then(|p| register_blob(&p.wasm).map(drop).map_err(|e| e.to_string()));
        if let Err(e) = loaded {
            bad.push(format!("{} v{}: {e}", i.manifest.id, i.manifest.version));
        }
    }
    bad
}

/// A module id becomes a file name, so it is held to a safe alphabet.
fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && !id.starts_with('.')
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
}

/// Checks `bytes`, applies the refusal rules, writes the package into `dir` and registers its
/// code. Installing the identical package again succeeds and changes nothing.
pub fn install(dir: &Path, bytes: &[u8]) -> Result<Installed, String> {
    let p = Package::from_bytes(bytes)?;
    let m = &p.manifest;
    if !safe_id(&m.id) {
        return Err(format!("`{}` is not a usable module id", m.id));
    }
    let (have, _) = scan(dir);
    for h in have.iter().filter(|h| h.manifest.id == m.id) {
        if h.manifest.author != m.author {
            return Err(format!(
                "{} is already installed from {}; a package from {} cannot replace it",
                m.id, h.manifest.author, m.author
            ));
        }
        if h.manifest.version == m.version && h.hash != m.sha256 {
            return Err(format!(
                "{} v{} is already installed with different code (installed {}, new {})",
                m.id, m.version, h.hash, m.sha256
            ));
        }
    }
    let path = dir.join(format!("{}-{}.kablmod", m.id, m.version));
    let first = have.is_empty();
    if !path.exists() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let tmp = path.with_extension("part");
        std::fs::write(&tmp, bytes)
            .and_then(|()| std::fs::rename(&tmp, &path))
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    register_blob(&p.wasm).map_err(|e| e.to_string())?;
    Ok(Installed {
        hash: m.sha256.clone(),
        manifest: p.manifest,
        path,
        first,
    })
}

/// The `(id, version)` pairs a patch uses that no installed package provides. The patch still
/// runs from its own copy; the report tells the user they cannot edit or reuse that version
/// elsewhere without the package.
pub fn missing(installed: &[Installed], used: &[(String, u32)]) -> Vec<(String, u32)> {
    used.iter()
        .filter(|(id, v)| {
            !installed
                .iter()
                .any(|i| &i.manifest.id == id && i.manifest.version == *v)
        })
        .cloned()
        .collect()
}
