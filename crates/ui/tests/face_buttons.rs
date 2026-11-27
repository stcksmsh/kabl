//! Composite face buttons: what the design page offers, pressing a closed face's button (a cue,
//! a chain into nested composites, a clock under the host clock), lit state, broken chains and
//! Apply after a nested composite lost a button the draft presses.
use egui::{Event, PointerButton, Pos2, RawInput, Rect};
use kabl_core::{
    panel::{Kind, Panel, Placement, Source},
    CompositeId, ModuleId, PatchState, Vec2,
};
use kabl_engine::patch_engine::Command;
use kabl_modules::builtins::{seq::BANKS, Transport};
use kabl_ui::{banks, composites, cues, design_page, help, panels, show, PatchEditor, UiState};

/// The performance patch's clock and its two sequencers.
const CLOCK: u64 = 1;
const BASS: u64 = 3;
const ARP: u64 = 4;

fn module(id: ModuleId, action: &str) -> Source {
    Source::Module {
        id,
        action: Some(action.into()),
    }
}

fn face(composite: CompositeId, key: u64) -> Source {
    Source::Face { composite, key }
}

fn placement(state: &PatchState, id: CompositeId, key: u64) -> &Placement {
    &state.composites[&id].panel.as_ref().unwrap().placements[&key]
}

/// `sources` as buttons on the face of `id`, after the elements already there, each under a
/// fresh key; the new keys in order.
fn add_buttons(e: &mut PatchEditor, id: CompositeId, sources: Vec<Source>) -> Vec<u64> {
    let mut c = e.state().composites[&id].clone();
    let mut panel = c.panel.as_deref().cloned().unwrap_or(Panel {
        width: 840.0,
        ..Panel::default()
    });
    let keys = sources
        .into_iter()
        .map(|source| {
            let (key, i) = (c.next_interface, panel.placements.len());
            c.next_interface += 1;
            panel.placements.insert(
                key,
                Placement {
                    kind: Kind::Button,
                    x: 12.0 + 138.0 * (i % 4) as f32,
                    y: 72.0 + 54.0 * (i / 4) as f32,
                    width: 132.0,
                    height: 48.0,
                    source: Some(source),
                    label: None,
                    icons: Vec::new(),
                },
            );
            key
        })
        .collect();
    c.panel = Some(Box::new(panel));
    composites::set(e, id, c).unwrap();
    keys
}

/// The keys of the buttons `fixture` puts on each face.
struct Keys {
    /// Deep: cue 1, arp bank B.
    deep: [u64; 2],
    /// Mid: Deep's two buttons.
    mid: [u64; 2],
    /// Top: cue 1, Mid's cue button, bass bank B, Mid's bank button, Run, Deep's cue button.
    top: [u64; 6],
}

