use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::log::PatchLog;
use crate::op::Entry;

/// v2 adds `PortRef::Param` (modulation routes), `Op::UnsetParam` and `Op::Group`; v3 adds
/// `Op::SetLabel`. Each is a strict superset of the one before, so older files load unchanged;
/// there is nothing to migrate.
pub const CURRENT_SCHEMA_VERSION: u32 = 3;

#[derive(Debug, Serialize, Deserialize)]
struct Meta {
    schema_version: u32,
}

#[derive(Debug)]
pub enum FormatError {
    Io(io::Error),
    Json(serde_json::Error),
    Toml(String),
    UnknownSchemaVersion(u32),
}

impl From<io::Error> for FormatError {
    fn from(e: io::Error) -> Self {
        FormatError::Io(e)
    }
}
impl From<serde_json::Error> for FormatError {
    fn from(e: serde_json::Error) -> Self {
        FormatError::Json(e)
    }
}

/// Save a patch as a directory: `log.jsonl` (one JSON `Entry` per line, the source of truth),
/// `checkpoint.json` (full state snapshot — written but not yet consulted on load, see
/// docs/decisions.md), `meta.toml` (schema version). Brief section 6.
pub fn save(dir: &Path, log: &PatchLog) -> Result<(), FormatError> {
    fs::create_dir_all(dir)?;

    let mut lines = String::new();
    for entry in log.entries() {
        lines.push_str(&serde_json::to_string(entry)?);
        lines.push('\n');
    }
    fs::write(dir.join("log.jsonl"), lines)?;

    let checkpoint = serde_json::to_string_pretty(log.state())?;
    fs::write(dir.join("checkpoint.json"), checkpoint)?;

    let meta = Meta {
        schema_version: CURRENT_SCHEMA_VERSION,
    };
    let meta_toml = toml::to_string_pretty(&meta).map_err(|e| FormatError::Toml(e.to_string()))?;
    fs::write(dir.join("meta.toml"), meta_toml)?;

    Ok(())
}

/// Load a patch by full replay of `log.jsonl`. Checkpoint-accelerated load is deferred (see
/// docs/decisions.md) — correct but not yet optimized for very long logs.
pub fn load(dir: &Path) -> Result<PatchLog, FormatError> {
    let meta_text = fs::read_to_string(dir.join("meta.toml"))?;
    let meta: Meta = toml::from_str(&meta_text).map_err(|e| FormatError::Toml(e.to_string()))?;
    if !(1..=CURRENT_SCHEMA_VERSION).contains(&meta.schema_version) {
        return Err(FormatError::UnknownSchemaVersion(meta.schema_version));
    }

    let log_text = fs::read_to_string(dir.join("log.jsonl"))?;
    let mut log = PatchLog::new();
    for line in log_text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let entry: Entry = serde_json::from_str(line)?;
        log.append_entry_raw(entry);
    }
    Ok(log)
}
