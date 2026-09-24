//! Section cues in the editor. A cue lives on a `cues` module as presentation params (never
//! audio): `cue<N>.on`, `cue<N>.clock` (reference clock id), `cue<N>.timing` (absent = next
//! bar), `cue<N>.seq.<id>` (bank per sequencer; absent = that sequencer keeps playing), and a
//! label `cue<N>` for its name. Editing a cue is an ordinary undoable edit; launching one is a
//! runtime command. See docs/composition-batch/design.md.

use kabl_core::{ModuleId, Op, ParamTarget, PatchState};
use kabl_engine::patch_engine::{Command, Launch, Timing, MAX_TARGETS};
use kabl_modules::builtins::CUES;

use crate::banks::TIMINGS;
use crate::PatchEditor;

#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    /// 1..=8.
    pub n: usize,
    pub name: String,
    pub clock: Option<ModuleId>,
    pub timing: Timing,
    /// (sequencer, bank) for sequencers that still exist.
    pub targets: Vec<(ModuleId, usize)>,
}

fn key(n: usize, rest: &str) -> String {
    format!("cue{n}.{rest}")
}

pub fn label_key(n: usize) -> String {
    format!("cue{n}")
}

fn set(id: ModuleId, param: String, value: Option<f32>) -> Op {
    let target = ParamTarget::Module { id, param };
    match value {
        Some(value) => Op::SetParam { target, value },
        None => Op::UnsetParam { target },
    }
}

/// The cues defined on `cues` module `id`, in order.
pub fn cues(state: &PatchState, id: ModuleId) -> Vec<Cue> {
    let Some(m) = state.modules.get(&id) else {
        return Vec::new();
    };
    let exists = |c: ModuleId, kind: &str| state.modules.get(&c).is_some_and(|m| m.kind == kind);
    (1..=CUES)
        .filter(|&n| m.params.contains_key(&key(n, "on")))
        .map(|n| {
            let prefix = key(n, "seq.");
            let targets = m
                .params
                .range(prefix.clone()..)
                .take_while(|(k, _)| k.starts_with(&prefix))
                .filter_map(|(k, &v)| {
                    let seq: ModuleId = k[prefix.len()..].parse().ok()?;
                    exists(seq, "seq").then_some((seq, v.round().clamp(0.0, 3.0) as usize))
                })
                .collect();
            Cue {
                n,
                name: state
                    .label(id, &label_key(n))
                    .map_or_else(|| format!("Cue {n}"), str::to_string),
                clock: m
                    .params
                    .get(&key(n, "clock"))
                    .map(|&v| v as ModuleId)
                    .filter(|&c| exists(c, "clock")),
                timing: m
                    .params
                    .get(&key(n, "timing"))
                    .and_then(|&v| TIMINGS.get(v.round() as usize))
                    .map_or(Timing::NextBar, |t| t.0),
                targets,
            }
        })
        .collect()
}

/// Adds the first free cue, named, on the first clock, holding what every sequencer plays now
/// (`playing`: sequencer → bank). One undo step. `None` when all eight exist.
pub fn add(
    editor: &mut PatchEditor,
    id: ModuleId,
    playing: &dyn Fn(ModuleId) -> usize,
) -> Option<usize> {
    let state = editor.state();
    let m = state.modules.get(&id)?;
    let n = (1..=CUES).find(|&n| !m.params.contains_key(&key(n, "on")))?;
    let mut ops = vec![set(id, key(n, "on"), Some(1.0))];
    if let Some((&clock, _)) = state.modules.iter().find(|(_, m)| m.kind == "clock") {
        ops.push(set(id, key(n, "clock"), Some(clock as f32)));
    }
    for (&s, _) in state.modules.iter().filter(|(_, m)| m.kind == "seq") {
        ops.push(set(
            id,
            key(n, &format!("seq.{s}")),
            Some(playing(s) as f32),
        ));
    }
    ops.push(Op::SetLabel {
        id,
        key: label_key(n),
        text: Some(format!("Cue {n}")),
    });
    editor.edit(ops);
    Some(n)
}

