//! The drawer's sequencer and cues editors.

use super::*;

/// The drawer's sequencer header: which bank the controls below edit, bank edits, and the
/// sequencer's own launch settings.
pub(crate) fn seq_panel(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    id: ModuleId,
    edit: usize,
) {
    let state = editor.state();
    let (playing, queued) = ui_state
        .seq_banks
        .get(&id)
        .copied()
        .unwrap_or((banks::startup(state, id), None));
    ui.horizontal_wrapped(|ui| {
        ui.label("Editing");
        for b in 0..seq::BANKS {
            let r = ui.selectable_label(b == edit, banks::title(state, id, b));
            ui_state.record(format!("drawer-bank:{id}.{b}"), r.rect);
            if r.clicked() {
                ui_state.edit_bank.insert(id, b);
            }
        }
    });
    ui.label(
        egui::RichText::new(format!(
            "Playing {}{}{}",
            banks::title(state, id, playing),
            queued.map_or(String::new(), |q| format!(
                " · queued {}",
                banks::title(state, id, q)
            )),
            if banks::startup(state, id) == playing {
                " · startup".to_string()
            } else {
                format!(
                    " · startup {}",
                    banks::title(state, id, banks::startup(state, id))
                )
            }
        ))
        .small(),
    );
    ui.menu_button(format!("Bank {} ↓", seq::BANK_NAMES[edit]), |ui| {
        bank_menu(editor, ui_state, ui, id, edit)
    });
    let (mut pick, mut tpick) = (None, None);
    ui.horizontal_wrapped(|ui| {
        let state = editor.state();
        ui.label("Launch on");
        let clocks: Vec<ModuleId> = state
            .modules
            .iter()
            .filter(|(_, m)| m.kind == "clock")
            .map(|(&c, _)| c)
            .collect();
        let chosen = state
            .modules
            .get(&id)
            .and_then(|m| m.params.get(banks::LAUNCH_CLOCK))
            .map(|&v| v as ModuleId);
        let current = banks::reference_clock(state, id);
        let text = match (current, chosen) {
            (Some(c), Some(_)) => format!("Clock #{c}"),
            (Some(c), None) => format!("Clock #{c} (patched)"),
            (None, _) => "no clock".into(),
        };
        egui::ComboBox::from_id_salt(("seq-clock", id))
            .selected_text(text)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(chosen.is_none(), "The patched clock")
                    .clicked()
                {
                    pick = Some(None);
                }
                for c in clocks {
                    if ui
                        .selectable_label(chosen == Some(c), format!("Clock #{c}"))
                        .clicked()
                    {
                        pick = Some(Some(c as f32));
                    }
                }
            });
        let timing = banks::timing(state, id);
        egui::ComboBox::from_id_salt(("seq-timing", id))
            .selected_text(
                banks::TIMINGS
                    .iter()
                    .find(|t| t.0 == timing)
                    .map_or("", |t| t.1),
            )
            .show_ui(ui, |ui| {
                for (i, (t, name)) in banks::TIMINGS.iter().enumerate() {
                    if ui.selectable_label(*t == timing, *name).clicked() {
                        tpick = Some(i);
                    }
                }
            });
    });
    if let Some(v) = pick {
        editor.set_presentation(&[(id, banks::LAUNCH_CLOCK.into(), v)]);
    }
    if let Some(i) = tpick {
        let v = (i != 2).then_some(i as f32);
        editor.set_presentation(&[(id, banks::LAUNCH_TIMING.into(), v)]);
    }
    ui.separator();
}

