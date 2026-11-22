use super::*;
use basedrop::Collector;
use kabl_engine::keyboard::{KeyEvent, MidiEvent, Source};
use kabl_engine::runtime::Feedback;
use kabl_standalone::default_patch;
use kabl_ui::control::{self, Delivery};
use kabl_ui::{record, PatchEditor, UiState};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// D06: CC 1 is the wheel for the audio thread and still a learnable CC for mappings
/// (soft takeover unchanged); bend and CC 121 go only to the audio thread; other CCs
/// only to the control thread; a disconnect ends the controller source, not everything.
#[test]
fn midi_messages_reach_the_wheel_and_the_mappings() {
    let (notes, mut notes_rx) = rtrb::RingBuffer::<Note>::new(64);
    let (mut midi, mut cc_rx) = Midi::new(notes);
    let send = |d: &[u8]| on_message(&mut midi.sink.lock().unwrap(), d);
    send(&[0xB2, 1, 99]);
    send(&[0xE2, 0, 0x60]);
    send(&[0xB2, 121, 0]);
    send(&[0xB2, 74, 10]);
    send(&[0x92, 60, 0]);
    let got: Vec<MidiEvent> = std::iter::from_fn(|| notes_rx.pop().ok().map(|n| n.1)).collect();
    let ev = |e| MidiEvent::new(Source::Controller, 2, e);
    assert_eq!(
        got,
        [
            ev(KeyEvent::Wheel(99)),
            ev(KeyEvent::Bend(0x60 << 7)),
            ev(KeyEvent::ResetControllers),
            ev(KeyEvent::Off { note: 60 }),
        ]
    );
    let ccs: Vec<(u8, u8, u8)> = std::iter::from_fn(|| cc_rx.pop().ok().map(|c| c.0)).collect();
    assert_eq!(ccs, [(2, 1, 99), (2, 74, 10)]);
    midi.disconnect();
    assert!(
        notes_rx.pop().is_err(),
        "nothing was connected: nothing to end"
    );
    midi.port = Some("kabl-player".into());
    midi.disconnect();
    assert_eq!(
        notes_rx.pop().unwrap().1,
        MidiEvent::new(Source::Controller, 0, KeyEvent::SourceLost)
    );
}

