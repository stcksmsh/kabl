//! The start patch's macros: Brightness (filter cutoff and how far the envelope opens it) and
//! Space (delay and reverb mix), pinned to Perform. At rest they leave the sound exactly as it was
//! before they existed, and either end of Brightness still plays cleanly.
use kabl_core::{CableId, ModuleId, PatchState, PortRef};
use kabl_engine::golden::SampleHash;
use kabl_ui::first_run;
use kabl_ui::perform::PIN_PREFIX;

const SECS: f32 = 10.0;
const RATE: f32 = 48000.0;

/// FNV-1a of 10 s of the start patch at 48 kHz (`first_run::render`), recorded from it as it was
/// before the macros were added (origin/master 34b9f4a9). Any differing sample fails.
const BEFORE: &str = "15b6969b4a280f2c";

fn find(state: &PatchState, kind: &str) -> ModuleId {
    state
        .modules
        .iter()
        .find(|(_, m)| m.kind == kind)
        .map(|(&id, _)| id)
        .unwrap_or_else(|| panic!("no {kind}"))
}

fn from_macro(state: &PatchState, knob: ModuleId, out: &str, to: &PortRef) -> bool {
    state.cables.values().any(|c| {
        c.from
            == PortRef::Module {
                id: knob,
                port: out.into(),
            }
            && &c.to == to
    })
}

fn param(id: ModuleId, p: &str) -> PortRef {
    PortRef::Param {
        id,
        param: p.into(),
    }
}

fn hash(state: &PatchState) -> String {
    let mut h = SampleHash::default();
    h.feed(&first_run::render(state, SECS, RATE).unwrap());
    h.hex()
}

#[test]
fn the_start_patch_has_brightness_and_space() {
    let e = first_run::start_patch();
    let s = e.state();
    let knob = find(s, "macro");
    assert_eq!(s.label(knob, "name.m1"), Some("Brightness"));
    assert_eq!(s.label(knob, "name.m2"), Some("Space"));
    // Both are pinned to Perform.
    for m in ["m1", "m2"] {
        assert!(
            s.modules[&knob]
                .params
                .contains_key(&format!("{PIN_PREFIX}{m}")),
            "{m} is not pinned"
        );
    }
    let (svf, adsr) = (find(s, "filter.svf"), find(s, "env.adsr"));
    assert!(from_macro(s, knob, "m1", &param(svf, "cutoff_hz")));
    // Brightness also sets how far the envelope opens the cutoff.
    let env_route: CableId = s
        .cables
        .iter()
        .find(|(_, c)| {
            c.from
                == PortRef::Module {
                    id: adsr,
                    port: "out".into(),
                }
                && c.to == param(svf, "cutoff_hz")
        })
        .map(|(&id, _)| id)
        .expect("the envelope route into the cutoff");
    assert!(from_macro(
        s,
        knob,
        "m1",
        &PortRef::CableParam {
            cable: env_route,
            param: kabl_engine::compile::DEPTH.into(),
        }
    ));
    for kind in ["delay", "reverb"] {
        assert!(
            from_macro(s, knob, "m2", &param(find(s, kind), "mix")),
            "Space does not reach the {kind} mix"
        );
    }
}

#[test]
fn at_rest_the_start_patch_renders_exactly_as_before() {
    assert_eq!(first_run::start_name(), "Pluck line");
    assert_eq!(hash(first_run::start_patch().state()), BEFORE);
}

#[test]
fn brightness_at_either_end_plays_cleanly() {
    let mut e = first_run::start_patch();
    let knob = find(e.state(), "macro");
    for v in [0.0, 1.0] {
        e.set_param(knob, "m1", v);
        let a = first_run::render(e.state(), SECS, RATE).unwrap();
        assert!(a.iter().all(|s| s.is_finite()));
        let peak = a.iter().fold(0f32, |m, s| m.max(s.abs()));
        let rms = (a.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / a.len() as f64).sqrt();
        let db = 20.0 * rms.log10();
        eprintln!("Brightness {v}: peak {peak:.3}, rms {db:.1} dB");
        assert!(peak < 1.0, "Brightness {v} clips: peak {peak}");
        assert!(db > -40.0, "Brightness {v} is near silent: {db:.1} dB");
    }
}

#[test]
fn the_macros_rest_at_zero_and_only_the_named_two_are_on_the_face() {
    let e = first_run::start_patch();
    let s = e.state();
    let knob = find(s, "macro");
    let params = &s.modules[&knob].params;
    for m in ["m1", "m2"] {
        assert_eq!(params.get(m).copied().unwrap_or(0.0), 0.0, "{m} rests at 0");
    }
    let info = kabl_modules::registry::info_for("macro").unwrap();
    let on: Vec<&str> = kabl_ui::rack::primary_set(&s.modules[&knob], info)
        .iter()
        .zip(info.params)
        .filter(|(on, _)| **on)
        .map(|(_, p)| p.name)
        .collect();
    assert_eq!(on, ["m1", "m2"]);
}

#[test]
fn brightness_and_space_at_full_play_cleanly() {
    let mut e = first_run::start_patch();
    let knob = find(e.state(), "macro");
    for (name, keys) in [("Space", &["m2"][..]), ("Both", &["m1", "m2"][..])] {
        for k in ["m1", "m2"] {
            e.set_param(knob, k, if keys.contains(&k) { 1.0 } else { 0.0 });
        }
        let a = first_run::render(e.state(), SECS, RATE).unwrap();
        assert!(a.iter().all(|s| s.is_finite()));
        let peak = a.iter().fold(0f32, |m, s| m.max(s.abs()));
        let rms = (a.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / a.len() as f64).sqrt();
        let db = 20.0 * rms.log10();
        eprintln!("{name} at 1: peak {peak:.3}, rms {db:.1} dB");
        assert!(peak < 1.0, "{name} at 1 clips: peak {peak}");
        assert!(db > -40.0, "{name} at 1 is near silent: {db:.1} dB");
    }
}
