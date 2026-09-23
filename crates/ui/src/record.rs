//! Stereo recording of the final engine output to a 32-bit float WAV at the device rate.
//!
//! The audio callback owns a `Tap` and hands it each frame it sends to the device; the tap
//! pushes into a bounded lock-free ring only while recording, and never allocates, locks or
//! blocks. A writer thread (`Recorder`, UI side) drains the ring into the file and checkpoints
//! the WAV header every half second (`hound`'s `flush`), so even a killed process leaves a file
//! readable up to the last checkpoint. Stop finalizes it; dropping the `Recorder` (app exit)
//! stops and finalizes too.
//!
//! A callback that finds too little room in the ring writes nothing and counts its frames as
//! lost. A take with lost frames is renamed `*-INCOMPLETE.wav` and reported; it is never
//! presented as complete. Recording is a runtime action: nothing here touches the patch.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use egui::RichText;

const IDLE: u8 = 0;
const REC: u8 = 1;
/// Asked to stop; the audio thread answers by setting `IDLE` on its next callback.
const STOPPING: u8 = 2;
/// Seconds of stereo audio the ring holds.
pub const RING_SECONDS: f32 = 4.0;
const CHECKPOINT: Duration = Duration::from_millis(500);
/// How long a stop waits for the audio thread to answer (no callbacks: device gone).
const ACK_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Default)]
struct Shared {
    state: AtomicU8,
    lost: AtomicU64,
}

/// Audio-thread end.
pub struct Tap {
    tx: rtrb::Producer<f32>,
    shared: Arc<Shared>,
}

impl Tap {
    /// Call once per callback before its `frame`s. True when this callback's `frames` frames
    /// should be pushed. No allocation, no locks.
    #[inline]
    pub fn begin(&mut self, frames: usize) -> bool {
        match self.shared.state.load(Ordering::Acquire) {
            REC => {}
            STOPPING => {
                let _ = self.shared.state.compare_exchange(
                    STOPPING,
                    IDLE,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                );
                return false;
            }
            _ => return false,
        }
        if self.tx.slots() < 2 * frames {
            self.shared.lost.fetch_add(frames as u64, Ordering::Relaxed);
            return false;
        }
        true
    }

    #[inline]
    pub fn frame(&mut self, l: f32, r: f32) {
        let _ = self.tx.push(l);
        let _ = self.tx.push(r);
    }
}

/// How the last take ended.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Complete {
        path: PathBuf,
        frames: u64,
    },
    Incomplete {
        path: PathBuf,
        frames: u64,
        lost: u64,
    },
    Failed(String),
}

struct Job {
    handle: JoinHandle<(rtrb::Consumer<f32>, Result<(), String>)>,
    stop: Arc<AtomicBool>,
    frames: Arc<AtomicU64>,
    path: PathBuf,
}

/// UI end: starts and stops takes and reports their state.
pub struct Recorder {
    shared: Arc<Shared>,
    rx: Option<rtrb::Consumer<f32>>,
    sample_rate: u32,
    /// Folder new takes go into.
    pub dir: String,
    job: Option<Job>,
    pub last: Option<Outcome>,
}

/// A recorder and its audio-thread tap, ring sized for `RING_SECONDS` at `sample_rate`.
pub fn pair(sample_rate: u32) -> (Recorder, Tap) {
    pair_sized(sample_rate, (RING_SECONDS * sample_rate as f32) as usize)
}

/// Same, with a ring of `frames` stereo frames.
pub fn pair_sized(sample_rate: u32, frames: usize) -> (Recorder, Tap) {
    let shared = Arc::new(Shared::default());
    let (tx, rx) = rtrb::RingBuffer::new(frames * 2);
    (
        Recorder {
            shared: shared.clone(),
            rx: Some(rx),
            sample_rate,
            dir: "recordings".into(),
            job: None,
            last: None,
        },
        Tap { tx, shared },
    )
}

