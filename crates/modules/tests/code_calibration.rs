//! `host::device_changed` (app startup and audio-device change): the block period is applied at
//! once, calibration runs off-thread, and the budgets end up on the measured value.

use kabl_modules::code::{self, host};
use std::time::{Duration, Instant};

#[test]
fn startup_calibration_replaces_the_fallback_budget() {
    host::register_examples().unwrap();
    let h = code::group("demo.logic").unwrap().health().clone();
    let fallback = h.fuel_budget();
    assert!(!host::is_calibrated());

    let t = Instant::now();
    host::device_changed(48_000.0, 64);
    assert!(t.elapsed() < Duration::from_millis(50), "must not block");
    while !host::is_calibrated() {
        assert!(
            t.elapsed() < Duration::from_secs(60),
            "calibration timed out"
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    let cal = host::budget().calibration();
    assert!(!cal.probes.is_empty(), "measured, not the fixed fallback");
    // Two examples at 25 % each fit the 50 % total: 25 % of a 64-frame period at 48 kHz.
    let want = cal.fuel_for_us(64.0 / 48_000.0 * 1e6 * 0.25);
    assert_eq!(h.fuel_budget(), want);
    assert_ne!(h.fuel_budget(), fallback);
    println!(
        "calibrated {:.0} fuel/us; per-module budget {} fuel ({:?})",
        cal.fuel_per_us,
        h.fuel_budget(),
        h.wall_cap()
    );
}