/// Removes cue `n` (params and name). One undo step.
pub fn delete(editor: &mut PatchEditor, id: ModuleId, n: usize) {
    let Some(m) = editor.state().modules.get(&id) else {
        return;
    };
    let prefix = format!("cue{n}.");
    let mut ops: Vec<Op> = m
        .params
        .keys()
        .filter(|k| k.starts_with(&prefix))
        .map(|k| set(id, k.clone(), None))
        .collect();
    ops.push(Op::SetLabel {
        id,
        key: label_key(n),
        text: None,
    });
    editor.edit(ops);
}

/// Bank for sequencer `seq` in cue `n`, or `None` = keep playing.
pub fn set_target(
    editor: &mut PatchEditor,
    id: ModuleId,
    n: usize,
    seq: ModuleId,
    bank: Option<usize>,
) {
    editor.edit(vec![set(
        id,
        key(n, &format!("seq.{seq}")),
        bank.map(|b| b as f32),
    )]);
}

pub fn set_clock(editor: &mut PatchEditor, id: ModuleId, n: usize, clock: ModuleId) {
    editor.edit(vec![set(id, key(n, "clock"), Some(clock as f32))]);
}

pub fn set_timing(editor: &mut PatchEditor, id: ModuleId, n: usize, timing: Timing) {
    let i = TIMINGS.iter().position(|t| t.0 == timing).unwrap_or(2);
    editor.edit(vec![set(
        id,
        key(n, "timing"),
        (i != 2).then_some(i as f32),
    )]);
}

pub fn rename(editor: &mut PatchEditor, id: ModuleId, n: usize, text: &str) {
    let text = text.trim();
    let text = if text.is_empty() {
        format!("Cue {n}")
    } else {
        text.to_string()
    };
    editor.set_label(id, &label_key(n), Some(text));
}

/// Queues the launch of cue `n`. Errors on a cue with no clock or no sequencer.
pub fn launch(
    out: &mut Vec<Command>,
    state: &PatchState,
    id: ModuleId,
    n: usize,
) -> Result<(), String> {
    let cue = cues(state, id)
        .into_iter()
        .find(|c| c.n == n)
        .ok_or("no such cue")?;
    let clock = cue
        .clock
        .ok_or_else(|| format!("{}: choose a reference clock first", cue.name))?;
    if cue.targets.is_empty() {
        return Err(format!("{}: no sequencer is set", cue.name));
    }
    let targets: Vec<(ModuleId, u8)> = cue
        .targets
        .iter()
        .take(MAX_TARGETS)
        .map(|&(s, b)| (s, b as u8))
        .collect();
    out.push(Command::Launch(Launch::new(clock, cue.timing, &targets)));
    Ok(())
}

/// How cue `n` stands against what the sequencers report (`bank_of`: playing, queued):
/// `(active, queued)`. Active: every target plays its bank with nothing else queued. Queued:
/// once what is queued lands, every target will play its bank, and one of them is still
/// waiting (a cue sharing only some banks with the queued one is not shown as queued).
pub fn standing(cue: &Cue, bank_of: &dyn Fn(ModuleId) -> (usize, Option<usize>)) -> (bool, bool) {
    if cue.targets.is_empty() {
        return (false, false);
    }
    let lands = |&(s, b): &(ModuleId, usize)| {
        let (playing, queued) = bank_of(s);
        queued.unwrap_or(playing) == b
    };
    let waiting = cue.targets.iter().any(|&(s, b)| bank_of(s).1 == Some(b));
    let all = cue.targets.iter().all(lands);
    (all && !waiting, all && waiting)
}

/// Ops removing every cue and launch-setting reference to module `removed` (a sequencer's cue
/// entries, a cue's or a sequencer's reference clock), for deleting it in the same undo step.
pub fn references_to(state: &PatchState, removed: ModuleId) -> Vec<Op> {
    let mut ops = Vec::new();
    let seq_suffix = format!(".seq.{removed}");
    for (&id, m) in &state.modules {
        for (k, &v) in &m.params {
            let refers = match m.kind.as_str() {
                "cues" => {
                    k.ends_with(&seq_suffix) || (k.ends_with(".clock") && v as ModuleId == removed)
                }
                "seq" => k == crate::banks::LAUNCH_CLOCK && v as ModuleId == removed,
                _ => false,
            };
            if refers && id != removed {
                ops.push(set(id, k.clone(), None));
            }
        }
    }
    ops
}