impl Recorder {
    pub fn recording(&self) -> bool {
        self.job.is_some()
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Seconds written so far in the current take.
    pub fn elapsed(&self) -> f32 {
        self.job.as_ref().map_or(0.0, |j| {
            j.frames.load(Ordering::Relaxed) as f32 / self.sample_rate as f32
        })
    }

    pub fn path(&self) -> Option<&Path> {
        self.job.as_ref().map(|j| j.path.as_path())
    }

    /// Frames lost to a full ring in the current take.
    pub fn lost(&self) -> u64 {
        self.shared.lost.load(Ordering::Relaxed)
    }

    /// Starts a take in `dir` with a timestamped name.
    pub fn start(&mut self) -> Result<PathBuf, String> {
        let name = format!("kabl-{}.wav", utc_stamp());
        self.start_at(Path::new(&self.dir).join(name))
    }

    pub fn start_at(&mut self, path: PathBuf) -> Result<PathBuf, String> {
        if self.job.is_some() {
            return Err("already recording".into());
        }
        if self.shared.state.load(Ordering::Acquire) != IDLE {
            return Err("still finishing the last take".into());
        }
        let Some(mut rx) = self.rx.take() else {
            return Err("recorder unavailable".into());
        };
        // Anything left from an aborted take is not part of this one.
        while rx.pop().is_ok() {}
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: self.sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let writer = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map_or(Ok(()), std::fs::create_dir_all)
            .map_err(|e| e.to_string())
            .and_then(|()| hound::WavWriter::create(&path, spec).map_err(|e| e.to_string()));
        let mut writer = match writer {
            Ok(w) => w,
            Err(e) => {
                self.rx = Some(rx);
                let msg = format!("can't record to {}: {e}", path.display());
                self.last = Some(Outcome::Failed(msg.clone()));
                return Err(msg);
            }
        };
        self.shared.lost.store(0, Ordering::Relaxed);
        let stop = Arc::new(AtomicBool::new(false));
        let frames = Arc::new(AtomicU64::new(0));
        let shared = self.shared.clone();
        let (stop2, frames2) = (stop.clone(), frames.clone());
        shared.state.store(REC, Ordering::Release);
        let handle = std::thread::Builder::new()
            .name("kabl-recorder".into())
            .spawn(move || {
                let mut result = Ok(());
                let mut checkpoint = Instant::now();
                let mut stop_at: Option<Instant> = None;
                loop {
                    let n = rx.slots() / 2 * 2;
                    if n > 0 {
                        let chunk = rx.read_chunk(n).expect("n slots are readable");
                        let (a, b) = chunk.as_slices();
                        if result.is_ok() {
                            for &s in a.iter().chain(b) {
                                if let Err(e) = writer.write_sample(s) {
                                    result = Err(e.to_string());
                                    // Tell the audio thread to stop pushing.
                                    let _ = shared.state.compare_exchange(
                                        REC,
                                        STOPPING,
                                        Ordering::AcqRel,
                                        Ordering::Relaxed,
                                    );
                                    break;
                                }
                            }
                        }
                        chunk.commit_all();
                        if result.is_ok() {
                            frames2.fetch_add(n as u64 / 2, Ordering::Relaxed);
                        }
                    }
                    if result.is_ok() && checkpoint.elapsed() >= CHECKPOINT {
                        checkpoint = Instant::now();
                        if let Err(e) = writer.flush() {
                            result = Err(e.to_string());
                            let _ = shared.state.compare_exchange(
                                REC,
                                STOPPING,
                                Ordering::AcqRel,
                                Ordering::Relaxed,
                            );
                        }
                    }
                    if stop2.load(Ordering::Acquire) || result.is_err() {
                        let since = *stop_at.get_or_insert_with(Instant::now);
                        let acked = shared.state.load(Ordering::Acquire) == IDLE;
                        if (acked && rx.is_empty()) || since.elapsed() > ACK_TIMEOUT {
                            break;
                        }
                    }
                    if n == 0 {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
                // No callbacks answered (device gone): nothing else will push.
                shared.state.store(IDLE, Ordering::Release);
                let fin = writer.finalize().map_err(|e| e.to_string());
                (rx, result.and(fin))
            })
            .expect("spawn recorder thread");
        self.job = Some(Job {
            handle,
            stop,
            frames,
            path: path.clone(),
        });
        Ok(path)
    }

    /// Stops the take, waits for the file to be finalized, and reports how it ended.
    pub fn stop(&mut self) -> Option<Outcome> {
        let job = self.job.take()?;
        let _ =
            self.shared
                .state
                .compare_exchange(REC, STOPPING, Ordering::AcqRel, Ordering::Relaxed);
        job.stop.store(true, Ordering::Release);
        let outcome = match job.handle.join() {
            Ok((rx, result)) => {
                self.rx = Some(rx);
                let frames = job.frames.load(Ordering::Relaxed);
                let lost = self.lost();
                match result {
                    Err(e) => Outcome::Failed(format!(
                        "recording failed after {:.1} s ({}): {e}",
                        frames as f32 / self.sample_rate as f32,
                        job.path.display()
                    )),
                    Ok(()) if lost > 0 => {
                        let stem = job.path.with_extension("");
                        let bad = PathBuf::from(format!("{}-INCOMPLETE.wav", stem.display()));
                        let path = match std::fs::rename(&job.path, &bad) {
                            Ok(()) => bad,
                            Err(_) => job.path.clone(),
                        };
                        Outcome::Incomplete { path, frames, lost }
                    }
                    Ok(()) => Outcome::Complete {
                        path: job.path.clone(),
                        frames,
                    },
                }
            }
            Err(_) => Outcome::Failed("recorder thread panicked".into()),
        };
        self.last = Some(outcome.clone());
        Some(outcome)
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.stop();
    }
}

/// `YYYYMMDD-HHMMSSZ` (UTC) for take names.
fn utc_stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (days, rem) = (secs / 86400, secs % 86400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days as i64 + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}{m:02}{d:02}-{:02}{:02}{:02}Z",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

fn clock(s: f32) -> String {
    let s = s as u64;
    format!("{:02}:{:02}", s / 60, s % 60)
}

/// Record/Stop, elapsed time, destination and the last outcome.
pub fn controls(ui_state: &mut crate::UiState, ui: &mut egui::Ui, th: &crate::theme::Theme) {
    let Some(rec) = ui_state.recorder.as_mut() else {
        ui.label(RichText::new("recording needs an audio device").color(th.ink2));
        return;
    };
    let mut hits = Vec::new();
    let mut message = None;
    // Right to left.
    if let Some(path) = rec.path() {
        let lost = rec.lost();
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let r = ui
            .label(RichText::new(format!("→ {name}")).small().monospace())
            .on_hover_text(path.display().to_string());
        hits.push(("rec-dest".to_string(), r.rect));
        if lost > 0 {
            ui.label(
                RichText::new(format!("{lost} frames lost: take incomplete"))
                    .color(egui::Color32::from_rgb(220, 60, 50))
                    .strong(),
            );
        }
        let r = ui.label(
            RichText::new(format!("● REC {}", clock(rec.elapsed())))
                .color(egui::Color32::from_rgb(220, 60, 50))
                .strong()
                .monospace(),
        );
        hits.push(("rec-time".to_string(), r.rect));
        let r = ui.button(RichText::new("■ Stop").strong());
        hits.push(("rec-stop".to_string(), r.rect));
        if r.clicked() {
            message = rec.stop().map(|o| describe(&o));
        }
    } else {
        let r = ui.button(RichText::new("● Record").strong());
        hits.push(("rec-start".to_string(), r.rect));
        if r.clicked() {
            if let Err(e) = rec.start() {
                message = Some(e);
            }
        }
        let r = ui.add(egui::TextEdit::singleline(&mut rec.dir).desired_width(120.0));
        hits.push(("rec-dir".to_string(), r.rect));
        ui.label("Record to");
        if let Some(o) = &rec.last {
            let (t, bad) = (describe(o), !matches!(o, Outcome::Complete { .. }));
            let text = RichText::new(t).small();
            ui.label(if bad {
                text.color(egui::Color32::from_rgb(220, 60, 50))
            } else {
                text
            });
        }
    }
    if rec.recording() {
        ui.ctx().request_repaint_after(Duration::from_millis(200));
    }
    for (k, r) in hits {
        ui_state.record(k, r);
    }
    if message.is_some() {
        ui_state.last_message = message;
    }
}

pub fn describe(o: &Outcome) -> String {
    match o {
        Outcome::Complete { path, frames } => {
            format!("saved {} ({} frames)", path.display(), frames)
        }
        Outcome::Incomplete { path, frames, lost } => format!(
            "INCOMPLETE: {lost} frames lost; {frames} written to {}",
            path.display()
        ),
        Outcome::Failed(e) => e.clone(),
    }
}
