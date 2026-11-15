//! The `.kablmod` package (D12 section 4.1): one stored zip holding `module.wasm`,
//! `manifest.json` and optional panel art and text files. The wasm is authoritative; the manifest
//! only repeats what the validator derived from it so a package can be described before it is
//! loaded, and a manifest that disagrees with the wasm rejects the package. A patch still embeds
//! only the wasm and its hash: the package is the distribution wrapper.
//!
//! The zip reader and writer here handle exactly the stored (method 0) subset the writer emits;
//! anything else, and any name outside the known set, is refused or ignored, never extracted.

use super::{hash_of, WasmiEngine, ABI_VERSION, MAX_BLOB_BYTES};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const WASM: &str = "module.wasm";
pub const MANIFEST: &str = "manifest.json";
/// The optional files a package may carry, besides the two required ones.
pub const OPTIONAL: [&str; 5] = [
    "panel/light.png",
    "panel/dark.png",
    "README.md",
    "LICENSE",
    "CHANGELOG.md",
];
/// A panel image is at most this big (D12 section 4.3).
pub const MAX_PNG: usize = 128 * 1024;
/// Whole-package cap: the wasm cap plus generous room for art and text.
pub const MAX_PACKAGE: usize = MAX_BLOB_BYTES + 2 * 1024 * 1024;
const MAX_TEXT: usize = 256 * 1024;

/// What publishing a package permits; shown when the first one is installed.
pub const REDISTRIBUTION: &str = "Installing a module package lets you use it in your songs. \
By publishing a package its author permits the unmodified module code to be redistributed \
inside songs that use it, so a song you save carries a copy of every module it uses.";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub version: u32,
    pub abi_major: u32,
    pub abi_minor: u32,
    pub name: String,
    pub author: String,
    /// SPDX expression. Required.
    pub licence: String,
    /// Where the source is. Required for a copyleft licence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Lower-case hex SHA-256 of `module.wasm`.
    pub sha256: String,
    /// Oldest kabl that can run it.
    pub min_kabl: String,
    #[serde(default)]
    pub description: String,
    /// Reserved: empty in v1.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Reserved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Memory, instances and declared worst-case fuel, when known.
    #[serde(default)]
    pub resources: BTreeMap<String, u64>,
}

/// Licences that require the source to stay available (prefix match on the SPDX id).
pub fn is_copyleft(spdx: &str) -> bool {
    const PREFIX: [&str; 8] = [
        "GPL", "AGPL", "LGPL", "MPL", "EPL", "EUPL", "CC-BY-SA", "CDDL",
    ];
    spdx.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '.'))
        .any(|t| PREFIX.iter().any(|p| t.starts_with(p)))
}

