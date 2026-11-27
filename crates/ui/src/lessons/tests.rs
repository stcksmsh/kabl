use super::run::complete;
use super::*;
use crate::browser::{Doc, DocOrigin};
use crate::inspect::Sel;
use crate::{PatchEditor, UiState};

const MINIMAL: &str = r#"
id = "t"
title = "T"
summary = "s"
start = { sound = "factory:x" }
[[step]]
text = "one"
[end]
unguided = "u"
"#;

#[test]
fn a_plugin_shows_plugin_text_when_the_step_has_it() {
    let mut s = parse("t.toml", MINIMAL).unwrap().steps[0].clone();
    assert_eq!(s.text_for(true), "one", "no plugin_text: the text");
    s.plugin_text = Some("from your host".into());
    assert_eq!(s.text_for(true), "from your host");
    assert_eq!(s.text_for(false), "one");
}

#[test]
fn every_embedded_lesson_parses_with_a_unique_id() {
    assert!(!FILES.is_empty());
    for (name, text) in FILES {
        parse(name, text).unwrap();
    }
    assert_eq!(all().len(), FILES.len(), "a lesson was dropped");
    let ids: BTreeSet<_> = all().iter().map(|l| &l.id).collect();
    assert_eq!(ids.len(), all().len());
}

#[test]
fn a_lesson_with_every_field_parses() {
    let text = r#"
id = "full"
title = "Full"
summary = "s"
start = { sound = "factory:recipes/filter-movement" }
[[step]]
text = "t"
show = { route = { from = 7, port = "out", to = 3, kind = "filter.svf", param = "cutoff_hz" } }
inspect = { id = 7, kind = "lfo", port = "out" }
play_note = 4.0
compare = true
anchor = "knob:3.*"
done = [
  { opened = "factory:recipes/filter-movement" },
  { param_moved = { id = 3, kind = "filter.svf", param = "cutoff_hz", by = 100.0 } },
  { route = { from = 7, port = "out", to = 3, kind = "filter.svf", param = "cutoff_hz" } },
  { inspecting = { id = 7, port = "out" } },
  "compare_restored",
]
[end]
unguided = "u"
"#;
    let l = parse("full.toml", text).unwrap();
    let s = &l.steps[0];
    assert_eq!(s.done.len(), 5);
    assert_eq!(s.done[4], Cond::CompareRestored);
    assert_eq!(
        s.show,
        Some(Target::Route {
            from: 7,
            port: "out".into(),
            to: 3,
            kind: "filter.svf".into(),
            param: "cutoff_hz".into()
        })
    );
    assert_eq!(parse("m.toml", MINIMAL).unwrap().steps[0].done, vec![]);
}

#[test]
fn bad_lessons_are_rejected_with_the_file_and_the_reason() {
    let cases = [
        (MINIMAL.replace("text = \"one\"", "txet = \"one\""), "unknown field `txet`"),
        (MINIMAL.replace("id = \"t\"\n", ""), "missing field `id`"),
        (MINIMAL.replace("id = \"t\"", "id = \" \""), "must not be empty"),
        (
            MINIMAL.replace("[[step]]\ntext = \"one\"\n", "step = []\n"),
            "at least one [[step]]",
        ),
        (
            MINIMAL.replace("text = \"one\"", "text = \"one\"\nplay_note = -1.0"),
            "step 1: `play_note`",
        ),
        (
            MINIMAL.replace("text = \"one\"", "text = \"one\"\nanchor = \"\""),
            "step 1: `anchor` is empty",
        ),
        (
            MINIMAL.replace("text = \"one\"", "text = \"one\"\ndone = [{ guessed = 1 }]"),
            "unknown variant `guessed`",
        ),
        (
            MINIMAL.replace(
                "text = \"one\"",
                "text = \"one\"\ndone = [{ param_moved = { id = 1, kind = \"lfo\", param = \"rate_hz\", by = 0.0 } }]",
            ),
            "`param_moved.by` must be above 0",
        ),
        (
            MINIMAL.replace(
                "text = \"one\"",
                "text = \"one\"\nshow = { control = { id = 1, kind = \"lfo\" } }",
            ),
            "missing field `param`",
        ),
        (MINIMAL.replace("[end]", "[end"), "lessons/bad.toml"),
    ];
    for (text, want) in cases {
        let e = parse("bad.toml", &text).expect_err(want);
        assert!(e.starts_with("lessons/bad.toml: "), "{e}");
        assert!(e.contains(want), "{want:?} not in {e}");
    }
}

