//! A composite control on a route's amount: the design page offers it as a knob only, the face
//! shows a stored amount as it is (no clamp, G2), and two controls on one route are refused.
use egui::{Event, PointerButton, Pos2, RawInput, Rect};
use kabl_core::{
    panel::{Kind, Panel, Placement},
    CableId, CompositeId, Definition, Exposure, ModuleId, PortRef,
};
use kabl_ui::{composites, show, PatchEditor, UiState};

/// The default patch's filter.
const FILTER: ModuleId = 3;

fn amount(cable: CableId) -> PortRef {
    PortRef::CableParam {
        cable,
        param: "amount".into(),
    }
}

/// The default patch with its envelope routed into the filter's cutoff.
fn routed() -> (PatchEditor, ModuleId, CableId) {
    let mut e = PatchEditor::seed_from(&kabl_standalone::default_patch());
    let env = *e
        .state()
        .modules
        .iter()
        .find(|(_, m)| m.kind == "env.adsr")
        .unwrap()
        .0;
    let route = e.connect_route(
        PortRef::Module {
            id: env,
            port: "out".into(),
        },
        FILTER,
        "cutoff_hz",
    );
    (e, env, route)
}

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    t: f64,
    events: Vec<Event>,
    id: CompositeId,
    route: CableId,
    /// The control's interface key.
    key: u64,
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
    /// The envelope and filter as a composite with a control on the route's amount, placed on
    /// its face as a knob when `placed`.
    fn new(placed: bool) -> Self {
        let (mut e, env, route) = routed();
        let id = composites::encapsulate(&mut e, [env, FILTER].into(), Default::default(), "Voice")
            .unwrap();
        composites::add_exposure(&mut e, id, amount(route), true).unwrap();
        let mut c = e.state().composites[&id].clone();
        let key = *c.controls.keys().next().unwrap();
        if placed {
            let mut panel = Panel::default();
            panel.placements.insert(
                key,
                Placement {
                    kind: Kind::Knob,
                    x: 12.0,
                    y: 72.0,
                    width: 84.0,
                    height: 110.0,
                    source: None,
                    label: None,
                    icons: Vec::new(),
                },
            );
            c.panel = Some(Box::new(panel));
            composites::set(&mut e, id, c).unwrap();
        }
        let mut h = H {
            ctx: egui::Context::default(),
            editor: e,
            ui: UiState::default(),
            t: 0.0,
            events: Vec::new(),
            id,
            route,
            key,
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

    fn hit(&self, key: &str) -> Rect {
        *self
            .ui
            .hits
            .get(key)
            .unwrap_or_else(|| panic!("no hit rect {key}"))
    }

    fn click(&mut self, key: &str) {
        let p = self.hit(key).center();
        self.events.push(Event::PointerMoved(p));
        self.frames(1);
        self.button(p, true);
        self.button(p, false);
        self.frames(2);
    }

    fn stored(&self) -> f32 {
        self.editor.state().cables[&self.route].params["amount"]
    }
}

#[test]
fn the_design_page_offers_a_route_amount_and_a_click_places_a_knob() {
    let mut h = H::new(false);
    let c = h.editor.state().composites[&h.id].clone();
    kabl_ui::panels::open(&mut h.ui, h.id, &c);
    h.frames(2);
    h.click(&format!("design:palette:{}", h.key));
    let page = h.ui.panels.page.as_ref().expect("the design page is open");
    let placed = &page.draft.panel.placements[&h.key];
    assert_eq!(placed.kind, Kind::Knob);
    assert!(h.ui.hits.contains_key(&format!("design:el:{}", h.key)));
}

#[test]
fn a_route_amount_is_drawn_as_a_knob_only() {
    let mut h = H::new(true);
    let mut c = h.editor.state().composites[&h.id].clone();
    let panel = c.panel.as_mut().unwrap();
    let p = panel.placements.get_mut(&h.key).unwrap();
    (p.kind, p.height) = (Kind::Slider, 168.0);
    let err = composites::set(&mut h.editor, h.id, c).unwrap_err();
    assert!(err.contains("a route's amount is drawn as a knob"), "{err}");
}

#[test]
fn a_stored_amount_outside_the_range_is_shown_and_kept_until_turned() {
    let mut h = H::new(true);
    // A file may carry any finite amount; the engine, not the face, decides what it does.
    let mut state = h.editor.state().clone();
    state
        .cables
        .get_mut(&h.route)
        .unwrap()
        .params
        .insert("amount".into(), 1.5);
    h.editor = PatchEditor::seed_from(&state);
    h.frames(3);
    assert!(h.texts.iter().any(|t| t == "+150 %"), "{:?}", h.texts);
    assert_eq!(h.stored(), 1.5, "drawing the knob does not clamp");
    // A drag writes the range's own values: down from 1.5 lands inside -1..1.
    let at = h.hit(&format!("knob:cable{}.amount", h.route)).center();
    h.events.push(Event::PointerMoved(at));
    h.frames(1);
    h.button(at, true);
    for i in 1..=8 {
        h.events
            .push(Event::PointerMoved(at + egui::vec2(0.0, 10.0 * i as f32)));
        h.frames(1);
    }
    h.button(at + egui::vec2(0.0, 80.0), false);
    assert!(h.stored() <= 1.0, "{}", h.stored());
}

#[test]
fn two_controls_on_one_route_amount_are_refused() {
    let (mut e, env, route) = routed();
    let knob = |label: &str| Exposure {
        label: label.into(),
        target: amount(route),
        also: vec![],
    };
    let mut log = e.log().clone();
    let err = composites::encapsulate_with(
        &mut log,
        "Voice",
        Definition {
            id: "voice".into(),
            version: 1,
        },
        [env, FILTER].into(),
        &[],
        &[knob("Depth"), knob("Depth 2")],
        None,
    )
    .unwrap_err();
    assert!(err.contains("is exposed twice"), "{err}");

    // Exposing it again from the editor adds nothing.
    let id =
        composites::encapsulate(&mut e, [env, FILTER].into(), Default::default(), "Voice").unwrap();
    for _ in 0..2 {
        composites::add_exposure(&mut e, id, amount(route), true).unwrap();
    }
    assert_eq!(e.state().composites[&id].controls.len(), 1);
}
