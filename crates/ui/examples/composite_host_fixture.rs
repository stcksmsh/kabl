//! Wrap an existing complete host sound without changing leaf, route or native identities.
use std::{collections::BTreeSet, path::Path};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let patch: kabl_core::PatchState = serde_json::from_value(value["patch"].clone()).unwrap();
    let mut e = kabl_ui::PatchEditor::seed_from(&patch);
    let members = patch
        .modules
        .iter()
        .filter(|(_, m)| m.kind != "out")
        .map(|(&id, _)| id)
        .collect();
    let id = kabl_ui::composites::encapsulate(
        &mut e,
        members,
        BTreeSet::new(),
        "Embedded arrangement voice",
    )
    .unwrap();
    let members = kabl_core::composite::leaves(e.state(), id);
    for lane in value["lanes"].as_array().unwrap() {
        if let Some(target) = lane.get("target").filter(|t| !t.is_null()) {
            let leaf = target["module"].as_u64().unwrap();
            if members.contains(&leaf) {
                kabl_ui::composites::add_exposure(
                    &mut e,
                    id,
                    kabl_core::PortRef::Param {
                        id: leaf,
                        param: target["param"].as_str().unwrap().into(),
                    },
                    true,
                )
                .unwrap();
            }
        }
    }
    assert_eq!(e.state().modules, patch.modules);
    assert_eq!(e.state().cables, patch.cables);
    value["patch"] = serde_json::to_value(e.state()).unwrap();
    value["version"] = 3.into();
    std::fs::write(
        Path::new(&args[2]),
        serde_json::to_vec_pretty(&value).unwrap(),
    )
    .unwrap();
}
