//! A closed composite's face names each port and control once: no "IN In" / "OUT Out", and no
//! control name drawn both by the face label and by the knob.

use std::collections::BTreeSet;

use kabl_core::panel::{Kind, Panel, Placement};
use kabl_core::PortRef;
use kabl_ui::{composites, show, PatchEditor, UiState};

fn placement(kind: Kind, x: f32, y: f32, width: f32, height: f32) -> Placement {
    Placement {
        kind,
        x,
        y,
        width,
        height,
    }
}

fn texts(shape: &egui::Shape, out: &mut Vec<String>) {
    match shape {
        egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
        egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
        _ => {}
    }
}

/// Every string the closed composite's face paints, after relabelling its exposures with
/// `ports` (in key order) and `controls`.
fn face_texts(ports: &[&str], controls: &[&str]) -> Vec<String> {
    let mut e = PatchEditor::seed_from(&kabl_standalone::default_patch());
    let members = e
        .state()
        .modules
        .iter()
        .filter(|(_, m)| m.kind != "out" && m.kind != "midi.in")
        .map(|(&id, _)| id)
        .collect();
    let id = composites::encapsulate(&mut e, members, BTreeSet::new(), "Voice").unwrap();
    for param in ["cutoff_hz", "resonance"] {
        composites::add_exposure(
            &mut e,
            id,
            PortRef::Param {
                id: 3,
                param: param.into(),
            },
            true,
        )
        .unwrap();
    }
    let mut c = e.state().composites[&id].clone();
    for (x, label) in c.ports.values_mut().zip(ports) {
        x.label = label.to_string();
    }
    let added: Vec<_> = c
        .controls
        .values_mut()
        .filter(|x| matches!(&x.target, PortRef::Param { id: 3, param } if param == "cutoff_hz" || param == "resonance"))
        .collect();
    for (x, label) in added.into_iter().zip(controls) {
        x.label = label.to_string();
    }
    let mut panel = Panel {
        width: 480.0,
        ..Panel::default()
    };
    for (i, &key) in c.ports.keys().enumerate() {
        let x = 10.0 + 80.0 * i as f32;
        panel
            .placements
            .insert(key, placement(Kind::Jack, x, 220.0, 60.0, 60.0));
    }
    for (i, &key) in c.controls.keys().enumerate() {
        let x = 10.0 + 100.0 * i as f32;
        panel
            .placements
            .insert(key, placement(Kind::Knob, x, 70.0, 90.0, 120.0));
    }
    c.panel = Some(Box::new(panel));
    composites::set(&mut e, id, c).unwrap();

    let ctx = egui::Context::default();
    let mut view = UiState::default();
    let size = egui::vec2(1440.0, 900.0);
    let mut out = Vec::new();
    for n in 0..3 {
        ctx.begin_pass(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            time: Some(n as f64 * 0.02),
            ..Default::default()
        });
        let mut root = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        );
        show(&mut e, &mut view, &mut root);
        out.clear();
        for s in ctx.end_pass().shapes {
            texts(&s.shape, &mut out);
        }
    }
    out
}

fn count(t: &[String], s: &str) -> usize {
    t.iter().filter(|x| x.as_str() == s).count()
}

#[test]
fn port_labels_do_not_repeat_the_direction() {
    let ports: Vec<String> = {
        let t = face_texts(&["In", "Out", "Out"], &["Tone", "Edge"]);
        t.into_iter().collect()
    };
    for bad in ["IN In", "OUT Out"] {
        assert!(!ports.iter().any(|x| x == bad), "{bad} painted: {ports:?}");
    }
    assert!(ports.iter().any(|x| x == "In"), "{ports:?}");
}

#[test]
fn direction_words_in_labels_are_not_repeated() {
    let t = face_texts(&["Input", "in L", "Out R"], &["Tone", "Edge"]);
    for bad in ["IN Input", "IN in L", "OUT Out R"] {
        assert!(!t.iter().any(|x| x == bad), "{bad} painted: {t:?}");
    }
    let t = face_texts(&["Audio", "Audio", "Audio"], &["Tone", "Edge"]);
    assert!(t.iter().any(|x| x == "IN Audio") || t.iter().any(|x| x == "OUT Audio"));
}

#[test]
fn empty_port_label_is_the_bare_cue() {
    // The model rejects an empty label; a blank one is the nearest thing a file can carry.
    let t = face_texts(&[" ", "  ", "   "], &["Tone", "Edge"]);
    assert!(count(&t, "IN") >= 1 && count(&t, "OUT") >= 1, "{t:?}");
}

#[test]
fn each_control_is_named_once_by_its_exposure_label() {
    let t = face_texts(&["In", "Out", "Out"], &["Tone", "Edge"]);
    assert_eq!(count(&t, "Tone"), 1, "{t:?}");
    assert_eq!(count(&t, "Edge"), 1, "{t:?}");
    // The knob's own name would be a second label for the same control.
    for own in ["Cutoff", "Reso", "Resonance"] {
        assert_eq!(count(&t, own), 0, "{own} painted next to its label: {t:?}");
    }
}

#[test]
fn blank_control_label_falls_back_to_the_param_name_once() {
    let t = face_texts(&["In", "Out", "Out"], &[" ", "Edge"]);
    assert_eq!(count(&t, "Cutoff"), 1, "{t:?}");
}
