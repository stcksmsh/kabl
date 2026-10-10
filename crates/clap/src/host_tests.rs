use super::*;
use assert_no_alloc::assert_no_alloc;
use kabl_core::Vec2;

#[test]
fn stable_lanes_legacy_state_and_atomic_rejection() {
    let p = Instrument::default();
    let ids: Vec<_> = p
        .shared
        .params
        .param_map()
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    assert_eq!(ids[0], "output_gain");
    assert_eq!(
        &ids[1..17],
        &(1..=16).map(|i| format!("slot_{i}")).collect::<Vec<_>>()
    );
    assert_eq!(ids[17], "host_clock");
    let original = serde_json::to_vec(&p.shared.snapshot()).unwrap();
    let mut invalid = p.shared.snapshot();
    invalid.lanes[1] = invalid.lanes[0].clone();
    assert!(p.shared.load(invalid).is_err());
    assert_eq!(serde_json::to_vec(&p.shared.snapshot()).unwrap(), original);
    let mut json: serde_json::Value = serde_json::from_slice(&original).unwrap();
    json["version"] = 1.into();
    for key in ["lanes", "slot_values", "host_clock"] {
        json.as_object_mut().unwrap().remove(key);
    }
    let legacy = SoundState::decode(&serde_json::to_vec(&json).unwrap()).unwrap();
    assert!(!legacy.host_clock && legacy.lanes.iter().all(|l| l.target.is_none()));
    {
        let mut c = p.shared.control.lock().unwrap();
        c.view.learn = Some((1, "m1".into()));
    }
    p.shared.load(legacy).unwrap();
    let mut c = p.shared.control.lock().unwrap();
    assert!(c.view.learn.is_none());
    let target = automation::candidates(c.editor.state())[0].clone();
    c.lanes[0].target = Some(target.clone());
    c.editor
        .move_module(target.module, Vec2 { x: 800.0, y: 400.0 });
    c.pump();
    assert_eq!(c.lanes[0].target, Some(target.clone()));
    c.editor.remove_module(target.module);
    c.pump();
    assert!(c.lanes[0].target.is_none() && c.lanes[0].retired);
    c.editor.undo();
    c.pump();
    assert!(c.lanes[0].target.is_none() && c.lanes[0].retired);
}

fn scheduled_render(partitions: &[usize]) -> Vec<f32> {
    let patch = kabl_standalone::default_patch();
    let collector = Collector::new();
    let mut engine = PatchEngine::new(&collector.handle(), &patch, 48000.0, VOICES).unwrap();
    let target = automation::Target {
        module: 3,
        kind: "filter.svf".into(),
        param: "cutoff_hz".into(),
    };
    let mut bank = [None; automation::SLOTS];
    bank[0] = Some(automation::resolve(&patch, &target).unwrap());
    let mut values = [0.0; automation::SLOTS];
    values[0] = 0.4;
    let mut schedule = schedule::Schedule::new(values);
    schedule.mode(true);
    let position = |seconds, beats, tempo, playing| {
        schedule::Event::Position(schedule::Position {
            seconds,
            beats,
            tempo,
            playing,
            valid: true,
        })
    };
    schedule.push(0, position(0.0, 0.0, 120.0, true));
    schedule.push(48000, position(1.0, 2.0, 140.0, true));
    schedule.push(72000, position(1.5, 19.0 / 6.0, 140.0, false));
    // Intentionally inserted out of order: raw lists are merged without allocation.
    schedule.push(
        63,
        schedule::Event::Value {
            slot: 0,
            value: 0.9,
            modulation: false,
        },
    );
    schedule.push(
        12000,
        schedule::Event::Value {
            slot: 0,
            value: -0.1,
            modulation: true,
        },
    );
    let mut timeline = Timeline::new();
    let total = 96000;
    for (at, bytes) in [
        (0, [0x90, 60, 100]),
        (10000, [0x90, 67, 100]),
        (60000, [0x80, 60, 0]),
        (70000, [0x80, 67, 0]),
    ] {
        timeline.push(at, total, MidiEvent::parse(Source::Host, &bytes).unwrap());
    }
    let mut left = vec![0.0; total];
    let mut right = vec![0.0; total];
    let mut at = 0;
    let mut index = 0;
    while at < total {
        let n = partitions[index % partitions.len()].min(total - at);
        assert_no_alloc(|| {
            timeline.render(
                &mut engine,
                &mut left[at..at + n],
                &mut right[at..at + n],
                |engine, start| schedule.block(engine, start, &bank, 48000.0),
            )
        });
        at += n;
        index += 1;
    }
    assert_eq!(engine.keys().0, 0);
    assert!(left.iter().any(|v| v.abs() > 0.001));
    left
}
#[test]
fn host_automation_tempo_stop_schedule_is_partition_independent_and_realtime_safe() {
    let reference = scheduled_render(&[256]);
    for partitions in [&[1][..], &[63, 65, 127, 512][..]] {
        assert_eq!(reference, scheduled_render(partitions));
    }
}

