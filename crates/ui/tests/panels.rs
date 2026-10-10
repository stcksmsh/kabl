use kabl_core::{
    panel::{Artwork, Kind, Panel, Placement},
    PortRef,
};
use kabl_ui::{composites, panels, PatchEditor};
use std::{collections::BTreeSet, io::Cursor};
fn fixture() -> (PatchEditor, u64, u64) {
    let mut e = PatchEditor::seed_from(&kabl_standalone::default_patch());
    let leaf = *e
        .state()
        .modules
        .iter()
        .find(|(_, m)| m.kind == "filter.svf")
        .unwrap()
        .0;
    let members = e
        .state()
        .modules
        .iter()
        .filter(|(_, m)| m.kind != "out" && m.kind != "midi.in")
        .map(|(&id, _)| id)
        .collect();
    let id = composites::encapsulate(&mut e, members, BTreeSet::new(), "Voice").unwrap();
    composites::add_exposure(
        &mut e,
        id,
        PortRef::Param {
            id: leaf,
            param: "cutoff_hz".into(),
        },
        true,
    )
    .unwrap();
    let key = *e.state().composites[&id].controls.keys().next().unwrap();
    (e, id, key)
}
fn art() -> Artwork {
    let image = image::RgbaImage::from_pixel(32, 32, image::Rgba([120, 140, 110, 255]));
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    Artwork {
        name: "original.png".into(),
        png: bytes.into_inner(),
    }
}
fn panel(key: u64) -> Panel {
    let mut p = Panel {
        light: Some(art()),
        dark: Some(art()),
        ..Panel::default()
    };
    p.placements.insert(
        key,
        Placement {
            kind: Kind::Knob,
            x: 12.,
            y: 72.,
            width: 84.,
            height: 110.,
        },
    );
    p
}
#[test]
fn edits_preserve_ids_sound_history_and_source_free_packages() {
    let (mut e, id, key) = fixture();
    let before = e.state().clone();
    let mut c = before.composites[&id].clone();
    c.panel = Some(Box::new(panel(key)));
    composites::set(&mut e, id, c).unwrap();
    let installed = e.state().clone();
    assert!(kabl_engine::runtime::runtime_changes(&before, &installed)
        .unwrap()
        .is_empty());
    let target = installed.composites[&id].controls[&key].target.clone();
    let mut c = installed.composites[&id].clone();
    c.controls.get_mut(&key).unwrap().label = "Warmth".into();
    c.panel
        .as_mut()
        .unwrap()
        .placements
        .get_mut(&key)
        .unwrap()
        .x = 120.;
    composites::set(&mut e, id, c).unwrap();
    let moved = e.state().clone();
    assert_eq!(e.state().composites[&id].controls[&key].target, target);
    e.undo();
    assert_eq!(e.state(), &installed);
    e.redo();
    assert_eq!(e.state(), &moved);
    let dir = tempfile::tempdir().unwrap();
    kabl_core::save(dir.path(), e.log()).unwrap();
    let mut restored = PatchEditor::from_log(kabl_core::load(dir.path()).unwrap());
    assert_eq!(restored.state(), &moved);
    restored.undo();
    assert_eq!(restored.state(), &installed);
    let path = dir.path().join("voice.json");
    panels::export(&e, id, &path).unwrap();
    assert!(panels::export(&e, id, &path).is_err());
    let pkg = panels::read_package(&path).unwrap();
    let mut fresh = PatchEditor::seed_from(&Default::default());
    let copy = composites::insert(&mut fresh, &pkg).unwrap();
    assert_eq!(
        fresh.state().composites[&copy].panel,
        moved.composites[&id].panel
    );
    assert_eq!(
        panels::decode_art(
            fresh.state().composites[&copy]
                .panel
                .as_ref()
                .unwrap()
                .light
                .as_ref()
                .unwrap()
        )
        .unwrap()
        .width(),
        32
    );
}
#[test]
fn unsafe_assets_bindings_overlap_and_dimensions_fail_atomically() {
    let (mut e, id, key) = fixture();
    let before = e.state().clone();
    let count = e.log().entries().len();
    let counters = e.reserved_ids();
    for failure in 0..9 {
        let mut c = before.composites[&id].clone();
        let mut p = panel(key);
        match failure {
            0 => p.width = 301.,
            1 => p.light.as_mut().unwrap().name = "../escape.png".into(),
            2 => p.light.as_mut().unwrap().png.resize(128 * 1024 + 1, 0),
            3 => p.light.as_mut().unwrap().png[16..20].copy_from_slice(&9999u32.to_be_bytes()),
            4 => p.placements.get_mut(&key).unwrap().x = -1.,
            5 => p.placements.get_mut(&key).unwrap().y = 300.,
            6 => p.placements.get_mut(&key).unwrap().kind = Kind::Selector,
            7 => {
                let other = c.next_interface;
                c.next_interface += 1;
                c.controls.insert(other, c.controls[&key].clone());
                p.placements.insert(other, p.placements[&key].clone());
            }
            _ => {
                p.light.as_mut().unwrap().png.truncate(33);
            }
        }
        c.panel = Some(Box::new(p));
        assert!(composites::set(&mut e, id, c).is_err(), "case {failure}");
        assert_eq!(e.state(), &before);
        assert_eq!(e.log().entries().len(), count);
        assert_eq!(e.reserved_ids(), counters);
    }
    let pkg = composites::package(&before, id).unwrap();
    let mut json = serde_json::to_value(pkg).unwrap();
    json["patch"]["composites"][id.to_string()]["panel"] =
        serde_json::to_value(panel(key)).unwrap();
    json["patch"]["composites"][id.to_string()]["panel"]["light"]["name"] = "/absolute.png".into();
    assert!(composites::decode(&serde_json::to_vec(&json).unwrap()).is_err());
}
#[test]
fn old_composites_missing_art_and_unplaced_public_controls_remain_valid() {
    let (mut e, id, key) = fixture();
    let pkg = composites::package(e.state(), id).unwrap();
    let json = serde_json::to_vec(&pkg).unwrap();
    assert!(!String::from_utf8_lossy(&json).contains("\"panel\""));
    assert!(composites::decode(&json).unwrap().patch.composites[&id]
        .panel
        .is_none());
    let mut c = e.state().composites[&id].clone();
    c.panel = Some(Box::new(Panel::default()));
    composites::set(&mut e, id, c).unwrap();
    assert!(e.state().composites[&id].controls.contains_key(&key));
    let copy = composites::duplicate(&mut e, id).unwrap();
    assert_eq!(
        e.state().composites[&id].panel,
        e.state().composites[&copy].panel
    );
}

