//! The drawer's sequencer and cues editors.

use super::*;

/// A seq param's value in the patch (its default when unset).
fn seq_value(state: &PatchState, id: ModuleId, name: &str) -> f32 {
    let info = registry::info_for("seq").and_then(|i| i.params.iter().find(|p| p.name == name));
    state
        .modules
        .get(&id)
        .and_then(|m| m.params.get(name).copied())
        .or(info.map(|p| p.default))
        .unwrap_or(0.0)
}

/// Writes seq params as part of one drag: the first write of a press opens the undo step.
fn seq_write(editor: &mut PatchEditor, ui: &egui::Ui, id: ModuleId, name: &str, v: f32) {
    let key = Id::new(("seq-gesture", id));
    let open = ui.data(|d| d.get_temp::<bool>(key)).unwrap_or(false);
    let target = kabl_core::ParamTarget::Module {
        id,
        param: name.into(),
    };
    editor.set_param_gesture(target, v, !open);
    ui.data_mut(|d| d.insert_temp(key, true));
}

/// A label, a slider and the slider's mono readout, writing `name`.
#[allow(clippy::too_many_arguments)]
fn seq_slider(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    id: ModuleId,
    label: &str,
    name: &str,
    range: std::ops::RangeInclusive<f64>,
    whole: bool,
    readout: String,
) {
    let mut v = f64::from(seq_value(editor.state(), id, name));
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(96.0, st.metrics.slider_h), Sense::hover());
        ui.painter().text(
            r.left_center(),
            egui::Align2::LEFT_CENTER,
            label,
            st.font(style::Role::Body),
            st.roles.text,
        );
        // No slider snap: it would write a fractional value (a face drag leaves one) back rounded.
        let resp = kit::slider(ui, st, &mut v, range, None, &readout);
        ui_state.record(format!("seq-{name}:{id}"), resp.rect);
        if resp.changed() {
            let v = if whole { v.round() } else { v };
            seq_write(editor, ui, id, name, v as f32);
        }
    });
    ui.add_space(10.0);
}

/// A stepped seq param as a labelled segmented row (direction, gate mode).
fn seq_choice(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    st: &Style,
    id: ModuleId,
    label: &str,
    name: &str,
) {
    let Some(labels) = routing::step_labels("seq", name) else {
        return;
    };
    let cur = seq_value(editor.state(), id, name).round() as usize;
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(96.0, st.metrics.control_h), Sense::hover());
        ui.painter().text(
            r.left_center(),
            egui::Align2::LEFT_CENTER,
            label,
            st.font(style::Role::Body),
            st.roles.text,
        );
        kit::segmented(ui, st, |ui| {
            for (k, l) in labels.iter().enumerate() {
                let r = kit::seg(ui, st, &kabl_title(l), k == cur);
                ui_state.record(format!("seq-{name}:{id}.{k}"), r.rect);
                if r.clicked() && k != cur {
                    editor.set_param(id, name, k as f32);
                }
            }
        });
    });
}

/// "FWD" -> "Fwd": the face's caps labels in the drawer's sentence case.
fn kabl_title(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_string() + &c.as_str().to_lowercase())
        .unwrap_or_default()
}

