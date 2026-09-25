//! A full disk (the log file is /dev/full: every write fails with ENOSPC) is counted as a
//! failure with its reason; logging keeps going without panicking.

use std::sync::atomic::Ordering;
use std::time::Duration;

use kabl_standalone::applog;

#[test]
fn a_full_disk_is_counted_and_reported() {
    let tmp = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink("/dev/full", tmp.path().join("kabl.log")).unwrap();
    let mut h = applog::init("test", log::LevelFilter::Info, tmp.path().into()).unwrap();
    for i in 0..10 {
        log::warn!(target: "test", "line {i}");
    }
    log::logger().flush();
    std::thread::sleep(Duration::from_millis(300));
    assert!(h.stats.failed.load(Ordering::Relaxed) >= 1);
    let p = h.problem().unwrap();
    assert!(p.contains("write log") && p.contains("No space"), "{p}");
    h.shutdown();
}
