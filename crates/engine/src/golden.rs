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
/// It also tracks finiteness, peak and energy, for the platforms where the hash itself is not
/// comparable (see [`SampleHash::assert_clean`]).
pub struct SampleHash {
    hash: u64,
    finite: bool,
    peak: f32,
    sum_sq: f64,
    count: usize,
}

impl Default for SampleHash {
    fn default() -> Self {
        Self {
            hash: 0xcbf2_9ce4_8422_2325,
            finite: true,
            peak: 0.0,
            sum_sq: 0.0,
            count: 0,
        }
    }
}

impl SampleHash {
    pub fn feed(&mut self, samples: &[f32]) {
        for s in samples {
            for b in s.to_bits().to_le_bytes() {
                self.hash = (self.hash ^ b as u64).wrapping_mul(0x0100_0000_01b3);
            }
            self.finite &= s.is_finite();
            self.peak = self.peak.max(s.abs());
            self.sum_sq += f64::from(*s).powi(2);
            self.count += 1;
        }
    }

    pub fn hex(&self) -> String {
        format!("{:016x}", self.hash)
    }

    /// Bit-exact hashes are a Linux reference: other platforms' libm differs in the last bit.
    /// There the render is only checked for being finite, within full scale, and not silent.
    pub fn assert_clean(&self, what: &str, max_peak: f32) {
        let db = 10.0 * (self.sum_sq / self.count.max(1) as f64).log10();
        assert!(self.finite, "{what}: non-finite samples");
        assert!(
            self.peak <= max_peak,
            "{what}: over full scale, peak {}",
            self.peak
        );
        assert!(db > -60.0, "{what}: near silent, {db:.1} dB");
    }
}

/// Render `blocks` blocks of the golden recipe and hash them: a three-note chord on voices 0 to 2
/// from block 0, released at `release`. `each_block` runs before every block, for extra events.
pub fn chord_hash(
    c: &mut crate::compile::CompiledPatch,
    blocks: usize,
    release: usize,
    each_block: impl FnMut(&mut crate::compile::CompiledPatch, usize),
) -> String {
    chord_render(c, blocks, release, each_block).hex()
}

/// [`chord_hash`], keeping the whole [`SampleHash`] so a caller can check the render's level.
pub fn chord_render(
    c: &mut crate::compile::CompiledPatch,
    blocks: usize,
    release: usize,
    mut each_block: impl FnMut(&mut crate::compile::CompiledPatch, usize),
) -> SampleHash {
    let mut h = SampleHash::default();
    for b in 0..blocks {
        if b == 0 {
            for (v, n) in [(0usize, 0.0f32), (1, 4.0), (2, 7.0)] {
                c.note_on(v, n, 0.8);
            }
        }
        if b == release {
            for v in 0..3 {
                c.note_off(v);
            }
        }
        each_block(c, b);
        c.process_block();
        h.feed(c.left());
        h.feed(c.right());
    }
    h
}
