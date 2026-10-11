//! The wavetable editor through real egui input: open it from the `osc.wt` face, draw, edit
//! harmonics, apply frame operations, import files (good and broken), and undo, redo, save and
//! reload what it wrote. The result is checked on the real `PatchEditor` state.

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect};
use kabl_core::{ParamTarget, Vec2};
use kabl_modules::wavetable::{self, FRAME};
use kabl_ui::{show, PatchEditor, UiState};

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    size: egui::Vec2,
    events: Vec<Event>,
    modifiers: Modifiers,
    t: f64,
    pointer: Pos2,
    wt: u64,
}

impl H {
    fn new() -> Self {
        let mut editor = PatchEditor::new();
        let wt = editor.add_module("osc.wt", Vec2 { x: 24.0, y: 20.0 });
        let mut h = H {
            ctx: egui::Context::default(),
            editor,
            ui: UiState::default(),
            size: egui::vec2(1440.0, 900.0),
            events: Vec::new(),
            modifiers: Modifiers::NONE,
            t: 0.0,
            pointer: Pos2::ZERO,
            wt,
        };
        h.frame();
        h.frame();
        h
    }

    fn frame(&mut self) {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            events: std::mem::take(&mut self.events),
            modifiers: self.modifiers,
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

    fn settle(&mut self) {
        for _ in 0..4 {
            self.frame();
        }
    }

    fn at(&self, key: &str) -> Pos2 {
        self.ui
            .hits
            .get(key)
            .unwrap_or_else(|| panic!("no hit target {key}"))
            .center()
    }

    fn rect(&self, key: &str) -> Rect {
        *self
            .ui
            .hits
            .get(key)
            .unwrap_or_else(|| panic!("no hit target {key}"))
    }

    fn move_to(&mut self, p: Pos2) {
        self.pointer = p;
        self.events.push(Event::PointerMoved(p));
        self.frame();
    }

    fn button(&mut self, pressed: bool) {
        self.events.push(Event::PointerButton {
            pos: self.pointer,
            button: PointerButton::Primary,
            pressed,
            modifiers: self.modifiers,
        });
        self.frame();
    }

    fn click(&mut self, key: &str) {
        let p = self.at(key);
        self.move_to(p);
        self.button(true);
        self.button(false);
        self.settle();
    }

    fn shift_click(&mut self, p: Pos2) {
        self.modifiers = Modifiers::SHIFT;
        self.move_to(p);
        self.button(true);
        self.button(false);
        self.modifiers = Modifiers::NONE;
        self.settle();
    }

    /// Presses at `from`, moves in steps to `to` over frames spaced 0.2 s apart (so the write
    /// throttle fires), releases.
    fn drag(&mut self, from: Pos2, to: Pos2) {
        self.move_to(from);
        self.button(true);
        for i in 1..=8 {
            self.t += 0.2;
            self.move_to(from + (to - from) * (i as f32 / 8.0));
        }
        self.button(false);
        self.settle();
    }

    fn open_editor(&mut self) {
        self.click(&format!("edit-table:{}", self.wt));
        assert!(self.ui.hits.contains_key("wt:wave"), "editor did not open");
    }

    fn user(&self) -> f32 {
        self.editor
            .state()
            .param(&ParamTarget::Module {
                id: self.wt,
                param: "user".into(),
            })
            .unwrap_or(0.0)
    }

    fn table_frames(&self, slot: u64) -> Vec<Vec<f32>> {
        let t = &self.editor.state().tables[&slot];
        wavetable::decode_canonical(&t.wav).unwrap()
    }

    fn type_path(&mut self, path: &str) {
        self.click("wt:path");
        self.events.push(Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        });
        self.frame();
        self.events.push(Event::Text(path.into()));
        self.settle();
    }
}

fn wav(samples: &[f32]) -> Vec<u8> {
    let mut b = b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x44\xac\0\0\x88\x58\x01\0\x02\0\x10\0data".to_vec();
    b.extend((samples.len() as u32 * 2).to_le_bytes());
    for &v in samples {
        b.extend(((v * 32767.0) as i16).to_le_bytes());
    }
    b
}

