//! Band-limited wavetables: import, canonical storage, mipmaps and the factory set.
//!
//! A table is 1-64 frames of 2048 samples, each frame one cycle limited to 512 harmonics. The
//! canonical form is a 16-bit mono .wav (the same bytes `kabl_core::Table` embeds in a patch
//! and that Serum-style tools read). Everything here runs on the control thread; the audio
//! thread only calls `WaveTable::level_for` and `WaveTable::read` on a table built beforehand.
//!
//! Mipmaps: level `k` keeps `512 >> k` harmonics, stored at 16 samples per top harmonic (never
//! under 512), so linear interpolation of the stored samples puts its images at least 47 dB
//! under the harmonic that makes them (the saw's 1/h roll-off adds to that). The oscillator picks the
//! richest level whose top harmonic stays under Nyquist, so a table never aliases by itself.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, Weak};

pub const FRAME: usize = 2048;
pub const MAX_FRAMES: usize = 64;
const MAX_HARMONICS: usize = FRAME / 4;
pub const LEVELS: usize = 10;
/// Longest frame `import_wav` resamples (the resampling costs frame length x 512).
pub const MAX_FRAME_SAMPLES: usize = 32768;
/// Largest .wav accepted by `import_wav` (bytes).
pub const MAX_IMPORT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableError {
    NotWav,
    Unsupported(String),
    Truncated,
    TooShort,
    TooLarge,
    TooManyFrames(usize),
    /// A frame (or a whole file taken as one cycle) longer than `MAX_FRAME_SAMPLES`.
    FrameTooLong(usize),
    Silent,
    NotFinite,
    NotCanonical,
}

impl std::fmt::Display for TableError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            TableError::NotWav => write!(f, "not a RIFF/WAVE file"),
            TableError::Unsupported(s) => write!(f, "unsupported .wav: {s}"),
            TableError::Truncated => write!(f, "the .wav is cut short"),
            TableError::TooShort => write!(f, "fewer than 16 samples"),
            TableError::TooLarge => write!(f, "larger than {} MB", MAX_IMPORT_BYTES >> 20),
            TableError::TooManyFrames(n) => {
                write!(f, "{n} frames; a table holds at most {MAX_FRAMES}")
            }
            TableError::FrameTooLong(n) => write!(
                f,
                "a {n}-sample frame; use a multiple of 2048 samples, or one cycle of at most {MAX_FRAME_SAMPLES}"
            ),
            TableError::Silent => write!(f, "the table is silent"),
            TableError::NotFinite => write!(f, "contains NaN or infinite samples"),
            TableError::NotCanonical => {
                write!(f, "not a kabl table (16-bit mono, 2048-sample frames)")
            }
        }
    }
}

impl std::error::Error for TableError {}

struct Level {
    len: usize,
    /// `frames * (len + 1)` samples: each frame repeats its first sample at the end, so
    /// interpolation never wraps.
    data: Box<[f32]>,
}

pub struct WaveTable {
    frames: usize,
    levels: Vec<Level>,
}

impl WaveTable {
    pub fn frames(&self) -> usize {
        self.frames
    }

    /// Builds the mipmaps from `frames` (each `FRAME` samples; DC and harmonics above 512 are
    /// dropped).
    pub fn from_frames(frames: &[Vec<f32>]) -> WaveTable {
        let spectra: Vec<(Vec<f64>, Vec<f64>)> = frames
            .iter()
            .map(|f| {
                let mut re: Vec<f64> = f.iter().map(|&v| v as f64).collect();
                let mut im = vec![0.0; FRAME];
                fft(&mut re, &mut im, false);
                (re, im)
            })
            .collect();
        let levels = (0..LEVELS)
            .map(|k| {
                let h_max = MAX_HARMONICS >> k;
                let len = (16 * h_max).max(512);
                let stride = len + 1;
                let mut data = vec![0.0f32; frames.len() * stride];
                let scale = len as f64 / FRAME as f64;
                for (fi, (sre, sim)) in spectra.iter().enumerate() {
                    let mut re = vec![0.0; len];
                    let mut im = vec![0.0; len];
                    for h in 1..=h_max {
                        re[h] = sre[h] * scale;
                        im[h] = sim[h] * scale;
                        re[len - h] = sre[h] * scale;
                        im[len - h] = -sim[h] * scale;
                    }
                    fft(&mut re, &mut im, true);
                    let dst = &mut data[fi * stride..(fi + 1) * stride];
                    for i in 0..len {
                        dst[i] = re[i] as f32;
                    }
                    dst[len] = dst[0];
                }
                Level {
                    len,
                    data: data.into_boxed_slice(),
                }
            })
            .collect();
        WaveTable {
            frames: frames.len(),
            levels,
        }
    }

