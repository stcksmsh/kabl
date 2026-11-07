use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::log::PatchLog;
use crate::op::Entry;

/// v2 adds `PortRef::Param` (modulation routes), `Op::UnsetParam` and `Op::Group`; v3 adds
/// `Op::SetLabel`; v4 adds `Op::SetComposite`; v5 adds `Op::SetTable` (embedded wavetables); v6
/// gives cable params a meaning for functional cables (`length`, `s1..`, `prob`, `r1..`; see the
/// `kabl-cables` crate); v7 adds `PortRef::CableParam` (a cable's morph, probability or glide as
/// a modulation destination). Each is a strict superset of the one before, so older files load
/// unchanged. An older build refuses a newer file instead of playing its functional cables, or
/// the routes into them, as plain ones.
///
/// v8 is not a superset: stereo effects, `out`, `audio.in`, `pan` and `delay` have one width-2
/// port (`in`, `out`) where v7 files name the pair `in_l`/`in_r`, `left`/`right`. A v7 build
/// refuses a v8 file. Files below v8 name the old ports; the migration that rewrites them lands
/// with step 2b of `docs/stereo/DESIGN.md`.
pub const CURRENT_SCHEMA_VERSION: u32 = 8;

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
    Composite(String),
    /// A `.kabl` file that does not start with the `KABL` header frame.
    BadMagic,
    /// A `.kabl` container version this build does not know.
    UnknownContainerVersion(u16),
    /// The header names a dictionary this build does not have (it knows 0, none, and 1).
    UnknownDictionary(u32),
    /// Truncated, damaged or failed-checksum `.kabl` data.
    Corrupt(String),
    /// The `.kabl` file or its decoded log passes the cap (a decompression-bomb guard).
    TooLarge(usize),
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

    replay(
        meta.schema_version,
        &fs::read_to_string(dir.join("log.jsonl"))?,
    )
}

/// Replay JSON-lines log text written at `schema_version` into a patch.
fn replay(schema_version: u32, log_text: &str) -> Result<PatchLog, FormatError> {
    let mut log = PatchLog::new();
    for line in log_text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let entry: Entry = serde_json::from_str(line)?;
        log.append_entry_raw(entry);
    }
    if schema_version < 4 && !log.state().composites.is_empty() {
        return Err(FormatError::UnknownSchemaVersion(4));
    }
    if schema_version < 5 && !log.state().tables.is_empty() {
        return Err(FormatError::UnknownSchemaVersion(5));
    }
    log.floor_after_migration();
    if schema_version < 8 {
        crate::migrate::migrate_v7_to_v8(&mut log);
    }
    crate::composite::validate(log.state()).map_err(FormatError::Composite)?;
    Ok(log)
}

/// Container version of the `.kabl` file. Bumped when the frame layout changes.
pub const KABL_CONTAINER_VERSION: u16 = 1;
/// Cap on a `.kabl` file and on its decoded log: a decompression bound. The largest factory
/// log is well under 1 MB.
pub const MAX_KABL_BYTES: usize = 64 * 1024 * 1024;
const SKIPPABLE_HEADER: u32 = 0x184D_2A50;
const HEADER_PAYLOAD: u32 = 12;
const KABL_ZSTD_LEVEL: i32 = 19;
/// Dictionary 1, trained offline by `docs/format/train_dict.sh` from the factory logs. Permanent:
/// files written with id 1 need these exact bytes, so a better dictionary gets a new id and this
/// one stays in the binary.
pub const KABL_DICT_1: &[u8] = include_bytes!("../assets/kabl-dict-1.zstdict");
/// The dictionary id `write_kabl` writes.
pub const KABL_DICT_ID: u32 = 1;

