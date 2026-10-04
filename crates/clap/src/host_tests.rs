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
    engine.clocks(|_, run| running = run);
    assert!(running);
    schedule.mode(true);
    assert_no_alloc(|| schedule.block(&mut engine, 0, &[None; automation::SLOTS], 48000.0));
    engine.clocks(|_, run| running = run);
    assert!(!running);
    schedule.mode(false);
    assert_no_alloc(|| schedule.block(&mut engine, 64, &[None; automation::SLOTS], 48000.0));
    engine.clocks(|_, run| running = run);
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
    assert_no_alloc(|| {
        automation::replace_bank(&mut engine, &mut bank, [None; automation::SLOTS], false)
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
    p.shared.gestures.store(1, Ordering::Release);
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
        automation::replace_bank(&mut engine, &mut bank, [None; automation::SLOTS], true)
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
    let (_, fresh) = p.session.as_mut().unwrap().banks.pop().unwrap();
    assert!(fresh);
}