impl Manifest {
    /// The rules that need no wasm: required fields and the copyleft source.
    pub fn check(&self) -> Result<(), String> {
        for (v, what) in [
            (&self.id, "id"),
            (&self.name, "name"),
            (&self.author, "author"),
            (&self.licence, "licence"),
            (&self.min_kabl, "min_kabl"),
        ] {
            if v.trim().is_empty() {
                return Err(format!("the manifest has no `{what}`"));
            }
        }
        if is_copyleft(&self.licence) && self.source.as_deref().is_none_or(|s| s.trim().is_empty())
        {
            return Err(format!(
                "licence {} is copyleft: the manifest needs a `source` URL",
                self.licence
            ));
        }
        if !self.capabilities.is_empty() {
            return Err("`capabilities` is reserved and must be empty".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Package {
    pub manifest: Manifest,
    pub wasm: Vec<u8>,
    /// The optional files present, by their package path.
    pub files: BTreeMap<String, Vec<u8>>,
}

pub fn sha256_hex(b: &[u8]) -> String {
    Sha256::digest(b)
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

impl Package {
    /// Builds a package from a validated wasm and its manifest (the hash is filled in).
    pub fn new(
        mut manifest: Manifest,
        wasm: Vec<u8>,
        files: BTreeMap<String, Vec<u8>>,
    ) -> Result<Self, String> {
        manifest.sha256 = sha256_hex(&wasm);
        let p = Package {
            manifest,
            wasm,
            files,
        };
        p.check()?;
        Ok(p)
    }

    /// Everything that can be decided without running the module, plus the match against the
    /// wasm's own metadata.
    pub fn check(&self) -> Result<(), String> {
        let m = &self.manifest;
        m.check()?;
        if m.sha256 != sha256_hex(&self.wasm) {
            return Err("the manifest's sha256 does not match module.wasm".into());
        }
        if m.abi_major != ABI_VERSION {
            return Err(format!(
                "built for ABI {} but this kabl runs ABI {ABI_VERSION}",
                m.abi_major
            ));
        }
        for (name, data) in &self.files {
            if !OPTIONAL.contains(&name.as_str()) {
                return Err(format!("`{name}` is not a package file"));
            }
            let cap = if name.ends_with(".png") {
                MAX_PNG
            } else {
                MAX_TEXT
            };
            if data.len() > cap {
                return Err(format!("`{name}` is over {} KiB", cap / 1024));
            }
        }
        let meta = WasmiEngine::new().read_meta(&self.wasm)?;
        if meta.kind != m.id || meta.version != m.version || meta.name != m.name {
            return Err(format!(
                "the manifest says {} v{} \"{}\" but the wasm says {} v{} \"{}\"",
                m.id, m.version, m.name, meta.kind, meta.version, meta.name
            ));
        }
        Ok(())
    }

    /// The stored zip.
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let manifest = serde_json::to_vec_pretty(&self.manifest).map_err(|e| e.to_string())?;
        let mut files: Vec<(&str, &[u8])> = vec![(WASM, &self.wasm), (MANIFEST, &manifest)];
        files.extend(self.files.iter().map(|(n, d)| (n.as_str(), d.as_slice())));
        zip_stored(&files)
    }

    /// Reads and fully checks a package.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_PACKAGE {
            return Err("the package is over the size cap".into());
        }
        let mut all = unzip_stored(bytes)?;
        let wasm = all.remove(WASM).ok_or("the package has no module.wasm")?;
        let manifest = all
            .remove(MANIFEST)
            .ok_or("the package has no manifest.json")?;
        let manifest: Manifest =
            serde_json::from_slice(&manifest).map_err(|e| format!("bad manifest.json: {e}"))?;
        all.retain(|n, _| OPTIONAL.contains(&n.as_str()));
        let p = Package {
            manifest,
            wasm,
            files: all,
        };
        p.check()?;
        Ok(p)
    }

    /// The wasm's identity as a song embeds it.
    pub fn hash(&self) -> super::CodeHash {
        hash_of(&self.wasm)
    }
}

fn zip_stored(files: &[(&str, &[u8])]) -> Result<Vec<u8>, String> {
    let le16 = |v: usize| {
        u16::try_from(v)
            .map(u16::to_le_bytes)
            .map_err(|e| e.to_string())
    };
    let le32 = |v: usize| {
        u32::try_from(v)
            .map(u32::to_le_bytes)
            .map_err(|e| e.to_string())
    };
    let (mut out, mut central) = (vec![], vec![]);
    for (name, data) in files {
        let crc = crc32fast::hash(data).to_le_bytes();
        let (len, nlen) = (le32(data.len())?, le16(name.len())?);
        let at = le32(out.len())?;
        // Fixed DOS date 1980-01-01: the same input always gives the same bytes.
        let head = |sig: &[u8], extra: &[&[u8]]| {
            let mut h = sig.to_vec();
            h.extend(extra.concat());
            h.extend([0, 0, 0, 0, 0, 0, 33, 0]); // flags, method 0, time, date
            h.extend(crc);
            h.extend(len);
            h.extend(len);
            h.extend(nlen);
            h
        };
        out.extend(head(b"PK\x03\x04", &[&[20, 0]]));
        out.extend([0, 0]);
        out.extend(name.as_bytes());
        out.extend(*data);
        central.extend(head(b"PK\x01\x02", &[&[20, 0], &[20, 0]]));
        central.extend([0; 12]); // extra, comment, disk, internal and external attributes (8 + 4)
        central.extend(at);
        central.extend(name.as_bytes());
    }
    let (n, cl, co) = (le16(files.len())?, le32(central.len())?, le32(out.len())?);
    out.extend(central);
    out.extend(b"PK\x05\x06\0\0\0\0");
    out.extend(n);
    out.extend(n);
    out.extend(cl);
    out.extend(co);
    out.extend([0, 0]);
    Ok(out)
}

fn unzip_stored(b: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let bad = |what: &str| format!("not a valid package: {what}");
    let u16_at = |o: usize| {
        b.get(o..o + 2)
            .map(|x| usize::from(u16::from_le_bytes([x[0], x[1]])))
    };
    let u32_at = |o: usize| {
        b.get(o..o + 4)
            .map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]) as usize)
    };
    let end = b
        .len()
        .checked_sub(22)
        .and_then(|s| {
            (0..=s)
                .rev()
                .take(1 << 16)
                .find(|&i| b[i..].starts_with(b"PK\x05\x06"))
        })
        .ok_or_else(|| bad("no zip directory"))?;
    let (count, mut at) = (
        u16_at(end + 10).ok_or_else(|| bad("truncated"))?,
        u32_at(end + 16).ok_or_else(|| bad("truncated"))?,
    );
    let mut out = BTreeMap::new();
    for _ in 0..count {
        b.get(at..at + 46)
            .filter(|h| h.starts_with(b"PK\x01\x02"))
            .ok_or_else(|| bad("bad directory entry"))?;
        let (method, crc) = (u16_at(at + 10), u32_at(at + 16));
        let (size, csize) = (u32_at(at + 24).unwrap_or(0), u32_at(at + 20).unwrap_or(1));
        let (nlen, xlen, clen) = (
            u16_at(at + 28).unwrap_or(0),
            u16_at(at + 30).unwrap_or(0),
            u16_at(at + 32).unwrap_or(0),
        );
        let local = u32_at(at + 42).unwrap_or(usize::MAX);
        let name = b
            .get(at + 46..at + 46 + nlen)
            .and_then(|n| std::str::from_utf8(n).ok())
            .ok_or_else(|| bad("file name"))?
            .to_owned();
        at += 46 + nlen + xlen + clen;
        if method != Some(0) || size != csize {
            return Err(bad("only stored (uncompressed) entries are supported"));
        }
        let lh = b
            .get(local..local + 30)
            .filter(|l| l.starts_with(b"PK\x03\x04"));
        lh.ok_or_else(|| bad("bad local header"))?;
        let start = local + 30 + u16_at(local + 26).unwrap_or(0) + u16_at(local + 28).unwrap_or(0);
        let data = b.get(start..start + size).ok_or_else(|| bad("truncated"))?;
        if Some(crc32fast::hash(data) as usize) != crc {
            return Err(bad(&format!("checksum mismatch in `{name}`")));
        }
        if out.insert(name.clone(), data.to_vec()).is_some() {
            return Err(bad(&format!("`{name}` appears twice")));
        }
    }
    Ok(out)
}

impl WasmiEngine {
    /// The text of the module's `kabl.notices` custom section, if it has one (shown in module info).
    pub fn notices(&self, wasm: &[u8]) -> Option<String> {
        let module = wasmi::Module::new(&self.engine, wasm).ok()?;
        let s = module
            .custom_sections()
            .find(|s| s.name() == "kabl.notices")?;
        std::str::from_utf8(s.data()).ok().map(str::to_owned)
    }
}