#[test]
fn panel_art_never_enters_audio_processing_or_changes_pcm() {
    let (mut e, id, key) = fixture();
    let before = e.state().clone();
    let mut c = before.composites[&id].clone();
    c.panel = Some(Box::new(panel(key)));
    composites::set(&mut e, id, c).unwrap();
    let mut a = kabl_engine::compile::compile(&before, 48000., 8).unwrap();
    let mut b = kabl_engine::compile::compile(e.state(), 48000., 8).unwrap();
    a.note_on(0, 0., 0.7);
    b.note_on(0, 0., 0.7);
    for _ in 0..500 {
        assert_no_alloc::assert_no_alloc(|| {
            a.process_block();
            b.process_block();
        });
        assert_eq!(a.left(), b.left());
        assert_eq!(a.right(), b.right());
    }
}

#[test]
fn aggregate_art_budget_is_checked_before_document_mutation() {
    let (e, id, _) = fixture();
    let mut p = e.state().clone();
    let image = image::RgbaImage::from_pixel(1024, 1024, image::Rgba([100, 120, 100, 255]));
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    let a = Artwork {
        name: "bounded.png".into(),
        png: bytes.into_inner(),
    };
    for fresh in 100..105 {
        let mut c = p.composites[&id].clone();
        c.members.clear();
        c.controls.clear();
        c.ports.clear();
        c.definition.id = format!("art-{fresh}");
        c.panel = Some(Box::new(Panel {
            light: Some(a.clone()),
            dark: Some(a.clone()),
            ..Panel::default()
        }));
        p.composites.insert(fresh, c);
    }
    assert!(kabl_core::composite::validate(&p)
        .unwrap_err()
        .contains("8 megapixels"));
}

#[test]
fn deleting_placed_leaf_prunes_metadata_and_undo_restores_complete_face() {
    let (mut e, id, key) = fixture();
    let leaf = e.state().composites[&id].controls[&key].target.module_id().unwrap();
    composites::add_exposure(
        &mut e,
        id,
        PortRef::Module {
            id: leaf,
            port: "in".into(),
        },
        false,
    )
    .unwrap();
    let jack = *e.state().composites[&id]
        .ports
        .iter()
        .find(|(_, x)| x.target.module_id() == Some(leaf))
        .unwrap()
        .0;
    let mut c = e.state().composites[&id].clone();
    c.panel = Some(Box::new(panel(key)));
    c.panel.as_mut().unwrap().placements.insert(
        jack,
        Placement {
            kind: Kind::Jack,
            x: 12.,
            y: 232.,
            width: 64.,
            height: 58.,
        },
    );
    composites::set(&mut e, id, c).unwrap();
    let before = e.state().clone();
    let leaf = before.composites[&id].controls[&key].target.module_id().unwrap();
    e.edit(vec![kabl_core::Op::RemoveModule { id: leaf }]);
    assert!(!e.state().composites[&id]
        .panel
        .as_ref()
        .unwrap()
        .placements
        .contains_key(&key));
    assert!(!e.state().composites[&id]
        .panel
        .as_ref()
        .unwrap()
        .placements
        .contains_key(&jack));
    composites::validate(e.state()).unwrap();
    let deleted = e.state().clone();
    let dir = tempfile::tempdir().unwrap();
    kabl_core::save(dir.path(), e.log()).unwrap();
    let mut restored = PatchEditor::from_log(kabl_core::load(dir.path()).unwrap());
    assert_eq!(restored.state(), &deleted);
    restored.undo();
    assert_eq!(restored.state(), &before);
    restored.redo();
    assert_eq!(restored.state(), &deleted);
}
