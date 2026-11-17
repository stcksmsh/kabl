//! The AccessKit tree of a small patch: painted rack widgets (knobs, jacks, module faces) and
//! icon-only buttons are named and have a role a screen reader can announce.
//!
//! `KABL_DUMP_TREE=<path>` writes the whole tree as text (docs/accesskit/tree.txt).
use egui::accesskit::{Role, TreeUpdate};
use egui::{Pos2, RawInput, Rect};
use kabl_core::PortRef;
use kabl_ui::{show, PatchEditor, UiState};

/// The mockup's patch: sequencer, VA oscillator, SVF, ADSR, mixer, delay, reverb, output.
fn mockup_patch() -> PatchEditor {
    let mut e = PatchEditor::seed_from(&Default::default());
    let at = |x: f32, y: f32| kabl_core::Vec2 { x, y };
    let seq = e.add_module("seq", at(24.0, 20.0));
    let osc = e.add_module("osc.va", at(714.0, 20.0));
    let svf = e.add_module("filter.svf", at(964.0, 20.0));
    let adsr = e.add_module("env.adsr", at(24.0, 320.0));
    let mix = e.add_module("mixer", at(244.0, 320.0));
    let delay = e.add_module("delay", at(504.0, 320.0));
    let verb = e.add_module("reverb", at(744.0, 320.0));
    let out = e.add_module("out", at(984.0, 320.0));
    let m = |id, port: &str| PortRef::Module {
        id,
        port: port.to_string(),
    };
    e.connect(m(seq, "pitch"), m(osc, "pitch"));
    e.connect(m(osc, "out"), m(svf, "in"));
    e.connect(m(seq, "gate"), m(adsr, "gate"));
    e.connect_route(m(adsr, "out"), svf, "cutoff_hz");
    e.connect(m(svf, "lp"), m(mix, "in1"));
    e.connect(m(mix, "out"), m(delay, "in"));
    e.connect(m(delay, "out"), m(verb, "in"));
    e.connect(m(verb, "out"), m(out, "in"));
    e
}

/// One row per named node: `Role  "label"  [value]`, in tree order.
fn rows(update: &TreeUpdate) -> Vec<(Role, String, Option<f64>)> {
    update
        .nodes
        .iter()
        .filter_map(|(_, n)| Some((n.role(), n.label()?.to_string(), n.numeric_value())))
        .collect()
}

fn tree() -> Vec<(Role, String, Option<f64>)> {
    let size = egui::vec2(1440.0, 900.0);
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut editor = mockup_patch();
    let mut ui_state = UiState::default();
    let mut update = None;
    for i in 0..6 {
        ctx.begin_pass(RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            time: Some(f64::from(i) / 60.0),
            ..Default::default()
        });
        let mut root = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::ZERO, size)),
        );
        show(&mut editor, &mut ui_state, &mut root);
        update = ctx.end_pass().platform_output.accesskit_update;
    }
    rows(&update.expect("AccessKit is enabled, so every pass emits a tree"))
}

#[test]
fn the_rack_is_named_in_the_accessibility_tree() {
    let rows = tree();
    if let Ok(path) = std::env::var("KABL_DUMP_TREE") {
        // The tree's node map has no order: sort so the dump diffs cleanly.
        let mut sorted: Vec<_> = rows.iter().collect();
        sorted.sort_by(|a, b| (format!("{:?}", a.0), &a.1).cmp(&(format!("{:?}", b.0), &b.1)));
        let mut text = String::new();
        for (role, label, value) in sorted {
            match value {
                Some(v) => text += &format!("{role:?}\t{label}\t{v}\n"),
                None => text += &format!("{role:?}\t{label}\n"),
            }
        }
        std::fs::write(path, text).expect("write tree dump");
    }
    let has = |role: Role, label: &str| {
        rows.iter()
            .any(|(r, l, _)| *r == role && l.as_str() == label)
    };

    // Knobs are sliders named "<Module> <Param>" with the param's value.
    let knob = rows
        .iter()
        .find(|(r, l, _)| *r == Role::Slider && l.contains("Filter") && l.contains("Cutoff"))
        .unwrap_or_else(|| panic!("no filter cutoff slider in {rows:#?}"));
    assert!(knob.2.is_some(), "a knob reports its value: {knob:?}");

    // Jacks are buttons that say module, port, direction and cable count.
    assert!(
        rows.iter().any(|(r, l, _)| *r == Role::Button
            && l.contains(" out, 1 cable")
            && l.contains("Oscillator")),
        "no patched oscillator output jack in {rows:#?}"
    );
    assert!(
        rows.iter()
            .any(|(r, l, _)| *r == Role::Button && l.ends_with(" in, 0 cables")),
        "no unpatched input jack in {rows:#?}"
    );

    // Module faces are named panes.
    assert!(
        rows.iter()
            .any(|(r, l, _)| *r == Role::Pane && l.ends_with(" module")),
        "no module face group in {rows:#?}"
    );

    // Icon-only buttons say what they do, never the variant's Debug name.
    for (role, label) in [
        (Role::Button, "Undo"),
        (Role::Button, "Redo"),
        (Role::Button, "Fit to view"),
    ] {
        assert!(has(role, label), "no {label} {role:?} in {rows:#?}");
    }
    for (_, label, _) in &rows {
        assert!(
            !matches!(label.as_str(), "Plus" | "Minus" | "StarFill" | "CableOff"),
            "icon button announced by its variant: {label}"
        );
    }
}
