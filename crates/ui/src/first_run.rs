//! First run (UI spec 7): the patch a new user starts on, already playing in the rack, with
//! lesson 00 (`lessons/00-start-here.toml`, on the Start card) over it.
//!
//! **The start patch.** It has to sound good untouched (a clock drives a sequencer, so it plays
//! by itself), have one obvious modulated knob (the filter's cutoff, opened by the envelope on
//! every note: the teal arc lesson 00 points at) and show the living rack (sequencer lane, beads
//! on the cables, scopes). Three candidates are built here from one recipe; `START` picks the one
//! the app ships. `tools/examples/first_run_candidates.rs` renders each for the listening gate.

use kabl_core::{PortRef, Vec2};

use crate::browser::{Doc, DocOrigin};
use crate::{PatchEditor, UiState};

/// One candidate start patch.
pub struct Candidate {
    pub id: &'static str,
    pub name: &'static str,
    /// What it is, for the listener.
    pub blurb: &'static str,
    spec: Spec,
}

struct Spec {
    bpm: f32,
    /// `osc.va` waveform: 0 sine, 1 triangle, 2 saw, 3 square.
    wave: f32,
    base_hz: f32,
    /// Sequencer pitches (semitones from `base_hz`) and gates, steps 1 to 8.
    notes: [f32; 8],
    gates: [bool; 8],
    /// Filter: cutoff at rest (Hz), resonance, and how far the envelope opens it (route amount).
    cutoff_hz: f32,
    resonance: f32,
    open: f32,
    /// Envelope: attack, decay (ms), sustain (0..1), release (ms).
    adsr: [f32; 4],
    /// Delay: sync (1/16, 1/8, dotted 1/8, 1/4 as 1..4), feedback and mix (%).
    delay: (f32, f32, f32),
    /// Reverb: decay (s) and mix (%).
    reverb: (f32, f32),
    /// Output level (dB): the gain staging, no limiter. About -17 dB RMS, 3 dB or more of peak
    /// headroom.
    level_db: f32,
}

/// The candidate the app starts on (an index into [`candidates`]).
pub const START: usize = 0;

pub fn candidates() -> Vec<Candidate> {
    vec![
        Candidate {
            id: "pluck-line",
            name: "Pluck line",
            blurb: "A saw pluck over a minor pentatonic line, dotted echoes, a short room. \
                    The cutoff snaps open on every note.",
            spec: Spec {
                bpm: 112.0,
                wave: 2.0,
                base_hz: 130.81,
                notes: [0.0, 7.0, 3.0, 10.0, 12.0, 7.0, 15.0, 10.0],
                gates: [true, true, false, true, true, true, false, true],
                cutoff_hz: 380.0,
                resonance: 0.45,
                open: 0.55,
                adsr: [3.0, 180.0, 0.15, 120.0],
                delay: (3.0, 45.0, 28.0),
                reverb: (2.2, 22.0),
                level_db: -9.0,
            },
        },
        Candidate {
            id: "warm-pulse",
            name: "Warm pulse",
            blurb: "A square bass pulse, slow and round; the filter breathes with every hit.",
            spec: Spec {
                bpm: 96.0,
                wave: 3.0,
                base_hz: 65.41,
                notes: [0.0, 0.0, 12.0, 0.0, 7.0, 0.0, 10.0, 12.0],
                gates: [true, false, true, true, false, true, true, false],
                cutoff_hz: 260.0,
                resonance: 0.55,
                open: 0.7,
                adsr: [2.0, 260.0, 0.2, 140.0],
                delay: (2.0, 30.0, 18.0),
                reverb: (1.6, 15.0),
                level_db: -13.0,
            },
        },
        Candidate {
            id: "glass-arp",
            name: "Glass arp",
            blurb: "A bright triangle arpeggio over a major seventh, long echoes and a wide room.",
            spec: Spec {
                bpm: 124.0,
                wave: 1.0,
                base_hz: 261.63,
                notes: [0.0, 4.0, 7.0, 11.0, 12.0, 11.0, 7.0, 4.0],
                gates: [true; 8],
                cutoff_hz: 900.0,
                resonance: 0.25,
                open: 0.4,
                adsr: [2.0, 140.0, 0.1, 200.0],
                delay: (2.0, 55.0, 38.0),
                reverb: (4.5, 35.0),
                level_db: -12.0,
            },
        },
    ]
}