    /// The richest level whose top harmonic stays under Nyquist at phase increment `dt`.
    #[inline]
    pub fn level_for(&self, dt: f32) -> usize {
        let allowed = (0.5 / dt.max(1e-6)) as usize;
        let mut k = 0;
        while k + 1 < LEVELS && (MAX_HARMONICS >> k) > allowed {
            k += 1;
        }
        k
    }

    /// The table at frame position `pos` (0..=1 across all frames, linear between frames) and
    /// cycle phase `phase` (0..1), read from mipmap `level`.
    #[inline]
    pub fn read(&self, level: usize, pos: f32, phase: f32) -> f32 {
        let lv = &self.levels[level];
        let stride = lv.len + 1;
        // NaN (a NaN on a CV input) reads the first frame.
        let pos = if pos.is_nan() {
            0.0
        } else {
            pos.clamp(0.0, 1.0)
        };
        let fpos = pos * (self.frames - 1) as f32;
        let f0 = fpos as usize;
        let fr = fpos - f0 as f32;
        let f1 = (f0 + 1).min(self.frames - 1);
        let x = phase * lv.len as f32;
        let i = (x as usize).min(lv.len - 1);
        let t = x - i as f32;
        let a = &lv.data[f0 * stride + i..];
        let v0 = a[0] + (a[1] - a[0]) * t;
        if fr == 0.0 || f1 == f0 {
            return v0;
        }
        let b = &lv.data[f1 * stride + i..];
        let v1 = b[0] + (b[1] - b[0]) * t;
        v0 + (v1 - v0) * fr
    }

    /// The table from canonical bytes (what `import_wav` returns), shared with every other
    /// caller that decoded the same bytes.
    pub fn from_canonical_cached(bytes: &[u8]) -> Result<Arc<WaveTable>, TableError> {
        type Key = (usize, u64, u64);
        static CACHE: OnceLock<Mutex<HashMap<Key, Weak<WaveTable>>>> = OnceLock::new();
        use std::hash::{Hash, Hasher};
        // Length plus two differently seeded hashes: a wrong-table hit needs all three to collide.
        let hash = |salt: u8| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            salt.hash(&mut h);
            bytes.hash(&mut h);
            h.finish()
        };
        let key = (bytes.len(), hash(0), hash(1));
        let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
        if let Some(t) = cache.get(&key).and_then(Weak::upgrade) {
            return Ok(t);
        }
        let t = Arc::new(WaveTable::from_frames(&decode_canonical(bytes)?));
        cache.retain(|_, w| w.strong_count() > 0);
        cache.insert(key, Arc::downgrade(&t));
        Ok(t)
    }
}

/// Complex radix-2 FFT, in place. The inverse includes the 1/n factor.
fn fft(re: &mut [f64], im: &mut [f64], inverse: bool) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let sign = if inverse { 1.0 } else { -1.0 };
    let mut len = 2;
    while len <= n {
        let ang = sign * 2.0 * std::f64::consts::PI / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (s, c) = (ang * k as f64).sin_cos();
                let (a, b) = (start + k, start + k + len / 2);
                let (tr, ti) = (re[b] * c - im[b] * s, re[b] * s + im[b] * c);
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
    if inverse {
        for v in re.iter_mut().chain(im.iter_mut()) {
            *v /= n as f64;
        }
    }
}

