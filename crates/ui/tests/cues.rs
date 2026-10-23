//! Cues, MIDI button actions and macros in the editor, headless through `show()`: cue
//! definitions are undoable edits, launches are runtime commands, deleting a referenced module
//! leaves valid cues and undo restores the references, buttons fire once per press.

use egui::{Pos2, RawInput, Rect};
use kabl_core::{ModuleId, Vec2};
use kabl_engine::patch_engine::{Command, Timing};
use kabl_modules::builtins::Transport;
use kabl_ui::{cues, perform, show, PatchEditor, UiState};

const CLOCK: u64 = 1;
const BASS: u64 = 3;
const ARP: u64 = 4;

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    t: f64,
    cues: ModuleId,
}

impl H {
    fn new() -> Self {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/performance");
        let mut editor = PatchEditor::from_log(kabl_core::load(&dir).expect("patch"));
        let cues = editor.add_module("cues", Vec2 { x: 30.0, y: 10.0 });
        let mut h = H {
            ctx: egui::Context::default(),
            editor,
            ui: UiState::default(),
            t: 0.0,
            cues,
        };
        h.frame();
        h
    }

    fn frame(&mut self) {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1440.0, 900.0))),
            time: Some(self.t),
            ..Default::default()
        };
        self.t += 1.0 / 60.0;
        self.ctx.begin_pass(raw);
        let mut root = egui::Ui::new(
            self.ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new()
                .max_rect(Rect::from_min_size(Pos2::ZERO, egui::vec2(1440.0, 900.0))),
        );
        show(&mut self.editor, &mut self.ui, &mut root);
        // What the control thread does every few milliseconds.
        kabl_ui::perform::rearm(&mut self.ui, self.t);
        let _ = self.ctx.end_pass();
    }

    /// A CC through the control layer, then a frame.
    fn cc(&mut self, cc: u8, v: u8) {
        kabl_ui::perform::apply_cc(&mut self.editor, &mut self.ui, &[(0, cc, v)], self.t);
        self.frame();
    }

    fn add_cue(&mut self, name: &str, bass: Option<usize>, arp: Option<usize>) -> usize {
        let n = cues::add(&mut self.editor, self.cues, &|_| 0).unwrap();
        cues::rename(&mut self.editor, self.cues, n, name);
        cues::set_target(&mut self.editor, self.cues, n, BASS, bass);
        cues::set_target(&mut self.editor, self.cues, n, ARP, arp);
        n
    }
}

#[test]
fn cue_definitions_are_saved_undoable_edits_without_audio_rebuilds() {
    let mut h = H::new();
    h.editor.take_dirty();
    let before = h.editor.state().clone();
    let n = h.add_cue("Main", Some(1), Some(2));
    cues::set_timing(&mut h.editor, h.cues, n, Timing::NextStep);
    assert!(!h.editor.is_dirty(), "cue edits never rebuild audio");
    let c = &cues::cues(h.editor.state(), h.cues)[0];
    assert_eq!(c.name, "Main");
    assert_eq!(c.clock, Some(CLOCK), "the first clock, chosen explicitly");
    assert_eq!(c.timing, Timing::NextStep);
    assert_eq!(c.targets, vec![(BASS, 1), (ARP, 2)]);

    let dir = std::env::temp_dir().join(format!("kabl-cues-{}", std::process::id()));
    kabl_core::save(&dir, h.editor.log()).unwrap();
    let back = kabl_core::load(&dir).unwrap();
    assert_eq!(
        cues::cues(back.state(), h.cues),
        cues::cues(h.editor.state(), h.cues)
    );
    let _ = std::fs::remove_dir_all(&dir);

    for _ in 0..5 {
        h.editor.undo();
    }
    assert_eq!(*h.editor.state(), before);
}

