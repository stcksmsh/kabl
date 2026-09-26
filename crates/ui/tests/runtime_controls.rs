//! D04 on the production control path, without an editor frame unless a test draws one:
//! `PatchEditor` edits → `control::deliver` / `control::midi` → the real `rtrb` queue →
//! `PatchEngine::drain` → rendered blocks. The dense piece (`patches/composition`) has macros
//! on CCs 20–23, a mixer level on CC 24 and MIDI buttons (Run 46, Restart 47, cues 40–45).
//!
//! Covered: no graph builds for knobs, macros, pins and mapped CCs; a structural edit in the
//! middle of control movement; undo/redo; comparison restore; deletion and recreation; a new
//! document; a paused audio consumer (bounded, converges on the last value); a failed compile;
//! MIDI buttons and pickup with no frame; save and undo after CC edits; probe lineage.

use std::sync::Arc;

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::Collector;
use kabl_core::{ModuleId, ParamTarget, PatchState, PortRef};
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::{Command, PatchEngine};
use kabl_engine::runtime::{Feedback, ToAudio};
use kabl_modules::builtins::Transport;
use kabl_modules::registry;
use kabl_ui::control::{self, Delivery, Outcome, MAX_GRAPHS_QUEUED};
use kabl_ui::{compare, perform, routing, PatchEditor, UiState};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;
const VOICES: usize = 8;
const CLOCK: ModuleId = 1;
const MACRO: ModuleId = 3;
const MIXER: ModuleId = 32;

fn piece() -> kabl_core::PatchLog {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/composition");
    kabl_core::load(&dir).expect("patch")
}

/// The app's pieces, wired as `main.rs` wires them, with the audio callback run by hand.
struct H {
    editor: PatchEditor,
    ui: UiState,
    d: Delivery,
    engine: PatchEngine,
    rx: rtrb::Consumer<ToAudio>,
    commands: rtrb::Consumer<Command>,
    transport: rtrb::Consumer<(ModuleId, Transport)>,
    fb: Arc<Feedback>,
    collector: Collector,
    /// Steady clock for the control layer, seconds.
    t: f64,
}

impl H {
    fn new() -> Self {
        let collector = Collector::new();
        let editor = PatchEditor::from_log(piece());
        let mut engine = PatchEngine::new(&collector.handle(), editor.state(), SR, VOICES).unwrap();
        engine.active_mut().rev = 1;
        engine.active_mut().generation = 1;
        let (tx, rx) = rtrb::RingBuffer::new(control::QUEUE);
        let (ctx, commands) = rtrb::RingBuffer::new(64);
        let (ttx, transport) = rtrb::RingBuffer::new(64);
        let fb = Arc::new(Feedback::default());
        let d = Delivery::new(
            Some(tx),
            Some(ctx),
            Some(ttx),
            collector.handle(),
            fb.clone(),
            SR,
            VOICES,
            Some(editor.state()),
        );
        let mut h = H {
            editor,
            ui: UiState::default(),
            d,
            engine,
            rx,
            commands,
            transport,
            fb,
            collector,
            t: 10.0,
        };
        perform::rearm(&mut h.ui, 0.0);
        h
    }

    /// One audio callback: what `main.rs` does before rendering, then `blocks` blocks. Every
    /// call in every test runs with allocation (and freeing) forbidden.
    fn callback(&mut self, blocks: usize) {
        let H {
            engine,
            rx,
            transport,
            commands,
            fb,
            ..
        } = self;
        assert_no_alloc(|| {
            engine.drain(rx, control::QUEUE, fb);
            while let Ok((id, t)) = transport.pop() {
                engine.transport(id, t);
            }
            while let Ok(c) = commands.pop() {
                engine.command(&c);
            }
            let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
            for _ in 0..blocks {
                engine.process_block(&mut l, &mut r);
            }
        });
    }

    fn settle(&mut self) {
        for _ in 0..8 {
            self.d.flush();
            self.callback(8);
        }
        self.collector.collect();
    }