struct Wav {
    samples: Vec<f64>,
    /// Frame length from a `clm ` chunk, if the file names one.
    frame_len: Option<usize>,
    bits: u16,
    channels: u16,
}

fn le16(b: &[u8], at: usize) -> Result<u16, TableError> {
    b.get(at..at + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or(TableError::Truncated)
}

fn le32(b: &[u8], at: usize) -> Result<u32, TableError> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or(TableError::Truncated)
}

/// Mono samples (channels averaged) of PCM 8/16/24/32-bit or float 32/64-bit .wav bytes.
fn parse_wav(b: &[u8]) -> Result<Wav, TableError> {
    if b.len() > MAX_IMPORT_BYTES {
        return Err(TableError::TooLarge);
    }
    if b.len() < 12 || &b[..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Err(TableError::NotWav);
    }
    let (mut fmt, mut data, mut frame_len) = (None, None, None);
    let mut at = 12;
    while at + 8 <= b.len() {
        let id = &b[at..at + 4];
        let size = le32(b, at + 4)? as usize;
        let body = at + 8;
        let end = body.checked_add(size).ok_or(TableError::Truncated)?;
        // A data chunk may be cut short (streamed files); every other chunk must be whole.
        if end > b.len() && id != b"data" {
            return Err(TableError::Truncated);
        }
        let chunk = &b[body..end.min(b.len())];
        match id {
            b"fmt " => fmt = Some(chunk),
            b"data" => data = Some(chunk),
            b"clm " if chunk.starts_with(b"<!>") => {
                let digits: String = chunk[3..]
                    .iter()
                    .take_while(|c| c.is_ascii_digit())
                    .map(|&c| c as char)
                    .collect();
                frame_len = digits.parse().ok();
            }
            _ => {}
        }
        at = end + (size & 1);
    }
    let fmt = fmt.ok_or(TableError::NotWav)?;
    let data = data.ok_or(TableError::NotWav)?;
    if fmt.len() < 16 {
        return Err(TableError::Truncated);
    }
    let mut tag = le16(fmt, 0)?;
    let channels = le16(fmt, 2)?;
    let bits = le16(fmt, 14)?;
    if tag == 0xFFFE && fmt.len() >= 26 {
        tag = le16(fmt, 24)?;
    }
    if channels == 0 {
        return Err(TableError::Unsupported("zero channels".into()));
    }
    let width = (bits / 8) as usize;
    let sample = |s: &[u8]| -> Result<f64, TableError> {
        Ok(match (tag, bits) {
            (1, 8) => (s[0] as f64 - 128.0) / 128.0,
            (1, 16) => i16::from_le_bytes([s[0], s[1]]) as f64 / 32768.0,
            (1, 24) => (i32::from_le_bytes([0, s[0], s[1], s[2]]) >> 8) as f64 / 8388608.0,
            (1, 32) => i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64 / 2147483648.0,
            (3, 32) => f32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64,
            (3, 64) => f64::from_le_bytes(s[..8].try_into().unwrap()),
            _ => {
                return Err(TableError::Unsupported(format!(
                    "format {tag} with {bits} bits"
                )))
            }
        })
    };
    let frame_bytes = width * channels as usize;
    let mut samples = Vec::with_capacity(data.len() / frame_bytes.max(1));
    if width == 0 {
        return Err(TableError::Unsupported(format!("{bits} bits")));
    }
    for fr in data.chunks_exact(frame_bytes) {
        let mut sum = 0.0;
        for s in fr.chunks_exact(width) {
            sum += sample(s)?;
        }
        let v = sum / channels as f64;
        if !v.is_finite() {
            return Err(TableError::NotFinite);
        }
        samples.push(v);
    }
    Ok(Wav {
        samples,
        frame_len,
        bits,
        channels,
    })
}