#[test]
fn missing_transport_stops_host_clocks_but_free_clock_keeps_running() {
    let mut patch = kabl_standalone::default_patch();
    patch.modules.insert(
        20,
        kabl_core::ModuleState {
            kind: "clock".into(),
            pos: Vec2::default(),
            params: Default::default(),
        },
    );
    let collector = Collector::new();
    let mut engine = PatchEngine::new(&collector.handle(), &patch, 48000.0, VOICES).unwrap();
    let mut schedule = schedule::Schedule::new([0.0; automation::SLOTS]);
    let mut running = false;
    engine.clocks(|_, run, _| running = run);
    assert!(running);
    schedule.mode(true);
    assert_no_alloc(|| schedule.block(&mut engine, 0, &[None; automation::SLOTS], 48000.0));
    engine.clocks(|_, run, _| running = run);
    assert!(!running);
    schedule.mode(false);
    assert_no_alloc(|| schedule.block(&mut engine, 64, &[None; automation::SLOTS], 48000.0));
    engine.clocks(|_, run, _| running = run);
    assert!(running);
}

#[test]
fn unassigning_a_lane_restores_its_document_base_without_allocations() {
    let mut patch = kabl_standalone::default_patch();
    patch
        .modules
        .get_mut(&5)
        .unwrap()
        .params
        .insert("gain".into(), 1.0);
    patch.cables.remove(&5);
    let collector = Collector::new();
    let mut engine = PatchEngine::new(&collector.handle(), &patch, 48000.0, VOICES).unwrap();
    let resolved = automation::resolve(
        &patch,
        &automation::Target {
            module: 5,
            kind: "vca".into(),
            param: "gain".into(),
        },
    )
    .unwrap();
    let mut bank = [None; automation::SLOTS];
    bank[0] = Some(resolved);
    engine.key_at(MidiEvent::parse(Source::Host, &[0x90, 60, 100]).unwrap(), 0);
    engine.automate(0, resolved.target, 0.0);
    let mut left = [0.0; 64];
    let mut right = [0.0; 64];
    for _ in 0..100 {
        assert_no_alloc(|| engine.process_block(&mut left, &mut right));
    }
    assert!(left.iter().all(|v| v.abs() < 1e-6));
    let restored = bank;
    assert_no_alloc(|| {
        automation::replace_bank(&mut engine, &mut bank, [None; automation::SLOTS], restored)
    });
    let mut peak = 0.0f32;
    for _ in 0..100 {
        assert_no_alloc(|| engine.process_block(&mut left, &mut right));
        peak = left.iter().fold(peak, |max, v| max.max(v.abs()));
    }
    assert!(
        peak > 0.001,
        "Unassigned target must recover its base: {peak}"
    );
}

