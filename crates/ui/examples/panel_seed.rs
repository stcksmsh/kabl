//! Seeds D09 graph/examples plus a stable waveform exposure for real GUI authoring. Does not add panel metadata.
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args[1] == "--empty" {
        let e = kabl_ui::PatchEditor::seed_from(&Default::default());
        kabl_core::save(std::path::Path::new(&args[2]), e.log()).unwrap();
        return;
    }
    let bytes = std::fs::read(&args[1]).unwrap();
    let pkg = kabl_ui::composites::decode(&bytes).unwrap();
    let mut e = kabl_ui::PatchEditor::seed_from(&pkg.patch);
    if let Some((&leaf, _)) = pkg.patch.modules.iter().find(|(_, m)| m.kind == "osc.va") {
        kabl_ui::composites::add_exposure(
            &mut e,
            pkg.root,
            kabl_core::PortRef::Param {
                id: leaf,
                param: "waveform".into(),
            },
            true,
        )
        .unwrap();
    }
    kabl_core::save(std::path::Path::new(&args[2]), e.log()).unwrap();
}