fn sines(frames: usize) -> Vec<f32> {
    (0..frames * FRAME)
        .map(|i| {
            let (f, s) = (i / FRAME, i % FRAME);
            (std::f32::consts::TAU * (f + 1) as f32 * s as f32 / FRAME as f32).sin() * 0.5
        })
        .collect()
}

#[test]
fn opening_edits_a_copy_of_the_factory_table_and_writes_nothing_until_an_edit() {
    let mut h = H::new();
    h.open_editor();
    assert!(h.editor.state().tables.is_empty());
    assert_eq!(h.user(), 0.0);
    assert_eq!(h.ui.tables.frames(), wavetable::factory_frames(0).len());
}

#[test]
fn a_pen_stroke_is_one_undo_step_that_writes_the_table_and_binds_the_module() {
    let mut h = H::new();
    h.open_editor();
    let before = h.editor.state().clone();
    let wave = h.rect("wt:wave");
    h.drag(
        wave.left_top() + egui::vec2(40.0, 30.0),
        wave.left_top() + egui::vec2(300.0, 200.0),
    );
    assert_eq!(h.user(), 1.0);
    let drawn = h.table_frames(1);
    assert_eq!(drawn.len(), wavetable::factory_frames(0).len());
    assert_ne!(drawn[0], wavetable::factory_frames(0)[0]);
    // The whole stroke, table and binding, is one entry.
    assert!(h.editor.undo());
    assert_eq!(h.editor.state().tables, before.tables);
    assert_eq!(h.user(), 0.0);
    h.settle();
    assert_eq!(h.ui.tables.frames(), wavetable::factory_frames(0).len());
    assert!(h.editor.redo());
    assert_eq!(h.user(), 1.0);
    assert_eq!(h.table_frames(1), drawn);
}

#[test]
fn frame_operations_each_undo_in_one_step() {
    let mut h = H::new();
    h.open_editor();
    let n = h.ui.tables.frames();
    h.click("wt:insert");
    assert_eq!(h.table_frames(1).len(), n + 1);
    h.click("wt:dup");
    assert_eq!(h.table_frames(1).len(), n + 2);
    h.click("wt:delete");
    assert_eq!(h.table_frames(1).len(), n + 1);
    h.click("wt:left");
    h.click("wt:right");
    h.click("wt:normalise");
    h.click("wt:dc");
    h.click("wt:fade");
    assert!(h.editor.undo()); // fade
    assert!(h.editor.undo()); // dc
    assert!(h.editor.undo()); // normalise
    assert!(h.editor.undo()); // right
    assert!(h.editor.undo()); // left
    assert!(h.editor.undo()); // delete
    assert_eq!(h.table_frames(1).len(), n + 2);
    h.settle();
    assert_eq!(h.ui.tables.frames(), n + 2);
}

#[test]
fn harmonic_bars_set_a_harmonic() {
    let mut h = H::new();
    h.open_editor();
    h.click("wt:insert"); // a sine frame, selected
    let n_before = h.table_frames(1).len();
    // Switch to the harmonic bars, press on the 4th bar near the top.
    h.click("wt:tab-bars");
    let bars = h.rect("wt:bars");
    let w = bars.width() / 64.0;
    let at = Pos2::new(bars.left() + w * 3.5, bars.top() + bars.height() * 0.3);
    h.drag(at, at + egui::vec2(0.0, 1.0));
    let frames = h.table_frames(1);
    assert_eq!(frames.len(), n_before);
    let sel = h.ui.tables.selected();
    let amps = kabl_modules::tableedit::harmonics(&frames[sel]);
    // y = 0.7 of the height gives sqrt-scale 0.7, amplitude about 0.49; the fit may scale it.
    assert!(amps[3] > 0.2, "{:?}", &amps[..6]);
}

#[test]
fn morph_between_two_frames_inserts_the_steps_between() {
    let mut h = H::new();
    h.open_editor();
    let n = h.ui.tables.frames();
    assert!(n >= 3);
    let stack = h.rect("wt:stack");
    // Select frame 0 by clicking low in the stack, then mark a frame above it.
    h.move_to(Pos2::new(stack.center().x, stack.bottom() - 4.0));
    h.button(true);
    h.button(false);
    h.settle();
    h.shift_click(Pos2::new(stack.center().x, stack.top() + 4.0));
    h.click("wt:morph");
    // Frames between the two marked ones were replaced or added; the table still decodes, and
    // is not larger than the format allows.
    let after = h.table_frames(1).len();
    assert!(after >= n && after <= wavetable::MAX_FRAMES);
    assert!(h.editor.undo());
    assert!(h.editor.state().tables.is_empty());
}

