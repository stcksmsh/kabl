//! Sequencer banks in the editor, through real egui input on `patches/performance`: edit vs
//! playing bank, inactive-bank edits, target identity, bank ops with undo, save/load, and
//! launches as runtime commands only.

use egui::{Event, PointerButton, Pos2, RawInput, Rect};
use kabl_core::{Op, ParamTarget, PortRef};
use kabl_engine::patch_engine::{Command, Timing};
use kabl_ui::{banks, rack, show, PatchEditor, UiState};

const CLOCK: u64 = 1;
const BASS: u64 = 3;

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    size: egui::Vec2,
    events: Vec<Event>,
    t: f64,
    pointer: Pos2,
}

impl H {
    fn new() -> Self {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/performance");
        let mut h = H {
            ctx: egui::Context::default(),
            editor: PatchEditor::from_log(kabl_core::load(&dir).expect("patch")),
            ui: UiState::default(),
            size: egui::vec2(1440.0, 900.0),
            events: Vec::new(),
            t: 0.0,
            pointer: Pos2::ZERO,
        };
        h.frame();
        h.frame();
        h
    }

    fn frame(&mut self) {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            events: std::mem::take(&mut self.events),
            time: Some(self.t),
            ..Default::default()
        };
        self.t += 1.0 / 60.0;
        self.ctx.begin_pass(raw);
        let mut root = egui::Ui::new(
            self.ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::ZERO, self.size)),
        );
        show(&mut self.editor, &mut self.ui, &mut root);
        let _ = self.ctx.end_pass();
    }

    /// Focuses the rack on `id` so its targets are on screen.
    fn focus(&mut self, id: u64) {
        self.ui.selected_module = Some(id);
        self.frame();
        self.click("zoom:focus");
    }

    fn click(&mut self, key: &str) {
        let p = self
            .ui
            .hits
            .get(key)
            .unwrap_or_else(|| panic!("no hit target {key}"))
            .center();
        self.pointer = p;
        self.events.push(Event::PointerMoved(p));
        self.frame();
        for pressed in [true, false] {
            self.events.push(Event::PointerButton {
                pos: self.pointer,
                button: PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            });
            self.frame();
        }
        self.frame();
    }

    fn param(&self, id: u64, p: &str) -> Option<f32> {
        self.editor.state().modules[&id].params.get(p).copied()
    }

    fn entries(&self) -> usize {
        self.editor.log().entries().len()
    }
}

#[test]
fn an_old_patch_is_bank_a_with_its_targets_unchanged() {
    let h = H::new();
    let m = &h.editor.state().modules[&BASS];
    // The stored pattern is bank A's params; banks B–D are absent (defaults).
    assert!(m.params.contains_key("p1"));
    assert!(!m.params.keys().any(|k| k.starts_with("b.")));
    assert_eq!(banks::startup(h.editor.state(), BASS), 0);
    // Pins, mappings and routes name what they named before.
    assert!(m.params.contains_key("pin.transpose"));
    let routed_from_bass = h.editor.state().cables.values().any(|c| {
        c.from
            == PortRef::Module {
                id: BASS,
                port: "velocity".into(),
            }
    });
    assert!(routed_from_bass);
    // The face shows bank A's controls, by their historic names.
    assert!(h.ui.hits.keys().any(|k| k == &format!("knob:{BASS}.p1")));
}

#[test]
fn the_edit_tab_changes_the_face_only() {
    let mut h = H::new();
    h.focus(BASS);
    let before = h.entries();
    h.click(&format!("bank-edit:{BASS}.2"));
    assert_eq!(h.ui.edit_bank_of(h.editor.state(), BASS), 2);
    assert!(h.ui.hits.contains_key(&format!("knob:{BASS}.c.p1")));
    assert!(!h.ui.hits.contains_key(&format!("knob:{BASS}.p1")));
    assert_eq!(h.entries(), before, "view choice, no op");
    assert!(h.ui.launches.is_empty(), "editing never launches");
    assert!(!h.editor.is_dirty());
}

#[test]
fn editing_an_inactive_bank_writes_only_that_banks_param() {
    let mut h = H::new();
    let a = h.param(BASS, "p1");
    h.editor.set_param(BASS, "b.p1", 12.0);
    assert_eq!(h.param(BASS, "p1"), a);
    assert_eq!(h.param(BASS, "b.p1"), Some(12.0));
}

#[test]
fn copy_and_clear_are_one_undo_step_each_and_restore_exactly() {
    let mut h = H::new();
    let original = h.editor.state().clone();
    banks::rename(&mut h.editor, BASS, 0, "Verse");
    banks::copy(&mut h.editor, BASS, 0, 1);
    let m = &h.editor.state().modules[&BASS];
    for slot in 0..kabl_modules::builtins::seq::SLOTS {
        use kabl_modules::builtins::seq::bank_param;
        assert_eq!(
            m.params.get(bank_param(1, slot)),
            m.params.get(bank_param(0, slot))
        );
    }
    assert_eq!(h.editor.state().label(BASS, "bank.b"), Some("Verse"));
    banks::clear(&mut h.editor, BASS, 1);
    assert_eq!(h.param(BASS, "b.g3"), Some(0.0));
    assert_eq!(h.param(BASS, "b.r3"), Some(100.0));
    assert!(h.editor.undo()); // clear
    assert!(h.editor.undo()); // copy
    assert!(h.editor.undo()); // rename
    assert_eq!(*h.editor.state(), original);
}