#[test]
fn live_mode_and_values_wait_for_complete_scene_publication() {
    let mut p = tests::instrument();
    let expected = p.session.as_ref().unwrap().epoch * 2;
    p.shared.committed.store(expected + 1, Ordering::SeqCst);
    unsafe {
        p.shared
            .params
            .host_clock
            .as_ptr()
            ._internal_set_normalized_value(1.0);
        p.shared.params.slots[0]
            .value
            .as_ptr()
            ._internal_set_normalized_value(0.9);
    }
    p.shared.gesture(0, 0.9);
    let (gestures, _) = assert_no_alloc(|| p.live_params());
    assert_eq!(gestures, 0);
    assert!(!p.session.as_ref().unwrap().host_clock);
    assert_eq!(p.shared.gestures.load(Ordering::Acquire), 1);
    // A completed new epoch must not leak into the old session either.
    p.shared.committed.store(expected + 2, Ordering::SeqCst);
    assert_eq!(assert_no_alloc(|| p.live_params()).0, 0);
    assert!(!p.session.as_ref().unwrap().host_clock);
    p.shared.committed.store(expected, Ordering::SeqCst);
    let state = p.shared.snapshot();
    p.shared.load(state).unwrap();
    assert_no_alloc(|| p.accept_loads());
    assert!(p.session.as_ref().unwrap().host_clock);
    assert!((p.session.as_ref().unwrap().slot_values[0] - 0.9).abs() < 1e-6);
}

#[test]
fn fresh_browser_load_does_not_restore_old_bases_into_reused_module_ids() {
    let collector = Collector::new();
    let mut old = kabl_standalone::default_patch();
    old.modules
        .get_mut(&5)
        .unwrap()
        .params
        .insert("gain".into(), 1.0);
    old.cables.remove(&5);
    let target = automation::Target {
        module: 5,
        kind: "vca".into(),
        param: "gain".into(),
    };
    let mut bank = [None; automation::SLOTS];
    bank[0] = automation::resolve(&old, &target);
    let mut fresh = old.clone();
    fresh
        .modules
        .get_mut(&5)
        .unwrap()
        .params
        .insert("gain".into(), 0.25);
    let mut engine = PatchEngine::new(&collector.handle(), &fresh, 48000.0, VOICES).unwrap();
    let mut reference = PatchEngine::new(&collector.handle(), &fresh, 48000.0, VOICES).unwrap();
    assert_no_alloc(|| {
        automation::replace_bank(
            &mut engine,
            &mut bank,
            [None; automation::SLOTS],
            [None; automation::SLOTS],
        )
    });
    let note = MidiEvent::parse(Source::Host, &[0x90, 60, 100]).unwrap();
    engine.key_at(note, 0);
    reference.key_at(note, 0);
    for _ in 0..100 {
        let mut left = [0.0; 64];
        let mut right = [0.0; 64];
        let mut expected = [0.0; 64];
        let mut expected_right = [0.0; 64];
        assert_no_alloc(|| {
            engine.process_block(&mut left, &mut right);
            reference.process_block(&mut expected, &mut expected_right);
        });
        assert_eq!(left, expected);
        assert_eq!(right, expected_right);
    }
}

#[test]
fn rejected_browser_load_keeps_previous_automation_bank() {
    let mut p = tests::instrument();
    let mut c = p.shared.control.lock().unwrap();
    let lanes = c.lanes.clone();
    let mut invalid = c.patch.clone();
    invalid.modules.get_mut(&2).unwrap().kind = "missing.kind".into();
    c.editor = PatchEditor::seed_from(&invalid);
    c.view.loaded = true;
    c.view.load_stopped = true;
    assert!(!c.pump());
    assert_eq!(c.lanes, lanes);
    assert!(c.view.loaded && c.view.load_stopped);
    let mut valid = c.patch.clone();
    valid
        .modules
        .get_mut(&2)
        .unwrap()
        .params
        .insert("base_hz".into(), 330.0);
    c.editor = PatchEditor::seed_from(&valid);
    assert!(c.pump());
    assert!(c.lanes.iter().all(|lane| lane.target.is_none()));
    drop(c);
    p.accept_loads();
    assert!(p.session.as_ref().unwrap().bank.iter().all(Option::is_none));
}

