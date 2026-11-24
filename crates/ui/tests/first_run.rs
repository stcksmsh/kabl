//! First run (UI spec 7): the start patch sounds right untouched, and a fresh profile opens it
//! playing in the Rack with lesson 00 on the Start card: no scrim, an anchor ring that takes no
//! input, Next from every step, Esc as Skip, Stop that silences it, and never again once the
//! first run happened (the old tour's flag included).
use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect};
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::PatchEngine;
use kabl_modules::builtins::Transport;
use kabl_ui::library::Library;
use kabl_ui::{first_run, lessons, show, PatchEditor, UiState};

/// Lesson 00's targets on the start patch (`lessons/00-start-here.toml`).
const FILTER: u64 = 3;
const MACRO: u64 = 9;
const CUTOFF: &str = "knob:3.cutoff_hz";

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    dir: tempfile::TempDir,
    events: Vec<Event>,
    t: f64,
    shapes: Vec<egui::epaint::ClippedShape>,
}

impl H {
    /// What the standalone app does on launch with no patch named, on the profile in `dir`.
    fn launch(dir: tempfile::TempDir) -> Self {
        let mut ui = UiState::default();
        ui.library = Some(Library::open(None, dir.path().to_path_buf()));
        // The standalone app owns the devices (`first_run::devices::take_over`); a plugin does not.
        ui.devices = Some(Default::default());
        // The app starts with the routing drawer closed.
        ui.drawer_open = false;
        let editor = first_run::open(&mut ui);
        let mut h = H {
            ctx: egui::Context::default(),
            editor,
            ui,
            dir,
            events: Vec::new(),
            t: 0.0,
            shapes: Vec::new(),
        };
        h.frames(3);
        h
    }

    fn fresh() -> Self {
        H::launch(tempfile::tempdir().unwrap())
    }

    fn frame(&mut self) {
        let size = egui::vec2(1440.0, 900.0);
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            events: std::mem::take(&mut self.events),
            time: Some(self.t),
            ..Default::default()
        };
        self.t += 1.0 / 60.0;
        self.ctx.begin_pass(raw);
        let mut root = egui::Ui::new(
            self.ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::ZERO, size)),
        );
        show(&mut self.editor, &mut self.ui, &mut root);
        self.shapes = self.ctx.end_pass().shapes;
    }

    /// A ring was painted around `r` (the anchor outline, 6 px out).
    fn ringed(&self, r: Rect) -> bool {
        let want = r.expand(6.0);
        self.shapes.iter().any(|c| {
            matches!(&c.shape, egui::Shape::Rect(s) if s.rect == want && s.stroke.width == 2.5)
                && c.clip_rect.contains_rect(want)
        })
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.frame();
        }
    }

    /// Lesson 00's step, `None` when it is not running.
    fn step(&self) -> Option<usize> {
        let a = self.ui.lessons.active.as_ref()?;
        assert_eq!(lessons::all()[a.lesson].id, "start-here");
        Some(a.step)
    }

    fn step_done(&self) -> bool {
        let a = self.ui.lessons.active.as_ref().unwrap();
        a.done[a.step]
    }

    fn rect(&self, key: &str) -> Rect {
        *self
            .ui
            .hits
            .get(key)
            .unwrap_or_else(|| panic!("no hit target {key}"))
    }

    fn press(&mut self, at: Pos2, pressed: bool) {
        self.events.push(Event::PointerMoved(at));
        self.events.push(Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
        self.frame();
    }

    fn click(&mut self, key: &str) {
        let at = self.rect(key).center();
        self.events.push(Event::PointerMoved(at));
        self.frame();
        self.press(at, true);
        self.press(at, false);
        self.frames(2);
    }

    /// Drags up by `px` from `from`.
    fn drag_up_from(&mut self, from: Pos2, px: f32) {
        self.events.push(Event::PointerMoved(from));
        self.frame();
        self.press(from, true);
        for k in 1..=8 {
            let at = from - egui::vec2(0.0, px * k as f32 / 8.0);
            self.events.push(Event::PointerMoved(at));
            self.frame();
        }
        self.press(from - egui::vec2(0.0, px), false);
        self.frames(2);
    }

    fn esc(&mut self) {
        for pressed in [true, false] {
            self.events.push(Event::Key {
                key: Key::Escape,
                physical_key: Some(Key::Escape),
                pressed,
                repeat: false,
                modifiers: Modifiers::NONE,
            });
            self.frame();
        }
    }

    fn param(&self, id: u64, p: &str) -> Option<f32> {
        self.editor.state().modules[&id].params.get(p).copied()
    }

    fn first_run_done(&self) -> bool {
        Library::open(None, self.dir.path().to_path_buf())
            .prefs
            .first_run_done
    }
}