/// The drawer's cue editor: per cue, its name, reference clock, timing and a bank (or Keep)
/// per sequencer; add, capture what plays, delete.
pub(crate) fn cues_panel(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    id: ModuleId,
) {
    let state = editor.state();
    let all = cues::cues(state, id);
    let clocks: Vec<ModuleId> = state
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "clock")
        .map(|(&c, _)| c)
        .collect();
    let seqs: Vec<ModuleId> = state
        .modules
        .iter()
        .filter(|(_, m)| matches!(m.kind.as_str(), "seq" | "drum.grid"))
        .map(|(&c, _)| c)
        .collect();
    let is_grid = |s: ModuleId| state.modules.get(&s).is_some_and(|m| m.kind == "drum.grid");
    let playing_now: HashMap<ModuleId, usize> = seqs
        .iter()
        .map(|&s| {
            (
                s,
                ui_state.seq_banks.get(&s).map_or_else(
                    || {
                        if is_grid(s) {
                            state.modules[&s]
                                .params
                                .get("pattern")
                                .map_or(0, |&v| v.round() as usize)
                        } else {
                            banks::startup(state, s)
                        }
                    },
                    |b| b.0,
                ),
            )
        })
        .collect();
    enum Edit {
        Add,
        Delete(usize),
        Capture(usize),
        Clock(usize, ModuleId),
        Timing(usize, kabl_engine::patch_engine::Timing),
        Target(usize, ModuleId, Option<usize>),
        Rename(usize, String),
        Macro(usize, ModuleId, u8, Option<f32>),
        Morph(usize, f32),
        StoreMacros(usize),
    }
    let macro_mods: Vec<ModuleId> = state
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "macro")
        .map(|(&c, _)| c)
        .collect();
    let mut edit = None;
    for c in &all {
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("{}", c.n)).strong());
            let mut name = ui_state
                .cue_rename
                .as_ref()
                .filter(|(i, n, _)| (*i, *n) == (id, c.n))
                .map_or_else(|| c.name.clone(), |(_, _, t)| t.clone());
            let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(120.0));
            ui_state.record(format!("cue-name:{id}.{}", c.n), r.rect);
            if r.changed() {
                ui_state.cue_rename = Some((id, c.n, name.clone()));
            }
            if r.lost_focus() {
                ui_state.cue_rename = None;
                if name != c.name {
                    edit = Some(Edit::Rename(c.n, name));
                }
            }
            let r = ui.small_button("Capture");
            ui_state.record(format!("cue-capture:{id}.{}", c.n), r.rect);
            if r.on_hover_text("Set every sequencer to the bank it plays now")
                .clicked()
            {
                edit = Some(Edit::Capture(c.n));
            }
            let r = ui.small_button("Delete");
            ui_state.record(format!("cue-delete:{id}.{}", c.n), r.rect);
            if r.clicked() {
                edit = Some(Edit::Delete(c.n));
            }
        });
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt(("cue-clock", id, c.n))
                .selected_text(
                    c.clock
                        .map_or("choose clock".into(), |k| format!("Clock #{k}")),
                )
                .show_ui(ui, |ui| {
                    for &k in &clocks {
                        if ui
                            .selectable_label(c.clock == Some(k), format!("Clock #{k}"))
                            .clicked()
                        {
                            edit = Some(Edit::Clock(c.n, k));
                        }
                    }
                });
            egui::ComboBox::from_id_salt(("cue-timing", id, c.n))
                .selected_text(
                    banks::TIMINGS
                        .iter()
                        .find(|t| t.0 == c.timing)
                        .map_or("", |t| t.1),
                )
                .show_ui(ui, |ui| {
                    for (t, name) in banks::TIMINGS {
                        if ui.selectable_label(c.timing == t, name).clicked() {
                            edit = Some(Edit::Timing(c.n, t));
                        }
                    }
                });
        });
        for &s in &seqs {
            ui.horizontal(|ui| {
                let grid = is_grid(s);
                let label = if grid { "Drum grid" } else { "Sequencer" };
                ui.label(egui::RichText::new(format!("{label} #{s}")).small());
                let current = c.targets.iter().find(|t| t.0 == s).map(|t| t.1);
                let r = ui.selectable_label(current.is_none(), "Keep");
                ui_state.record(format!("cue-bank:{id}.{}.{s}.keep", c.n), r.rect);
                if r.on_hover_text("This cue leaves it playing").clicked() {
                    edit = Some(Edit::Target(c.n, s, None));
                }
                let names: &[&str] = if grid {
                    &kabl_modules::builtins::PATTERN_NAMES
                } else {
                    &seq::BANK_NAMES
                };
                for (b, &name) in names.iter().enumerate() {
                    let hover = if grid {
                        format!("Pattern {name}")
                    } else {
                        banks::title(state, s, b)
                    };
                    let r = ui
                        .selectable_label(current == Some(b), name)
                        .on_hover_text(hover);
                    ui_state.record(format!("cue-bank:{id}.{}.{s}.{b}", c.n), r.rect);
                    if r.clicked() {
                        edit = Some(Edit::Target(c.n, s, Some(b)));
                    }
                }
            });
        }
        if !macro_mods.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("Macros").small());
                for &mm in &macro_mods {
                    for k in cues::active_macros(state, mm) {
                        let set = c.macros.iter().find(|t| (t.0, t.1) == (mm, k)).map(|t| t.2);
                        let name = state
                            .label(mm, &format!("name.m{}", k + 1))
                            .map_or_else(|| format!("m{}", k + 1), str::to_string);
                        let r = ui.selectable_label(set.is_some(), name).on_hover_text(
                            "Tick to set a target for this macro; unticked leaves it",
                        );
                        ui_state.record(format!("cue-macro:{id}.{}.{mm}.{k}", c.n), r.rect);
                        if r.clicked() {
                            let v = set.is_none().then(|| ui_state.macro_now(state, mm, k));
                            edit = Some(Edit::Macro(c.n, mm, k, v));
                        }
                        if let Some(mut v) = set {
                            let d = ui.add(
                                egui::DragValue::new(&mut v)
                                    .range(0.0..=1.0)
                                    .speed(0.005)
                                    .fixed_decimals(2),
                            );
                            ui_state
                                .record(format!("cue-macro-value:{id}.{}.{mm}.{k}", c.n), d.rect);
                            if d.changed() {
                                edit = Some(Edit::Macro(c.n, mm, k, Some(v)));
                            }
                        }
                    }
                }
            });
            ui.horizontal(|ui| {
                let mut beats = c.morph_beats;
                ui.label(egui::RichText::new("Morph").small());
                let d = ui.add(
                    egui::DragValue::new(&mut beats)
                        .range(0.0..=cues::MAX_MORPH_BEATS)
                        .speed(0.1)
                        .fixed_decimals(1)
                        .suffix(" beats"),
                );
                ui_state.record(format!("cue-morph:{id}.{}", c.n), d.rect);
                d.on_hover_text(
                    "Time the macros take to reach this cue's values, on the clock; 0 jumps",
                );
                if beats != c.morph_beats {
                    edit = Some(Edit::Morph(c.n, beats));
                }
                let r = ui.small_button("Store macros");
                ui_state.record(format!("cue-store-macros:{id}.{}", c.n), r.rect);
                if r.on_hover_text("Set every macro to where it is now")
                    .clicked()
                {
                    edit = Some(Edit::StoreMacros(c.n));
                }
            });
        }
    }
    ui.separator();
    let r = ui.add_enabled(
        all.len() < kabl_modules::builtins::CUES,
        egui::Button::new("Add cue (from what plays)"),
    );
    ui_state.record(format!("cue-add:{id}"), r.rect);
    if r.clicked() {
        edit = Some(Edit::Add);
    }
    match edit {
        Some(Edit::Add) => {
            cues::add(editor, id, &|s| playing_now.get(&s).copied().unwrap_or(0));
        }
        Some(Edit::Delete(n)) => cues::delete(editor, id, n),
        Some(Edit::Capture(n)) => {
            let ops = seqs
                .iter()
                .map(|&s| kabl_core::Op::SetParam {
                    target: kabl_core::ParamTarget::Module {
                        id,
                        param: format!("cue{n}.seq.{s}"),
                    },
                    value: playing_now[&s] as f32,
                })
                .collect();
            editor.edit(ops);
        }
        Some(Edit::Clock(n, k)) => cues::set_clock(editor, id, n, k),
        Some(Edit::Timing(n, t)) => cues::set_timing(editor, id, n, t),
        Some(Edit::Target(n, s, b)) => cues::set_target(editor, id, n, s, b),
        Some(Edit::Rename(n, t)) => cues::rename(editor, id, n, &t),
        Some(Edit::Macro(n, m, k, v)) => cues::set_macro(editor, id, n, m, k, v),
        Some(Edit::Morph(n, b)) => cues::set_morph(editor, id, n, b),
        Some(Edit::StoreMacros(n)) => {
            let state = editor.state().clone();
            cues::store_macros(editor, id, n, &|m, k| ui_state.macro_now(&state, m, k));
        }
        None => {}
    }
}
