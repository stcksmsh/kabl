//! The design mockup's patch, shared by the capture examples (`#[path = "support/mockup.rs"]`).
use kabl_ui::PatchEditor;

/// The mockup's patch: sequencer, VA (or, with `wt`, wavetable) oscillator, SVF, ADSR, mixer,
/// delay, reverb, output.
pub fn mockup_patch(wt: bool) -> PatchEditor {
    use kabl_core::PortRef;
    let mut e = PatchEditor::seed_from(&Default::default());
    let at = |x: f32, y: f32| kabl_core::Vec2 { x, y };
    let seq = e.add_module("seq", at(24.0, 20.0));
    let osc = e.add_module(if wt { "osc.wt" } else { "osc.va" }, at(714.0, 20.0));
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
    assert_eq!(e.state().cables.len(), 8, "mockup patch cables");
    e
}