    fn deliver(&mut self) -> Outcome {
        control::deliver(&mut self.editor, &mut self.ui, &mut self.d)
    }

    /// CC messages on channel 1, through the control thread's entry point (no frame).
    fn cc(&mut self, events: &[(u8, u8)]) -> Outcome {
        self.t += 0.01;
        let ev: Vec<(u8, u8, u8)> = events.iter().map(|&(c, v)| (0, c, v)).collect();
        control::midi(&mut self.editor, &mut self.ui, &mut self.d, &ev, self.t)
    }

    fn drag(&mut self, id: ModuleId, param: &str, value: f32, first: bool) -> Outcome {
        self.editor.set_param_gesture(
            ParamTarget::Module {
                id,
                param: param.into(),
            },
            value,
            first,
        );
        self.deliver()
    }

    fn base(&self, id: ModuleId, param: &str) -> f32 {
        let s = self.editor.state();
        let info = registry::info_for(&s.modules[&id].kind).unwrap();
        let p = info.params.iter().find(|p| p.name == param).unwrap();
        routing::base_value(s, id, p)
    }

    /// Every param value of every module the playing graph has equals the document's.
    fn assert_engine_matches_document(&mut self) {
        let doc = self.editor.state().clone();
        let g = self.engine.active_mut();
        for (&id, m) in &doc.modules {
            let info = registry::info_for(&m.kind).unwrap();
            for (i, p) in info.params.iter().enumerate() {
                let want = routing::base_value(&doc, id, p);
                if let Some(got) = g.param_value(id, i) {
                    assert!(
                        (got - want).abs() <= 1e-4 * want.abs().max(1.0),
                        "#{id} {} engine {got} document {want}",
                        p.name
                    );
                }
            }
        }
        for (&cid, c) in &doc.cables {
            if let (PortRef::Param { .. }, Some(got)) = (&c.to, g.route_amount(cid)) {
                let want = c.params.get("amount").copied().unwrap_or(0.25);
                assert!((got - want).abs() < 1e-5, "route {cid}: {got} vs {want}");
            }
        }
    }

    fn graphs(&self) -> u64 {
        self.d.counts.graphs
    }
}

#[test]
fn routine_controls_never_build_a_graph() {
    let mut h = H::new();
    h.settle();
    // A rack knob drag (the mixer level), a Perform macro (m2), a direct pin (seq transpose):
    // 30 frames each.
    for k in 0..30 {
        let first = k == 0;
        assert!(matches!(
            h.drag(MIXER, "level1", 0.2 + k as f32 * 0.01, first),
            Outcome::Values(1)
        ));
        h.drag(MACRO, "m2", k as f32 / 30.0, first);
        h.callback(4);
    }
    h.drag(6, "transpose", 5.0, true);
    // Mapped CCs: macro m1 on CC 20, first reaching the stored value (pickup), then moving.
    let at = (h.base(MACRO, "m1") * 127.0).round() as u8;
    for v in at..at.saturating_add(40).min(127) {
        h.cc(&[(20, v)]);
        h.callback(2);
    }
    assert!(h.base(MACRO, "m1") > 0.0);
    h.settle();
    assert_eq!(h.graphs(), 0, "no compile for runtime controls");
    assert_eq!(Feedback::get(&h.fb.graphs_taken), 0);
    assert!(h.d.counts.values >= 60);
    assert_eq!(Feedback::get(&h.fb.unresolved), 0);
    assert_eq!(h.d.pending(), 0);
    assert!(!h.engine.is_swapping());
    h.assert_engine_matches_document();
    // The macro moved as one undo step for the CC gesture.
    let before = h.base(MACRO, "m1");
    assert!(h.editor.undo());
    assert_ne!(h.base(MACRO, "m1"), before);
    assert!(matches!(h.deliver(), Outcome::Values(_)));
    h.settle();
    h.assert_engine_matches_document();
    assert_eq!(h.graphs(), 0);
}

