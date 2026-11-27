//! Whole blocks on a composite face (cue pads, banks, transport) and "Pin to Perform" from a
//! composite face control.
use egui::{Event, PointerButton, Pos2, RawInput, Rect};
use kabl_core::{
    panel::{Kind, Panel, Placement, Source},
    CompositeId, ModuleId, PortRef, Vec2,
};
use kabl_engine::patch_engine::Command;
use kabl_modules::builtins::Transport;
use kabl_ui::{composites, cues, design_page, perform, show, PatchEditor, UiState};
use std::collections::{BTreeMap, BTreeSet};

/// The performance patch's clock and bass sequencer.
const CLOCK: u64 = 1;
const BASS: u64 = 3;

fn block(kind: Kind, leaf: ModuleId, x: f32, y: f32, width: f32, height: f32) -> Placement {
    Placement {
        kind,
        x,
        y,
        width,
        height,
        source: Some(Source::Module {
            id: leaf,
            action: None,
        }),
        label: None,
        icons: Vec::new(),
    }
}

struct Fix {
    editor: PatchEditor,
    /// Holds the clock, the cues module and `inner`.
    outer: CompositeId,
    /// Holds the bass sequencer.
    inner: CompositeId,
    cues: ModuleId,
    /// Outer's control exposing the bass sequencer's `gate_len` as "Bass gate".
    gate: u64,
}

/// The performance patch with a cues module whose cue 1 sets the bass to bank B. "Inner" holds
/// the bass, "Outer" the clock, the cues module and "Inner". Outer's face shows a "Bass gate"
/// knob and the cue pads, banks and transport blocks.
fn fixture() -> Fix {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/performance");
    let mut e = PatchEditor::from_log(kabl_core::load(&dir).expect("patch"));
    let cue_module = e.add_module("cues", Vec2 { x: 30.0, y: 10.0 });
    let n = cues::add(&mut e, cue_module, &|_| 0).unwrap();
    cues::set_target(&mut e, cue_module, n, BASS, Some(1));
    let inner = composites::encapsulate(&mut e, [BASS].into(), BTreeSet::new(), "Inner").unwrap();
    let outer =
        composites::encapsulate(&mut e, [CLOCK, cue_module].into(), [inner].into(), "Outer")
            .unwrap();
    let target = PortRef::Param {
        id: BASS,
        param: "gate_len".into(),
    };
    composites::add_exposure(&mut e, outer, target.clone(), true).unwrap();
    let mut c = e.state().composites[&outer].clone();
    let gate = *c
        .controls
        .iter()
        .find(|(_, x)| x.target == target)
        .unwrap()
        .0;
    c.controls.get_mut(&gate).unwrap().label = "Bass gate".into();
    let mut p = Panel {
        width: 840.0,
        ..Panel::default()
    };
    p.placements.insert(
        gate,
        Placement {
            source: None,
            ..block(Kind::Knob, 0, 12.0, 72.0, 84.0, 110.0)
        },
    );
    for (kind, leaf, x, y, w, h) in [
        (Kind::Transport, CLOCK, 108.0, 72.0, 240.0, 36.0),
        (Kind::Banks, BASS, 108.0, 120.0, 372.0, 18.0),
        (Kind::CuePads, cue_module, 492.0, 72.0, 336.0, 150.0),
    ] {
        p.placements
            .insert(c.next_interface, block(kind, leaf, x, y, w, h));
        c.next_interface += 1;
    }
    c.panel = Some(Box::new(p));
    composites::set(&mut e, outer, c).unwrap();
    Fix {
        editor: e,
        outer,
        inner,
        cues: cue_module,
        gate,
    }
}

const SIZE: egui::Vec2 = egui::vec2(1440.0, 900.0);

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    t: f64,
    events: Vec<Event>,
}

impl H {
    fn with(editor: PatchEditor) -> Self {
        let mut h = H {
            ctx: egui::Context::default(),
            editor,
            ui: UiState::default(),
            t: 0.0,
            events: Vec::new(),
        };
        h.frames(3);
        h
    }

