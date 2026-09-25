//! D03-R1: the stream-error hand-off keeps outstanding error ownership bounded while the
//! consumer is paused, counts every error by kind, recovers when drained, and leaves nothing
//! behind at shutdown. Both binaries call `hand_off_stream_error` from their error callbacks.
//!
//! This drives the hand-off directly (no audio device fails here). Errors are built the way
//! cpal 0.18.2's ALSA backend builds them, on the thread that then calls the error callback:
//! `BackendError` with a message from `alsa::Error`'s `Display` (heap; the only owned message
//! on this build), `DeviceNotAvailable` with a static message, and message-less kinds.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicIsize, Ordering};

use kabl_standalone::{hand_off_stream_error, StreamErrorCounts, STREAM_ERROR_QUEUE};

/// Live heap blocks, process-wide, plus this thread's allocation and free counts.
struct Counting;
static LIVE: AtomicIsize = AtomicIsize::new(0);
thread_local! {
    static ALLOCS: Cell<u64> = const { Cell::new(0) };
    static FREES: Cell<u64> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        LIVE.fetch_add(1, Ordering::Relaxed);
        ALLOCS.with(|c| c.set(c.get() + 1));
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(1, Ordering::Relaxed);
        FREES.with(|c| c.set(c.get() + 1));
        System.dealloc(p, l)
    }
}

#[global_allocator]
static A: Counting = Counting;

fn live() -> isize {
    LIVE.load(Ordering::Relaxed)
}

/// (allocations, frees) made on this thread by `f`.
fn counted(f: impl FnOnce()) -> (u64, u64) {
    let (a, d) = (ALLOCS.with(Cell::get), FREES.with(Cell::get));
    f();
    (ALLOCS.with(Cell::get) - a, FREES.with(Cell::get) - d)
}

fn backend_error(i: u64) -> cpal::Error {
    // `alsa::Error`'s Display, as cpal's `From<alsa::Error>` formats it: one heap block.
    cpal::Error::with_message(
        cpal::ErrorKind::BackendError,
        format!("ALSA function 'snd_pcm_avail_delay' failed with error 'EIO: I/O error ({i})'"),
    )
}

#[test]
fn paused_consumer_keeps_ownership_bounded_and_recovers() {
    use cpal::ErrorKind::*;
    let before = live();
    let (mut tx, mut rx) = rtrb::RingBuffer::<cpal::Error>::new(STREAM_ERROR_QUEUE);
    let ring = live() - before;
    let counts = StreamErrorCounts::default();
    let kind = |k: cpal::ErrorKind| {
        let i = kabl_standalone::STREAM_ERROR_KINDS
            .iter()
            .position(|x| *x == k)
            .unwrap();
        counts.by_kind[i].load(Ordering::Relaxed)
    };
    std::thread::scope(|s| s.spawn(|| {}).join().unwrap()); // warm up thread spawning
    let base = live();

    // Paused consumer: 1000 owned errors plus other kinds, on an "audio" thread.
    std::thread::scope(|s| {
        s.spawn(|| {
            let inner = live();
            for i in 0..1000u64 {
                let err = backend_error(i);
                let (a, d) = counted(|| hand_off_stream_error(err, &mut tx, &counts));
                let queued = i < STREAM_ERROR_QUEUE as u64;
                assert_eq!(a, 0, "hand-off never allocates");
                assert_eq!(d, u64::from(!queued), "overflow frees only that message");
                assert!(
                    live() - inner <= STREAM_ERROR_QUEUE as isize,
                    "bounded at {i}"
                );
                if i % 100 == 50 {
                    let others = [
                        cpal::Error::with_message(DeviceNotAvailable, "Device disconnected"),
                        cpal::Error::from(DeviceBusy),
                        cpal::Error::from(Xrun), // bare: callers count it; not handed over
                    ];
                    for e in others {
                        let (a, d) = counted(|| hand_off_stream_error(e, &mut tx, &counts));
                        assert_eq!((a, d), (0, 0), "no message, nothing to free");
                    }
                }
            }
            assert_eq!(live() - inner, STREAM_ERROR_QUEUE as isize);
        });
    });
    assert_eq!(
        live() - base,
        STREAM_ERROR_QUEUE as isize,
        "only the queued messages"
    );
    assert_eq!(kind(BackendError), 1000);
    assert_eq!(kind(DeviceNotAvailable), 10);
    assert_eq!(kind(DeviceBusy), 10);
    assert_eq!(kind(Xrun), 0);
    let total = 1020;
    assert_eq!(
        counts.undelivered.load(Ordering::Relaxed),
        total - STREAM_ERROR_QUEUE as u64
    );
    let mut seen = [0; kabl_standalone::STREAM_ERROR_KINDS.len() + 2];
    assert_eq!(
        counts.since(&mut seen),
        (
            "DeviceBusy=10 DeviceNotAvailable=10 BackendError=1000".to_string(),
            total - STREAM_ERROR_QUEUE as u64
        )
    );
    assert_eq!(counts.since(&mut seen), (String::new(), 0));

    // Drain: the first errors arrive whole; their messages are freed here.
    let mut n = 0;
    while let Ok(e) = rx.pop() {
        assert_eq!(e.kind(), BackendError);
        assert!(e.message().unwrap().ends_with(&format!("({n})'")));
        n += 1;
    }
    assert_eq!(n, STREAM_ERROR_QUEUE as u64);
    assert_eq!(live(), base, "drained");

    // Recovery: new errors are delivered again.
    std::thread::scope(|s| {
        s.spawn(|| {
            for i in 0..5 {
                hand_off_stream_error(backend_error(i), &mut tx, &counts);
            }
        });
    });
    assert_eq!(counts.since(&mut seen), ("BackendError=5".to_string(), 0));
    assert_eq!(live() - base, 5);

    // Shutdown with errors still queued: dropping both ends frees them and the queue.
    drop(tx);
    drop(rx);
    assert_eq!(live(), base - ring, "nothing outstanding after shutdown");
}
