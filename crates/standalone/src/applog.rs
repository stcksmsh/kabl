//! Operational logging for the kabl executables (D03, docs/signal-inspection/design.md §5):
//! the `log` facade plus one small backend, installed once by each binary's `main`. Libraries
//! only use the `log` macros.
//!
//! - Level: INFO by default; `KABL_LOG` or `--log-level` (`off`, `error`, `warn`, `info`,
//!   `debug`, `trace`) in the same release binary.
//! - File: `kabl.log` in `log_dir()`, rotated at `MAX_FILE_BYTES` into `kabl.1.log` …
//!   `kabl.<KEEP>.log`, so at most `(KEEP + 1) × MAX_FILE_BYTES` on disk.
//! - Calls format the line on the calling thread and hand it to a writer thread through a
//!   bounded queue. A full queue drops the line and counts it; a failing file counts the
//!   failure and keeps the instrument running. Records at INFO and above are echoed to stderr
//!   as `<app>: message`, as the binaries printed before.
//! - Never call the logger from the audio thread (it formats and allocates). Audio-side
//!   events are counters the UI thread reports.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use log::{Level, LevelFilter, Log, Metadata, Record};

/// Rotate the current file at this size.
pub const MAX_FILE_BYTES: u64 = 1024 * 1024;
/// Rotated files kept besides the current one.
pub const KEEP: usize = 3;
/// Lines waiting for the writer; more are dropped (and counted).
pub const QUEUE: usize = 1024;
/// How long shutdown waits for the writer to drain.
pub const FLUSH_WAIT: Duration = Duration::from_millis(500);

/// Build identity: the package version and the git commit the build script found.
pub fn build_id() -> String {
    format!(
        "{} (commit {})",
        env!("CARGO_PKG_VERSION"),
        option_env!("KABL_COMMIT").unwrap_or("unavailable")
    )
}

/// Counters the UI can show. All relaxed atomics; `last_error` is only touched off the audio
/// thread.
#[derive(Default)]
pub struct Stats {
    pub written: AtomicU64,
    /// Lines dropped because the queue was full or the writer had stopped.
    pub dropped: AtomicU64,
    /// Failed opens/writes/rotations.
    pub failed: AtomicU64,
    pub last_error: Mutex<Option<String>>,
}

/// Returned by `init`; `shutdown` flushes (bounded).
pub struct LogHandle {
    pub path: PathBuf,
    pub stats: Arc<Stats>,
    pub level: LevelFilter,
    pub session: String,
    done: Option<Receiver<()>>,
}

impl LogHandle {
    /// A copy for display (path, counters); only the original's `shutdown` waits.
    pub fn view(&self) -> LogHandle {
        LogHandle {
            path: self.path.clone(),
            stats: self.stats.clone(),
            level: self.level,
            session: self.session.clone(),
            done: None,
        }
    }

    /// Asks the writer to finish and waits at most `FLUSH_WAIT`.
    pub fn shutdown(&mut self) {
        log::logger().flush();
        if let Some(done) = self.done.take() {
            if let Some(l) = LOGGER.get() {
                let _ = l.tx.try_send(Msg::Stop);
            }
            let _ = done.recv_timeout(FLUSH_WAIT);
        }
    }

    /// A one-line problem report for the UI when lines were dropped or the file failed.
    pub fn problem(&self) -> Option<String> {
        let dropped = self.stats.dropped.load(Ordering::Relaxed);
        let failed = self.stats.failed.load(Ordering::Relaxed);
        if dropped == 0 && failed == 0 {
            return None;
        }
        let err = self.stats.last_error.lock().ok().and_then(|e| e.clone());
        Some(format!(
            "logging: {dropped} lines dropped, {failed} write failures{}",
            err.map_or(String::new(), |e| format!(" (last: {e})"))
        ))
    }
}

enum Msg {
    Line(Level, String),
    Flush,
    Stop,
}

struct Logger {
    session: String,
    start: Instant,
    tx: SyncSender<Msg>,
    stats: Arc<Stats>,
}

static LOGGER: std::sync::OnceLock<Logger> = std::sync::OnceLock::new();

/// `--log-level X` in `args`, else `KABL_LOG`, else INFO. An unknown word is INFO (and
/// returned as the second value so `main` can warn once the logger runs).
pub fn level_from(args: &[String]) -> (LevelFilter, Option<String>) {
    let word = args
        .iter()
        .position(|a| a == "--log-level")
        .and_then(|i| args.get(i + 1).cloned())
        .or_else(|| std::env::var("KABL_LOG").ok());
    match word {
        None => (LevelFilter::Info, None),
        Some(w) => match w.trim().parse::<LevelFilter>() {
            Ok(l) => (l, None),
            Err(_) => (LevelFilter::Info, Some(w)),
        },
    }
}

