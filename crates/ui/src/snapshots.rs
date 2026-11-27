//! Snapshot recall (FACES.md section 3.2). A recall sets the composite's exposed controls in one
//! undo step, so the document holds the targets; with a morph length and a clock it also asks
//! `control::deliver` to morph the engine there instead of sending the values.
use kabl_core::{CompositeId, Op, ParamTarget, PortRef};
use kabl_engine::morph::{MorphPart, MorphTarget, PART_TARGETS};
use kabl_engine::runtime::{runtime_changes, RuntimeTarget};
use kabl_modules::Taper;

use crate::{cues, panels, routing, PatchEditor, UiState};

/// One recall's morph: the parts `deliver` queues and the targets `sync` must not send.
#[derive(Clone, Debug, PartialEq)]
pub struct MorphRequest {
    pub parts: Vec<MorphPart>,
    pub targets: Vec<RuntimeTarget>,
}

/// Recalls snapshot `snap` of composite `id`: every exposed control the snapshot holds a value
/// for is set to it (clamped to its range, a stepped one to a whole option), as one undo step,
/// and only the controls whose value changes. A control no longer exposed is left alone. The
/// snapshot is lit as the last recalled even when nothing changes. False when it does not exist.
pub fn recall(editor: &mut PatchEditor, view: &mut UiState, id: CompositeId, snap: u64) -> bool {
    let before = editor.state().clone();
    let Some(c) = before.composites.get(&id) else {
        return false;
    };
    let Some(s) = c.snapshots.iter().find(|s| s.id == snap) else {
        return false;
    };
    view.last_snapshot.insert(id, snap);
    let mut ops = Vec::new();
    for (key, &v) in &s.values {
        let Some(e) = c.controls.get(key) else {
            continue;
        };
        let Ok(p) = panels::param_info(&before, &e.target) else {
            continue;
        };
        let (target, now) = match &e.target {
            PortRef::Param { id: leaf, param } => (
                ParamTarget::Module {
                    id: *leaf,
                    param: param.clone(),
                },
                routing::base_value(&before, *leaf, p),
            ),
            PortRef::CableParam { cable, param } => (
                ParamTarget::Cable {
                    id: *cable,
                    param: param.clone(),
                },
                panels::route_amount(&before, *cable),
            ),
            PortRef::Module { .. } => continue,
        };
        // In range and a whole option. Not `from_norm(to_norm(v))`: that round trip moves an
        // exponential value by a few ulps, so a recalled 400 would not read back as 400.
        let to = match p.taper {
            Taper::Stepped => v.round(),
            _ => v,
        }
        .clamp(p.min, p.max);
        if to.to_bits() != now.to_bits() {
            ops.push(Op::SetParam { target, value: to });
        }
    }
    if ops.is_empty() {
        return true;
    }
    editor.edit(ops);
    let clock = s
        .clock
        .filter(|c| before.modules.get(c).is_some_and(|m| m.kind == "clock"));
    let (Some(clock), true) = (clock, s.morph_beats > 0.0) else {
        return true;
    };
    // Each engine target with where it is and where it goes; a change only a compile can make
    // cannot morph, so the recall jumps.
    let after = editor.state();
    let (Some(to), Some(from)) = (
        runtime_changes(&before, after),
        runtime_changes(after, &before),
    ) else {
        return true;
    };
    let targets: Vec<MorphTarget> = to
        .iter()
        .zip(&from)
        .filter(|((a, _), (b, _))| a == b)
        .map(|(&(target, to), &(_, from))| MorphTarget { target, from, to })
        .collect();
    if targets.is_empty() || targets.len() != to.len() {
        return true;
    }
    view.morph_seq = view.morph_seq.wrapping_add(1);
    let n = targets.chunks(PART_TARGETS).len();
    let parts = targets
        .chunks(PART_TARGETS)
        .enumerate()
        .map(|(i, chunk)| {
            let mut part = MorphPart {
                seq: view.morph_seq,
                last: i + 1 == n,
                clock,
                pulses: s.morph_beats.min(cues::MAX_MORPH_BEATS) * cues::PULSES_PER_BEAT,
                count: chunk.len() as u8,
                targets: [chunk[0]; PART_TARGETS],
            };
            part.targets[..chunk.len()].copy_from_slice(chunk);
            part
        })
        .collect();
    view.morphs.push(MorphRequest {
        parts,
        targets: targets.iter().map(|t| t.target).collect(),
    });
    true
}