/// The drawer's sequencer editor (G4 `seq-pattern-editor`): header, the bank being edited and
/// its menu, what plays and the launch quantise, the step grid of the edited bank (pitch, gate,
/// velocity, probability, ratchet) with the playhead, then length, gate length, transpose,
/// direction and gate mode.
pub(crate) fn seq_panel(
    editor: &mut PatchEditor,
    ui_state: &mut UiState,
    ui: &mut egui::Ui,
    id: ModuleId,
    edit: usize,
) {
    let st = ui_state.style.clone();
    let st = &*st;
    if !ui.input(|i| i.pointer.any_down()) {
        ui.data_mut(|d| d.insert_temp(Id::new(("seq-gesture", id)), false));
    }
    // Header: title, kind chip, Explain.
    ui.horizontal(|ui| {
        kit::label(ui, st, style::Role::H3, kit::Tone::Text, "Sequencer");
        kit::tag(ui, st, &format!("seq #{id}"), st.roles.text2);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let r = ui.add(
                kit::Button::new(st, "Explain")
                    .ghost()
                    .small()
                    .tone(kit::Tone::Accent),
            );
            ui_state.record(format!("explain-module:{id}"), r.rect);
            if r.clicked() {
                explain::open(ui_state, explain::Subject::Module(id), None);
            }
        });
    });
    ui.add_space(st.sp(2));
    // Edit A-D and the bank's menu.
    ui.horizontal(|ui| {
        kit::section(ui, st, "Edit");
        kit::segmented(ui, st, |ui| {
            for b in 0..seq::BANKS {
                let r = kit::seg(ui, st, seq::BANK_NAMES[b], b == edit);
                ui_state.record(format!("drawer-bank:{id}.{b}"), r.rect);
                if r.clicked() {
                    ui_state.edit_bank.insert(id, b);
                }
            }
        });
        let r = kit::icon_menu(ui, st, kit::Ic::More, |ui| {
            bank_menu(editor, ui_state, ui, id, edit)
        });
        ui_state.record(format!("seq-bank-menu:{id}"), r.rect);
    });
    // What plays, and the launch settings behind the quantise label.
    let state = editor.state();
    let (playing, queued) = ui_state
        .seq_banks
        .get(&id)
        .copied()
        .unwrap_or((banks::startup(state, id), None));
    let mut line = format!("Playing {}", banks::title(state, id, playing));
    if let Some(q) = queued {
        line += &format!(" · queued {}", banks::title(state, id, q));
    }
    let timing = banks::timing(state, id);
    let timing_name = banks::TIMINGS
        .iter()
        .find(|t| t.0 == timing)
        .map_or("", |t| t.1);
    let (mut pick, mut tpick) = (None, None);
    ui.horizontal(|ui| {
        kit::label(ui, st, style::Role::Value, kit::Tone::Text2, &line);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let r = ui.add(kit::Button::new(st, timing_name).ghost().small());
            ui_state.record(format!("seq-launch:{id}"), r.rect);
            let dot = pos2(r.rect.left() - 4.0, r.rect.center().y);
            ui.painter().circle_filled(dot, 3.5, st.roles.accent);
            egui::Popup::menu(&r)
                .frame(kit::popover_frame(st))
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .width(220.0)
                .show(|ui| {
                    kit::section(ui, st, "Launch");
                    for (i, (t, name)) in banks::TIMINGS.iter().enumerate() {
                        let r = kit::menu_item(ui, st, name, *t == timing);
                        ui_state.record(format!("menu:seq-timing.{i}"), r.rect);
                        if r.clicked() {
                            tpick = Some(i);
                            ui.close();
                        }
                    }
                    kit::menu_rule(ui, st);
                    kit::section(ui, st, "Launch on");
                    let chosen = state
                        .modules
                        .get(&id)
                        .and_then(|m| m.params.get(banks::LAUNCH_CLOCK))
                        .map(|&v| v as ModuleId);
                    let r = kit::menu_item(ui, st, "The patched clock", chosen.is_none());
                    ui_state.record("menu:seq-clock.patched".into(), r.rect);
                    if r.clicked() {
                        pick = Some(None);
                        ui.close();
                    }
                    for (&c, _) in state.modules.iter().filter(|(_, m)| m.kind == "clock") {
                        let r = kit::menu_item(ui, st, &format!("Clock #{c}"), chosen == Some(c));
                        ui_state.record(format!("menu:seq-clock.{c}"), r.rect);
                        if r.clicked() {
                            pick = Some(Some(c as f32));
                            ui.close();
                        }
                    }
                });
        });
    });
    if let Some(v) = pick {
        editor.set_presentation(&[(id, banks::LAUNCH_CLOCK.into(), v)]);
    }
    if let Some(i) = tpick {
        let v = (i != 2).then_some(i as f32);
        editor.set_presentation(&[(id, banks::LAUNCH_TIMING.into(), v)]);
    }
    ui.add_space(st.sp(1));

    // The step grid of the edited bank. Values are normalised for the grid and written back
    // in the params' own units.
    let pre = ["", "b.", "c.", "d."][edit];
    let name = |k: &str, i: usize| format!("{pre}{k}{}", i + 1);
    let state = editor.state();
    let lane = |k: &str, scale: f32| -> Vec<f32> {
        (0..seq::STEPS)
            .map(|i| seq_value(state, id, &name(k, i)) / scale)
            .collect()
    };
    let (mut pitch, mut gate, mut vel, mut prob, mut ratch) = (
        lane("p", 24.0),
        lane("g", 1.0),
        lane("v", 100.0),
        lane("r", 100.0),
        lane("k", 1.0),
    );
    let before = [
        pitch.clone(),
        gate.clone(),
        vel.clone(),
        prob.clone(),
        ratch.clone(),
    ];
    let r = &st.roles;
    let mut lanes = [
        kit::g4::Lane {
            unit: "st",
            color: Some(r.pitch),
            height: Some(86.0),
            readout: Some(|v| format!("{:+}", (v * 24.0).round() as i32).replace("+0", "0")),
            ..kit::g4::Lane::new(
                "Pitch",
                kit::g4::LaneKind::Bars { bipolar: true },
                &mut pitch,
            )
        },
        kit::g4::Lane {
            color: Some(r.gate),
            ..kit::g4::Lane::new("Gate", kit::g4::LaneKind::Gate, &mut gate)
        },
        kit::g4::Lane {
            color: Some(r.gate),
            ..kit::g4::Lane::new("Vel", kit::g4::LaneKind::Bars { bipolar: false }, &mut vel)
        },
        kit::g4::Lane::new(
            "Prob",
            kit::g4::LaneKind::Bars { bipolar: false },
            &mut prob,
        ),
        kit::g4::Lane::new("Ratchet", kit::g4::LaneKind::Count { max: 4 }, &mut ratch),
    ];
    let playhead = ui_state.seq_steps.get(&id).copied();
    let w = ui.available_width();
    let resp = kit::g4::step_grid(ui, st, &mut lanes, playhead, w, true);
    ui_state.record(format!("seq-grid:{id}"), resp.rect);
    if resp.changed() {
        let after = [pitch, gate, vel, prob, ratch];
        for ((k, scale), (a, b)) in [
            ("p", 24.0),
            ("g", 1.0),
            ("v", 100.0),
            ("r", 100.0),
            ("k", 1.0),
        ]
        .into_iter()
        .zip(before.iter().zip(after.iter()))
        {
            for i in 0..seq::STEPS {
                if a[i] != b[i] {
                    let v = b[i] * scale;
                    let v = if k == "p" { v.round() } else { v };
                    seq_write(editor, ui, id, &name(k, i), v);
                }
            }
        }
    }
    ui.add_space(st.sp(2));
    kit::rule(ui, st);
    ui.add_space(st.sp(2));
    let len = seq_value(editor.state(), id, &format!("{pre}length"));
    seq_slider(
        editor,
        ui_state,
        ui,
        st,
        id,
        "Length",
        &format!("{pre}length"),
        1.0..=seq::STEPS as f64,
        true,
        format!("{len:.0}"),
    );
    let gl = seq_value(editor.state(), id, &format!("{pre}gate_len"));
    seq_slider(
        editor,
        ui_state,
        ui,
        st,
        id,
        "Gate length",
        &format!("{pre}gate_len"),
        1.0..=100.0,
        false,
        format!("{gl:.0} %"),
    );
    let tr = seq_value(editor.state(), id, "transpose");
    seq_slider(
        editor,
        ui_state,
        ui,
        st,
        id,
        "Transpose",
        "transpose",
        -24.0..=24.0,
        true,
        format!("{tr:+.0} st"),
    );
    ui.add_space(st.sp(1));
    seq_choice(editor, ui_state, ui, st, id, "Direction", "direction");
    seq_choice(
        editor,
        ui_state,
        ui,
        st,
        id,
        "Gate mode",
        &format!("{pre}gate_mode"),
    );
    ui.add_space(st.sp(3));
    kit::rule(ui, st);
    ui.add_space(st.sp(2));
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