/// Peak of `secs` of `engine`'s output.
fn peak(engine: &mut PatchEngine, secs: f32) -> f32 {
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    let mut m = 0f32;
    for _ in 0..(secs * 48000.0) as usize / BLOCK {
        engine.process_block(&mut l, &mut r);
        m = l.iter().chain(&r).fold(m, |m, s| m.max(s.abs()));
    }
    m
}

fn engine(h: &H) -> (basedrop::Collector, PatchEngine) {
    let c = basedrop::Collector::new();
    let e = PatchEngine::new(&c.handle(), h.editor.state(), 48000.0, 8).unwrap();
    (c, e)
}

#[test]
fn every_start_patch_sounds_right_untouched() {
    for c in first_run::candidates() {
        let e = c.build();
        let state = e.state();
        // It plays by itself: a clock drives the sequencer.
        let kinds: Vec<_> = state.modules.values().map(|m| m.kind.as_str()).collect();
        for k in [
            "clock",
            "seq",
            "osc.va",
            "filter.svf",
            "env.adsr",
            "vca",
            "out",
        ] {
            assert!(kinds.contains(&k), "{}: no {k}", c.id);
        }
        // One obvious modulated knob: the envelope into the filter cutoff (Brightness, a
        // macro, reaches it too).
        let into_cutoff = state.cables.values().filter(|cb| {
            matches!(&cb.to, kabl_core::PortRef::Param { param, .. } if param == "cutoff_hz")
                && matches!(&cb.from, kabl_core::PortRef::Module { id, .. }
                    if state.modules[id].kind == "env.adsr")
        });
        assert_eq!(into_cutoff.count(), 1, "{}", c.id);
        // Gain staging, no limiter: headroom and a sensible level, and it never goes silent.
        let a = first_run::render(state, 8.0, 48000.0).unwrap();
        assert!(a.iter().all(|s| s.is_finite()), "{}", c.id);
        let peak = a.iter().fold(0f32, |m, s| m.max(s.abs()));
        let rms = (a.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / a.len() as f64).sqrt();
        let db = 20.0 * rms.log10();
        assert!(peak <= 0.75, "{}: peak {peak}", c.id);
        assert!((-24.0..=-12.0).contains(&db), "{}: rms {db:.1} dB", c.id);
    }
}

#[test]
fn a_fresh_profile_starts_in_the_rack_with_the_start_patch_playing_and_lesson_00() {
    let h = H::fresh();
    // The Rack, not Sounds or Perform.
    assert!(!h.ui.browser_open, "Sounds is open");
    assert!(!h.ui.perform_open);
    // The start patch, under its own name, clean.
    assert_eq!(h.editor.state(), first_run::start_patch().state());
    assert_eq!(h.ui.doc.as_ref().unwrap().name, first_run::start_name());
    assert!(!kabl_ui::browser::is_modified(&h.editor, &h.ui));
    // Its transport runs: nothing stopped it, and an engine on it plays by itself.
    assert!(h.ui.clock_running.values().all(|r| *r));
    assert!(!h.ui.transport.iter().any(|(_, t)| *t == Transport::Stop));
    let (_c, mut e) = engine(&h);
    assert!(peak(&mut e, 2.0) > 0.01, "silent");
    // Lesson 00 at step 1, on the Start card.
    assert_eq!(h.step(), Some(0));
    for key in ["start-card", "start-next", "start-skip", "start-stop"] {
        assert!(h.ui.hits.contains_key(key), "no {key}");
    }
    // Remembered as it starts: closing the app mid-lesson does not bring it back.
    assert!(h.first_run_done());
    let again = H::launch(h.dir);
    assert_eq!(again.step(), None);
    assert!(!again.ui.hits.contains_key("start-card"));
    assert!(
        again.ui.browser_open,
        "later launches open Sounds over the start patch"
    );
}