/// Where logs go: `KABL_LOG_DIR`, else `KABL_USER_DIR/logs`, else
/// `$XDG_STATE_HOME/kabl/logs`, else `~/.local/state/kabl/logs`.
pub fn log_dir() -> PathBuf {
    let var = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if let Some(d) = var("KABL_LOG_DIR") {
        return d;
    }
    if let Some(d) = var("KABL_USER_DIR") {
        return d.join("logs");
    }
    if let Some(d) = var("XDG_STATE_HOME") {
        return d.join("kabl").join("logs");
    }
    var("HOME")
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local/state/kabl/logs")
}

/// Installs the logger once for this process (`app` prefixes the stderr echo). A second call
/// returns `None`. A directory that cannot be created is counted as a failure; logging to
/// stderr continues.
pub fn init(app: &'static str, level: LevelFilter, dir: PathBuf) -> Option<LogHandle> {
    let stats = Arc::new(Stats::default());
    let (tx, rx) = sync_channel(QUEUE);
    let (done_tx, done) = sync_channel(1);
    let session = session_id();
    let logger = Logger {
        session: session.clone(),
        start: Instant::now(),
        tx,
        stats: stats.clone(),
    };
    LOGGER.set(logger).ok()?;
    let path = dir.join("kabl.log");
    let mut sink = Sink {
        dir,
        file: None,
        size: 0,
        stats: stats.clone(),
        app,
        retry_at: None,
    };
    std::thread::Builder::new()
        .name("kabl-log".into())
        .spawn(move || {
            sink.open();
            while let Ok(m) = rx.recv() {
                match m {
                    Msg::Line(level, line) => sink.write(level, &line),
                    Msg::Flush => sink.flush(),
                    Msg::Stop => break,
                }
            }
            sink.flush();
            let _ = done_tx.send(());
        })
        .ok()?;
    log::set_logger(LOGGER.get()?).ok()?;
    log::set_max_level(level);
    Some(LogHandle {
        path,
        stats,
        level,
        session,
        done: Some(done),
    })
}

impl Log for Logger {
    fn enabled(&self, m: &Metadata) -> bool {
        m.level() <= log::max_level()
    }

    fn log(&self, r: &Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let line = format!(
            "{} +{:.3}s {:<5} [{}] session={} {}\n",
            utc_now(),
            self.start.elapsed().as_secs_f64(),
            r.level(),
            r.target(),
            self.session,
            r.args()
        );
        match self.tx.try_send(Msg::Line(r.level(), line)) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
                self.stats.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn flush(&self) {
        let _ = self.tx.try_send(Msg::Flush);
    }
}

struct Sink {
    dir: PathBuf,
    file: Option<File>,
    size: u64,
    stats: Arc<Stats>,
    app: &'static str,
    /// After a failure, the next open is tried no sooner than this.
    retry_at: Option<Instant>,
}

impl Sink {
    fn fail(&mut self, what: &str, e: std::io::Error) {
        self.stats.failed.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut last) = self.stats.last_error.lock() {
            *last = Some(format!("{what}: {e}"));
        }
        self.file = None;
        self.retry_at = Some(Instant::now() + Duration::from_secs(5));
    }