/// Write `log` as a `.kabl` file: a skippable header frame (`KABL`, container version, schema
/// version, dictionary id 1) and one zstd level-19 frame using dictionary 1, with a content checksum, holding the
/// log as JSON lines (`inverse` kept). The checkpoint is not stored; load replays the log.
pub fn write_kabl<W: Write>(mut w: W, log: &PatchLog) -> Result<(), FormatError> {
    let mut lines = String::new();
    for entry in log.entries() {
        lines.push_str(&serde_json::to_string(entry)?);
        lines.push('\n');
    }
    w.write_all(&SKIPPABLE_HEADER.to_le_bytes())?;
    w.write_all(&HEADER_PAYLOAD.to_le_bytes())?;
    w.write_all(b"KABL")?;
    w.write_all(&KABL_CONTAINER_VERSION.to_le_bytes())?;
    w.write_all(&(CURRENT_SCHEMA_VERSION as u16).to_le_bytes())?;
    w.write_all(&KABL_DICT_ID.to_le_bytes())?;
    let mut enc = zstd::stream::write::Encoder::with_dictionary(w, KABL_ZSTD_LEVEL, KABL_DICT_1)?;
    enc.include_checksum(true)?;
    enc.write_all(lines.as_bytes())?;
    // The writer comes back so a buffered writer's flush error is not lost in its drop.
    enc.finish()?.flush()?;
    Ok(())
}

/// Read a `.kabl` file and replay its log. Rejects a bad magic, an unknown container or schema
/// version, an unknown dictionary, damaged or truncated data (zstd's content checksum) and a
/// file or decoded log over [`MAX_KABL_BYTES`].
pub fn read_kabl<R: Read>(r: R) -> Result<PatchLog, FormatError> {
    read_kabl_limited(r, MAX_KABL_BYTES)
}

/// [`read_kabl`] with an explicit cap on the encoded and the decoded size.
pub fn read_kabl_limited<R: Read>(r: R, max: usize) -> Result<PatchLog, FormatError> {
    let mut bytes = Vec::new();
    r.take(max as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > max {
        return Err(FormatError::TooLarge(max));
    }
    let head = bytes.get(..20).ok_or(FormatError::BadMagic)?;
    let u32_at = |i: usize| u32::from_le_bytes(head[i..i + 4].try_into().unwrap());
    let u16_at = |i: usize| u16::from_le_bytes(head[i..i + 2].try_into().unwrap());
    if u32_at(0) != SKIPPABLE_HEADER || u32_at(4) != HEADER_PAYLOAD || &head[8..12] != b"KABL" {
        return Err(FormatError::BadMagic);
    }
    if u16_at(12) != KABL_CONTAINER_VERSION {
        return Err(FormatError::UnknownContainerVersion(u16_at(12)));
    }
    let schema = u32::from(u16_at(14));
    if !(1..=CURRENT_SCHEMA_VERSION).contains(&schema) {
        return Err(FormatError::UnknownSchemaVersion(schema));
    }
    let dict: &[u8] = match u32_at(16) {
        0 => &[],
        KABL_DICT_ID => KABL_DICT_1,
        other => return Err(FormatError::UnknownDictionary(other)),
    };
    let corrupt = |e: io::Error| FormatError::Corrupt(e.to_string());
    let mut dec =
        zstd::stream::read::Decoder::with_dictionary(&bytes[20..], dict).map_err(corrupt)?;
    let mut text = Vec::new();
    // One byte past the cap tells a bomb from a log that fits exactly.
    (&mut dec)
        .take(max as u64 + 1)
        .read_to_end(&mut text)
        .map_err(corrupt)?;
    if text.len() > max {
        return Err(FormatError::TooLarge(max));
    }
    let text = String::from_utf8(text).map_err(|e| FormatError::Corrupt(e.to_string()))?;
    replay(schema, &text)
}

/// Save `log` to `path` (`name.kabl`): write `name.kabl.tmp`, fsync, then rename over `path`.
pub fn save_kabl(path: &Path, log: &PatchLog) -> Result<(), FormatError> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let result = (|| {
        let mut f = fs::File::create(&tmp)?;
        write_kabl(io::BufWriter::new(&mut f), log)?;
        f.sync_all()?;
        fs::rename(&tmp, path)?;
        // So the rename survives a power loss.
        #[cfg(unix)]
        if let Some(dir) = path.parent() {
            fs::File::open(if dir.as_os_str().is_empty() {
                Path::new(".")
            } else {
                dir
            })?
            .sync_all()?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Load a `.kabl` file by replaying its log.
pub fn load_kabl(path: &Path) -> Result<PatchLog, FormatError> {
    read_kabl(fs::File::open(path)?)
}
