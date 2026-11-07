use crate::embedded::{EmbedError, Embedded, MAX_CODE_BLOB_BYTES};
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
    /// A custom-code blob over the 2 MiB cap, or otherwise unusable embedded code.
    Code(crate::embedded::EmbedError),
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

/// Skippable frame (after the header) that carries a sound's `sound.toml` text. Optional: a file
/// without it (every file before it existed) reads as no metadata, and an older reader skips it
/// as it skips any skippable frame, so the container version stays 1.
const SKIPPABLE_META: u32 = 0x184D_2A51;
/// Skippable frames (after the header) that carry embedded code (D11): `2A52` one wasm blob
/// (hash, version, module id, bytes), `2A53` one instance-to-hash reference, `2A54` one state
/// kept from an older module version. All optional and skippable, like the metadata frame.
const SKIPPABLE_CODE: u32 = 0x184D_2A52;
const SKIPPABLE_CODE_REF: u32 = 0x184D_2A53;
const SKIPPABLE_CODE_STATE: u32 = 0x184D_2A54;
/// Cap on the metadata text.
const MAX_META_BYTES: usize = 1024 * 1024;

/// Write `log` as a `.kabl` file: a skippable header frame (`KABL`, container version, schema
/// version, dictionary id 1) and one zstd level-19 frame using dictionary 1, with a content checksum, holding the
/// log as JSON lines (`inverse` kept). The checkpoint is not stored; load replays the log.
pub fn write_kabl<W: Write>(w: W, log: &PatchLog) -> Result<(), FormatError> {
    write_kabl_meta(w, log, None)
}

/// [`write_kabl`] with a sound's metadata (`sound.toml` text) in its own skippable frame.
pub fn write_kabl_meta<W: Write>(
    w: W,
    log: &PatchLog,
    meta: Option<&str>,
) -> Result<(), FormatError> {
    write_kabl_full(w, log, meta, None)
}

fn write_frame<W: Write>(w: &mut W, magic: u32, payload: &[u8]) -> Result<(), FormatError> {
    w.write_all(&magic.to_le_bytes())?;
    w.write_all(&(payload.len() as u32).to_le_bytes())?;
    w.write_all(payload)?;
    Ok(())
}

/// [`write_kabl_meta`] with embedded code: one frame per blob, per reference and per kept
/// state. A blob over the 2 MiB cap is refused; blobs nothing references are still written
/// (call [`Embedded::prune_unreferenced`] first to drop them).
pub fn write_kabl_full<W: Write>(
    mut w: W,
    log: &PatchLog,
    meta: Option<&str>,
    code: Option<&Embedded>,
) -> Result<(), FormatError> {
    if let Some(c) = code {
        for (_, b) in c.blobs() {
            if b.bytes.len() > MAX_CODE_BLOB_BYTES {
                return Err(FormatError::Code(EmbedError::TooLarge {
                    size: b.bytes.len(),
                    cap: MAX_CODE_BLOB_BYTES,
                }));
            }
        }
    }
    if meta.is_some_and(|m| m.len() > MAX_META_BYTES) {
        return Err(FormatError::TooLarge(MAX_META_BYTES));
    }
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
    if let Some(m) = meta {
        write_frame(&mut w, SKIPPABLE_META, m.as_bytes())?;
    }
    if let Some(c) = code {
        for (h, b) in c.blobs() {
            write_frame(&mut w, SKIPPABLE_CODE, &Embedded::blob_payload(h, b))?;
        }
        for (i, h) in c.refs() {
            write_frame(&mut w, SKIPPABLE_CODE_REF, &Embedded::ref_payload(i, h))?;
        }
        for k in c.kept_states() {
            write_frame(&mut w, SKIPPABLE_CODE_STATE, &Embedded::kept_payload(k))?;
        }
    }
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
    read_kabl_meta_limited(r, max).map(|(log, _)| log)
}

/// [`read_kabl`] that also returns the metadata frame's text, when the file has one.
pub fn read_kabl_meta<R: Read>(r: R) -> Result<(PatchLog, Option<String>), FormatError> {
    read_kabl_meta_limited(r, MAX_KABL_BYTES)
}

fn read_kabl_meta_limited<R: Read>(
    r: R,
    max: usize,
) -> Result<(PatchLog, Option<String>), FormatError> {
    read_kabl_full_limited(r, max).map(|(log, meta, _)| (log, meta))
}

/// [`read_kabl_meta`] that also returns the embedded code (empty for a file without any).
pub fn read_kabl_full<R: Read>(r: R) -> Result<(PatchLog, Option<String>, Embedded), FormatError> {
    read_kabl_full_limited(r, MAX_KABL_BYTES)
}