    fn open(&mut self) {
        if let Err(e) = fs::create_dir_all(&self.dir) {
            return self.fail("create log dir", e);
        }
        let path = self.dir.join("kabl.log");
        match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(f) => {
                self.size = f.metadata().map(|m| m.len()).unwrap_or(0);
                self.file = Some(f);
                self.retry_at = None;
            }
            Err(e) => self.fail("open log", e),
        }
    }

    fn rotate(&mut self) {
        self.file = None;
        let name = |n: usize| {
            self.dir.join(if n == 0 {
                "kabl.log".to_string()
            } else {
                format!("kabl.{n}.log")
            })
        };
        let _ = fs::remove_file(name(KEEP));
        for n in (0..KEEP).rev() {
            let from = name(n);
            if from.exists() {
                if let Err(e) = fs::rename(&from, name(n + 1)) {
                    return self.fail("rotate log", e);
                }
            }
        }
        self.open();
    }

    fn write(&mut self, level: Level, line: &str) {
        if level <= Level::Info {
            let text = line.split_once("] session=").map_or(line, |(_, rest)| {
                rest.split_once(' ').map_or(rest, |(_, msg)| msg)
            });
            let _ = write!(std::io::stderr(), "{}: {text}", self.app);
        }
        if self.file.is_none() {
            if self.retry_at.is_some_and(|t| Instant::now() < t) {
                self.stats.dropped.fetch_add(1, Ordering::Relaxed);
                return;
            }
            self.open();
        }
        if self.size + line.len() as u64 > MAX_FILE_BYTES && self.size > 0 {
            self.rotate();
        }
        let Some(f) = self.file.as_mut() else {
            self.stats.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        };
        match f.write_all(line.as_bytes()) {
            Ok(()) => {
                self.size += line.len() as u64;
                self.stats.written.fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => {
                self.stats.dropped.fetch_add(1, Ordering::Relaxed);
                self.fail("write log", e);
            }
        }
    }

    fn flush(&mut self) {
        if let Some(f) = self.file.as_mut() {
            let _ = f.flush();
        }
    }
}

/// 8 hex digits from the time and process id; enough to tell sessions apart in one file.
fn session_id() -> String {
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    format!("{:08x}", (t ^ (std::process::id() as u64) << 20) as u32)
}

/// `2026-09-25T12:34:56.789Z`.
pub fn utc_now() -> String {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    utc(d.as_secs() as i64, d.subsec_millis())
}

fn utc(secs: i64, ms: u32) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{ms:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Test helper: the log files in `dir`, newest first, with their sizes.
pub fn files(dir: &Path) -> Vec<(String, u64)> {
    let mut v: Vec<(String, u64)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            n.starts_with("kabl")
                .then(|| (n, e.metadata().map(|m| m.len()).unwrap_or(0)))
        })
        .collect();
    v.sort();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_formats_known_instants() {
        assert_eq!(utc(0, 0), "1970-01-01T00:00:00.000Z");
        assert_eq!(utc(951_782_400, 5), "2000-02-29T00:00:00.005Z");
        assert_eq!(utc(1_790_000_000, 999), "2026-09-21T14:13:20.999Z");
    }

    #[test]
    fn levels_parse_and_unknown_is_info() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            level_from(&a(&["x", "--log-level", "debug"])).0,
            LevelFilter::Debug
        );
        assert_eq!(
            level_from(&a(&["x", "--log-level", "TRACE"])).0,
            LevelFilter::Trace
        );
        let (l, bad) = level_from(&a(&["x", "--log-level", "loud"]));
        assert_eq!((l, bad.as_deref()), (LevelFilter::Info, Some("loud")));
    }

    #[test]
    fn rotation_keeps_the_cap() {
        let tmp = tempfile::tempdir().unwrap();
        let stats = Arc::new(Stats::default());
        let mut s = Sink {
            dir: tmp.path().into(),
            file: None,
            size: 0,
            stats: stats.clone(),
            app: "test",
            retry_at: None,
        };
        s.open();
        let line = format!("{}\n", "x".repeat(999));
        for _ in 0..6000 {
            s.write(Level::Debug, &line);
        }
        s.flush();
        let f = files(tmp.path());
        let names: Vec<&str> = f.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            ["kabl.1.log", "kabl.2.log", "kabl.3.log", "kabl.log"]
        );
        let total: u64 = f.iter().map(|(_, n)| n).sum();
        assert!(total <= (KEEP as u64 + 1) * MAX_FILE_BYTES, "{total}");
        assert!(f.iter().all(|(_, n)| *n <= MAX_FILE_BYTES));
        assert_eq!(stats.failed.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn an_unwritable_directory_counts_failures_and_never_panics() {
        let tmp = tempfile::tempdir().unwrap();
        let blocker = tmp.path().join("file");
        fs::write(&blocker, "not a dir").unwrap();
        let stats = Arc::new(Stats::default());
        let mut s = Sink {
            dir: blocker.join("logs"),
            file: None,
            size: 0,
            stats: stats.clone(),
            app: "test",
            retry_at: None,
        };
        s.open();
        for _ in 0..100 {
            s.write(Level::Debug, "line\n");
        }
        assert_eq!(
            stats.failed.load(Ordering::Relaxed),
            1,
            "retried only after a pause"
        );
        assert_eq!(stats.dropped.load(Ordering::Relaxed), 100);
        assert!(stats.last_error.lock().unwrap().is_some());
    }
}
