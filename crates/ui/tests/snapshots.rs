//! Snapshot recall (FACES.md section 3.2): one undo step that sets the changed controls, an
//! engine morph through `control::deliver` when the snapshot has a length and a clock, a jump
//! whenever the frame cannot morph, and the face's snapshot buttons and strip.
use basedrop::Collector;
use egui::{Event, PointerButton, Pos2, RawInput, Rect};
use kabl_core::{
    composite::Snapshot,
    panel::{Kind, Panel, Placement, Source},
    CompositeId, ModuleId, PortRef, Vec2,
};
use kabl_engine::morph::MorphPart;
use kabl_engine::patch_engine::Command;
use kabl_engine::runtime::{param_class, Feedback, ParamClass, RuntimeTarget, ToAudio};
use kabl_modules::registry;
use kabl_ui::control::{self, Delivery, Outcome, MAX_HELD, MAX_HELD_ACTIONS};
use kabl_ui::{composites, panels, routing, show, snapshots, PatchEditor, UiState};
use std::sync::Arc;

const SR: f32 = 48000.0;
const CLOCK: ModuleId = 1;
const OSC: ModuleId = 10;
const FILTER: ModuleId = 11;
const ENV: ModuleId = 12;
const MIXER: ModuleId = 32;

struct Fix {
    editor: PatchEditor,
    lead: CompositeId,
    cutoff: u64,
    resonance: u64,
    decay: u64,
}

/// The composition piece with "Lead" around its first voice's oscillator, filter and envelope,
/// exposing cutoff, resonance and decay.
fn fixture() -> Fix {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/composition");
    let mut editor = PatchEditor::from_log(kabl_core::load(&dir).expect("patch"));
    let lead = composites::encapsulate(
        &mut editor,
        [OSC, FILTER, ENV].into(),
        Default::default(),
        "Lead",
    )
    .unwrap();
    let mut keys = Vec::new();
    for (id, param) in [
        (FILTER, "cutoff_hz"),
        (FILTER, "resonance"),
        (ENV, "decay_ms"),
    ] {
        let target = PortRef::Param {
            id,
            param: param.into(),
        };
        composites::add_exposure(&mut editor, lead, target.clone(), true).unwrap();
        let c = &editor.state().composites[&lead];
        keys.push(
            *c.controls
                .iter()
                .find(|(_, e)| e.target == target)
                .unwrap()
                .0,
        );
    }
    Fix {
        editor,
        lead,
        cutoff: keys[0],
        resonance: keys[1],
        decay: keys[2],
    }
}

/// Adds a snapshot of `values` to composite `id`; its id.
fn add_snapshot(
    e: &mut PatchEditor,
    id: CompositeId,
    name: &str,
    values: &[(u64, f32)],
    morph_beats: f32,
    clock: Option<ModuleId>,
) -> u64 {
    let mut c = e.state().composites[&id].clone();
    let snap = c.next_interface;
    c.next_interface += 1;
    c.snapshots.push(Snapshot {
        id: snap,
        name: name.into(),
        values: values.iter().copied().collect(),
        morph_beats,
        clock,
    });
    composites::set(e, id, c).unwrap();
    snap
}

fn value(e: &PatchEditor, id: ModuleId, param: &str) -> f32 {
    let s = e.state();
    let info = registry::info_for(&s.modules[&id].kind).unwrap();
    let p = info.params.iter().find(|p| p.name == param).unwrap();
    routing::base_value(s, id, p)
}

fn on_filter(t: &RuntimeTarget) -> bool {
    matches!(t, RuntimeTarget::Param { id: FILTER, .. })
}

