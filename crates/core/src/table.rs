//! User wavetables embedded in the patch, so a saved sound carries its tables like a composite
//! carries its artwork (`panel.rs`): the bytes live in the document, the name is metadata and
//! never a path. The bytes are a canonical .wav; `kabl-modules` owns decoding and import.
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Slots a patch can fill; a wavetable module's `user` param picks one (0 = factory table).
pub const MAX_TABLES: u64 = 8;
/// 64 frames of 2048 16-bit samples plus a header, with slack.
pub const MAX_TABLE_BYTES: usize = 64 * 2048 * 2 + 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Table {
    pub name: String,
    #[serde(with = "b64")]
    pub wav: Vec<u8>,
}

impl Table {
    pub fn validate(&self) -> Result<(), String> {
        let ok_name = !self.name.is_empty()
            && self.name.len() <= 80
            && self.name != "."
            && self.name != ".."
            && self
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b));
        if !ok_name {
            return Err("Table name must be a simple local filename; paths are forbidden".into());
        }
        let b = &self.wav;
        if b.len() < 44 || b.len() > MAX_TABLE_BYTES || &b[..4] != b"RIFF" || &b[8..12] != b"WAVE" {
            return Err("Table must be a .wav of at most 64 frames of 2048 samples".into());
        }
        Ok(())
    }
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

mod b64 {
    use super::*;

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for c in bytes.chunks(3) {
            let n = (c[0] as u32) << 16
                | (*c.get(1).unwrap_or(&0) as u32) << 8
                | *c.get(2).unwrap_or(&0) as u32;
            for k in 0..4 {
                if k <= c.len() {
                    out.push(ALPHABET[(n >> (18 - 6 * k) & 63) as usize] as char);
                } else {
                    out.push('=');
                }
            }
        }
        s.serialize_str(&out)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(d)?;
        let bad = || serde::de::Error::custom("Invalid base64 table data");
        if text.len() % 4 != 0 {
            return Err(bad());
        }
        let mut out = Vec::with_capacity(text.len() / 4 * 3);
        let chunks = text.as_bytes().chunks(4);
        let last = chunks.len().saturating_sub(1);
        for (i, c) in chunks.enumerate() {
            let mut n = 0u32;
            let mut pad = 0;
            for &ch in c {
                n <<= 6;
                if ch == b'=' && i == last {
                    pad += 1;
                } else if pad > 0 {
                    return Err(bad());
                } else {
                    n |= ALPHABET.iter().position(|&a| a == ch).ok_or_else(bad)? as u32;
                }
            }
            if pad > 2 {
                return Err(bad());
            }
            out.extend_from_slice(&n.to_be_bytes()[1..4 - pad]);
        }
        Ok(out)
    }
}
