//! D03 listening recipes: exactly three short, optional experiments, each on one factory
//! patch, with explicit targets, one audible change per action, D02 Show/Back, optional
//! inspection and a final task without guidance. Rules: docs/signal-inspection/design.md §4.
//!
//! Recipe state is view only. Starting one opens its patch through the ordinary unsaved-changes
//! protection; the recipe becomes active only when that open completed. Closing keeps the
//! current patch and its edits. Opening another sound detaches the recipe (it never follows a
//! different patch by name or id).

use kabl_core::{ModuleId, PatchState, PortRef, Vec2};
use kabl_engine::patch_engine::{Command, MAX_PREVIEW_NOTES};

use crate::browser::{self, DocOrigin, Pending};
use crate::explain::{self, Target};
use crate::inspect::{self, Sel};
use crate::perform::PIN_PREFIX;
use crate::{PatchEditor, UiState};

/// What a step points at, by explicit identity (id and kind; a route by its endpoints).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tgt {
    Control {
        id: ModuleId,
        kind: &'static str,
        param: &'static str,
    },
    /// The modulation route from `from`'s `port` into `to`'s `param`.
    Route {
        from: ModuleId,
        port: &'static str,
        to: ModuleId,
        kind: &'static str,
        param: &'static str,
    },
    Module {
        id: ModuleId,
        kind: &'static str,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct Step {
    pub text: &'static str,
    pub target: Option<Tgt>,
    /// An output to inspect: (module, kind, port).
    pub inspect: Option<(ModuleId, &'static str, &'static str)>,
    /// Offer a test note of this length (seconds): the same note length every time.
    pub note_secs: Option<f32>,
    /// Offer the comparison section.
    pub compare: bool,
}

const fn step(text: &'static str) -> Step {
    Step {
        text,
        target: None,
        inspect: None,
        note_secs: None,
        compare: false,
    }
}

pub struct Recipe {
    pub key: &'static str,
    pub title: &'static str,
    pub intent: &'static str,
    /// Library id of its patch.
    pub sound: &'static str,
    pub steps: &'static [Step],
    pub unguided: &'static str,
}

const ENV: &str = "env.adsr";
const FILTER: &str = "filter.svf";

