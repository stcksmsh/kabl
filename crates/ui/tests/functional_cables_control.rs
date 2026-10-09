//! Functional-cable edits on the production control path (editor -> `control::deliver` -> the
//! real queue -> `PatchEngine`): the first pattern compiles a graph, later edits play in place.

use std::sync::Arc;

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::Collector;
use kabl_core::{CableId, ParamTarget};
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::PatchEngine;
use kabl_engine::runtime::Feedback;
use kabl_ui::control::{self, Delivery, Outcome};
use kabl_ui::{PatchEditor, UiState};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;
/// A jack cable of `patches/composition` (a clock drives its sequencers).
const CABLE: CableId = 7;

fn set(editor: &mut PatchEditor, name: &str, v: f32) {
    editor.set_param_gesture(
        ParamTarget::Cable {
            id: CABLE,
            param: name.into(),
        },
        v,
        true,
    );
}

#[test]
fn first_pattern_compiles_then_edits_play_in_place() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/composition");
    let mut editor = PatchEditor::from_log(kabl_core::load(&dir).unwrap());
    let collector = Collector::new();
    let mut engine = PatchEngine::new(&collector.handle(), editor.state(), SR, 8).unwrap();
    engine.active_mut().rev = 1;
    let (tx, mut rx) = rtrb::RingBuffer::new(control::QUEUE);
    let fb = Arc::new(Feedback::default());
    let mut d = Delivery::new(
        Some(tx),
        collector.handle(),
        fb.clone(),
        SR,
        8,
        Some(editor.state()),
    );
    let mut ui = UiState::default();
    let mut audio = |engine: &mut PatchEngine, d: &mut Delivery, blocks: usize| {
        for _ in 0..4 {
            d.flush();
            assert_no_alloc(|| {
                engine.drain(&mut rx, control::QUEUE, &fb);
                let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
                for _ in 0..blocks {
                    engine.process_block(&mut l, &mut r);
                }
            });
        }
    };
    audio(&mut engine, &mut d, 8);

    set(&mut editor, "length", 4.0);
    assert_eq!(
        control::deliver(&mut editor, &mut ui, &mut d),
        Outcome::Compiled
    );
    audio(&mut engine, &mut d, 400);
    let graphs = d.counts.graphs;

    for (k, name) in ["s2", "s3", "r1", "prob"].into_iter().enumerate() {
        set(&mut editor, name, 0.5 + k as f32 * 0.1);
        assert!(matches!(
            control::deliver(&mut editor, &mut ui, &mut d),
            Outcome::Values(1)
        ));
        audio(&mut engine, &mut d, 8);
    }
    assert_eq!(d.counts.graphs, graphs, "pattern edits build no graph");
    assert_eq!(Feedback::get(&fb.unresolved), 0, "every value found its cable node");

    assert!(editor.undo());
    assert!(matches!(
        control::deliver(&mut editor, &mut ui, &mut d),
        Outcome::Values(_)
    ));
}