#[test]
fn inspecting_an_off_bank_target_reveals_it_without_launching() {
    let mut h = H::new();
    h.editor.connect_route(
        PortRef::Module {
            id: 2,
            port: "out".into(),
        },
        BASS,
        "c.p3",
    );
    h.frame();
    assert_eq!(h.ui.edit_bank_of(h.editor.state(), BASS), 0);
    h.ui.inspected = Some((BASS, "c.p3".into()));
    h.frame();
    h.frame();
    assert_eq!(h.ui.edit_bank_of(h.editor.state(), BASS), 2);
    assert!(h.ui.launches.is_empty());
    // Picking another tab afterwards is not forced back.
    h.focus(BASS);
    h.click(&format!("bank-edit:{BASS}.0"));
    assert_eq!(h.ui.edit_bank_of(h.editor.state(), BASS), 0);
    // The route still names bank C.
    let r = kabl_ui::routing::routes_into(h.editor.state(), BASS, "c.p3");
    assert_eq!(r.len(), 1);
}

#[test]
fn play_sends_a_runtime_launch_on_the_patched_clock() {
    let mut h = H::new();
    h.focus(BASS);
    let before = h.entries();
    h.click(&format!("bank-play:{BASS}.1"));
    match h.ui.launches.as_slice() {
        [Command::Launch(l)] => {
            assert_eq!((l.clock, l.timing, l.count), (CLOCK, Timing::NextBar, 1));
            assert_eq!(l.targets[0], (BASS, 1));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(h.entries(), before, "a launch is not an op");
}

#[test]
fn queued_state_shows_cancel_which_sends_a_cancel() {
    let mut h = H::new();
    h.focus(BASS);
    h.ui.seq_banks.insert(BASS, (0, Some(2)));
    h.frame();
    h.click(&format!("bank-cancel:{BASS}"));
    assert_eq!(h.ui.launches, vec![Command::Cancel(Some(BASS))]);
}

#[test]
fn face_choices_are_shared_by_every_bank() {
    let mut h = H::new();
    let info = kabl_modules::registry::info_for("seq").unwrap();
    let mut set = rack::primary_set(&h.editor.state().modules[&BASS], info);
    let i = info.params.iter().position(|p| p.name == "b.v1").unwrap();
    let j = info.params.iter().position(|p| p.name == "v1").unwrap();
    for (k, p) in info.params.iter().enumerate() {
        if rack::face_name(info, p.name) == "v1" {
            set[k] = true;
        }
    }
    assert!(set[i] && set[j]);
    h.editor.set_primary(BASS, &set);
    assert_eq!(h.param(BASS, "face.v1"), Some(1.0));
    assert_eq!(h.param(BASS, "face.b.v1"), None);
    let set = rack::primary_set(&h.editor.state().modules[&BASS], info);
    assert!(set[i] && set[j]);
}

#[test]
fn names_startup_and_launch_settings_save_and_load() {
    let mut h = H::new();
    banks::rename(&mut h.editor, BASS, 3, "Break");
    h.editor.set_param(BASS, "bank", 3.0);
    h.editor.set_presentation(&[
        (BASS, banks::LAUNCH_TIMING.into(), Some(1.0)),
        (BASS, banks::LAUNCH_CLOCK.into(), Some(CLOCK as f32)),
    ]);
    let dir = std::env::temp_dir().join(format!("kabl-banks-{}", std::process::id()));
    kabl_core::save(&dir, h.editor.log()).unwrap();
    let back = kabl_core::load(&dir).unwrap();
    let s = back.state();
    assert_eq!(banks::title(s, BASS, 3), "D Break");
    assert_eq!(banks::startup(s, BASS), 3);
    assert_eq!(banks::timing(s, BASS), Timing::NextStep);
    assert_eq!(banks::reference_clock(s, BASS), Some(CLOCK));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn launch_settings_never_rebuild_audio() {
    let mut h = H::new();
    h.editor.take_dirty();
    h.editor
        .set_presentation(&[(BASS, banks::LAUNCH_TIMING.into(), Some(0.0))]);
    banks::rename(&mut h.editor, BASS, 1, "X");
    assert!(!h.editor.is_dirty());
    // A bank param edit does rebuild (the engine then carries the playing bank).
    h.editor.edit(vec![Op::SetParam {
        target: ParamTarget::Module {
            id: BASS,
            param: "b.p2".into(),
        },
        value: 1.0,
    }]);
    assert!(h.editor.is_dirty());
}