/// The performance patch with a cues module whose cue 1, "Intro", sets both sequencers to bank B.
/// "Top" holds the clock and "Mid", which holds the bass and "Deep", which holds the arp and the
/// cues module. Returns the editor, (Top, Mid, Deep), the cues module and the button keys.
fn fixture() -> (PatchEditor, [CompositeId; 3], ModuleId, Keys) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/performance");
    let mut e = PatchEditor::from_log(kabl_core::load(&dir).expect("patch"));
    let cue_module = e.add_module("cues", Vec2 { x: 30.0, y: 10.0 });
    let n = cues::add(&mut e, cue_module, &|_| 0).unwrap();
    assert_eq!(n, 1);
    cues::rename(&mut e, cue_module, n, "Intro");
    cues::set_target(&mut e, cue_module, n, BASS, Some(1));
    cues::set_target(&mut e, cue_module, n, ARP, Some(1));
    let deep =
        composites::encapsulate(&mut e, [ARP, cue_module].into(), Default::default(), "Deep")
            .unwrap();
    let mid = composites::encapsulate(&mut e, [BASS].into(), [deep].into(), "Mid").unwrap();
    let top = composites::encapsulate(&mut e, [CLOCK].into(), [mid].into(), "Top").unwrap();
    let d = add_buttons(
        &mut e,
        deep,
        vec![module(cue_module, "cue.1"), module(ARP, "bank.1")],
    );
    let m = add_buttons(&mut e, mid, vec![face(deep, d[0]), face(deep, d[1])]);
    let t = add_buttons(
        &mut e,
        top,
        vec![
            module(cue_module, "cue.1"),
            face(mid, m[0]),
            module(BASS, "bank.1"),
            face(mid, m[1]),
            module(CLOCK, "run"),
            face(deep, d[0]),
        ],
    );
    let keys = Keys {
        deep: [d[0], d[1]],
        mid: [m[0], m[1]],
        top: t.try_into().unwrap(),
    };
    (e, [top, mid, deep], cue_module, keys)
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
            // Nothing is painted: the tests read the state, hit rects and runtime commands.
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

    /// The one cue launch queued since the last call, with its targets.
    fn launched(&mut self) -> Vec<(ModuleId, u8)> {
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
            [l] => {
                assert_eq!(l.clock, CLOCK);
                l.targets[..l.count].to_vec()
            }
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn the_design_page_lists_every_leaf_action_and_the_buttons_of_nested_faces() {
    let (e, [top, mid, deep], cue_module, k) = fixture();
    let state = e.state();
    let title = |kind: &str, id| help::module_title(kind, id);
    let bank_buttons = |seq: ModuleId, at: &str| {
        (0..BANKS)
            .map(|b| {
                (
                    module(seq, &format!("bank.{b}")),
                    format!(
                        "{at}{} · Launch bank {}",
                        title("seq", seq),
                        banks::title(state, seq, b)
                    ),
                )
            })
            .collect::<Vec<_>>()
    };
    let cues_name = format!("Mid / Deep / {}", title("cues", cue_module));
    let arp_bank_b = banks::title(state, ARP, 1);
    let mut want = vec![
        (
            module(CLOCK, "run"),
            format!("{} · Run / Stop", title("clock", CLOCK)),
        ),
        (
            module(CLOCK, "restart"),
            format!("{} · Restart", title("clock", CLOCK)),
        ),
    ];
    want.extend(bank_buttons(BASS, "Mid / "));
    want.extend(bank_buttons(ARP, "Mid / Deep / "));
    want.extend([
        (
            module(cue_module, "cue.1"),
            format!("{cues_name} · Launch Intro"),
        ),
        (
            module(cue_module, "cancel"),
            format!("{cues_name} · Cancel queued launches"),
        ),
        (face(deep, k.deep[0]), "Mid / Deep / Launch Intro".into()),
        (
            face(deep, k.deep[1]),
            format!("Mid / Deep / Launch bank {arp_bank_b}"),
        ),
        (face(mid, k.mid[0]), "Mid / Launch Intro".into()),
        (
            face(mid, k.mid[1]),
            format!("Mid / Launch bank {arp_bank_b}"),
        ),
    ]);
    assert!(deep < mid, "the list walks nested composites in id order");
    assert_eq!(design_page::buttons(state, top), want);

    // The page shows the list; a click places a button pressing a nested face's button.
    let mut h = H::with(e);
    let c = h.editor.state().composites[&top].clone();
    panels::open(&mut h.ui, top, &c);
    h.frames(2);
    h.hit(&format!("design:button:{CLOCK}.run"));
    h.hit(&format!("design:button:{cue_module}.cue.1"));
    h.hit(&format!("design:button:face:{deep}.{}", k.deep[1]));
    let restart = module(CLOCK, "restart");
    h.click(&format!("design:button:{CLOCK}.restart"));
    let page = h.ui.panels.page.as_ref().expect("the design page is open");
    assert_eq!(page.note, None);
    let key = page
        .button_of(&restart)
        .expect("the restart button is placed");
    assert_eq!(key, c.next_interface);
    // The page's preview names the draft's button, which is not in the document yet.
    let draft = &page.draft.panel.placements[&key];
    assert_eq!(panels::button_name(h.editor.state(), draft), "Restart");
    h.click("design:apply");
    let applied = &h.editor.state().composites[&top];
    let p = &applied.panel.as_ref().unwrap().placements[&key];
    assert_eq!((p.kind, p.source.as_ref()), (Kind::Button, Some(&restart)));
    panels::validate(h.editor.state()).unwrap();
}

#[test]
fn pressing_a_cue_button_on_a_closed_face_queues_that_cue() {
    let (e, [top, ..], cue_module, k) = fixture();
    let mut h = H::with(e);
    assert!(h.ui.composites.open.is_empty(), "every composite is closed");
    h.ui.launches.clear();
    h.click(&format!("composite:{top}:button:{}", k.top[0]));
    assert_eq!(h.launched(), vec![(BASS, 1), (ARP, 1)]);
    assert_eq!(h.ui.last_cue.get(&cue_module), Some(&1));
    assert_eq!(h.ui.last_message, None);

    // At the smallest zoom the pad's hit rect still fires.
    h.ui.zoom = kabl_ui::MIN_ZOOM;
    h.frames(2);
    h.click(&format!("composite:{top}:button:{}", k.top[0]));
    assert_eq!(h.launched(), vec![(BASS, 1), (ARP, 1)]);
}

#[test]
fn a_button_two_composites_down_fires_the_innermost_action() {
    let (e, [top, ..], cue_module, k) = fixture();
    for key in [k.top[1], k.top[5]] {
        assert_eq!(
            panels::resolve(e.state(), top, key),
            Some((cue_module, "cue.1".into()))
        );
    }
    let mut h = H::with(e);
    h.ui.launches.clear();
    // Top -> Mid -> Deep's cue button, and Top straight to Deep's.
    for key in [k.top[1], k.top[5]] {
        h.click(&format!("composite:{top}:button:{key}"));
        assert_eq!(h.launched(), vec![(BASS, 1), (ARP, 1)]);
    }
    // Top -> Mid -> Deep's arp bank button launches bank B on the arp alone.
    h.ui.launches.clear();
    h.click(&format!("composite:{top}:button:{}", k.top[3]));
    assert_eq!(h.launched(), vec![(ARP, 1)]);
}

#[test]
fn a_clock_button_under_the_host_clock_is_refused_as_on_the_module_face() {
    let (e, [top, ..], _, k) = fixture();
    let mut h = H::with(e);
    h.ui.host_clock = true;
    h.ui.transport.clear();
    h.click(&format!("composite:{top}:button:{}", k.top[4]));
    assert!(h.ui.transport.is_empty());
    assert_eq!(
        h.ui.last_message.as_deref(),
        Some("Host clock: use REAPER transport or switch to Free")
    );
    h.ui.host_clock = false;
    h.click(&format!("composite:{top}:button:{}", k.top[4]));
    assert_eq!(h.ui.transport, vec![(CLOCK, Transport::Toggle)]);
}

#[test]
fn lit_state_follows_the_cue_and_the_bank_directly_and_through_a_chain() {
    let (e, [top, ..], _, k) = fixture();
    let mut view = UiState::default();
    let lit = |view: &UiState, key| panels::lit(view, e.state(), placement(e.state(), top, key));
    let [cue, cue_chain, bass_b, arp_b_chain, run, _] = k.top;
    // Both sequencers at their startup bank A: no cue of B, no bank B.
    for key in [cue, cue_chain, bass_b, arp_b_chain, run] {
        assert_eq!(lit(&view, key), (false, false), "{key}");
    }
    // Both queued to B: the cue and the banks blink.
    view.seq_banks.insert(BASS, (0, Some(1)));
    view.seq_banks.insert(ARP, (0, Some(1)));
    for key in [cue, cue_chain, bass_b, arp_b_chain] {
        assert_eq!(lit(&view, key), (false, true), "{key}");
    }
    // Landed on B: lit.
    view.seq_banks.insert(BASS, (1, None));
    view.seq_banks.insert(ARP, (1, None));
    for key in [cue, cue_chain, bass_b, arp_b_chain] {
        assert_eq!(lit(&view, key), (true, false), "{key}");
    }
    assert_eq!(lit(&view, run), (false, false));
    // Only the arp on B: the cue is off, the bass bank is off, the arp's chain stays lit.
    view.seq_banks.insert(BASS, (0, None));
    for key in [cue, cue_chain, bass_b] {
        assert_eq!(lit(&view, key), (false, false), "{key}");
    }
    assert_eq!(lit(&view, arp_b_chain), (true, false));
}

#[test]
fn a_chain_whose_nested_button_is_gone_does_nothing_and_does_not_panic() {
    let (e, [top, _, deep], _, k) = fixture();
    // Left dangling in a state that skipped the prune: the chain resolves to nothing.
    let mut dangling = e.state().clone();
    let d = dangling.composites.get_mut(&deep).unwrap();
    d.panel.as_mut().unwrap().placements.remove(&k.deep[0]);
    assert_eq!(panels::resolve(&dangling, top, k.top[1]), None);
    assert_eq!(panels::resolve(&dangling, top, k.top[5]), None);
    assert_eq!(
        panels::lit(
            &UiState::default(),
            &dangling,
            placement(&dangling, top, k.top[1])
        ),
        (false, false)
    );
    assert_eq!(
        panels::button_name(&dangling, placement(&dangling, top, k.top[1])),
        ""
    );

    // In the document, removing the arp prunes Deep's arp button and the chain pressing it from
    // Top, and the face still draws and presses what is left.
    let mut h = H::with(e);
    h.editor.remove_module(ARP);
    h.frames(3);
    let hit = |h: &H, key| {
        h.ui.hits
            .contains_key(&format!("composite:{top}:button:{key}"))
    };
    assert!(!hit(&h, k.top[3]));
    assert!(hit(&h, k.top[1]));
    h.ui.launches.clear();
    h.click(&format!("composite:{top}:button:{}", k.top[1]));
    assert!(h.launched().contains(&(BASS, 1)));
}

#[test]
fn apply_lists_the_buttons_a_nested_composite_lost_and_asks_before_dropping_them() {
    // Top holds the clock and Mid, which holds the bass and a spare, unpatched sequencer whose
    // bank button is on Mid's face; Top's draft presses it.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/performance");
    let mut e = PatchEditor::from_log(kabl_core::load(&dir).expect("patch"));
    let spare = e.add_module("seq", Vec2 { x: 30.0, y: 10.0 });
    let mid = composites::encapsulate(&mut e, [BASS, spare].into(), Default::default(), "Mid");
    let mid = mid.unwrap();
    let top = composites::encapsulate(&mut e, [CLOCK].into(), [mid].into(), "Top").unwrap();
    let extra = add_buttons(&mut e, mid, vec![module(spare, "bank.2")])[0];
    let mut h = H::with(e);
    let c = h.editor.state().composites[&top].clone();
    panels::open(&mut h.ui, top, &c);
    h.frames(2);
    let target = face(mid, extra);
    h.click(&format!("design:button:face:{mid}.{extra}"));
    let page = h.ui.panels.page.as_ref().unwrap();
    assert_eq!(page.note, None);
    let key = page.button_of(&target).expect("Mid's button is placed");
    // Also a placement Apply keeps: the clock's Restart.
    h.click(&format!("design:button:{CLOCK}.restart"));
    let restart =
        h.ui.panels
            .page
            .as_ref()
            .unwrap()
            .button_of(&module(CLOCK, "restart"))
            .unwrap();

    // Removing the spare takes Mid's button with it; Top itself is unchanged.
    h.editor.remove_module(spare);
    let m = &h.editor.state().composites[&mid].panel;
    assert!(m
        .as_ref()
        .is_none_or(|p| !p.placements.contains_key(&extra)));
    assert_eq!(h.editor.state().composites[&top], c);
    h.frames(2);

    h.click("design:apply");
    let page = h.ui.panels.page.as_ref().unwrap();
    assert_eq!(page.ask, Some(design_page::Ask::Drop));
    assert_eq!(
        page.dropping,
        vec![(key, format!("Button · element {key}"))]
    );
    h.hit(&format!("design:dropping:{key}"));
    assert_eq!(h.editor.state().composites[&top], c, "nothing applied yet");

    h.click("design:ask:keep");
    assert_eq!(h.ui.panels.page.as_ref().unwrap().ask, None);
    assert_eq!(h.editor.state().composites[&top], c, "nothing applied");

    h.click("design:apply");
    h.click("design:ask:apply");
    let page = h.ui.panels.page.as_ref().unwrap();
    assert_eq!((page.ask, page.note.as_deref()), (None, None));
    let placements = &h.editor.state().composites[&top]
        .panel
        .as_ref()
        .unwrap()
        .placements;
    assert!(!placements.contains_key(&key));
    assert_eq!(placements[&restart].source, Some(module(CLOCK, "restart")));
    panels::validate(h.editor.state()).unwrap();
}