/// The patch the app starts on.
pub fn start_patch() -> PatchEditor {
    candidates().swap_remove(START).build()
}

/// The name the start patch's document carries (the header shows it, clean until edited).
pub fn start_name() -> &'static str {
    candidates().swap_remove(START).name
}

impl Candidate {
    pub fn build(&self) -> PatchEditor {
        build(&self.spec)
    }
}

fn build(s: &Spec) -> PatchEditor {
    let mut e = PatchEditor::seed_from(&Default::default());
    let at = |x: f32, y: f32| Vec2 { x, y };
    // The mockup's rack. Top row: sequencer, oscillator, filter, with free room to the right of
    // the filter and below it. Middle row: envelope, VCA, delay,
    // reverb, output. Bottom row: the clock that makes it play by
    // itself, and the macros under the effects they reach.
    let seq = e.add_module("seq", at(24.0, 20.0));
    let osc = e.add_module("osc.va", at(714.0, 20.0));
    let svf = e.add_module("filter.svf", at(964.0, 20.0));
    let adsr = e.add_module("env.adsr", at(24.0, 320.0));
    let vca = e.add_module("vca", at(264.0, 320.0));
    let delay = e.add_module("delay", at(474.0, 320.0));
    let verb = e.add_module("reverb", at(714.0, 320.0));
    let out = e.add_module("out", at(954.0, 320.0));
    let knob = e.add_module("macro", at(474.0, 620.0));
    let clock = e.add_module("clock", at(24.0, 620.0));
    let m = |id, port: &str| PortRef::Module {
        id,
        port: port.to_string(),
    };
    e.set_param(clock, "bpm", s.bpm);
    e.set_param(osc, "waveform", s.wave);
    e.set_param(osc, "base_hz", s.base_hz);
    for (k, (&n, &g)) in s.notes.iter().zip(&s.gates).enumerate() {
        e.set_param(seq, &format!("p{}", k + 1), n);
        e.set_param(seq, &format!("g{}", k + 1), if g { 1.0 } else { 0.0 });
    }
    e.set_param(svf, "resonance", s.resonance);
    for (name, v) in ["attack_ms", "decay_ms", "sustain", "release_ms"]
        .into_iter()
        .zip(s.adsr)
    {
        e.set_param(adsr, name, v);
    }
    e.set_param(vca, "gain", 0.0);
    e.set_param(delay, "sync", s.delay.0);
    e.set_param(delay, "feedback", s.delay.1);
    e.set_param(verb, "decay_s", s.reverb.0);
    e.set_param(out, "level_db", s.level_db);

    e.connect(m(clock, "gate"), m(seq, "clock"));
    e.connect(m(clock, "gate"), m(delay, "clock"));
    e.connect(m(seq, "pitch"), m(osc, "pitch"));
    e.connect(m(seq, "gate"), m(adsr, "gate"));
    e.connect(m(osc, "out"), m(svf, "in"));
    // Brightness (m1) and Space (m2), both resting at 0 with the rack knobs at the spec's
    // values: a macro only adds. At 0 a route adds exactly nothing, so the start patch plays bit
    // for bit as it did without the macros (`tests/first_run_macros.rs`).
    for (key, name) in [("m1", "Brightness"), ("m2", "Space")] {
        e.set_label(knob, &format!("name.{key}"), Some(name.into()));
    }
    let norm = |kind: &str, param: &str, v: f32| {
        kabl_modules::registry::info_for(kind)
            .and_then(|i| i.params.iter().find(|p| p.name == param))
            .expect("a built-in param")
            .to_norm(v)
    };
    // Brightness raises the cutoff to 2.5 times the spec's.
    e.set_param(svf, "cutoff_hz", s.cutoff_hz);
    let r = e.connect_route(m(knob, "m1"), svf, "cutoff_hz");
    let up = norm("filter.svf", "cutoff_hz", s.cutoff_hz * 2.5)
        - norm("filter.svf", "cutoff_hz", s.cutoff_hz);
    e.set_route_amount(r, up, true);
    // Space raises both mixes to twice the spec's.
    for (id, kind, mix) in [(delay, "delay", s.delay.2), (verb, "reverb", s.reverb.1)] {
        e.set_param(id, "mix", mix);
        let r = e.connect_route(m(knob, "m2"), id, "mix");
        e.set_route_amount(
            r,
            norm(kind, "mix", (2.0 * mix).min(100.0)) - norm(kind, "mix", mix),
            true,
        );
    }
    // The one obvious modulated knob: the envelope opens the cutoff on every note, and
    // Brightness deepens that (up to 1.4 times the spec's depth).
    let route = e.connect_route(m(adsr, "out"), svf, "cutoff_hz");
    e.set_route_amount(route, s.open, true);
    let depth = e
        .connect_cable_route(m(knob, "m1"), route, kabl_engine::compile::DEPTH)
        .expect("a route has a depth");
    e.set_route_amount(depth, 0.4 * s.open, true);
    e.connect(m(svf, "lp"), m(vca, "in"));
    e.connect(m(adsr, "out"), m(vca, "cv"));
    e.connect(m(vca, "out"), m(delay, "in"));
    e.connect(m(delay, "out"), m(verb, "in"));
    e.connect(m(verb, "out"), m(out, "in"));
    // Perform, as the factory presets pin it: the transport, then the macros on CC 20 and 21.
    // Only the two named macros sit on the module's face.
    use crate::perform::{CC_PREFIX, PIN_PREFIX, TRANSPORT};
    let off = (3..=kabl_modules::builtins::MACROS)
        .map(|k| (knob, crate::rack::face_key(&format!("m{k}")), Some(0.0)));
    e.set_presentation(
        &off.chain([
            (clock, format!("{PIN_PREFIX}{TRANSPORT}"), Some(0.0)),
            (knob, format!("{PIN_PREFIX}m1"), Some(1.0)),
            (knob, format!("{CC_PREFIX}m1"), Some(20.0)),
            (knob, format!("{PIN_PREFIX}m2"), Some(2.0)),
            (knob, format!("{CC_PREFIX}m2"), Some(21.0)),
        ])
        .collect::<Vec<_>>(),
    );
    e
}