#[test]
fn importing_a_file_shows_what_it_found_and_a_chosen_size_changes_it() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("sweep.wav");
    // 8 frames of 2048 with no clm chunk: read as 8 frames, but ambiguous.
    std::fs::write(&file, wav(&sines(8))).unwrap();
    let mut h = H::new();
    h.open_editor();
    h.type_path(file.to_str().unwrap());
    h.click("wt:import");
    assert_eq!(h.ui.tables.pending_frames(), Some(8));
    // Read as one long cycle instead, then back to detection.
    h.click("wt:size-whole");
    assert_eq!(h.ui.tables.pending_frames(), Some(1));
    h.click("wt:size-1024");
    assert_eq!(h.ui.tables.pending_frames(), Some(16));
    h.click("wt:size-auto");
    assert_eq!(h.ui.tables.pending_frames(), Some(8));
    // Nothing is written until "Use as table".
    assert!(h.editor.state().tables.is_empty());
    h.click("wt:use");
    assert_eq!(h.table_frames(1).len(), 8);
    assert_eq!(h.user(), 1.0);
    assert_eq!(h.editor.state().tables[&1].name, "sweep.wav");
    assert!(h.editor.undo());
    assert!(h.editor.state().tables.is_empty());
}

#[test]
fn broken_files_give_an_error_and_change_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut cases: Vec<(&str, Vec<u8>)> = vec![
        ("empty.wav", vec![]),
        ("text.wav", b"not a wav file at all".to_vec()),
        ("cut.wav", wav(&sines(2))[..30].to_vec()),
        ("silent.wav", wav(&[0.0; 4096])),
        ("tiny.wav", wav(&[0.5; 4])),
        ("many.wav", wav(&vec![0.3; 70 * FRAME])),
    ];
    // A "wav" whose fmt chunk claims more bytes than the file has.
    let mut lie = wav(&sines(1));
    lie[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
    cases.push(("lie.wav", lie));
    let mut h = H::new();
    h.open_editor();
    for (name, bytes) in cases {
        let p = dir.path().join(name);
        std::fs::write(&p, bytes).unwrap();
        h.type_path(p.to_str().unwrap());
        h.click("wt:import");
        // Either refused at once, or shown as a preview that cannot be used.
        assert!(h.ui.tables.pending_frames().is_none(), "{name} was accepted");
        assert!(h.editor.state().tables.is_empty(), "{name} changed the patch");
    }
    // Missing file, and a directory.
    for p in [dir.path().join("missing.wav"), dir.path().to_path_buf()] {
        h.type_path(p.to_str().unwrap());
        h.click("wt:import");
        assert!(h.ui.tables.error().is_some());
        assert!(h.ui.tables.pending_frames().is_none());
    }
    assert!(h.editor.state().tables.is_empty());
}

#[test]
fn a_file_over_the_size_limit_is_refused_without_reading_it_all() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("huge.wav");
    let f = std::fs::File::create(&p).unwrap();
    f.set_len(wavetable::MAX_IMPORT_BYTES as u64 * 4).unwrap(); // sparse
    let mut h = H::new();
    h.open_editor();
    h.type_path(p.to_str().unwrap());
    h.click("wt:import");
    assert!(h.ui.tables.pending_frames().is_none());
    assert!(h.editor.state().tables.is_empty());
}

#[test]
fn an_edited_table_survives_save_and_reload() {
    let mut h = H::new();
    h.open_editor();
    h.click("wt:insert");
    let before = h.table_frames(1);
    let dir = tempfile::tempdir().unwrap();
    kabl_core::save(dir.path(), h.editor.log()).unwrap();
    let back = PatchEditor::from_log(kabl_core::load(dir.path()).unwrap());
    assert_eq!(back.state().tables, h.editor.state().tables);
    assert_eq!(
        wavetable::decode_canonical(&back.state().tables[&1].wav).unwrap(),
        before
    );
}