/// One cycle of any length resampled to `FRAME` samples, band-limited to 512 harmonics, DC
/// removed.
fn resample_cycle(cycle: &[f64]) -> Vec<f64> {
    let n = cycle.len();
    let h_max = (n / 2).saturating_sub(1).min(MAX_HARMONICS);
    let cos: Vec<f64> = (0..n)
        .map(|k| (2.0 * std::f64::consts::PI * k as f64 / n as f64).cos())
        .collect();
    let sin: Vec<f64> = (0..n)
        .map(|k| (2.0 * std::f64::consts::PI * k as f64 / n as f64).sin())
        .collect();
    let mut re = vec![0.0; FRAME];
    let mut im = vec![0.0; FRAME];
    for h in 1..=h_max {
        let (mut a, mut b) = (0.0, 0.0);
        for (i, &x) in cycle.iter().enumerate() {
            let k = (h * i) % n;
            a += x * cos[k];
            b -= x * sin[k];
        }
        re[h] = a / n as f64 * FRAME as f64;
        im[h] = b / n as f64 * FRAME as f64;
        re[FRAME - h] = re[h];
        im[FRAME - h] = -im[h];
    }
    fft(&mut re, &mut im, true);
    re
}

/// Reads any supported .wav into canonical table bytes: 1-64 frames of 2048 samples, 16-bit
/// mono, peak-normalised. A `clm ` chunk (Serum convention) names the frame length; without
/// one, a file whose length is a multiple of 2048 is that many frames and anything else is one
/// cycle, resampled. Pure computation: no I/O, no state.
pub fn import_wav(bytes: &[u8]) -> Result<Vec<u8>, TableError> {
    let wav = parse_wav(bytes)?;
    let n = wav.samples.len();
    if n < 16 {
        return Err(TableError::TooShort);
    }
    let frame_len = match wav.frame_len {
        Some(f) if (16..=n).contains(&f) && n % f == 0 => f,
        _ if n >= FRAME && n % FRAME == 0 => FRAME,
        _ => n,
    };
    if frame_len > MAX_FRAME_SAMPLES {
        return Err(TableError::FrameTooLong(frame_len));
    }
    let count = n / frame_len;
    if count > MAX_FRAMES {
        return Err(TableError::TooManyFrames(count));
    }
    let frames: Vec<Vec<f64>> = wav
        .samples
        .chunks_exact(frame_len)
        .map(resample_cycle)
        .collect();
    let peak = frames.iter().flatten().fold(0.0f64, |m, v| m.max(v.abs()));
    if peak < 1e-9 {
        return Err(TableError::Silent);
    }
    let _ = (wav.bits, wav.channels);
    let frames: Vec<Vec<f32>> = frames
        .iter()
        .map(|f| f.iter().map(|&v| (v / peak * 0.98) as f32).collect())
        .collect();
    Ok(canonical_wav(&frames))
}

/// A table name safe to embed (`kabl_core::Table::validate`): the file name with anything but
/// letters, digits, `-`, `_` and `.` replaced, at most 80 bytes.
pub fn table_name(file_name: &str) -> String {
    let mut name: String = file_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "-_.".contains(c) {
                c
            } else {
                '_'
            }
        })
        .take(80)
        .collect();
    if name.is_empty() || name == "." || name == ".." {
        name = "table.wav".into();
    }
    name
}

/// Reads and imports a .wav file: `(embeddable name, canonical bytes)`. Control thread only.
pub fn import_file(path: &std::path::Path) -> Result<(String, Vec<u8>), String> {
    let shown = path.display();
    let len = std::fs::metadata(path)
        .map_err(|e| format!("{shown}: {e}"))?
        .len();
    if len > MAX_IMPORT_BYTES as u64 {
        return Err(format!("{shown}: {}", TableError::TooLarge));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("{shown}: {e}"))?;
    let canonical = import_wav(&bytes).map_err(|e| format!("{shown}: {e}"))?;
    let file = path
        .file_name()
        .map_or(String::new(), |f| f.to_string_lossy().into_owned());
    Ok((table_name(&file), canonical))
}