/// Filter #3 (filter.svf), LFO #7 routed into its cutoff, bypassed.
fn fixture() -> (PatchEditor, UiState, kabl_core::CableId) {
    let e = patches::filter_movement();
    let cable = *e
        .state()
        .cables
        .iter()
        .find(|(_, c)| matches!(&c.to, kabl_core::PortRef::Param { .. }))
        .unwrap()
        .0;
    (e, UiState::default(), cable)
}

#[test]
fn conditions_observe_the_real_patch_and_view() {
    let (mut e, mut ui, cable) = fixture();
    let base = Baseline::of(&e);
    let moved = Cond::ParamMoved {
        id: 3,
        kind: "filter.svf".into(),
        param: "cutoff_hz".into(),
        by: 100.0,
    };
    let route = Cond::Route {
        from: 7,
        port: "out".into(),
        to: 3,
        kind: "filter.svf".into(),
        param: "cutoff_hz".into(),
        amount_below: None,
    };
    let inspecting = Cond::Inspecting {
        id: 7,
        port: "out".into(),
    };
    let opened = Cond::Opened("factory:recipes/filter-movement".into());
    for c in [&moved, &route, &inspecting, &opened] {
        assert!(
            !holds(c, &e, &ui, &base),
            "{c:?} holds before anything happened"
        );
    }

    e.set_param(3, "cutoff_hz", 850.0);
    assert!(!holds(&moved, &e, &ui, &base), "50 Hz is less than `by`");
    e.set_param(3, "cutoff_hz", 700.0);
    assert!(holds(&moved, &e, &ui, &base));
    // Measured from the step's start, not from the patch's first value.
    assert!(!holds(&moved, &e, &ui, &Baseline::of(&e)));
    // Same id, another kind: not the target.
    let wrong = Cond::ParamMoved {
        id: 3,
        kind: "lfo".into(),
        param: "cutoff_hz".into(),
        by: 1.0,
    };
    assert!(!holds(&wrong, &e, &ui, &base));

    e.set_route_bypass(cable, false);
    assert!(holds(&route, &e, &ui, &base));
    let negative = Cond::Route {
        from: 7,
        port: "out".into(),
        to: 3,
        kind: "filter.svf".into(),
        param: "cutoff_hz".into(),
        amount_below: Some(0.0),
    };
    assert!(!holds(&negative, &e, &ui, &base), "depth is +35 %");
    e.set_route_amount(cable, -0.2, true);
    assert!(holds(&negative, &e, &ui, &base));
    e.set_route_bypass(cable, true);
    assert!(!holds(&route, &e, &ui, &base));
    assert!(!holds(&negative, &e, &ui, &base));

    ui.inspect.select(
        Sel {
            id: 7,
            port: "out".into(),
        },
        0.0,
    );
    assert!(holds(&inspecting, &e, &ui, &base));

    ui.doc = Some(Doc::new(
        "Filter movement",
        DocOrigin::Library("factory:recipes/filter-movement".into()),
        e.state(),
    ));
    assert!(holds(&opened, &e, &ui, &base));
    ui.doc = Some(Doc::new(
        "Pad",
        DocOrigin::Library("factory:palette/pad".into()),
        e.state(),
    ));
    assert!(!holds(&opened, &e, &ui, &base));
}

#[test]
fn compare_restored_counts_only_a_restore_made_during_the_step() {
    let (mut e, mut ui, _) = fixture();
    ui.compare.capture(&e, "test");
    e.set_param(3, "cutoff_hz", 300.0);
    let base = Baseline::of(&e);
    assert!(!holds(&Cond::CompareRestored, &e, &ui, &base));
    crate::compare::restore(&mut e, &mut ui);
    assert!(holds(&Cond::CompareRestored, &e, &ui, &base));
    // Already restored when the next step starts: that step needs a restore of its own.
    assert!(!holds(&Cond::CompareRestored, &e, &ui, &Baseline::of(&e)));
    e.undo();
    assert!(!holds(&Cond::CompareRestored, &e, &ui, &base));
}

#[test]
fn a_step_completes_only_when_all_its_conditions_hold() {
    let (mut e, mut ui, cable) = fixture();
    let mut l = parse("m.toml", MINIMAL).unwrap();
    let a = Active {
        lesson: 0,
        step: 0,
        editor: e.instance(),
        detached: false,
        base: Baseline::of(&e),
        origin: Baseline::of(&e),
        completed_at: None,
        armed: false,
        done: vec![false; l.steps.len()],
        met_at_start: vec![false; l.steps.len()],
    };
    assert!(!complete(&l, &a, &e, &ui), "no conditions: Next only");
    l.steps[0].done = vec![
        Cond::Route {
            from: 7,
            port: "out".into(),
            to: 3,
            kind: "filter.svf".into(),
            param: "cutoff_hz".into(),
            amount_below: None,
        },
        Cond::Inspecting {
            id: 7,
            port: "out".into(),
        },
    ];
    e.set_route_bypass(cable, false);
    assert!(!complete(&l, &a, &e, &ui), "one of two");
    ui.inspect.select(
        Sel {
            id: 7,
            port: "out".into(),
        },
        0.0,
    );
    assert!(complete(&l, &a, &e, &ui));
}

