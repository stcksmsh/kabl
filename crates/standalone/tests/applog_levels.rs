//! The default level writes INFO and above and nothing below; lines carry time, elapsed time,
//! level, subsystem and the session id.

use kabl_standalone::applog;

#[test]
fn info_default_suppresses_debug_and_trace() {
    let tmp = tempfile::tempdir().unwrap();
    let (level, _) = applog::level_from(&["kabl".to_string()]);
    assert_eq!(level, log::LevelFilter::Info, "KABL_LOG unset in tests");
    let mut h = applog::init("test", level, tmp.path().into()).unwrap();
    log::error!(target: "audio", "e1");
    log::warn!(target: "audio", "w1");
    log::info!(target: "doc", "i1 id=3");
    log::debug!(target: "graph", "d1");
    log::trace!(target: "graph", "t1");
    log::info!(target: "winit::platform", "third-party info");
    h.shutdown();
    let text = std::fs::read_to_string(&h.path).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3, "{text}");
    assert!(!text.contains("d1") && !text.contains("t1"));
    assert!(!text.contains("third-party"), "other crates' INFO is capped at WARN");
    let l = lines[2];
    assert!(l.contains(" INFO  [doc] session="), "{l}");
    assert!(l.contains(&format!("session={} i1 id=3", h.session)), "{l}");
    assert!(l.starts_with("20") && l.contains("Z +"), "{l}");
    assert!(!applog::build_id().is_empty());
}