#[test]
fn a_structural_edit_while_controls_move_keeps_every_value() {
    let mut h = H::new();
    h.settle();
    let at = (h.base(MACRO, "m1") * 127.0).round() as u8;
    h.cc(&[(20, at)]);
    for k in 0..40u8 {
        h.cc(&[(20, at.saturating_sub(k))]);
        h.drag(MIXER, "level1", 0.1 + k as f32 * 0.01, k == 0);
        if k == 10 {
            // Add a module and patch it: a compile, in the middle of the movement.
            let lfo = h
                .editor
                .add_module("lfo", kabl_core::Vec2 { x: 0.0, y: 0.0 });
            h.editor.connect_route(
                PortRef::Module {
                    id: lfo,
                    port: "out".into(),
                },
                MIXER,
                "level2",
            );
            assert_eq!(h.deliver(), Outcome::Compiled);
        }
        // Delayed audio: the graph waits in the queue while values keep coming behind it.
        if k % 7 == 0 {
            h.callback(3);
        }
    }
    h.settle();
    assert_eq!(h.graphs(), 1);
    assert_eq!(Feedback::get(&h.fb.stale_graphs), 0);
    h.assert_engine_matches_document();
    // Undo the route and the module (compiles), redo them, then a knob (runtime).
    assert!(h.editor.undo()); // the last knob drag
    h.deliver();
    let g = h.graphs();
    while h.editor.can_undo() && h.editor.state().modules.len() > 39 {
        h.editor.undo();
    }
    h.deliver();
    assert_eq!(h.graphs(), g + 1);
    h.editor.redo();
    h.deliver();
    h.settle();
    h.assert_engine_matches_document();
    h.drag(MIXER, "level1", 0.33, true);
    assert_eq!(h.graphs(), g + 2);
    h.settle();
    h.assert_engine_matches_document();
}

#[test]
fn comparison_restore_and_recipe_like_edits_converge_on_the_document() {
    let mut h = H::new();
    h.settle();
    h.ui.compare.capture(&h.editor, "test");
    // Only values changed since the reference: the restore is runtime values.
    h.drag(MIXER, "level1", 0.05, true);
    h.drag(MACRO, "m3", 0.9, true);
    h.settle();
    compare::restore(&mut h.editor, &mut h.ui);
    assert!(matches!(h.deliver(), Outcome::Values(2)));
    h.settle();
    h.assert_engine_matches_document();
    assert_eq!(h.graphs(), 0);
    // Undo returns the user's version, redo the reference: still runtime.
    h.editor.undo();
    assert!(matches!(h.deliver(), Outcome::Values(2)));
    h.editor.redo();
    h.deliver();
    h.settle();
    h.assert_engine_matches_document();
    // A reference with other wiring: the restore compiles.
    let c = *h
        .editor
        .state()
        .cables
        .iter()
        .find(|(_, c)| matches!(c.to, PortRef::Param { .. }))
        .unwrap()
        .0;
    h.editor.disconnect(c);
    h.deliver();
    compare::restore(&mut h.editor, &mut h.ui);
    assert_eq!(h.deliver(), Outcome::Compiled);
    h.settle();
    h.assert_engine_matches_document();
}

#[test]
fn deletion_recreation_and_a_new_document_reject_old_values() {
    let mut h = H::new();
    h.settle();
    // Values for the mixer wait behind a paused audio thread...
    for k in 0..5 {
        h.drag(MIXER, "level1", 0.5 + k as f32 * 0.1, k == 0);
    }
    // ...then the mixer is deleted and brought back (same id, same kind): compiles.
    h.editor.remove_module(MIXER);
    assert_eq!(h.deliver(), Outcome::Compiled);
    h.editor.undo();
    assert_eq!(h.deliver(), Outcome::Compiled);
    h.drag(MIXER, "level1", 0.123, true);
    h.settle();
    h.assert_engine_matches_document();
    assert_eq!(h.engine.active_mut().param_value(MIXER, 0), Some(0.123));
    // A new document (a Load): fresh graph; no value of the old one lands in it.
    h.drag(MIXER, "level1", 0.9, true);
    let log = piece();
    kabl_ui::browser::replace_patch(&mut h.editor, &mut h.ui, log);
    assert_eq!(h.deliver(), Outcome::Compiled);
    h.settle();
    h.assert_engine_matches_document();
    assert_ne!(h.engine.active_mut().param_value(MIXER, 0), Some(0.9));
}