pub static RECIPES: [Recipe; 3] = [
    Recipe {
        key: "pluck-to-pad",
        title: "Pluck to pad",
        intent: "Turn a short pluck into a slow pad with one envelope, one control at a time.",
        sound: "factory:recipes/pluck-to-pad",
        steps: &[
            Step {
                inspect: Some((6, ENV, "out")),
                note_secs: Some(2.0),
                ..step(
                    "Play the 2-second test note: a short pluck. The inspector shows ADSR #6's \
                     output: it rises and falls back to 0 while the key is still held.",
                )
            },
            Step {
                target: Some(Tgt::Control {
                    id: 6,
                    kind: ENV,
                    param: "sustain",
                }),
                note_secs: Some(2.0),
                ..step(
                    "Raise Sustain to about 70 % and play again: the note now holds at that \
                     level for as long as the key is down.",
                )
            },
            Step {
                target: Some(Tgt::Control {
                    id: 6,
                    kind: ENV,
                    param: "release_ms",
                }),
                note_secs: Some(2.0),
                ..step(
                    "Raise Release to about 1 s: after the key goes up the note fades instead \
                     of stopping. Release only starts when the key is released.",
                )
            },
            Step {
                target: Some(Tgt::Control {
                    id: 6,
                    kind: ENV,
                    param: "attack_ms",
                }),
                note_secs: Some(2.0),
                ..step(
                    "Raise Attack to about 600 ms: the note swells in. Slow attack plus sustain \
                     plus release is a pad.",
                )
            },
            Step {
                compare: true,
                note_secs: Some(2.0),
                ..step(
                    "Compare: Restore the reference (the pluck you started from), play, then \
                     Back to my version (Undo) to keep your pad.",
                )
            },
        ],
        unguided: "On your own: make a bowed string from the same envelope — medium attack, \
                   full sustain, short release — then compare it with your pad.",
    },
    Recipe {
        key: "filter-movement",
        title: "Filter movement",
        intent: "Make the brightness move by itself with one LFO route into the filter cutoff.",
        sound: "factory:recipes/filter-movement",
        steps: &[
            Step {
                target: Some(Tgt::Control {
                    id: 3,
                    kind: FILTER,
                    param: "cutoff_hz",
                }),
                note_secs: Some(4.0),
                ..step(
                    "Play the 4-second test note and turn Cutoff down to about 300 Hz, then \
                     back up: darker, brighter.",
                )
            },
            Step {
                target: Some(Tgt::Route {
                    from: 7,
                    port: "out",
                    to: 3,
                    kind: FILTER,
                    param: "cutoff_hz",
                }),
                note_secs: Some(4.0),
                ..step(
                    "Show the route from LFO #7 into Cutoff and turn its Bypass off: the \
                     brightness now moves by itself. Source LFO #7, depth +35 %, destination \
                     Filter #3 Cutoff.",
                )
            },
            Step {
                target: Some(Tgt::Control {
                    id: 7,
                    kind: "lfo",
                    param: "rate_hz",
                }),
                inspect: Some((7, "lfo", "out")),
                note_secs: Some(4.0),
                ..step(
                    "Inspect LFO #7: it swings between −1 and 1. Set Rate to about 2 Hz: the \
                     movement gets faster.",
                )
            },
            Step {
                target: Some(Tgt::Route {
                    from: 7,
                    port: "out",
                    to: 3,
                    kind: FILTER,
                    param: "cutoff_hz",
                }),
                note_secs: Some(4.0),
                ..step(
                    "Drag the route's depth below 0: the same LFO now moves the cutoff the \
                     other way. Bypass the route to hear the filter without it.",
                )
            },
            Step {
                compare: true,
                note_secs: Some(4.0),
                ..step(
                    "Compare: Restore the reference (route bypassed, as you started), play, \
                     then Back to my version (Undo).",
                )
            },
        ],
        unguided: "On your own: add a second route from LFO #7 to Filter #3 Resonance with a \
                   small depth, and decide whether you like it better.",
    },
    Recipe {
        key: "interlocking",
        title: "Interlocking sequences",
        intent: "Hear how two sequences on one clock drift and lock as their lengths change.",
        sound: "factory:interlocking",
        steps: &[
            Step {
                target: Some(Tgt::Module {
                    id: 1,
                    kind: "clock",
                }),
                inspect: Some((1, "clock", "gate")),
                ..step(
                    "Press Start. Clock #1 drives Seq #3 (bass, 7 steps, every 16th) directly \
                     and Seq #4 (lead, 5 steps) through Divider #2. The inspector counts the \
                     clock's edges.",
                )
            },
            Step {
                target: Some(Tgt::Module {
                    id: 2,
                    kind: "clock.div",
                }),
                inspect: Some((2, "clock.div", "gate")),
                ..step(
                    "Inspect Divider #2's gate: half as many edges as the clock, so the lead \
                     steps on 8ths.",
                )
            },
            Step {
                target: Some(Tgt::Control {
                    id: 4,
                    kind: "seq",
                    param: "length",
                }),
                ..step(
                    "Set Seq #4 Length from 5 to 4: the lead now repeats every bar while the \
                     7-step bass still drifts against it.",
                )
            },
            Step {
                target: Some(Tgt::Control {
                    id: 3,
                    kind: "seq",
                    param: "length",
                }),
                inspect: Some((3, "seq", "gate")),
                ..step(
                    "Set Seq #3 Length from 7 to 8: now both patterns meet at the start of \
                     every bar. Keep whichever you like.",
                )
            },
            Step {
                compare: true,
                ..step(
                    "Compare: Restore the reference (7 against 5), listen, then Back to my \
                     version (Undo).",
                )
            },
        ],
        unguided: "On your own: set Divider #2 to divide by 3 and pick a lead length that \
                   interlocks with the bass in a way you like.",
    },
];

/// The active recipe.
#[derive(Debug, Clone, PartialEq)]
pub struct Active {
    pub recipe: usize,
    pub step: usize,
    /// The editor the recipe's patch was opened into.
    pub editor: u64,
    /// Another sound was opened since: the steps no longer apply.
    pub detached: bool,
}

/// Recipe panel state. View only.
#[derive(Debug, Default)]
pub struct Learn {
    pub open: bool,
    pub active: Option<Active>,
    /// A start waiting for its open (recipe, editor instance at the request).
    pending: Option<(usize, u64)>,
    pub note: Option<String>,
}

