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
    /// Set by the writer when it hits an error (the take then ends on its own).
    failed: Arc<AtomicBool>,
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
    /// Failure injection for testing (`KABL_RECORD_FAIL_AFTER`): the writer reports a write
    /// error once this many frames are written.
    pub fail_after: Option<u64>,
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
            fail_after: None,
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

    /// The writer failed and finished; `stop` collects the error.
    pub fn failed(&self) -> bool {
        self.job
            .as_ref()
            .is_some_and(|j| j.failed.load(Ordering::Acquire))
    }

    pub fn path(&self) -> Option<&Path> {
        self.job.as_ref().map(|j| j.path.as_path())
    }

    /// Frames lost to a full ring in the current take.
    pub fn lost(&self) -> u64 {
        self.shared.lost.load(Ordering::Relaxed)
    }

    /// Starts a take in `dir` with a timestamped name (`-2`, `-3`, … when that name is taken).
    pub fn start(&mut self) -> Result<PathBuf, String> {
        let stamp = utc_stamp();
        let dir = Path::new(&self.dir).to_path_buf();
        self.start_at(dir.join(format!("kabl-{stamp}.wav")))
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
        let writer = create_new(&path, spec);
        let (mut writer, path) = match writer {
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
        let fail_after = self.fail_after;
        let failed = Arc::new(AtomicBool::new(false));
        let failed2 = failed.clone();
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
                        let injected = fail_after
                            .is_some_and(|f| frames2.load(Ordering::Relaxed) + n as u64 / 2 > f);
                        if result.is_ok() && injected {
                            result = Err("injected write failure (KABL_RECORD_FAIL_AFTER)".into());
                            let _ = shared.state.compare_exchange(
                                REC,
                                STOPPING,
                                Ordering::AcqRel,
                                Ordering::Relaxed,
                            );
                        }
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
                if result.is_err() {
                    failed2.store(true, Ordering::Release);
                }
                // No callbacks answered (device gone): nothing else will push.
                shared.state.store(IDLE, Ordering::Release);
                let fin = writer.finalize().map_err(|e| e.to_string());
                (rx, result.and(fin))
            })
            .expect("spawn recorder thread");
        self.job = Some(Job {
            handle,
            failed,
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
                // A take that lost frames or hit an error keeps what was written, marked.
                let mark = |p: &Path| {
                    let bad =
                        PathBuf::from(format!("{}-INCOMPLETE.wav", p.with_extension("").display()));
                    if !bad.exists() && std::fs::rename(p, &bad).is_ok() {
                        bad
                    } else {
                        p.to_path_buf()
                    }
                };
                match result {
                    Err(e) => {
                        let kept = if job.path.exists() {
                            format!(", kept as {}", mark(&job.path).display())
                        } else {
                            String::new()
                        };
                        Outcome::Failed(format!(
                            "recording failed after {:.1} s: {e}{kept}",
                            frames as f32 / self.sample_rate as f32
                        ))
                    }
                    Ok(()) if lost > 0 => Outcome::Incomplete {
                        path: mark(&job.path),
                        frames,
                        lost,
                    },
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

type Writer = hound::WavWriter<std::io::BufWriter<std::fs::File>>;

/// Creates `path` (and its folder) without ever replacing a file: if the name is taken, the
/// first free `name-2.wav`, `name-3.wav`, … is used instead.
fn create_new(path: &Path, spec: hound::WavSpec) -> Result<(Writer, PathBuf), String> {
    if let Some(dir) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let stem = path.with_extension("");
    for k in 1..1000 {
        let candidate = if k == 1 {
            path.to_path_buf()
        } else {
            PathBuf::from(format!("{}-{k}.wav", stem.display()))
        };
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(f) => {
                let w = hound::WavWriter::new(std::io::BufWriter::new(f), spec)
                    .map_err(|e| e.to_string())?;
                return Ok((w, candidate));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("no free file name".into())
}

/// Opens a folder in the desktop's file manager.
pub fn open_folder(dir: &Path) -> Result<(), String> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener)
        .arg(dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("can't open {}: {e}", dir.display()))
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

/// The recorder row: Record/Stop, elapsed time, the take's file, the destination folder and
/// the last take. Long names are truncated (hover shows them whole), so the buttons stay put.
pub fn controls(ui_state: &mut crate::UiState, ui: &mut egui::Ui, th: &crate::theme::Theme) {
    let red = egui::Color32::from_rgb(220, 60, 50);
    let Some(rec) = ui_state.recorder.as_mut() else {
        ui.label(RichText::new("Recording needs an audio device.").color(th.ink2));
        return;
    };
    let mut hits = Vec::new();
    let mut message = None;
    if rec.failed() {
        message = rec.stop().map(|o| describe(&o));
    }
    let name = |p: &Path| {
        p.file_name().map_or_else(
            || p.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        )
    };
    if let Some(path) = rec.path().map(Path::to_path_buf) {
        let r = ui
            .add(egui::Button::new(RichText::new("■ Stop").strong()).min_size([84.0, 0.0].into()));
        hits.push(("rec-stop".to_string(), r.rect));
        if r.clicked() {
            message = rec.stop().map(|o| describe(&o));
        }
        let r = ui.label(
            RichText::new(format!("● REC {}", clock(rec.elapsed())))
                .color(red)
                .strong()
                .monospace(),
        );
        hits.push(("rec-time".to_string(), r.rect));
        let lost = rec.lost();
        if lost > 0 {
            ui.label(
                RichText::new(format!("{lost} frames lost: incomplete"))
                    .color(red)
                    .strong(),
            );
        }
        let r = ui
            .add(
                egui::Label::new(
                    RichText::new(format!("→ {}", name(&path)))
                        .small()
                        .monospace(),
                )
                .truncate(),
            )
            .on_hover_text(path.display().to_string());
        hits.push(("rec-dest".to_string(), r.rect));
    } else {
        let r = ui.add(
            egui::Button::new(RichText::new("● Record").strong()).min_size([84.0, 0.0].into()),
        );
        hits.push(("rec-start".to_string(), r.rect));
        if r.clicked() {
            if let Err(e) = rec.start() {
                message = Some(e);
            }
        }
        ui.label("to");
        let r = ui
            .add(egui::TextEdit::singleline(&mut rec.dir).desired_width(140.0))
            .on_hover_text("Folder for new takes (created if missing)");
        hits.push(("rec-dir".to_string(), r.rect));
        let r = ui.small_button("Open folder");
        hits.push(("rec-open".to_string(), r.rect));
        if r.clicked() {
            let dir = std::path::PathBuf::from(&rec.dir);
            if let Err(e) = std::fs::create_dir_all(&dir)
                .map_err(|e| e.to_string())
                .and_then(|()| open_folder(&dir))
            {
                message = Some(e);
            }
        }
        if let Some(o) = rec.last.clone() {
            let (path, ok) = match &o {
                Outcome::Complete { path, .. } => (Some(path.clone()), true),
                Outcome::Incomplete { path, .. } => (Some(path.clone()), false),
                Outcome::Failed(_) => (None, false),
            };
            ui.separator();
            let text = match &o {
                Outcome::Complete { path, frames } => format!(
                    "Last take: {} ({})",
                    name(path),
                    clock(*frames as f32 / rec.sample_rate() as f32)
                ),
                Outcome::Incomplete { path, lost, .. } => {
                    format!("Last take INCOMPLETE ({lost} frames lost): {}", name(path))
                }
                Outcome::Failed(e) => format!("Recording failed: {e}"),
            };
            let t = RichText::new(text).small();
            let r = ui
                .add(egui::Label::new(if ok { t } else { t.color(red) }).truncate())
                .on_hover_text(describe(&o));
            hits.push(("rec-last".to_string(), r.rect));
            if let Some(p) = path {
                let r = ui.small_button("Copy path");
                hits.push(("rec-copy".to_string(), r.rect));
                if r.clicked() {
                    ui.ctx().copy_text(p.display().to_string());
                }
            }
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

/// Final-output peak telemetry: the audio callback folds each frame's |L|, |R| in with
/// `fetch_max` on the f32 bits (non-negative floats order like their bits), the UI takes and
/// clears them. Lock-free, fixed size.
#[derive(Default)]
pub struct PeakTap {
    l: std::sync::atomic::AtomicU32,
    r: std::sync::atomic::AtomicU32,
    nonfinite: AtomicBool,
}

impl PeakTap {
    /// Audio thread.
    #[inline]
    pub fn feed(&self, l: f32, r: f32) {
        if !(l.is_finite() && r.is_finite()) {
            self.nonfinite.store(true, Ordering::Relaxed);
            return;
        }
        self.l.fetch_max(l.abs().to_bits(), Ordering::Relaxed);
        self.r.fetch_max(r.abs().to_bits(), Ordering::Relaxed);
    }

    /// UI thread: peaks since the last take, and whether a non-finite sample was seen.
    pub fn take(&self) -> ([f32; 2], bool) {
        (
            [
                f32::from_bits(self.l.swap(0, Ordering::Relaxed)),
                f32::from_bits(self.r.swap(0, Ordering::Relaxed)),
            ],
            self.nonfinite.swap(false, Ordering::Relaxed),
        )
    }
}

/// The output meter's display state: peak hold falling at 20 dB/s, a clip latch (any sample at
/// or above 0 dBFS, or non-finite) that stays until clicked.
#[derive(Debug, Default, Clone, Copy)]
pub struct Meter {
    pub level: [f32; 2],
    pub clip: bool,
    pub nonfinite: bool,
    /// Highest peak since the last reset, dBFS.
    pub max_db: Option<f32>,
}

impl Meter {
    pub fn update(&mut self, peaks: [f32; 2], nonfinite: bool, dt: f32) {
        let fall = 10f32.powf(-20.0 * dt / 20.0);
        for (lv, p) in self.level.iter_mut().zip(peaks) {
            *lv = (*lv * fall).max(p);
            if p >= 1.0 {
                self.clip = true;
            }
        }
        let p = peaks[0].max(peaks[1]);
        if p > 0.0 {
            let db = 20.0 * p.log10();
            self.max_db = Some(self.max_db.map_or(db, |m| m.max(db)));
        }
        self.nonfinite |= nonfinite;
        self.clip |= nonfinite;
    }
}

/// Two bars (L, R), −60..0 dBFS, the peak hold, and a CLIP light. Click resets.
pub fn meter_ui(ui: &mut egui::Ui, meter: &mut Meter) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(132.0, 20.0), egui::Sense::click());
    let p = ui.painter();
    let bars = egui::Rect::from_min_size(rect.min, egui::vec2(84.0, rect.height()));
    let bg = ui.visuals().extreme_bg_color;
    let to_x = |v: f32| {
        let db = if v > 0.0 { 20.0 * v.log10() } else { -120.0 };
        bars.left() + bars.width() * ((db + 60.0) / 60.0).clamp(0.0, 1.0)
    };
    for (k, &v) in meter.level.iter().enumerate() {
        let y0 = bars.top() + 2.0 + k as f32 * 9.0;
        let track = egui::Rect::from_min_max(
            egui::pos2(bars.left(), y0),
            egui::pos2(bars.right(), y0 + 7.0),
        );
        p.rect_filled(track, 2.0, bg);
        let x = to_x(v);
        let color = if v >= 1.0 {
            egui::Color32::from_rgb(220, 60, 50)
        } else if v >= 10f32.powf(-6.0 / 20.0) {
            egui::Color32::from_rgb(220, 170, 50)
        } else {
            egui::Color32::from_rgb(90, 190, 110)
        };
        p.rect_filled(
            egui::Rect::from_min_max(track.min, egui::pos2(x, track.bottom())),
            2.0,
            color,
        );
    }
    // 0 dB is the right edge; a tick at −6 dB.
    let x6 = to_x(10f32.powf(-6.0 / 20.0));
    p.line_segment(
        [egui::pos2(x6, bars.top()), egui::pos2(x6, bars.bottom())],
        egui::Stroke::new(1.0, ui.visuals().weak_text_color()),
    );
    let clip = egui::Rect::from_min_size(
        egui::pos2(bars.right() + 4.0, rect.top() + 2.0),
        egui::vec2(40.0, 16.0),
    );
    p.rect_filled(
        clip,
        3.0,
        if meter.clip {
            egui::Color32::from_rgb(220, 60, 50)
        } else {
            bg
        },
    );
    p.text(
        clip.center(),
        egui::Align2::CENTER_CENTER,
        if meter.nonfinite { "NaN" } else { "CLIP" },
        egui::FontId::proportional(10.0),
        if meter.clip {
            egui::Color32::WHITE
        } else {
            ui.visuals().weak_text_color()
        },
    );
    let max = meter
        .max_db
        .map_or("no signal".to_string(), |m| format!("{m:+.1} dBFS"));
    let resp = resp.on_hover_text(format!(
        "Final output, measured: peak since reset {max}. Click to reset the clip light."
    ));
    if resp.clicked() {
        *meter = Meter::default();
    }
    resp
}