#[test]
fn an_include_step_is_the_named_step_and_a_bad_include_drops_its_lesson() {
    let t = parse("t.toml", MINIMAL).unwrap();
    let named = |id: &str, r: &str| {
        let text = MINIMAL
            .replace("id = \"t\"", &format!("id = \"{id}\""))
            .replace("text = \"one\"", &format!("include = {r}"));
        parse(&format!("{id}.toml"), &text).unwrap()
    };
    let b = named("b", "{ lesson = \"t\", step = 1 }");
    let (got, errors) = resolve_includes(vec![t.clone(), b.clone()]);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(got[1].steps, t.steps, "the step as it is in lesson t");
    for (r, want) in [
        ("{ lesson = \"x\", step = 1 }", "unknown lesson \"x\""),
        ("{ lesson = \"t\", step = 2 }", "\"t\" has no step 2"),
        ("{ lesson = \"t\", step = 0 }", "\"t\" has no step 0"),
        ("{ lesson = \"b\", step = 1 }", "\"b\" step 1 is an include"),
    ] {
        let (got, errors) = resolve_includes(vec![t.clone(), b.clone(), named("c", r)]);
        assert_eq!(got.len(), 2, "{want}: lesson c is left out");
        assert!(errors.len() == 1 && errors[0].contains(want), "{errors:?}");
    }
    let both = MINIMAL.replace(
        "text = \"one\"",
        "text = \"one\"\ninclude = { lesson = \"t\", step = 1 }",
    );
    let e = parse("d.toml", &both).unwrap_err();
    assert!(
        e.contains("step 1: a step with `include` has no other field"),
        "{e}"
    );
    let neither = MINIMAL.replace("text = \"one\"", "anchor = \"browser\"");
    let e = parse("e.toml", &neither).unwrap_err();
    assert!(e.contains("step 1: `text` is empty"), "{e}");
}

#[test]
fn notes_played_sees_keys_and_midi_voices() {
    let (e, mut ui, _) = fixture();
    let base = Baseline::of(&e);
    assert!(!holds(&Cond::NotesPlayed, &e, &ui, &base));
    ui.status_voices = Some((0, 16));
    assert!(
        !holds(&Cond::NotesPlayed, &e, &ui, &base),
        "no voice sounds"
    );
    ui.status_voices = Some((1, 16));
    assert!(holds(&Cond::NotesPlayed, &e, &ui, &base));
    ui.status_voices = None;
    ui.keys.strip_to(Some(60));
    assert!(holds(&Cond::NotesPlayed, &e, &ui, &base), "the key strip");
    ui.keys.strip_to(None);
    assert!(!holds(&Cond::NotesPlayed, &e, &ui, &base));
}

#[test]
fn favorited_and_saved_as_user_look_at_the_open_document() {
    let (mut e, mut ui, _) = fixture();
    let base = Baseline::of(&e);
    let user = tempfile::tempdir().unwrap();
    ui.library = Some(crate::library::Library::open(None, user.path().into()));
    let doc = |id: &str, e: &PatchEditor| {
        Some(Doc::new(
            "Filter movement",
            DocOrigin::Library(id.into()),
            e.state(),
        ))
    };
    let factory = "factory:recipes/filter-movement";
    assert!(!holds(&Cond::Favorited, &e, &ui, &base), "no document");
    ui.doc = doc(factory, &e);
    assert!(!holds(&Cond::Favorited, &e, &ui, &base), "not a favorite");
    assert!(
        !holds(&Cond::SavedAsUser, &e, &ui, &base),
        "a factory sound"
    );
    let lib = ui.library.as_mut().unwrap();
    lib.prefs.favorites.insert(factory.into());
    assert!(holds(&Cond::Favorited, &e, &ui, &base));
    ui.doc = doc("factory:palette/pad", &e);
    assert!(!holds(&Cond::Favorited, &e, &ui, &base), "another sound");

    ui.doc = doc("user:my-filter-movement", &e);
    assert!(holds(&Cond::SavedAsUser, &e, &ui, &base));
    e.set_param(3, "cutoff_hz", 500.0);
    assert!(
        !holds(&Cond::SavedAsUser, &e, &ui, &base),
        "edited since the save"
    );
    ui.doc = Some(Doc::new("Untitled", DocOrigin::New, e.state()));
    assert!(!holds(&Cond::SavedAsUser, &e, &ui, &base), "never saved");
}