/// Is `t` in `state` with the kind the recipe expects? The resolved Show target when it is.
pub fn resolve(state: &PatchState, t: &Tgt) -> Option<Target> {
    let kind_ok = |id: ModuleId, kind: &str| state.modules.get(&id).is_some_and(|m| m.kind == kind);
    match *t {
        Tgt::Control { id, kind, param } => (kind_ok(id, kind)
            && explain::param_of(state, id, param).is_some())
        .then(|| Target::Control {
            id,
            param: param.to_string(),
            cable: None,
        }),
        Tgt::Route {
            from,
            port,
            to,
            kind,
            param,
        } => {
            if !kind_ok(to, kind) {
                return None;
            }
            let cable = state.cables.iter().find_map(|(&cid, c)| {
                let from_ok =
                    matches!(&c.from, PortRef::Module { id, port: p } if *id == from && p == port);
                let to_ok =
                    matches!(&c.to, PortRef::Param { id, param: p } if *id == to && p == param);
                (from_ok && to_ok).then_some(cid)
            })?;
            Some(Target::Control {
                id: to,
                param: param.to_string(),
                cable: Some(cable),
            })
        }
        Tgt::Module { id, kind } => kind_ok(id, kind).then_some(Target::Module(id)),
    }
}

fn target_name(state: &PatchState, t: &Tgt) -> String {
    match *t {
        Tgt::Control { id, param, .. } => format!("{} {param}", inspect::title(state, id)),
        Tgt::Route {
            from, to, param, ..
        } => format!("route #{from} → #{to} {param}"),
        Tgt::Module { id, .. } => format!("module #{id}"),
    }
}

/// Starts recipe `i`: asks the ordinary unsaved question before opening its patch.
pub fn start(editor: &mut PatchEditor, ui: &mut UiState, i: usize) {
    ui.recipes.pending = Some((i, editor.instance()));
    ui.recipes.note = None;
    log::debug!(target: "recipe", "start requested recipe={}", RECIPES[i].key);
    browser::request(editor, ui, Pending::Open(RECIPES[i].sound.to_string()));
}

/// Once per frame: a start whose open completed becomes active (and captures the reference);
/// one that was cancelled or failed is dropped; another open detaches an active recipe.
pub fn frame(editor: &PatchEditor, ui: &mut UiState) {
    if let Some((i, before)) = ui.recipes.pending {
        let opened = editor.instance() != before
            && matches!(&ui.doc, Some(d) if d.origin == DocOrigin::Library(RECIPES[i].sound.into()));
        if opened {
            ui.recipes.pending = None;
            ui.recipes.active = Some(Active {
                recipe: i,
                step: 0,
                editor: editor.instance(),
                detached: false,
            });
            ui.recipes.open = true;
            ui.compare.capture(editor, "the recipe");
            log::debug!(target: "recipe", "active recipe={}", RECIPES[i].key);
        } else if ui.browser.dialog.is_none() {
            ui.recipes.pending = None;
            ui.recipes.note = Some("The recipe did not start; your sound is unchanged.".into());
            log::debug!(target: "recipe", "start cancelled recipe={}", RECIPES[i].key);
        }
    }
    if let Some(a) = ui.recipes.active.as_mut() {
        if !a.detached && a.editor != editor.instance() {
            a.detached = true;
            log::debug!(target: "recipe", "detached recipe={}", RECIPES[a.recipe].key);
        }
    }
}

fn weak(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.add(egui::Label::new(egui::RichText::new(text.into()).small().weak()).wrap());
}

fn para(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.add(egui::Label::new(text.into()).wrap());
}

/// A test note of `secs` seconds on middle C, through the engine's preview (bounded there).
fn test_note(ui_state: &mut UiState, secs: f32) {
    let mut notes = [0u8; MAX_PREVIEW_NOTES];
    notes[0] = 60;
    ui_state.launches.push(Command::Preview {
        notes,
        count: 1,
        velocity: 100,
        blocks: (secs * ui_state.sample_rate / kabl_engine::graph::BLOCK as f32) as u32,
    });
}

