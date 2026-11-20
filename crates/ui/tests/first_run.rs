//! First run (UI spec 7): the start patch sounds right untouched, and the tour walks its four
//! steps, skips with Esc, never comes back once dismissed, and leaves the spotlit knob usable.
use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect};
use kabl_ui::browser::{Doc, DocOrigin};
use kabl_ui::library::Library;
use kabl_ui::tour::{self, Step};
use kabl_ui::{first_run, show, PatchEditor, UiState};

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    dir: std::path::PathBuf,
    events: Vec<Event>,
    t: f64,
}

impl H {
    /// The start patch in the rack, a library in a temporary folder, the tour at step 1.
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("kabl-first-run-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut ui = UiState::default();
        ui.library = Some(Library::open(None, dir.clone()));
        ui.first_run = tour::begin(false);
        let mut h = H {
            ctx: egui::Context::default(),
            editor: first_run::start_patch(),
            ui,
            dir,
            events: Vec::new(),
            t: 0.0,
        };
        h.ui.browser_open = true;
        // The app starts with the routing drawer closed.
        h.ui.drawer_open = false;
        h.frames(3);
        h
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
        let _ = self.ctx.end_pass();
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.frame();
        }
    }

    fn step(&self) -> Option<Step> {
        self.ui.first_run.as_ref().map(|t| t.step)
    }

    fn has_toast(&self, title: &str) -> bool {
        self.ui.toasts.iter().any(|t| t.title == title)
    }

    /// A sound from the library is loaded (what the browser's Load does to the document).
    fn load_a_sound(&mut self) {
        self.ui.doc = Some(Doc::new(
            "Acid Line",
            DocOrigin::Library("factory:acid-line".into()),
            self.editor.state(),
        ));
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

    /// Drags up from the middle of `key` by `px`.
    fn drag_up(&mut self, key: &str, px: f32) {
        let from = self.rect(key).center();
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
        self.events.push(Event::Key {
            key: Key::Escape,
            physical_key: Some(Key::Escape),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        });
        self.frame();
        self.events.push(Event::Key {
            key: Key::Escape,
            physical_key: Some(Key::Escape),
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        });
        self.frame();
    }

    /// Loads a sound and runs the tour to the spotlight (step 3).
    fn go_to_the_spotlight(&mut self) {
        self.load_a_sound();
        self.frames(3);
        assert_eq!(self.step(), Some(Step::Flows));
        self.t += 5.0;
        self.frames(4);
        assert_eq!(self.step(), Some(Step::Knob));
        // The knob's hit target is recorded by the frame after the view switched.
        self.frames(3);
    }

    fn cutoff_key(&self) -> String {
        let id = first_run::find_filter(self.editor.state()).expect("a filter");
        format!("knob:{id}.cutoff_hz")
    }

    fn cutoff(&self) -> f32 {
        let id = first_run::find_filter(self.editor.state()).unwrap();
        self.editor.state().modules[&id].params["cutoff_hz"]
    }
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
        // One obvious modulated knob: the envelope into the filter cutoff.
        let into_cutoff = state.cables.values().filter(
            |cb| matches!(&cb.to, kabl_core::PortRef::Param { param, .. } if param == "cutoff_hz"),
        );
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
fn a_dismissed_tour_never_comes_back() {
    assert!(tour::begin(false).is_some());
    assert!(tour::begin(true).is_none());
    let mut h = H::new("esc");
    assert_eq!(h.step(), Some(Step::Sounds));
    assert!(h.has_toast(tour::TOASTS[0]), "step 1's toast");
    h.esc();
    assert_eq!(h.step(), None, "Esc ends the tour");
    assert!(!h.has_toast(tour::TOASTS[0]), "its toast goes with it");
    assert!(h.ui.library.as_ref().unwrap().prefs.first_run_done);
    // Persisted: a library opened on the same folder says so.
    let again = Library::open(None, h.dir.clone());
    assert!(again.prefs.first_run_done);
    assert!(tour::begin(again.prefs.first_run_done).is_none());
}

#[test]
fn the_tour_walks_its_four_steps() {
    let mut h = H::new("walk");
    // 1: Sounds is open, the toast says what to do, and nothing moves on by itself.
    assert_eq!(h.step(), Some(Step::Sounds));
    assert!(h.has_toast(tour::TOASTS[0]));
    h.frames(30);
    assert_eq!(h.step(), Some(Step::Sounds));
    // 2: a sound is loaded: the rack, and the toast about the glowing cables.
    h.load_a_sound();
    h.frames(3);
    assert_eq!(h.step(), Some(Step::Flows));
    assert!(!h.ui.browser_open, "the view goes to the rack");
    assert!(h.has_toast(tour::TOASTS[1]));
    assert!(!h.has_toast(tour::TOASTS[0]));
    // 3: the spotlight, with its card.
    h.t += 5.0;
    h.frames(4);
    assert_eq!(h.step(), Some(Step::Knob));
    h.frames(3);
    assert!(h.ui.hits.contains_key("tour-next") && h.ui.hits.contains_key("tour-skip"));
    // Next: 4, make it yours.
    h.click("tour-next");
    assert_eq!(h.step(), Some(Step::Own));
    h.frames(2);
    assert!(h.has_toast(tour::TOASTS[2]));
    assert!(!h.ui.library.as_ref().unwrap().prefs.first_run_done);
    // Dismissing the last toast ends it, for good.
    h.ui.toasts.retain(|t| t.title != tour::TOASTS[2]);
    h.frames(2);
    assert_eq!(h.step(), None);
    assert!(h.ui.library.as_ref().unwrap().prefs.first_run_done);
}

#[test]
fn skip_tour_on_the_card_ends_it() {
    let mut h = H::new("skip");
    h.go_to_the_spotlight();
    h.click("tour-skip");
    assert_eq!(h.step(), None);
    assert!(h.ui.library.as_ref().unwrap().prefs.first_run_done);
}

#[test]
fn the_spotlit_knob_works_and_the_rest_of_the_rack_does_not() {
    let mut h = H::new("knob");
    h.go_to_the_spotlight();
    // Another knob under the scrim does not move.
    let delay = h
        .editor
        .state()
        .modules
        .iter()
        .find(|(_, m)| m.kind == "delay")
        .map(|(&id, _)| id)
        .unwrap();
    let before = h.editor.state().modules[&delay]
        .params
        .get("feedback")
        .copied();
    h.drag_up(&format!("knob:{delay}.feedback"), 60.0);
    assert_eq!(
        h.editor.state().modules[&delay]
            .params
            .get("feedback")
            .copied(),
        before,
        "the scrim takes the pointer"
    );
    assert_eq!(h.step(), Some(Step::Knob));
    // The cutoff does: turning it is the step, and the tour moves on.
    let was = h.cutoff();
    h.drag_up(&h.cutoff_key(), 60.0);
    assert!(h.cutoff() > was, "the knob turned: {was} -> {}", h.cutoff());
    assert_eq!(h.step(), Some(Step::Own));
}

#[test]
fn unplugging_the_output_offers_to_plug_it_back() {
    let mut h = H::new("unplug");
    h.go_to_the_spotlight();
    h.click("tour-next");
    assert_eq!(h.step(), Some(Step::Own));
    h.frames(3);
    // The cable into Output.
    let out = h
        .editor
        .state()
        .modules
        .iter()
        .find(|(_, m)| m.kind == "out")
        .map(|(&id, _)| id)
        .unwrap();
    let cable = h
        .editor
        .state()
        .cables
        .iter()
        .find(|(_, c)| matches!(&c.to, kabl_core::PortRef::Module { id, .. } if *id == out))
        .map(|(&id, _)| id)
        .unwrap();
    let cables = h.editor.state().cables.len();
    h.editor.disconnect(cable);
    h.frames(3);
    assert!(
        h.ui.hits.contains_key("tour-plug-back"),
        "the empty-state hint"
    );
    assert!(h.ui.hits.contains_key("tour-browse"));
    // The tour waits while the hint shows, even past its own timeout.
    h.t += 60.0;
    h.frames(3);
    assert_eq!(h.step(), Some(Step::Own));
    // Plug it back: the cable returns, the hint goes.
    h.click("tour-plug-back");
    assert_eq!(h.editor.state().cables.len(), cables, "the cable is back");
    h.frames(3);
    assert!(!h.ui.hits.contains_key("tour-plug-back"));
}

#[test]
fn the_card_sits_beside_the_spotlight_and_keeps_off_the_other_modules() {
    let mut h = H::new("card");
    h.go_to_the_spotlight();
    let card = h.rect("tour-card");
    let filter = first_run::find_filter(h.editor.state()).unwrap();
    for (k, face) in &h.ui.hits {
        let Some(id) = k
            .strip_prefix("module:")
            .and_then(|s| s.parse::<u64>().ok())
        else {
            continue;
        };
        if id == filter {
            continue;
        }
        assert!(
            !card.intersects(*face),
            "the card {card:?} covers module {id} at {face:?}"
        );
    }
    // And it is clear of the spotlit knob itself.
    assert!(!card.intersects(h.rect(&h.cutoff_key())));
}
