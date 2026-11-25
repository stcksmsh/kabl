//! Composite face toggles: a click flips the leaf param's base value as one undo step, also
//! while the param is modulated.
use egui::{Event, PointerButton, Pos2, RawInput, Rect};
use kabl_core::{
    panel::{Kind, Panel, Placement},
    PortRef,
};
use kabl_ui::{composites, show, CableView, PatchEditor, UiState};

/// The default patch's VCA.
const VCA: u64 = 5;

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    t: f64,
    events: Vec<Event>,
    /// Every string painted by the last frame.
    texts: Vec<String>,
}

fn texts(shape: &egui::Shape, out: &mut Vec<String>) {
    match shape {
        egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
        egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
        _ => {}
    }
}

impl H {
    /// The VCA as a composite whose face is one toggle on its "exponential" switch.
    fn new() -> Self {
        let mut e = PatchEditor::seed_from(&kabl_standalone::default_patch());
        let id = composites::encapsulate(&mut e, [VCA].into(), Default::default(), "Amp").unwrap();
        let target = PortRef::Param {
            id: VCA,
            param: "exponential".into(),
        };
        composites::add_exposure(&mut e, id, target, true).unwrap();
        let mut c = e.state().composites[&id].clone();
        let key = *c.controls.keys().next().unwrap();
        let mut panel = Panel::default();
        panel.placements.insert(
            key,
            Placement {
                kind: Kind::Toggle,
                x: 12.0,
                y: 72.0,
                width: 64.0,
                height: 55.0,
                source: None,
                label: None,
                icons: Vec::new(),
            },
        );
        c.panel = Some(Box::new(panel));
        composites::set(&mut e, id, c).unwrap();
        let mut h = H {
            ctx: egui::Context::default(),
            editor: e,
            ui: UiState::default(),
            t: 0.0,
            events: Vec::new(),
            texts: Vec::new(),
        };
        h.frames(3);
        h
    }

    fn frames(&mut self, n: usize) {
        let size = egui::vec2(1440.0, 900.0);
        for _ in 0..n {
            let raw = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                time: Some(self.t),
                events: std::mem::take(&mut self.events),
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
            self.texts.clear();
            for s in self.ctx.end_pass().shapes {
                texts(&s.shape, &mut self.texts);
            }
        }
    }

    fn button(&mut self, p: Pos2, pressed: bool) {
        self.events.push(Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        });
        self.frames(1);
    }

    /// Clicks the toggle through its recorded hit rect.
    fn click(&mut self) {
        let p = self
            .ui
            .hits
            .get(&format!("toggle:{VCA}.exponential"))
            .expect("the face toggle records its rect")
            .center();
        self.events.push(Event::PointerMoved(p));
        self.frames(1);
        self.button(p, true);
        self.button(p, false);
        self.frames(2);
    }

    fn exponential(&self) -> f32 {
        self.editor.state().modules[&VCA].params["exponential"]
    }

    fn undo_depth(&self) -> usize {
        self.editor.log().entries().len()
    }
}

#[test]
fn a_click_flips_the_toggle_as_one_undo_step() {
    let mut h = H::new();
    assert_eq!(h.exponential(), 0.0);
    let depth = h.undo_depth();
    h.click();
    assert_eq!(h.exponential(), 1.0);
    assert_eq!(h.undo_depth(), depth + 1);
    h.click();
    assert_eq!(h.exponential(), 0.0, "a second click flips back");
    assert_eq!(h.undo_depth(), depth + 2, "each click is its own step");
    h.editor.undo();
    assert_eq!(h.exponential(), 1.0);
    h.editor.undo();
    assert_eq!(h.exponential(), 0.0);
}

#[test]
fn a_modulated_toggle_flips_its_base_value() {
    let mut h = H::new();
    h.editor.connect_route(
        PortRef::Module {
            id: 4,
            port: "out".into(),
        },
        VCA,
        "exponential",
    );
    let target = PortRef::Param {
        id: VCA,
        param: "exponential".into(),
    };
    assert!(h.editor.state().cables.values().any(|c| c.to == target));
    h.frames(2);
    h.click();
    assert_eq!(h.exponential(), 1.0);
    h.editor.undo();
    assert!(h.editor.state().cables.values().any(|c| c.to == target));
    assert_eq!(h.exponential(), 0.0);
}

#[test]
fn a_modulated_toggle_shows_the_route_badge() {
    let mut h = H::new();
    h.ui.cable_view = CableView::Hidden;
    h.frames(2);
    assert!(!h.texts.iter().any(|t| t == "< 1 mod"));
    h.editor.connect_route(
        PortRef::Module {
            id: 4,
            port: "out".into(),
        },
        VCA,
        "exponential",
    );
    h.frames(2);
    assert!(h.texts.iter().any(|t| t == "< 1 mod"), "no route badge");
}