    fn begin(&mut self) -> egui::Ui {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)),
            time: Some(self.t),
            events: std::mem::take(&mut self.events),
            ..Default::default()
        };
        self.t += 1.0 / 60.0;
        self.ctx.begin_pass(raw);
        egui::Ui::new(
            self.ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::ZERO, SIZE)),
        )
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            let mut root = self.begin();
            show(&mut self.editor, &mut self.ui, &mut root);
            drop(self.ctx.end_pass());
        }
    }

    fn hit(&self, key: &str) -> Rect {
        *self
            .ui
            .hits
            .get(key)
            .unwrap_or_else(|| panic!("no hit rect {key}"))
    }

    fn press(&mut self, key: &str, button: PointerButton) {
        let p = self.hit(key).center();
        self.events.push(Event::PointerMoved(p));
        self.frames(1);
        for pressed in [true, false] {
            self.events.push(Event::PointerButton {
                pos: p,
                button,
                pressed,
                modifiers: Default::default(),
            });
            self.frames(1);
        }
        self.frames(2);
    }

    fn click(&mut self, key: &str) {
        self.press(key, PointerButton::Primary);
    }

    /// Right-clicks `key`, then picks "Pin to Perform" in the menu that opens.
    fn pin(&mut self, key: &str) {
        self.ui.hits.remove("menu:face-pin");
        self.press(key, PointerButton::Secondary);
        self.click("menu:face-pin");
    }

    /// The one cue or bank launch queued since the last call: its clock and targets.
    fn launched(&mut self) -> (ModuleId, Vec<(ModuleId, u8)>) {
        let launches: Vec<_> = self
            .ui
            .launches
            .drain(..)
            .filter_map(|c| match c {
                Command::Launch(l) => Some(l),
                _ => None,
            })
            .collect();
        match launches.as_slice() {
            [l] => (l.clock, l.targets[..l.count].to_vec()),
            other => panic!("{other:?}"),
        }
    }
}

fn gate_pins(state: &kabl_core::PatchState) -> Vec<perform::Pin> {
    perform::pins(state)
        .into_iter()
        .filter(|p| p.id == BASS && p.key == "gate_len")
        .collect()
}

#[test]
fn block_hit_keys_stay_unique_with_the_module_faces_drawn_in_the_same_pass() {
    let f = fixture();
    let (outer, cues) = (f.outer, f.cues);
    let mut h = H::with(f.editor);
    // A second view inside the composites, where the leaves' own faces draw.
    let mut inside = UiState::default();
    std::mem::swap(&mut h.ui, &mut inside);
    h.frames(3);
    h.click(&format!("composite:{outer}:open"));
    std::mem::swap(&mut h.ui, &mut inside);
    // One pass draws both views.
    let mut root = h.begin();
    show(&mut h.editor, &mut h.ui, &mut root);
    show(&mut h.editor, &mut inside, &mut root);
    drop(h.ctx.end_pass());
    let p = format!("composite:{outer}:");
    for key in [
        format!("{p}run:{CLOCK}"),
        format!("{p}restart:{CLOCK}"),
        format!("{p}bank-play:{BASS}.1"),
        format!("{p}cue:{cues}.1"),
    ] {
        assert!(h.ui.hits.contains_key(&key), "{key} on the closed face");
    }
    for key in [format!("run:{CLOCK}"), format!("cue:{cues}.1")] {
        assert!(
            inside.hits.contains_key(&key),
            "{key} on the module's own face"
        );
    }
    // No block key of one view is a key of the other.
    let blocks = |hits: &BTreeMap<String, Rect>| -> BTreeSet<String> {
        hits.keys()
            .filter(|k| {
                let k = k.strip_prefix(&p).unwrap_or(k);
                ["run:", "restart:", "bank-", "cue:", "cue-cancel:"]
                    .iter()
                    .any(|b| k.starts_with(b))
            })
            .cloned()
            .collect()
    };
    let (face, own) = (blocks(&h.ui.hits), blocks(&inside.hits));
    assert!(face.len() >= 4 && own.len() >= 2);
    assert!(
        face.is_disjoint(&own),
        "{:?}",
        face.intersection(&own).collect::<Vec<_>>()
    );
}

#[test]
fn a_block_whose_module_is_gone_draws_nothing_and_does_not_panic() {
    let f = fixture();
    let outer = f.outer;
    let mut h = H::with(f.editor);
    h.editor.remove_module(BASS);
    h.frames(2);
    let p = format!("composite:{outer}:");
    assert!(!h
        .ui
        .hits
        .keys()
        .any(|k| k.starts_with(&format!("{p}bank-"))));
    assert!(h.ui.hits.contains_key(&format!("{p}run:{CLOCK}")));
}

#[test]
fn a_cue_pad_on_the_face_queues_that_cue() {
    let f = fixture();
    let (outer, cues) = (f.outer, f.cues);
    let mut h = H::with(f.editor);
    h.click(&format!("composite:{outer}:cue:{cues}.1"));
    let (clock, targets) = h.launched();
    assert_eq!(clock, CLOCK);
    assert!(targets.contains(&(BASS, 1)), "{targets:?}");
}

#[test]
fn a_bank_on_the_face_queues_that_bank() {
    let f = fixture();
    let outer = f.outer;
    let mut h = H::with(f.editor);
    h.click(&format!("composite:{outer}:bank-play:{BASS}.2"));
    assert_eq!(h.launched(), (CLOCK, vec![(BASS, 2)]));
}