#[test]
fn the_old_first_run_flag_means_no_lesson_00() {
    let dir = tempfile::tempdir().unwrap();
    // A profile where the old tour ran: its flag set and saved.
    let mut lib = Library::open(None, dir.path().to_path_buf());
    lib.prefs.first_run_done = true;
    lib.save_prefs();
    let h = H::launch(dir);
    assert_eq!(h.step(), None);
    assert!(!h.ui.hits.contains_key("start-card"));
    assert_eq!(h.editor.state(), first_run::start_patch().state());
}

#[test]
fn the_anchor_ring_takes_no_input_and_nothing_else_is_blocked() {
    let mut h = H::fresh();
    h.click("start-next");
    assert_eq!(h.step(), Some(1));
    // The ring goes around the cutoff knob; a drag where it is turns the knob.
    let knob = lessons::anchor_rect(&h.ui, CUTOFF).expect("the ringed knob is drawn");
    let was = h.param(FILTER, "cutoff_hz").unwrap();
    h.drag_up_from(knob.center(), 60.0);
    let now = h.param(FILTER, "cutoff_hz").unwrap();
    assert!(now > was, "the knob under the ring turned: {was} -> {now}");
    assert!(
        h.step_done() || h.step() == Some(2),
        "turning it is the step"
    );
    // No scrim: another knob, away from the ring, turns too.
    let delay = h
        .editor
        .state()
        .modules
        .iter()
        .find(|(_, m)| m.kind == "delay")
        .map(|(&id, _)| id)
        .unwrap();
    let before = h.param(delay, "feedback");
    h.drag_up_from(h.rect(&format!("knob:{delay}.feedback")).center(), 60.0);
    assert_ne!(h.param(delay, "feedback"), before, "the rack is blocked");
    // The card keeps off the ringed knob.
    assert!(!h.rect("start-card").intersects(knob));
}

#[test]
fn next_moves_on_from_every_step_and_close_ends_it() {
    let mut h = H::fresh();
    let l = lessons::all().iter().find(|l| l.card).unwrap();
    let ops = h.editor.log().all_entries().len();
    for (k, s) in l.steps.iter().enumerate() {
        assert_eq!(h.step(), Some(k));
        if let Some(a) = &s.anchor {
            assert!(
                lessons::anchor_rect(&h.ui, a).is_some(),
                "step {}: anchor {a:?} is not drawn",
                k + 1
            );
        }
        h.click("start-next");
    }
    assert_eq!(h.step(), Some(l.steps.len()), "the end page");
    assert!(!h.ui.hits.contains_key("start-next"));
    // The lesson never edits the patch.
    assert_eq!(h.editor.log().all_entries().len(), ops);
    h.click("start-skip");
    assert_eq!(h.step(), None);
    assert!(!h.ui.hits.contains_key("start-card"));
}

#[test]
fn a_step_met_before_it_is_reached_shows_done() {
    let mut h = H::fresh();
    // On step 1 the user turns the cutoff, which step 2 asks for.
    let was = h.param(FILTER, "cutoff_hz").unwrap();
    h.editor.set_param(FILTER, "cutoff_hz", was + 200.0);
    h.frames(2);
    assert_eq!(h.step(), Some(0), "step 1 has nothing to do but listen");
    h.click("start-next");
    h.frames(5);
    assert_eq!(h.step(), Some(1), "met earlier: it waits for Next");
    assert!(h.step_done(), "step 2 shows done on arrival");
    h.click("start-next");
    assert_eq!(h.step(), Some(2));
    assert!(!h.step_done(), "the macro was never turned");
    // Either macro completes its step; done here, it moves on by itself.
    h.editor.set_param(MACRO, "m2", 0.3);
    h.frames(2);
    assert_eq!(
        h.step(),
        Some(3),
        "turning the second macro completes step 3"
    );
    h.ui.browser_open = true;
    h.frames(2);
    assert_eq!(h.step(), Some(4), "opening Sounds completes step 4");
}