#[test]
fn a_paused_audio_thread_bounds_the_queue_and_gets_the_last_value() {
    let mut h = H::new();
    h.settle();
    // The audio thread stops taking messages. 2000 CC values on four macros and a mixer, and
    // a structural edit every 200 messages.
    let starts: Vec<u8> = ["m1", "m2", "m3", "m4"]
        .iter()
        .map(|m| (h.base(MACRO, m) * 127.0).round() as u8)
        .collect();
    for (k, &s) in starts.iter().enumerate() {
        h.cc(&[(20 + k as u8, s)]);
    }
    let route = *h
        .editor
        .state()
        .cables
        .iter()
        .find(|(_, c)| matches!(c.to, PortRef::Param { .. }))
        .unwrap()
        .0;
    let mut last = 0;
    for i in 0..400u32 {
        let v = (i % 128) as u8;
        h.cc(&[(20, v), (21, v), (22, v), (23, v), (24, v)]);
        last = v;
        if i % 40 == 39 {
            // A runtime value (bpm) and a structural edit (a route's bypass).
            h.editor.set_param(CLOCK, "bpm", 100.0 + i as f32 / 10.0);
            h.editor.set_route_bypass(route, (i / 40) % 2 == 0);
            assert_eq!(h.deliver(), Outcome::Compiled);
        }
        // Bounded: at most MAX_GRAPHS_QUEUED graphs in the queue, and here one graph and one
        // value per target (five CCs, the bpm).
        assert!(h.d.graphs_queued() <= MAX_GRAPHS_QUEUED);
        assert!(h.d.waiting() <= 1 + 6, "waiting {}", h.d.waiting());
    }
    assert!((1..=MAX_GRAPHS_QUEUED).contains(&h.d.graphs_queued()));
    assert_eq!(h.rx.slots(), control::QUEUE, "the queue is full");
    assert!(h.graphs() >= 10);
    assert_eq!(Feedback::get(&h.fb.graphs_taken), 0);
    // The audio thread resumes: bounded work per callback, and every final value arrives.
    let mut callbacks = 0;
    while h.d.waiting() > 0 || !h.rx.is_empty() {
        h.d.flush();
        h.callback(1);
        callbacks += 1;
        assert!(callbacks < 100);
    }
    h.settle();
    assert_eq!(h.base(MACRO, "m4"), last as f32 / 127.0);
    h.assert_engine_matches_document();
    assert!(h.d.counts.coalesced > 0);
    assert!(Feedback::get(&h.fb.graphs_taken) <= h.graphs());
    assert_eq!(h.d.graphs_queued(), 0);
}

#[test]
fn a_failed_compile_keeps_the_graph_and_runtime_values_still_apply() {
    let mut h = H::new();
    h.settle();
    h.editor
        .add_module("no-such-module", kabl_core::Vec2 { x: 0.0, y: 0.0 });
    assert_eq!(h.deliver(), Outcome::Failed);
    assert!(h.d.compile_error.is_some());
    h.settle();
    assert_eq!(
        Feedback::get(&h.fb.graphs_taken),
        0,
        "nothing replaced the graph"
    );
    // A knob on a module the playing graph has still applies; the error stays shown.
    h.drag(MIXER, "level1", 0.42, true);
    h.settle();
    assert_eq!(h.engine.active_mut().param_value(MIXER, 0), Some(0.42));
    assert!(h.d.compile_error.is_some());
    // Undoing the bad module compiles again and clears the error.
    h.editor.undo(); // the knob
    h.editor.undo(); // the module
    assert_eq!(h.deliver(), Outcome::Compiled);
    assert!(h.d.compile_error.is_none());
    h.settle();
    h.assert_engine_matches_document();
}