/// The standalone app's first document when no patch is named: the start patch, playing. On a
/// fresh profile it opens in the Rack with lesson 00 on the Start card; after that, with Sounds
/// open over it. The plugin never calls this: it starts silent on its own patch.
pub fn open(ui: &mut UiState) -> PatchEditor {
    let start = start_patch();
    // Its own name in the header, and clean: nothing edited yet.
    ui.doc = Some(Doc::new(start_name(), DocOrigin::New, start.state()));
    ui.browser_open = !crate::lessons::first_run(&start, ui);
    start
}

/// A plugin's first run: lesson 00 on the Start card over the plugin's own patch, once per
/// profile, shared with the standalone app and every other instance. It neither replaces the
/// patch nor plays anything: the host's transport and MIDI decide when it sounds. True when it
/// started.
pub fn plugin(editor: &PatchEditor, ui: &mut UiState) -> bool {
    // Another instance, or the standalone app, may have had the first run since this one read
    // `library.json`.
    if let Some(lib) = ui.library.as_mut() {
        let on_disk = std::fs::read_to_string(lib.user_root.join(crate::library::PREFS_FILE))
            .ok()
            .and_then(|t| serde_json::from_str::<crate::library::Prefs>(&t).ok());
        if on_disk.is_some_and(|p| p.first_run_done) {
            lib.prefs.first_run_done = true;
        }
    }
    crate::lessons::first_run(editor, ui)
}

/// Renders `secs` of `state` offline at `rate`, stereo interleaved: what the patch plays on its
/// own from a fresh start (the clock restarts). Used by the listening-gate renders and tests.
pub fn render(state: &kabl_core::PatchState, secs: f32, rate: f32) -> Result<Vec<f32>, String> {
    use kabl_engine::graph::BLOCK;
    use kabl_engine::patch_engine::PatchEngine;
    use kabl_modules::builtins::Transport;
    let collector = basedrop::Collector::new();
    let mut engine = PatchEngine::new(&collector.handle(), state, rate, 8)
        .map_err(|e| format!("does not compile: {e}"))?;
    for (&id, m) in &state.modules {
        if m.kind == "clock" {
            engine.transport(id, Transport::Restart);
        }
    }
    let frames = (secs * rate) as usize / BLOCK * BLOCK;
    let mut out = Vec::with_capacity(frames * 2);
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    for _ in 0..frames / BLOCK {
        engine.process_block(&mut l, &mut r);
        for i in 0..BLOCK {
            out.push(l[i]);
            out.push(r[i]);
        }
    }
    Ok(out)
}