#[test]
fn transport_on_the_face_queues_run_stop_and_restart() {
    let f = fixture();
    let outer = f.outer;
    let mut h = H::with(f.editor);
    let entries = h.editor.log().entries().len();
    h.click(&format!("composite:{outer}:run:{CLOCK}"));
    h.click(&format!("composite:{outer}:restart:{CLOCK}"));
    assert_eq!(
        h.ui.transport,
        [(CLOCK, Transport::Stop), (CLOCK, Transport::Restart)]
    );
    assert_eq!(h.editor.log().entries().len(), entries, "no op-log entries");
}

#[test]
fn the_palette_offers_each_leaf_block_and_places_one_in_one_draft_step() {
    let f = fixture();
    let (outer, cues) = (f.outer, f.cues);
    let mut h = H::with(f.editor);
    let listed: Vec<_> = design_page::blocks(h.editor.state(), outer)
        .into_iter()
        .map(|(kind, leaf, _)| (kind, leaf))
        .collect();
    assert_eq!(
        listed,
        [
            (Kind::Transport, CLOCK),
            (Kind::Banks, BASS),
            (Kind::CuePads, cues)
        ]
    );
    // Inner's face may show its own leaves' blocks only.
    let inner: Vec<_> = design_page::blocks(h.editor.state(), f.inner)
        .into_iter()
        .map(|(kind, _, _)| kind)
        .collect();
    assert_eq!(inner, [Kind::Banks]);
    let mut c = h.editor.state().composites[&f.inner].clone();
    // Wide enough for the banks row; a narrower face says to widen it, as for any element.
    c.panel = Some(Box::new(Panel {
        width: 840.0,
        ..Panel::default()
    }));
    composites::set(&mut h.editor, f.inner, c.clone()).unwrap();
    kabl_ui::panels::open(&mut h.ui, f.inner, &c);
    h.frames(2);
    let page = |h: &H| h.ui.panels.page.as_ref().unwrap().draft.clone();
    let before = page(&h);
    h.click(&format!("design:block:{BASS}.Banks"));
    let after = page(&h);
    assert_eq!(after.panel.placements.len(), 1);
    let (_, p) = after.panel.placements.iter().next().unwrap();
    assert_eq!(p.kind, Kind::Banks);
    assert_eq!(
        p.source,
        Some(Source::Module {
            id: BASS,
            action: None
        })
    );
    // Placing it again selects it instead.
    h.click(&format!("design:block:{BASS}.Banks"));
    assert_eq!(page(&h), after);
    h.click("design:undo");
    assert_eq!(page(&h), before);
}

#[test]
fn right_click_pins_a_nested_leaf_param_under_the_exposure_label() {
    let f = fixture();
    let (outer, gate) = (f.outer, f.gate);
    let mut h = H::with(f.editor);
    let control = format!("composite:{outer}:control:{gate}");
    assert!(gate_pins(h.editor.state()).is_empty());
    let entries = h.editor.log().entries().len();
    h.pin(&control);
    let pins = gate_pins(h.editor.state());
    assert_eq!(pins.len(), 1);
    assert_eq!(perform::pin_label(h.editor.state(), &pins[0]), "Bass gate");
    assert_eq!(h.editor.log().entries().len(), entries + 1, "one undo step");
    assert!(h.ui.perform_open);
    // The menu offers it again checked; picking it unpins rather than pinning twice.
    h.pin(&control);
    assert!(gate_pins(h.editor.state()).is_empty());
}

#[test]
fn pin_source_names_the_composite_path() {
    let f = fixture();
    let mut h = H::with(f.editor);
    h.pin(&format!("composite:{}:control:{}", f.outer, f.gate));
    let state = h.editor.state();
    let pin = &gate_pins(state)[0];
    let mut flat = state.clone();
    flat.composites.clear();
    assert_eq!(
        perform::pin_source(state, pin),
        format!("Outer / Inner · {}", perform::pin_source(&flat, pin))
    );
    assert!(perform::pin_source(&flat, pin).starts_with(&format!("Sequencer #{BASS} · ")));
}

#[test]
fn removing_the_exposure_leaves_the_pin_on_the_leaf() {
    let f = fixture();
    let (outer, gate) = (f.outer, f.gate);
    let mut h = H::with(f.editor);
    h.pin(&format!("composite:{outer}:control:{gate}"));
    composites::remove_exposure(&mut h.editor, outer, gate, true).unwrap();
    h.frames(2);
    let pins = gate_pins(h.editor.state());
    assert_eq!(pins.len(), 1);
    assert_eq!(perform::pin_label(h.editor.state(), &pins[0]), "Bass gate");
}

#[test]
fn pin_is_not_offered_on_blocks() {
    let f = fixture();
    let (outer, cues) = (f.outer, f.cues);
    let mut h = H::with(f.editor);
    h.ui.hits.remove("menu:face-pin");
    h.press(
        &format!("composite:{outer}:cue:{cues}.1"),
        PointerButton::Secondary,
    );
    assert!(!h.ui.hits.contains_key("menu:face-pin"));
}