#[test]
fn esc_is_skip_and_the_lesson_does_not_come_back() {
    let mut h = H::fresh();
    h.esc();
    assert_eq!(h.step(), None, "Esc ends it");
    assert!(!h.ui.hits.contains_key("start-card"));
    // The start patch plays on, untouched.
    assert_eq!(h.editor.state(), first_run::start_patch().state());
    assert!(h.first_run_done());
    let again = H::launch(h.dir);
    assert_eq!(again.step(), None);
}

#[test]
fn skip_ends_it_too() {
    let mut h = H::fresh();
    h.click("start-next");
    h.click("start-skip");
    assert_eq!(h.step(), None);
}

#[test]
fn stop_silences_the_output() {
    let mut h = H::fresh();
    let (_c, mut e) = engine(&h);
    assert!(peak(&mut e, 1.0) > 0.01, "plays before Stop");
    h.ui.transport.clear();
    h.click("start-stop");
    let sent = std::mem::take(&mut h.ui.transport);
    assert!(!sent.is_empty() && sent.iter().all(|(_, t)| *t == Transport::Stop));
    for (id, t) in sent {
        e.transport(id, t);
    }
    // The echoes and the room die away, and nothing new plays.
    peak(&mut e, 8.0);
    assert!(peak(&mut e, 1.0) < 1e-4, "still sounding after Stop");
    assert!(h.ui.clock_running.values().all(|r| !*r));
    // The same button now plays it again; Stop is not Skip.
    h.click("start-stop");
    assert!(!h.ui.transport.is_empty());
    assert!(h.ui.transport.iter().all(|(_, t)| *t == Transport::Run));
    assert_eq!(h.step(), Some(0));
}

#[test]
fn no_audio_output_still_opens_the_rack_and_the_card_says_so() {
    let mut h = H::fresh();
    h.ui.inspect.audio = false;
    h.frames(2);
    assert!(
        !h.ui.hits.contains_key("start-no-output"),
        "not while it may still be starting"
    );
    h.t += 3.0;
    h.frames(2);
    assert!(h.ui.hits.contains_key("start-no-output"));
    assert!(!h.ui.browser_open);
    h.ui.inspect.audio = true;
    h.frames(2);
    assert!(!h.ui.hits.contains_key("start-no-output"));
}

#[test]
fn learn_starts_it_again_only_over_a_saved_sound() {
    let mut h = H::fresh();
    h.esc();
    let i = lessons::all().iter().position(|l| l.card).unwrap();
    // Unsaved changes: it asks to save first and leaves the sound alone.
    h.editor.set_param(FILTER, "resonance", 0.9);
    let edited = h.editor.state().clone();
    lessons::start(&mut h.editor, &mut h.ui, i);
    assert_eq!(h.step(), None);
    assert!(h.ui.lessons.note.is_some());
    assert_eq!(h.editor.state(), &edited);
    // Nothing unsaved: the start patch opens again with the lesson on it.
    h.editor.undo();
    lessons::start(&mut h.editor, &mut h.ui, i);
    h.frames(3);
    assert_eq!(h.step(), Some(0));
    assert!(h.ui.hits.contains_key("start-card"));
    assert_eq!(h.editor.state(), first_run::start_patch().state());
}

#[test]
fn every_step_rings_its_anchor_on_screen_the_sounds_tab_included() {
    let mut h = H::fresh();
    let l = lessons::all().iter().find(|l| l.card).unwrap();
    for (k, s) in l.steps.iter().enumerate() {
        if let Some(a) = &s.anchor {
            let r = lessons::anchor_rect(&h.ui, a).unwrap();
            assert!(h.ringed(r), "step {}: no ring around {a:?}", k + 1);
        }
        h.click("start-next");
    }
}