#[test]
fn queued_old_banks_cannot_change_fresh_browser_document() {
    let mut p = tests::instrument();
    let mut c = p.shared.control.lock().unwrap();
    c.lanes.fill(automation::Lane::default());
    c.bank_dirty = true;
    c.pump(); // Queue an old unassignment without processing it.
    let mut fresh = c.patch.clone();
    fresh
        .modules
        .get_mut(&5)
        .unwrap()
        .params
        .insert("gain".into(), 0.25);
    fresh.cables.remove(&5);
    c.editor = PatchEditor::seed_from(&fresh);
    c.view.loaded = true;
    assert!(c.pump());
    drop(c);
    assert_no_alloc(|| p.accept_loads());
    let collector = Collector::new();
    let mut reference = PatchEngine::new(&collector.handle(), &fresh, 48000.0, VOICES).unwrap();
    let note = MidiEvent::parse(Source::Host, &[0x90, 60, 100]).unwrap();
    let session: &mut Session = p.session.as_mut().unwrap();
    session.engine.key_at(note, 0);
    reference.key_at(note, 0);
    for _ in 0..100 {
        let mut left = [0.0; 64];
        let mut right = [0.0; 64];
        let mut expected = [0.0; 64];
        let mut expected_right = [0.0; 64];
        assert_no_alloc(|| {
            session
                .engine
                .drain(&mut session.rx, kabl_ui::control::QUEUE, &session.feedback);
            while let Ok((bank, restored)) = session.banks.pop() {
                automation::replace_bank(&mut session.engine, &mut session.bank, bank, restored);
            }
            session.engine.process_block(&mut left, &mut right);
            reference.process_block(&mut expected, &mut expected_right);
        });
        assert_eq!(left, expected);
        assert_eq!(right, expected_right);
    }
}
#[test]
fn requested_rack_value_does_not_wait_for_native_host_echo() {
    let mut p = tests::instrument();
    let native = p.shared.params.slots[0].value.unmodulated_plain_value();
    p.shared.gesture(0, 0.91);
    let (mask, values) = assert_no_alloc(|| p.live_params());
    assert_eq!(mask, 1);
    assert_eq!(values[0], 0.91);
    assert_eq!(
        p.shared.params.slots[0].value.unmodulated_plain_value(),
        native
    );
    assert_eq!(p.live_params().0, 0);
}

#[test]
fn state_load_replaces_pending_exact_values_even_if_old_mask_is_retried() {
    let mut p = tests::instrument();
    p.shared.gesture(0, 0.91);
    p.shared.flushed_values[0].store(0.92f32.to_bits(), Ordering::Relaxed);
    p.shared.value_notify.store(1, Ordering::Release);
    p.shared.flushed_modulation[0].store(0.5f32.to_bits(), Ordering::Relaxed);
    p.shared.modulation_notify.store(1, Ordering::Release);
    let gesture = p.shared.gestures.swap(0, Ordering::Acquire);
    let value = p.shared.value_notify.swap(0, Ordering::Acquire);
    let modulation = p.shared.modulation_notify.swap(0, Ordering::Acquire);
    let mut loaded = p.shared.snapshot();
    loaded.slot_values[0] = 0.2;
    p.shared.load(loaded).unwrap();
    // Force old audio callback's retry after main thread has completed replacement.
    p.shared.gestures.fetch_or(gesture, Ordering::Release);
    p.shared.value_notify.fetch_or(value, Ordering::Release);
    p.shared
        .modulation_notify
        .fetch_or(modulation, Ordering::Release);
    assert_no_alloc(|| p.shared.apply_recall());
    assert_no_alloc(|| p.accept_loads());
    assert_eq!(p.live_params().1[0], 0.2);
    assert_eq!(p.live_flush(false).1[0], 0.2);
    assert_eq!(p.live_flush(true).1[0], 0.0);
}

#[test]
fn state_recall_clears_native_modulation_before_restoring_saved_bases() {
    let mut p = tests::instrument();
    unsafe {
        p.shared.params.gain.as_ptr()._internal_modulate_value(-1.0);
        p.shared.params.slots[0]
            .value
            .as_ptr()
            ._internal_modulate_value(-1.0);
    }
    let mut state = p.shared.snapshot();
    state.output_gain = 0.5;
    state.slot_values[0] = 0.25;
    p.shared.load(state).unwrap();
    assert_eq!(p.shared.native_value(0, true), 0.5);
    assert_eq!(p.shared.native_value(1, true), 0.25);
    assert_no_alloc(|| p.shared.apply_recall());
    assert_eq!(p.shared.params.gain.value(), 0.5);
    assert_eq!(p.shared.params.gain.unmodulated_plain_value(), 0.5);
    assert_eq!(p.shared.params.slots[0].value.value(), 0.25);
    assert_eq!(
        p.shared.params.slots[0].value.unmodulated_plain_value(),
        0.25
    );
    assert_no_alloc(|| p.accept_loads());
    assert_eq!(p.session.as_ref().unwrap().gain, 0.5);
}

