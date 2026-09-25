//! A log sink that never accepts a byte (a fifo nobody reads: opening it blocks the writer
//! thread forever) must not block or crash the callers: the queue fills, lines are dropped
//! and counted, and shutdown gives up after `FLUSH_WAIT`.

use std::time::{Duration, Instant};

use kabl_standalone::applog;

#[test]
fn a_stalled_sink_drops_and_counts_without_blocking() {
    let tmp = tempfile::tempdir().unwrap();
    let fifo = tmp.path().join("kabl.log");
    assert!(std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap()
        .success());
    let mut h = applog::init("test", log::LevelFilter::Info, tmp.path().into()).unwrap();
    let start = Instant::now();
    let n = 5 * applog::QUEUE as u64;
    for i in 0..n {
        log::info!(target: "test", "line {i}");
    }
    let took = start.elapsed();
    assert!(took < Duration::from_secs(1), "callers blocked: {took:?}");
    let dropped = h.stats.dropped.load(std::sync::atomic::Ordering::Relaxed);
    assert!(
        dropped >= n - applog::QUEUE as u64 - 1,
        "dropped {dropped} of {n}"
    );
    assert!(h.problem().unwrap().contains("dropped"));
    let start = Instant::now();
    h.shutdown();
    assert!(start.elapsed() < applog::FLUSH_WAIT + Duration::from_millis(300));
}