/// Canonical bytes for `frames` (each `FRAME` samples in -1..1).
pub fn canonical_wav(frames: &[Vec<f32>]) -> Vec<u8> {
    let samples = frames.len() * FRAME;
    let clm = b"<!>2048 00000000 wavetable (kabl)";
    let clm_len = clm.len() + (clm.len() & 1);
    let data_len = samples * 2;
    let mut out = Vec::with_capacity(44 + 8 + clm_len + data_len);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((36 + 8 + clm_len + data_len) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&48000u32.to_le_bytes());
    out.extend_from_slice(&96000u32.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"clm ");
    out.extend_from_slice(&(clm.len() as u32).to_le_bytes());
    out.extend_from_slice(clm);
    out.resize(out.len() + (clm.len() & 1), 0);
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_len as u32).to_le_bytes());
    for &v in frames.iter().flatten() {
        out.extend_from_slice(&((v.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
    }
    out
}

/// Frames of canonical table bytes.
pub fn decode_canonical(bytes: &[u8]) -> Result<Vec<Vec<f32>>, TableError> {
    let wav = parse_wav(bytes)?;
    let n = wav.samples.len();
    if wav.bits != 16 || wav.channels != 1 || n == 0 || n % FRAME != 0 {
        return Err(TableError::NotCanonical);
    }
    if n / FRAME > MAX_FRAMES {
        return Err(TableError::TooManyFrames(n / FRAME));
    }
    Ok(wav
        .samples
        .as_chunks::<FRAME>()
        .0
        .iter()
        .map(|f| f.iter().map(|&v| v as f32).collect())
        .collect())
}

/// The built-in tables; sources and licences: `assets/wavetables/PROVENANCE.md`.
pub const FACTORY_NAMES: [&str; 8] = [
    "Saw to Square",
    "Harmonic Sweep",
    "Vowels",
    "Glass Bell",
    "Digital Hollow",
    "Electric Piano",
    "Organ",
    "Choir",
];
const FACTORY_BYTES: [&[u8]; 8] = [
    include_bytes!("../assets/wavetables/saw-square.wav"),
    include_bytes!("../assets/wavetables/harmonic-sweep.wav"),
    include_bytes!("../assets/wavetables/vowels.wav"),
    include_bytes!("../assets/wavetables/glass-bell.wav"),
    include_bytes!("../assets/wavetables/digital-hollow.wav"),
    include_bytes!("../assets/wavetables/epiano.wav"),
    include_bytes!("../assets/wavetables/organ.wav"),
    include_bytes!("../assets/wavetables/choir.wav"),
];

pub const FACTORY_COUNT: usize = FACTORY_NAMES.len();

/// Builds every factory table now. The oscillator calls this from `prepare` (control thread), so
/// `factory` never builds on the audio thread.
pub fn init_factory() {
    factory(0);
}

/// Raw frames of factory table `i`, for drawing the current table in an interface (control thread
/// only: it decodes). User tables decode the same way with `decode_canonical`.
pub fn factory_frames(i: usize) -> Vec<Vec<f32>> {
    decode_canonical(FACTORY_BYTES[i.min(FACTORY_COUNT - 1)]).unwrap_or_default()
}

pub fn factory(i: usize) -> &'static WaveTable {
    static TABLES: OnceLock<Vec<WaveTable>> = OnceLock::new();
    let all = TABLES.get_or_init(|| {
        FACTORY_BYTES
            .iter()
            .zip(FACTORY_NAMES)
            .map(|(bytes, name)| {
                WaveTable::from_frames(
                    &decode_canonical(bytes)
                        .unwrap_or_else(|e| panic!("factory table {name}: {e}")),
                )
            })
            .collect()
    });
    &all[i.min(FACTORY_COUNT - 1)]
}