#[test]
fn queued_unassignment_cannot_overwrite_later_document_base_edit() {
    unassignment_base_edit(false);
    unassignment_base_edit(true);
}
fn unassignment_base_edit(saturated: bool) {
    let mut p = tests::instrument();
    let mut state = p.shared.snapshot();
    state.patch.cables.remove(&5);
    state
        .patch
        .modules
        .get_mut(&5)
        .unwrap()
        .params
        .insert("gain".into(), 0.5);
    p.shared.load(state).unwrap();
    p.accept_loads();
    let mut c = p.shared.control.lock().unwrap();
    if saturated {
        for _ in 0..4 {
            c.bank_dirty = true;
            c.pump();
        }
        assert_eq!(c.banks.slots(), 0);
    }
    for lane in &mut c.lanes {
        if lane
            .target
            .as_ref()
            .is_some_and(|t| t.module == 5 && t.param == "gain")
        {
            *lane = automation::Lane::default();
        }
    }
    c.bank_dirty = true;
    c.pump();
    c.editor.set_param(5, "gain", 0.25);
    assert!(c.pump());
    let fresh = c.patch.clone();
    drop(c);
    let collector = Collector::new();
    let mut reference = PatchEngine::new(&collector.handle(), &fresh, 48000.0, VOICES).unwrap();
    let note = MidiEvent::parse(Source::Host, &[0x90, 60, 100]).unwrap();
    p.session.as_mut().unwrap().engine.key_at(note, 0);
    reference.key_at(note, 0);
    for _ in 0..100 {
        let mut left = [0.0; 64];
        let mut right = [0.0; 64];
        assert_no_alloc(|| p.render(&mut left, &mut right));
    }
    if saturated {
        let mut c = p.shared.control.lock().unwrap();
        assert!(c.bank_dirty);
        c.pump();
        drop(c);
        let mut l = [0.0; 64];
        let mut r = [0.0; 64];
        for _ in 0..100 {
            p.render(&mut l, &mut r);
        }
    }
    let mut expected = [0.0; 64];
    let mut expected_right = [0.0; 64];
    let mut reference_timeline = Timeline::new();
    for _ in 0..(100 + 100 * usize::from(saturated)) {
        reference_timeline.render(
            &mut reference,
            &mut expected,
            &mut expected_right,
            |_, _| {},
        );
    }
    let mut left = [0.0; 64];
    let mut right = [0.0; 64];
    p.render(&mut left, &mut right);
    reference_timeline.render(
        &mut reference,
        &mut expected,
        &mut expected_right,
        |_, _| {},
    );
    let gain = p.shared.params.gain.value();
    assert!(left
        .iter()
        .zip(expected)
        .all(|(&v, e)| (v - e * gain).abs() < 1e-6));
}