/// The drawer section.
pub fn panel(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui, now: f64) {
    ui.horizontal(|ui| {
        ui.strong("Listening recipes");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let r = ui.small_button("Close");
            ui_state.record("learn-close".into(), r.rect);
            if r.clicked() {
                // Keeps the current patch and its edits.
                ui_state.recipes.open = false;
                ui_state.recipes.active = None;
            }
        });
    });
    if let Some(n) = ui_state.recipes.note.clone() {
        para(ui, n);
    }
    let Some(a) = ui_state.recipes.active.clone() else {
        weak(
            ui,
            "Optional. Each opens its own patch (you are asked first if your sound has unsaved \
             changes). Leave any time: your patch stays as it is.",
        );
        for (i, r) in RECIPES.iter().enumerate() {
            ui.horizontal_wrapped(|ui| {
                let b = ui.button(r.title).on_hover_text(r.intent);
                ui_state.record(format!("recipe:{}", r.key), b.rect);
                if b.clicked() {
                    start(editor, ui_state, i);
                }
                weak(ui, r.intent);
            });
        }
        return;
    };
    let r = &RECIPES[a.recipe];
    ui.label(egui::RichText::new(r.title).strong());
    if a.detached {
        para(
            ui,
            "Another sound was opened, so these steps no longer apply to the patch in front of \
             you. Your current sound is untouched.",
        );
        ui.horizontal(|ui| {
            let b = ui.button("Open the recipe patch again");
            ui_state.record("recipe-reopen".into(), b.rect);
            if b.clicked() {
                start(editor, ui_state, a.recipe);
            }
        });
        return;
    }
    weak(ui, r.intent);
    let n = r.steps.len();
    if a.step >= n {
        para(ui, r.unguided);
        weak(
            ui,
            "Close keeps your patch and every edit. Save it from Sounds if you like it.",
        );
    } else {
        let s = r.steps[a.step];
        ui.label(egui::RichText::new(format!("Step {} of {n}", a.step + 1)).small());
        para(ui, s.text);
        let state = editor.state();
        let resolved = s
            .target
            .as_ref()
            .map(|t| (resolve(state, t), target_name(state, t)));
        if let Some((None, name)) = &resolved {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                format!(
                    "{name} is not in the open patch (deleted, or changed). Undo can bring it \
                     back; nothing is guessed from names."
                ),
            );
        }
        ui.horizontal_wrapped(|ui| {
            if let Some((Some(t), _)) = resolved.clone() {
                let b = ui
                    .button("Show")
                    .on_hover_text("Bring it into view (no change to the sound)");
                ui_state.record("recipe-show".into(), b.rect);
                if b.clicked() {
                    explain::go(editor, ui_state, t, now);
                }
                if explain::can_go_back(&ui_state.explain) {
                    let b = ui.button("Back");
                    ui_state.record("recipe-back".into(), b.rect);
                    if b.clicked() {
                        explain::back(ui_state, now);
                    }
                }
            }
            if let Some((id, kind, port)) = s.inspect {
                let ok = editor
                    .state()
                    .modules
                    .get(&id)
                    .is_some_and(|m| m.kind == kind);
                let b = ui.add_enabled(ok, egui::Button::new(format!("Inspect #{id} {port}")));
                ui_state.record("recipe-inspect".into(), b.rect);
                if b.clicked() {
                    ui_state.selected_module = Some(id);
                    ui_state.inspect.select(
                        Sel {
                            id,
                            port: port.to_string(),
                        },
                        now,
                    );
                }
            }
            if let Some(secs) = s.note_secs {
                let b = ui
                    .button(format!("Play test note ({secs:.0} s)"))
                    .on_hover_text("Middle C, same length and velocity every time");
                ui_state.record("recipe-note".into(), b.rect);
                if b.clicked() {
                    test_note(ui_state, secs);
                }
            }
            if s.compare {
                let b = ui.button("Open Compare");
                ui_state.record("recipe-compare".into(), b.rect);
                if b.clicked() {
                    ui_state.compare.open = true;
                    ui_state.compare.reveal = true;
                }
            }
        });
    }
    ui.horizontal(|ui| {
        let b = ui.add_enabled(a.step > 0, egui::Button::new("◀ Previous"));
        ui_state.record("recipe-prev".into(), b.rect);
        if b.clicked() {
            step_to(ui_state, a.step - 1);
        }
        let b = ui.add_enabled(
            a.step < n,
            egui::Button::new(if a.step + 1 == n {
                "Try on your own ▶"
            } else {
                "Next ▶"
            }),
        );
        ui_state.record("recipe-next".into(), b.rect);
        if b.clicked() {
            step_to(ui_state, a.step + 1);
        }
    });
}