#[test]
fn recall_is_one_undo_step_that_sets_only_the_controls_that_change() {
    let mut f = fixture();
    let resonance = value(&f.editor, FILTER, "resonance");
    let snap = add_snapshot(
        &mut f.editor,
        f.lead,
        "Dark",
        &[
            (f.cutoff, 800.0),
            (f.resonance, resonance),
            (f.decay, 400.0),
        ],
        0.0,
        None,
    );
    let (before, entries) = (f.editor.state().clone(), f.editor.log().entries().len());
    let mut ui = UiState::default();
    assert!(snapshots::recall(&mut f.editor, &mut ui, f.lead, snap));
    assert_eq!(f.editor.log().entries().len(), entries + 1);
    let kabl_core::Op::Group { ops } = &f.editor.log().entries().last().unwrap().op else {
        panic!("not a group");
    };
    // Resonance already holds its value: no op for it.
    let set: Vec<(String, f32)> = ops
        .iter()
        .map(|op| match op {
            kabl_core::Op::SetParam {
                target: kabl_core::ParamTarget::Module { param, .. },
                value,
            } => (param.clone(), *value),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        set,
        vec![("cutoff_hz".into(), 800.0), ("decay_ms".into(), 400.0)]
    );
    assert_eq!(value(&f.editor, FILTER, "cutoff_hz"), 800.0);
    assert_eq!(ui.last_snapshot.get(&f.lead), Some(&snap));
    // A jump queues no morph.
    assert!(ui.morphs.is_empty());
    assert!(f.editor.undo());
    assert_eq!(f.editor.state(), &before);
    // Recalled again with nothing to change: no undo step, still lit.
    assert!(f.editor.redo());
    let entries = f.editor.log().entries().len();
    ui.last_snapshot.clear();
    assert!(snapshots::recall(&mut f.editor, &mut ui, f.lead, snap));
    assert_eq!(f.editor.log().entries().len(), entries);
    assert_eq!(ui.last_snapshot.get(&f.lead), Some(&snap));
}

#[test]
fn recall_clamps_through_the_taper() {
    let mut f = fixture();
    let snap = add_snapshot(&mut f.editor, f.lead, "Wild", &[(f.cutoff, 1e9)], 0.0, None);
    snapshots::recall(&mut f.editor, &mut UiState::default(), f.lead, snap);
    let info = registry::info_for("filter.svf").unwrap();
    let max = info
        .params
        .iter()
        .find(|p| p.name == "cutoff_hz")
        .unwrap()
        .max;
    assert_eq!(value(&f.editor, FILTER, "cutoff_hz"), max);
}

#[test]
fn a_control_un_exposed_after_capture_is_not_touched_by_a_recall() {
    let mut f = fixture();
    let snap = add_snapshot(
        &mut f.editor,
        f.lead,
        "Dark",
        &[(f.cutoff, 800.0), (f.decay, 400.0)],
        0.0,
        None,
    );
    composites::remove_exposure(&mut f.editor, f.lead, f.cutoff, true).unwrap();
    let cutoff = value(&f.editor, FILTER, "cutoff_hz");
    assert_ne!(cutoff, 800.0);
    snapshots::recall(&mut f.editor, &mut UiState::default(), f.lead, snap);
    assert_eq!(value(&f.editor, FILTER, "cutoff_hz"), cutoff);
    assert_eq!(value(&f.editor, ENV, "decay_ms"), 400.0);
}

/// The app's control side with the audio queue's consumer held by the test.
struct Audio {
    d: Delivery,
    rx: rtrb::Consumer<ToAudio>,
    _collector: Collector,
}

fn audio(f: &Fix) -> Audio {
    let collector = Collector::new();
    let (tx, rx) = rtrb::RingBuffer::new(control::QUEUE);
    let d = Delivery::new(
        Some(tx),
        collector.handle(),
        Arc::new(Feedback::default()),
        SR,
        8,
        Some(f.editor.state()),
    );
    Audio {
        d,
        rx,
        _collector: collector,
    }
}

impl Audio {
    fn drain(&mut self) -> Vec<ToAudio> {
        std::iter::from_fn(|| self.rx.pop().ok()).collect()
    }
}

fn parts(sent: &[ToAudio]) -> Vec<MorphPart> {
    sent.iter()
        .filter_map(|m| match m {
            ToAudio::Command(Command::Morph(p)) => Some(*p),
            _ => None,
        })
        .collect()
}

fn filter_sets(sent: &[ToAudio]) -> usize {
    sent.iter()
        .filter(|m| matches!(m, ToAudio::Set(s) if on_filter(&s.target)))
        .count()
}

/// A morphing snapshot of cutoff and resonance over 2 beats on the clock.
fn morphing(f: &mut Fix) -> u64 {
    add_snapshot(
        &mut f.editor,
        f.lead,
        "Open",
        &[(f.cutoff, 3000.0), (f.resonance, 0.7)],
        2.0,
        Some(CLOCK),
    )
}

#[test]
fn a_morphing_recall_sends_other_values_then_its_parts_and_never_a_value_for_its_targets() {
    let mut f = fixture();
    let snap = morphing(&mut f);
    let mut a = audio(&f);
    let mut ui = UiState::default();
    control::deliver(&mut f.editor, &mut ui, &mut a.d);
    a.drain();
    let (cutoff, resonance) = (
        value(&f.editor, FILTER, "cutoff_hz"),
        value(&f.editor, FILTER, "resonance"),
    );
    // A hand on the mixer in the same frame as the recall.
    f.editor.set_param(MIXER, "level2", 0.2);
    snapshots::recall(&mut f.editor, &mut ui, f.lead, snap);
    assert_eq!(ui.morphs.len(), 1);
    assert!(matches!(
        control::deliver(&mut f.editor, &mut ui, &mut a.d),
        Outcome::Values(_)
    ));
    assert!(ui.morphs.is_empty());
    let sent = a.drain();
    let first_part = sent
        .iter()
        .position(|m| matches!(m, ToAudio::Command(Command::Morph(_))))
        .expect("a part");
    assert!(sent[..first_part]
        .iter()
        .any(|m| matches!(m, ToAudio::Set(s) if matches!(s.target, RuntimeTarget::Param { id: MIXER, .. }))));
    assert!(sent[first_part..]
        .iter()
        .all(|m| matches!(m, ToAudio::Command(Command::Morph(_)))));
    assert_eq!(filter_sets(&sent), 0);
    let p = parts(&sent);
    assert_eq!(p.len(), 1);
    assert!(p[0].last);
    assert_eq!((p[0].clock, p[0].pulses, p[0].count), (CLOCK, 8.0, 2));
    let mut moves: Vec<(f32, f32)> = p[0].targets[..2]
        .iter()
        .inspect(|t| assert!(on_filter(&t.target)))
        .map(|t| (t.from, t.to))
        .collect();
    moves.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(moves, vec![(resonance, 0.7), (cutoff, 3000.0)]);
    // Later frames diff against a document that already holds the targets.
    f.editor.set_param(MIXER, "level2", 0.3);
    control::deliver(&mut f.editor, &mut ui, &mut a.d);
    let sent = a.drain();
    assert_eq!(filter_sets(&sent), 0);
    assert_eq!(sent.len(), 1);
}

/// Recalls the morphing snapshot after `live` reported on the cutoff, then undoes it; the
/// cutoff morph's start.
fn cutoff_from(f: &mut Fix, snap: u64, live: Option<(f32, std::time::Instant)>) -> f32 {
    let mut ui = UiState::default();
    snapshots::recall(&mut f.editor, &mut ui, f.lead, snap);
    let cutoff = |ui: &UiState| {
        *ui.morphs[0].parts[0]
            .targets
            .iter()
            .find(|t| t.to == 3000.0)
            .unwrap()
    };
    let target = cutoff(&ui).target;
    assert!(f.editor.undo());
    let mut ui = UiState::default();
    if let Some((progress, at)) = live {
        ui.accept_morph(target, 1000.0, 2000.0, progress, at);
    }
    snapshots::recall(&mut f.editor, &mut ui, f.lead, snap);
    let from = cutoff(&ui).from;
    assert!(f.editor.undo());
    from
}

#[test]
fn a_recall_during_a_running_morph_starts_from_the_live_engine_value() {
    let mut f = fixture();
    let snap = morphing(&mut f);
    let stored = value(&f.editor, FILTER, "cutoff_hz");
    assert_ne!(stored, 1000.0);
    let now = std::time::Instant::now();
    assert_eq!(cutoff_from(&mut f, snap, None), stored);
    assert_eq!(cutoff_from(&mut f, snap, Some((0.5, now))), 1000.0);
    // An arrived morph, or a report that stopped coming, starts from the document.
    assert_eq!(cutoff_from(&mut f, snap, Some((1.0, now))), stored);
    let stale = now - std::time::Duration::from_secs(1);
    assert_eq!(cutoff_from(&mut f, snap, Some((0.5, stale))), stored);
}

#[test]
fn a_recall_without_a_clock_or_length_jumps() {
    let mut f = fixture();
    for (beats, clock) in [(0.0, Some(CLOCK)), (2.0, None)] {
        let snap = add_snapshot(
            &mut f.editor,
            f.lead,
            "Jump",
            &[(f.cutoff, 2000.0 + beats)],
            beats,
            clock,
        );
        let mut ui = UiState::default();
        snapshots::recall(&mut f.editor, &mut ui, f.lead, snap);
        assert!(ui.morphs.is_empty());
    }
}

/// Recalls the morphing snapshot after `before` set the frame up, delivers, and checks the
/// recall jumped: the values reach the audio thread (as `Set`s or in a graph) and no part does.
fn assert_jumps(before: impl FnOnce(&mut Fix, &mut Audio, &mut UiState), want: Outcome) {
    let mut f = fixture();
    let snap = morphing(&mut f);
    let mut a = audio(&f);
    let mut ui = UiState::default();
    control::deliver(&mut f.editor, &mut ui, &mut a.d);
    a.drain();
    before(&mut f, &mut a, &mut ui);
    snapshots::recall(&mut f.editor, &mut ui, f.lead, snap);
    assert_eq!(ui.morphs.len(), 1);
    assert_eq!(control::deliver(&mut f.editor, &mut ui, &mut a.d), want);
    assert!(ui.morphs.is_empty());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut sent = Vec::new();
    loop {
        sent.extend(a.drain());
        if a.d.waiting() == 0 || std::time::Instant::now() > deadline {
            break;
        }
        a.d.flush();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(a.d.waiting(), 0);
    assert!(parts(&sent).is_empty(), "a part was queued");
    let graph = sent.iter().rev().find_map(|m| match m {
        ToAudio::Graph(g) => Some(g),
        _ => None,
    });
    match graph {
        Some(g) => {
            let info = registry::info_for("filter.svf").unwrap();
            let i = info
                .params
                .iter()
                .position(|p| p.name == "cutoff_hz")
                .unwrap();
            // The graph keeps the value as knob travel: a few ulps off on the way back.
            let v = g.param_value(FILTER, i).unwrap();
            assert!((v - 3000.0).abs() < 0.01, "{v}");
        }
        None => assert_eq!(filter_sets(&sent), 2),
    }
}

#[test]
fn a_recall_in_the_frame_of_a_load_jumps() {
    assert_jumps(|_, _, ui| ui.loaded = true, Outcome::Compiled);
}

#[test]
fn a_recall_while_a_compile_is_in_flight_jumps() {
    assert_jumps(
        |f, a, ui| {
            a.d.async_compile = true;
            a.d.compile_delay_ms = 300;
            f.editor.add_module("lfo", Vec2 { x: 0.0, y: 0.0 });
            assert_eq!(
                control::deliver(&mut f.editor, ui, &mut a.d),
                Outcome::Compiled
            );
        },
        Outcome::Compiled,
    );
}

#[test]
fn a_recall_while_a_compile_waits_jumps() {
    assert_jumps(
        |f, a, ui| {
            a.d.async_compile = true;
            a.d.compile_delay_ms = 300;
            for x in [0.0, 40.0] {
                f.editor.add_module("lfo", Vec2 { x, y: 0.0 });
                assert_eq!(
                    control::deliver(&mut f.editor, ui, &mut a.d),
                    Outcome::Compiled
                );
            }
        },
        Outcome::Compiled,
    );
}

#[test]
fn a_recall_in_the_frame_of_the_max_held_fallback_jumps() {
    assert_jumps(
        |f, a, ui| {
            // Enough sequencers that one edit of all their runtime values passes `MAX_HELD`.
            let added: Vec<ModuleId> = (0..40)
                .map(|i| {
                    f.editor.add_module(
                        "seq",
                        Vec2 {
                            x: 40.0 * i as f32,
                            y: 900.0,
                        },
                    )
                })
                .collect();
            control::deliver(&mut f.editor, ui, &mut a.d);
            a.drain();
            let mut n = 0;
            let mut ops = Vec::new();
            for id in added {
                let info = registry::info_for("seq").unwrap();
                for p in info.params {
                    if param_class("seq", p.name) == ParamClass::Structural {
                        continue;
                    }
                    let v = p.from_norm(0.37);
                    if v != routing::base_value(f.editor.state(), id, p) {
                        n += 1;
                        ops.push(kabl_core::Op::SetParam {
                            target: kabl_core::ParamTarget::Module {
                                id,
                                param: p.name.into(),
                            },
                            value: v,
                        });
                    }
                }
            }
            assert!(n > MAX_HELD, "{n} values");
            f.editor.edit(ops);
        },
        Outcome::Compiled,
    );
}

#[test]
fn a_recall_without_room_for_its_parts_jumps() {
    assert_jumps(
        |_, a, _| {
            for _ in 0..MAX_HELD_ACTIONS {
                assert!(a.d.command(Command::Cancel(None)));
            }
        },
        Outcome::Values(2),
    );
}

/// An egui run of the whole UI, as `face_buttons.rs` drives it.
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
            drop(self.ctx.end_pass());
        }
    }

    fn click(&mut self, key: &str) {
        let p = self
            .ui
            .hits
            .get(key)
            .unwrap_or_else(|| panic!("no hit rect {key}"))
            .center();
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
}