#[test]
fn recall_after_native_sync_cannot_be_overwritten_by_old_callback() {
    let mut p = tests::instrument();
    assert_no_alloc(|| p.shared.apply_recall());
    let mut recalled = p.shared.snapshot();
    recalled.output_gain = 0.61;
    recalled.host_clock = true;
    recalled.slot_values = [0.37; automation::SLOTS];
    let barriers = std::sync::Barrier::new(2);
    let shared = p.shared.clone();
    std::thread::scope(|scope| {
        let paused = scope.spawn(|| {
            // Old callback has synchronized native caches, then producer commits recall.
            barriers.wait();
            barriers.wait();
            assert_no_alloc(|| unsafe {
                shared.native_ptr(0)._internal_set_normalized_value(0.2);
                shared.native_ptr(17)._internal_set_normalized_value(0.0);
                shared.native_ptr(1)._internal_set_normalized_value(0.9);
            });
        });
        barriers.wait();
        p.shared.load(recalled).unwrap();
        barriers.wait();
        paused.join().unwrap();
    });
    let observed = p.shared.snapshot();
    assert_eq!(observed.output_gain, 0.61);
    assert!(observed.host_clock);
    assert_eq!(observed.slot_values, [0.37; automation::SLOTS]);
    assert_no_alloc(|| p.accept_loads());
    assert_eq!(p.live_params().0, 0);
    assert_eq!(p.session.as_ref().unwrap().gain, 0.61);
    assert!(p.session.as_ref().unwrap().host_clock);
    assert_no_alloc(|| p.shared.apply_recall());
    assert_eq!(p.shared.params.gain.value(), 0.61);
    assert!(p.shared.params.host_clock.value());
    assert_eq!(p.shared.params.slots[0].value.value(), 0.37);
    assert_eq!(p.shared.native_epoch.load(Ordering::Acquire), 2);
}

#[test]
fn composite_embedded_state_preserves_lanes_and_rejects_legacy_envelope_atomically() {
    let p = tests::instrument();
    let mut state = p.shared.snapshot();
    let original = state.patch.clone();
    let lanes = state.lanes.clone();
    let mut editor = kabl_ui::PatchEditor::seed_from(&state.patch);
    let members = state
        .patch
        .modules
        .iter()
        .filter(|(_, m)| m.kind != "out" && m.kind != "midi.in")
        .map(|(&id, _)| id)
        .collect();
    kabl_ui::composites::encapsulate(
        &mut editor,
        members,
        std::collections::BTreeSet::new(),
        "Host composite",
    )
    .unwrap();
    state.patch = editor.state().clone();
    state.version = 3;
    assert_eq!(state.patch.modules, original.modules);
    assert_eq!(state.patch.cables, original.cables);
    p.shared.load(state).unwrap();
    let recalled = p.shared.snapshot();
    assert_eq!(recalled.lanes, lanes);
    assert!(!recalled.patch.composites.is_empty());
    let before = serde_json::to_vec(&recalled).unwrap();
    let mut invalid = recalled;
    invalid.version = 2;
    assert!(p.shared.load(invalid).is_err());
    assert_eq!(serde_json::to_vec(&p.shared.snapshot()).unwrap(), before);
}

/// A wavetable patch saved by the plugin and reopened in a fresh instance (a host project
/// reopen) plays the same: tables ride in the state, not in any file.
#[test]
fn wavetable_state_survives_a_host_project_reopen() {
    let p = tests::instrument();
    let mut state = p.shared.snapshot();
    let wav = kabl_modules::wavetable::import_wav(&{
        let mut b =
            b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x44\xac\0\0\x88\x58\x01\0\x02\0\x10\0data"
                .to_vec();
        b.extend(1200u32.to_le_bytes());
        for i in 0..600 {
            b.extend((((i as f32 * 0.05).sin() * 20000.0) as i16).to_le_bytes());
        }
        b
    })
    .unwrap();
    state.patch.tables.insert(
        2,
        kabl_core::Table {
            name: "mine.wav".into(),
            wav,
        },
    );
    let id = state.patch.modules.keys().max().unwrap() + 1;
    state.patch.modules.insert(
        id,
        kabl_core::ModuleState {
            kind: "osc.wt".into(),
            pos: Vec2 { x: 0., y: 0. },
            params: [("user".to_string(), 2.0)].into(),
        },
    );
    state.version = 4;
    p.shared.load(state).unwrap();

    let saved = serde_json::to_vec(&p.shared.snapshot()).unwrap();
    let reopened = tests::instrument();
    reopened
        .shared
        .load(SoundState::decode(&saved).unwrap())
        .unwrap();
    let again = reopened.shared.snapshot();
    assert_eq!(again.version, 4);
    assert_eq!(again.patch.tables[&2].name, "mine.wav");
    assert_eq!(serde_json::to_vec(&again).unwrap(), saved);
}