#[test]
fn delayed_compile_failure_and_repair_update_the_visible_notice() {
    let patch = default_patch();
    let collector = basedrop::Collector::new();
    let (tx, _rx) = rtrb::RingBuffer::new(control::QUEUE);
    let mut delivery = Delivery::new(
        Some(tx),
        collector.handle(),
        Arc::new(Feedback::default()),
        48000.0,
        8,
        Some(&patch),
    );
    delivery.async_compile = true;
    delivery.compile_delay_ms = 80;
    let mut editor = PatchEditor::seed_from(&patch);
    let mut notice = CompileNotice::new(&delivery);
    let mut status = "audio running · current graph installed".to_string();
    editor.add_module("no-such-module", kabl_core::Vec2 { x: 0.0, y: 0.0 });
    assert_eq!(
        delivery.sync(editor.state(), false, false),
        control::Outcome::Compiled
    );
    assert!(notice.observe(AudioPhase::Running, &mut status, &delivery));
    assert!(status.contains("compiling"));
    assert!(notice.error.is_none(), "the worker has not finished yet");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while delivery.compile_error.is_none() {
        delivery.flush();
        assert!(
            std::time::Instant::now() < deadline,
            "compile failure timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(notice.observe(AudioPhase::Running, &mut status, &delivery));
    assert!(status.contains("recompile failed"));
    assert!(notice.error.as_deref().unwrap().contains("no-such-module"));
    assert!(!notice.observe(AudioPhase::Running, &mut status, &delivery));

    editor.undo();
    assert_eq!(
        delivery.sync(editor.state(), false, false),
        control::Outcome::Compiled
    );
    assert!(
        delivery.compile_error.is_some(),
        "failure remains visible while compiling"
    );
    assert!(notice.observe(AudioPhase::Running, &mut status, &delivery));
    assert!(status.contains("recompile failed"));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while delivery.compile_error.is_some() {
        delivery.flush();
        assert!(
            std::time::Instant::now() < deadline,
            "compile repair timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(notice.observe(AudioPhase::Running, &mut status, &delivery));
    assert!(notice.error.is_none());
    assert!(
        status.contains("queued"),
        "must not claim playback before callback ack"
    );
    assert!(!notice.observe(AudioPhase::Running, &mut status, &delivery));
    assert!(status.contains("queued"));
    delivery
        .feedback
        .applied_rev
        .store(delivery.rev(), Ordering::Relaxed);
    assert!(!notice.observe(AudioPhase::Running, &mut status, &delivery));
    assert!(status.contains("installed"));

    // A failed retry leaves an actionable status even when a later edit compiles.
    editor.add_module("no-such-module", kabl_core::Vec2 { x: 0.0, y: 0.0 });
    assert_eq!(
        delivery.sync(editor.state(), false, false),
        control::Outcome::Compiled
    );
    status = "document cannot play: invalid graph; edit to repair, then Retry".into();
    assert!(notice.observe(AudioPhase::Failed, &mut status, &delivery));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while delivery.compile_error.is_none() {
        delivery.flush();
        assert!(
            std::time::Instant::now() < deadline,
            "retry compile failure timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(notice.observe(AudioPhase::Failed, &mut status, &delivery));
    editor.undo();
    assert_eq!(
        delivery.sync(editor.state(), false, false),
        control::Outcome::Compiled
    );
    notice.observe(AudioPhase::Failed, &mut status, &delivery);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while delivery.compile_error.is_some() {
        delivery.flush();
        assert!(
            std::time::Instant::now() < deadline,
            "retry repair timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(notice.observe(AudioPhase::Failed, &mut status, &delivery));
    assert!(status.contains("Retry"));
    assert!(!status.contains("playing"));
}

/// The real control pump and cpal callback keep processing a mapped CC while a 500 ms
/// Save or Open worker runs. The environment has a software PCM, not a controller.
#[test]
#[ignore = "set ALSA_CONFIG_PATH to an ALSA null PCM and run explicitly"]
fn cc_to_callback_during_delayed_save_and_open() {
    use kabl_ui::{
        browser,
        library::{Library, Meta},
        routing,
    };
    let dir = tempfile::tempdir().unwrap();
    let factory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches");
    let log = kabl_core::load(&factory.join("composition")).unwrap();
    // The library reads packed `.kabl` factory files, not the raw `patches/` folders.
    let mut lib = Library::open(
        Some(kabl_ui::library::built_factory_dir()),
        dir.path().to_path_buf(),
    );
    let meta = Meta {
        name: "CC latency piece".into(),
        ..Meta::default()
    };
    let id = lib.save(&log, meta, None).unwrap();
    let editor = PatchEditor::from_log(log);
    let mut ui = UiState::default();
    ui.library = Some(lib);
    ui.doc = Some(browser::Doc::new(
        "CC latency piece",
        browser::DocOrigin::Library(id),
        editor.state(),
    ));
    ui.browser.async_io = true;
    ui.browser.io_delay_ms = 500;
    let (_, notes_rx) = rtrb::RingBuffer::new(1024);
    let request = AudioRequest {
        rate: Some(48000),
        frames: Some(256),
        realtime: false,
        device: None,
        input: false,
    };
    let (mut host, mut delivery) = AudioHost::start(
        editor.state(),
        notes_rx,
        Arc::new(AtomicU64::new(0)),
        &request,
        false,
        Arc::new(record::PeakTap::default()),
    );
    assert!(host._stream.is_some(), "{}", host.status);
    delivery.async_compile = true;
    let feedback = delivery.feedback.clone();
    let core = Arc::new(Mutex::new(Core {
        editor,
        ui,
        delivery,
        latency: None,
        progress: None,
        record_finish: None,
        record_dir: String::new(),
        record_last: None,
        cc_cutoff: f64::NEG_INFINITY,
        stale_cc: 0,
    }));
    let (mut cc_tx, cc_rx) = rtrb::RingBuffer::new(128);
    let stop = Arc::new(AtomicBool::new(false));
    let pump_core = core.clone();
    let pump_stop = stop.clone();
    let pump_thread = std::thread::spawn(move || pump(pump_core, cc_rx, pump_stop));

    let send_and_measure =
        |core: &Arc<Mutex<Core>>, tx: &mut rtrb::Producer<((u8, u8, u8), f64)>| {
            let c = core.lock().unwrap();
            let before = c.delivery.rev();
            let state = c.editor.state();
            let module = &state.modules[&3];
            let info = kabl_modules::registry::info_for(&module.kind).unwrap();
            let p = info.params.iter().find(|p| p.name == "m1").unwrap();
            let at = (routing::base_value(state, 3, p) * 127.0).round() as u8;
            drop(c);
            let moved = if at < 107 { at + 20 } else { at - 20 };
            let start = std::time::Instant::now();
            tx.push(((0, 20, at), clock_s())).unwrap();
            tx.push(((0, 20, moved), clock_s())).unwrap();
            let deadline = start + std::time::Duration::from_millis(350);
            loop {
                let rev = core.lock().unwrap().delivery.rev();
                if rev > before && Feedback::get(&feedback.applied_rev) >= rev {
                    return start.elapsed();
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "CC did not reach callback within 350 ms during file I/O"
                );
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        };
    {
        let mut c = core.lock().unwrap();
        let Core { editor, ui, .. } = &mut *c;
        assert!(browser::save(editor, ui, None).is_none());
        assert!(browser::io_busy(ui));
    }
    let save_latency = send_and_measure(&core, &mut cc_tx);
    assert!(core.lock().unwrap().ui.browser.async_io);
    let save_deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        let mut c = core.lock().unwrap();
        if !browser::io_busy(&c.ui) {
            break;
        }
        assert!(
            std::time::Instant::now() < save_deadline,
            "Save worker did not finish"
        );
        let Core { editor, ui, .. } = &mut *c;
        browser::poll_io(editor, ui);
        drop(c);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    {
        let mut c = core.lock().unwrap();
        let Core { editor, ui, .. } = &mut *c;
        browser::perform(
            editor,
            ui,
            browser::Pending::Open("factory:palette/pad".into()),
        );
        assert!(browser::io_busy(ui));
        ui.takeover.clear();
    }
    let open_latency = send_and_measure(&core, &mut cc_tx);
    eprintln!(
        "CC to callback during 500 ms worker: Save {:.1} ms, Open {:.1} ms",
        save_latency.as_secs_f64() * 1e3,
        open_latency.as_secs_f64() * 1e3
    );
    assert!(save_latency.as_millis() < 350 && open_latency.as_millis() < 350);
    let open_deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        let mut c = core.lock().unwrap();
        if !browser::io_busy(&c.ui) {
            assert!(matches!(
                c.ui.browser.dialog,
                Some(browser::Dialog::Unsaved { .. })
            ));
            break;
        }
        assert!(
            std::time::Instant::now() < open_deadline,
            "Open worker did not finish"
        );
        let Core { editor, ui, .. } = &mut *c;
        browser::poll_io(editor, ui);
        drop(c);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    stop.store(true, Ordering::Release);
    pump_thread.thread().unpark();
    pump_thread.join().unwrap();
    drop(host._stream.take());
    drop(core);
    host.collector.collect();
}

#[test]
fn callback_failure_sticks_and_xrun_alone_does_not_fail() {
    let timing = Arc::new(CallbackTiming::default());
    let gate = Arc::new(AtomicU64::new(u64::MAX));
    let mut p = Progress::new(timing.clone(), gate.clone());
    timing.xruns.store(3, Ordering::Relaxed);
    timing.count.store(8, Ordering::Relaxed);
    p.observe();
    assert_eq!(p.failure, None);
    p.at = std::time::Instant::now() - std::time::Duration::from_secs(2);
    p.observe();
    assert_eq!(p.failure, Some(AudioPhase::Stalled));
    assert_eq!(gate.load(Ordering::Relaxed), 0);
    timing.count.store(9, Ordering::Relaxed);
    p.observe();
    assert_eq!(p.failure, Some(AudioPhase::Stalled));
    gate.store(2, Ordering::Relaxed);
    p.observe();
    assert_eq!(
        gate.load(Ordering::Relaxed),
        0,
        "sticky failure closes a later gate too"
    );
}

#[test]
fn stale_callback_cannot_reopen_a_failed_or_revised_gate() {
    let gate = AtomicU64::new(4);
    // Callback read revision 4; the detector failed or an edit became revision 5.
    gate.store(0, Ordering::Release);
    assert!(!unmute_if_current(&gate, 4));
    assert_eq!(gate.load(Ordering::Acquire), 0);
    gate.store(5, Ordering::Release);
    assert!(!unmute_if_current(&gate, 4));
    assert_eq!(gate.load(Ordering::Acquire), 5);
    assert!(unmute_if_current(&gate, 5));
    assert_eq!(gate.load(Ordering::Acquire), u64::MAX);
}

#[test]
fn retry_boundary_discards_only_earlier_cc_arrivals() {
    let events = [((0, 24, 50), 1.0), ((0, 46, 127), 2.0), ((0, 24, 60), 3.0)];
    let mut fresh = Vec::new();
    let (dropped, earliest) = fresh_cc(&events, 2.0, &mut fresh);
    assert_eq!(dropped, 2);
    assert_eq!(earliest, 3.0);
    assert_eq!(fresh, vec![(0, 24, 60)]);
}

/// Opt-in cpal stream and callback test against an ALSA null PCM. No physical device.
#[test]
#[ignore = "set ALSA_CONFIG_PATH to an ALSA null PCM and run explicitly"]
fn actual_backend_repeated_reopen_and_fault() {
    let patch = default_patch();
    let request = AudioRequest {
        rate: Some(48000),
        frames: Some(256),
        realtime: false,
        device: None,
        input: false,
    };
    let keys = Arc::new(AtomicU64::new(0));
    let peaks = Arc::new(record::PeakTap::default());
    let (_, rx) = rtrb::RingBuffer::new(1024);
    let mut invalid = request.clone();
    invalid.device = Some((usize::MAX, "missing".into()));
    let (unavailable, _) =
        AudioHost::start(&patch, rx, keys.clone(), &invalid, true, peaks.clone());
    assert!(unavailable._stream.is_none());
    let mut old = AudioHost::offline(Collector::new(), "offline".into());
    for (fail, delay) in [(true, 20), (false, 0), (false, 0)] {
        let (notes, rx) = rtrb::RingBuffer::new(1024);
        let at = std::time::Instant::now();
        let old_delivery = old.offline_delivery(&patch);
        let mut done = retry_worker(
            (old, old_delivery),
            notes,
            rx,
            keys.clone(),
            request.clone(),
            peaks.clone(),
            RetryFault {
                delay_ms: delay,
                fail_reopen: fail,
            },
        );
        assert_eq!(done.retired_graphs, 0);
        assert!(at.elapsed().as_millis() >= delay as u128);
        assert_eq!(done.audio._stream.is_some(), !fail);
        if !fail {
            assert_eq!(
                done.delivery.sync(&patch, true, true),
                control::Outcome::Compiled
            );
            done.audio
                .gate
                .store(done.delivery.rev(), Ordering::Relaxed);
            std::thread::sleep(std::time::Duration::from_millis(120));
            assert!(
                done.audio
                    .timing
                    .as_ref()
                    .unwrap()
                    .count
                    .load(Ordering::Relaxed)
                    > 0
            );
            assert_eq!(done.audio.gate.load(Ordering::Relaxed), u64::MAX);
        }
        drop(done.delivery);
        drop(done.notes);
        old = done.audio;
    }
    drop(old._stream.take());
    old.collector.collect();
    assert_eq!(old.collector.alloc_count(), 0);
}

/// Exercises the production cpal closure, including queue drain, MIDI, reports, gate,
/// recording tap and timing, after two startup callbacks. Requires ALSA null PCM.
#[test]
#[ignore = "set ALSA_CONFIG_PATH to an ALSA null PCM and run explicitly"]
fn production_callback_warmed_path_has_no_alloc_or_dealloc() {
    let patch = default_patch();
    let (mut notes, rx) = rtrb::RingBuffer::new(1024);
    let request = AudioRequest {
        rate: Some(48000),
        frames: Some(256),
        realtime: false,
        device: None,
        input: false,
    };
    let (mut host, mut delivery) = AudioHost::start(
        &patch,
        rx,
        Arc::new(AtomicU64::new(0)),
        &request,
        true,
        Arc::new(record::PeakTap::default()),
    );
    assert!(host._stream.is_some(), "{}", host.status);
    assert_eq!(
        delivery.sync(&patch, true, true),
        control::Outcome::Compiled
    );
    host.gate.store(delivery.rev(), Ordering::Relaxed);
    let dir = tempfile::tempdir().unwrap();
    host.recorder
        .as_mut()
        .unwrap()
        .start_at(dir.path().join("callback.wav"))
        .unwrap();
    notes
        .push(note_now(KeyEvent::On {
            note: 60,
            velocity: 96,
        }))
        .unwrap();
    // D06: expression through the timeline's queue in the same warmed callbacks.
    for e in [
        KeyEvent::Bend(12000),
        KeyEvent::Wheel(90),
        KeyEvent::Sustain(true),
    ] {
        notes.push(note_now(e)).unwrap();
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert_eq!(host.gate.load(Ordering::Relaxed), u64::MAX);
    drop(host._stream.take());
    assert!(host.timing.as_ref().unwrap().count.load(Ordering::Relaxed) > 2);
    assert_eq!(super::alloc_count::ALLOCS.load(Ordering::Relaxed), 0);
    assert_eq!(super::alloc_count::DEALLOCS.load(Ordering::Relaxed), 0);
    assert!(host.recorder.as_mut().unwrap().stop().is_some());
    drop(delivery);
    host.collector.collect();
}

#[test]
#[ignore = "set ALSA_CONFIG_PATH to an ALSA null PCM and run explicitly"]
fn actual_callback_fault_sticks_and_mutes() {
    std::env::set_var("KABL_AUDIO_FAULT", "stall");
    std::env::set_var("KABL_AUDIO_FAULT_AFTER", "3");
    let patch = default_patch();
    let (_, rx) = rtrb::RingBuffer::new(1024);
    let request = AudioRequest {
        rate: Some(48000),
        frames: Some(256),
        realtime: false,
        device: None,
        input: false,
    };
    let (mut host, delivery) = AudioHost::start(
        &patch,
        rx,
        Arc::new(AtomicU64::new(0)),
        &request,
        false,
        Arc::new(record::PeakTap::default()),
    );
    assert!(host._stream.is_some(), "{}", host.status);
    let mut progress = Progress::new(host.timing.as_ref().unwrap().clone(), host.gate.clone());
    std::thread::sleep(std::time::Duration::from_millis(120));
    progress.observe();
    assert!(progress.last >= 3);
    std::thread::sleep(std::time::Duration::from_millis(1600));
    progress.observe();
    assert_eq!(progress.failure, Some(AudioPhase::Stalled));
    assert_eq!(host.gate.load(Ordering::Relaxed), 0);
    drop(delivery);
    drop(host._stream.take());
    host.collector.collect();
    assert_eq!(host.collector.alloc_count(), 0);
    std::env::remove_var("KABL_AUDIO_FAULT");
    std::env::remove_var("KABL_AUDIO_FAULT_AFTER");
}