/// The audio output and MIDI input the standalone app remembers across launches (`[devices]`
/// in `settings.toml`, `style::Prefs`), chosen in lesson 00's Sound-check strip or the status
/// bar. A plugin uses the host's audio and MIDI and leaves them alone.
pub mod devices {
    use serde::{Deserialize, Serialize};

    use crate::UiState;

    /// One remembered choice.
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum Choice {
        /// Never chosen (or "System default" for the output): the default opens.
        #[default]
        Unset,
        /// "No MIDI input", chosen on purpose: nothing opens.
        None,
        Named(String),
    }

    /// The remembered output device and MIDI input.
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Devices {
        pub output: Choice,
        pub midi: Choice,
    }

    /// What a launch opens for a remembered choice among the names available now.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Pick {
        /// The default; `missing` when a remembered name was not among them.
        Default {
            missing: bool,
        },
        /// `available[i]`.
        Listed(usize),
        None,
    }

    /// The remembered name when it is available (exactly, else by [`base_name`], since ALSA
    /// renumbers a replugged MIDI device), the default when it is not or nothing was chosen,
    /// and nothing for an explicit none.
    pub fn pick(remembered: &Choice, available: &[String]) -> Pick {
        match remembered {
            Choice::Unset => Pick::Default { missing: false },
            Choice::None => Pick::None,
            Choice::Named(n) => available
                .iter()
                .position(|a| a == n)
                .or_else(|| available.iter().position(|a| base_name(a) == base_name(n)))
                .map_or(Pick::Default { missing: true }, Pick::Listed),
        }
    }

    /// A MIDI port name without ALSA's trailing `client:port` numbers.
    pub fn base_name(name: &str) -> &str {
        match name.rsplit_once(' ') {
            Some((head, tail))
                if tail.split(':').count() == 2
                    && tail
                        .split(':')
                        .all(|p| p.chars().all(|c| c.is_ascii_digit())) =>
            {
                head
            }
            _ => name,
        }
    }

    /// The standalone app takes over the remembered devices (after `use_user_prefs`) and gets
    /// them, to open at launch.
    pub fn take_over(ui: &mut UiState) -> Devices {
        let d = ui
            .prefs_saved
            .as_ref()
            .map(|p| p.devices.clone())
            .unwrap_or_default();
        ui.devices = Some(d.clone());
        d
    }

    /// The user picked an output (`None`: the system default): remembered for the next launch.
    /// Nothing happens in a plugin.
    pub fn remember_output(ui: &mut UiState, name: Option<String>) {
        remember(ui, |d| d.output = name.map_or(Choice::Unset, Choice::Named));
    }

    /// The user picked a MIDI input; `None` is "No MIDI input", remembered as such.
    pub fn remember_midi(ui: &mut UiState, port: Option<String>) {
        remember(ui, |d| d.midi = port.map_or(Choice::None, Choice::Named));
    }

    fn remember(ui: &mut UiState, f: impl FnOnce(&mut Devices)) {
        if let Some(d) = ui.devices.as_mut() {
            f(d);
            ui.save_prefs();
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn pick_opens_the_remembered_one_else_the_default_and_none_stays_none() {
            let list = ["Speakers".to_string(), "Keystep 28:0".to_string()];
            let named = |n: &str| Choice::Named(n.into());
            assert_eq!(pick(&named("Speakers"), &list), Pick::Listed(0));
            // Replugged: the same device under new ALSA numbers.
            assert_eq!(pick(&named("Keystep 24:0"), &list), Pick::Listed(1));
            assert_eq!(
                pick(&named("Headphones"), &list),
                Pick::Default { missing: true }
            );
            assert_eq!(
                pick(&Choice::Unset, &list),
                Pick::Default { missing: false }
            );
            assert_eq!(pick(&Choice::None, &list), Pick::None);
            assert_eq!(
                pick(&named("Speakers"), &[]),
                Pick::Default { missing: true }
            );
        }
    }
}