/// `elements` on the face of `id`, each under a fresh key, in a row; the keys.
fn place(
    e: &mut PatchEditor,
    id: CompositeId,
    elements: Vec<(Kind, Option<Source>, f32)>,
) -> Vec<u64> {
    let mut c = e.state().composites[&id].clone();
    let mut panel = c.panel.as_deref().cloned().unwrap_or(Panel {
        width: 840.0,
        ..Panel::default()
    });
    let mut x = 12.0;
    let keys = elements
        .into_iter()
        .map(|(kind, source, width)| {
            let key = c.next_interface;
            c.next_interface += 1;
            panel.placements.insert(
                key,
                Placement {
                    kind,
                    x,
                    y: 72.0,
                    width,
                    height: 48.0,
                    source,
                    label: None,
                    icons: Vec::new(),
                },
            );
            x += width + 6.0;
            key
        })
        .collect();
    c.panel = Some(Box::new(panel));
    composites::set(e, id, c).unwrap();
    keys
}

#[test]
fn snapshot_buttons_and_the_strip_recall_and_light_the_last_recalled() {
    let mut f = fixture();
    let dark = add_snapshot(
        &mut f.editor,
        f.lead,
        "Dark",
        &[(f.cutoff, 800.0)],
        0.0,
        None,
    );
    let open = add_snapshot(
        &mut f.editor,
        f.lead,
        "Open",
        &[(f.cutoff, 3000.0)],
        0.0,
        None,
    );
    let keys = place(
        &mut f.editor,
        f.lead,
        vec![
            (Kind::Button, Some(Source::Snapshot { id: dark }), 132.0),
            (Kind::Snapshots, None, 270.0),
        ],
    );
    let (button, strip) = (keys[0], keys[1]);
    let lead = f.lead;
    let state = f.editor.state().clone();
    let face = &state.composites[&lead].panel.as_ref().unwrap().placements[&button];
    assert_eq!(panels::face_button_name(&state, lead, face), "Dark");
    let mut h = H::with(f.editor);
    let lit = |h: &H| {
        let p = &h.editor.state().composites[&lead]
            .panel
            .as_ref()
            .unwrap()
            .placements[&button];
        panels::lit(&h.ui, h.editor.state(), lead, p).0
    };
    assert!(!lit(&h));
    h.click(&format!("composite:{lead}:snapshot:{strip}.{open}"));
    assert_eq!(value(&h.editor, FILTER, "cutoff_hz"), 3000.0);
    assert_eq!(h.ui.last_snapshot.get(&lead), Some(&open));
    assert!(!lit(&h));
    h.click(&format!("composite:{lead}:button:{button}"));
    assert_eq!(value(&h.editor, FILTER, "cutoff_hz"), 800.0);
    assert_eq!(h.ui.last_snapshot.get(&lead), Some(&dark));
    assert!(lit(&h));
}

