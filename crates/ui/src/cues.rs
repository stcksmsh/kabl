//! Section cues in the editor. A cue lives on a `cues` module as presentation params (never
//! audio): `cue<N>.on`, `cue<N>.clock` (reference clock id), `cue<N>.timing` (absent = next
//! bar), `cue<N>.seq.<id>` (bank per sequencer; absent = that sequencer keeps playing), and a
//! label `cue<N>` for its name. Editing a cue is an ordinary undoable edit; launching one is a
//! runtime command. See docs/composition-batch/design.md.
//!
//! A cue can also move macros: `cue<N>.macro.<id>.<k>` (macro module `id`, output `k` = 0..7)
//! holds the value that macro heads to, absent = left where it is, and `cue<N>.morph` the
//! morph length in beats (absent or 0 = jump). The engine moves them (`kabl_engine::morph`).

use kabl_core::{ModuleId, Op, ParamTarget, PatchState};
use kabl_engine::morph::{MacroTarget, MAX_MACRO_TARGETS};
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
    /// (macro module, output 0..7, value 0..1) for macros that still exist.
    pub macros: Vec<(ModuleId, u8, f32)>,
    /// Morph length in beats; 0 = jump.
    pub morph_beats: f32,
}

/// Pulses (16ths) in a beat.
pub const PULSES_PER_BEAT: f32 = 4.0;
/// Longest morph the editor offers, in beats (four bars).
pub const MAX_MORPH_BEATS: f32 = 16.0;

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
            let mprefix = key(n, "macro.");
            let macros = m
                .params
                .range(mprefix.clone()..)
                .take_while(|(k, _)| k.starts_with(&mprefix))
                .filter_map(|(k, &v)| {
                    let (module, out) = k[mprefix.len()..].split_once('.')?;
                    let (module, out): (ModuleId, u8) = (module.parse().ok()?, out.parse().ok()?);
                    (out < kabl_modules::builtins::MACROS as u8 && exists(module, "macro"))
                        .then_some((module, out, v.clamp(0.0, 1.0)))
                })
                .collect();
            Cue {
                n,
                macros,
                morph_beats: m
                    .params
                    .get(&key(n, "morph"))
                    .map_or(0.0, |v| v.clamp(0.0, MAX_MORPH_BEATS)),
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

/// Target of macro `k` of module `module` in cue `n`; `None` = the cue leaves it alone.
pub fn set_macro(
    editor: &mut PatchEditor,
    id: ModuleId,
    n: usize,
    module: ModuleId,
    k: u8,
    value: Option<f32>,
) {
    editor.edit(vec![set(
        id,
        key(n, &format!("macro.{module}.{k}")),
        value.map(|v| v.clamp(0.0, 1.0)),
    )]);
}

/// Morph length of cue `n` in beats (0 = jump).
pub fn set_morph(editor: &mut PatchEditor, id: ModuleId, n: usize, beats: f32) {
    let beats = beats.clamp(0.0, MAX_MORPH_BEATS);
    editor.edit(vec![set(
        id,
        key(n, "morph"),
        (beats > 0.0).then_some(beats),
    )]);
}

/// The macros of module `id` a cue offers: the first four, and any further one that is routed,
/// pinned to Perform or named, so an unused `m5`..`m8` does not crowd the cue.
pub fn active_macros(state: &PatchState, id: ModuleId) -> Vec<u8> {
    (0..kabl_modules::builtins::MACROS as u8)
        .filter(|&k| {
            let port = format!("m{}", k + 1);
            k < 4
                || crate::perform::is_pinned(state, id, &port)
                || state.label(id, &format!("name.{port}")).is_some()
                || state.cables.values().any(|c| {
                    matches!(&c.from, kabl_core::PortRef::Module { id: i, port: p }
                        if *i == id && *p == port)
                })
        })
        .collect()
}

/// "Store current": cue `n` takes the value of every macro as `now` gives it, one undo step.
/// Targets of macros that no longer exist are kept out.
pub fn store_macros(
    editor: &mut PatchEditor,
    id: ModuleId,
    n: usize,
    now: &dyn Fn(ModuleId, u8) -> f32,
) {
    let ops: Vec<Op> = editor
        .state()
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "macro")
        .flat_map(|(&module, _)| {
            active_macros(editor.state(), module)
                .into_iter()
                .map(move |k| (module, k))
        })
        .map(|(module, k)| {
            set(
                id,
                key(n, &format!("macro.{module}.{k}")),
                Some(now(module, k).clamp(0.0, 1.0)),
            )
        })
        .collect();
    editor.edit(ops);
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
    if cue.targets.is_empty() && cue.macros.is_empty() {
        return Err(format!(
            "{}: nothing is set, no sequencer or macro",
            cue.name
        ));
    }
    let targets: Vec<(ModuleId, u8)> = cue
        .targets
        .iter()
        .take(MAX_TARGETS)
        .map(|&(s, b)| (s, b as u8))
        .collect();
    let macros: Vec<MacroTarget> = cue
        .macros
        .iter()
        .take(MAX_MACRO_TARGETS)
        .map(|&(id, index, value)| MacroTarget { id, index, value })
        .collect();
    out.push(Command::Launch(
        Launch::new(clock, cue.timing, &targets)
            .with_macros(&macros, cue.morph_beats * PULSES_PER_BEAT),
    ));
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

/// `standing` for the drawn cues: a cue with no sequencer has no banks to show it playing, so
/// it counts as active once it was the last one launched (`last`).
pub fn standing_now(
    cue: &Cue,
    bank_of: &dyn Fn(ModuleId) -> (usize, Option<usize>),
    last: Option<usize>,
) -> (bool, bool) {
    if cue.targets.is_empty() {
        (last == Some(cue.n), false)
    } else {
        standing(cue, bank_of)
    }
}

/// Ops removing every cue and launch-setting reference to module `removed` (a sequencer's cue
/// entries, a cue's or a sequencer's reference clock), for deleting it in the same undo step.
pub fn references_to(state: &PatchState, removed: ModuleId) -> Vec<Op> {
    let mut ops = Vec::new();
    let seq_suffix = format!(".seq.{removed}");
    let macro_part = format!(".macro.{removed}.");
    for (&id, m) in &state.modules {
        for (k, &v) in &m.params {
            let refers = match m.kind.as_str() {
                "cues" => {
                    k.ends_with(&seq_suffix)
                        || k.contains(&macro_part)
                        || (k.ends_with(".clock") && v as ModuleId == removed)
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