#[test]
fn launching_a_cue_is_one_runtime_command_with_every_target() {
    let mut h = H::new();
    let n = h.add_cue("Break", Some(3), None);
    let entries = h.editor.log().entries().len();
    cues::launch(&mut h.ui.launches, h.editor.state(), h.cues, n).unwrap();
    match h.ui.launches.as_slice() {
        [Command::Launch(l)] => {
            assert_eq!((l.clock, l.timing, l.count), (CLOCK, Timing::NextBar, 1));
            assert_eq!(l.targets[0], (BASS, 3));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(h.editor.log().entries().len(), entries);
    // Undo never replays it: undoing edits sends nothing.
    h.ui.launches.clear();
    h.editor.undo();
    h.frame();
    assert!(h.ui.launches.is_empty());
}

#[test]
fn deleting_a_referenced_module_leaves_valid_cues_and_undo_restores_them() {
    let mut h = H::new();
    let n = h.add_cue("Main", Some(1), Some(2));
    h.editor.remove_module(ARP);
    let c = &cues::cues(h.editor.state(), h.cues)[0];
    assert_eq!(c.targets, vec![(BASS, 1)]);
    assert!(!h.editor.state().modules[&h.cues]
        .params
        .contains_key(&format!("cue{n}.seq.{ARP}")));
    h.editor.remove_module(CLOCK);
    let c = &cues::cues(h.editor.state(), h.cues)[0];
    assert_eq!(c.clock, None);
    assert!(cues::launch(&mut h.ui.launches, h.editor.state(), h.cues, n).is_err());
    h.frame(); // draws a cue with no clock without trouble
    h.editor.undo();
    h.editor.undo();
    let c = &cues::cues(h.editor.state(), h.cues)[0];
    assert_eq!(c.clock, Some(CLOCK));
    assert_eq!(c.targets, vec![(BASS, 1), (ARP, 2)]);
}

#[test]
fn a_midi_button_fires_once_per_press_and_never_takes_over() {
    let mut h = H::new();
    let n = h.add_cue("Intro", Some(0), Some(0));
    h.t += 1.0; // past the connect guard
    h.frame();
    h.ui.learn = Some((h.cues, format!("btn.cue.{n}")));
    h.cc(60, 127); // learn on the press
    h.cc(60, 0);
    assert_eq!(
        perform::button_mapping(h.editor.state(), h.cues, &format!("cue.{n}")),
        Some((0, 60))
    );
    assert!(h.ui.launches.is_empty(), "learning does not fire");
    h.cc(60, 127);
    assert_eq!(h.ui.launches.len(), 1);
    // Repeated highs and the release do nothing.
    h.cc(60, 127);
    h.cc(60, 100);
    h.cc(60, 0);
    assert_eq!(h.ui.launches.len(), 1);
    h.cc(60, 127);
    assert_eq!(h.ui.launches.len(), 2);
    // No soft takeover state, no param changed.
    assert!(h.ui.takeover.is_empty());
}

#[test]
fn a_reconnect_cannot_fire_an_action() {
    let mut h = H::new();
    h.editor
        .set_presentation(&[(CLOCK, "btn.restart".into(), Some(61.0))]);
    h.t += 1.0;
    h.frame();
    h.ui.button_rearm = true; // what main.rs sets on (re)connect
    h.frame();
    h.cc(61, 127); // a controller reporting a held button on connect
    assert!(h.ui.transport.is_empty());
    h.t += 1.0;
    h.cc(61, 0);
    h.cc(61, 127);
    assert_eq!(h.ui.transport, vec![(CLOCK, Transport::Restart)]);
}

#[test]
fn buttons_and_continuous_mappings_take_a_cc_from_each_other() {
    let mut h = H::new();
    // The performance patch maps CC 20 (ch 1) to a mixer level.
    let (id, param) = h
        .editor
        .state()
        .modules
        .iter()
        .find_map(|(&id, m)| {
            m.params
                .iter()
                .find(|(k, &v)| k.starts_with("cc.") && v == 20.0)
                .map(|(k, _)| (id, k[3..].to_string()))
        })
        .expect("CC 20 mapped");
    h.ui.learn = Some((BASS, "btn.bank.1".into()));
    h.cc(20, 127);
    assert_eq!(perform::mapping(h.editor.state(), id, &param), None);
    assert_eq!(
        perform::button_mapping(h.editor.state(), BASS, "bank.1"),
        Some((0, 20))
    );
    // And back: learning the level takes CC 20 from the button.
    h.ui.learn = Some((id, param.clone()));
    h.cc(20, 10);
    assert_eq!(
        perform::button_mapping(h.editor.state(), BASS, "bank.1"),
        None
    );
    assert_eq!(
        perform::mapping(h.editor.state(), id, &param),
        Some((0, 20))
    );
}

#[test]
fn macros_are_named_pinned_and_learnable_like_any_knob() {
    let mut h = H::new();
    let m = h.editor.add_module("macro", Vec2 { x: 30.0, y: 380.0 });
    h.editor.set_label(m, "name.m1", Some("Energy".into()));
    perform::toggle_pin(&mut h.editor, m, "m1");
    let pin = perform::pins(h.editor.state())
        .into_iter()
        .find(|p| p.id == m)
        .unwrap();
    assert_eq!(perform::pin_label(h.editor.state(), &pin), "Energy");
    assert_eq!(
        kabl_ui::routing::source_label(h.editor.state(), m, "m1"),
        format!("Energy (Macros #{m})")
    );
    h.ui.learn = Some((m, "m1".into()));
    h.cc(90, 64);
    assert_eq!(perform::mapping(h.editor.state(), m, "m1"), Some((0, 90)));
    h.frame();
}

#[test]
fn a_cue_shows_queued_only_when_all_its_banks_are_coming() {
    let cue = |targets: Vec<(ModuleId, usize)>| cues::Cue {
        n: 1,
        name: "x".into(),
        clock: Some(CLOCK),
        timing: Timing::NextBar,
        targets,
        macros: Vec::new(),
        morph_beats: 0.0,
    };
    // Bass queued B, arp queued B: Main (B, B) is queued, Return (B, C) is not.
    let now = |_: ModuleId| (0, Some(1));
    assert_eq!(
        cues::standing(&cue(vec![(BASS, 1), (ARP, 1)]), &now),
        (false, true)
    );
    assert_eq!(
        cues::standing(&cue(vec![(BASS, 1), (ARP, 2)]), &now),
        (false, false)
    );
    // Landed: Main active.
    let now = |_: ModuleId| (1, None);
    assert_eq!(
        cues::standing(&cue(vec![(BASS, 1), (ARP, 1)]), &now),
        (true, false)
    );
    // Bass already on B, only the arp queued: still queued, not active.
    let now = |s: ModuleId| if s == BASS { (1, None) } else { (0, Some(1)) };
    assert_eq!(
        cues::standing(&cue(vec![(BASS, 1), (ARP, 1)]), &now),
        (false, true)
    );
}

/// A macro module with a cue on the performance patch.
fn with_macro(h: &mut H) -> ModuleId {
    let m = h.editor.add_module("macro", Vec2 { x: 30.0, y: 380.0 });
    perform::toggle_pin(&mut h.editor, m, "m1");
    perform::toggle_pin(&mut h.editor, m, "m2");
    m
}

#[test]
fn cue_macro_targets_are_saved_undoable_edits_without_audio_rebuilds() {
    let mut h = H::new();
    let m = with_macro(&mut h);
    let n = h.add_cue("Verse", Some(1), None);
    h.editor.take_dirty();
    let before = h.editor.state().clone();
    cues::set_macro(&mut h.editor, h.cues, n, m, 0, Some(0.25));
    cues::set_macro(&mut h.editor, h.cues, n, m, 1, Some(0.9));
    cues::set_morph(&mut h.editor, h.cues, n, 8.0);
    assert!(!h.editor.is_dirty(), "cue edits never rebuild audio");
    let c = &cues::cues(h.editor.state(), h.cues)[0];
    assert_eq!(c.macros, vec![(m, 0, 0.25), (m, 1, 0.9)]);
    assert_eq!(c.morph_beats, 8.0);

    let dir = std::env::temp_dir().join(format!("kabl-cue-macros-{}", std::process::id()));
    kabl_core::save(&dir, h.editor.log()).unwrap();
    let back = kabl_core::load(&dir).unwrap();
    assert_eq!(
        cues::cues(back.state(), h.cues),
        cues::cues(h.editor.state(), h.cues)
    );
    let _ = std::fs::remove_dir_all(&dir);

    cues::set_macro(&mut h.editor, h.cues, n, m, 1, None);
    assert_eq!(
        cues::cues(h.editor.state(), h.cues)[0].macros,
        vec![(m, 0, 0.25)]
    );
    cues::set_morph(&mut h.editor, h.cues, n, 0.0);
    assert_eq!(cues::cues(h.editor.state(), h.cues)[0].morph_beats, 0.0);
    for _ in 0..5 {
        h.editor.undo();
    }
    assert_eq!(*h.editor.state(), before);
}

#[test]
fn store_current_takes_every_macro_as_it_is_now_live_or_at_rest() {
    let mut h = H::new();
    let m = with_macro(&mut h);
    let n = h.add_cue("Chorus", Some(2), None);
    let state = h.editor.state().clone();
    // m1 is being moved by a cue (live 0.6); the rest sit at their knobs.
    h.ui.macro_live.insert(
        (m, 0),
        (
            kabl_ui::MacroLive {
                value: 0.6,
                to: 1.0,
                progress: 0.5,
            },
            std::time::Instant::now(),
        ),
    );
    cues::store_macros(&mut h.editor, h.cues, n, &|id, k| {
        h.ui.macro_now(&state, id, k)
    });
    let c = &cues::cues(h.editor.state(), h.cues)[0];
    assert_eq!(c.macros.len(), 4);
    assert_eq!(c.macros[0], (m, 0, 0.6));
    assert!(c.macros[1..].iter().all(|t| t.2 == 0.0));
    h.frame(); // Perform draws a macro that is on its way without trouble
}

#[test]
fn a_launch_carries_the_macro_targets_and_the_morph_in_pulses() {
    let mut h = H::new();
    let m = with_macro(&mut h);
    let n = h.add_cue("Drop", None, None);
    // Nothing set at all: refused, as before.
    assert!(cues::launch(&mut h.ui.launches, h.editor.state(), h.cues, n).is_err());
    cues::set_macro(&mut h.editor, h.cues, n, m, 2, Some(0.4));
    cues::set_morph(&mut h.editor, h.cues, n, 2.0);
    cues::launch(&mut h.ui.launches, h.editor.state(), h.cues, n).unwrap();
    match h.ui.launches.as_slice() {
        [Command::Launch(l)] => {
            assert_eq!(l.count, 0, "a cue of macros only has no sequencer");
            assert_eq!(l.macro_count, 1);
            assert_eq!(
                (l.macros[0].id, l.macros[0].index, l.macros[0].value),
                (m, 2, 0.4)
            );
            assert_eq!(l.morph_pulses, 8.0);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn deleting_a_macro_module_drops_its_cue_targets_and_undo_restores_them() {
    let mut h = H::new();
    let m = with_macro(&mut h);
    let n = h.add_cue("Verse", Some(1), None);
    cues::set_macro(&mut h.editor, h.cues, n, m, 0, Some(0.3));
    h.editor.remove_module(m);
    assert!(cues::cues(h.editor.state(), h.cues)[0].macros.is_empty());
    assert!(!h.editor.state().modules[&h.cues]
        .params
        .keys()
        .any(|k| k.contains(".macro.")));
    h.editor.undo();
    assert_eq!(
        cues::cues(h.editor.state(), h.cues)[0].macros,
        vec![(m, 0, 0.3)]
    );
}

#[test]
fn a_cue_without_sequencers_is_playing_once_launched() {
    let cue = cues::Cue {
        n: 2,
        name: "x".into(),
        clock: Some(CLOCK),
        timing: Timing::NextBar,
        targets: Vec::new(),
        macros: vec![(9, 0, 0.5)],
        morph_beats: 0.0,
    };
    let now = |_: ModuleId| (0, None);
    assert_eq!(cues::standing_now(&cue, &now, None), (false, false));
    assert_eq!(cues::standing_now(&cue, &now, Some(1)), (false, false));
    assert_eq!(cues::standing_now(&cue, &now, Some(2)), (true, false));
}

#[test]
fn a_cue_offers_macros_five_to_eight_only_once_they_are_in_use() {
    let mut h = H::new();
    let m = with_macro(&mut h);
    assert_eq!(cues::active_macros(h.editor.state(), m), vec![0, 1, 2, 3]);
    perform::toggle_pin(&mut h.editor, m, "m6");
    h.editor.set_label(m, "name.m8", Some("Wobble".to_string()));
    assert_eq!(
        cues::active_macros(h.editor.state(), m),
        vec![0, 1, 2, 3, 5, 7]
    );
    let n = h.add_cue("Drop", None, None);
    cues::set_macro(&mut h.editor, h.cues, n, m, 7, Some(0.7));
    cues::store_macros(&mut h.editor, h.cues, n, &|_, _| 0.5);
    let c = &cues::cues(h.editor.state(), h.cues)[0];
    assert_eq!(c.macros.len(), 6);
    assert!(c.macros.contains(&(m, 7, 0.5)));
}