#[test]
fn a_nested_button_chain_ending_on_a_snapshot_recalls_it() {
    let mut f = fixture();
    let dark = add_snapshot(
        &mut f.editor,
        f.lead,
        "Dark",
        &[(f.cutoff, 800.0)],
        0.0,
        None,
    );
    let inner = place(
        &mut f.editor,
        f.lead,
        vec![(Kind::Button, Some(Source::Snapshot { id: dark }), 132.0)],
    )[0];
    let outer =
        composites::encapsulate(&mut f.editor, [CLOCK].into(), [f.lead].into(), "Outer").unwrap();
    let top = place(
        &mut f.editor,
        outer,
        vec![(
            Kind::Button,
            Some(Source::Face {
                composite: f.lead,
                key: inner,
            }),
            132.0,
        )],
    )[0];
    assert_eq!(
        panels::resolve(f.editor.state(), outer, top),
        Some(panels::End::Snapshot(f.lead, dark))
    );
    let state = f.editor.state().clone();
    let p = &state.composites[&outer].panel.as_ref().unwrap().placements[&top];
    assert_eq!(panels::face_button_name(&state, outer, p), "Dark");
    let mut h = H::with(f.editor);
    h.click(&format!("composite:{outer}:button:{top}"));
    assert_eq!(value(&h.editor, FILTER, "cutoff_hz"), 800.0);
    assert_eq!(h.ui.last_snapshot.get(&f.lead), Some(&dark));
    let p = &h.editor.state().composites[&outer]
        .panel
        .as_ref()
        .unwrap()
        .placements[&top];
    assert!(panels::lit(&h.ui, h.editor.state(), outer, p).0);
}

