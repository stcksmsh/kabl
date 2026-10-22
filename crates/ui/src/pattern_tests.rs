use crate::pattern::playing_step;
use crate::{PatchEditor, UiState};

fn reference() -> PatchEditor {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../patches/functional-cables/morph-arc");
    PatchEditor::from_log(kabl_core::load(&dir).expect("patch"))
}

#[test]
fn the_playhead_follows_the_clock_only_while_it_runs() {
    let e = reference();
    let clock = *e
        .state()
        .modules
        .iter()
        .find(|(_, m)| m.kind == "clock")
        .map(|(id, _)| id)
        .expect("a clock");
    let mut ui = UiState::default();
    assert_eq!(playing_step(&e, &ui, 4), None, "no report, no playhead");
    ui.clock_pos.insert(clock, 6.4);
    assert_eq!(
        playing_step(&e, &ui, 4),
        Some(2),
        "pulse 6 plays step 6 % 4"
    );
    assert_eq!(playing_step(&e, &ui, 0), None, "no pattern, no playhead");
    ui.clock_running.insert(clock, false);
    assert_eq!(playing_step(&e, &ui, 4), None, "a stopped clock shows none");
}
