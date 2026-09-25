//! D03: the audio-side paths stay allocation-free with the logger installed at TRACE: the
//! engine with the tap on (swaps, selection changes), the report hand-off into a full queue,
//! and the stream-error hand-off into a full queue with an error that owns its message
//! (forgotten, not freed, on the audio thread). The callback's timing counters are relaxed
//! atomics and are not repeated here.

use std::sync::atomic::{AtomicU64, Ordering};

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::Collector;
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::{Command, PatchEngine};
use kabl_engine::probe::{ProbeReport, ProbeTarget};
use kabl_standalone::{applog, hand_off_stream_error};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

#[test]
fn audio_side_paths_do_not_allocate_with_trace_logging() {
    let tmp = tempfile::tempdir().unwrap();
    let mut log = applog::init("test", log::LevelFilter::Trace, tmp.path().into()).unwrap();
    log::trace!(target: "test", "logger running at trace");

    let c = Collector::new();
    let h = c.handle();
    let patch = kabl_core::load(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/composition"),
    )
    .unwrap()
    .state()
    .clone();
    let mut e = PatchEngine::new(&h, &patch, 48000.0, 8).unwrap();
    let (mut ptx, _prx) = rtrb::RingBuffer::<ProbeReport>::new(1);
    let (mut etx, _erx) = rtrb::RingBuffer::<cpal::Error>::new(1);
    let lost = AtomicU64::new(0);
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    for i in 0..400u64 {
        let g = (i % 40 == 0).then(|| e.build_swap(&h, &patch).unwrap());
        // Made off the audio thread, like cpal does; handed over on it.
        let err = cpal::Error::with_message(
            cpal::ErrorKind::DeviceNotAvailable,
            format!("device went away #{i}"),
        );
        assert_no_alloc(|| {
            if let Some(g) = g {
                e.receive_swap(g);
            }
            if i % 50 == 0 {
                e.command(&Command::Inspect(Some(ProbeTarget {
                    token: i,
                    module: 26,
                    port: 0,
                    kind: "filter.svf",
                })));
            }
            e.process_block(&mut l, &mut r);
            if let Some(rep) = e.take_probe_report() {
                let _ = ptx.push(rep);
            }
            hand_off_stream_error(err, &mut etx, &lost);
        });
    }
    assert_eq!(
        lost.load(Ordering::Relaxed),
        399,
        "one delivered, the rest forgotten"
    );
    log.shutdown();
}
