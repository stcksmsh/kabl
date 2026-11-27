//! Snapshot recall (FACES.md section 3.2). A recall sets the composite's exposed controls in one
//! undo step, so the document holds the targets; with a morph length and a clock it also asks
//! `control::deliver` to morph the engine there instead of sending the values.
use kabl_core::{CompositeId, Op, ParamTarget, PatchState, PortRef};
use kabl_engine::morph::{MorphPart, MorphTarget, PART_TARGETS};
use kabl_engine::runtime::{runtime_changes, RuntimeTarget};
use kabl_modules::Taper;

use crate::{cues, panels, routing, PatchEditor, UiState};

/// The param control exposure `target` aliases, its document value and its info.
pub(crate) fn control(
    state: &PatchState,
    target: &PortRef,
) -> Option<(ParamTarget, f32, &'static kabl_modules::ParamInfo)> {
    let p = panels::param_info(state, target).ok()?;
    Some(match target {
        PortRef::Param { id, param } => (
            ParamTarget::Module {
                id: *id,
                param: param.clone(),
            },
            routing::base_value(state, *id, p),
            p,
        ),
        PortRef::CableParam { cable, param } => (
            ParamTarget::Cable {
                id: *cable,
                param: param.clone(),
            },
            panels::route_amount(state, *cable),
            p,
        ),
        PortRef::Module { .. } => return None,
    })
}

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
        let Some((target, now, p)) = control(&before, &e.target) else {
            continue;
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
        .map(|(&(target, to), &(_, from))| {
            // A morph already moving the target: the new one starts where it is.
            let from = view
                .live_param(&target)
                .filter(|l| l.progress < 1.0)
                .map_or(from, |l| l.value);
            MorphTarget { target, from, to }
        })
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

/// The design page's snapshot list (FACES.md section 7): capture, re-capture, rename, morph
/// length and clock, delete, each one draft step; Recall is a project edit, offered once Apply
/// has written the snapshot.
pub(crate) fn design_list(
    editor: &mut PatchEditor,
    view: &mut UiState,
    ui: &mut egui::Ui,
    st: &crate::style::Style,
    page: &mut crate::design_page::Page,
    rect: egui::Rect,
) {
    use crate::kit;
    let clocks: Vec<kabl_core::ModuleId> = editor
        .state()
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "clock")
        .map(|(&k, _)| k)
        .collect();
    let applied: Vec<u64> = editor
        .state()
        .composites
        .get(&page.id)
        .map(|c| c.snapshots.iter().map(|s| s.id).collect())
        .unwrap_or_default();
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(12.0)), |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);
        ui.horizontal(|ui| {
            kit::section(ui, st, "Snapshots");
            let r = ui.add(kit::Button::new(st, "Capture").small());
            view.record("design:snap:capture".into(), r.rect);
            if r.clicked() {
                // A refusal is already in `page.note`, which the inspector shows.
                let _ = page.capture(editor.state());
            }
        });
        egui::ScrollArea::vertical()
            .id_salt("design-snapshots")
            .show(ui, |ui| {
                for s in page.draft.snapshots.clone() {
                    ui.horizontal(|ui| {
                        let key = format!("design:snap:name:{}", s.id);
                        let (r, name) =
                            crate::design_page::text_field(ui, st, &key, &s.name, 180.0);
                        view.record(key, r.rect);
                        if let Some(n) = name.filter(|n| *n != s.name) {
                            // Refusals land in `page.note`, as for Capture.
                            let _ = page.rename(s.id, &n);
                        }
                        let key = format!("design:snap:beats:{}", s.id);
                        let (r, beats) = crate::design_page::num_field(ui, st, &key, s.morph_beats);
                        view.record(key, r.rect);
                        if let Some(b) = beats {
                            let _ = page.set_morph(editor.state(), s.id, b, s.clock);
                        }
                        ui.label("beats");
                        let current = s.clock.map_or("no clock".into(), |k| format!("Clock #{k}"));
                        let mut clock = None;
                        let r =
                            kit::dropdown(ui, st, ("snap-clock", s.id), &current, 130.0, |ui| {
                                if kit::menu_item(ui, st, "no clock", s.clock.is_none()).clicked() {
                                    clock = Some(None);
                                    ui.close();
                                }
                                for &k in &clocks {
                                    if kit::menu_item(
                                        ui,
                                        st,
                                        &format!("Clock #{k}"),
                                        s.clock == Some(k),
                                    )
                                    .clicked()
                                    {
                                        clock = Some(Some(k));
                                        ui.close();
                                    }
                                }
                            });
                        view.record(format!("design:snap:clock:{}", s.id), r.rect);
                        if let Some(c) = clock {
                            let _ = page.set_morph(editor.state(), s.id, s.morph_beats, c);
                        }
                        let r = ui.add(kit::Button::new(st, "Re-capture").ghost().small());
                        view.record(format!("design:snap:recapture:{}", s.id), r.rect);
                        if r.clicked() {
                            page.recapture(editor.state(), s.id);
                        }
                        let r = ui.add(
                            kit::Button::new(st, "Recall")
                                .ghost()
                                .small()
                                .enabled(applied.contains(&s.id)),
                        );
                        view.record(format!("design:snap:recall:{}", s.id), r.rect);
                        if r.clicked() {
                            recall(editor, view, page.id, s.id);
                        }
                        let r = ui.add(
                            kit::Button::new(st, "Delete")
                                .ghost()
                                .small()
                                .tone(kit::Tone::Bad),
                        );
                        view.record(format!("design:snap:delete:{}", s.id), r.rect);
                        if r.clicked() {
                            page.delete_snapshot(s.id);
                        }
                    });
                }
            });
    });
}
