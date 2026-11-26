//! Composite face displays: which leaves the design page offers, which sources a face refuses,
//! the tap a display asks the engine for, and drawing a leaf nested two composites deep.
use egui::{Event, PointerButton, Pos2, RawInput, Rect};
use kabl_core::{
    panel::{Kind, Panel, Placement, Source},
    CompositeId, ModuleId,
};
use kabl_engine::patch_engine::Command;
use kabl_ui::{composites, design_page, help, panels, show, PatchEditor, UiState};

/// The default patch's oscillator, filter, envelope and VCA.
const OSC: u64 = 2;
const FILTER: u64 = 3;
const ENV: u64 = 4;
const VCA: u64 = 5;

fn display(leaf: ModuleId, x: f32, y: f32) -> Placement {
    Placement {
        kind: Kind::Display,
        x,
        y,
        width: 120.0,
        height: 60.0,
        source: Some(Source::Module {
            id: leaf,
            action: None,
        }),
        label: None,
        icons: Vec::new(),
    }
}

/// "Top" holds the filter and the VCA and "Mid", which holds the envelope and "Deep", which
/// holds the oscillator.
fn nested() -> (PatchEditor, CompositeId) {
    let mut e = PatchEditor::seed_from(&kabl_standalone::default_patch());
    let deep = composites::encapsulate(&mut e, [OSC].into(), Default::default(), "Deep").unwrap();
    let mid = composites::encapsulate(&mut e, [ENV].into(), [deep].into(), "Mid").unwrap();
    let top = composites::encapsulate(&mut e, [FILTER, VCA].into(), [mid].into(), "Top").unwrap();
    (e, top)
}

/// `placements` as the face of `id`, each under a fresh key; the keys in order.
fn with_face(e: &mut PatchEditor, id: CompositeId, placements: Vec<Placement>) -> Vec<u64> {
    let mut c = e.state().composites[&id].clone();
    let mut panel = Panel {
        width: 840.0,
        ..Panel::default()
    };
    let keys: Vec<u64> = placements
        .into_iter()
        .map(|p| {
            let key = c.next_interface;
            c.next_interface += 1;
            panel.placements.insert(key, p);
            key
        })
        .collect();
    c.panel = Some(Box::new(panel));
    composites::set(e, id, c).unwrap();
    keys
}

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    t: f64,
    events: Vec<Event>,
}

impl H {
    fn with(editor: PatchEditor) -> Self {
        let mut ui = UiState::default();
        // Taps are asked for only while audio runs; nothing is played here.
        ui.inspect.audio = true;
        let mut h = H {
            ctx: egui::Context::default(),
            editor,
            ui,
            t: 0.0,
            events: Vec::new(),
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
            // Nothing is painted: the tests read the state, hit rects and engine commands.
            let _ = self.ctx.end_pass();
        }
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
        for pressed in [true, false] {
            self.events.push(Event::PointerButton {
                pos: p,
                button: PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            });
            self.frames(1);
        }
        self.frames(2);
    }

    /// The modules the last face-tap command sent to the engine reads with a view.
    fn tapped(&self) -> Vec<ModuleId> {
        let targets = self
            .ui
            .launches
            .iter()
            .rev()
            .find_map(|c| match c {
                Command::Faces(t) => Some(*t),
                _ => None,
            })
            .expect("a face-tap command");
        targets.map_or(Vec::new(), |t| {
            t.t[..t.n as usize]
                .iter()
                .filter(|t| t.view)
                .map(|t| t.module)
                .collect()
        })
    }
}