#[test]
fn preview_played_holds_while_a_sound_preview_plays() {
    let (e, mut ui, _) = fixture();
    let base = Baseline::of(&e);
    assert!(!holds(&Cond::PreviewPlayed, &e, &ui, &base));
    let user = tempfile::tempdir().unwrap();
    let lib = crate::library::Library::open(
        Some(crate::library::built_factory_dir()),
        user.path().into(),
    );
    let id = "factory:recipes/filter-movement";
    let entry = lib.get(id).unwrap().clone();
    let collector = basedrop::Collector::new();
    let (player, _voice) = crate::sound_preview::channel(collector.handle());
    ui.browser.clips.attach(Some(player));
    ui.browser
        .clips
        .ensure(id, &entry.dir, &entry.meta, 48000.0);
    let start = std::time::Instant::now();
    while !matches!(
        ui.browser.clips.look(id),
        crate::sound_preview::ClipLook::Ready(_)
    ) {
        assert!(start.elapsed().as_secs() < 60, "the clip never rendered");
        std::thread::sleep(std::time::Duration::from_millis(20));
        ui.browser.clips.poll();
    }
    assert!(
        !holds(&Cond::PreviewPlayed, &e, &ui, &base),
        "rendered, not played"
    );
    ui.browser.clips.play(id);
    assert!(holds(&Cond::PreviewPlayed, &e, &ui, &base));
    ui.browser.clips.stop();
    assert!(!holds(&Cond::PreviewPlayed, &e, &ui, &base));
}

#[test]
fn cable_removed_and_reaches_output_follow_the_vca_cable() {
    let (mut e, ui, _) = fixture();
    let base = Baseline::of(&e);
    let removed = Cond::CableRemoved {
        from: 4,
        port: "out".into(),
        to: 5,
        input: "in".into(),
    };
    let reaches = Cond::ReachesOutput {
        id: 4,
        kind: "vca".into(),
    };
    assert!(
        !holds(&removed, &e, &ui, &base),
        "VCA #4 is plugged into Output #5"
    );
    assert!(holds(&reaches, &e, &ui, &base));
    let wrong = Cond::ReachesOutput {
        id: 4,
        kind: "lfo".into(),
    };
    assert!(!holds(&wrong, &e, &ui, &base), "same id, another kind");
    let cable = *e
        .state()
        .cables
        .iter()
        .find(|(_, c)| {
            c.from
                == kabl_core::PortRef::Module {
                    id: 4,
                    port: "out".into(),
                }
        })
        .unwrap()
        .0;
    e.disconnect(cable);
    assert!(holds(&removed, &e, &ui, &base));
    assert!(!holds(&reaches, &e, &ui, &base));
    e.undo();
    assert!(!holds(&removed, &e, &ui, &base), "Undo plugs it back");
    assert!(holds(&reaches, &e, &ui, &base));
    e.remove_module(5);
    assert!(
        !holds(&removed, &e, &ui, &base),
        "a deleted Output is not an unplugged cable"
    );
}

#[test]
fn explain_open_and_pinned_name_one_control() {
    let (mut e, mut ui, _) = fixture();
    let base = Baseline::of(&e);
    let explain = Cond::ExplainOpen {
        id: 3,
        key: "cutoff_hz".into(),
    };
    let pinned = Cond::Pinned {
        id: 6,
        param: "sustain".into(),
    };
    assert!(!holds(&explain, &e, &ui, &base));
    let control = |key: &str| crate::explain::Subject::Control {
        id: 3,
        key: key.into(),
    };
    crate::explain::open(&mut ui, control("resonance"), None);
    assert!(!holds(&explain, &e, &ui, &base), "another control");
    crate::explain::open(&mut ui, control("cutoff_hz"), Some((3, "cutoff_hz".into())));
    assert!(holds(&explain, &e, &ui, &base));
    ui.explain.close();
    assert!(!holds(&explain, &e, &ui, &base));

    assert!(!holds(&pinned, &e, &ui, &base));
    crate::perform::toggle_pin(&mut e, 6, "release_ms");
    assert!(!holds(&pinned, &e, &ui, &base), "another control");
    crate::perform::toggle_pin(&mut e, 6, "sustain");
    assert!(holds(&pinned, &e, &ui, &base));
    e.undo();
    assert!(!holds(&pinned, &e, &ui, &base));
}
