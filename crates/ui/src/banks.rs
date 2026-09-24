//! Sequencer pattern banks in the editor: names, the startup bank, copy and clear (one undo
//! step each), and a sequencer's own launch settings. Launches themselves are runtime commands
//! (`UiState::launches`), never ops. See docs/composition-batch/design.md.

use kabl_core::{ModuleId, Op, ParamTarget, PatchState, PortRef};
use kabl_engine::patch_engine::{Command, Launch, Timing};
use kabl_modules::builtins::seq::{self, BANKS, BANK_NAMES, SLOTS};

use crate::PatchEditor;

/// Label key of bank `b`'s name.
pub fn name_key(b: usize) -> String {
    format!("bank.{}", BANK_NAMES[b].to_lowercase())
}

/// Presentation params of a sequencer's own launches: the reference clock's id and the timing.
pub const LAUNCH_CLOCK: &str = "launch.clock";
pub const LAUNCH_TIMING: &str = "launch.timing";
pub const TIMINGS: [(Timing, &str); 3] = [
    (Timing::Now, "Now"),
    (Timing::NextStep, "Next step"),
    (Timing::NextBar, "Next bar"),
];

/// "B", or "B Verse" when bank B is named.
pub fn title(state: &PatchState, id: ModuleId, b: usize) -> String {
    match state.label(id, &name_key(b)) {
        Some(n) => format!("{} {n}", BANK_NAMES[b]),
        None => BANK_NAMES[b].to_string(),
    }
}

pub fn startup(state: &PatchState, id: ModuleId) -> usize {
    state
        .modules
        .get(&id)
        .and_then(|m| m.params.get("bank"))
        .map_or(0, |v| (v.round().max(0.0) as usize).min(BANKS - 1))
}

fn target(id: ModuleId, param: &str) -> ParamTarget {
    ParamTarget::Module {
        id,
        param: param.to_string(),
    }
}

/// Bank `to` becomes a copy of bank `from` (values stored as they are: a default stays
/// absent), and takes its name. One undo step.
pub fn copy(editor: &mut PatchEditor, id: ModuleId, from: usize, to: usize) {
    let Some(m) = editor.state().modules.get(&id) else {
        return;
    };
    let mut ops = Vec::new();
    for slot in 0..SLOTS {
        let dst = seq::bank_param(to, slot);
        let t = target(id, dst);
        match m.params.get(seq::bank_param(from, slot)) {
            Some(&value) => ops.push(Op::SetParam { target: t, value }),
            None if m.params.contains_key(dst) => ops.push(Op::UnsetParam { target: t }),
            None => {}
        }
    }
    let text = editor
        .state()
        .label(id, &name_key(from))
        .map(str::to_string);
    if editor.state().label(id, &name_key(to)) != text.as_deref() {
        ops.push(Op::SetLabel {
            id,
            key: name_key(to),
            text,
        });
    }
    editor.edit(ops);
}

/// Bank `b` becomes an empty pattern: every step off at 0 st, 100 % velocity and probability,
/// the default length and gate settings. One undo step.
pub fn clear(editor: &mut PatchEditor, id: ModuleId, b: usize) {
    let ops = (0..SLOTS)
        .map(|slot| Op::SetParam {
            target: target(id, seq::bank_param(b, slot)),
            value: seq::cleared(slot),
        })
        .collect();
    editor.edit(ops);
}

pub fn rename(editor: &mut PatchEditor, id: ModuleId, b: usize, text: &str) {
    let text = text.trim();
    editor.set_label(
        id,
        &name_key(b),
        (!text.is_empty()).then(|| text.to_string()),
    );
}

/// The clock whose ticks time this sequencer's own launches: the one chosen in its settings,
/// else the clock its `clock` input comes from (through dividers).
pub fn reference_clock(state: &PatchState, id: ModuleId) -> Option<ModuleId> {
    let is_clock = |c: ModuleId| state.modules.get(&c).is_some_and(|m| m.kind == "clock");
    if let Some(c) = state
        .modules
        .get(&id)
        .and_then(|m| m.params.get(LAUNCH_CLOCK))
        .map(|&v| v as ModuleId)
        .filter(|&c| is_clock(c))
    {
        return Some(c);
    }
    upstream_clock(state, id)
}

/// The clock a module's `clock` input comes from, following dividers.
pub fn upstream_clock(state: &PatchState, mut id: ModuleId) -> Option<ModuleId> {
    for _ in 0..8 {
        let from = state.cables.values().find_map(|c| match (&c.from, &c.to) {
            (PortRef::Module { id: f, .. }, PortRef::Module { id: t, port })
                if *t == id && port == "clock" =>
            {
                Some(*f)
            }
            _ => None,
        })?;
        match state.modules.get(&from).map(|m| m.kind.as_str()) {
            Some("clock") => return Some(from),
            Some("clock.div") => id = from,
            _ => return None,
        }
    }
    None
}

pub fn timing(state: &PatchState, id: ModuleId) -> Timing {
    let i = state
        .modules
        .get(&id)
        .and_then(|m| m.params.get(LAUNCH_TIMING))
        .map_or(2, |&v| v.round() as usize);
    TIMINGS.get(i).map_or(Timing::NextBar, |t| t.0)
}

/// Queues a launch of bank `b` on sequencer `id` with its own settings. Errors when it has no
/// reference clock.
pub fn launch(
    out: &mut Vec<Command>,
    state: &PatchState,
    id: ModuleId,
    b: usize,
) -> Result<(), String> {
    let clock = reference_clock(state, id)
        .ok_or("no reference clock: patch a clock into this sequencer or choose one")?;
    out.push(Command::Launch(Launch::new(
        clock,
        timing(state, id),
        &[(id, b as u8)],
    )));
    Ok(())
}