fn step_to(ui_state: &mut UiState, step: usize) {
    if let Some(a) = ui_state.recipes.active.as_mut() {
        a.step = step;
        log::debug!(target: "recipe", "step recipe={} step={step}", RECIPES[a.recipe].key);
    }
}

fn port(id: ModuleId, p: &str) -> PortRef {
    PortRef::Module { id, port: p.into() }
}

fn at(row: usize, x: f32) -> Vec2 {
    Vec2 {
        x,
        y: crate::rack::row_y(row),
    }
}

fn pin(e: &mut PatchEditor, pins: &[(ModuleId, &str, &str)]) {
    let changes: Vec<_> = pins
        .iter()
        .enumerate()
        .map(|(i, (id, key, _))| (*id, format!("{PIN_PREFIX}{key}"), Some(i as f32)))
        .collect();
    e.set_presentation(&changes);
    for (id, key, label) in pins {
        e.set_label(*id, &format!("{PIN_PREFIX}{key}"), Some(label.to_string()));
    }
}

/// A keyboard voice: MIDI In #1 → Osc #2 → Filter #3 → VCA #4 → Output #5, ADSR #6 on the VCA.
fn voice(cutoff: f32, res: f32, env: [f32; 4]) -> PatchEditor {
    let mut e = PatchEditor::new();
    let midi = e.add_module("midi.in", at(0, 24.0));
    let osc = e.add_module("osc.va", at(0, 240.0));
    let filter = e.add_module("filter.svf", at(0, 420.0));
    let vca = e.add_module("vca", at(0, 600.0));
    let out = e.add_module("out", at(0, 760.0));
    let adsr = e.add_module("env.adsr", at(1, 600.0));
    e.set_param(osc, "waveform", 2.0); // saw
    e.set_param(filter, "cutoff_hz", cutoff);
    e.set_param(filter, "resonance", res);
    for (k, v) in ["attack_ms", "decay_ms", "sustain", "release_ms"]
        .iter()
        .zip(env)
    {
        e.set_param(adsr, k, v);
    }
    e.set_param(vca, "gain", 0.0);
    e.connect(port(midi, "pitch"), port(osc, "pitch"));
    e.connect(port(osc, "out"), port(filter, "in"));
    e.connect(port(filter, "lp"), port(vca, "in"));
    e.connect(port(midi, "gate"), port(adsr, "gate"));
    e.connect(port(adsr, "out"), port(vca, "cv"));
    e.connect(port(vca, "out"), port(out, "left"));
    e.connect(port(vca, "out"), port(out, "right"));
    e
}

/// `patches/recipes/pluck-to-pad`: a plucked saw, envelope controls pinned.
pub fn pluck_to_pad() -> PatchEditor {
    let mut e = voice(2400.0, 0.2, [2.0, 220.0, 0.0, 120.0]);
    pin(
        &mut e,
        &[
            (6, "attack_ms", "Attack"),
            (6, "decay_ms", "Decay"),
            (6, "sustain", "Sustain"),
            (6, "release_ms", "Release"),
        ],
    );
    e
}

/// `patches/recipes/filter-movement`: a held saw through a resonant low-pass, with LFO #7
/// routed to Cutoff (+35 %) but bypassed.
pub fn filter_movement() -> PatchEditor {
    let mut e = voice(900.0, 0.45, [10.0, 300.0, 0.85, 300.0]);
    let lfo = e.add_module("lfo", at(1, 240.0));
    e.set_param(lfo, "rate_hz", 0.5);
    let route = e.connect_route(port(lfo, "out"), 3, "cutoff_hz");
    e.set_route_amount(route, 0.35, true);
    e.set_route_bypass(route, true);
    pin(
        &mut e,
        &[
            (3, "cutoff_hz", "Cutoff"),
            (3, "resonance", "Resonance"),
            (lfo, "rate_hz", "LFO rate"),
        ],
    );
    e
}
