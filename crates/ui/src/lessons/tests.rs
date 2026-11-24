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
        completed_at: None,
        armed: false,
        done: vec![false; l.steps.len()],
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