fn read_kabl_full_limited<R: Read>(
    r: R,
    max: usize,
) -> Result<(PatchLog, Option<String>, Embedded), FormatError> {
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
        // Literal ids: a later dictionary adds an arm and bumps KABL_DICT_ID (the writer's id);
        // it must never change what id 1 means.
        1 => KABL_DICT_1,
        other => return Err(FormatError::UnknownDictionary(other)),
    };
    let corrupt = |e: io::Error| FormatError::Corrupt(e.to_string());
    let mut body = &bytes[20..];
    let mut meta = None;
    let mut code = Embedded::new();
    // Skippable frames in any order; ones this build does not know are passed over.
    while body.len() >= 8 {
        let magic = u32::from_le_bytes(body[..4].try_into().unwrap());
        if !(0x184D_2A50..=0x184D_2A5F).contains(&magic) {
            break;
        }
        let n = u32::from_le_bytes(body[4..8].try_into().unwrap()) as usize;
        if matches!(
            magic,
            SKIPPABLE_CODE | SKIPPABLE_CODE_REF | SKIPPABLE_CODE_STATE
        ) && n > MAX_CODE_BLOB_BYTES + (1 << 17)
        {
            return Err(FormatError::Code(EmbedError::TooLarge {
                size: n,
                cap: MAX_CODE_BLOB_BYTES,
            }));
        }
        let cut = || FormatError::Corrupt("skippable frame is cut".into());
        let payload = body.get(8..8usize.saturating_add(n)).ok_or_else(cut)?;
        match magic {
            SKIPPABLE_META => {
                if n > MAX_META_BYTES || meta.is_some() {
                    return Err(FormatError::Corrupt(
                        "metadata frame is cut or too large".into(),
                    ));
                }
                meta = Some(
                    String::from_utf8(payload.to_vec())
                        .map_err(|e| FormatError::Corrupt(e.to_string()))?,
                );
            }
            SKIPPABLE_CODE | SKIPPABLE_CODE_REF | SKIPPABLE_CODE_STATE => {
                let r = match magic {
                    SKIPPABLE_CODE => code.read_blob(payload),
                    SKIPPABLE_CODE_REF => code.read_ref(payload),
                    _ => code.read_kept(payload),
                };
                r.map_err(FormatError::Corrupt)?;
            }
            _ => {}
        }
        body = &body[8 + n..];
    }
    let mut dec = zstd::stream::read::Decoder::with_dictionary(body, dict).map_err(corrupt)?;
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
    Ok((replay(schema, &text)?, meta, code))
}

/// Save `log` to `path` (`name.kabl`): write `name.kabl.tmp`, fsync, then rename over `path`.
pub fn save_kabl(path: &Path, log: &PatchLog) -> Result<(), FormatError> {
    save_kabl_meta(path, log, None)
}

/// [`save_kabl`] with a sound's metadata (`sound.toml` text).
pub fn save_kabl_meta(path: &Path, log: &PatchLog, meta: Option<&str>) -> Result<(), FormatError> {
    save_kabl_full(path, log, meta, None)
}

/// [`save_kabl_meta`] with embedded code.
pub fn save_kabl_full(
    path: &Path,
    log: &PatchLog,
    meta: Option<&str>,
    code: Option<&Embedded>,
) -> Result<(), FormatError> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let result = (|| {
        let mut f = fs::File::create(&tmp)?;
        write_kabl_full(io::BufWriter::new(&mut f), log, meta, code)?;
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

/// [`load_kabl`] that also returns the metadata text, when the file has it.
pub fn load_kabl_meta(path: &Path) -> Result<(PatchLog, Option<String>), FormatError> {
    read_kabl_meta(fs::File::open(path)?)
}

/// [`load_kabl_meta`] that also returns the embedded code.
pub fn load_kabl_full(path: &Path) -> Result<(PatchLog, Option<String>, Embedded), FormatError> {
    read_kabl_full(fs::File::open(path)?)
}

/// Pack a patch folder (`meta.toml`, `log.jsonl`, and `sound.toml` when present) into one
/// `.kabl` file at `out`. The one packer behind `kabl pack`, the build's factory packing and
/// the packaging script.
pub fn pack_dir(dir: &Path, out: &Path) -> Result<(), FormatError> {
    let log = load(dir)?;
    let meta = fs::read_to_string(dir.join("sound.toml")).ok();
    save_kabl_meta(out, &log, meta.as_deref())
}

/// Pack every patch folder under `src` (a folder holding `meta.toml`) into `dst`, keeping the
/// relative layout: `src/a/b` becomes `dst/a/b.kabl`. Returns how many it packed.
pub fn pack_tree(src: &Path, dst: &Path) -> Result<usize, FormatError> {
    let mut n = 0;
    for e in fs::read_dir(src)?.flatten() {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        let name = e.file_name();
        if p.join("meta.toml").is_file() {
            fs::create_dir_all(dst)?;
            let mut out = dst.join(&name).into_os_string();
            out.push(".kabl");
            pack_dir(&p, Path::new(&out))?;
            n += 1;
        } else {
            n += pack_tree(&p, &dst.join(&name))?;
        }
    }
    Ok(n)
}
