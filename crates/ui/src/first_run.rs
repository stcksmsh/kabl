//! First run (UI spec 7): the patch a new user starts on, and the guided tour over it.
//!
//! **The start patch.** It has to sound good untouched (a clock drives a sequencer, so it plays
//! by itself), have one obvious modulated knob (the filter's cutoff, opened by the envelope on
//! every note: the teal arc the tour points at) and show the living rack (sequencer lane, beads
//! on the cables, scopes). Three candidates are built here from one recipe; `START` picks the one
//! the app ships. `tools/examples/first_run_candidates.rs` renders each for the listening gate.

use kabl_core::{ModuleId, PortRef, Vec2};

use crate::PatchEditor;

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
    // the filter (where the tour's card goes). Bottom row: envelope, VCA, delay, reverb, output
    // and, past it, the clock that makes it play by itself.
    let seq = e.add_module("seq", at(24.0, 20.0));
    let osc = e.add_module("osc.va", at(714.0, 20.0));
    let svf = e.add_module("filter.svf", at(964.0, 20.0));
    let adsr = e.add_module("env.adsr", at(24.0, 320.0));
    let vca = e.add_module("vca", at(264.0, 320.0));
    let delay = e.add_module("delay", at(504.0, 320.0));
    let verb = e.add_module("reverb", at(744.0, 320.0));
    let out = e.add_module("out", at(984.0, 320.0));
    let clock = e.add_module("clock", at(1164.0, 320.0));
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
    e.set_param(svf, "cutoff_hz", s.cutoff_hz);
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
    e.set_param(delay, "mix", s.delay.2);
    e.set_param(verb, "decay_s", s.reverb.0);
    e.set_param(verb, "mix", s.reverb.1);
    e.set_param(out, "level_db", s.level_db);

    e.connect(m(clock, "gate"), m(seq, "clock"));
    e.connect(m(clock, "gate"), m(delay, "clock"));
    e.connect(m(seq, "pitch"), m(osc, "pitch"));
    e.connect(m(seq, "gate"), m(adsr, "gate"));
    e.connect(m(osc, "out"), m(svf, "in"));
    // The one obvious modulated knob: the envelope opens the cutoff on every note.
    let route = e.connect_route(m(adsr, "out"), svf, "cutoff_hz");
    e.set_route_amount(route, s.open, true);
    e.connect(m(svf, "lp"), m(vca, "in"));
    e.connect(m(adsr, "out"), m(vca, "cv"));
    e.connect(m(vca, "out"), m(delay, "in"));
    e.connect(m(delay, "out"), m(verb, "in"));
    e.connect(m(verb, "out"), m(out, "in"));
    e
}

/// The filter of a start patch (or any patch): where the tour's spotlight goes.
pub fn find_filter(state: &kabl_core::PatchState) -> Option<ModuleId> {
    state
        .modules
        .iter()
        .find(|(_, m)| m.kind.starts_with("filter."))
        .map(|(&id, _)| id)
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
