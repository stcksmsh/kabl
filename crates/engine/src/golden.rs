//! Shared plumbing for the golden-render tests and tools: finding patch folders and hashing
//! rendered audio. Not used by the engine itself.

use std::path::{Path, PathBuf};

/// Push every patch folder (one holding `log.jsonl` and `meta.toml`) found under `root`, looking
/// at most `depth` directory levels down. A patch folder is not searched further.
pub fn patch_dirs(root: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if root.join("log.jsonl").is_file() && root.join("meta.toml").is_file() {
        out.push(root.to_path_buf());
        return;
    }
    if depth == 0 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(root) else {
        return;
    };
    for e in rd.flatten() {
        if e.path().is_dir() {
            patch_dirs(&e.path(), out, depth - 1);
        }
    }
}

/// FNV-1a over the little-endian bit patterns of the samples fed to it.
pub struct SampleHash(u64);

impl Default for SampleHash {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl SampleHash {
    pub fn feed(&mut self, samples: &[f32]) {
        for s in samples {
            for b in s.to_bits().to_le_bytes() {
                self.0 = (self.0 ^ b as u64).wrapping_mul(0x0100_0000_01b3);
            }
        }
    }

    pub fn hex(&self) -> String {
        format!("{:016x}", self.0)
    }
}
