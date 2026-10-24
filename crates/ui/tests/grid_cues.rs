//! A cue can target a drum grid's pattern: the editor lists it, the launch carries it, the
//! patch saves it as the same `cue<N>.seq.<id>` param a sequencer bank uses.

use kabl_core::Vec2;
use kabl_engine::patch_engine::{Command, Timing};
use kabl_ui::{cues, PatchEditor};

fn rig() -> (PatchEditor, u64, u64) {
    let mut e = PatchEditor::new();
    e.add_module("clock", Vec2::default());
    let grid = e.add_module("drum.grid", Vec2::default());
    let cue = e.add_module("cues", Vec2::default());
    (e, grid, cue)
}

#[test]
fn a_cue_sets_a_grid_pattern_and_launches_it() {
    let (mut e, grid, cue) = rig();
    let n = cues::add(&mut e, cue, &|_| 0).expect("cue");
    cues::set_target(&mut e, cue, n, grid, Some(2));
    let c = &cues::cues(e.state(), cue)[0];
    assert_eq!(c.targets, vec![(grid, 2)]);
    assert_eq!(c.timing, Timing::NextBar);
    let mut out = Vec::new();
    cues::launch(&mut out, e.state(), cue, n).unwrap();
    let [Command::Launch(l)] = &out[..] else {
        panic!("one launch, got {out:?}")
    };
    assert_eq!((l.count, l.targets[0]), (1, (grid, 2)));
}

#[test]
fn a_grid_without_a_target_is_left_alone_and_deleting_it_drops_the_target() {
    let (mut e, grid, cue) = rig();
    let n = cues::add(&mut e, cue, &|_| 0).expect("cue");
    assert!(cues::cues(e.state(), cue)[0].targets.is_empty());
    cues::set_target(&mut e, cue, n, grid, Some(1));
    cues::set_target(&mut e, cue, n, grid, None);
    assert!(cues::cues(e.state(), cue)[0].targets.is_empty());
    cues::set_target(&mut e, cue, n, grid, Some(1));
    let ops = cues::references_to(e.state(), grid);
    assert_eq!(ops.len(), 1);
}