#[test]
fn midi_buttons_and_pickup_work_without_a_frame_and_the_ui_catches_up() {
    let mut h = H::new();
    h.settle();
    let running = |h: &mut H| {
        let mut r = None;
        h.engine.clocks(|id, run| {
            if id == CLOCK {
                r = Some(run);
            }
        });
        r.unwrap()
    };
    assert!(running(&mut h));
    // Run/Stop (CC 46): one action per press, decided on the audio thread.
    h.cc(&[(46, 127)]);
    h.callback(1);
    assert!(!running(&mut h));
    h.cc(&[(46, 100), (46, 0)]);
    h.callback(1);
    assert!(!running(&mut h), "repeats and the release do nothing");
    h.cc(&[(46, 127)]);
    h.callback(1);
    assert!(running(&mut h));
    // Pickup: the mixer level (CC 24) does not jump to a far hardware position...
    let stored = h.base(MIXER, "level1");
    let far = if stored > 0.5 { 0 } else { 127 };
    assert_eq!(h.cc(&[(24, far)]), Outcome::Nothing);
    assert_eq!(h.base(MIXER, "level1"), stored);
    // ...and follows once it crosses it.
    let near = (stored * 127.0).round() as u8;
    h.cc(&[(24, near)]);
    h.cc(&[(24, near.saturating_add(10).min(127))]);
    h.settle();
    let moved = h.base(MIXER, "level1");
    assert_ne!(moved, stored);
    assert_eq!(h.engine.active_mut().param_value(MIXER, 0), Some(moved));
    assert_eq!(h.graphs(), 0);
    // The UI resumes: a frame replays nothing, the document already has the value; saving
    // writes it; undo takes the CC gesture back as one step, delivered as a value.
    let ctx = egui::Context::default();
    ctx.begin_pass(Default::default());
    let mut root = egui::Ui::new(
        ctx.clone(),
        egui::Id::new("root"),
        egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1440.0, 900.0),
        )),
    );
    kabl_ui::show(&mut h.editor, &mut h.ui, &mut root);
    let _ = ctx.end_pass();
    assert_eq!(h.deliver(), Outcome::Nothing, "the frame replayed nothing");
    assert_eq!(h.base(MIXER, "level1"), moved);
    let dir = tempfile::tempdir().unwrap();
    kabl_core::save(dir.path(), h.editor.log()).unwrap();
    let saved: PatchState = kabl_core::load(dir.path()).unwrap().state().clone();
    assert_eq!(saved.modules[&MIXER].params["level1"], moved);
    h.editor.undo();
    assert_eq!(h.base(MIXER, "level1"), stored);
    assert!(matches!(h.deliver(), Outcome::Values(1)));
    h.settle();
    h.assert_engine_matches_document();
    // A mapping change is presentation: nothing for the audio side.
    h.editor
        .set_presentation(&[(MIXER, "cc.level1".into(), None)]);
    assert_eq!(h.deliver(), Outcome::Nothing);
    assert_eq!(h.cc(&[(24, 0)]), Outcome::Nothing);
}

#[test]
fn inspector_readings_survive_runtime_edits_but_not_rewiring() {
    let mut h = H::new();
    h.settle();
    let g = h.d.generation;
    for k in 0..10 {
        h.drag(MIXER, "level1", k as f32 / 10.0, k == 0);
    }
    assert_eq!(h.d.generation, g, "runtime edits keep the graph lineage");
    let c = *h.editor.state().cables.keys().next().unwrap();
    h.editor.disconnect(c);
    h.deliver();
    assert_eq!(
        h.d.generation,
        g + 1,
        "rewiring is a new graph the inspector hears about"
    );
}