/// The piece played through the real control path and engine: one second, then a recall of the
/// morphing snapshot, then three seconds. The stereo audio and, once per beat/8 from the recall
/// on, the filter cutoff the engine plays.
fn morph_render() -> (Vec<f32>, Vec<(f32, f32)>) {
    use kabl_engine::graph::BLOCK;
    use kabl_engine::patch_engine::PatchEngine;
    let mut f = fixture();
    let snap = morphing(&mut f);
    let mut collector = Collector::new();
    let mut engine = PatchEngine::new(&collector.handle(), f.editor.state(), SR, 8).unwrap();
    engine.active_mut().rev = 1;
    engine.active_mut().generation = 1;
    let (tx, mut rx) = rtrb::RingBuffer::new(control::QUEUE);
    let fb = Arc::new(Feedback::default());
    let mut d = Delivery::new(
        Some(tx),
        collector.handle(),
        fb.clone(),
        SR,
        8,
        Some(f.editor.state()),
    );
    let mut ui = UiState::default();
    let i = registry::info_for("filter.svf")
        .unwrap()
        .params
        .iter()
        .position(|p| p.name == "cutoff_hz")
        .unwrap();
    let bpm = value(&f.editor, CLOCK, "bpm");
    let per_eighth_beat = (60.0 / bpm / 8.0 * SR) as usize;
    let (mut audio, mut series) = (Vec::new(), Vec::new());
    let total = 4 * SR as usize / BLOCK;
    let recall_at = SR as usize / BLOCK;
    let mut next = 0;
    for block in 0..total {
        if block == recall_at {
            snapshots::recall(&mut f.editor, &mut ui, f.lead, snap);
            assert_eq!(ui.morphs.len(), 1);
        }
        control::deliver(&mut f.editor, &mut ui, &mut d);
        engine.drain(&mut rx, control::QUEUE, &fb);
        let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
        engine.process_block(&mut l, &mut r);
        audio.extend(l.iter().zip(&r).flat_map(|(&l, &r)| [l, r]));
        if block >= recall_at {
            let at = (block + 1 - recall_at) * BLOCK;
            if at >= next {
                let beats = at as f32 / SR * bpm / 60.0;
                series.push((beats, engine.active_mut().param_value(FILTER, i).unwrap()));
                next += per_eighth_beat;
            }
        }
        collector.collect();
    }
    (audio, series)
}