#[test]
fn the_design_page_lists_every_leaf_with_a_display_nested_ones_by_their_composites() {
    let (e, top) = nested();
    let title = |kind: &str, id| help::module_title(kind, id);
    let want = vec![
        (OSC, format!("Mid / Deep / {}", title("osc.va", OSC))),
        (FILTER, title("filter.svf", FILTER)),
        (ENV, format!("Mid / {}", title("env.adsr", ENV))),
    ];
    assert_eq!(design_page::displays(e.state(), top), want);

    // The page shows the list; a click places the leaf's display under a fresh key.
    let mut h = H::with(e);
    let c = h.editor.state().composites[&top].clone();
    panels::open(&mut h.ui, top, &c);
    h.frames(2);
    for (leaf, _) in &want {
        h.hit(&format!("design:display:{leaf}"));
    }
    h.click(&format!("design:display:{OSC}"));
    let page = h.ui.panels.page.as_ref().expect("the design page is open");
    assert_eq!(page.note, None);
    let key = page
        .display_of(OSC)
        .expect("the oscillator's display is placed");
    assert_eq!(key, c.next_interface);
    assert_eq!(page.draft.next_interface, c.next_interface + 1);
    h.click("design:apply");
    let applied = &h.editor.state().composites[&top];
    let p = &applied.panel.as_ref().unwrap().placements[&key];
    assert_eq!(p.kind, Kind::Display);
    assert_eq!(applied.next_interface, c.next_interface + 1);
    panels::validate(h.editor.state()).unwrap();
}

#[test]
fn a_display_on_a_kind_without_one_is_refused_naming_the_binding() {
    let (mut e, top) = nested();
    let key = with_face(&mut e, top, vec![display(FILTER, 12.0, 72.0)])[0];
    let mut state = e.state().clone();
    let panel = state
        .composites
        .get_mut(&top)
        .unwrap()
        .panel
        .as_mut()
        .unwrap();
    panel.placements.get_mut(&key).unwrap().source = Some(Source::Module {
        id: VCA,
        action: None,
    });
    assert_eq!(
        panels::validate(&state).unwrap_err(),
        format!("Binding {key}: vca modules have no display")
    );
    assert!(design_page::displays(&state, top)
        .iter()
        .all(|&(m, _)| m != VCA));
}

#[test]
fn a_closed_composite_asks_for_the_tap_of_a_leaf_two_composites_deep_and_draws_it() {
    let (mut e, top) = nested();
    let key = with_face(&mut e, top, vec![display(OSC, 12.0, 72.0)])[0];
    let mut h = H::with(e);
    assert!(h.ui.composites.open.is_empty(), "every composite is closed");
    // The Output module's meter, outside the composite, asks for its own tap.
    assert!(h.tapped().contains(&OSC), "{:?}", h.tapped());
    let drawn = h.hit(&format!("composite:{top}:display:{key}"));
    assert!(drawn.width() > 0.0 && drawn.height() > 0.0);
    // At the smallest zoom the display still draws, in its smaller footprint.
    h.ui.zoom = kabl_ui::MIN_ZOOM;
    h.frames(3);
    let small = h.hit(&format!("composite:{top}:display:{key}"));
    assert!(small.width() < drawn.width() && small.height() > 0.0);
}

#[test]
fn past_the_tap_budget_the_extra_displays_draw_without_a_tap() {
    let mut e = PatchEditor::seed_from(&kabl_standalone::default_patch());
    let lfos: Vec<ModuleId> = (0..20)
        .map(|i| {
            e.add_module(
                "lfo",
                kabl_core::Vec2 {
                    x: 40.0 * i as f32,
                    y: 400.0,
                },
            )
        })
        .collect();
    let id = composites::encapsulate(
        &mut e,
        lfos.iter().copied().collect(),
        Default::default(),
        "Many",
    )
    .unwrap();
    let placements = lfos
        .iter()
        .enumerate()
        .map(|(i, &m)| {
            display(
                m,
                12.0 + 126.0 * (i % 6) as f32,
                66.0 + 66.0 * (i / 6) as f32,
            )
        })
        .take(18)
        .collect();
    let keys = with_face(&mut e, id, placements);
    let h = H::with(e);
    assert_eq!(h.tapped().len(), kabl_engine::facetap::MAX_FACE_TAPS);
    for key in keys {
        h.hit(&format!("composite:{id}:display:{key}"));
    }
}

#[test]
fn removing_the_source_module_drops_its_display_and_the_face_still_draws() {
    let (mut e, top) = nested();
    let key = with_face(&mut e, top, vec![display(OSC, 12.0, 72.0)])[0];
    let mut h = H::with(e);
    h.editor.remove_module(OSC);
    h.frames(3);
    let panel = h.editor.state().composites[&top].panel.clone();
    assert!(panel.is_none_or(|p| !p.placements.contains_key(&key)));
    assert!(!h
        .ui
        .hits
        .contains_key(&format!("composite:{top}:display:{key}")));
}