/// Also the evidence render: with `KABL_SNAPSHOT_WAV` set, writes the audio there; the cutoff
/// series prints with `--nocapture`.
#[test]
fn a_morphing_recall_moves_the_engine_over_its_beats() {
    let (audio, series) = morph_render();
    for (beats, cutoff) in series.iter().take_while(|s| s.0 <= 3.0) {
        println!("{beats:.3} beats  cutoff {cutoff:.1} Hz");
    }
    if let Some(path) = std::env::var_os("KABL_SNAPSHOT_WAV") {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: SR as u32,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        for s in audio {
            w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16)
                .unwrap();
        }
        w.finalize().unwrap();
    }
    let at = |beats: f32| {
        series
            .iter()
            .min_by(|a, b| (a.0 - beats).abs().total_cmp(&(b.0 - beats).abs()))
            .unwrap()
            .1
    };
    let (start, mid, end) = (at(0.0), at(1.0), at(2.5));
    assert!((start - 330.0).abs() < 30.0, "{series:?}");
    assert!(mid > 500.0 && mid < 2500.0, "{series:?}");
    assert!((end - 3000.0).abs() < 0.5, "{series:?}");
    assert!(
        series.windows(2).all(|w| w[1].1 >= w[0].1 - 1e-3),
        "{series:?}"
    );
}
